# Gateway sidecar

The `aigateway` binary runs as a **loopback sidecar**: it exposes an inbound
Anthropic Messages endpoint (`POST /v1/messages`), translates each request to a
configured OpenAI-compatible upstream, and translates the response — including
streaming SSE, tool calls, and thinking blocks — back into Anthropic wire
format.

This lets a bare OpenAI-compatible or local-model API key back a **Claude Code**
session offline, with no hosted gateway in the middle. A supervising daemon
(e.g. LinkCode) spawns the binary, reads the bound address from stdout, and
points Claude Code's `ANTHROPIC_BASE_URL` at it.

## CLI / spawn contract

```
aigateway serve --host 127.0.0.1 --port 0 --config <path>
```

| Flag       | Default     | Meaning                                                    |
| ---------- | ----------- | ---------------------------------------------------------- |
| `--host`   | `127.0.0.1` | Bind address. Keep on loopback.                            |
| `--port`   | `0`         | Bind port. `0` lets the OS assign a free port.             |
| `--config` | *(required)*| Path to the TOML config file.                             |
| `--log-level` | `info`   | Log level (`error`/`warn`/`info`/`debug`/`trace`). `RUST_LOG` overrides. |

Structured logs go to **stderr** (ANSI colors only on a tty); the `listening
on …` line is the **only** thing on stdout, so a daemon can parse it cleanly.

On successful bind — **before** serving — the process prints exactly one line to
**stdout** and flushes it:

```
listening on http://127.0.0.1:49157
```

A spawning daemon should read stdout until it sees this line, parse the
`http://<host>:<port>` address, and use it as the Anthropic base URL. With
`--port 0` this is the only way to learn the actual port (mirrors the
`linkcode-pty/runtime.json` pattern). Errors (bad config, bind failure) are
reported on stderr with a non-zero exit code.

Shutdown is graceful on `SIGTERM` or `Ctrl-C` (`SIGINT`).

## Configuration (TOML)

Minimal:

```toml
[upstream]
base_url = "https://api.openai.com/v1"
api_key  = "sk-..."
```

Full:

```toml
[upstream]
base_url        = "https://api.openai.com/v1"
api_key         = "sk-..."
wire            = "openai-chat"   # "openai-chat" (default) | "openai-responses"
timeout_seconds = 600             # idle read timeout; long streams are never cut
# proxy         = "http://127.0.0.1:8888"   # route upstream traffic via a proxy

# Map inbound (Anthropic) model names → upstream model names.
[upstream.models]
"claude-sonnet-4-20250514" = "gpt-4.1"
"claude-3-5-haiku-20241022" = "gpt-4.1-mini"

# Optional: fall back to this upstream model when no mapping matches.
# When unset, the inbound model name is forwarded unchanged.
default_model = "gpt-4.1"

# Optional: extra headers sent on every upstream request.
[upstream.default_headers]
x-custom-header = "value"

# Optional: openai-chat capability flags. Defaults are all-true; set false to
# strip fields a limited/local upstream rejects.
[upstream.quirks]
supports_tool_choice = true
supports_parallel_tool_calls = true
supports_vision = true
supports_streaming = true
```

| Field                    | Required | Notes                                                             |
| ------------------------ | -------- | ---------------------------------------------------------------- |
| `upstream.base_url`      | yes      | OpenAI-compatible base; `/chat/completions` or `/responses` is appended. |
| `upstream.api_key`       | yes      | Injected as `Authorization: Bearer <key>`. Redacted in logs.    |
| `upstream.wire`          | no       | `openai-chat` (default) or `openai-responses`.                   |
| `upstream.timeout_seconds` | no     | Idle read timeout (default 600s), not a total-request timeout.   |
| `upstream.models`        | no       | Inbound → upstream model name map.                               |
| `upstream.default_model` | no       | Fallback upstream model; passthrough if unset.                   |
| `upstream.proxy`         | no       | Proxy URL for upstream traffic; direct (bypasses system proxy) if unset. |
| `upstream.default_headers` | no     | Extra upstream request headers.                                  |
| `upstream.quirks`        | no       | `openai-chat` capability flags; strips unsupported fields for limited upstreams. |

## Endpoints

### `POST /v1/messages`

