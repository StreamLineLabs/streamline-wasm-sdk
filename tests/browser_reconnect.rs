//! Browser integration tests for WebSocket auto-reconnection.
//!
//! These tests run in a real browser environment (headless Chrome)
//! and exercise the actual WebSocket + setTimeout reconnection logic.

use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use streamline_wasm_sdk::*;

fn install_mock_websocket() -> bool {
    js_sys::Function::new_no_args(
        r#"
        const originalWebSocket = globalThis.WebSocket;
        const originalSetTimeout = globalThis.setTimeout;
        const state = {
            originalWebSocket,
            originalSetTimeout,
            instances: [],
            messages: [],
            states: [],
        };

        class MockWebSocket {
            static CONNECTING = 0;
            static OPEN = 1;
            static CLOSING = 2;
            static CLOSED = 3;

            constructor(url) {
                this.url = url;
                this.readyState = MockWebSocket.CONNECTING;
                this.binaryType = "blob";
                this.sent = [];
                this.id = state.instances.length + 1;
                state.instances.push(this);
                queueMicrotask(() => {
                    if (this.readyState === MockWebSocket.CONNECTING) {
                        this.readyState = MockWebSocket.OPEN;
                        this.onopen?.(new Event("open"));
                    }
                });
            }

            send(message) {
                if (this.readyState !== MockWebSocket.OPEN) {
                    throw new DOMException("Socket is not open", "InvalidStateError");
                }
                this.sent.push(message);
                state.messages.push({ id: this.id, message });
            }

            close(code = 1000, reason = "") {
                if (this.readyState === MockWebSocket.CLOSED) {
                    return;
                }
                this.readyState = MockWebSocket.CLOSED;
                queueMicrotask(() => {
                    this.onclose?.(new CloseEvent("close", { code, reason, wasClean: code === 1000 }));
                });
            }

            fail() {
                this.close(1006, "mock failure");
            }

            emitMessage(message) {
                this.onmessage?.(new MessageEvent("message", { data: message }));
            }
        }

        globalThis.__streamlineMock = state;
        globalThis.WebSocket = MockWebSocket;
        globalThis.setTimeout = (callback, _delay, ...args) =>
            originalSetTimeout(callback, 0, ...args);
        return true;
        "#,
    )
    .call0(&wasm_bindgen::JsValue::NULL)
    .ok()
    .and_then(|value| value.as_bool())
    .unwrap_or(false)
}

fn restore_mock_websocket() {
    let _ = js_sys::Function::new_no_args(
        r#"
        const state = globalThis.__streamlineMock;
        if (state) {
            globalThis.WebSocket = state.originalWebSocket;
            globalThis.setTimeout = state.originalSetTimeout;
            delete globalThis.__streamlineMock;
        }
        "#,
    )
    .call0(&wasm_bindgen::JsValue::NULL);
}

fn run_mock_script(script: &str) -> Option<wasm_bindgen::JsValue> {
    js_sys::Function::new_no_args(script)
        .call0(&wasm_bindgen::JsValue::NULL)
        .ok()
}

/// `StreamlineError` is exported *to* JS (not imported), so it does not
/// implement `JsCast`/`dyn_into`. Its `code`/`message` getters compile to
/// real JS accessor properties, so `Reflect::get` invokes them directly.
fn error_code_is_unsupported(err: &wasm_bindgen::JsValue) -> bool {
    let code_is_unsupported_ordinal = js_sys::Reflect::get(err, &"code".into())
        .ok()
        .and_then(|value| value.as_f64())
        .map(|code| code as u32 == ErrorCode::Unsupported as u32)
        .unwrap_or(false);
    let message_names_the_reason = js_sys::Reflect::get(err, &"message".into())
        .ok()
        .and_then(|value| value.as_string())
        .map(|message| message.contains("acknowledgement protocol"))
        .unwrap_or(false);
    code_is_unsupported_ordinal && message_names_the_reason
}

