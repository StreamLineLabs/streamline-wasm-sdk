# Streamline WASM SDK

[![CI](https://github.com/streamlinelabs/streamline-wasm-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/streamlinelabs/streamline-wasm-sdk/actions/workflows/ci.yml)
[![codecov](https://img.shields.io/codecov/c/github/streamlinelabs/streamline-wasm-sdk?style=flat-square)](https://codecov.io/gh/streamlinelabs/streamline-wasm-sdk)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![WASM](https://img.shields.io/badge/WASM-purple.svg)](https://webassembly.org/)
[![npm](https://img.shields.io/npm/v/@streamlinelabs/streamline-wasm)](https://www.npmjs.com/package/@streamlinelabs/streamline-wasm)
[![Docs](https://img.shields.io/badge/docs-streamlinelabs.dev-blue.svg)](https://streamlinelabs.dev/docs/sdks/wasm)

Browser-native WebAssembly SDK for the [Streamline](https://github.com/streamlinelabs/streamline) streaming platform. Stream messages directly from the browser using WebSocket — no server-side proxy required.

## Features

- **Zero-dependency browser client** — compiled to WASM, runs natively in any modern browser
- **WebSocket transport** — connects to Streamline's HTTP API WebSocket endpoint
- **Auto-reconnection** — exponential backoff with configurable retry limits
- **Full streaming API** — produce, consume, subscribe, and manage topics
- **Admin client** — HTTP-based topic CRUD, consumer groups, and server health
- **Query client** — execute SQL queries against stream data from the browser
- **Schema Registry** — register, retrieve, and validate schemas (Avro, Protobuf, JSON)
- **TypeScript definitions** — generated `.d.ts` files for full IDE support
- **Structured error types** — errors include a `retryable` flag and resolution `hint` for programmatic error handling
- **Tiny footprint** — small WASM binary, fast initialization

## Requirements

- Rust 1.80 or later (for building from source)
- wasm-pack 0.12 or later
- Streamline server 0.2.0 or later (with WebSocket gateway enabled)

## Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `url` | `string` | — | WebSocket URL (e.g., `ws://localhost:9094/ws`) |
| `reconnect` | `boolean` | `true` | Auto-reconnect on disconnect |
| `reconnectInterval` | `number` | `1000` | Reconnect delay in milliseconds |
| `maxReconnectAttempts` | `number` | `10` | Maximum reconnection attempts |
| `compression` | `string` | `none` | Compression codec (`none`, `lz4`, `snappy`) |

## Quick Start

### Via npm

```bash
npm install @streamlinelabs/streamline-wasm
```

```javascript
import init, { StreamlineClient } from '@streamlinelabs/streamline-wasm';

async function main() {
  await init();

  const client = new StreamlineClient('ws://localhost:9094/ws');
  client.connect();

  // Produce a message
  client.produce('my-topic', 'Hello from browser!');

  // Subscribe to a topic
  client.subscribe('my-topic', (msg) => {
    console.log('Received:', msg);
  });
}

main();
```

### Via CDN (unpkg)

```html
<script type="module">
  import init, { StreamlineClient } from 'https://unpkg.com/@streamlinelabs/streamline-wasm/pkg/streamline_wasm_sdk.js';

  await init();
  const client = new StreamlineClient('ws://localhost:9094/ws');
  client.connect();
  client.produce('events', JSON.stringify({ action: 'click', ts: Date.now() }));
</script>
```

### Via CDN (jsDelivr)

```html
<script type="module">
  import init, { StreamlineClient } from 'https://cdn.jsdelivr.net/npm/@streamlinelabs/streamline-wasm/pkg/streamline_wasm_sdk.js';

  await init();
  const client = new StreamlineClient('ws://localhost:9094/ws');
  client.connect();
  client.produce('events', JSON.stringify({ action: 'click', ts: Date.now() }));
</script>
```

## API Reference

### `StreamlineClient`

High-level client wrapping a WebSocket connection.

```javascript
const client = new StreamlineClient('ws://localhost:9094/ws');
client.connect();

// Produce
client.produce('topic', 'value');
client.produce_with_key('topic', 'key', 'value');

// Subscribe / Unsubscribe
client.subscribe('topic', callback);
client.unsubscribe('topic');

// Topic administration
client.create_topic('new-topic', 3);  // 3 partitions
client.delete_topic('old-topic');
client.list_topics();

// Connection state
client.is_connected();
client.on_state_change((state) => console.log(state));
client.disconnect();
```

### `Producer`

Dedicated producer with optional default topic.

```javascript
const producer = new Producer('ws://localhost:9094/ws', 'my-topic');
producer.connect();
producer.send('hello');                        // uses default topic
producer.send('hello', 'other-topic');         // override topic
producer.send_keyed('key-1', 'hello', null);   // keyed message
producer.disconnect();
```

### `Consumer`

Dedicated consumer bound to a single topic.

```javascript
const consumer = new Consumer('ws://localhost:9094/ws', 'my-topic');
consumer.start((msg) => console.log('Got:', msg));
// ... later
consumer.stop();
```

### `TopicAdmin`

Administrative operations on topics.

```javascript
const admin = new TopicAdmin('ws://localhost:9094/ws');
admin.connect();
admin.on_response((resp) => console.log(JSON.parse(resp)));
admin.create_topic('new-topic', 6);
admin.list_topics();
admin.delete_topic('old-topic');
admin.disconnect();
```

## Telemetry

The SDK includes browser-native telemetry for timing produce and consume operations.
It uses `console.time`/`console.timeEnd` for console-visible timing and
`Performance.mark`/`Performance.measure` for DevTools Performance panel integration.

### Setup

```javascript
import { Telemetry, generate_traceparent } from '@streamlinelabs/streamline-wasm';

const telemetry = new Telemetry();
```

### Usage

```javascript
// Time a produce operation
const span = telemetry.start_produce('orders');
client.produce('orders', 'message data');
telemetry.end_span(span);

// Time a consume operation
const cspan = telemetry.start_consume('events');
// ... process messages ...
telemetry.end_span(cspan);

// Record an error
const pspan = telemetry.start_produce('orders');
try {
  client.produce('orders', data);
  telemetry.end_span(pspan);
} catch (e) {
  telemetry.end_span_with_error(pspan, e.message);
}

// Disable telemetry
telemetry.enabled = false;
```

### W3C Trace Context

For distributed tracing across browser and server, the SDK supports
W3C TraceContext header generation:

```javascript
import { generate_traceparent, parse_traceparent } from '@streamlinelabs/streamline-wasm';

// Generate a traceparent header for a produce operation
const traceparent = generate_traceparent();
// "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"

// Parse an incoming traceparent
const parsed = parse_traceparent(traceparent);
// { version: "00", trace_id: "...", span_id: "...", trace_flags: "01" }
```

### Span Conventions

Performance marks and measures follow the same naming as the server-side SDKs:

| Mark/Measure name | Format |
|-------------------|--------|
| Produce | `{topic} produce` (e.g., "orders produce") |
| Consume | `{topic} consume` (e.g., "events consume") |
| Process | `{topic} process` (e.g., "events process") |

All timing data is visible in the browser DevTools Performance tab.

## Error Handling

All SDK methods that communicate over WebSocket can throw errors. Wrap calls
in try/catch blocks for robust error handling:

```javascript
import init, { StreamlineClient } from '@streamlinelabs/streamline-wasm';

await init();
const client = new StreamlineClient('ws://localhost:9094/ws');

// Handle connection errors
try {
  client.connect();
} catch (err) {
  console.error('Connection failed:', err);
}

// Handle produce errors
try {
  client.produce('my-topic', 'Hello!');
} catch (err) {
  if (err.toString().includes('not connected')) {
    console.error('Cannot produce: WebSocket is disconnected');
  } else {
    console.error('Produce error:', err);
  }
}

// Monitor connection state for auto-recovery
client.on_state_change((state) => {
  switch (state) {
    case 'Connected':
      console.log('✓ Connected');
      break;
    case 'Disconnected':
      console.warn('⚠ Disconnected — attempting reconnect...');
      break;
    case 'Reconnecting':
      console.log('↻ Reconnecting...');
      break;
  }
});
```

## Browser Demo

An interactive browser demo is included in [`demo/`](demo/). It lets you connect to a Streamline server, manage topics, produce and consume messages — all from the browser.

```bash
# Build the WASM package
wasm-pack build --target web

# Serve locally
python3 -m http.server 8080
```

Open [http://localhost:8080/demo/](http://localhost:8080/demo/). If no server is available, the demo runs in **Demo Mode** with simulated messages.

See [`demo/README.md`](demo/README.md) for details.

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) 1.80+
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

```bash
# Add WASM target
rustup target add wasm32-unknown-unknown

# Build (both web and bundler targets)
npm run build

# Or build targets individually
npm run build:web       # ESM for browsers & CDN
npm run build:bundler   # For webpack/rollup/vite

# Run tests
cargo test                                # unit tests
wasm-pack test --headless --chrome        # browser tests
```

### npm Publishing

```bash
# Build and publish
npm run build
npm publish --access public
```

The `prepublishOnly` script runs the build automatically before publishing.

## Architecture

```
Browser  ──WebSocket──▶  Streamline Server (port 9094/ws)
  │                           │
  ├─ StreamlineClient         ├─ Produce → topic
  ├─ Producer                 ├─ Subscribe → push messages
  ├─ Consumer                 ├─ Admin → create/delete/list topics
  └─ TopicAdmin               └─ Ack/Error responses
```

The SDK communicates over a JSON-based protocol on top of WebSocket. Messages are framed as `BrowserMessage` (client → server) and `BrowserResponse` (server → client).

## Admin Client

The `AdminClient` provides HTTP-based topic and consumer group management:

```javascript
import init, { AdminClient } from '@streamlinelabs/streamline-wasm';

await init();
const admin = new AdminClient('http://localhost:9094');

// Topic management
const topics = await admin.list_topics();
await admin.create_topic('orders', 3);
const info = await admin.describe_topic('orders');
await admin.delete_topic('orders');

// Consumer groups
const groups = await admin.list_consumer_groups();
const detail = await admin.describe_consumer_group('my-group');

// Server health
const healthy = await admin.health();
const info = await admin.server_info();
```

## Query Client

Execute SQL queries against stream data directly from the browser:

```javascript
import init, { QueryClient } from '@streamlinelabs/streamline-wasm';

await init();
const query = new QueryClient('http://localhost:9094');

const result = await query.execute('SELECT * FROM orders LIMIT 10');
console.log('Columns:', result.columns);
console.log('Rows:', result.rows);
console.log('Count:', result.row_count);

// Raw JSON response
const raw = await query.execute_raw('SELECT count(*) FROM events');
```

## Schema Registry

Register, retrieve, and validate schemas from the browser:

```javascript
import init, { SchemaRegistryClient, SchemaFormat } from '@streamlinelabs/streamline-wasm';

await init();
const registry = new SchemaRegistryClient('http://localhost:9094');

// Register a JSON schema
const id = await registry.register_schema(
  'orders-value',
  '{"type":"object","required":["orderId","amount"]}',
  SchemaFormat.Json
);

// Retrieve latest schema
const schema = await registry.get_latest_schema('orders-value');

// Check compatibility before evolving
const compatible = await registry.check_compatibility(
  'orders-value', newSchema, SchemaFormat.Json
);

// List subjects
const subjects = await registry.list_subjects();

// Client-side JSON validation
const valid = registry.validate_json(schema.schema, '{"orderId":"123","amount":99.99}');
```

## Contributing

Contributions are welcome! Please see the [organization contributing guide](https://github.com/streamlinelabs/.github/blob/main/CONTRIBUTING.md) for guidelines.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

## Security

To report a security vulnerability, please email **security@streamline.dev**.
Do **not** open a public issue.

See the [Security Policy](https://github.com/streamlinelabs/streamline/blob/main/SECURITY.md) for details.

