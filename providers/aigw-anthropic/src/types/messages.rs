//! Anthropic Messages API native types.
//!
//! These types map 1:1 to the Anthropic wire format.
//! See <https://docs.anthropic.com/en/api/messages>

use bon::Builder;
use serde::{Deserialize, Serialize, Serializer};

// ─── Request ─────────────────────────────────────────────────────────────────

/// POST `/v1/messages` request body.
///
/// # Example
///
/// ```
/// use aigw_anthropic::types::*;
///
/// let req = MessagesRequest::builder()
///     .model("claude-sonnet-4-20250514")
///     .messages(vec![Message {
///         role: Role::User,
///         content: MessageContent::Text("Hello".into()),
///     }])
///     .max_tokens(1024)
///     .temperature(0.7)
///     .build();
/// ```
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(on(String, into))]
pub struct MessagesRequest {
    /// Model identifier (e.g. `"claude-sonnet-4-20250514"`).
    pub model: String,
    /// Input messages. Must alternate between `user` and `assistant` roles.
    pub messages: Vec<Message>,
    /// Maximum number of output tokens. **Required** by the Anthropic API.
    pub max_tokens: u64,

    /// System prompt, separate from messages (Anthropic has no `role: "system"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    /// Sampling temperature (0.0–1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Nucleus sampling parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// Top-K sampling parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Custom stop sequences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Enable SSE streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Available tools for the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// Tool selection strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Request metadata (e.g. `user_id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    /// Extended thinking configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,

    /// Provider-specific fields that we don't interpret.
    #[serde(flatten)]
    #[builder(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// System prompt — can be a plain string or an array of text blocks with cache control.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    /// Plain text system prompt.
    Text(String),
    /// Array of text blocks (supports cache control).
    Blocks(Vec<TextBlock>),
}

/// A text block used in system prompts and tool results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    /// Block type — always `"text"`.
    pub r#type: String,
    /// The text content.
    pub text: String,
    /// Optional cache control for prompt caching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

/// Cache control directive for prompt caching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    /// Cache type — currently only `"ephemeral"`.
    pub r#type: String,
    /// Optional cache lifetime. Absent means the server default of 5
    /// minutes. The Anthropic API requires that any block with a longer TTL
    /// appear *before* any short-TTL block in evaluation order
    /// (tools → system → messages); see
    /// [`crate::translate::cache_control::normalize_ttl_ordering`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<CacheTtl>,
}

/// Lifetime of a prompt cache entry, as the API spells it (`"5m"`, `"1h"`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum CacheTtl {
    /// Five minutes, the default.
    #[serde(rename = "5m")]
    FiveMinutes,
    /// One hour.
    #[serde(rename = "1h")]
    OneHour,
    /// Forward-compatible catch-all for durations added later.
    #[serde(untagged)]
    Other(String),
}

impl CacheTtl {
    /// Whether this lifetime is longer than the 5-minute default. Unknown
    /// values count as longer, so ordering normalisation strips them
    /// rather than risking a rejected request.
    #[must_use]
    pub fn is_extended(&self) -> bool {
        !matches!(self, Self::FiveMinutes)
    }
}

/// Request metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// Opaque user identifier for abuse detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

// ─── Messages ────────────────────────────────────────────────────────────────

/// A conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message role.
    pub role: Role,
    /// Message content — plain string or array of content blocks.
    pub content: MessageContent,
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User message.
    User,
    /// Assistant message.
    Assistant,
    /// Inline system message accepted when deserializing requests from newer
    /// Claude Code clients. Anthropic's upstream API requires clients to move
    /// this content to the top-level `system` field before serialization.
    System,
}

impl Serialize for Role {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::User => serializer.serialize_str("user"),
            Self::Assistant => serializer.serialize_str("assistant"),
            Self::System => Err(serde::ser::Error::custom(
                "role `system` is inbound-only; move its content to the top-level `system` field",
            )),
        }
    }
}

/// Content can be a plain string or an array of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content.
    Text(String),
    /// Array of typed content blocks (text, image, tool_use, tool_result, etc.).
    Blocks(Vec<ContentBlock>),
}

// ─── Content Blocks ──────────────────────────────────────────────────────────