async fn drain_mock_tasks() {
    for _ in 0..4 {
        let promise = js_sys::Function::new_no_args(
            "return new Promise((resolve) => setTimeout(resolve, 0));",
        )
        .call0(&wasm_bindgen::JsValue::NULL)
        .ok()
        .and_then(|value| value.dyn_into::<js_sys::Promise>().ok());
        if let Some(promise) = promise {
            let _ = JsFuture::from(promise).await;
        }
    }
}

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
    assert!(consumer.advance_offset(0).is_ok());
    assert_eq!(consumer.current_offset(), 1); // offset + 1

    assert!(consumer.advance_offset(4).is_ok());
    assert_eq!(consumer.current_offset(), 5);

    // Advancing to an earlier offset is a no-op
    assert!(consumer.advance_offset(2).is_ok());
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

// ── Shared browser lifecycle state ──────────────────────────────────

#[wasm_bindgen_test(async)]
async fn test_shared_state_callbacks_reconnect_and_consumer_start() {
    let mock_installed = install_mock_websocket();

    let mut client = StreamlineClient::new("ws://mock.test/ws");
    let connect_ok = client.connect().is_ok();

    let state_callback =
        js_sys::Function::new_with_args("state", "globalThis.__streamlineMock.states.push(state);");
    client.on_state_change(state_callback);
    let connecting_observed =
        run_mock_script("return globalThis.__streamlineMock.states.at(-1) === 'Connecting';")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

    drain_mock_tasks().await;
    let initial_ready = client.is_connected()
        && client.connection_state() == ConnectionState::Connected
        && run_mock_script("return globalThis.__streamlineMock.states.at(-1) === 'Connected';")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

    let message_callback = js_sys::Function::new_with_args(
        "message",
        "globalThis.__streamlineMock.lastMessage = message;",
    );
    let subscribe_ok = client.subscribe("events", message_callback).is_ok();
    let _ = run_mock_script(
        r#"
        globalThis.__streamlineMock.instances[0].emitMessage(JSON.stringify({
            type: "message", topic: "events", value: "first-message", offset: 0, timestamp: 0,
        }));
        "#,
    );
    let late_callback_honored = run_mock_script(
        "return JSON.parse(globalThis.__streamlineMock.lastMessage).value === 'first-message';",
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);

    let _ = run_mock_script("globalThis.__streamlineMock.instances[0].fail();");
    drain_mock_tasks().await;
    let first_reconnect_ready = client.is_connected()
        && run_mock_script(
            r#"
            const instances = globalThis.__streamlineMock.instances;
            return instances.length === 2 &&
                instances[1].sent.some((message) =>
                    message.includes('"type":"subscribe"') &&
                    message.includes('"topic":"events"')
                );
            "#,
        )
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let retained_reconnect_socket = client.produce("events", "after-reconnect").is_ok()
        && run_mock_script("return globalThis.__streamlineMock.messages.at(-1)?.id === 2;")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

    let _ = run_mock_script(
        r#"
        globalThis.__streamlineMock.instances[1].emitMessage(JSON.stringify({
            type: "message", topic: "events", value: "reconnected-message", offset: 1, timestamp: 0,
        }));
        "#,
    );
    let reconnect_message_handler = run_mock_script(
        "return JSON.parse(globalThis.__streamlineMock.lastMessage).value === 'reconnected-message';",
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);

    let _ = run_mock_script("globalThis.__streamlineMock.instances[1].fail();");
    drain_mock_tasks().await;
    let reconnect_close_handler = client.is_connected()
        && run_mock_script(
            r#"
            const instances = globalThis.__streamlineMock.instances;
            return instances.length === 3 &&
                instances[2].sent.some((message) =>
                    message.includes('"type":"subscribe"') &&
                    message.includes('"topic":"events"')
                );
            "#,
        )
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let mut consumer = Consumer::new("ws://mock.test/ws", "consumer-topic");
    let consumer_callback = js_sys::Function::new_with_args(
        "message",
        "globalThis.__streamlineMock.consumerMessage = message;",
    );
    let consumer_start_ok = consumer.start(consumer_callback).is_ok();
    drain_mock_tasks().await;
    let consumer_subscribed_after_open = run_mock_script(
        r#"
        const instances = globalThis.__streamlineMock.instances;
        const consumerSocket = instances[instances.length - 1];
        return consumerSocket.sent.some((message) =>
            message.includes('"type":"subscribe"') &&
            message.includes('"topic":"consumer-topic"')
        );
        "#,
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);

    consumer.stop();
    client.disconnect();
    restore_mock_websocket();

    assert!(mock_installed);
    assert!(connect_ok);
    assert!(connecting_observed);
    assert!(initial_ready);
    assert!(subscribe_ok);
    assert!(late_callback_honored);
    assert!(first_reconnect_ready);
    assert!(retained_reconnect_socket);
    assert!(reconnect_message_handler);
    assert!(reconnect_close_handler);
    assert!(consumer_start_ok);
    assert!(consumer_subscribed_after_open);
}

