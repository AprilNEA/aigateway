//! Stream translation: OpenAI SSE chunks → canonical `StreamEvent`s.
//!
//! Unlike request/response translation, streaming requires explicit mapping
//! because `ChatCompletionChunk` (wire) and `StreamEvent` (canonical) are
//! fundamentally different types: the wire type mirrors the HTTP payload,
//! while the canonical type is a granular event enum.

use aigw_core::error::TranslateError;
use aigw_core::model::{FinishReason, StreamEvent, Usage};
use aigw_core::translate::StreamParser;

use crate::wire_types::ChatCompletionChunk;

/// Parses OpenAI SSE chunks into canonical streaming events.
///
/// Minimal state — OpenAI chunks are mostly self-contained. We only track
/// whether `ResponseMeta` has been emitted (from the first chunk).
pub struct OpenAIStreamParser {
    meta_emitted: bool,
}

impl Default for OpenAIStreamParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIStreamParser {
    pub fn new() -> Self {
        Self {
            meta_emitted: false,
        }
    }
}

impl StreamParser for OpenAIStreamParser {
    fn parse_event(
        &mut self,
        _event_type: &str,
        data: &str,
    ) -> Result<Vec<StreamEvent>, TranslateError> {
        if data.trim() == "[DONE]" {
            return Ok(vec![StreamEvent::Done]);
        }

        let chunk: ChatCompletionChunk =
            serde_json::from_str(data).map_err(|e| TranslateError::StreamParse {
                message: format!("failed to parse OpenAI chunk: {e}"),
            })?;

        let mut events = Vec::new();

        // Emit ResponseMeta from the first chunk.
        if !self.meta_emitted {
            events.push(StreamEvent::ResponseMeta {
                id: chunk.id.clone(),
                model: chunk.model.clone(),
            });
            self.meta_emitted = true;
        }

        // Process each choice's delta.
        for choice in &chunk.choices {
            // Text content delta.
            if let Some(text) = &choice.delta.content
                && !text.is_empty()
            {
                events.push(StreamEvent::ContentDelta(text.clone()));
            }

            // Tool call deltas.
            if let Some(tool_calls) = &choice.delta.tool_calls {
                for tc in tool_calls {
                    // ToolCallStart: first appearance has id + function.name.
                    if let (Some(id), Some(func)) = (&tc.id, &tc.function)
                        && let Some(name) = &func.name
                    {
                        events.push(StreamEvent::ToolCallStart {
                            index: tc.index,
                            id: id.clone(),
                            name: name.clone(),
                        });
                    }

                    // ToolCallDelta: subsequent chunks carry function.arguments.
                    if let Some(func) = &tc.function
                        && let Some(args) = &func.arguments
                        && !args.is_empty()
                    {
                        events.push(StreamEvent::ToolCallDelta {
                            index: tc.index,
                            arguments: args.clone(),
                        });
                    }
                }
            }

            // Finish reason.
            if let Some(reason) = &choice.finish_reason {
                events.push(StreamEvent::Finish(map_finish_reason(reason)));
            }
        }

        // Usage (typically in the second-to-last chunk when stream_options.include_usage=true).
        if let Some(usage) = &chunk.usage {
            events.push(StreamEvent::Usage(Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
                extra: usage.extra.clone(),
            }));
        }

        Ok(events)
    }

    fn finish(&mut self) -> Result<Vec<StreamEvent>, TranslateError> {
        // OpenAI always sends `[DONE]`, so Done is emitted from parse_event.
        Ok(vec![])
    }
}