/// Content block in a message (request or response).
///
/// Uses the same `Typed | Raw` pattern as `aigw-openai`'s `ChatContentPart`:
/// known block types are strongly typed; unknown types fall back to raw JSON
/// so new Anthropic block types (e.g. `document`) don't break deserialization.
///
/// ```
/// # use aigw_anthropic::types::ContentBlock;
/// // A known block deserializes into Typed:
/// let json = r#"{"type": "text", "text": "hello"}"#;
/// let block: ContentBlock = serde_json::from_str(json).unwrap();
/// assert!(matches!(block, ContentBlock::Typed(_)));
///
/// // An unknown block type falls back to Raw:
/// let json = r#"{"type": "document", "source": {"type": "url", "url": "..."}}"#;
/// let block: ContentBlock = serde_json::from_str(json).unwrap();
/// assert!(matches!(block, ContentBlock::Raw(_)));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentBlock {
    /// A known, strongly-typed content block.
    Typed(TypedContentBlock),
    /// Forward-compatible fallback for unknown block types.
    Raw(serde_json::Map<String, serde_json::Value>),
}

/// Strongly-typed content block variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TypedContentBlock {
    /// Text content.
    Text {
        /// The text value.
        text: String,
        /// Optional cache control.
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Image content (base64 or URL).
    Image {
        /// Image source.
        source: ImageSource,
        /// Optional cache control.
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Tool use block — the model's request to call a tool.
    ToolUse {
        /// Unique tool call ID (e.g. `"toolu_01T1x..."`).
        id: String,
        /// Tool name.
        name: String,
        /// Tool input parameters as a JSON object.
        input: serde_json::Value,
        /// Optional cache control.
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Tool result block — the user's response to a tool call.
    ToolResult {
        /// The `id` of the corresponding `tool_use` block.
        tool_use_id: String,
        /// Result content.
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<ToolResultContent>,
        /// Whether this result represents an error.
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        /// Optional cache control.
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Extended thinking block.
    Thinking {
        /// The model's chain-of-thought text.
        thinking: String,
        /// Integrity signature.
        signature: String,
    },
    /// Redacted thinking block (content hidden for safety).
    RedactedThinking {
        /// Opaque redacted data.
        data: String,
    },
}

/// Content of a tool result — either a plain string or content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Plain text result.
    Text(String),
    /// Structured result as content blocks.
    Blocks(Vec<ContentBlock>),
}

/// Image source — base64-encoded or URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64-encoded image data.
    Base64 {
        /// MIME type (e.g. `"image/png"`).
        media_type: String,
        /// Base64-encoded image bytes.
        data: String,
    },
    /// Image URL.
    Url {
        /// The image URL.
        url: String,
    },
}

// ─── Tools ───────────────────────────────────────────────────────────────────

/// Tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name.
    pub name: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
    /// Optional cache control for prompt caching. Setting this on the last
    /// tool produces a cache breakpoint covering all preceding tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

/// Tool selection strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// Let the model decide whether to use tools.
    Auto {
        /// Disable parallel tool calls.
        #[serde(skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// Force the model to use at least one tool.
    Any {
        /// Disable parallel tool calls.
        #[serde(skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// Force the model to use a specific tool.
    Tool {
        /// The tool name to force.
        name: String,
        /// Disable parallel tool calls.
        #[serde(skip_serializing_if = "Option::is_none")]
        disable_parallel_tool_use: Option<bool>,
    },
    /// Disable tool use entirely. Struct variant for forward compatibility.
    None {
        /// Reserved for future fields.
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    },
}

// ─── Thinking ────────────────────────────────────────────────────────────────

/// Extended thinking configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingConfig {
    /// Enable extended thinking with a token budget. Required for Claude
    /// models that don't support adaptive thinking (Claude 4.5 and earlier).
    Enabled {
        /// Maximum tokens the model may spend on thinking.
        budget_tokens: u64,
    },
    /// Adaptive thinking — let the model dynamically allocate thinking tokens.
    ///
    /// Available on Claude 4.6+. Pair with the request-root `output_config`
    /// field (sibling of `thinking`) to bias the dynamic allocation, e.g.
    /// `output_config: { effort: "high" }`.
    Adaptive,
    /// Disable extended thinking.
    Disabled,
}

// ─── Response ────────────────────────────────────────────────────────────────

/// POST `/v1/messages` response body (non-streaming).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesResponse {
    /// Unique message ID (e.g. `"msg_01XFDUDYJgAACzvnptvVoYEL"`).
    pub id: String,
    /// Object type — always `"message"`.
    pub r#type: String,
    /// Always `assistant`.
    pub role: Role,
    /// Response content blocks.
    pub content: Vec<ContentBlock>,
    /// Model that generated the response.
    pub model: String,
    /// Why the model stopped generating.
    pub stop_reason: Option<StopReason>,
    /// The specific stop sequence that was hit, if any.
    pub stop_sequence: Option<String>,
    /// Token usage statistics.
    pub usage: Usage,
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Model finished its response naturally.
    EndTurn,
    /// Hit the `max_tokens` limit.
    MaxTokens,
    /// Hit a custom stop sequence.
    StopSequence,
    /// Model initiated a tool call.
    ToolUse,
    /// Forward-compatible catch-all for unknown stop reasons.
    #[serde(untagged)]
    Other(String),
}

