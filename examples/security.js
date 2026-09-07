// security.js — Authentication with Streamline WASM SDK
//
// Demonstrates supported HTTP bearer authentication and secure transport.
// The current WebSocket client cannot set browser handshake headers.
//
// Usage:
//   1. Start Streamline with auth: streamline --auth-enabled --auth-users-file users.yaml
//   2. Include in a bundler or HTML page with the WASM SDK loaded.

async function webSocketAuthLimitationExample() {
  console.log('WebSocket Authentication');
  console.log('-'.repeat(40));

  console.log('  The current browser WebSocket API does not expose custom');
  console.log('  Authorization headers, and this SDK does not place bearer');
  console.log('  tokens in URLs because URLs can leak through logs/history.');
  console.log('  Use a same-origin authenticated gateway or another server-');
  console.log('  supported browser credential mechanism before connecting.\n');
}

async function schemaRegistryAuthExample(authToken) {
  console.log('Schema Registry with Auth Token');
  console.log('-'.repeat(40));

  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  const registry = new wasm.SchemaRegistryClient('http://localhost:9094');

  if (!authToken) {
    console.log('  Skipped: pass a token from secure application state.\n');
    return;
  }

  // HTTP clients support an Authorization: Bearer header.
  registry.set_auth_token(authToken);

  console.log('  ✅ Schema registry client configured with auth token');
  console.log('  (Use registry.validate_json() / register operations with auth)\n');
}

async function secureWebSocketExample() {
  console.log('Secure WebSocket (WSS) Connection');
  console.log('-'.repeat(40));

  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  // For production, use wss:// (TLS is handled by the browser)
  const secureUrl = 'wss://streamline.example.com:9094/ws';
  console.log(`  Connecting to ${secureUrl}...`);
  console.log('  (Browser handles TLS certificate validation automatically)');
  console.log('  (No manual CA/cert configuration needed in browser environments)\n');

  // In a real app:
  // const client = new wasm.StreamlineClient(secureUrl);
  // client.on_state_change((state) => console.log(state));
  // client.connect(); // wait for Connected before sending
}

async function main() {
  console.log('Streamline WASM SDK Security Examples');
  console.log('='.repeat(40));
  console.log();

  const authToken = typeof process !== 'undefined'
    ? process.env.STREAMLINE_AUTH_TOKEN
    : undefined;

  await webSocketAuthLimitationExample();
  await schemaRegistryAuthExample(authToken);
  await secureWebSocketExample();

  console.log('Done!');
}

main().catch(console.error);
