//! Response translation: OpenAI HTTP response → canonical types.
//!
//! Non-streaming responses are deserialized directly into canonical
//! `ChatResponse` (the formats are identical). Error responses are mapped
//! to semantic `ProviderError` variants.

use aigw_core::error::{ProviderError, TranslateError, map_error_status};
use aigw_core::model::ChatResponse;
use aigw_core::translate::{ResponseTranslator, StreamParser};
use http::{HeaderMap, StatusCode};

use crate::wire_types::ApiErrorResponse;

use super::stream::OpenAIStreamParser;

/// Translates OpenAI responses into canonical types.
///
/// Stateless — the canonical format IS the OpenAI format, so response
/// translation is direct deserialization.
pub struct OpenAIResponseTranslator;

impl ResponseTranslator for OpenAIResponseTranslator {
    fn translate_response(
        &self,
        _status: StatusCode,
        body: &[u8],
    ) -> Result<ChatResponse, TranslateError> {
        let resp: ChatResponse = serde_json::from_slice(body)?;
        Ok(resp)
    }

    fn stream_parser(&self) -> Box<dyn StreamParser> {
        Box::new(OpenAIStreamParser::new())
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

        map_error_status(status.as_u16(), headers, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aigw_core::model::FinishReason;
    use aigw_core::translate::ResponseTranslator;
    use std::time::Duration;

    #[test]
    fn translate_minimal_response() {
        let json = r#"{
            "id": "chatcmpl-abc",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4.1",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        }"#;

        let translator = OpenAIResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        assert_eq!(resp.id, "chatcmpl-abc");
        assert_eq!(resp.model, "gpt-4.1");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].finish_reason, Some(FinishReason::Stop));
        let usage = resp.usage.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, Some(10));
        assert_eq!(usage.completion_tokens, Some(5));
        assert_eq!(usage.total_tokens, Some(15));
    }

    #[test]
    fn translate_response_with_tool_calls() {
        let json = r#"{
            "id": "chatcmpl-tools",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4.1",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"SF\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;

        let translator = OpenAIResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        let choice = &resp.choices[0];
        assert_eq!(choice.finish_reason, Some(FinishReason::ToolCalls));
        let tc = choice.message.tool_calls.as_ref().unwrap();
        assert_eq!(tc[0].function.name, "get_weather");
        assert_eq!(tc[0].function.arguments, "{\"location\":\"SF\"}");
    }

    #[test]
    fn translate_response_preserves_extra_fields() {
        let json = r#"{
            "id": "chatcmpl-extra",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4.1",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "Hi" },
                "finish_reason": "stop",
                "logprobs": { "content": [] }
            }],
            "service_tier": "default",
            "system_fingerprint": "fp_abc"
        }"#;

        let translator = OpenAIResponseTranslator;
        let resp = translator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        // Wire-specific fields land in extra
        assert!(resp.extra.contains_key("service_tier"));
        assert!(resp.extra.contains_key("system_fingerprint"));
        assert!(resp.choices[0].extra.contains_key("logprobs"));
    }

    #[test]
    fn translate_error_429_with_retry_after() {
        let body = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "30".parse().unwrap());

        let translator = OpenAIResponseTranslator;
        let err =
            translator.translate_error(StatusCode::TOO_MANY_REQUESTS, &headers, body.as_bytes());

        match err {
            ProviderError::RateLimited {
                retry_after,
                message,
            } => {
                assert_eq!(retry_after, Some(Duration::from_secs(30)));
                assert!(message.contains("Rate limit"));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn translate_error_401() {
        let body = r#"{"error":{"message":"Invalid API key","type":"authentication_error"}}"#;

        let translator = OpenAIResponseTranslator;
        let err = translator.translate_error(
            StatusCode::UNAUTHORIZED,
            &HeaderMap::new(),
            body.as_bytes(),
        );

        assert!(matches!(err, ProviderError::AuthenticationFailed { .. }));
    }

    #[test]
    fn translate_response_without_object_or_created() {
        // GitHub Copilot's non-streaming envelope: no `object`, no `created`,
        // plus a provider-specific `copilot_usage`.
        let json = r#"{
            "id": "chatcmpl-copilot",
            "model": "gpt-4.1-2025-04-14",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "pong" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 12, "completion_tokens": 2, "total_tokens": 14 },
            "copilot_usage": { "premium_requests": 0 },
            "service_tier": "default",
            "system_fingerprint": "fp_1"
        }"#;
        let before = aigw_core::unix_now();
        let resp = OpenAIResponseTranslator
            .translate_response(StatusCode::OK, json.as_bytes())
            .unwrap();

        assert_eq!(resp.object, "chat.completion");
        assert!(resp.created >= before);
        assert!(resp.extra.contains_key("copilot_usage"));

        // Re-serialized for clients, the envelope is complete again.
        let out = serde_json::to_value(&resp).unwrap();
        assert_eq!(out["object"], "chat.completion");
        assert!(out["created"].as_u64().is_some());
    }
}
