<div align="center">

# ⚡ AI Gateway

**A protocol-faithful, multi-provider AI gateway written in Rust.**

Route requests across OpenAI, Anthropic, Google Gemini, and the entire OpenAI-compatible ecosystem — with native wire types, streaming SSE, and zero lowest-common-denominator abstractions.

[![CI](https://github.com/AprilNEA/aigateway/actions/workflows/ci.yml/badge.svg)](https://github.com/AprilNEA/aigateway/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-2024_edition-orange)

</div>

---

## Why AI Gateway?

Most multi-provider LLM wrappers force every provider into a single "universal" message format, silently dropping fields and features along the way. AI Gateway takes the opposite approach:

- **Protocol-faithful** — Each provider crate models the upstream API exactly as documented. No fields are silently dropped, no semantics are lost.
- **Forward-compatible** — All wire types carry `#[serde(flatten)] extra` to survive upstream API changes without recompilation.
- **Translation at the edge** — Cross-provider mapping happens at the gateway layer via `TryFrom`/`Into`, not inside individual providers.
- **Streaming-first** — Full SSE event fidelity for every provider.

## Architecture

```mermaid
flowchart TB
    Client([Your Application])

    Client -->|Unified API| Gateway

    subgraph Gateway["⚡ AI Gateway"]
        direction TB
        Router[Request Router]
        Translate[Protocol Translation]
        Stream[SSE Streaming Engine]
        Router --> Translate --> Stream
    end

    subgraph Providers["Provider Crates"]
        direction LR
        OpenAI["aigw-openai"]
        Anthropic["aigw-anthropic"]
        Compat["aigw-openai-compat"]
        Gemini["aigw-gemini"]
    end

    Gateway --> OpenAI & Anthropic & Compat & Gemini

    OpenAI -->|Responses API\nChat Completions| OpenAI_API["OpenAI API"]
    Anthropic -->|Messages API| Anthropic_API["Anthropic API"]
    Compat -->|Chat Completions| Compat_APIs["Groq · Together\nvLLM · Fireworks\nPerplexity · LM Studio"]
    Gemini -->|generateContent| Gemini_API["Google Gemini API"]
```

## Supported Providers

<table>
  <tr>
    <th></th>
    <th>Provider</th>
    <th>Crate</th>
    <th>Chat</th>
    <th>Streaming</th>
    <th>Status</th>
  </tr>
  <tr>
    <td><img src="docs/images/openai.svg" width="28" /></td>
    <td><b>OpenAI</b></td>
    <td><a href="providers/aigw-openai/"><code>aigw-openai</code></a></td>
    <td>✅</td>
    <td>✅</td>
    <td>🚧 Active</td>
  </tr>
  <tr>
    <td><img src="docs/images/anthropic.svg" width="28" /></td>
    <td><b>Anthropic</b></td>
    <td><a href="providers/aigw-anthropic/"><code>aigw-anthropic</code></a></td>
    <td>✅</td>
    <td>✅</td>
    <td>🚧 Active</td>
  </tr>
  <tr>
    <td><img src="docs/images/gemini.svg" width="28" /></td>
    <td><b>Google Gemini</b></td>
    <td><a href="providers/aigw-gemini/"><code>aigw-gemini</code></a></td>
    <td>–</td>
    <td>–</td>
    <td>🏗️ Skeleton</td>
  </tr>
</table>

### OpenAI-Compatible Providers via [`aigw-openai-compat`](providers/aigw-openai-compat/)

Configure any OpenAI-compatible provider with a `base_url` + `Quirks` capability flags — no new crate needed.

<p>
  <img src="docs/images/groq.svg" width="24" title="Groq" />&ensp;
  <img src="docs/images/together.svg" width="24" title="Together AI" />&ensp;
  <img src="docs/images/fireworks.svg" width="24" title="Fireworks" />&ensp;
  <img src="docs/images/perplexity.svg" width="24" title="Perplexity" />&ensp;
  <img src="docs/images/ollama.svg" width="24" title="Ollama" />&ensp;
  <img src="docs/images/vllm.svg" width="24" title="vLLM" />&ensp;
  <img src="docs/images/lm-studio.svg" width="24" title="LM Studio" />
</p>

> **Adding a new provider?** If the API is OpenAI-compatible, add a `Quirks` config to `aigw-openai-compat`. Only create a new crate for providers with a distinct wire format.

## Run as a gateway sidecar

Beyond the provider libraries, the `aigateway` binary runs as a **loopback
sidecar** that serves the Anthropic Messages API (`POST /v1/messages`) on top of
any OpenAI-compatible upstream — so a bare OpenAI/local-model key can back a
**Claude Code** session offline, with no hosted gateway.

```toml
# gateway.toml
[upstream]
base_url = "https://api.openai.com/v1"
api_key  = "sk-..."

[upstream.models]
"claude-sonnet-4-20250514" = "gpt-4.1"
```

```bash
aigateway serve --host 127.0.0.1 --port 0 --config gateway.toml
# → listening on http://127.0.0.1:49157

export ANTHROPIC_BASE_URL="http://127.0.0.1:49157"   # point Claude Code here
```

Streaming SSE, tool calls, and thinking blocks are translated back to Anthropic
wire format. See [**docs/gateway-sidecar.md**](docs/gateway-sidecar.md) for the
full CLI/spawn contract, config schema, and behavior notes.

## Design Principles

### No universal message type

Each provider crate defines its own native request/response types mirroring the upstream API. Translation between providers is handled via `TryFrom`/`Into` at the gateway layer.

### Quirks-based compat

Third-party OpenAI-compatible providers declare their capabilities through a `Quirks` struct. Unsupported fields are stripped before sending, not silently ignored.

```rust
Quirks {
    supports_responses_api: false,
    supports_tool_choice: true,
    supports_parallel_tool_calls: false,
    supports_vision: true,
    supports_streaming: true,
}
```

### Secrets never leak

API keys are stored as `secrecy::SecretString` — they never implement `Debug`, never appear in logs.

## Quick Start

```bash
cargo build --workspace         # Build
cargo test --workspace          # Test
cargo clippy --workspace        # Lint
```

See [**CONTRIBUTING.md**](CONTRIBUTING.md) for the full development guide, code conventions, and how to add new providers.

## License

[MIT](LICENSE)
