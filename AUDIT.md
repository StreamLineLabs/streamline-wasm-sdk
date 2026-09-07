# Clean Code and SRP Audit

## Summary

- **Highest-leverage future split:** separate `Producer` batch delivery from
  transaction buffering after stronger browser callback/timer coverage.
- Public modules and wasm-bindgen types are consumed directly from JavaScript
  and Rust; moving them can change generated TypeScript/module paths even when
  Rust re-exports compile.
- `AdminClient` and `QueryClient` share HTTP mechanics but own different public
  contracts; extraction should target a private request helper, not merge the
  clients.
- Strict Clippy, formatting, native tests, WASM build, and headless Chrome tests
  are now green.
- Large protocol/schema test blocks reflect contract breadth, not mixed
  production responsibilities.

## Findings

| ID | Location | Category | Severity | Actors in conflict | Cost | Size | Behavior risk |
|---|---|---|---|---|---|---|---|
| WASM-SRP-1 | `src/lib.rs:170-359` `Producer` | State partition | P2 | batch delivery; transaction semantics; JS callbacks | Batch queue/counters/callbacks and transaction buffer/state change for different reasons but share flush ordering. | M | High |
| WASM-SRP-2 | `src/admin.rs` | Module SRP | P2 | topic/group/server admin; SQL query API; HTTP transport | Admin and query public clients plus DTOs share one public module and repeated request mechanics. | L | High |
| WASM-CC-1 | `src/websocket.rs` | Stateful lifecycle | P2 | browser events; reconnect policy; callbacks | Connection state, event closures, backoff, and deliberate disconnect share one wasm-bound type; extraction risks closure lifetime regressions. | L | High |

## Ordered Refactor Sequence

1. Add headless-browser tests for delivery callbacks, transaction commit/abort
   ordering, reconnect cancellation, and closure lifetime.
2. Move batch state unchanged into a private Rust value type that is not
   wasm-bound.
3. Keep `Producer` as the wasm-bindgen facade and transaction orchestrator.
4. Characterize Admin/Query request paths, headers, auth, status, and decoding.
5. Extract only private HTTP request mechanics while retaining public module
   paths and wasm-bindgen names.

## Deferred

- Producer and WebSocket state extraction lack enough browser lifecycle
  characterization.
- Moving public types/modules requires generated TypeScript compatibility
  checks and a release decision.
- HTTP route changes require the org contract decision.

## Out of Scope

- `protocol.rs`: wire contract definitions/tests.
- `schema_registry.rs`: one Schema Registry actor despite length.
- `circuit_breaker.rs`: one reliability state machine.
- `telemetry.rs`: one DevTools telemetry actor.
