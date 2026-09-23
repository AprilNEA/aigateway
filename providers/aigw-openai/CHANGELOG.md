# Changelog

All notable changes to this project will be documented in this file.
## [0.6.1](https://github.com/AprilNEA/aigateway/compare/aigw-openai-v0.6.0...aigw-openai-v0.6.1) - 2026-09-23

### Fixed
- *(aigw-openai)* accept chat envelopes without object or created

## [0.6.0](https://github.com/arcboxlabs/aigateway/compare/aigw-openai-v0.5.0...aigw-openai-v0.6.0) - 2026-08-20

### Build
- switch reqwest from native-tls to rustls (webpki-roots)

## [0.5.0](https://github.com/arcboxlabs/aigateway/compare/aigw-openai-v0.4.0...aigw-openai-v0.5.0) - 2026-05-14

### Added
- *(aigw-openai)* Chat Completions thinking projector + Value-level helper- *(aigw-openai)* wire canonical thinking + ReasoningStart/End in Responses- *(aigw-openai)* normalize web_search_preview tool aliases for Codex

### Miscellaneous
- fix CI — clippy + nightly rustfmt

## [0.4.0](https://github.com/arcboxlabs/aigateway/compare/aigw-openai-v0.3.0...aigw-openai-v0.4.0) - 2026-04-15

### Added
- *(aigw-openai)* expose build_responses_create_request for library consumers

## [0.3.0](https://github.com/arcboxlabs/aigateway/compare/aigw-openai-v0.2.0...aigw-openai-v0.3.0) - 2026-04-15

### Added
- *(aigw-openai)* add Responses API translation layer with Codex support

### Style
- apply nightly cargo fmt to new responses translation modules

## [0.2.0](https://github.com/arcboxlabs/aigateway/compare/aigw-openai-v0.1.0...aigw-openai-v0.2.0) - 2026-04-05

### Added
- *(aigw-core)* add canonical model, translator traits, and unify JsonObject- implement translate layer for openai, openai-compat, and anthropic providers

### Fixed
- resolve CI failures — cargo fmt and clippy collapsible_if

### Miscellaneous
- unify all crate versions via workspace.package

## [0.1.0](https://github.com/arcboxlabs/aigateway/compare/aigw-openai-v0.0.1...aigw-openai-v0.1.0) - 2026-04-05

### Added
- *(aigw-openai)* Responses API wire types, transport improvements

### Refactored
- *(aigw-openai)* replace hand-written constructors with bon builders

### Miscellaneous
- update repository org from AprilNEA to arcboxlabs- add GitHub Actions workflow with check, test, fmt, clippy, audit

### Init
- workspace with openai, openai-compat, anthropic, gemini provider crates
