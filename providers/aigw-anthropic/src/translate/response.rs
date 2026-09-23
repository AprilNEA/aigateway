//! Response translation: Anthropic Messages API → canonical types.

use aigw_core::ForwardCompatible;
use aigw_core::error::{ProviderError, TranslateError, map_error_status};
use aigw_core::model::{
    ChatResponse, Choice, ContentPart, FinishReason, Message, MessageContent, Role, ThinkingSource,
    TypedContentPart, Usage,
};
use aigw_core::translate::{ResponseTranslator, StreamParser};
use http::{HeaderMap, StatusCode};

use crate::types::{ApiErrorResponse, ContentBlock, MessagesResponse, TypedContentBlock};

use super::stream::AnthropicStreamParser;
use super::tools;

/// Translates Anthropic responses into canonical types.
pub struct AnthropicResponseTranslator;

impl ResponseTranslator for AnthropicResponseTranslator {
    fn translate_response(
        &self,
        _status: StatusCode,
        body: &[u8],
    ) -> Result<ChatResponse, TranslateError> {
        let native: MessagesResponse = serde_json::from_slice(body)?;

        // Walk the content blocks once, splitting into:
        // - thinking_parts: canonical Thinking / RedactedThinking content parts.
        // - text_parts: text fragments (joined later).
        // - tool_calls: ToolUse → canonical tool calls.
        //
        // If there are no thinking parts, we keep the existing
        // `MessageContent::Text` shape for backward compatibility. With
        // thinking parts present, the shape switches to `Parts(...)` so
        // callers can replay thinking history on subsequent turns.
        let mut thinking_parts: Vec<ContentPart> = Vec::new();
        let mut text_parts: Vec<&str> = Vec::new();
        let mut tool_calls = Vec::new();

        for block in &native.content {
            match block {
                ContentBlock::Typed(TypedContentBlock::Text { text, .. }) => {
                    text_parts.push(text.as_str());
                }
                ContentBlock::Typed(TypedContentBlock::ToolUse {
                    id, name, input, ..
                }) => {
                    tool_calls.push(tools::tool_use_to_canonical(id, name, input));
                }
                ContentBlock::Typed(TypedContentBlock::Thinking {
                    thinking,
                    signature,
                }) => {
                    thinking_parts.push(ForwardCompatible::Known(TypedContentPart::Thinking {
                        thinking: thinking.clone(),
                        signature: signature.clone(),
                        source: Some(ThinkingSource::Anthropic),
                        extra: Default::default(),
                    }));
                }
                ContentBlock::Typed(TypedContentBlock::RedactedThinking { data }) => {
                    thinking_parts.push(ForwardCompatible::Known(
                        TypedContentPart::RedactedThinking {
                            data: data.clone(),
                            source: Some(ThinkingSource::Anthropic),
                            extra: Default::default(),
                        },
                    ));
                }
                // Image/ToolResult/Raw: skip.
                _ => {}
            }
        }

        let content = if thinking_parts.is_empty() {
            // Legacy shape: text-only message body.
            if text_parts.is_empty() {
                None
            } else {
                Some(MessageContent::Text(text_parts.join("")))
            }
        } else {
            // With thinking present, emit a Parts array so consumers can
            // round-trip thinking blocks back to Anthropic on the next turn.
            let mut parts = thinking_parts;
            if !text_parts.is_empty() {
                parts.push(ForwardCompatible::Known(TypedContentPart::Text {
                    text: text_parts.join(""),
                    extra: Default::default(),
                }));
            }
            Some(MessageContent::Parts(parts))
        };

        let tool_calls_opt = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        };

        let message = Message {
            role: Role::Assistant,
            content,
            name: None,
            tool_call_id: None,
            tool_calls: tool_calls_opt,
            extra: Default::default(),
        };

        let finish_reason = native.stop_reason.map(FinishReason::from);

        let usage = Usage {
            prompt_tokens: Some(native.usage.input_tokens),
            completion_tokens: Some(native.usage.output_tokens),
            total_tokens: Some(native.usage.input_tokens + native.usage.output_tokens),
            extra: {
                let mut extra = serde_json::Map::new();
                if let Some(v) = native.usage.cache_creation_input_tokens {
                    extra.insert(
                        "cache_creation_input_tokens".into(),
                        serde_json::Value::Number(v.into()),
                    );
                }
                if let Some(v) = native.usage.cache_read_input_tokens {
                    extra.insert(
                        "cache_read_input_tokens".into(),
                        serde_json::Value::Number(v.into()),
                    );
                }
                extra
            },
        };

