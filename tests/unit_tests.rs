//! Pure-logic unit tests that run natively (no browser/WASM required).
//!
//! Tests here cover the public API surface using `#[test]` so they can
//! run in `cargo test` without `wasm-pack`.
//!
//! Tests that trigger `JsValue` creation (error paths) are gated to
//! `target_arch = "wasm32"` because `JsValue::from_str` aborts outside WASM.

use streamline_wasm_sdk::{
    AdminAction, AdminClient, BrowserMessage, BrowserResponse, CircuitBreaker, CircuitState,
    ConnectionState, Consumer, ErrorCode, Producer, QueryClient, SchemaFormat,
    SchemaRegistryClient, StreamlineClient, StreamlineError, Telemetry, TopicAdmin, TopicInfo,
    WsConnection,
};

// ── StreamlineClient construction ────────────────────────────────────

#[test]
fn client_new_is_not_connected() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    assert!(!client.is_connected());
}

#[test]
fn client_disconnect_on_fresh() {
    let mut client = StreamlineClient::new("ws://localhost:9094/ws");
    client.disconnect();
    assert!(!client.is_connected());
}

#[test]
fn client_disconnect_is_idempotent() {
    let mut client = StreamlineClient::new("ws://localhost:9094/ws");
    client.disconnect();
    client.disconnect();
    client.disconnect();
    assert!(!client.is_connected());
}

#[test]
fn client_connection_state_initial() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    assert_eq!(client.connection_state(), ConnectionState::Disconnected);
}

// ── Producer batch logic ─────────────────────────────────────────────

#[test]
fn producer_initial_pending_count_is_zero() {
    let producer = Producer::new("ws://localhost:9094/ws", Some("topic".into()));
    assert_eq!(producer.pending_count(), 0);
}

#[test]
fn producer_batch_accumulates_under_threshold() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("topic".into()));
    // batch_size defaults to 100; send 99 without triggering auto-flush
    for i in 0..99 {
        let _ = producer.send(&format!("msg-{i}"), None);
    }
    assert_eq!(producer.pending_count(), 99);
}

#[test]
fn producer_set_batch_size_affects_capacity() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("topic".into()));
    producer.set_batch_size(50);
    for _ in 0..49 {
        let _ = producer.send("msg", None);
    }
    assert_eq!(producer.pending_count(), 49);
}

#[test]
fn producer_send_with_explicit_topic_overrides_default() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("default".into()));
    let _ = producer.send("msg", Some("override".into()));
    assert_eq!(producer.pending_count(), 1);
}

#[test]
fn producer_send_keyed_accumulates() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("topic".into()));
    let _ = producer.send_keyed("k1", "v1", None);
    let _ = producer.send_keyed("k2", "v2", None);
    assert_eq!(producer.pending_count(), 2);
}

#[test]
fn producer_flush_empty_batch_succeeds() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("topic".into()));
    let result = producer.flush();
    assert!(result.is_ok());
    assert_eq!(producer.pending_count(), 0);
}

#[test]
fn producer_disconnect_on_fresh() {
    let mut producer = Producer::new("ws://localhost:9094/ws", None);
    producer.disconnect();
    assert_eq!(producer.pending_count(), 0);
}

#[test]
fn producer_send_keyed_with_explicit_topic() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("default".into()));
    let _ = producer.send_keyed("key", "val", Some("explicit".into()));
    assert_eq!(producer.pending_count(), 1);
}

#[test]
fn producer_mixed_send_and_send_keyed() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("t".into()));
    let _ = producer.send("v1", None);
    let _ = producer.send_keyed("k", "v2", None);
    let _ = producer.send("v3", None);
    assert_eq!(producer.pending_count(), 3);
}

// Tests that trigger JsValue error paths (send without topic, flush with messages)
#[cfg(target_arch = "wasm32")]
mod producer_error_paths {
    use super::*;

    #[test]
    fn producer_send_without_topic_or_default_errors() {
        let mut producer = Producer::new("ws://localhost:9094/ws", None);
        let result = producer.send("value", None);
        assert!(result.is_err());
        assert_eq!(producer.pending_count(), 0);
    }

    #[test]
    fn producer_send_keyed_without_topic_errors() {
        let mut producer = Producer::new("ws://localhost:9094/ws", None);
        let result = producer.send_keyed("key", "value", None);
        assert!(result.is_err());
    }
}

// ── Consumer offset tracking ─────────────────────────────────────────

#[test]
fn consumer_initial_offsets() {
    let consumer = Consumer::new("ws://localhost:9094/ws", "test-topic");
    assert_eq!(consumer.current_offset(), 0);
    assert_eq!(consumer.committed_offset(), -1);
}

