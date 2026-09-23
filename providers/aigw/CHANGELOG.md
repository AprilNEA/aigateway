# Changelog

All notable changes to this project will be documented in this file.
## [0.6.1](https://github.com/AprilNEA/aigateway/compare/aigw-v0.6.0...aigw-v0.6.1) - 2026-09-23

### Fixed
- *(aigw-openai)* accept chat envelopes without object or created

## [0.6.0](https://github.com/arcboxlabs/aigateway/compare/aigw-v0.5.0...aigw-v0.6.0) - 2026-08-20

### Added
- *(aigateway)* echo the requested Anthropic model in responses- *(aigateway)* openai-responses wire, richer stream usage, upstream proxy- *(aigw-anthropic)* add inbound native bridge

### Fixed
- *(aigateway)* accept inline system messages ([#7](https://github.com/arcboxlabs/aigateway/pull/7))- *(aigw-anthropic)* clean placeholder for image tool results

### Build
- switch reqwest from native-tls to rustls (webpki-roots)

## [0.5.0](https://github.com/arcboxlabs/aigateway/compare/aigw-v0.4.0...aigw-v0.5.0) - 2026-05-14

### Added
- *(aigw-openai)* Chat Completions thinking projector + Value-level helper- *(aigw-openai)* wire canonical thinking + ReasoningStart/End in Responses- *(aigw-openai)* normalize web_search_preview tool aliases for Codex- *(aigw-gemini)* add translate module + canonical thinking projector- *(aigw-anthropic)* pluggable cache_control strategy + always-on API rules- *(aigw-anthropic)* wire canonical thinking + source-tagged history filter- *(aigw-gemini)* GenerateContentRequest also derives Deserialize- *(aigw-gemini)* native-protocol bridge (Gemini-native ↔ canonical)- *(aigw-gemini)* expose build_generate_content_request public helper

### Fixed
- *(aigw-gemini)* schema-type case + native-bridge fidelity

### Miscellaneous
- fix CI — clippy + nightly rustfmt

## [0.4.0](https://github.com/arcboxlabs/aigateway/compare/aigw-v0.3.0...aigw-v0.4.0) - 2026-04-15

### Added
- *(aigw-openai)* expose build_responses_create_request for library consumers

## [0.3.0](https://github.com/arcboxlabs/aigateway/compare/aigw-v0.2.0...aigw-v0.3.0) - 2026-04-15

### Added
- *(aigw-openai)* add Responses API translation layer with Codex support

### Style
- apply nightly cargo fmt to new responses translation modules

## [0.2.0](https://github.com/arcboxlabs/aigateway/compare/aigw-v0.0.1...aigw-v0.2.0) - 2026-04-05

### Added
- *(aigw-core)* add canonical model, translator traits, and unify JsonObject- implement translate layer for openai, openai-compat, and anthropic providers

### Fixed
- resolve CI failures — cargo fmt and clippy collapsible_if

### Refactored
- Gemini forward-compat extra fields, normalize validation, umbrella crate aliases

### Miscellaneous
- unify all crate versions via workspace.package

## [0.0.1](https://github.com/arcboxlabs/aigateway/releases/tag/aigw-v0.0.1) - 2026-04-05

### Added
- *(aigw)* add umbrella crate re-exporting all providers
