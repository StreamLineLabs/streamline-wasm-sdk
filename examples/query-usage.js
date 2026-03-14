// query-usage.js — SQL Analytics with Streamline WASM SDK
//
// Demonstrates using Streamline's SQL query capabilities from the browser.
// Queries are executed via the HTTP REST API (not WebSocket).
//
// Usage:
//   1. Start Streamline: docker run -p 9092:9092 -p 9094:9094 ghcr.io/streamlinelabs/streamline:0.2.0
//   2. Include in a bundler or HTML page.

async function main() {
  const httpUrl = 'http://localhost:9094';

  // Initialize WASM SDK for producing sample data
  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  const client = new wasm.StreamlineClient('ws://localhost:9094/ws');
  await client.connect();

  // Create topic and produce sample data
  try {
    await client.create_topic('query-demo', 1);
  } catch (e) { /* topic may exist */ }

  console.log('Producing sample events...');
  for (let i = 0; i < 10; i++) {
    await client.produce('query-demo', JSON.stringify({
      user: `user-${i}`,
      action: i % 2 === 0 ? 'click' : 'scroll',
      value: i * 10,
      ts: new Date().toISOString(),
    }));
  }
  console.log('✅ Produced 10 events\n');

  // Execute SQL queries via the REST API
  async function query(sql) {
    const resp = await fetch(`${httpUrl}/v1/query`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ query: sql }),
    });
    if (!resp.ok) throw new Error(`Query failed: ${resp.status} ${resp.statusText}`);
    return resp.json();
  }

  // Query 1: Select all events
  console.log('--- All events (LIMIT 5) ---');
  const all = await query("SELECT * FROM topic('query-demo') LIMIT 5");
  console.table(all.rows || all);

  // Query 2: Count by action
  console.log('\n--- Count by action ---');
  const counts = await query("SELECT action, COUNT(*) as cnt FROM topic('query-demo') GROUP BY action");
  console.table(counts.rows || counts);

  // Query 3: Filter by value
  console.log('\n--- Events with value > 50 ---');
  const filtered = await query("SELECT user, value FROM topic('query-demo') WHERE value > 50");
  console.table(filtered.rows || filtered);

  client.disconnect();
  console.log('\n✅ Done');
}

main().catch(console.error);
