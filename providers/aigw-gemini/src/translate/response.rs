//! Response translation: Gemini [`GenerateContentResponse`] → canonical
//! [`ChatResponse`].
//!
//! Gemini bundles thinking blocks, text, and function calls as separate
//! `Part`s on the same `Content` object. The translator splits them:
//! - `text` parts (non-thought) → joined into a single text segment.
//! - `text` parts with `thought=true` → canonical thinking content parts
//!   (with [`ThinkingSource::Gemini`]).
//! - `function_call` parts → canonical tool calls.
//!
//! When thinking parts are present the message body becomes
//! [`MessageContent::Parts`] so consumers can replay the thinking history
//! on the next turn (Gemini requires `thought_signature` round-tripping).
//!
//! [`GenerateContentResponse`]: crate::types::GenerateContentResponse
//! [`ChatResponse`]: aigw_core::model::ChatResponse
//! [`MessageContent::Parts`]: aigw_core::model::MessageContent::Parts
//! [`ThinkingSource::Gemini`]: aigw_core::model::ThinkingSource::Gemini

use aigw_core::ForwardCompatible;
use aigw_core::error::{ProviderError, TranslateError, map_error_status};
use aigw_core::model::{
    ChatResponse, Choice, ContentPart, FinishReason as CanonicalFinishReason, FunctionCall,
    Message, MessageContent, Role, ThinkingSource, ToolCall, TypedContentPart, Usage,
};
use aigw_core::translate::{ResponseTranslator, StreamParser};
use http::{HeaderMap, StatusCode};

use super::stream::GeminiStreamParser;
use crate::types::{
    Candidate, FinishReason as NativeFinishReason, GenerateContentResponse, GoogleErrorResponse,
    Part,
};

/// Translates Gemini responses into canonical types.
pub struct GeminiResponseTranslator;

impl ResponseTranslator for GeminiResponseTranslator {
    fn translate_response(
        &self,
        _status: StatusCode,
        body: &[u8],
    ) -> Result<ChatResponse, TranslateError> {
        let native: GenerateContentResponse = serde_json::from_slice(body)?;
        Ok(native_to_canonical(native))
    }

    fn stream_parser(&self) -> Box<dyn StreamParser> {
        Box::new(GeminiStreamParser::new())
    }

    fn translate_error(
        &self,
        status: StatusCode,
        headers: &HeaderMap,
        body: &[u8],
    ) -> ProviderError {
        let parsed = serde_json::from_slice::<GoogleErrorResponse>(body);
        let message = parsed
            .map(|e| e.error.message)
            .unwrap_or_else(|_| String::from_utf8_lossy(body).into_owned());
        map_error_status(status.as_u16(), headers, message)
    }
}

/// Convert a complete native response to canonical [`ChatResponse`].
///
/// Pulled out of the translator so it can be shared with the stream parser
/// when finalising at end-of-stream.
pub(crate) fn native_to_canonical(native: GenerateContentResponse) -> ChatResponse {
    let id = native.response_id.unwrap_or_default();
    let model = native.model_version.unwrap_or_default();

    let usage = native.usage_metadata.map(|u| Usage {
        prompt_tokens: u.prompt_token_count,
        completion_tokens: u.candidates_token_count,
        total_tokens: u.total_token_count,
        extra: {
            let mut extra = serde_json::Map::new();
            if let Some(t) = u.cached_content_token_count {
                extra.insert(
                    "cached_content_token_count".into(),
                    serde_json::Value::Number(t.into()),
                );
            }
            if let Some(t) = u.thoughts_token_count {
                extra.insert(
                    "thoughts_token_count".into(),
                    serde_json::Value::Number(t.into()),
                );
            }
            extra
        },
    });

    let choices: Vec<Choice> = native
        .candidates
        .into_iter()
        .enumerate()
        .map(|(i, c)| candidate_to_choice(i as u32, c))
        .collect();

    ChatResponse {
        id,
        object: "chat.completion".to_owned(),
        created: aigw_core::unix_now(),
        model,
        choices,
        usage,
        extra: serde_json::Map::new(),
    }
}

