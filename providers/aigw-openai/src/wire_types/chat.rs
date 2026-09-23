use bon::Builder;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::shared::{JsonObject, OneOrMany, json_object_from_value, json_object_is_empty};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ChatResponseFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<OneOrMany<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<ChatStreamOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ChatToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatTool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    #[builder(default)]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatMessageRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<ChatMessageContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

impl ChatMessage {
    pub fn has_image_content(&self) -> bool {
        self.content
            .as_ref()
            .is_some_and(ChatMessageContent::has_image_content)
    }
}

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
pub enum ChatMessageRole {
    Developer,
    System,
    User,
    Assistant,
    Tool,
    #[serde(untagged)]
    #[strum(default)]
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatMessageContent {
    Text(String),
    Parts(Vec<ChatContentPart>),
}

impl ChatMessageContent {
    pub fn has_image_content(&self) -> bool {
        match self {
            Self::Text(_) => false,
            Self::Parts(parts) => parts.iter().any(ChatContentPart::is_image_content),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContentPart {
    Typed(TypedChatContentPart),
    Raw(JsonObject),
}

impl ChatContentPart {
    pub fn is_image_content(&self) -> bool {
        matches!(self, Self::Typed(TypedChatContentPart::ImageUrl { .. }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TypedChatContentPart {
    Text {
        text: String,
        #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
        extra: JsonObject,
    },
    ImageUrl {
        image_url: ChatImageUrl,
        #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
        extra: JsonObject,
    },
    InputAudio {
        input_audio: Value,
        #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
        extra: JsonObject,
    },
    File {
        file: Value,
        #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
        extra: JsonObject,
    },
    Refusal {
        refusal: String,
        #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
        extra: JsonObject,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatImageUrl {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatToolChoice {
    Mode(ChatToolChoiceMode),
    Named(ChatNamedToolChoice),
    Raw(JsonObject),
}

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
pub enum ChatToolChoiceMode {
    None,
    Auto,
    Required,
    #[serde(untagged)]
    #[strum(default)]
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatNamedToolChoice {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ChatNamedToolChoiceFunction,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatNamedToolChoiceFunction {
    pub name: String,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatTool {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ChatFunctionDefinition,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatFunctionDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

/// `object` and `created` may be absent on input — OpenAI-compatible upstreams
/// such as GitHub Copilot omit them — and are always serialized.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    #[serde(default = "chat_completion_object")]
    pub object: String,
    #[serde(default = "aigw_core::unix_now")]
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatCompletionResponseChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionResponseChoice {
    pub index: u32,
    pub message: ChatMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Value>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

/// As with [`ChatCompletionResponse`], `object` and `created` may be absent on
/// input; GitHub Copilot's chunks carry `created` but no `object`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    #[serde(default = "chat_completion_chunk_object")]
    pub object: String,
    #[serde(default = "aigw_core::unix_now")]
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatCompletionChunkChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

fn chat_completion_object() -> String {
    "chat.completion".to_owned()
}

fn chat_completion_chunk_object() -> String {
    "chat.completion.chunk".to_owned()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionChunkChoice {
    pub index: u32,
    pub delta: ChatMessageDelta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<Value>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<ChatMessageRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCallDelta>>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ChatFunctionCall,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatFunctionCall {
    pub name: String,
    pub arguments: String,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatToolCallDelta {
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<ChatFunctionCallDelta>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatFunctionCallDelta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatResponseFormat {
    Text {
        extra: JsonObject,
    },
    JsonObject {
        extra: JsonObject,
    },
    JsonSchema {
        json_schema: ChatJsonSchema,
        extra: JsonObject,
    },
    Raw(JsonObject),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum KnownChatResponseFormat {
    Text {
        #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
        extra: JsonObject,
    },
    JsonObject {
        #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
        extra: JsonObject,
    },
    JsonSchema {
        json_schema: ChatJsonSchema,
        #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
        extra: JsonObject,
    },
}

impl Serialize for ChatResponseFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Text { extra } => KnownChatResponseFormat::Text {
                extra: extra.clone(),
            }
            .serialize(serializer),
            Self::JsonObject { extra } => KnownChatResponseFormat::JsonObject {
                extra: extra.clone(),
            }
            .serialize(serializer),
            Self::JsonSchema { json_schema, extra } => KnownChatResponseFormat::JsonSchema {
                json_schema: json_schema.clone(),
                extra: extra.clone(),
            }
            .serialize(serializer),
            Self::Raw(raw) => raw.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ChatResponseFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let object = match &value {
            Value::Object(object) => object,
            _ => {
                return Err(D::Error::custom(
                    "chat response_format must be a JSON object",
                ));
            }
        };

        match object.get("type").and_then(Value::as_str) {
            Some("text") => {
                let known: KnownChatResponseFormat =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(match known {
                    KnownChatResponseFormat::Text { extra } => Self::Text { extra },
                    _ => unreachable!(),
                })
            }
            Some("json_object") => {
                let known: KnownChatResponseFormat =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(match known {
                    KnownChatResponseFormat::JsonObject { extra } => Self::JsonObject { extra },
                    _ => unreachable!(),
                })
            }
            Some("json_schema") => {
                let known: KnownChatResponseFormat =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(match known {
                    KnownChatResponseFormat::JsonSchema { json_schema, extra } => {
                        Self::JsonSchema { json_schema, extra }
                    }
                    _ => unreachable!(),
                })
            }
            _ => Ok(Self::Raw(
                json_object_from_value(value).map_err(D::Error::custom)?,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatJsonSchema {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatStreamOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}

#[cfg(test)]
mod tests {
    use super::{
        ChatCompletionRequest, ChatContentPart, ChatMessageContent, ChatMessageRole,
        ChatResponseFormat, ChatToolChoice, ChatToolChoiceMode,
    };

    #[test]
    fn deserialize_unknown_content_part_without_losing_shape() {
        let json = r#"{
            "model": "gpt-4.1",
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": "hello" },
                    { "type": "mystery_part", "payload": 1 }
                ]
            }]
        }"#;

        let request: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        let parts = match request.messages[0].content.as_ref().unwrap() {
            ChatMessageContent::Parts(parts) => parts,
            ChatMessageContent::Text(_) => panic!("expected multipart content"),
        };

        assert!(matches!(parts[0], ChatContentPart::Typed(_)));
        assert!(matches!(parts[1], ChatContentPart::Raw(_)));

        let reserialized = serde_json::to_value(&request).unwrap();
        assert_eq!(
            reserialized["messages"][0]["content"][1]["type"],
            "mystery_part"
        );
    }

    #[test]
    fn deserialize_string_tool_choice_mode() {
        let value: ChatToolChoice = serde_json::from_str(r#""required""#).unwrap();
        assert_eq!(value, ChatToolChoice::Mode(ChatToolChoiceMode::Required));
    }

    #[test]
    fn deserialize_unknown_role_round_trips() {
        let role: ChatMessageRole = serde_json::from_str(r#""critic""#).unwrap();
        assert_eq!(role, ChatMessageRole::Unknown("critic".to_owned()));
        assert_eq!(serde_json::to_string(&role).unwrap(), r#""critic""#);
    }

    #[test]
    fn deserialize_unknown_tool_choice_mode_round_trips() {
        let value: ChatToolChoice = serde_json::from_str(r#""parallel_required""#).unwrap();
        assert_eq!(
            value,
            ChatToolChoice::Mode(ChatToolChoiceMode::Unknown("parallel_required".to_owned()))
        );
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            r#""parallel_required""#
        );
    }

    #[test]
    fn deserialize_unknown_response_format_round_trips() {
        let value: ChatResponseFormat =
            serde_json::from_str(r#"{"type":"xml_schema","schema":{"root":"x"}}"#).unwrap();
        let reserialized = serde_json::to_value(&value).unwrap();
        assert_eq!(reserialized["type"], "xml_schema");
        assert_eq!(reserialized["schema"]["root"], "x");
    }
}
