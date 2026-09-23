# Changelog

All notable changes to this project will be documented in this file.

## [0.6.1](https://github.com/AprilNEA/aigateway/compare/aigw-anthropic-v0.6.0...aigw-anthropic-v0.6.1) - 2026-09-23

### Fixed
- *(aigw-openai)* accept chat envelopes without object or created

## [0.6.0](https://github.com/arcboxlabs/aigateway/compare/aigw-anthropic-v0.5.0...aigw-anthropic-v0.6.0) - 2026-08-20

### Added
- *(aigateway)* echo the requested Anthropic model in responses- *(aigateway)* openai-responses wire, richer stream usage, upstream proxy- *(aigw-anthropic)* add inbound native bridge

### Fixed
- *(aigateway)* accept inline system messages ([#7](https://github.com/arcboxlabs/aigateway/pull/7))- *(aigw-anthropic)* clean placeholder for image tool results

### Build
- switch reqwest from native-tls to rustls (webpki-roots)

## [0.5.0](https://github.com/arcboxlabs/aigateway/compare/aigw-anthropic-v0.4.0...aigw-anthropic-v0.5.0) - 2026-05-14

### Added
- *(aigw-anthropic)* pluggable cache_control strategy + always-on API rules- *(aigw-anthropic)* wire canonical thinking + source-tagged history filter

### Miscellaneous
- fix CI — clippy + nightly rustfmt

## [0.2.0](https://github.com/arcboxlabs/aigateway/compare/aigw-anthropic-v0.1.0...aigw-anthropic-v0.2.0) - 2026-04-05

### Added
- *(aigw-core)* add canonical model, translator traits, and unify JsonObject- implement translate layer for openai, openai-compat, and anthropic providers

### Fixed
- resolve CI failures — cargo fmt and clippy collapsible_if

### Miscellaneous
- unify all crate versions via workspace.package

## [0.1.0](https://github.com/arcboxlabs/aigateway/compare/aigw-anthropic-v0.0.1...aigw-anthropic-v0.1.0) - 2026-04-05

### Added
- *(aigw-anthropic)* transport layer, rate limits, split types modules

### Refactored
- *(aigw-anthropic)* review fixes, SecretString, bon builder, docs- *(anthropic)* replace hand-written builders with bon derive

### Miscellaneous
- update repository org from AprilNEA to arcboxlabs- add GitHub Actions workflow with check, test, fmt, clippy, audit

### Init
- workspace with openai, openai-compat, anthropic, gemini provider crates