/// Token usage statistics.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Usage {
    /// Number of input tokens consumed.
    pub input_tokens: u64,
    /// Number of output tokens generated.
    pub output_tokens: u64,
    /// Tokens used to create a new cache entry (prompt caching).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u64>,
    /// Tokens read from an existing cache entry (prompt caching).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u64>,
}

// ─── Streaming Events ────────────────────────────────────────────────────────

/// Top-level SSE event envelope.
///
/// Events arrive in this order:
/// `message_start` → (`content_block_start` → `content_block_delta`* → `content_block_stop`)*
/// → `message_delta` → `message_stop`. `ping` may appear at any point.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// First event — contains the initial [`MessagesResponse`] with empty content.
    MessageStart {
        /// The initial message object.
        message: MessagesResponse,
    },
    /// A new content block begins.
    ContentBlockStart {
        /// Zero-based index of this block in the response content array.
        index: usize,
        /// The initial (empty) content block.
        content_block: ContentBlock,
    },
    /// Incremental update to the current content block.
    ContentBlockDelta {
        /// Index of the block being updated.
        index: usize,
        /// The delta payload.
        delta: ContentDelta,
    },
    /// The current content block is complete.
    ContentBlockStop {
        /// Index of the completed block.
        index: usize,
    },
    /// Top-level message update (stop reason, final usage).
    MessageDelta {
        /// Updated message fields.
        delta: MessageDeltaBody,
        /// Cumulative output token usage.
        usage: MessageDeltaUsage,
    },
    /// Stream is complete.
    MessageStop,
    /// Keep-alive event.
    Ping,
    /// In-stream error.
    Error {
        /// The error details.
        error: ApiError,
    },
    /// Forward-compatible catch-all for unknown event types.
    #[serde(other)]
    Unknown,
}

/// Incremental content delta within a streaming content block.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    /// Text content delta.
    TextDelta {
        /// The incremental text.
        text: String,
    },
    /// Tool input JSON delta (partial JSON string).
    InputJsonDelta {
        /// Partial JSON for the tool's input parameters.
        partial_json: String,
    },
    /// Extended thinking delta.
    ThinkingDelta {
        /// Incremental thinking text.
        thinking: String,
    },
    /// Thinking signature delta (appears at end of thinking block).
    SignatureDelta {
        /// Incremental signature data.
        signature: String,
    },
    /// Forward-compatible catch-all.
    #[serde(other)]
    Unknown,
}

/// Updated fields in a [`StreamEvent::MessageDelta`].
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaBody {
    /// The stop reason, if the model has finished.
    pub stop_reason: Option<StopReason>,
    /// The stop sequence that was hit, if any.
    pub stop_sequence: Option<String>,
}

/// Usage update in a [`StreamEvent::MessageDelta`].
///
/// Note: `output_tokens` is **cumulative**, not incremental.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaUsage {
    /// Cumulative output tokens generated so far.
    pub output_tokens: u64,
}

// ─── API Error ───────────────────────────────────────────────────────────────

/// Anthropic API error body.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    /// Error type (e.g. `"invalid_request_error"`, `"rate_limit_error"`).
    pub r#type: String,
    /// Human-readable error message.
    pub message: String,
}

/// Top-level error response wrapper.
///
/// ```json
/// { "type": "error", "error": { "type": "invalid_request_error", "message": "..." } }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorResponse {
    /// Always `"error"`.
    pub r#type: String,
    /// The nested error object.
    pub error: ApiError,
}