// ── Per-topic subscription callback demultiplexing ───────────────────

#[wasm_bindgen_test(async)]
async fn test_subscribe_demultiplexes_callbacks_per_topic() {
    let mock_installed = install_mock_websocket();

    let mut client = StreamlineClient::new("ws://mock.test/ws");
    let _ = client.connect();
    drain_mock_tasks().await;

    let cb_a = js_sys::Function::new_with_args(
        "message",
        "globalThis.__streamlineMock.topicACount = (globalThis.__streamlineMock.topicACount || 0) + 1;\
         globalThis.__streamlineMock.lastA = message;",
    );
    let cb_b = js_sys::Function::new_with_args(
        "message",
        "globalThis.__streamlineMock.topicBCount = (globalThis.__streamlineMock.topicBCount || 0) + 1;\
         globalThis.__streamlineMock.lastB = message;",
    );

    // Subscribing to a second, different topic must not overwrite the
    // first topic's callback (single shared WsConnection under the hood).
    let subscribe_a_ok = client.subscribe("topic-a", cb_a).is_ok();
    let subscribe_b_ok = client.subscribe("topic-b", cb_b).is_ok();

    let _ = run_mock_script(
        r#"
        const socket = globalThis.__streamlineMock.instances[0];
        socket.emitMessage(JSON.stringify({ type: "message", topic: "topic-a", value: "a1", offset: 0, timestamp: 0 }));
        socket.emitMessage(JSON.stringify({ type: "message", topic: "topic-b", value: "b1", offset: 0, timestamp: 0 }));
        socket.emitMessage(JSON.stringify({ type: "message", topic: "topic-a", value: "a2", offset: 1, timestamp: 0 }));
        "#,
    );

    let topic_a_received_only_its_own_messages = run_mock_script(
        "return globalThis.__streamlineMock.topicACount === 2 && \
         JSON.parse(globalThis.__streamlineMock.lastA).value === 'a2';",
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);
    let topic_b_received_only_its_own_messages = run_mock_script(
        "return globalThis.__streamlineMock.topicBCount === 1 && \
         JSON.parse(globalThis.__streamlineMock.lastB).value === 'b1';",
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);

    client.disconnect();
    restore_mock_websocket();

    assert!(mock_installed);
    assert!(subscribe_a_ok);
    assert!(subscribe_b_ok);
    assert!(topic_a_received_only_its_own_messages);
    assert!(topic_b_received_only_its_own_messages);
}