/// Parse a finish reason string into a canonical [`FinishReason`].
///
/// Delegates to strum's `FromStr` — known values map to typed variants,
/// unknown strings fall into `FinishReason::Unknown`.
fn map_finish_reason(reason: &str) -> FinishReason {
    // strum's #[strum(default)] guarantees this never fails.
    reason
        .parse()
        .unwrap_or(FinishReason::Unknown(reason.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> OpenAIStreamParser {
        OpenAIStreamParser::new()
    }

    #[test]
    fn first_chunk_emits_response_meta() {
        let mut p = parser();
        let data = r#"{
            "id": "chatcmpl-abc",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": "gpt-4.1",
            "choices": [{
                "index": 0,
                "delta": { "role": "assistant", "content": "" },
                "finish_reason": null
            }]
        }"#;

        let events = p.parse_event("", data).unwrap();
        assert!(matches!(
            events[0],
            StreamEvent::ResponseMeta { ref id, ref model }
            if id == "chatcmpl-abc" && model == "gpt-4.1"
        ));
    }

    #[test]
    fn content_delta() {
        let mut p = parser();
        // Skip first chunk to emit meta.
        let first = r#"{"id":"c","object":"chat.completion.chunk","created":0,"model":"m","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#;
        p.parse_event("", first).unwrap();

        let data = r#"{"id":"c","object":"chat.completion.chunk","created":0,"model":"m","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let events = p.parse_event("", data).unwrap();
        assert!(matches!(events[0], StreamEvent::ContentDelta(ref s) if s == "Hello"));
    }

    #[test]
    fn tool_call_start_and_delta() {
        let mut p = parser();
        let first = r#"{"id":"c","object":"chat.completion.chunk","created":0,"model":"m","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#;
        p.parse_event("", first).unwrap();

        // Tool call start: id + name + initial arguments.
        let start = r#"{"id":"c","object":"chat.completion.chunk","created":0,"model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}"#;
        let events = p.parse_event("", start).unwrap();
        assert!(matches!(
            events[0],
            StreamEvent::ToolCallStart { index: 0, ref id, ref name }
            if id == "call_1" && name == "get_weather"
        ));

        // Tool call delta: partial arguments.
        let delta = r#"{"id":"c","object":"chat.completion.chunk","created":0,"model":"m","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"loc"}}]},"finish_reason":null}]}"#;
        let events = p.parse_event("", delta).unwrap();
        assert!(matches!(
            events[0],
            StreamEvent::ToolCallDelta { index: 0, ref arguments }
            if arguments == "{\"loc"
        ));
    }

    #[test]
    fn finish_and_usage() {
        let mut p = parser();
        let first = r#"{"id":"c","object":"chat.completion.chunk","created":0,"model":"m","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#;
        p.parse_event("", first).unwrap();

        // Finish reason.
        let finish = r#"{"id":"c","object":"chat.completion.chunk","created":0,"model":"m","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let events = p.parse_event("", finish).unwrap();
        assert!(matches!(events[0], StreamEvent::Finish(FinishReason::Stop)));

        // Usage chunk.
        let usage = r#"{"id":"c","object":"chat.completion.chunk","created":0,"model":"m","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let events = p.parse_event("", usage).unwrap();
        match &events[0] {
            StreamEvent::Usage(u) => {
                assert_eq!(u.prompt_tokens, Some(10));
                assert_eq!(u.completion_tokens, Some(5));
            }
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    #[test]
    fn done_event() {
        let mut p = parser();
        let events = p.parse_event("", "[DONE]").unwrap();
        assert!(matches!(events[0], StreamEvent::Done));
    }

    #[test]
    fn finish_returns_empty() {
        let mut p = parser();
        let events = p.finish().unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn chunk_without_object_parses() {
        // GitHub Copilot's chunks carry `created` but no `object`.
        let mut p = parser();
        let data = r#"{
            "id": "chatcmpl-copilot",
            "created": 1790163964,
            "model": "gpt-4.1-2025-04-14",
            "system_fingerprint": "fp_1",
            "choices": [{ "index": 0, "delta": { "content": "hi" } }]
        }"#;
        let events = p.parse_event("", data).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, StreamEvent::ContentDelta(t) if t == "hi"))
        );
    }
}