fn candidate_to_choice(index: u32, c: Candidate) -> Choice {
    let finish_reason = c.finish_reason.map(map_finish_reason);
    let parts = c.content.map(|cnt| cnt.parts).unwrap_or_default();
    let (content, tool_calls) = split_parts(parts);
    Choice {
        index,
        message: Message {
            role: Role::Assistant,
            content,
            name: None,
            tool_call_id: None,
            tool_calls,
            extra: Default::default(),
        },
        finish_reason,
        extra: Default::default(),
    }
}

/// Split a Gemini Part array into a canonical `MessageContent` and a
/// vector of tool calls.
///
/// - `text` parts (non-thought) are joined into a single text segment.
/// - `text` parts with `thought == Some(true)` become
///   [`TypedContentPart::Thinking`].
/// - `function_call` parts become canonical [`ToolCall`]s.
///
/// When no thinking parts are present the function preserves the legacy
/// `MessageContent::Text` shape; otherwise it returns
/// `MessageContent::Parts` so thinking history round-trips.
pub(crate) fn split_parts(parts: Vec<Part>) -> (Option<MessageContent>, Option<Vec<ToolCall>>) {
    let mut thinking_parts: Vec<ContentPart> = Vec::new();
    let mut text_pieces: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for part in parts {
        let is_thought = part.thought.unwrap_or(false);
        if let Some(fc) = part.function_call {
            let id = fc
                .id
                .unwrap_or_else(|| format!("call_{}", tool_calls.len()));
            let arguments = serde_json::to_string(&fc.args).unwrap_or_else(|_| "{}".to_owned());
            tool_calls.push(ToolCall {
                id,
                kind: "function".to_owned(),
                function: FunctionCall {
                    name: fc.name,
                    arguments,
                    extra: Default::default(),
                },
                extra: Default::default(),
            });
        } else if is_thought {
            // Thought part: surface as canonical thinking content.
            thinking_parts.push(ForwardCompatible::Known(TypedContentPart::Thinking {
                thinking: part.text.unwrap_or_default(),
                signature: part.thought_signature.unwrap_or_default(),
                source: Some(ThinkingSource::Gemini),
                extra: Default::default(),
            }));
        } else if let Some(t) = part.text {
            text_pieces.push(t);
        }
        // inline_data / file_data / executable_code / etc.: not surfaced in
        // the canonical message body for now.
    }

    let content = if thinking_parts.is_empty() {
        if text_pieces.is_empty() {
            None
        } else {
            Some(MessageContent::Text(text_pieces.join("")))
        }
    } else {
        let mut all = thinking_parts;
        if !text_pieces.is_empty() {
            all.push(ForwardCompatible::Known(TypedContentPart::Text {
                text: text_pieces.join(""),
                extra: Default::default(),
            }));
        }
        Some(MessageContent::Parts(all))
    };

    let tool_calls_opt = if tool_calls.is_empty() {
        None
    } else {
        Some(tool_calls)
    };

    (content, tool_calls_opt)
}