#[wasm_bindgen_test(async)]
async fn test_unsubscribe_removes_only_its_own_topic_callback() {
    let mock_installed = install_mock_websocket();

    let mut client = StreamlineClient::new("ws://mock.test/ws");
    let _ = client.connect();
    drain_mock_tasks().await;

    let cb_a =
        js_sys::Function::new_with_args("message", "globalThis.__streamlineMock.lastA = message;");
    let cb_b =
        js_sys::Function::new_with_args("message", "globalThis.__streamlineMock.lastB = message;");
    let _ = client.subscribe("topic-a", cb_a);
    let _ = client.subscribe("topic-b", cb_b);

    let unsubscribe_ok = client.unsubscribe("topic-a").is_ok();
    let _ = run_mock_script("globalThis.__streamlineMock.lastA = null;");

    let _ = run_mock_script(
        r#"
        const socket = globalThis.__streamlineMock.instances[0];
        socket.emitMessage(JSON.stringify({ type: "message", topic: "topic-a", value: "after-unsub", offset: 0, timestamp: 0 }));
        socket.emitMessage(JSON.stringify({ type: "message", topic: "topic-b", value: "still-active", offset: 0, timestamp: 0 }));
        "#,
    );

    let topic_a_no_longer_dispatched =
        run_mock_script("return globalThis.__streamlineMock.lastA === null;")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let topic_b_unaffected_by_unsubscribe = run_mock_script(
        "return JSON.parse(globalThis.__streamlineMock.lastB).value === 'still-active';",
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);

    client.disconnect();
    restore_mock_websocket();

    assert!(mock_installed);
    assert!(unsubscribe_ok);
    assert!(topic_a_no_longer_dispatched);
    assert!(topic_b_unaffected_by_unsubscribe);
}

#[wasm_bindgen_test(async)]
async fn test_reconnect_replay_preserves_per_topic_callback_ownership() {
    let mock_installed = install_mock_websocket();

    let mut client = StreamlineClient::new("ws://mock.test/ws");
    let _ = client.connect();
    drain_mock_tasks().await;

    let cb_a =
        js_sys::Function::new_with_args("message", "globalThis.__streamlineMock.lastA = message;");
    let cb_b =
        js_sys::Function::new_with_args("message", "globalThis.__streamlineMock.lastB = message;");
    let _ = client.subscribe("topic-a", cb_a);
    let _ = client.subscribe("topic-b", cb_b);

    // Force a reconnect; both subscriptions must be replayed on the new socket.
    let _ = run_mock_script("globalThis.__streamlineMock.instances[0].fail();");
    drain_mock_tasks().await;

    let both_resubscribed_on_new_socket = run_mock_script(
        r#"
        const socket = globalThis.__streamlineMock.instances[1];
        return socket.sent.some((m) => m.includes('"topic":"topic-a"')) &&
               socket.sent.some((m) => m.includes('"topic":"topic-b"'));
        "#,
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);

    let _ = run_mock_script(
        r#"
        const socket = globalThis.__streamlineMock.instances[1];
        socket.emitMessage(JSON.stringify({ type: "message", topic: "topic-a", value: "a-after-reconnect", offset: 0, timestamp: 0 }));
        socket.emitMessage(JSON.stringify({ type: "message", topic: "topic-b", value: "b-after-reconnect", offset: 0, timestamp: 0 }));
        "#,
    );

    let ownership_preserved_after_reconnect = run_mock_script(
        r#"
        return JSON.parse(globalThis.__streamlineMock.lastA).value === 'a-after-reconnect' &&
               JSON.parse(globalThis.__streamlineMock.lastB).value === 'b-after-reconnect';
        "#,
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);

    client.disconnect();
    restore_mock_websocket();

    assert!(mock_installed);
    assert!(both_resubscribed_on_new_socket);
    assert!(ownership_preserved_after_reconnect);
}

// ── Consumer: offset advancement wiring + fail-closed commit ─────────

