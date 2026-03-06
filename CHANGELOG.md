# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


- fix: resolve WASM memory growth issue (2026-03-06)
## [Unreleased]

## [0.2.0] - 2026-02-28
- **Testing**: add wasm-pack test suite for browser target
- **Fixed**: resolve memory leak in message buffer allocation
- **Added**: add wasm-bindgen exports for producer API

### Added
- Browser-compatible WebSocket transport
- Streaming message iterator for JS

### Changed
- Simplify WASM binding initialization

### Performance
- Reduce WASM binary size with LTO

## [0.2.0] - 2024-01-01

### Added

- Initial release of the Streamline WASM SDK
- `StreamlineClient` — high-level WebSocket client for browser environments
- `Producer` — send messages to topics via WebSocket
- `Consumer` — subscribe to topics and receive messages with callback pattern
- `TopicAdmin` — create, delete, and list topics via admin messages
- `WsConnection` — WebSocket transport with auto-reconnection and exponential backoff
- JSON-based browser protocol (`BrowserMessage` / `BrowserResponse`)
- Full `#[wasm_bindgen]` exports for JavaScript interop
- npm package support (`@streamlinelabs/streamline-wasm-sdk`)
- GitHub Actions CI with wasm-pack build and test

