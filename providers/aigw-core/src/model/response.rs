//! Canonical response types.

use serde::{Deserialize, Serialize};

use crate::{JsonObject, json_object_is_empty, unix_now};

use super::request::Message;

// ─── ChatResponse ───────────────────────────────────────────────────────────

/// Canonical chat completion response.
///
/// Follows the OpenAI `ChatCompletion` format: an envelope with a `choices`
/// array. Providers that don't use choices (Anthropic, Gemini) produce a
/// single-element `choices` array.
///
/// `object` and `created` are always serialized but may be absent on input:
/// OpenAI-compatible upstreams such as GitHub Copilot omit them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Unique response ID.
    pub id: String,
    /// Object type — typically `"chat.completion"`.
    #[serde(default = "chat_completion_object")]
    pub object: String,
    /// Unix timestamp of creation; the time of receipt if the upstream omits it.
    #[serde(default = "unix_now")]
    pub created: u64,
    /// Model that generated the response.
    pub model: String,
    /// Response choices.
    pub choices: Vec<Choice>,
    /// Token usage statistics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// Pass-through fields.
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

/// A single choice in the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// Choice index (for `n > 1`).
    pub index: u32,
    /// The assistant's message.
    pub message: Message,
    /// Why generation stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

fn chat_completion_object() -> String {
    "chat.completion".to_owned()
}

// ─── FinishReason ───────────────────────────────────────────────────────────

/// Normalized finish reason across providers.
///
/// | Canonical       | OpenAI           | Anthropic        | Gemini         |
/// |-----------------|------------------|------------------|----------------|
/// | `Stop`          | `stop`           | `end_turn`       | `STOP`         |
/// | `Length`         | `length`         | `max_tokens`     | `MAX_TOKENS`   |
/// | `ToolCalls`     | `tool_calls`     | `tool_use`       | —              |
/// | `ContentFilter` | `content_filter` | —                | `SAFETY`       |
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumString,
    strum::AsRefStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum FinishReason {
    /// Model finished naturally (end of turn, hit stop sequence).
    Stop,
    /// Hit the max token limit.
    Length,
    /// Model initiated tool calls.
    ToolCalls,
    /// Content was filtered for safety.
    ContentFilter,
    /// Forward-compatible catch-all.
    #[serde(untagged)]
    #[strum(default)]
    Unknown(String),
}

// ─── Usage ──────────────────────────────────────────────────────────────────

/// Token usage statistics.
///
/// All fields are optional because different providers report different subsets.
/// Translators fill in what the provider gives; the gateway can compute
/// `total_tokens` if missing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Input/prompt tokens consumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    /// Output/completion tokens generated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    /// Total tokens (prompt + completion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    /// Provider-specific usage fields (e.g. `cache_creation_input_tokens`).
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_reason_round_trips() {
        for (json, expected) in [
            (r#""stop""#, FinishReason::Stop),
            (r#""length""#, FinishReason::Length),
            (r#""tool_calls""#, FinishReason::ToolCalls),
            (r#""content_filter""#, FinishReason::ContentFilter),
            (
                r#""custom_reason""#,
                FinishReason::Unknown("custom_reason".into()),
            ),
        ] {
            let parsed: FinishReason = serde_json::from_str(json).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn usage_with_extra_fields() {
        let json = r#"{
            "prompt_tokens": 100,
            "completion_tokens": 50,
            "total_tokens": 150,
            "cache_creation_input_tokens": 80
        }"#;

        let usage: Usage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.prompt_tokens, Some(100));
        assert_eq!(usage.extra.get("cache_creation_input_tokens").unwrap(), 80);
    }
}
