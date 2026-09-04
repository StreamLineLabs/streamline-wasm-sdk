# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

### Fixed
- `Producer.flush()` previously drained the entire batch up front
  (`std::mem::take`) and then attempted to send every message, but *always*
  returned `Ok(())` regardless of outcome — any message whose send failed
  (or any message queued after the first failure) was permanently lost, and
  the caller was never told delivery had failed. `flush()` now sends
  messages in order and stops at the first failure: the failed message and
  every unattempted message after it are preserved in the batch (inspect via
  `pending_count()`), in their original order, ready for a retry; messages
  already sent successfully are removed so a retried `flush()` never
  duplicates them; and the underlying transport error is now returned
  instead of being swallowed.
- `Producer.send()`/`send_keyed()` auto-flush (triggered once the batch
  reaches `batch_size`) inherited the bug above, so a threshold-triggered
  flush could silently drop records and report success even though nothing
  was delivered. Fixed by the `flush()` change above — `send()`/`send_keyed()`
  already propagate `flush()`'s `Result`, so a failed auto-flush now
  correctly surfaces as an `Err` from `send()`/`send_keyed()` with the
  undelivered record(s) retained in the batch instead of silently dropped.
- Delivery error metrics previously counted every retained and unattempted
  record as a fresh failure on each flush. `total_errors()` and the second
  `on_delivery` callback argument now count only actual failed send attempts;
  retained backlog size remains available independently through
  `pending_count()`.
- `Producer.disconnect()` called `flush()` and discarded its result
  (`let _ = self.flush();`) before closing the socket, so a lost flush during
  disconnect was invisible to the caller even though records may not have
  been delivered. `disconnect()` now returns `Result<(), JsValue>`: the
  socket is still always closed (this is an intentional disconnect), but any
  flush failure is surfaced truthfully and undelivered records remain in the
  batch rather than being silently discarded.
- `WsConnection`'s `onclose` handler treated **any** close with code 1000
  ("Normal Closure") as equivalent to our own intentional `disconnect()` call
  and unconditionally suppressed reconnection — even when the *peer* (server
  restart, load balancer, idle timeout, etc.) closed the socket cleanly and
  `auto_reconnect` was still enabled. Only `intentional_disconnect` (set by
  our own `disconnect()` call) now suppresses reconnection; a peer-initiated
  close with code 1000 is treated like any other unexpected drop and
  reconnects when `auto_reconnect` is enabled, matching a normal server
  restart or graceful peer-side close.
- WebSocket reconnection now replays active subscriptions on the *new* socket
  instance after a drop, instead of silently going quiet until the caller
  re-subscribes. `StreamlineClient`/`Consumer` re-send their persisted
  `subscribe` message once the reconnected socket reaches `Connected`.
- Explicit disconnects and disabling auto-reconnect now cancel any pending
  reconnect timeout, preventing a stale timer from retaining browser state or
  reconnecting after teardown.
- Callback ownership (`on_message`, `on_state_change`, `on_reconnect_failed`)
  moved behind `WsConnection` setter methods instead of public struct fields,
  removing a class of bugs where a caller could overwrite/desync callback
  state relative to the connection's internal lifecycle.
