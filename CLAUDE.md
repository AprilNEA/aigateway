# AI Gateway — Workspace Overview

A Rust workspace for a **protocol-faithful, multi-provider AI gateway** that routes requests across OpenAI, Anthropic, Google Gemini, and OpenAI-compatible providers with native wire types, streaming SSE, and zero lowest-common-denominator abstractions.

**Repository:** [AprilNEA/aigateway](https://github.com/AprilNEA/aigateway)  
**Edition:** Rust 2024, License: MIT

---

## Architecture

```
Client Application
    ↓ (OpenAI format request)
⚡ AI Gateway
    ├── Request Router
    ├── Protocol Translation (aigw-core traits)
    └── SSE Streaming Engine
    ↓
Providers
    ├── aigw-openai (Official OpenAI)
    ├── aigw-anthropic (Anthropic Messages API)
    ├── aigw-openai-compat (Groq, Together, vLLM, etc.)
    └── aigw-gemini (Google Gemini)
    ↓
Upstream APIs
```

---

## Workspace Structure

### Root Workspace (`Cargo.toml`)
- **Members:** aigw, aigw-core, aigw-openai, aigw-openai-compat, aigw-anthropic, aigw-gemini
- **Resolver:** 3 (workspace dependencies)
- **Shared version:** 0.1.0
- **Edition:** 2024

### Core Crate: `aigw-core`

**Purpose:** Define canonical request/response types and translation trait interfaces. No IO, no HTTP client dependency.

**Key Modules:**

1. **`model/` — Canonical message format (OpenAI-style)**
   - `request.rs`
     - `ChatRequest` — Standard request (model, messages, temperature, max_tokens, tools, response_format, etc.)
     - `Message` — Conversation message (role, content, tool_calls, tool_call_id, extra)
     - `Role` enum — system, developer, user, assistant, tool, Unknown(String)
     - `MessageContent` — Text(String) | Parts(Vec<ContentPart>)
     - `ContentPart` — Typed parts (text, image_url, input_audio, file) + Raw catch-all
     - `Tool` / `ToolCall` / `FunctionDefinition` / `FunctionCall` — Function calling types
     - `ToolChoice` — Mode("auto"|"none"|"required") | Named(specific function) | Raw
     - `ResponseFormat` — Text | JsonObject | JsonSchema
     - `StopSequence` — One(String) | Many(Vec<String>)

   - `response.rs`
     - `ChatResponse` — id, object, created, model, choices[], usage, extra
     - `Choice` — index, message, finish_reason, extra
     - `FinishReason` enum — Stop, Length, ToolCalls, ContentFilter, Unknown
     - `Usage` — prompt_tokens, completion_tokens, total_tokens, extra

   - `stream.rs`
     - `StreamEvent` enum — ResponseMeta, ContentDelta(String), ToolCallStart{index, id, name}, ToolCallDelta{index, arguments}, Finish(FinishReason), Usage, Done
     - Intermediate events that providers translate SSE → canonical format

2. **`translate.rs` — Translation trait interfaces (pure data mapping)**
   - `TranslatedRequest` — url, method, headers, body (pre-serialized JSON bytes)
   - `RequestTranslator` trait — translate_request(ChatRequest) → TranslatedRequest
   - `ResponseTranslator` trait — translate_response() → ChatResponse, stream_parser() → StreamParser, translate_error() → ProviderError
   - `StreamParser` trait — parse_event(event_type, data) → Vec<StreamEvent>, finish() → Vec<StreamEvent>
   - `ProviderTranslator<Req, Res>` — convenience facade

3. **`error.rs` — Translation & provider error types**
   - `TranslateError` — MissingField, UnsupportedFeature, IncompatibleContent, Json, StreamParse, Other
   - `ProviderError` — RateLimited, AuthenticationFailed, PermissionDenied, ModelNotFound, ContextLengthExceeded, InvalidRequest, Overloaded, ServerError, Unknown

4. **`macros.rs`**
   - `string_enum!` macro — Generate string-backed enums with Unknown catch-all, auto Display/Serialize/Deserialize

5. **`lib.rs`**
   - `JsonObject` type alias — BTreeMap<String, serde_json::Value> (deterministic order for serialization)
   - `json_object_is_empty()` — Helper for skip_serializing_if

**Design:** 
- No IO or HTTP client dependencies
- All types carry `#[serde(flatten)] extra` or Unknown variants for forward compatibility
- Message format matches OpenAI Chat Completions (clients deserialize directly; translators map to/from provider formats)
- Provider-specific fields (logprobs, thinking, safety_settings) flow through `extra` untouched

---

### Umbrella Crate: `aigw` (`providers/aigw/src/lib.rs`)

Re-exports all provider crates behind feature flags. All features enabled by default.

```rust
#[cfg(feature = "openai")]
pub mod openai { pub use aigw_openai::*; }

#[cfg(feature = "anthropic")]
pub mod anthropic { pub use aigw_anthropic::*; }

#[cfg(feature = "openai-compat")]
pub mod openai_compat { pub use aigw_openai_compat::*; }

#[cfg(feature = "gemini")]
pub mod gemini { pub use aigw_gemini::*; }
```

Users can select: `aigw = { version = "0.0.1", default-features = false, features = ["anthropic", "openai"] }`

---

### Provider: `aigw-openai` (`providers/aigw-openai/`)

**Purpose:** Official OpenAI API client — protocol-faithful implementation of Chat Completions and Responses APIs.

**Key Modules:**
- `client.rs` — OpenAIClient, OpenAIResponse, OpenAIResponseStream, RequestOptions
- `transport.rs` — HttpTransportConfig, OpenAIAuthConfig, OpenAITransport, OpenAITransportConfig
- `wire_types/` — Native request/response types mirroring OpenAI format
  - `chat.rs` — ChatCompletionRequest, ChatCompletionResponse, ChatMessage, ChatTool, ChatToolCall, ChatUsage, etc.
  - `embeddings.rs` — EmbeddingRequest, EmbeddingResponse, Embedding
  - `responses.rs` — ResponseCreateRequest, ResponseCompactRequest, ResponseUse, ResponseStreamEvent, ResponseOutputItem, ResponseInput, ResponseInputItem, ResponseReasoning, ResponseContextManagement, ResponseConversation, ResponseToolChoice, ResponseTool (with variants: ResponseNamespaceTool, ResponseObject, etc.)
  - `responses_output.rs` — ResponseCodeInterpreterOutput, ResponseFileSearchResult, ResponseOutputTextAnnotation, ResponseReasoningContentPart, ResponseShellAction, ResponseShellCallOutcome
  - `models.rs` — Model, ModelListResponse
  - `shared.rs` — JsonObject, ApiErrorResponse, ApiErrorBody, OneOrMany, json_object_is_empty

**Design Principles:**
- Near-passthrough — unified format **is** the OpenAI format
- Handles OpenAI-specific headers: Authorization: Bearer, OpenAI-Organization, OpenAI-Project
- Responses API prioritized over Chat Completions (per OpenAI's official direction)
- SSE parsing: data: {...}, data: [DONE]
- All wire types carry `#[serde(flatten)] extra` for forward compatibility
- Tool union types include: function, file_search, web_search_preview, code_interpreter, computer_use, image_generation, mcp, custom, apply_patch, shell

---

### Provider: `aigw-anthropic` (`providers/aigw-anthropic/`)

**Purpose:** Anthropic Messages API client with rate limit tracking and (optional) Claude Code features.

**Cargo.toml Features:**
- `default = []`
- `claude-code` — Enables event_logging.rs and oauth.rs modules (non-standard Anthropic endpoints)

**Key Modules:**

1. **`transport.rs`**
   - `TransportConfig` — api_key (SecretString), auth_mode, base_url, version, timeout, beta, extra_headers
   - `AuthMode` enum — ApiKey (x-api-key header), Bearer (Authorization header)
   - `Transport` — Validated config, builds headers and URL helper

2. **`rate_limit.rs`**
   - `RateLimitInfo` — Parsed from response headers (anthropic-ratelimit-*-{limit,remaining,reset}, retry-after)
   - `ApiResponse<T>` — Pairs response body + RateLimitInfo from headers

3. **`client.rs`**
   - `Client` — Thin HTTP wrapper over Transport
   - Methods: messages(), messages_stream(), count_tokens(), models()
   - Supports ANTHROPIC_API_KEY env var via from_env()

4. **`streaming.rs`**
   - `parse_sse_stream()` — EventSource + StreamEvent parser
   - Handles Anthropic's named SSE events: message_start, content_block_start, content_block_delta, content_block_stop, message_delta, message_stop, ping

5. **`types/mod.rs`** — Wire format types
   - `messages.rs` — MessagesRequest, MessagesResponse, Message, MessageContent, Role, ContentBlock variants (text, tool_use, tool_result), ToolUseBlock, ToolResultBlock, Tool, ToolChoice (auto, any, tool {name}), TextBlock, etc.
   - `models.rs` — Model, ModelListResponse
   - `count_tokens.rs` — CountTokensRequest, CountTokensResponse
   - `event_logging.rs` — (feature: claude-code) Batch event logging types
   - `oauth.rs` — (feature: claude-code) OAuth token endpoint types

6. **`error.rs`**
   - Custom error variants for Anthropic-specific scenarios

7. **`lib.rs`**
   - Public API: Client, Transport, TransportConfig, AuthMode, ApiResponse, RateLimitInfo, Error, and all types from types::*

**Design:**
- API key stored as SecretString (never leaks in Debug)
- Rate limit headers parsed automatically on every response
- Supports both x-api-key and Bearer auth modes
- Beta feature flags via anthropic-beta header
- Forward-compatible: all types carry extra fields

---

### Provider: `aigw-openai-compat` (`providers/aigw-openai-compat/`)

**Purpose:** Configure OpenAI-compatible third-party providers (Groq, Together, vLLM, Fireworks, Perplexity, LM Studio, Ollama, DeepSeek, xAI, Mistral La Plateforme, etc.).

**Key Types:**

1. **`OpenAICompatProvider`** — Wraps config and provides accessors
2. **`OpenAICompatConfig`**
   - name — provider name (required)
   - http — HttpTransportConfig (base_url, timeout_seconds, default_headers)
   - auth — OpenAIAuthConfig (api_key, organization, project)
   - quirks — Quirks struct (feature flags)

3. **`Quirks`** — Capability flags
   - supports_responses_api (default: false)
   - supports_chat_completions (default: true)
   - supports_embeddings (default: true)
   - supports_streaming (default: true)
   - supports_tool_choice (default: true)
   - supports_parallel_tool_calls (default: true)
   - supports_vision (default: true)

4. **`OpenAICompatConfigError`** — Config validation errors

**Design:**
- Base URL + quirks-based differentiation
- Reuses OpenAI wire types (from aigw-openai)
- Pre-send sanitization: strips unsupported fields based on Quirks before HTTP request
- No new crate needed for new OpenAI-compatible provider — just add a Quirks config

---

### Provider: `aigw-gemini` (`providers/aigw-gemini/`)

**Purpose:** Google Gemini API client (currently skeleton status).

**Key Characteristics:**
- Model in URL path (not body): POST /v1beta/models/{model}:generateContent
- Auth via x-goog-api-key header
- Content always parts: [Part] (not string)
- Tool call args as JSON object (not string)
- Schema types UPPERCASE ("STRING", "NUMBER")
- Finish reasons SCREAMING_SNAKE_CASE (STOP, MAX_TOKENS)
- Streaming sends full response per SSE event (not deltas)
- Part as flat struct with all-Option fields (not enum) to support metadata co-existence (thought, thoughtSignature alongside data field)
- thoughtSignature must be preserved exactly in subsequent turns (opaque validation string)
- Gemini 3+ only provides tool call IDs (older models need synthetic ID generation)

**Status:** Skeleton (basic types and endpoints structure, not production-ready)

---

## Documentation Files

### `/docs/provider-translation.md` (150+ lines)
Field-level mapping tables between unified OpenAI format and each provider's native format:
- OpenAI & OpenAI-compat near-passthrough
- Anthropic detailed request/response translation (system message extraction, tool format, tool choice, finish reason mapping)
- Translation rules for messages, content, tools, tool calls, tool results

### `/docs/best-practices.md` (100+ lines)
Type modeling and architecture rationale:
- String-or-array content duality with #[serde(untagged)]
- Internally tagged enums for content parts
- Forward compatibility patterns (Unknown variants)
- Passthrough fields with #[serde(flatten)]
- Thin provider trait (protocol translation, not application framework)
- Provider-specific types with TryFrom/Into

### `/docs/anthropic-api-spec.md`
Anthropic API reference snapshot

### `README.md`
High-level overview, supported providers table, design principles, quick start

### `CONTRIBUTING.md`
Build/test commands, project structure, code conventions (serde, builders via bon, secrets as SecretString, errors with thiserror, streaming with eventsource-stream), adding new providers guide

---

## Key Design Principles

### 1. Protocol Faithfulness
Each provider crate mirrors its upstream API exactly. No fields are silently dropped. Unknown fields flow through `#[serde(flatten)] extra` for forward compatibility.

### 2. No Universal Message Type at Provider Level
Translation between providers happens via aigw-core traits (RequestTranslator, ResponseTranslator, StreamParser) at the gateway layer, not inside individual provider crates. Each provider owns its native wire types.

### 3. Quirks-Based Compat
OpenAI-compatible providers declare capabilities through Quirks. Unsupported fields are stripped before sending, not silently ignored.

### 4. Streaming-First
Full SSE event fidelity for every provider. Canonical StreamEvent enum handles one-to-many mapping (single provider event → multiple canonical events, or many → one).

### 5. Secrets Never Leak
API keys are secrecy::SecretString — they never implement Debug, never appear in logs.

### 6. Thin Traits, No Associated Types
Provider traits are pure protocol translation: RequestTranslator::translate_request(), ResponseTranslator::translate_response(), StreamParser::parse_event(). They return unified types, not provider-specific types. This keeps the gateway's control flow clean.

---

## Common Patterns

### Wire Type Structure
```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct SomeRequest {
    pub required_field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional_field: Option<String>,
    #[serde(flatten, default, skip_serializing_if = "json_object_is_empty")]
    pub extra: JsonObject,
}
```

### String-or-Array Content
```rust
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}
```

### Internally Tagged Enum
```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String, #[serde(flatten)] extra: JsonObject },
    ImageUrl { image_url: ImageUrl, #[serde(flatten)] extra: JsonObject },
}
```

### SSE Event Parsing
Use eventsource-stream to parse SSE. Dispatch on the JSON type field, not the SSE event: line.

### Builder Pattern
Use bon::Builder derive:
```rust
#[derive(bon::Builder)]
#[builder(on(String, into))]
pub struct SomeRequest { ... }
```

---

## Test Coverage

- JSON round-trip tests (deserialize official docs, re-serialize, verify)
- Wire-format snapshot tests
- SSE event replay tests (official spec examples)
- Config validation tests
- Error handling (429, 401, 5xx, malformed responses)
- Streaming termination (connection close vs data: [DONE])
- Rate limit header parsing
- Secret redaction in Debug output

---

## Dependencies

### Core Crate
- serde, serde_json — Serialization
- thiserror — Error handling

### Provider Crates
- reqwest — HTTP client
- futures — Async streams
- tokio — Async runtime (features: rt for client side)
- tokio-stream — Stream utilities
- eventsource-stream — SSE parsing
- bytes — Byte buffer manipulation
- secrecy — Secrets that don't leak in Debug
- bon — Builder pattern derivation

---

## Environment & Build

- Stable Rust: 1.85+ (edition 2024)
- Build: cargo build --workspace
- Test: cargo test --workspace
- Lint: cargo clippy --workspace
- Doc: cargo doc --workspace --open

---

## Current Status

- OpenAI provider: Active (Chat Completions + Responses API)
- Anthropic provider: Active (Messages API + streaming)
- OpenAI-compat: Active (Quirks-based configuration)
- Gemini provider: Skeleton (basic types, not production)

---

## Related Files

- Root Cargo.toml — Workspace members, shared version/edition/license
- AGENTS.md — Agent workflow documentation (if team-based)
- .github/workflows/ — CI/CD (if present)

---

## Quick Navigation

| Goal | Location |
|------|----------|
| Understand unified types | aigw-core/src/model/ |
| Add a new provider | aigw-openai/ as template; add new crate or Quirks config |
| Translate requests | aigw-core/src/translate.rs |
| Parse Anthropic SSE | aigw-anthropic/src/streaming.rs |
| Configure OpenAI-compat | Read OpenAICompatConfig and Quirks |
| View provider mapping | docs/provider-translation.md |
| Understand design rationale | docs/best-practices.md, README.md |

---

## Notes for Development

1. Always preserve unknown fields — Use `#[serde(flatten)] extra` on all request/response/event types. Provider APIs evolve; don't force recompilation.

2. Use aigw-core traits, not direct HTTP — When adding gateway logic, depend on RequestTranslator/ResponseTranslator/StreamParser, not individual provider clients.

3. Provider crates are libraries, not binaries — The gateway binary (if any) lives at /src/main.rs; provider crates are reusable.

4. Quirks before new crate — For an OpenAI-like provider, add a Quirks config. Only create a new crate if the wire format is fundamentally different.

5. SSE events are 1-to-many — Anthropic's content_block_start(tool_use) maps to both ToolCallStart + ToolCallDelta. Use StreamParser's Vec<StreamEvent> return to handle this.

6. Test with official examples — Use examples from official API docs as JSON test fixtures and SSE replay tests.
