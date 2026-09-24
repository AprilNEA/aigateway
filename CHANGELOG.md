# Changelog

All notable changes to this project will be documented in this file.
## [0.7.0](https://github.com/AprilNEA/aigateway/compare/v0.6.1...v0.7.0) - 2026-09-24

### Fixed
- *(aigw-anthropic)* [**breaking**] spell cache_control ttl as the API does

## [0.6.1](https://github.com/AprilNEA/aigateway/compare/v0.6.0...v0.6.1) - 2026-09-23

### Fixed
- *(deps)* patch the rustls advisory and a yanked chacha20- *(aigw-openai)* accept chat envelopes without object or created

### Miscellaneous
- move actions off the removed Node 20 runtime- *(release)* release as aprilnea[bot] from AprilNEA/aigateway

## [0.6.0](https://github.com/arcboxlabs/aigateway/compare/v0.5.0...v0.6.0) - 2026-08-20

### Added
- *(aigateway)* structured logging (tracing)- *(aigateway)* echo the requested Anthropic model in responses- *(aigateway)* expose upstream Quirks in config- *(aigateway)* keepalive ping during streaming- *(aigateway)* add /v1/models and /v1/messages/count_tokens- *(aigateway)* openai-responses wire, richer stream usage, upstream proxy- *(aigateway)* loopback Anthropic → OpenAI-compatible gateway server- *(aigw-anthropic)* add inbound native bridge

### Fixed
- *(aigateway)* accept inline system messages ([#7](https://github.com/arcboxlabs/aigateway/pull/7))- *(aigateway)* raise inbound body limit to 64 MiB- *(aigateway)* always terminate the Anthropic stream- *(aigw-anthropic)* clean placeholder for image tool results

### Documentation
- document the gateway sidecar spawn contract and config

### Miscellaneous
- build darwin-x64 on macos-14 (cross-compile)- build + publish per-platform binaries on release tags- add devenv development environment

### Build
- switch reqwest from native-tls to rustls (webpki-roots)

## [0.5.0](https://github.com/arcboxlabs/aigateway/compare/v0.4.0...v0.5.0) - 2026-05-14

### Added
- *(aigw-openai)* Chat Completions thinking projector + Value-level helper- *(aigw-openai)* wire canonical thinking + ReasoningStart/End in Responses- *(aigw-openai)* normalize web_search_preview tool aliases for Codex- *(aigw-gemini)* add translate module + canonical thinking projector- *(aigw-anthropic)* pluggable cache_control strategy + always-on API rules- *(aigw-anthropic)* wire canonical thinking + source-tagged history filter- *(aigw-gemini)* GenerateContentRequest also derives Deserialize- *(aigw-gemini)* native-protocol bridge (Gemini-native ↔ canonical)- *(aigw-gemini)* expose build_generate_content_request public helper

### Fixed
- *(aigw-gemini)* schema-type case + native-bridge fidelity

### Testing
- add live OAuth e2e suite for subscription-backed providers

### Miscellaneous
- fix CI — clippy + nightly rustfmt

## [0.4.0](https://github.com/arcboxlabs/aigateway/compare/v0.3.0...v0.4.0) - 2026-04-15

### Added
- *(aigw-openai)* expose build_responses_create_request for library consumers

## [0.3.0](https://github.com/arcboxlabs/aigateway/compare/v0.2.0...v0.3.0) - 2026-04-15

### Added
- *(aigw-openai)* add Responses API translation layer with Codex support

### Documentation
- add Responses API translation reference and update design notes

### Miscellaneous
- *(release-plz)* disable semver_check during 0.x rapid iteration

### Style
- apply nightly cargo fmt to new responses translation modules

## [0.2.0](https://github.com/arcboxlabs/aigateway/compare/v0.1.0...v0.2.0) - 2026-04-05

### Added
- *(aigw-core)* add canonical model, translator traits, and unify JsonObject- implement translate layer for openai, openai-compat, and anthropic providers

### Fixed
- resolve CI failures — cargo fmt and clippy collapsible_if

### Refactored
- Gemini forward-compat extra fields, normalize validation, umbrella crate aliases

### Miscellaneous
- unify all crate versions via workspace.package

## [0.1.0](https://github.com/arcboxlabs/aigateway/compare/v0.0.1...v0.1.0) - 2026-04-05

### Added
- *(aigw)* add umbrella crate re-exporting all providers- *(aigw-openai)* Responses API wire types, transport improvements- *(aigw-anthropic)* transport layer, rate limits, split types modules- *(aigw-gemini)* scaffold Gemini provider crate

### Refactored
- *(anthropic)* replace hand-written builders with bon derive- *(aigw-openai)* replace hand-written constructors with bon builders- *(aigw-anthropic)* review fixes, SecretString, bon builder, docs

### Documentation
- add README with architecture diagram, provider logos, and CONTRIBUTING guide- *(aigw-openai-compat)* translate README from Chinese to English

### Miscellaneous
- add release-plz workflow and config for automated releases- update repository org from AprilNEA to arcboxlabs- add GitHub Actions workflow with check, test, fmt, clippy, audit

### Init
- workspace with openai, openai-compat, anthropic, gemini provider crates

### Style
- apply cargo fmt to aigw-gemini, update lockfile