#[wasm_bindgen_test(async)]
async fn test_consumer_start_wires_delivered_messages_to_offset_advancement() {
    let mock_installed = install_mock_websocket();

    let mut consumer = Consumer::new("ws://mock.test/ws", "auto-topic");
    assert_eq!(consumer.current_offset(), 0);

    let cb = js_sys::Function::new_with_args(
        "message",
        "globalThis.__streamlineMock.consumerSeen = (globalThis.__streamlineMock.consumerSeen || 0) + 1;",
    );
    let start_ok = consumer.start(cb).is_ok();
    drain_mock_tasks().await;

    let _ = run_mock_script(
        r#"
        const instances = globalThis.__streamlineMock.instances;
        const socket = instances[instances.length - 1];
        socket.emitMessage(JSON.stringify({ type: "message", topic: "auto-topic", value: "m1", offset: 4, timestamp: 0 }));
        "#,
    );
    // Offset advancement happens synchronously as part of message dispatch —
    // no manual `advance_offset()` call is required by the caller.
    let offset_after_first = consumer.current_offset();

    let _ = run_mock_script(
        r#"
        const instances = globalThis.__streamlineMock.instances;
        const socket = instances[instances.length - 1];
        socket.emitMessage(JSON.stringify({ type: "message", topic: "auto-topic", value: "m2", offset: 9, timestamp: 0 }));
        "#,
    );
    let offset_after_second = consumer.current_offset();

    let user_callback_still_invoked_for_each_message =
        run_mock_script("return globalThis.__streamlineMock.consumerSeen === 2;")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

    consumer.stop();
    restore_mock_websocket();

    assert!(mock_installed);
    assert!(start_ok);
    assert_eq!(offset_after_first, 5); // last delivered offset (4) + 1
    assert_eq!(offset_after_second, 10); // last delivered offset (9) + 1
    assert!(user_callback_still_invoked_for_each_message);
    // No ack protocol exists, so committed_offset must never be claimed.
    assert_eq!(consumer.committed_offset(), -1);
}

#[wasm_bindgen_test(async)]
async fn test_consumer_commit_is_unsupported_fail_closed_even_when_connected() {
    let mock_installed = install_mock_websocket();

    let mut consumer = Consumer::new("ws://mock.test/ws", "commit-topic");
    let cb = js_sys::Function::new_no_args("");
    let _ = consumer.start(cb);
    drain_mock_tasks().await;
    let is_connected = consumer.is_connected();

    let commit_result = consumer.commit();
    let commit_offset_result = consumer.commit_offset(5);

    let commit_is_unsupported = commit_result
        .err()
        .map(|err| error_code_is_unsupported(&err))
        .unwrap_or(false);
    let commit_offset_is_unsupported = commit_offset_result
        .err()
        .map(|err| error_code_is_unsupported(&err))
        .unwrap_or(false);

    let committed_offset_never_claimed = consumer.committed_offset() == -1;

    consumer.stop();
    restore_mock_websocket();

    assert!(mock_installed);
    // The fail-closed behaviour must hold even though the consumer is
    // actually connected — lack of an ack protocol, not connectivity, is
    // the reason these are unsupported.
    assert!(is_connected);
    assert!(commit_is_unsupported);
    assert!(commit_offset_is_unsupported);
    assert!(committed_offset_never_claimed);
}

#[wasm_bindgen_test]
fn test_consumer_auto_commit_configuration_fails_closed() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "auto-commit-topic");
    let configured = consumer.set_auto_commit(2);
    let is_unsupported = configured
        .err()
        .map(|err| error_code_is_unsupported(&err))
        .unwrap_or(false);

    assert!(is_unsupported);
    assert!(consumer.advance_offset(0).is_ok());
    assert!(consumer.advance_offset(1).is_ok());
    assert_eq!(consumer.current_offset(), 2);
    assert_eq!(consumer.committed_offset(), -1);
}

// ── Producer.flush(): no drain-and-drop on transport failure ─────────
//
// Regression coverage for: flush() must not silently drain the batch and
// claim success when the underlying send fails. A failed/unattempted
// record must stay queued, in original order, and the transport error must
// be surfaced — with no duplication of records that already made it out.