#[test]
fn consumer_advance_offset_stores_next() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
    consumer.advance_offset(0);
    assert_eq!(consumer.current_offset(), 1);
    consumer.advance_offset(1);
    assert_eq!(consumer.current_offset(), 2);
    consumer.advance_offset(2);
    assert_eq!(consumer.current_offset(), 3);
}

#[test]
fn consumer_advance_offset_ignores_lower() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
    consumer.advance_offset(10);
    assert_eq!(consumer.current_offset(), 11);
    consumer.advance_offset(5);
    assert_eq!(consumer.current_offset(), 11);
    consumer.advance_offset(9);
    assert_eq!(consumer.current_offset(), 11);
}

#[test]
fn consumer_advance_offset_accepts_equal_to_current() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
    consumer.advance_offset(5);
    assert_eq!(consumer.current_offset(), 6);
    // 5 < 6, ignored
    consumer.advance_offset(5);
    assert_eq!(consumer.current_offset(), 6);
    // 6 >= 6, advances
    consumer.advance_offset(6);
    assert_eq!(consumer.current_offset(), 7);
}

#[test]
fn consumer_advance_offset_large_jump() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
    consumer.advance_offset(1_000_000);
    assert_eq!(consumer.current_offset(), 1_000_001);
}

// consumer.stop() calls send_message which creates JsValue on error — WASM only
#[cfg(target_arch = "wasm32")]
#[test]
fn consumer_stop_on_fresh_does_not_panic() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
    consumer.stop();
}

#[test]
fn consumer_advance_offset_zero_on_fresh() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
    consumer.advance_offset(0);
    assert_eq!(consumer.current_offset(), 1);
    assert_eq!(consumer.committed_offset(), -1);
}

#[test]
fn consumer_committed_offset_stays_negative_without_commit() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
    consumer.advance_offset(100);
    assert_eq!(consumer.current_offset(), 101);
    // committed_offset stays at -1 because we never called commit
    assert_eq!(consumer.committed_offset(), -1);
}

// Tests that trigger JsValue error paths (commit/seek without connection)
#[cfg(target_arch = "wasm32")]
mod consumer_error_paths {
    use super::*;

    #[test]
    fn consumer_commit_requires_connection() {
        let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
        assert!(consumer.commit().is_err());
    }

    #[test]
    fn consumer_commit_offset_requires_connection() {
        let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
        assert!(consumer.commit_offset(10).is_err());
    }

    #[test]
    fn consumer_seek_requires_connection() {
        let mut consumer = Consumer::new("ws://localhost:9094/ws", "topic");
        assert!(consumer.seek(10).is_err());
    }
}

// ── TopicAdmin construction ──────────────────────────────────────────

#[test]
fn topic_admin_disconnect_on_fresh() {
    let mut admin = TopicAdmin::new("ws://localhost:9094/ws");
    admin.disconnect();
}

// Tests that trigger JsValue error paths (operations without connection)
#[cfg(target_arch = "wasm32")]
mod topic_admin_error_paths {
    use super::*;

    #[test]
    fn topic_admin_list_requires_connection() {
        let admin = TopicAdmin::new("ws://localhost:9094/ws");
        assert!(admin.list_topics().is_err());
    }

    #[test]
    fn topic_admin_create_requires_connection() {
        let admin = TopicAdmin::new("ws://localhost:9094/ws");
        assert!(admin.create_topic("t", Some(3)).is_err());
    }

    #[test]
    fn topic_admin_delete_requires_connection() {
        let admin = TopicAdmin::new("ws://localhost:9094/ws");
        assert!(admin.delete_topic("t").is_err());
    }
}

// ── WsConnection ─────────────────────────────────────────────────────

#[test]
fn ws_connection_initial_state() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert_eq!(conn.state(), ConnectionState::Disconnected);
    assert!(!conn.is_connected());
}

#[test]
fn ws_connection_disconnect_fresh() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    conn.disconnect();
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}

#[test]
fn ws_connection_auto_reconnect_default() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert!(conn.auto_reconnect());
}

#[test]
fn ws_connection_toggle_auto_reconnect() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    conn.set_auto_reconnect(false);
    assert!(!conn.auto_reconnect());
    conn.set_auto_reconnect(true);
    assert!(conn.auto_reconnect());
}

#[test]
fn ws_connection_reconnect_delay_initial() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert_eq!(conn.reconnect_delay_ms(), 1000);
}

