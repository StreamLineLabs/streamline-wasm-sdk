// circuit-breaker.js — Circuit Breaker Pattern with Streamline WASM SDK
//
// Demonstrates implementing a circuit breaker around the WASM SDK client
// to handle intermittent connectivity in browser environments.
//
// Usage:
//   1. Start a compatible fixture: docker run -p 9092:9092 -p 9094:9094 "$STREAMLINE_FIXTURE_IMAGE"
//   2. Include in a bundler or HTML page with the WASM SDK loaded.

// Simple circuit breaker implementation for browser environments
class CircuitBreaker {
  constructor({ failureThreshold = 5, openTimeout = 10000, successThreshold = 2 } = {}) {
    this.failureThreshold = failureThreshold;
    this.openTimeout = openTimeout;
    this.successThreshold = successThreshold;
    this.failures = 0;
    this.successes = 0;
    this.state = 'CLOSED'; // CLOSED | OPEN | HALF_OPEN
    this.openedAt = null;
  }

  allow() {
    if (this.state === 'CLOSED') return true;
    if (this.state === 'OPEN') {
      if (Date.now() - this.openedAt >= this.openTimeout) {
        this.state = 'HALF_OPEN';
        this.successes = 0;
        console.log('[Circuit Breaker] OPEN → HALF_OPEN');
        return true;
      }
      return false;
    }
    return true; // HALF_OPEN allows probes
  }

  recordSuccess() {
    this.failures = 0;
    if (this.state === 'HALF_OPEN') {
      this.successes++;
      if (this.successes >= this.successThreshold) {
        this.state = 'CLOSED';
        console.log('[Circuit Breaker] HALF_OPEN → CLOSED');
      }
    }
  }

  recordFailure() {
    this.failures++;
    if (this.failures >= this.failureThreshold && this.state === 'CLOSED') {
      this.state = 'OPEN';
      this.openedAt = Date.now();
      console.log('[Circuit Breaker] CLOSED → OPEN');
    }
  }

  reset() {
    this.state = 'CLOSED';
    this.failures = 0;
    this.successes = 0;
    this.openedAt = null;
  }
}

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
  const wasm = await import('@streamlinelabs/streamline-wasm');
  await wasm.default();

  const serverUrl = 'ws://localhost:9094/ws';
  const client = new wasm.StreamlineClient(serverUrl);

  const breaker = new CircuitBreaker({
    failureThreshold: 3,
    openTimeout: 5000,
    successThreshold: 2,
  });

  await connectAndWait(client);
  console.log(`✅ Connected. Circuit: ${breaker.state}`);

  // Create topic
  const admin = new wasm.AdminClient('http://localhost:9094');
  try {
    await admin.create_topic('cb-demo', 1);
  } catch (e) { /* topic may exist */ }

  // Send messages through the circuit breaker
  for (let i = 0; i < 15; i++) {
    if (!breaker.allow()) {
      console.log(`  Message ${i}: ❌ REJECTED (circuit ${breaker.state})`);
      await new Promise(r => setTimeout(r, 1000));
      continue;
    }

    try {
      await client.produce('cb-demo', JSON.stringify({ i, ts: Date.now() }));
      breaker.recordSuccess();
      console.log(`  Message ${i}: ✅ sent (circuit: ${breaker.state})`);
    } catch (err) {
      breaker.recordFailure();
      console.log(`  Message ${i}: ❌ failed (${err}) (circuit: ${breaker.state})`);
    }
  }

  console.log(`\nFinal circuit state: ${breaker.state}`);
  console.log(`Failures: ${breaker.failures}`);

  client.disconnect();
}

main().catch(console.error);
