//! Browser integration tests for the Streamline WASM SDK.
//!
//! Run with: `wasm-pack test --headless --chrome`

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use streamline_wasm_sdk::{
    AdminAction, BrowserMessage, BrowserResponse, ConnectionState, StreamlineClient, TopicInfo,
    WsConnection,
};

// ── StreamlineClient construction ────────────────────────────────────

#[wasm_bindgen_test]
fn client_new_is_not_connected() {
    let client = StreamlineClient::new("ws://localhost:9094/ws");
    assert!(!client.is_connected());
}

#[wasm_bindgen_test]
fn client_disconnect_on_fresh_client() {
    let mut client = StreamlineClient::new("ws://localhost:9094/ws");
    client.disconnect();
    assert!(!client.is_connected());
}

// ── Client state transitions (requires browser WebSocket API) ────────

#[wasm_bindgen_test]
#[cfg(target_arch = "wasm32")]
fn client_connect_transitions_away_from_disconnected() {
    let mut client = StreamlineClient::new("ws://localhost:9094/ws");
    // connect() sets state to Connecting before attempting the WebSocket handshake.
    // The handshake won't complete (no server), but the state change is synchronous.
    let _ = client.connect();
    // is_connected() is false because state is Connecting, not Connected
    assert!(!client.is_connected());
}

#[wasm_bindgen_test]
#[cfg(target_arch = "wasm32")]
fn client_disconnect_after_connect_returns_to_disconnected() {
    let mut client = StreamlineClient::new("ws://localhost:9094/ws");
    let _ = client.connect();
    client.disconnect();
    assert!(!client.is_connected());
}

// ── WsConnection state management ───────────────────────────────────

#[wasm_bindgen_test]
fn ws_connection_initial_state_is_disconnected() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert_eq!(conn.state(), ConnectionState::Disconnected);
    assert!(!conn.is_connected());
}

#[wasm_bindgen_test]
fn ws_connection_reconnect_delay_initial() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    assert_eq!(conn.reconnect_delay_ms(), 1_000);
}

#[wasm_bindgen_test]
fn ws_connection_set_max_reconnect_attempts() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    conn.set_max_reconnect_attempts(10);
    // Verify it doesn't panic; max_reconnect_attempts is not publicly readable
    // but the setter should succeed without error.
}

#[wasm_bindgen_test]
fn ws_connection_disconnect_on_fresh() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    conn.disconnect();
    assert_eq!(conn.state(), ConnectionState::Disconnected);
}

#[wasm_bindgen_test]
#[cfg(target_arch = "wasm32")]
fn ws_connection_connect_sets_connecting_state() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    let _ = conn.connect();
    assert_eq!(conn.state(), ConnectionState::Connecting);
}

#[wasm_bindgen_test]
#[cfg(target_arch = "wasm32")]
fn ws_connection_disconnect_after_connect() {
    let mut conn = WsConnection::new("ws://localhost:9094/ws");
    let _ = conn.connect();
    conn.disconnect();
    assert_eq!(conn.state(), ConnectionState::Disconnected);
    assert!(!conn.is_connected());
}

#[wasm_bindgen_test]
fn ws_connection_send_without_connect_fails() {
    let conn = WsConnection::new("ws://localhost:9094/ws");
    let result = conn.send("test message");
    assert!(result.is_err());
}

// ── BrowserMessage serialization/deserialization round-trips ─────────

#[wasm_bindgen_test]
fn browser_message_produce_roundtrip() {
    let msg = BrowserMessage::Produce {
        topic: "events".into(),
        key: Some("user-123".into()),
        value: r#"{"action":"click"}"#.into(),
    };
    let json = msg.to_json().unwrap();
    assert!(json.contains("\"type\":\"produce\""));
    assert!(json.contains("\"topic\":\"events\""));
    assert!(json.contains("\"key\":\"user-123\""));

    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Produce { topic, key, value } => {
            assert_eq!(topic, "events");
            assert_eq!(key.unwrap(), "user-123");
            assert_eq!(value, r#"{"action":"click"}"#);
        }
        _ => panic!("expected Produce variant"),
    }
}

