# Changelog

All notable changes to this project will be documented in this file.

## [0.6.1](https://github.com/AprilNEA/aigateway/compare/aigw-gemini-v0.6.0...aigw-gemini-v0.6.1) - 2026-09-23

### Fixed
- *(aigw-openai)* accept chat envelopes without object or created

## [0.6.0](https://github.com/arcboxlabs/aigateway/compare/aigw-gemini-v0.5.0...aigw-gemini-v0.6.0) - 2026-08-20

### Build
- switch reqwest from native-tls to rustls (webpki-roots)

## [0.5.0](https://github.com/arcboxlabs/aigateway/compare/aigw-gemini-v0.4.0...aigw-gemini-v0.5.0) - 2026-05-14

### Added
- *(aigw-gemini)* GenerateContentRequest also derives Deserialize- *(aigw-gemini)* native-protocol bridge (Gemini-native ↔ canonical)- *(aigw-gemini)* expose build_generate_content_request public helper- *(aigw-gemini)* add translate module + canonical thinking projector

### Fixed
- *(aigw-gemini)* schema-type case + native-bridge fidelity

## [0.2.0](https://github.com/arcboxlabs/aigateway/compare/aigw-gemini-v0.1.0...aigw-gemini-v0.2.0) - 2026-04-05

### Fixed
- resolve CI failures — cargo fmt and clippy collapsible_if

### Refactored
- Gemini forward-compat extra fields, normalize validation, umbrella crate aliases

### Miscellaneous
- unify all crate versions via workspace.package

## [0.1.0](https://github.com/arcboxlabs/aigateway/compare/aigw-gemini-v0.0.1...aigw-gemini-v0.1.0) - 2026-04-05

### Added
- *(aigw-gemini)* scaffold Gemini provider crate

### Miscellaneous
- update repository org from AprilNEA to arcboxlabs

### Init
- workspace with openai, openai-compat, anthropic, gemini provider crates

### Style
- apply cargo fmt to aigw-gemini, update lockfile
