// basic-usage.js — Streamline WASM SDK Basic Usage Example
//
// Demonstrates producing and consuming messages using the WASM SDK
// from a standard JavaScript environment (browser or bundler).
//
// Usage:
//   1. Start a compatible fixture: docker run -p 9092:9092 -p 9094:9094 "$STREAMLINE_FIXTURE_IMAGE"
//   2. Include this script in an HTML page with the WASM SDK loaded, or use a bundler.
//
// With a bundler (webpack, vite, etc.):
//   import init, { StreamlineClient } from '@streamlinelabs/streamline-wasm';
//
// From CDN (in HTML <script type="module">):
//   import init, { StreamlineClient } from 'https://cdn.jsdelivr.net/npm/@streamlinelabs/streamline-wasm@0.2.0/pkg/streamline_wasm_sdk.js';

function connectAndWait(client) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('Connection timed out')), 10000);
    client.on_state_change((state) => {
      if (state === 'Connected') {
        clearTimeout(timeout);
        resolve();
      }
    });
    client.on_reconnect_failed((message) => {
      clearTimeout(timeout);
      reject(new Error(message));
    });
    client.connect();
  });
}

async function main() {
  // Initialize the WASM module (required once before any API calls)
  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  const serverUrl = 'ws://localhost:9094/ws';
  console.log(`Connecting to ${serverUrl}...`);

  // Create a topic (idempotent — safe to call if it already exists)
  const admin = new wasm.AdminClient('http://localhost:9094');
  try {
    await admin.create_topic('demo-events', 3); // 3 partitions
    console.log('✅ Topic "demo-events" created (or already exists)');
  } catch (err) {
    console.log(`Topic creation: ${err}`);
  }

  const client = new wasm.StreamlineClient(serverUrl);
  await connectAndWait(client);
  console.log('✅ Connected');

  // --- Producing Messages ---
  // Produce 5 messages
  for (let i = 0; i < 5; i++) {
    const message = JSON.stringify({
      event: 'page_view',
      user_id: `user-${i}`,
      timestamp: new Date().toISOString(),
    });
    client.produce_with_key('demo-events', `key-${i}`, message);
    console.log(`📤 Produced message ${i + 1}`);
  }

  // --- Consuming Messages ---
  let received = 0;
  client.subscribe('demo-events', (messageJson) => {
    received++;
    const message = JSON.parse(messageJson);
    console.log(`📥 Received [${message.key ?? 'no-key'}]: ${message.value}`);
  });

  // Wait briefly for messages to arrive
  await new Promise((resolve) => setTimeout(resolve, 2000));
  console.log(`\n✅ Done — produced 5 messages, received ${received}`);

  // Cleanup
  client.disconnect();
}

main().catch(console.error);
