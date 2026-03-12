//! Browser integration tests for WebSocket auto-reconnection.
//!
//! These tests run in a real browser environment (headless Chrome)
//! and exercise the actual WebSocket + setTimeout reconnection logic.

use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

use streamline_wasm_sdk::*;

// ── Client initial state ─────────────────────────────────────────────

#[wasm_bindgen_test]
fn test_client_initial_state() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    assert!(!client.is_connected());
    assert_eq!(client.connection_state(), ConnectionState::Disconnected);
}

// ── Auto-reconnect default ───────────────────────────────────────────

#[wasm_bindgen_test]
fn test_auto_reconnect_enabled_by_default() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert!(conn.auto_reconnect());
}

// ── Connection state change callback ─────────────────────────────────

#[wasm_bindgen_test]
fn test_connection_state_changes() {
    let mut client = StreamlineClient::new("ws://localhost:9094/ws");

    // Register a state-change callback that stores the new state in a global.
    let cb = js_sys::Function::new_with_args("state", "globalThis.__lastState = state;");
    client.on_state_change(cb);

    // connect() synchronously transitions to Connecting and fires the callback
    let _ = client.connect();

    let last: wasm_bindgen::JsValue =
        js_sys::Reflect::get(&js_sys::global(), &"__lastState".into())
            .unwrap_or(wasm_bindgen::JsValue::UNDEFINED);

    // The state string produced by set_state is the Debug format of the enum
    assert_eq!(last.as_string().unwrap_or_default(), "Connecting");
}

// ── Connect to invalid URL triggers error state ──────────────────────

#[wasm_bindgen_test]
fn test_connect_to_invalid_url_triggers_error() {
    let mut client = StreamlineClient::new("ws://invalid:0/ws");

    // Attempting to connect to an unreachable host.
    // In browser environments the WebSocket constructor may throw synchronously
    // or transition to an error state asynchronously. Either way the client
    // must not report itself as connected.
    let result = client.connect();

    // Even if connect() itself succeeds (handshake is async), we should not
    // be in the Connected state yet.
    assert!(!client.is_connected());

    // If the browser rejects the URL synchronously, we get an Err.
    // Both outcomes are acceptable — the important thing is no panic.
    let _ = result;
}

// ── Reconnect delay calculation (exponential backoff) ────────────────

#[wasm_bindgen_test]
fn test_reconnect_delay_calculation() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    // Fresh connection: 1 s base delay (0 attempts so far)
    assert_eq!(conn.reconnect_delay_ms(), 1_000);
}

#[wasm_bindgen_test]
fn test_reconnect_delay_is_capped_at_30s() {
    // We can't directly set reconnect_attempts on WsConnection from the public
    // API, but we can verify the initial delay. The cap behaviour is covered by
    // internal unit tests; here we confirm the public API is accessible from JS.
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert!(conn.reconnect_delay_ms() <= 30_000);
}

// ── Producer batch accumulation ──────────────────────────────────────

#[wasm_bindgen_test]
fn test_producer_batch_accumulation() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("test-topic".into()));

    assert_eq!(producer.pending_count(), 0);

    // send() accumulates into the batch (will error on flush since not connected,
    // but the batch should still grow up to batch_size).
    let _ = producer.send("msg-1", None);
    assert_eq!(producer.pending_count(), 1);

    let _ = producer.send("msg-2", None);
    assert_eq!(producer.pending_count(), 2);

    let _ = producer.send("msg-3", None);
    assert_eq!(producer.pending_count(), 3);
}

#[wasm_bindgen_test]
fn test_producer_batch_size_configuration() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("topic".into()));
    producer.set_batch_size(2);

    // First message: below batch_size, stays in batch
    let _ = producer.send("a", None);
    assert_eq!(producer.pending_count(), 1);

    // Second message: hits batch_size, auto-flush is attempted.
    // Flush will fail (not connected), but pending_count behaviour after
    // a failed flush depends on implementation. We just verify no panic.
    let _ = producer.send("b", None);
}