/// Map Gemini's wire finish reason to canonical.
pub(crate) fn map_finish_reason(reason: NativeFinishReason) -> CanonicalFinishReason {
    match reason {
        NativeFinishReason::Stop => CanonicalFinishReason::Stop,
        NativeFinishReason::MaxTokens => CanonicalFinishReason::Length,
        NativeFinishReason::Safety
        | NativeFinishReason::Recitation
        | NativeFinishReason::Blocklist
        | NativeFinishReason::ProhibitedContent
        | NativeFinishReason::Spii
        | NativeFinishReason::ImageSafety => CanonicalFinishReason::ContentFilter,
        NativeFinishReason::Language
        | NativeFinishReason::Other
        | NativeFinishReason::MalformedFunctionCall
        | NativeFinishReason::NoImage => CanonicalFinishReason::Stop,
        NativeFinishReason::Unknown(s) => CanonicalFinishReason::Unknown(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_text_response() {
        let json = r#"{
            "candidates": [{
                "content": { "role": "model", "parts": [{ "text": "Hello!" }] },
                "finishReason": "STOP"
            }],
            "modelVersion": "gemini-2.5-flash",
            "responseId": "resp-1",
            "usageMetadata": { "promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15 }
        }"#;
        let translator = GeminiResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();
        assert_eq!(resp.id, "resp-1");
        assert_eq!(resp.model, "gemini-2.5-flash");
        assert_eq!(resp.choices.len(), 1);
        let choice = &resp.choices[0];
        match &choice.message.content {
            Some(MessageContent::Text(s)) => assert_eq!(s, "Hello!"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(choice.finish_reason, Some(CanonicalFinishReason::Stop));
        let usage = resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.completion_tokens, Some(5));
    }

    #[test]
    fn translate_tool_call_response() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        { "functionCall": { "name": "get_weather", "args": {"location":"SF"}, "id": "fc1" } }
                    ]
                },
                "finishReason": "STOP"
            }],
            "modelVersion": "gemini-2.5-pro",
            "responseId": "r1"
        }"#;
        let translator = GeminiResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();
        let choice = &resp.choices[0];
        let tc = choice.message.tool_calls.as_ref().unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].id, "fc1");
        assert_eq!(tc[0].function.name, "get_weather");
        assert_eq!(tc[0].function.arguments, r#"{"location":"SF"}"#);
    }

    #[test]
    fn translate_thinking_response_emits_parts() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        { "text": "thinking through...", "thought": true, "thoughtSignature": "sig123" },
                        { "text": "The answer is 42." }
                    ]
                },
                "finishReason": "STOP"
            }],
            "modelVersion": "gemini-2.5-flash",
            "responseId": "r2"
        }"#;
        let translator = GeminiResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();
        let parts = match resp.choices[0].message.content.as_ref().unwrap() {
            MessageContent::Parts(p) => p,
            other => panic!("expected Parts, got {other:?}"),
        };
        assert_eq!(parts.len(), 2);
        match &parts[0] {
            ContentPart::Known(TypedContentPart::Thinking {
                thinking,
                signature,
                source,
                ..
            }) => {
                assert_eq!(thinking, "thinking through...");
                assert_eq!(signature, "sig123");
                assert_eq!(*source, Some(ThinkingSource::Gemini));
            }
            other => panic!("expected Thinking, got {other:?}"),
        }
        match &parts[1] {
            ContentPart::Known(TypedContentPart::Text { text, .. }) => {
                assert_eq!(text, "The answer is 42.");
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn translate_max_tokens_finish_reason() {
        let json = r#"{
            "candidates": [{
                "content": { "role": "model", "parts": [{ "text": "..." }] },
                "finishReason": "MAX_TOKENS"
            }],
            "responseId": "r3"
        }"#;
        let translator = GeminiResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();
        assert_eq!(
            resp.choices[0].finish_reason,
            Some(CanonicalFinishReason::Length)
        );
    }

    #[test]
    fn translate_safety_finish_reason_maps_to_content_filter() {
        let json = r#"{
            "candidates": [{
                "content": { "role": "model", "parts": [] },
                "finishReason": "SAFETY"
            }],
            "responseId": "r4"
        }"#;
        let translator = GeminiResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();
        assert_eq!(
            resp.choices[0].finish_reason,
            Some(CanonicalFinishReason::ContentFilter)
        );
    }

    #[test]
    fn translate_response_includes_thoughts_token_count() {
        let json = r#"{
            "candidates": [{ "content": { "role": "model", "parts": [{"text":"."}] } }],
            "responseId": "r5",
            "usageMetadata": {
                "promptTokenCount": 100,
                "candidatesTokenCount": 50,
                "totalTokenCount": 150,
                "thoughtsTokenCount": 25
            }
        }"#;
        let translator = GeminiResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();
        let usage = resp.usage.unwrap();
        assert_eq!(usage.extra.get("thoughts_token_count").unwrap(), 25);
    }
}