#[test]
fn ws_connection_set_max_reconnect_attempts() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    conn.set_max_reconnect_attempts(0);
    conn.set_max_reconnect_attempts(10);
    conn.set_max_reconnect_attempts(100);
}

#[test]
fn ws_connection_reconnect_attempts_initial() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert_eq!(conn.reconnect_attempts(), 0);
}

// JsValue error path
#[cfg(target_arch = "wasm32")]
#[test]
fn ws_connection_send_fails_without_connection() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert!(conn.send("test").is_err());
}

// ── SDK version ──────────────────────────────────────────────────────

#[test]
fn sdk_version_is_not_empty() {
    assert!(!streamline_wasm_sdk::SDK_VERSION.is_empty());
}

#[test]
fn sdk_version_is_semver() {
    let parts: Vec<&str> = streamline_wasm_sdk::SDK_VERSION.split('.').collect();
    assert_eq!(parts.len(), 3, "Version should be semver (x.y.z)");
    for part in parts {
        assert!(
            part.parse::<u32>().is_ok(),
            "Each version part should be numeric"
        );
    }
}

// ── Cross-type integration ───────────────────────────────────────────

#[test]
fn produce_message_serializes_correctly_for_client() {
    let msg = BrowserMessage::Produce {
        topic: "orders".into(),
        key: Some("user-1".into()),
        value: r#"{"item":"widget"}"#.into(),
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "produce");
    assert_eq!(parsed["topic"], "orders");
    assert_eq!(parsed["key"], "user-1");
}

#[test]
fn subscribe_unsubscribe_symmetry() {
    let sub = BrowserMessage::Subscribe {
        topic: "events".into(),
    };
    let unsub = BrowserMessage::Unsubscribe {
        topic: "events".into(),
    };
    let sub_json: serde_json::Value = serde_json::from_str(&sub.to_json().unwrap()).unwrap();
    let unsub_json: serde_json::Value = serde_json::from_str(&unsub.to_json().unwrap()).unwrap();
    assert_eq!(sub_json["topic"], unsub_json["topic"]);
    assert_ne!(sub_json["type"], unsub_json["type"]);
}

#[test]
fn error_response_round_trip() {
    let resp = BrowserResponse::Error {
        code: 503,
        message: "service unavailable".into(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    let deser = BrowserResponse::from_json(&json).unwrap();
    match deser {
        BrowserResponse::Error { code, message } => {
            assert_eq!(code, 503);
            assert_eq!(message, "service unavailable");
        }
        _ => panic!("expected Error"),
    }
}

#[test]
fn admin_message_create_with_partitions() {
    let msg = BrowserMessage::Admin {
        action: AdminAction::CreateTopic {
            name: "orders".into(),
            partitions: Some(12),
        },
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "admin");
    assert_eq!(parsed["action"]["action"], "create_topic");
    assert_eq!(parsed["action"]["name"], "orders");
    assert_eq!(parsed["action"]["partitions"], 12);
}

#[test]
fn topic_list_response_with_many_topics() {
    let topics: Vec<TopicInfo> = (0..100)
        .map(|i| TopicInfo {
            name: format!("topic-{i}"),
            partitions: (i % 8 + 1) as u32,
        })
        .collect();
    let resp = BrowserResponse::TopicList { topics };
    let json = serde_json::to_string(&resp).unwrap();
    let deser = BrowserResponse::from_json(&json).unwrap();
    match deser {
        BrowserResponse::TopicList { topics } => {
            assert_eq!(topics.len(), 100);
            assert_eq!(topics[0].name, "topic-0");
            assert_eq!(topics[99].name, "topic-99");
        }
        _ => panic!("expected TopicList"),
    }
}

#[test]
fn message_response_with_large_timestamp() {
    let json = r#"{"type":"message","topic":"t","value":"v","offset":0,"timestamp":9999999999999}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Message { timestamp, .. } => {
            assert_eq!(timestamp, 9999999999999);
        }
        _ => panic!("expected Message"),
    }
}

// ── CircuitBreaker from public API ───────────────────────────────────

#[test]
fn circuit_breaker_public_api() {
    let mut cb = CircuitBreaker::new(3, 2, 30000.0);
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.allow());
    assert_eq!(cb.failure_count(), 0);
    assert_eq!(cb.success_count(), 0);

    cb.record_failure();
    assert_eq!(cb.failure_count(), 1);

    cb.record_success();
    assert_eq!(cb.failure_count(), 0);

    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
}

// ── Error types from public API ──────────────────────────────────────