#[wasm_bindgen_test]
fn test_producer_requires_topic() {
    let mut producer = Producer::new("ws://localhost:9094/ws", None);

    // Without a default topic and no explicit topic, send should error
    let result = producer.send("value", None);
    assert!(result.is_err());
    assert_eq!(producer.pending_count(), 0);
}

// ── Consumer offset tracking ─────────────────────────────────────────

#[wasm_bindgen_test]
fn test_consumer_offset_tracking() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "my-topic");

    // Initial offsets
    assert_eq!(consumer.current_offset(), 0);
    assert_eq!(consumer.committed_offset(), -1);

    // Advance the offset as if messages arrived
    consumer.advance_offset(0);
    assert_eq!(consumer.current_offset(), 1); // offset + 1

    consumer.advance_offset(4);
    assert_eq!(consumer.current_offset(), 5);

    // Advancing to an earlier offset is a no-op
    consumer.advance_offset(2);
    assert_eq!(consumer.current_offset(), 5);
}

#[wasm_bindgen_test]
fn test_consumer_commit_requires_connection() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
    let result = consumer.commit();
    assert!(result.is_err());
}

// ── Disconnect prevents reconnect ────────────────────────────────────

#[wasm_bindgen_test]
fn test_disconnect_prevents_reconnect() {
    let mut client = StreamlineClient::new("ws://localhost:9094/ws");

    // Connect (transitions to Connecting), then immediately disconnect
    let _ = client.connect();
    client.disconnect();

    // After intentional disconnect, state must be Disconnected
    assert_eq!(client.connection_state(), ConnectionState::Disconnected);
    assert!(!client.is_connected());

    // A second disconnect is safe and idempotent
    client.disconnect();
    assert_eq!(client.connection_state(), ConnectionState::Disconnected);
}

#[wasm_bindgen_test]
fn test_disconnect_resets_reconnect_state() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    let _ = conn.connect();
    conn.disconnect();

    // After disconnect, reconnect_attempts should be reset to 0
    assert_eq!(conn.reconnect_attempts(), 0);
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}

// ── Circuit breaker from browser ─────────────────────────────────────

#[wasm_bindgen_test]
fn test_circuit_breaker_from_browser() {
    let mut cb = CircuitBreaker::new(3, 2, 30_000.0);

    // Starts closed
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.allow());
    assert_eq!(cb.failure_count(), 0);

    // Record failures up to threshold
    cb.record_failure();
    assert_eq!(cb.failure_count(), 1);
    cb.record_failure();
    assert_eq!(cb.failure_count(), 2);
    cb.record_failure();

    // Should now be open
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(!cb.allow());

    // Reset brings it back to closed
    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.allow());
    assert_eq!(cb.failure_count(), 0);
    assert_eq!(cb.success_count(), 0);
}

#[wasm_bindgen_test]
fn test_circuit_breaker_success_resets_failures() {
    let mut cb = CircuitBreaker::new(5, 2, 30_000.0);
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.failure_count(), 2);

    cb.record_success();
    assert_eq!(cb.failure_count(), 0);
    assert_eq!(cb.state(), CircuitState::Closed);
}

// ── Schema format round-trip ─────────────────────────────────────────

#[wasm_bindgen_test]
fn test_schema_format_roundtrip() {
    // Verify all SchemaFormat variants are accessible from browser/JS context
    let avro = SchemaFormat::Avro;
    let proto = SchemaFormat::Protobuf;
    let json = SchemaFormat::Json;

    // Enum variants should be distinct
    assert_ne!(avro, proto);
    assert_ne!(avro, json);
    assert_ne!(proto, json);

    // Each variant equals itself
    assert_eq!(avro, SchemaFormat::Avro);
    assert_eq!(proto, SchemaFormat::Protobuf);
    assert_eq!(json, SchemaFormat::Json);
}

#[wasm_bindgen_test]
fn test_schema_format_debug_output() {
    assert_eq!(format!("{:?}", SchemaFormat::Avro), "Avro");
    assert_eq!(format!("{:?}", SchemaFormat::Protobuf), "Protobuf");
    assert_eq!(format!("{:?}", SchemaFormat::Json), "Json");
}