        Ok(ChatResponse {
            id: native.id,
            object: "chat.completion".to_owned(),
            created: aigw_core::unix_now(),
            model: native.model,
            choices: vec![Choice {
                index: 0,
                message,
                finish_reason,
                extra: Default::default(),
            }],
            usage: Some(usage),
            extra: Default::default(),
        })
    }

    fn stream_parser(&self) -> Box<dyn StreamParser> {
        Box::new(AnthropicStreamParser::new())
    }

    fn translate_error(
        &self,
        status: StatusCode,
        headers: &HeaderMap,
        body: &[u8],
    ) -> ProviderError {
        let parsed = serde_json::from_slice::<ApiErrorResponse>(body);
        let message = parsed
            .as_ref()
            .map(|e| e.error.message.clone())
            .unwrap_or_else(|_| String::from_utf8_lossy(body).into_owned());

        // Handle Anthropic-specific 529 (overloaded) before generic mapping.
        if status.as_u16() == 529 {
            return ProviderError::Overloaded { message };
        }

        map_error_status(status.as_u16(), headers, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aigw_core::translate::ResponseTranslator;
    use std::time::Duration;

    #[test]
    fn translate_text_response() {
        let json = r#"{
            "id": "msg_01XFD",
            "type": "message",
            "role": "assistant",
            "content": [
                { "type": "text", "text": "Hello! How can I help?" }
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": { "input_tokens": 25, "output_tokens": 10 }
        }"#;

        let translator = AnthropicResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        assert_eq!(resp.id, "msg_01XFD");
        assert_eq!(resp.object, "chat.completion");
        assert_eq!(resp.model, "claude-sonnet-4-20250514");
        assert_eq!(resp.choices.len(), 1);

        let choice = &resp.choices[0];
        assert_eq!(choice.finish_reason, Some(FinishReason::Stop));
        match &choice.message.content {
            Some(MessageContent::Text(s)) => assert_eq!(s, "Hello! How can I help?"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert!(choice.message.tool_calls.is_none());

        let usage = resp.usage.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, Some(25));
        assert_eq!(usage.completion_tokens, Some(10));
        assert_eq!(usage.total_tokens, Some(35));
    }

    #[test]
    fn translate_tool_use_response() {
        let json = r#"{
            "id": "msg_tools",
            "type": "message",
            "role": "assistant",
            "content": [
                { "type": "text", "text": "Let me check." },
                {
                    "type": "tool_use",
                    "id": "toolu_01",
                    "name": "get_weather",
                    "input": { "location": "SF" }
                }
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "tool_use",
            "stop_sequence": null,
            "usage": { "input_tokens": 50, "output_tokens": 30 }
        }"#;

        let translator = AnthropicResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        let choice = &resp.choices[0];
        assert_eq!(choice.finish_reason, Some(FinishReason::ToolCalls));

        // Text content.
        match &choice.message.content {
            Some(MessageContent::Text(s)) => assert_eq!(s, "Let me check."),
            other => panic!("expected Text, got {other:?}"),
        }

        // Tool calls.
        let tc = choice.message.tool_calls.as_ref().unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].id, "toolu_01");
        assert_eq!(tc[0].function.name, "get_weather");
        assert_eq!(tc[0].function.arguments, r#"{"location":"SF"}"#);
    }

    #[test]
    fn translate_response_with_cache_usage() {
        let json = r#"{
            "id": "msg_cache",
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "text", "text": "Hi" }],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 100,
                "output_tokens": 5,
                "cache_creation_input_tokens": 80,
                "cache_read_input_tokens": 20
            }
        }"#;

        let translator = AnthropicResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        let usage = resp.usage.as_ref().unwrap();
        assert_eq!(usage.extra.get("cache_creation_input_tokens").unwrap(), 80);
        assert_eq!(usage.extra.get("cache_read_input_tokens").unwrap(), 20);
    }

    #[test]
    fn translate_thinking_response_emits_parts_with_anthropic_source() {
        let json = r#"{
            "id": "msg_thinking",
            "type": "message",
            "role": "assistant",
            "content": [
                { "type": "thinking", "thinking": "Let me reason...", "signature": "ErWj123" },
                { "type": "text", "text": "The answer is 42." }
            ],
            "model": "claude-opus-4-6",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": { "input_tokens": 30, "output_tokens": 20 }
        }"#;

        let translator = AnthropicResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        let choice = &resp.choices[0];
        let parts = match choice.message.content.as_ref().unwrap() {
            MessageContent::Parts(p) => p,
            other => panic!("expected Parts (thinking present), got {other:?}"),
        };
        assert_eq!(parts.len(), 2);

        match &parts[0] {
            ContentPart::Known(TypedContentPart::Thinking {
                thinking,
                signature,
                source,
                ..
            }) => {
                assert_eq!(thinking, "Let me reason...");
                assert_eq!(signature, "ErWj123");
                assert_eq!(*source, Some(ThinkingSource::Anthropic));
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
    fn translate_redacted_thinking_response() {
        let json = r#"{
            "id": "msg_red",
            "type": "message",
            "role": "assistant",
            "content": [
                { "type": "redacted_thinking", "data": "blob" },
                { "type": "text", "text": "ok" }
            ],
            "model": "claude-opus-4-6",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        }"#;

        let translator = AnthropicResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        let parts = match resp.choices[0].message.content.as_ref().unwrap() {
            MessageContent::Parts(p) => p,
            other => panic!("expected Parts, got {other:?}"),
        };

        assert!(matches!(
            &parts[0],
            ContentPart::Known(TypedContentPart::RedactedThinking {
                data, source: Some(ThinkingSource::Anthropic), ..
            }) if data == "blob"
        ));
    }

    #[test]
    fn translate_error_429() {
        let body =
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"Too many requests"}}"#;
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "60".parse().unwrap());

        let translator = AnthropicResponseTranslator;
        let err =
            translator.translate_error(StatusCode::TOO_MANY_REQUESTS, &headers, body.as_bytes());

        match err {
            ProviderError::RateLimited {
                retry_after,
                message,
            } => {
                assert_eq!(retry_after, Some(Duration::from_secs(60)));
                assert!(message.contains("Too many requests"));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn translate_error_529_overloaded() {
        let body = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;

        let translator = AnthropicResponseTranslator;
        let err = translator.translate_error(
            StatusCode::from_u16(529).unwrap(),
            &HeaderMap::new(),
            body.as_bytes(),
        );

        assert!(matches!(err, ProviderError::Overloaded { .. }));
    }
}