#[wasm_bindgen_test]
fn browser_message_produce_without_key_roundtrip() {
    let msg = BrowserMessage::Produce {
        topic: "metrics".into(),
        key: None,
        value: "42".into(),
    };
    let json = msg.to_json().unwrap();
    assert!(!json.contains("\"key\""), "key should be omitted when None");

    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Produce { key, .. } => assert!(key.is_none()),
        _ => panic!("expected Produce variant"),
    }
}

#[wasm_bindgen_test]
fn browser_message_subscribe_roundtrip() {
    let msg = BrowserMessage::Subscribe {
        topic: "notifications".into(),
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Subscribe { topic } => assert_eq!(topic, "notifications"),
        _ => panic!("expected Subscribe variant"),
    }
}

#[wasm_bindgen_test]
fn browser_message_unsubscribe_roundtrip() {
    let msg = BrowserMessage::Unsubscribe {
        topic: "notifications".into(),
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Unsubscribe { topic } => assert_eq!(topic, "notifications"),
        _ => panic!("expected Unsubscribe variant"),
    }
}

#[wasm_bindgen_test]
fn browser_message_consume_roundtrip() {
    let msg = BrowserMessage::Consume {
        topic: "logs".into(),
        group_id: Some("my-group".into()),
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Consume { topic, group_id } => {
            assert_eq!(topic, "logs");
            assert_eq!(group_id.unwrap(), "my-group");
        }
        _ => panic!("expected Consume variant"),
    }
}

#[wasm_bindgen_test]
fn browser_message_consume_without_group_roundtrip() {
    let msg = BrowserMessage::Consume {
        topic: "logs".into(),
        group_id: None,
    };
    let json = msg.to_json().unwrap();
    assert!(!json.contains("\"group_id\""));
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Consume { group_id, .. } => assert!(group_id.is_none()),
        _ => panic!("expected Consume variant"),
    }
}

#[wasm_bindgen_test]
fn browser_message_admin_create_topic_roundtrip() {
    let msg = BrowserMessage::Admin {
        action: AdminAction::CreateTopic {
            name: "new-topic".into(),
            partitions: Some(8),
        },
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Admin { action } => match action {
            AdminAction::CreateTopic { name, partitions } => {
                assert_eq!(name, "new-topic");
                assert_eq!(partitions.unwrap(), 8);
            }
            _ => panic!("expected CreateTopic"),
        },
        _ => panic!("expected Admin variant"),
    }
}

#[wasm_bindgen_test]
fn browser_message_admin_delete_topic_roundtrip() {
    let msg = BrowserMessage::Admin {
        action: AdminAction::DeleteTopic {
            name: "old-topic".into(),
        },
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Admin { action } => match action {
            AdminAction::DeleteTopic { name } => assert_eq!(name, "old-topic"),
            _ => panic!("expected DeleteTopic"),
        },
        _ => panic!("expected Admin variant"),
    }
}

#[wasm_bindgen_test]
fn browser_message_admin_list_topics_roundtrip() {
    let msg = BrowserMessage::Admin {
        action: AdminAction::ListTopics,
    };
    let json = msg.to_json().unwrap();
    assert!(json.contains("\"action\":\"list_topics\""));
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Admin { action } => match action {
            AdminAction::ListTopics => {}
            _ => panic!("expected ListTopics"),
        },
        _ => panic!("expected Admin variant"),
    }
}

// ── BrowserResponse parsing ──────────────────────────────────────────

