# CLAUDE.md — Streamline WASM SDK

## Overview
WebAssembly SDK for [Streamline](https://github.com/streamlinelabs/streamline), enabling browser-based streaming via WebSocket. Built with `wasm-bindgen`.

## Build & Test
```bash
make build                # Build WASM package (web target)
make test                 # Run tests
make lint                 # Clippy
make fmt                  # Format
make clean                # Clean artifacts
```

## Architecture
```
src/
├── lib.rs                # StreamlineClient, Producer, Consumer, TopicAdmin exports
├── protocol.rs           # Kafka wire protocol over WebSocket
├── websocket.rs          # WebSocket transport layer
├── telemetry.rs          # Performance timing with console/DevTools
```

## Coding Conventions
- **wasm-bindgen**: All public types use `#[wasm_bindgen]` attribute
- **No `.unwrap()` in production**: `#[warn(clippy::unwrap_used)]` enforced
- **Callbacks**: Use `js_sys::Function` or `Closure` for JS interop
- **Async**: Use `wasm-bindgen-futures` for async operations
- **Error handling**: Return `Result<T, JsValue>` for JS-facing functions

## Public API
```typescript
import init, { StreamlineClient } from '@streamlinelabs/streamline-wasm';

await init();
const client = new StreamlineClient('ws://localhost:9092');
await client.connect();
await client.produce('topic', JSON.stringify({ key: 'value' }));
client.subscribe('topic', (msg) => console.log(msg));
```

## Key Types
- `StreamlineClient` — Full-featured client (produce, subscribe, admin)
- `Producer` — Simplified producer with optional default topic
- `Consumer` — Topic-specific consumer with `start(callback)` / `stop()`
- `TopicAdmin` — Create, delete, list topics
- `Telemetry` — Performance timing

## Build Targets
- `web` — Browser target via `wasm-pack build --target web`
- `bundler` — Webpack/Vite target via `wasm-pack build --target bundler`

## npm Package
Published as `@streamlinelabs/streamline-wasm` with TypeScript definitions in `streamline-wasm.d.ts`.