#[wasm_bindgen_test(async)]
async fn test_producer_flush_preserves_failed_and_unattempted_records_in_order() {
    let mock_installed = install_mock_websocket();

    let mut producer = Producer::new("ws://mock.test/ws", Some("orders".into()));
    producer.set_batch_size(1); // auto-flush on every send() for a tight, deterministic repro
    let _ = run_mock_script("globalThis.__streamlineMock.deliveryMetrics = [];");
    producer.on_delivery(js_sys::Function::new_with_args(
        "sent, failed",
        "globalThis.__streamlineMock.deliveryMetrics.push([sent, failed]);",
    ));
    let connect_ok = producer.connect().is_ok();
    drain_mock_tasks().await;

    // 1) Healthy send while the socket is OPEN: delivered immediately.
    let first_send_ok = producer.send("first", None).is_ok();
    let sent_after_first = producer.total_sent();
    let pending_after_first = producer.pending_count();

    // 2) Simulate a lost/broken transport *without* going through the normal
    //    close handshake (no onclose fires, no reconnect gets scheduled) —
    //    this isolates flush()'s own bookkeeping from reconnection logic.
    let _ = run_mock_script("globalThis.__streamlineMock.instances[0].readyState = 3;"); // CLOSED

    // 3) send() now auto-flushes immediately and must fail *and* keep the
    //    record queued rather than dropping it.
    let second_send_result = producer.send("second", None);
    let second_send_is_err = second_send_result.is_err();
    let pending_after_second = producer.pending_count();
    let errors_after_second = producer.total_errors();

    // 4) A third send is appended behind the still-queued "second" record.
    //    Flushing again must attempt "second" first (original order) and
    //    must not have duplicated or dropped anything so far.
    let third_send_result = producer.send("third", None);
    let third_send_is_err = third_send_result.is_err();
    let pending_after_third = producer.pending_count();
    let errors_after_third = producer.total_errors();
    let sent_unchanged_while_broken = producer.total_sent() == sent_after_first;

    // 5) Restore the transport and flush the retained backlog. The mock
    //    records every message actually handed to `ws.send()`, so we can
    //    prove no duplication (no repeat of "first") and correct ordering
    //    ("second" before "third") once delivery resumes.
    let _ = run_mock_script("globalThis.__streamlineMock.instances[0].readyState = 1;"); // OPEN
    let recovery_flush_ok = producer.flush().is_ok();
    let pending_after_recovery = producer.pending_count();
    let sent_after_recovery = producer.total_sent();

    let delivered_messages_in_order = run_mock_script(
        r#"
        const sent = globalThis.__streamlineMock.instances[0].sent;
        const values = sent.map((raw) => JSON.parse(raw).value);
        return values.length === 3 &&
            values[0] === 'first' &&
            values[1] === 'second' &&
            values[2] === 'third';
        "#,
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);
    let delivery_metrics_match_attempts = run_mock_script(
        r#"
        const metrics = globalThis.__streamlineMock.deliveryMetrics;
        return JSON.stringify(metrics) === JSON.stringify([
            [1, 0],
            [0, 1],
            [0, 1],
            [2, 0],
        ]);
        "#,
    )
    .and_then(|value| value.as_bool())
    .unwrap_or(false);

    let _ = producer.disconnect();
    restore_mock_websocket();

    assert!(mock_installed);
    assert!(connect_ok);
    assert!(first_send_ok);
    assert_eq!(sent_after_first, 1);
    assert_eq!(pending_after_first, 0);
    assert!(
        second_send_is_err,
        "send() must surface the transport failure"
    );
    assert_eq!(
        pending_after_second, 1,
        "the failed record must be retained, not dropped"
    );
    assert_eq!(errors_after_second, 1);
    assert!(
        third_send_is_err,
        "flush is still broken, so this must fail too"
    );
    assert_eq!(
        pending_after_third, 2,
        "both the failed and the newly-queued record must be retained, in order"
    );
    assert_eq!(
        errors_after_third, 2,
        "each flush retry records only its one attempted send failure, not the full backlog"
    );
    assert!(
        sent_unchanged_while_broken,
        "no duplicate sends while broken"
    );
    assert!(recovery_flush_ok);
    assert_eq!(
        pending_after_recovery, 0,
        "queue drains once the transport recovers"
    );
    assert_eq!(sent_after_recovery, 3);
    assert!(delivered_messages_in_order);
    assert!(
        delivery_metrics_match_attempts,
        "on_delivery must report failed attempts, while pending_count reports backlog"
    );
}

