# Changelog

All notable changes to this project will be documented in this file.
- test: add browser compatibility tests (2026-02-22)
- refactor: simplify WASM binding initialization (2026-02-22)
- test: add wasm-bindgen-test suite for core API (2026-02-21)
- perf: reduce WASM binary size with lto (2026-02-20)
- feat: add browser-compatible WebSocket transport (2026-02-18)
- style: normalize import paths across modules (2026-02-18)
- feat: add streaming message iterator for JS (2026-02-15)

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