#[test]
fn streamline_error_public_api() {
    let err = StreamlineError::new(ErrorCode::TopicNotFound, "no such topic", false);
    assert_eq!(err.code(), ErrorCode::TopicNotFound);
    assert_eq!(err.message(), "no such topic");
    assert!(!err.retryable());
    assert!(err.hint().contains("admin API"));
}

#[test]
fn streamline_error_retryable_vs_non_retryable() {
    let retryable = StreamlineError::new(ErrorCode::Timeout, "slow", true);
    assert!(retryable.retryable());
    assert!(retryable.is_retryable());

    let non_retryable = StreamlineError::new(ErrorCode::AuthenticationFailed, "bad", false);
    assert!(!non_retryable.retryable());
    assert!(!non_retryable.is_retryable());
}

// ── Telemetry from public API ────────────────────────────────────────

#[test]
fn telemetry_disabled_workflow() {
    let t = Telemetry::disabled();
    assert!(!t.enabled());
    let span = t.start_produce("t");
    t.end_span(span);
}

#[test]
fn telemetry_enabled_creates_spans() {
    // Use disabled() since enabled telemetry calls browser console APIs
    let t = Telemetry::disabled();
    let span = t.start_consume("events");
    assert_eq!(span.label(), "events consume");
}

#[test]
fn telemetry_all_span_types() {
    let t = Telemetry::disabled();
    let produce = t.start_produce("t");
    let consume = t.start_consume("t");
    let process = t.start_process("t");
    assert_eq!(produce.label(), "t produce");
    assert_eq!(consume.label(), "t consume");
    assert_eq!(process.label(), "t process");
    t.end_span(produce);
    t.end_span(consume);
    t.end_span_with_error(process, "test error");
}

// ── SchemaFormat from public API ─────────────────────────────────────

#[test]
fn schema_format_values() {
    assert_eq!(SchemaFormat::Avro, SchemaFormat::Avro);
    assert_ne!(SchemaFormat::Avro, SchemaFormat::Json);
    assert_ne!(SchemaFormat::Json, SchemaFormat::Protobuf);
}

// ── AdminClient / QueryClient from public API ────────────────────────

#[test]
fn admin_client_construction() {
    let mut client = AdminClient::new("http://localhost:9094");
    client.set_auth_token("secret");
}

#[test]
fn query_client_construction() {
    let mut client = QueryClient::new("http://localhost:9094");
    client.set_auth_token("token");
}

// ── SchemaRegistryClient from public API ─────────────────────────────

#[test]
fn schema_registry_client_construction() {
    let mut client = SchemaRegistryClient::new("http://localhost:9094/");
    client.set_auth_token("tok");
}

// validate_json happy path works natively; error paths need WASM
#[test]
fn schema_registry_validate_json_happy_path() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    let schema = r#"{"type":"object","required":["name"]}"#;
    let valid = r#"{"name":"test"}"#;
    let invalid = r#"{"foo":"bar"}"#;
    assert!(client.validate_json(schema, valid).unwrap());
    assert!(!client.validate_json(schema, invalid).unwrap());
}

#[test]
fn schema_registry_validate_json_string_type() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    assert!(client
        .validate_json(r#"{"type":"string"}"#, r#""hello""#)
        .unwrap());
}

#[test]
fn schema_registry_validate_json_number_type() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    assert!(client.validate_json(r#"{"type":"number"}"#, "42").unwrap());
}

#[test]
fn schema_registry_validate_json_boolean_type() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    assert!(client
        .validate_json(r#"{"type":"boolean"}"#, "true")
        .unwrap());
}

#[test]
fn schema_registry_validate_json_array_type() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    assert!(client
        .validate_json(r#"{"type":"array"}"#, "[1,2,3]")
        .unwrap());
}

#[test]
fn schema_registry_validate_json_null_type() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    assert!(client.validate_json(r#"{"type":"null"}"#, "null").unwrap());
}

#[test]
fn schema_registry_validate_json_no_type_passes_anything() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    assert!(client.validate_json(r#"{}"#, r#""anything""#).unwrap());
    assert!(client.validate_json(r#"{}"#, "42").unwrap());
}

// Error path tests (JsValue creation) — WASM only
#[cfg(target_arch = "wasm32")]
mod schema_registry_error_paths {
    use super::*;

    #[test]
    fn validate_json_invalid_schema() {
        let client = SchemaRegistryClient::new("http://localhost:9094");
        assert!(client.validate_json("not json", r#"{"a":1}"#).is_err());
    }

    #[test]
    fn validate_json_invalid_value() {
        let client = SchemaRegistryClient::new("http://localhost:9094");
        assert!(client
            .validate_json(r#"{"type":"object"}"#, "not json")
            .is_err());
    }
}
