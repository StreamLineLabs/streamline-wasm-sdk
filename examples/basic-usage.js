// basic-usage.js — Streamline WASM SDK Basic Usage Example
//
// Demonstrates producing and consuming messages using the WASM SDK
// from a standard JavaScript environment (browser or bundler).
//
// Usage:
//   1. Start Streamline: docker run -p 9092:9092 -p 9094:9094 ghcr.io/streamlinelabs/streamline:0.2.0
//   2. Include this script in an HTML page with the WASM SDK loaded, or use a bundler.
//
// With a bundler (webpack, vite, etc.):
//   import init, { StreamlineClient, TopicAdmin } from '@streamlinelabs/streamline-wasm';
//
// From CDN (in HTML <script type="module">):
//   import init, { StreamlineClient, TopicAdmin } from 'https://cdn.jsdelivr.net/npm/@streamlinelabs/streamline-wasm@0.2.0/pkg/streamline_wasm_sdk.js';

async function main() {
  // Initialize the WASM module (required once before any API calls)
  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  const serverUrl = 'ws://localhost:9094/ws';
  console.log(`Connecting to ${serverUrl}...`);

  // --- Topic Administration ---
  const admin = new wasm.TopicAdmin(serverUrl);

  // Create a topic (idempotent — safe to call if it already exists)
  try {
    await admin.create_topic('demo-events', 3); // 3 partitions
    console.log('✅ Topic "demo-events" created (or already exists)');
  } catch (err) {
    console.log(`Topic creation: ${err}`);
  }

  // List all topics
  const topics = await admin.list_topics();
  console.log(`📋 Topics: ${JSON.stringify(topics)}`);

  // --- Producing Messages ---
  const client = new wasm.StreamlineClient(serverUrl);
  await client.connect();
  console.log('✅ Connected');

  // Produce 5 messages
  for (let i = 0; i < 5; i++) {
    const message = JSON.stringify({
      event: 'page_view',
      user_id: `user-${i}`,
      timestamp: new Date().toISOString(),
    });
    await client.produce('demo-events', `key-${i}`, message);
    console.log(`📤 Produced message ${i + 1}`);
  }

  // --- Consuming Messages ---
  let received = 0;
  client.subscribe('demo-events', (message) => {
    received++;
    console.log(`📥 Received [${message.key}]: ${message.value}`);
  });

  // Wait briefly for messages to arrive
  await new Promise((resolve) => setTimeout(resolve, 2000));
  console.log(`\n✅ Done — produced 5 messages, received ${received}`);

  // Cleanup
  client.disconnect();
}

main().catch(console.error);
