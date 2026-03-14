// security.js — Authentication with Streamline WASM SDK
//
// Demonstrates configuring authentication tokens for the WASM SDK.
// The browser WASM SDK uses WebSocket connections with token-based auth
// (TLS is handled at the transport layer by the browser).
//
// Usage:
//   1. Start Streamline with auth: streamline --auth-enabled --auth-users-file users.yaml
//   2. Include in a bundler or HTML page with the WASM SDK loaded.

async function tokenAuthExample() {
  console.log('Token Authentication');
  console.log('-'.repeat(40));

  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  // The WASM SDK uses the WebSocket URL, and authentication is passed
  // via the connection URL or headers depending on server configuration.
  const authToken = typeof process !== 'undefined'
    ? (process.env.STREAMLINE_AUTH_TOKEN || 'demo-token')
    : 'demo-token';

  // Connect with auth token in the URL
  const serverUrl = `ws://localhost:9094/ws?token=${encodeURIComponent(authToken)}`;
  const client = new wasm.StreamlineClient(serverUrl);

  try {
    await client.connect();
    console.log('  ✅ Connected with token authentication');

    // Produce a message to verify auth works
    await client.produce('secure-events', JSON.stringify({
      action: 'login',
      user: 'alice',
      authenticated: true,
    }));
    console.log('  ✅ Produced authenticated message');

  } catch (err) {
    console.error(`  ❌ Auth failed: ${err}`);
  } finally {
    client.disconnect();
    console.log('  Disconnected.\n');
  }
}

async function schemaRegistryAuthExample() {
  console.log('Schema Registry with Auth Token');
  console.log('-'.repeat(40));

  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  const registry = new wasm.SchemaRegistryClient('http://localhost:9094');

  // Set auth token for registry API calls
  const authToken = typeof process !== 'undefined'
    ? (process.env.STREAMLINE_AUTH_TOKEN || 'demo-token')
    : 'demo-token';
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
  // await client.connect();
}

async function main() {
  console.log('Streamline WASM SDK Security Examples');
  console.log('='.repeat(40));
  console.log();

  await tokenAuthExample();
  await schemaRegistryAuthExample();
  await secureWebSocketExample();

  console.log('Done!');
}

main().catch(console.error);