#[wasm_bindgen_test]
fn browser_response_parse_message() {
    let json = r#"{"type":"message","topic":"events","key":"k1","value":"v1","offset":100,"timestamp":1700000000}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Message {
            topic,
            key,
            value,
            offset,
            timestamp,
        } => {
            assert_eq!(topic, "events");
            assert_eq!(key.unwrap(), "k1");
            assert_eq!(value, "v1");
            assert_eq!(offset, 100);
            assert_eq!(timestamp, 1700000000);
        }
        _ => panic!("expected Message"),
    }
}

#[wasm_bindgen_test]
fn browser_response_parse_message_without_key() {
    let json = r#"{"type":"message","topic":"t","value":"v","offset":0,"timestamp":0}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Message { key, .. } => assert!(key.is_none()),
        _ => panic!("expected Message"),
    }
}

#[wasm_bindgen_test]
fn browser_response_parse_ack() {
    let json = r#"{"type":"ack","topic":"events","offset":42}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Ack { topic, offset } => {
            assert_eq!(topic.unwrap(), "events");
            assert_eq!(offset.unwrap(), 42);
        }
        _ => panic!("expected Ack"),
    }
}

#[wasm_bindgen_test]
fn browser_response_parse_ack_minimal() {
    let json = r#"{"type":"ack"}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Ack { topic, offset } => {
            assert!(topic.is_none());
            assert!(offset.is_none());
        }
        _ => panic!("expected Ack"),
    }
}

#[wasm_bindgen_test]
fn browser_response_parse_error() {
    let json = r#"{"type":"error","code":503,"message":"service unavailable"}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Error { code, message } => {
            assert_eq!(code, 503);
            assert_eq!(message, "service unavailable");
        }
        _ => panic!("expected Error"),
    }
}

#[wasm_bindgen_test]
fn browser_response_parse_topic_list() {
    let json = r#"{"type":"topic_list","topics":[{"name":"alpha","partitions":3},{"name":"beta","partitions":1}]}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::TopicList { topics } => {
            assert_eq!(topics.len(), 2);
            assert_eq!(topics[0].name, "alpha");
            assert_eq!(topics[0].partitions, 3);
            assert_eq!(topics[1].name, "beta");
            assert_eq!(topics[1].partitions, 1);
        }
        _ => panic!("expected TopicList"),
    }
}

#[wasm_bindgen_test]
fn browser_response_parse_empty_topic_list() {
    let json = r#"{"type":"topic_list","topics":[]}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::TopicList { topics } => assert!(topics.is_empty()),
        _ => panic!("expected TopicList"),
    }
}

#[wasm_bindgen_test]
fn browser_response_invalid_json_returns_error() {
    assert!(BrowserResponse::from_json("not valid json").is_err());
}

#[wasm_bindgen_test]
fn browser_response_unknown_type_returns_error() {
    let json = r#"{"type":"unknown","data":"something"}"#;
    assert!(BrowserResponse::from_json(json).is_err());
}

// ── TopicInfo construction ───────────────────────────────────────────

#[wasm_bindgen_test]
fn topic_info_construction_and_fields() {
    let info = TopicInfo {
        name: "my-topic".into(),
        partitions: 12,
    };
    assert_eq!(info.name, "my-topic");
    assert_eq!(info.partitions, 12);
}

#[wasm_bindgen_test]
fn topic_info_serialization_roundtrip() {
    let info = TopicInfo {
        name: "orders".into(),
        partitions: 6,
    };
    let json = serde_json::to_string(&info).unwrap();
    let deser: TopicInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.name, "orders");
    assert_eq!(deser.partitions, 6);
}

#[wasm_bindgen_test]
fn topic_info_clone() {
    let info = TopicInfo {
        name: "original".into(),
        partitions: 4,
    };
    let cloned = info.clone();
    assert_eq!(cloned.name, "original");
    assert_eq!(cloned.partitions, 4);
}

// ── AdminAction variants ─────────────────────────────────────────────

