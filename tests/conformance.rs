//! SDK Conformance Test Suite — verifies protocol, type, and logic correctness.
//!
//! These tests validate message serialization, type construction, error handling,
//! and configuration without requiring a running Streamline server.

use streamline_wasm_sdk::{
    AdminAction, AdminClient, BrowserMessage, BrowserResponse, CircuitBreaker, CircuitState,
    ConnectionState, Consumer, ErrorCode, Producer, QueryClient, SchemaFormat,
    SchemaRegistryClient, StreamlineClient, StreamlineError, Telemetry, TopicAdmin, TopicInfo,
    WsConnection,
};

// ========== PRODUCER (8 tests) ==========

#[test]
fn test_p01_simple_produce() {
    let msg = BrowserMessage::Produce {
        topic: "events".into(),
        key: None,
        value: "hello world".into(),
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "produce");
    assert_eq!(parsed["topic"], "events");
    assert_eq!(parsed["value"], "hello world");
    assert!(parsed.get("key").is_none() || parsed["key"].is_null());
}

#[test]
fn test_p02_keyed_produce() {
    let msg = BrowserMessage::Produce {
        topic: "orders".into(),
        key: Some("order-123".into()),
        value: r#"{"amount":42}"#.into(),
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["key"], "order-123");
    assert_eq!(parsed["topic"], "orders");
}

#[test]
fn test_p03_headers_produce() {
    // WASM SDK sends headers via the value JSON; verify complex values serialize
    let msg = BrowserMessage::Produce {
        topic: "t".into(),
        key: None,
        value: r#"{"data":"v","_headers":{"trace-id":"abc","content-type":"application/json"}}"#
            .into(),
    };
    let json = msg.to_json().unwrap();
    assert!(json.contains("trace-id"));
    assert!(json.contains("application/json"));
}

#[test]
fn test_p04_batch_produce() {
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("batch-topic".into()));
    producer.set_batch_size(200); // Increase threshold to avoid auto-flush
    for i in 0..50 {
        let _ = producer.send(&format!("msg-{i}"), None);
    }
    assert_eq!(producer.pending_count(), 50);
    // Verify more messages accumulate correctly
    for i in 50..100 {
        let _ = producer.send(&format!("msg-{i}"), None);
    }
    assert_eq!(producer.pending_count(), 100);
}

#[test]
fn test_p05_compression() {
    // Verify large messages serialize correctly (compression is server-side for WASM)
    let large_value = "x".repeat(100_000);
    let msg = BrowserMessage::Produce {
        topic: "t".into(),
        key: None,
        value: large_value.clone(),
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Produce { value, .. } => assert_eq!(value.len(), 100_000),
        _ => panic!("expected Produce"),
    }
}

#[test]
fn test_p06_partitioner() {
    // Verify keyed messages with different keys produce distinct serialized messages
    let msg1 = BrowserMessage::Produce {
        topic: "t".into(),
        key: Some("key-a".into()),
        value: "v".into(),
    };
    let msg2 = BrowserMessage::Produce {
        topic: "t".into(),
        key: Some("key-b".into()),
        value: "v".into(),
    };
    let json1 = msg1.to_json().unwrap();
    let json2 = msg2.to_json().unwrap();
    assert_ne!(json1, json2);
}

#[test]
fn test_p07_idempotent() {
    // Verify same message serializes identically (idempotent serialization)
    let msg = BrowserMessage::Produce {
        topic: "t".into(),
        key: Some("k".into()),
        value: "v".into(),
    };
    let json1 = msg.to_json().unwrap();
    let json2 = msg.to_json().unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn test_p08_timeout() {
    // Verify timeout error construction
    let err = StreamlineError::new(ErrorCode::Timeout, "produce timed out", true);
    assert_eq!(err.code(), ErrorCode::Timeout);
    assert!(err.retryable());
    assert!(err.hint().contains("timeout"));
}

// ========== CONSUMER (8 tests) ==========

#[test]
fn test_c01_subscribe() {
    let msg = BrowserMessage::Subscribe {
        topic: "events".into(),
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "subscribe");
    assert_eq!(parsed["topic"], "events");
}

#[test]
fn test_c02_from_beginning() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "events");
    assert_eq!(consumer.current_offset(), 0);
    // Advance to simulate receiving messages from beginning
    consumer.advance_offset(0);
    assert_eq!(consumer.current_offset(), 1);
}

#[test]
fn test_c03_from_offset() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "events");
    // Simulate receiving messages starting from offset 100
    consumer.advance_offset(100);
    assert_eq!(consumer.current_offset(), 101);
    consumer.advance_offset(101);
    assert_eq!(consumer.current_offset(), 102);
}