// ── WebSocket close code 1000: only intentional disconnect suppresses
//    reconnect ─────────────────────────────────────────────────────────

#[wasm_bindgen_test(async)]
async fn test_peer_initiated_close_code_1000_reconnects_when_auto_reconnect_enabled() {
    let mock_installed = install_mock_websocket();

    let mut conn = WsConnection::new("ws://mock.test/ws");
    assert!(conn.auto_reconnect(), "auto-reconnect defaults to enabled");
    let connect_ok = conn.connect().is_ok();
    drain_mock_tasks().await;
    let initially_connected = conn.is_connected();

    // Peer-initiated normal closure: the *server* (not our client) closes
    // cleanly with code 1000. This must behave like any other unexpected
    // drop and trigger reconnection, not be mistaken for our own
    // `disconnect()` call.
    let _ = run_mock_script(
        r#"globalThis.__streamlineMock.instances[0].close(1000, "server closed normally");"#,
    );
    drain_mock_tasks().await;

    let reconnect_socket_created =
        run_mock_script("return globalThis.__streamlineMock.instances.length === 2;")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let reconnected = conn.is_connected() && conn.state() == ConnectionState::Connected;

    conn.disconnect();
    restore_mock_websocket();

    assert!(mock_installed);
    assert!(connect_ok);
    assert!(initially_connected);
    assert!(
        reconnect_socket_created,
        "a new socket must be opened after a peer-initiated code-1000 close"
    );
    assert!(reconnected, "connection must recover automatically");
}

#[wasm_bindgen_test(async)]
async fn test_peer_initiated_close_code_1000_does_not_reconnect_when_auto_reconnect_disabled() {
    let mock_installed = install_mock_websocket();

    let mut conn = WsConnection::new("ws://mock.test/ws");
    conn.set_auto_reconnect(false);
    let connect_ok = conn.connect().is_ok();
    drain_mock_tasks().await;
    let initially_connected = conn.is_connected();

    let _ = run_mock_script(r#"globalThis.__streamlineMock.instances[0].close(1000, "bye");"#);
    drain_mock_tasks().await;

    let no_new_socket =
        run_mock_script("return globalThis.__streamlineMock.instances.length === 1;")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let stayed_disconnected = !conn.is_connected() && conn.state() == ConnectionState::Disconnected;

    conn.disconnect();
    restore_mock_websocket();

    assert!(mock_installed);
    assert!(connect_ok);
    assert!(initially_connected);
    assert!(
        no_new_socket,
        "auto_reconnect(false) must not reconnect even for code 1000"
    );
    assert!(stayed_disconnected);
}

#[wasm_bindgen_test(async)]
async fn test_intentional_disconnect_suppresses_reconnect_despite_close_code_1000() {
    let mock_installed = install_mock_websocket();

    let mut conn = WsConnection::new("ws://mock.test/ws");
    let connect_ok = conn.connect().is_ok();
    drain_mock_tasks().await;
    let initially_connected = conn.is_connected();

    // Our own `disconnect()` call closes the socket, and the mock's default
    // close code is 1000 — the same code a peer-initiated normal closure
    // uses. Only `intentional_disconnect` (not the close code) may suppress
    // reconnection, so this must stay disconnected.
    conn.disconnect();
    drain_mock_tasks().await;

    let no_new_socket =
        run_mock_script("return globalThis.__streamlineMock.instances.length === 1;")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let stayed_disconnected = !conn.is_connected() && conn.state() == ConnectionState::Disconnected;

    restore_mock_websocket();

    assert!(mock_installed);
    assert!(connect_ok);
    assert!(initially_connected);
    assert!(
        no_new_socket,
        "intentional disconnect must never trigger a reconnect"
    );
    assert!(stayed_disconnected);
}