#[wasm_bindgen_test]
fn admin_action_create_topic_serialization() {
    let action = AdminAction::CreateTopic {
        name: "new-topic".into(),
        partitions: Some(5),
    };
    let json = serde_json::to_string(&action).unwrap();
    assert!(json.contains("\"action\":\"create_topic\""));
    assert!(json.contains("\"name\":\"new-topic\""));
    assert!(json.contains("\"partitions\":5"));
}

#[wasm_bindgen_test]
fn admin_action_create_topic_without_partitions() {
    let action = AdminAction::CreateTopic {
        name: "auto-topic".into(),
        partitions: None,
    };
    let json = serde_json::to_string(&action).unwrap();
    assert!(json.contains("\"action\":\"create_topic\""));
    assert!(!json.contains("\"partitions\""));
}

#[wasm_bindgen_test]
fn admin_action_delete_topic_serialization() {
    let action = AdminAction::DeleteTopic {
        name: "old-topic".into(),
    };
    let json = serde_json::to_string(&action).unwrap();
    assert!(json.contains("\"action\":\"delete_topic\""));
    assert!(json.contains("\"name\":\"old-topic\""));
}

#[wasm_bindgen_test]
fn admin_action_list_topics_serialization() {
    let action = AdminAction::ListTopics;
    let json = serde_json::to_string(&action).unwrap();
    assert!(json.contains("\"action\":\"list_topics\""));
}

#[wasm_bindgen_test]
fn admin_action_clone() {
    let action = AdminAction::CreateTopic {
        name: "cloned".into(),
        partitions: Some(2),
    };
    let cloned = action.clone();
    let json = serde_json::to_string(&cloned).unwrap();
    assert!(json.contains("\"name\":\"cloned\""));
    assert!(json.contains("\"partitions\":2"));
}

#[wasm_bindgen_test]
fn admin_action_roundtrip() {
    let action = AdminAction::DeleteTopic {
        name: "to-delete".into(),
    };
    let json = serde_json::to_string(&action).unwrap();
    let deser: AdminAction = serde_json::from_str(&json).unwrap();
    match deser {
        AdminAction::DeleteTopic { name } => assert_eq!(name, "to-delete"),
        _ => panic!("expected DeleteTopic"),
    }
}

// ── Consumer offset tracking ─────────────────────────────────────────

#[wasm_bindgen_test]
fn consumer_initial_offset_is_zero() {
    let consumer = streamline_wasm_sdk::Consumer::new("ws://localhost:9094/ws", "test-topic");
    assert_eq!(consumer.current_offset(), 0);
    assert_eq!(consumer.committed_offset(), -1);
}

#[wasm_bindgen_test]
fn consumer_advance_offset() {
    let mut consumer = streamline_wasm_sdk::Consumer::new("ws://localhost:9094/ws", "test-topic");
    consumer.advance_offset(5);
    assert_eq!(consumer.current_offset(), 6); // offset + 1

    // Advancing to a lower offset should be ignored
    consumer.advance_offset(3);
    assert_eq!(consumer.current_offset(), 6);
}

#[wasm_bindgen_test]
fn consumer_advance_offset_sequential() {
    let mut consumer = streamline_wasm_sdk::Consumer::new("ws://localhost:9094/ws", "events");
    consumer.advance_offset(0);
    assert_eq!(consumer.current_offset(), 1);
    consumer.advance_offset(1);
    assert_eq!(consumer.current_offset(), 2);
    consumer.advance_offset(2);
    assert_eq!(consumer.current_offset(), 3);
}

#[wasm_bindgen_test]
fn consumer_commit_requires_connection() {
    let mut consumer = streamline_wasm_sdk::Consumer::new("ws://localhost:9094/ws", "test-topic");
    // Not connected, commit should fail
    let result = consumer.commit();
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn consumer_seek_requires_connection() {
    let mut consumer = streamline_wasm_sdk::Consumer::new("ws://localhost:9094/ws", "test-topic");
    let result = consumer.seek(10);
    assert!(result.is_err());
}