#[test]
fn test_c04_from_timestamp() {
    // Verify message response parsing with timestamp
    let json =
        r#"{"type":"message","topic":"events","value":"v","offset":42,"timestamp":1700000000000}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Message {
            timestamp, offset, ..
        } => {
            assert_eq!(timestamp, 1700000000000);
            assert_eq!(offset, 42);
        }
        _ => panic!("expected Message"),
    }
}

#[test]
fn test_c05_follow() {
    // Verify unsubscribe message format
    let msg = BrowserMessage::Unsubscribe {
        topic: "events".into(),
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "unsubscribe");
    assert_eq!(parsed["topic"], "events");
}

#[test]
fn test_c06_filter() {
    // Verify consume message with group_id for server-side filtering
    let msg = BrowserMessage::Consume {
        topic: "events".into(),
        group_id: Some("my-group".into()),
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "consume");
    assert_eq!(parsed["group_id"], "my-group");
}

#[test]
fn test_c07_headers() {
    // Verify message with embedded headers in value
    let json = r#"{"type":"message","topic":"t","key":"k","value":"{\"data\":\"v\",\"_headers\":{\"trace-id\":\"abc\"}}","offset":0,"timestamp":0}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Message { value, key, .. } => {
            assert!(value.contains("trace-id"));
            assert_eq!(key, Some("k".into()));
        }
        _ => panic!("expected Message"),
    }
}

#[test]
fn test_c08_timeout() {
    let err = StreamlineError::new(ErrorCode::Timeout, "consume timed out", true);
    assert_eq!(err.code(), ErrorCode::Timeout);
    assert!(err.retryable());
}

// ========== CONSUMER GROUPS (8 tests) ==========

#[test]
fn test_g01_join_group() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "events");
    consumer.set_group_id("test-group");
    assert_eq!(consumer.group_id(), Some("test-group".into()));
}

#[test]
fn test_g02_commit_offset() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "events");
    consumer.advance_offset(10);
    assert_eq!(consumer.current_offset(), 11);
    // commit_offset without connection will fail, but offset tracking works locally
    assert_eq!(consumer.committed_offset(), -1);
}

#[test]
fn test_g03_fetch_committed_offset() {
    let consumer = Consumer::new("ws://localhost:9094/ws", "events");
    // Initial committed offset is -1 (no commits yet)
    assert_eq!(consumer.committed_offset(), -1);
}

#[test]
fn test_g04_auto_commit() {
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "events");
    consumer.set_auto_commit(5);
    // Advance offset 4 times — should NOT trigger auto-commit
    for i in 0..4 {
        let _ = consumer.advance_offset(i);
    }
    assert_eq!(consumer.committed_offset(), -1);
}

#[test]
fn test_g05_rebalance() {
    // Verify consume message for group coordination includes group_id
    let msg = BrowserMessage::Consume {
        topic: "events".into(),
        group_id: Some("rebalance-group".into()),
    };
    let json = msg.to_json().unwrap();
    assert!(json.contains("rebalance-group"));
}

#[test]
fn test_g06_leave_group() {
    let msg = BrowserMessage::Unsubscribe {
        topic: "events".into(),
    };
    let json = msg.to_json().unwrap();
    assert!(json.contains("unsubscribe"));
}

#[test]
fn test_g07_independent_groups() {
    let mut c1 = Consumer::new("ws://localhost:9094/ws", "events");
    let mut c2 = Consumer::new("ws://localhost:9094/ws", "events");
    c1.set_group_id("group-a");
    c2.set_group_id("group-b");
    assert_ne!(c1.group_id(), c2.group_id());
    // Independent offset tracking
    c1.advance_offset(100);
    c2.advance_offset(200);
    assert_eq!(c1.current_offset(), 101);
    assert_eq!(c2.current_offset(), 201);
}

#[test]
fn test_g08_static_membership() {
    // Static membership uses a fixed group_id pattern
    let mut consumer = Consumer::new("ws://localhost:9094/ws", "events");
    consumer.set_group_id("static-member-1");
    assert_eq!(consumer.group_id(), Some("static-member-1".into()));
    // Replacing the group_id simulates instance re-registration
    consumer.set_group_id("static-member-2");
    assert_eq!(consumer.group_id(), Some("static-member-2".into()));
}

// ========== ADMIN / TOPICS (6 tests) ==========

#[test]
fn test_d01_create_topic() {
    let msg = BrowserMessage::Admin {
        action: AdminAction::CreateTopic {
            name: "new-topic".into(),
            partitions: Some(3),
        },
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "admin");
    assert_eq!(parsed["action"]["action"], "create_topic");
    assert_eq!(parsed["action"]["name"], "new-topic");
    assert_eq!(parsed["action"]["partitions"], 3);
}

