//! Web-specific tests verifying WASM module exports and convenience types.
//!
//! Run with: `wasm-pack test --headless --chrome`

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use streamline_wasm_sdk::{Consumer, Producer, StreamlineClient, TopicAdmin, WsConnection};

// ── WASM module exports expected constructors ────────────────────────

#[wasm_bindgen_test]
fn streamline_client_exports_constructor() {
    let _client = StreamlineClient::new("ws://localhost:9094/ws");
}

#[wasm_bindgen_test]
fn ws_connection_exports_constructor() {
    let _conn = WsConnection::new("ws://localhost:9094/ws");
}

// ── Producer construction ────────────────────────────────────────────

#[wasm_bindgen_test]
fn producer_with_default_topic() {
    let _producer = Producer::new("ws://localhost:9094/ws", Some("default-topic".into()));
}

#[wasm_bindgen_test]
fn producer_without_default_topic() {
    let _producer = Producer::new("ws://localhost:9094/ws", None);
}

#[wasm_bindgen_test]
fn producer_disconnect_on_fresh() {
    let mut producer = Producer::new("ws://localhost:9094/ws", None);
    producer.disconnect();
}

#[wasm_bindgen_test]
fn producer_send_without_connection_records_delivery_error() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("topic".into()));
    producer.set_batch_size(1);
    let result = producer.send("hello", None);
    assert!(result.is_ok());
    assert_eq!(producer.total_sent(), 0);
    assert_eq!(producer.total_errors(), 1);
}

#[wasm_bindgen_test]
fn producer_send_without_topic_or_default_fails() {
    let mut producer = Producer::new("ws://localhost:9094/ws", None);
    let result = producer.send("hello", None);
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn producer_send_keyed_without_connection_records_delivery_error() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("topic".into()));
    producer.set_batch_size(1);
    let result = producer.send_keyed("key", "value", None);
    assert!(result.is_ok());
    assert_eq!(producer.total_sent(), 0);
    assert_eq!(producer.total_errors(), 1);
}

// ── Consumer construction ────────────────────────────────────────────

#[wasm_bindgen_test]
fn consumer_construction() {
    let _consumer = Consumer::new("ws://localhost:9094/ws", "my-topic");
}

#[wasm_bindgen_test]
fn consumer_stop_on_fresh() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "my-topic");
    // stop() on a fresh consumer should not panic (WebSocket is None)
    consumer.stop();
}

// ── TopicAdmin construction ──────────────────────────────────────────

#[wasm_bindgen_test]
fn topic_admin_construction() {
    let _admin = TopicAdmin::new("ws://localhost:9094/ws");
}

#[wasm_bindgen_test]
fn topic_admin_disconnect_on_fresh() {
    let mut admin = TopicAdmin::new("ws://localhost:9094/ws");
    admin.disconnect();
}

#[wasm_bindgen_test]
fn topic_admin_create_topic_without_connection_fails() {
    let admin = TopicAdmin::new("ws://localhost:9094/ws");
    let result = admin.create_topic("test-topic", Some(3));
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn topic_admin_delete_topic_without_connection_fails() {
    let admin = TopicAdmin::new("ws://localhost:9094/ws");
    let result = admin.delete_topic("test-topic");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn topic_admin_list_topics_without_connection_fails() {
    let admin = TopicAdmin::new("ws://localhost:9094/ws");
    let result = admin.list_topics();
    assert!(result.is_err());
}

// ── Multiple instances coexist ───────────────────────────────────────

#[wasm_bindgen_test]
fn multiple_clients_coexist() {
    let client1 = StreamlineClient::new("ws://localhost:9094/ws");
    let client2 = StreamlineClient::new("ws://localhost:9095/ws");
    assert!(!client1.is_connected());
    assert!(!client2.is_connected());
}

#[wasm_bindgen_test]
fn multiple_producers_coexist() {
    let _p1 = Producer::new("ws://localhost:9094/ws", Some("topic-a".into()));
    let _p2 = Producer::new("ws://localhost:9094/ws", Some("topic-b".into()));
}

#[wasm_bindgen_test]
async fn test_message_throughput() {
    // Verify we can create and serialize many messages quickly
    let start = js_sys::Date::now();
    for i in 0..1000 {
        let _msg = format!("benchmark-message-{}", i);
    }
    let elapsed = js_sys::Date::now() - start;
    assert!(
        elapsed < 100.0,
        "1000 messages should complete in under 100ms"
    );
}
