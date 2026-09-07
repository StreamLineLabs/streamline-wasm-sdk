//! Reconnection and connection lifecycle tests for the Streamline WASM SDK.

use streamline_wasm_sdk::{ConnectionState, StreamlineClient, WsConnection};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ── Reconnection configuration ───────────────────────────────────────

#[wasm_bindgen_test]
fn default_reconnect_delay_is_one_second() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert_eq!(conn.reconnect_delay_ms(), 1000);
}

#[wasm_bindgen_test]
fn reconnect_delay_starts_at_1s() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    // Fresh connection should have base delay of 1s (no attempts yet)
    assert_eq!(conn.reconnect_delay_ms(), 1000);
}

#[wasm_bindgen_test]
fn set_max_reconnect_attempts_does_not_panic() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    conn.set_max_reconnect_attempts(0);
    conn.set_max_reconnect_attempts(1);
    conn.set_max_reconnect_attempts(100);
}

// ── Connection state lifecycle ───────────────────────────────────────

#[wasm_bindgen_test]
fn initial_state_is_disconnected() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}

#[wasm_bindgen_test]
fn disconnect_from_disconnected_is_idempotent() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    conn.disconnect();
    assert_eq!(conn.state(), ConnectionState::Disconnected);
    conn.disconnect();
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}

#[wasm_bindgen_test]
#[cfg(target_arch = "wasm32")]
fn connect_transitions_to_connecting() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    let _ = conn.connect();
    assert_eq!(conn.state(), ConnectionState::Connecting);
}

#[wasm_bindgen_test]
#[cfg(target_arch = "wasm32")]
fn disconnect_from_connecting_goes_to_disconnected() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    let _ = conn.connect();
    conn.disconnect();
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}

#[wasm_bindgen_test]
#[cfg(target_arch = "wasm32")]
fn repeated_connect_disconnect_cycles() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    for _ in 0..5 {
        let _ = conn.connect();
        assert!(!conn.is_connected()); // Connecting, not yet Connected
        conn.disconnect();
        assert_eq!(conn.state(), ConnectionState::Disconnected);
    }
}

// ── Client lifecycle ─────────────────────────────────────────────────

#[wasm_bindgen_test]
fn client_fresh_state() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    assert!(!client.is_connected());
}

#[wasm_bindgen_test]
fn client_disconnect_is_idempotent() {
    let mut client = StreamlineClient::new("ws://localhost:9094/ws");
    client.disconnect();
    client.disconnect();
    assert!(!client.is_connected());
}

#[wasm_bindgen_test]
fn client_produce_without_connection_fails() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    let result = client.produce("topic", "value");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn client_produce_with_key_without_connection_fails() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    let result = client.produce_with_key("topic", Some("key".into()), "value");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn client_unsubscribe_without_connection_fails() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    let result = client.unsubscribe("topic");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn client_create_topic_without_connection_fails() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    let result = client.create_topic("new-topic", Some(3));
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn client_delete_topic_without_connection_fails() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    let result = client.delete_topic("old-topic");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn client_list_topics_without_connection_fails() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    let result = client.list_topics();
    assert!(result.is_err());
}

// ── Different URL formats ────────────────────────────────────────────

#[wasm_bindgen_test]
fn connection_with_wss_url() {
    let conn = WsConnection::new("wss://secure.example.com/ws");
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}

#[wasm_bindgen_test]
fn connection_with_port_in_url() {
    let conn = WsConnection::new("ws://localhost:8080/ws");
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}

#[wasm_bindgen_test]
fn connection_with_path_in_url() {
    let conn = WsConnection::new("ws://localhost:9094/api/v1/ws");
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}
// resolve panic in wasm32 target serialization