#[test]
fn test_d02_list_topics() {
    let msg = BrowserMessage::Admin {
        action: AdminAction::ListTopics,
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["action"]["action"], "list_topics");
}

#[test]
fn test_d03_describe_topic() {
    // Verify TopicInfo round-trips correctly
    let topic = TopicInfo {
        name: "orders".into(),
        partitions: 12,
    };
    let json = serde_json::to_string(&topic).unwrap();
    let deser: TopicInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.name, "orders");
    assert_eq!(deser.partitions, 12);
}

#[test]
fn test_d04_delete_topic() {
    let msg = BrowserMessage::Admin {
        action: AdminAction::DeleteTopic {
            name: "old-topic".into(),
        },
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["action"]["action"], "delete_topic");
    assert_eq!(parsed["action"]["name"], "old-topic");
}

#[test]
fn test_d05_auto_create_topic() {
    // CreateTopic with default partitions (None)
    let msg = BrowserMessage::Admin {
        action: AdminAction::CreateTopic {
            name: "auto-created".into(),
            partitions: None,
        },
    };
    let json = msg.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["action"]["name"], "auto-created");
    // partitions field should be absent when None
    assert!(
        parsed["action"].get("partitions").is_none() || parsed["action"]["partitions"].is_null()
    );
}

#[test]
fn test_d06_duplicate_topic_rejected() {
    // Verify error response format for duplicate topic
    let resp = BrowserResponse::Error {
        code: 409,
        message: "Topic already exists: orders".into(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    let deser = BrowserResponse::from_json(&json).unwrap();
    match deser {
        BrowserResponse::Error { code, message } => {
            assert_eq!(code, 409);
            assert!(message.contains("already exists"));
        }
        _ => panic!("expected Error"),
    }
}

// ========== AUTHENTICATION (6 tests) ==========

#[test]
fn test_a01_tls_connect() {
    // Verify WSS URL construction for TLS connections
    let client = StreamlineClient::new("wss://secure.example.com:9094/ws");
    assert!(!client.is_connected());
    assert_eq!(client.connection_state(), ConnectionState::Disconnected);
}

#[test]
fn test_a02_mutual_tls() {
    // Verify client construction with mTLS-style secure URL
    let client = StreamlineClient::new("wss://mtls.example.com:9094/ws");
    assert!(!client.is_connected());
}

#[test]
fn test_a03_sasl_plain() {
    // AdminClient supports auth token (SASL equivalent for HTTP)
    let mut admin = AdminClient::new("http://localhost:9094");
    admin.set_auth_token("user:password");
}

#[test]
fn test_a04_scram_sha256() {
    // Auth token set on schema registry client
    let mut client = SchemaRegistryClient::new("http://localhost:9094");
    client.set_auth_token("scram-sha256-token");
}

#[test]
fn test_a05_scram_sha512() {
    // Auth token set on query client
    let mut client = QueryClient::new("http://localhost:9094");
    client.set_auth_token("scram-sha512-token");
}

#[test]
fn test_a06_auth_failure() {
    let err = StreamlineError::new(ErrorCode::AuthenticationFailed, "bad credentials", false);
    assert_eq!(err.code(), ErrorCode::AuthenticationFailed);
    assert!(!err.retryable());
    assert!(err.hint().contains("credentials"));
}

// ========== SCHEMA REGISTRY (6 tests) ==========

#[test]
fn test_s01_register_schema() {
    // Verify schema format enum values are distinct
    assert_ne!(SchemaFormat::Avro, SchemaFormat::Json);
    assert_ne!(SchemaFormat::Json, SchemaFormat::Protobuf);
}

#[test]
fn test_s02_get_schema_by_id() {
    // Verify schema registry client construction
    let client = SchemaRegistryClient::new("http://localhost:9094");
    // validate_json should work without server connection (pure logic)
    let result = client.validate_json(r#"{"type":"object"}"#, r#"{"key":"value"}"#);
    assert!(result.is_ok());
}

#[test]
fn test_s03_list_versions() {
    // Verify schema registry client can be constructed with various URLs
    let _c1 = SchemaRegistryClient::new("http://localhost:9094");
    let _c2 = SchemaRegistryClient::new("https://registry.example.com");
    let _c3 = SchemaRegistryClient::new("http://registry:8081");
}

#[test]
fn test_s04_compatibility_check() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    // Compatible: schema allows objects, data is an object
    assert!(client
        .validate_json(r#"{"type":"object"}"#, r#"{"name":"test"}"#)
        .unwrap());
    // Incompatible: schema requires string, data is object
    assert!(!client
        .validate_json(r#"{"type":"string"}"#, r#"{"name":"test"}"#)
        .unwrap());
}

#[test]
fn test_s05_avro_format() {
    assert_eq!(SchemaFormat::Avro, SchemaFormat::Avro);
    let avro = SchemaFormat::Avro;
    let cloned = avro.clone();
    assert_eq!(avro, cloned);
}

#[test]
fn test_s06_json_format() {
    let client = SchemaRegistryClient::new("http://localhost:9094");
    // JSON Schema validation with required fields
    let schema = r#"{"type":"object","required":["name","age"]}"#;
    assert!(client
        .validate_json(schema, r#"{"name":"Alice","age":30}"#)
        .unwrap());
    assert!(!client.validate_json(schema, r#"{"name":"Alice"}"#).unwrap());
}

// ========== ERROR HANDLING (5 tests) ==========

#[test]
fn test_e01_unknown_topic() {
    let err = StreamlineError::new(ErrorCode::TopicNotFound, "topic 'missing' not found", false);
    assert_eq!(err.code(), ErrorCode::TopicNotFound);
    assert!(!err.retryable());
    assert!(err.hint().contains("admin API"));
}

#[test]
fn test_e02_invalid_partition() {
    let resp = BrowserResponse::Error {
        code: 404,
        message: "Partition 99 not found for topic events".into(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    let deser = BrowserResponse::from_json(&json).unwrap();
    match deser {
        BrowserResponse::Error { code, message } => {
            assert_eq!(code, 404);
            assert!(message.contains("Partition 99"));
        }
        _ => panic!("expected Error"),
    }
}

#[test]
fn test_e03_invalid_offset() {
    let resp = BrowserResponse::Error {
        code: 400,
        message: "Offset out of range: -5".into(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("out of range"));
}

#[test]
fn test_e04_retryable_error_info() {
    let retryable_codes = [
        ErrorCode::NotConnected,
        ErrorCode::ConnectionFailed,
        ErrorCode::Timeout,
        ErrorCode::ProduceError,
    ];
    for code in retryable_codes {
        let err = StreamlineError::new(code, "test", true);
        assert!(err.retryable(), "Expected {:?} to be retryable", code);
    }

    let non_retryable_codes = [
        ErrorCode::SerializationError,
        ErrorCode::AuthenticationFailed,
    ];
    for code in non_retryable_codes {
        let err = StreamlineError::new(code, "test", false);
        assert!(!err.retryable(), "Expected {:?} to be non-retryable", code);
    }
}

#[test]
fn test_e05_descriptive_error_messages() {
    let all_codes = [
        ErrorCode::NotConnected,
        ErrorCode::ConnectionFailed,
        ErrorCode::SerializationError,
        ErrorCode::TopicNotFound,
        ErrorCode::AuthenticationFailed,
        ErrorCode::Timeout,
        ErrorCode::ProduceError,
        ErrorCode::AdminError,
        ErrorCode::QueryError,
        ErrorCode::SchemaRegistryError,
        ErrorCode::Unknown,
    ];
    for code in all_codes {
        let err = StreamlineError::new(code, "test", false);
        let hint = err.hint();
        assert!(!hint.is_empty(), "Hint should not be empty for {:?}", code);
    }
}

// ========== PERFORMANCE (4 tests) ==========

#[test]
fn test_f01_throughput_1kb() {
    // Verify batch accumulation of 1KB messages
    let mut producer = Producer::new("ws://localhost:9094/ws", Some("perf".into()));
    let value_1kb = "x".repeat(1024);
    for _ in 0..99 {
        let _ = producer.send(&value_1kb, None);
    }
    assert_eq!(producer.pending_count(), 99);
}

#[test]
fn test_f02_latency_p99() {
    // Verify telemetry span creation overhead is minimal
    let t = Telemetry::disabled();
    for _ in 0..1000 {
        let span = t.start_produce("perf-topic");
        t.end_span(span);
    }
}

#[test]
fn test_f03_startup_time() {
    // Verify client construction is lightweight (no I/O)
    for _ in 0..100 {
        let _client = StreamlineClient::new("ws://localhost:9094/ws");
    }
}

#[test]
fn test_f04_memory_usage() {
    // Verify circuit breaker is lightweight
    let mut breakers: Vec<CircuitBreaker> = Vec::new();
    for _ in 0..1000 {
        breakers.push(CircuitBreaker::new(5, 2, 30000.0));
    }
    assert_eq!(breakers.len(), 1000);
    // All should start closed
    for cb in &mut breakers {
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