Accepts an Anthropic [Messages API](https://docs.anthropic.com/en/api/messages)
request body. `max_tokens` is required (as upstream Anthropic requires). Honors
`stream: true` for SSE.

- **Streaming** returns `Content-Type: text/event-stream` with the Anthropic
  event lifecycle: `message_start` → (`content_block_start` →
  `content_block_delta`* → `content_block_stop`)* → `message_delta` →
  `message_stop`. Text, `tool_use` (with streamed `input_json_delta`), and
  `thinking` blocks (with `signature_delta`) are all emitted.
- **Unary** returns a JSON Anthropic `message` object.

### `POST /v1/messages/count_tokens`

Anthropic's token-count endpoint. OpenAI upstreams have no equivalent, so this
returns a **heuristic** estimate (`~chars/4` over text + tool schemas). Advisory
only — Claude Code uses it for the context indicator and degrades gracefully
when it's imprecise.

### `GET /v1/models`

Lists the inbound Anthropic model names the gateway accepts — the configured
`[upstream.models]` keys — in Anthropic's `{"data":[…],"has_more":false}` shape.
Claude Code probes this at startup, so keep `[upstream.models]` populated with
the model names you'll request.

### `GET /health`

Returns `200 {"status":"ok"}`.

## Authentication

Inbound authentication is **ignored** — Claude Code sends a placeholder token,
which the gateway discards. The real upstream key is injected from config. Only
bind loopback; do not expose this endpoint to a network.

## Behavior notes

- **Keepalive** — during streaming the gateway emits an Anthropic `ping` event
  after each idle gap (~15s of upstream silence), so a slow or local upstream
  doesn't idle-timeout the client before the first token.
- **Networking** — by default the outbound HTTP client bypasses the system
  proxy, so a loopback sidecar talks directly to its configured upstream
  (important for `localhost` model servers). Set `upstream.proxy` to route
  upstream traffic through an explicit proxy instead.
- **Token accounting** — OpenAI-style upstreams (both wires) report usage only
  at the end of a stream, after `message_start` has already been sent. So
  streaming `message_start.usage.input_tokens` is `0`; the real input/output
  counts — plus `cache_read_input_tokens` when the upstream reports cache hits —
  arrive in the terminal `message_delta.usage`, which Claude Code reads. Unary
  responses carry full usage directly.
- **Response `model`** — the response echoes the client's requested Anthropic
  model name (so Claude Code's model-based logic stays consistent), not the
  upstream model that actually served it.

## Example

Start the gateway:

```bash
aigateway serve --host 127.0.0.1 --port 0 --config gateway.toml
# → listening on http://127.0.0.1:49157
```

Point Claude Code at it:

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:49157"
export ANTHROPIC_API_KEY="placeholder"   # ignored by the gateway
```

Or drive it directly:

```bash
curl -N http://127.0.0.1:49157/v1/messages \
  -H 'content-type: application/json' \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 256,
    "stream": true,
    "messages": [{ "role": "user", "content": "Hello!" }]
  }'
```

## Prebuilt binaries

The `release-binaries` workflow (`.github/workflows/release-binaries.yml`) runs
on every `v<version>` tag and attaches per-platform archives to that GitHub
Release, for tools like LinkCode's managed-asset catalog to download.

| Platform key | Rust target                  | Archive                                             |
| ------------ | ---------------------------- | --------------------------------------------------- |
| darwin-arm64 | aarch64-apple-darwin         | `aigateway-<version>-aarch64-apple-darwin.tar.gz`   |
| darwin-x64   | x86_64-apple-darwin          | `aigateway-<version>-x86_64-apple-darwin.tar.gz`    |
| linux-arm64  | aarch64-unknown-linux-musl   | `aigateway-<version>-aarch64-unknown-linux-musl.tar.gz` (static) |
| linux-x64    | x86_64-unknown-linux-musl    | `aigateway-<version>-x86_64-unknown-linux-musl.tar.gz` (static)  |
| win32-x64    | x86_64-pc-windows-msvc       | `aigateway-<version>-x86_64-pc-windows-msvc.zip`    |

- Archive members: `aigateway` (unix) / `aigateway.exe` (win), at the archive root.
- Linux builds are fully static musl (no glibc, no OpenSSL — rustls).
- Asset URL: `https://github.com/AprilNEA/aigateway/releases/download/v<version>/<archive>`.
- **Integrity**: a `SHA256SUMS` asset lists hex digests, and the release notes
  include an SRI column (`sha256-<base64>`) for each archive. The GitHub Release
  API also exposes each asset's `digest` (`sha256:<hex>`). SRI = base64 of the
  raw SHA-256 bytes, e.g. `openssl dgst -sha256 -binary <file> | base64`.
