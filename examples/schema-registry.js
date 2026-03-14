// schema-registry.js — Schema Registry with Streamline WASM SDK
//
// Demonstrates using the Schema Registry from the browser.
// The WASM SDK includes a SchemaRegistryClient for schema validation.
//
// Usage:
//   1. Start Streamline: docker run -p 9092:9092 -p 9094:9094 ghcr.io/streamlinelabs/streamline:0.2.0
//   2. Include in a bundler or HTML page with the WASM SDK loaded.

async function main() {
  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  const httpUrl = 'http://localhost:9094';

  // --- Schema Registry Client ---
  const registry = new wasm.SchemaRegistryClient(httpUrl);
  console.log('Schema Registry Examples');
  console.log('='.repeat(40));

  // Define schemas
  const userSchema = JSON.stringify({
    type: 'object',
    properties: {
      id: { type: 'integer' },
      name: { type: 'string' },
      email: { type: 'string', format: 'email' },
    },
    required: ['id', 'name'],
  });

  // Register a schema via REST API
  console.log('\n--- Register Schema ---');
  try {
    const resp = await fetch(`${httpUrl}/v1/schemas/subjects/user-events-value/versions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        schema: userSchema,
        schemaType: 'JSON',
      }),
    });
    const result = await resp.json();
    console.log(`✅ Registered schema ID: ${result.id}`);
  } catch (err) {
    console.log(`Schema registration: ${err}`);
  }

  // Validate messages against the schema locally
  console.log('\n--- Local Validation ---');

  const validMessage = JSON.stringify({ id: 1, name: 'Alice', email: 'alice@example.com' });
  const invalidMessage = JSON.stringify({ name: 'Bob' }); // missing required 'id'

  try {
    const isValid = registry.validate_json(userSchema, validMessage);
    console.log(`Valid message:   ${isValid ? '✅ PASS' : '❌ FAIL'}`);
  } catch (err) {
    console.log(`Validation error: ${err}`);
  }

  try {
    const isValid = registry.validate_json(userSchema, invalidMessage);
    console.log(`Invalid message: ${isValid ? '❌ UNEXPECTED PASS' : '✅ Correctly rejected'}`);
  } catch (err) {
    console.log(`Invalid message: ✅ Correctly rejected (${err})`);
  }

  // Produce validated messages
  console.log('\n--- Produce Validated Messages ---');
  const client = new wasm.StreamlineClient('ws://localhost:9094/ws');
  await client.connect();

  try {
    await client.create_topic('user-events', 1);
  } catch (e) { /* topic may exist */ }

  const users = [
    { id: 1, name: 'Alice', email: 'alice@example.com' },
    { id: 2, name: 'Bob', email: 'bob@example.com' },
    { id: 3, name: 'Charlie' }, // valid: email is optional
  ];

  for (const user of users) {
    const msg = JSON.stringify(user);
    try {
      const valid = registry.validate_json(userSchema, msg);
      if (valid) {
        await client.produce('user-events', msg);
        console.log(`  ✅ Produced: ${msg}`);
      }
    } catch (err) {
      console.log(`  ❌ Validation failed for ${msg}: ${err}`);
    }
  }

  client.disconnect();
  console.log('\n✅ Done');
}

main().catch(console.error);