- `StreamlineClient.subscribe()`/`Consumer.start()` previously shared a
  single `on_message` callback slot per `WsConnection`, so subscribing to a
  second topic silently overwrote the first topic's callback on the same
  client. Incoming messages are now demultiplexed per topic (`WsConnection`
  keeps a callback per topic, keyed off the message's `topic` field), so
  each subscription keeps its own callback; `unsubscribe()`/`Consumer.stop()`
  remove only their own topic's registration, and reconnection replay
  preserves per-topic callback ownership across the new socket.
- `Consumer.commit()`, `commit_offset()`, and auto-commit previously sent a
  `commit_offset` message and then
  *unconditionally* treated it as successful — updating `committed_offset`
  locally even though this SDK has no wire protocol for the broker to
  acknowledge a commit. This claimed a durability guarantee the SDK could not
  verify. `commit()`/`commit_offset()` now fail closed, and non-zero
  `set_auto_commit()` is rejected immediately with `ErrorCode.Unsupported`;
  `committed_offset()` always reports `-1`. `current_offset` tracking is
  independent and is now *wired directly* to messages delivered via
  `Consumer.start()` (previously it only advanced when a caller manually
  invoked `advance_offset()`).
- CodeQL was scanning this Rust codebase with the `cpp` language extractor;
  it now uses the `rust` extractor so code scanning results are meaningful.
- `make integration-test` / the `integration` CI workflow previously started
  an unpinned `ghcr.io/streamlinelabs/streamline:latest` container and ran
  `wasm-pack test --headless --chrome || true`, so the mandatory live-browser
  suite could report success without exercising a real server. Both now
  require an explicitly configured fixture (`STREAMLINE_FIXTURE_IMAGE` or
  `STREAMLINE_LIVE_HEALTH_URL`/`STREAMLINE_LIVE_WEBSOCKET_URL`) and fail
  closed — with no fallback — when one isn't provided.
- `cargo clippy --all-targets` in CI/`make lint` only checked the host target,
  silently skipping `#[cfg(target_arch = "wasm32")]` code paths. It now also
  runs against `wasm32-unknown-unknown`, the crate's actual target.

### Added
- `ErrorCode.Unsupported` and `StreamlineError::unsupported()`: a fail-closed
  error category for operations this SDK version deliberately refuses to
  perform rather than claim an unverifiable result — currently used by
  `Consumer.commit()`/`commit_offset()`.
- `Consumer.is_connected()`, matching the existing method on
  `StreamlineClient`/`TopicAdmin`.
- `tests/live_browser.rs`: a `live-browser-tests`-gated, mandatory
  headless-Chrome test against a real configured Streamline fixture
  (`STREAMLINE_LIVE_WEBSOCKET_URL`). No fixture means the gate fails instead
  of silently skipping.
- `tests/browser_reconnect.rs`: browser-based mock-WebSocket coverage for
  shared callback ownership, late-bound message/state callbacks, retained
  socket identity across reconnects, subscription replay after a forced
  disconnect, per-topic subscription demultiplexing (including across
  unsubscribe and reconnect), `Consumer` offset-advancement wiring plus
  fail-closed commit/auto-commit behavior, `Producer.flush()` order
  preservation / no-duplication under a simulated transport failure, and
  peer-initiated vs. intentional close-code-1000 reconnect behavior.
- Release workflow `package-checks` job: validates that the tag, `Cargo.toml`,
  and `package.json` versions match, builds both wasm-pack targets, asserts
  the npm package surface (`.d.ts`, `.wasm`, `NOTICE`) is present and exports
  the expected public API, and runs `npm pack --dry-run` to catch packaging
  regressions before publish.

### Changed
- `Producer.disconnect()` now returns `Result<(), JsValue>` instead of `()`.
  The socket is still closed unconditionally, but a flush failure during
  disconnect is now returned to the caller instead of being silently
  discarded; existing callers that ignore the return value are unaffected.
- Cargo package now declares `publish = false`; the supported distribution
  channel is the npm package `@streamlinelabs/streamline-wasm`, not
  crates.io.
- `npm publish` now runs with `--provenance`.
- `deny.toml` targets include `wasm32-unknown-unknown`, and `unknown-git` is
  now denied rather than a warning.
- Removed the hand-maintained `streamline-wasm.d.ts`; the npm package now
  ships only the `.d.ts` generated by `wasm-pack`/`wasm-bindgen`, avoiding
  drift between hand-written and generated type definitions.
- Documented the current lack of production TLS/token examples for
  `examples/security.js`: the browser `WebSocket` API cannot set an
  `Authorization` header, and this SDK deliberately avoids passing bearer
  tokens via the WebSocket URL.


## [0.3.0] - 2026-04-20

### Added
- `moonshot` module — read-safe subset of the Streamline Moonshot HTTP API
  exposed via `wasm_bindgen`:
  - `SearchClient.search(topic, query, k)` (M2)
  - `MemoryReadClient.recall(agent, query, k)` (M1)
- Excluded by design: attestation signing, contract registration, branch
  mutation, and `memory/remember`. These are admin/write/signing operations
  that must not run in browser contexts; route through a server-side gateway.

### Added
- Typed error system (`error.rs`): `StreamlineError` and `ErrorCode` enums exported to JS via wasm_bindgen
- Producer batching with configurable `batch_size`, auto-flush on threshold, and `flush()` method
- `pending_count()` method on Producer for batch inspection

### Fixed
- WebSocket errors now use typed `StreamlineError` instead of opaque `JsValue` strings
- Producer `send()`/`send_keyed()` errors use typed `StreamlineError::produce_error()`

### Changed
- fix: resolve WASM memory growth issue (2026-03-06)
- test: add playwright e2e tests (2026-03-06)
- refactor: optimize serialization for WASM target (2026-03-06)
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
- test: scaffold WASM runtime initialization conformance tests
- test: add conformance test for WASM memory boundary limits
- test: add edge case assertions for WASM stack overflow
