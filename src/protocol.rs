use serde::{Deserialize, Serialize};

/// Messages sent from the browser client to the Streamline server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserMessage {
    Produce {
        topic: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        value: String,
    },
    Consume {
        topic: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        group_id: Option<String>,
    },
    Subscribe {
        topic: String,
    },
    Unsubscribe {
        topic: String,
    },
    Admin {
        action: AdminAction,
    },
}

/// Administrative actions for topic management.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AdminAction {
    CreateTopic {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        partitions: Option<u32>,
    },
    DeleteTopic {
        name: String,
    },
    ListTopics,
}

/// Responses received from the Streamline server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserResponse {
    Message {
        topic: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        value: String,
        offset: u64,
        timestamp: u64,
    },
    Ack {
        #[serde(skip_serializing_if = "Option::is_none")]
        topic: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        offset: Option<u64>,
    },
    TopicList {
        topics: Vec<TopicInfo>,
    },
    Error {
        code: u32,
        message: String,
    },
}

/// Metadata about a topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
    pub name: String,
    pub partitions: u32,
}

impl BrowserMessage {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl BrowserResponse {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── BrowserMessage serialization ─────────────────────────────────

    #[test]
    fn test_produce_with_key() {
        let msg = BrowserMessage::Produce {
            topic: "test-topic".into(),
            key: Some("key-1".into()),
            value: "hello".into(),
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"produce\""));
        assert!(json.contains("\"topic\":\"test-topic\""));
        assert!(json.contains("\"key\":\"key-1\""));
        assert!(json.contains("\"value\":\"hello\""));
    }

    #[test]
    fn test_produce_without_key_omits_field() {
        let msg = BrowserMessage::Produce {
            topic: "t".into(),
            key: None,
            value: "v".into(),
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"produce\""));
        assert!(!json.contains("\"key\""), "key should be skipped when None");
    }

    #[test]
    fn test_consume_with_group_id() {
        let msg = BrowserMessage::Consume {
            topic: "events".into(),
            group_id: Some("grp-1".into()),
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"consume\""));
        assert!(json.contains("\"topic\":\"events\""));
        assert!(json.contains("\"group_id\":\"grp-1\""));
    }

    #[test]
    fn test_consume_without_group_id() {
        let msg = BrowserMessage::Consume {
            topic: "events".into(),
            group_id: None,
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"consume\""));
        assert!(!json.contains("\"group_id\""));
    }

    #[test]
    fn test_subscribe_serialization() {
        let msg = BrowserMessage::Subscribe {
            topic: "logs".into(),
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"subscribe\""));
        assert!(json.contains("\"topic\":\"logs\""));
    }

    #[test]
    fn test_unsubscribe_serialization() {
        let msg = BrowserMessage::Unsubscribe {
            topic: "logs".into(),
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"unsubscribe\""));
        assert!(json.contains("\"topic\":\"logs\""));
    }

    #[test]
    fn test_admin_create_topic_with_partitions() {
        let msg = BrowserMessage::Admin {
            action: AdminAction::CreateTopic {
                name: "new-topic".into(),
                partitions: Some(3),
            },
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"admin\""));
        assert!(json.contains("\"action\":\"create_topic\""));
        assert!(json.contains("\"name\":\"new-topic\""));
        assert!(json.contains("\"partitions\":3"));
    }

    #[test]
    fn test_admin_create_topic_without_partitions() {
        let msg = BrowserMessage::Admin {
            action: AdminAction::CreateTopic {
                name: "t".into(),
                partitions: None,
            },
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"action\":\"create_topic\""));
        assert!(!json.contains("\"partitions\""));
    }

    #[test]
    fn test_admin_delete_topic() {
        let msg = BrowserMessage::Admin {
            action: AdminAction::DeleteTopic {
                name: "old-topic".into(),
            },
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"admin\""));
        assert!(json.contains("\"action\":\"delete_topic\""));
        assert!(json.contains("\"name\":\"old-topic\""));
    }

    #[test]
    fn test_admin_list_topics() {
        let msg = BrowserMessage::Admin {
            action: AdminAction::ListTopics,
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"type\":\"admin\""));
        assert!(json.contains("\"action\":\"list_topics\""));
    }

    // ── BrowserMessage round-trip ────────────────────────────────────

    #[test]
    fn test_produce_roundtrip() {
        let msg = BrowserMessage::Produce {
            topic: "rt".into(),
            key: Some("k".into()),
            value: "v".into(),
        };
        let json = msg.to_json().unwrap();
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Produce { topic, key, value } => {
                assert_eq!(topic, "rt");
                assert_eq!(key.unwrap(), "k");
                assert_eq!(value, "v");
            }
            _ => panic!("expected Produce"),
        }
    }

    #[test]
    fn test_subscribe_roundtrip() {
        let msg = BrowserMessage::Subscribe {
            topic: "sub-rt".into(),
        };
        let json = msg.to_json().unwrap();
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Subscribe { topic } => assert_eq!(topic, "sub-rt"),
            _ => panic!("expected Subscribe"),
        }
    }

    #[test]
    fn test_consume_roundtrip() {
        let msg = BrowserMessage::Consume {
            topic: "cons-rt".into(),
            group_id: Some("g".into()),
        };
        let json = msg.to_json().unwrap();
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Consume { topic, group_id } => {
                assert_eq!(topic, "cons-rt");
                assert_eq!(group_id.unwrap(), "g");
            }
            _ => panic!("expected Consume"),
        }
    }

    #[test]
    fn test_admin_roundtrip() {
        let msg = BrowserMessage::Admin {
            action: AdminAction::CreateTopic {
                name: "art".into(),
                partitions: Some(6),
            },
        };
        let json = msg.to_json().unwrap();
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Admin { action } => match action {
                AdminAction::CreateTopic { name, partitions } => {
                    assert_eq!(name, "art");
                    assert_eq!(partitions.unwrap(), 6);
                }
                _ => panic!("expected CreateTopic"),
            },
            _ => panic!("expected Admin"),
        }
    }

    // ── BrowserResponse deserialization ──────────────────────────────

    #[test]
    fn test_ack_response() {
        let json = r#"{"type":"ack","topic":"test","offset":42}"#;
        let resp = BrowserResponse::from_json(json).unwrap();
        match resp {
            BrowserResponse::Ack { topic, offset } => {
                assert_eq!(topic.unwrap(), "test");
                assert_eq!(offset.unwrap(), 42);
            }
            _ => panic!("expected Ack"),
        }
    }

    #[test]
    fn test_ack_response_minimal() {
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

    #[test]
    fn test_error_response() {
        let json = r#"{"type":"error","code":404,"message":"topic not found"}"#;
        let resp = BrowserResponse::from_json(json).unwrap();
        match resp {
            BrowserResponse::Error { code, message } => {
                assert_eq!(code, 404);
                assert_eq!(message, "topic not found");
            }
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn test_message_response() {
        let json = r#"{"type":"message","topic":"t","key":"k","value":"v","offset":10,"timestamp":1700000000}"#;
        let resp = BrowserResponse::from_json(json).unwrap();
        match resp {
            BrowserResponse::Message {
                topic,
                key,
                value,
                offset,
                timestamp,
            } => {
                assert_eq!(topic, "t");
                assert_eq!(key.unwrap(), "k");
                assert_eq!(value, "v");
                assert_eq!(offset, 10);
                assert_eq!(timestamp, 1700000000);
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_message_response_without_key() {
        let json = r#"{"type":"message","topic":"t","value":"v","offset":0,"timestamp":0}"#;
        let resp = BrowserResponse::from_json(json).unwrap();
        match resp {
            BrowserResponse::Message { key, .. } => assert!(key.is_none()),
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_topic_list_response() {
        let json = r#"{"type":"topic_list","topics":[{"name":"a","partitions":1},{"name":"b","partitions":4}]}"#;
        let resp = BrowserResponse::from_json(json).unwrap();
        match resp {
            BrowserResponse::TopicList { topics } => {
                assert_eq!(topics.len(), 2);
                assert_eq!(topics[0].name, "a");
                assert_eq!(topics[0].partitions, 1);
                assert_eq!(topics[1].name, "b");
                assert_eq!(topics[1].partitions, 4);
            }
            _ => panic!("expected TopicList"),
        }
    }

    #[test]
    fn test_topic_list_empty() {
        let json = r#"{"type":"topic_list","topics":[]}"#;
        let resp = BrowserResponse::from_json(json).unwrap();
        match resp {
            BrowserResponse::TopicList { topics } => assert!(topics.is_empty()),
            _ => panic!("expected TopicList"),
        }
    }

    // ── BrowserResponse round-trip ───────────────────────────────────

    #[test]
    fn test_response_message_roundtrip() {
        let resp = BrowserResponse::Message {
            topic: "rt".into(),
            key: Some("rk".into()),
            value: "rv".into(),
            offset: 7,
            timestamp: 123456,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deser = BrowserResponse::from_json(&json).unwrap();
        match deser {
            BrowserResponse::Message {
                topic,
                key,
                value,
                offset,
                timestamp,
            } => {
                assert_eq!(topic, "rt");
                assert_eq!(key.unwrap(), "rk");
                assert_eq!(value, "rv");
                assert_eq!(offset, 7);
                assert_eq!(timestamp, 123456);
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_response_error_roundtrip() {
        let resp = BrowserResponse::Error {
            code: 500,
            message: "internal".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deser = BrowserResponse::from_json(&json).unwrap();
        match deser {
            BrowserResponse::Error { code, message } => {
                assert_eq!(code, 500);
                assert_eq!(message, "internal");
            }
            _ => panic!("expected Error"),
        }
    }

    // ── TopicInfo ────────────────────────────────────────────────────

    #[test]
    fn test_topic_info_serialization() {
        let info = TopicInfo {
            name: "my-topic".into(),
            partitions: 12,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"my-topic\""));
        assert!(json.contains("\"partitions\":12"));
    }

    #[test]
    fn test_topic_info_roundtrip() {
        let info = TopicInfo {
            name: "x".into(),
            partitions: 1,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deser: TopicInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.name, "x");
        assert_eq!(deser.partitions, 1);
    }

    // ── Edge / negative cases ────────────────────────────────────────

    #[test]
    fn test_invalid_json_returns_error() {
        assert!(BrowserResponse::from_json("not json").is_err());
    }

    #[test]
    fn test_unknown_type_returns_error() {
        let json = r#"{"type":"unknown_variant","foo":"bar"}"#;
        assert!(BrowserResponse::from_json(json).is_err());
    }

    // ── MessageFormat ────────────────────────────────────────────────

    #[test]
    fn test_message_format_from_magic_legacy_v0() {
        assert_eq!(MessageFormat::from_magic(0), Some(MessageFormat::Legacy));
    }

    #[test]
    fn test_message_format_from_magic_legacy_v1() {
        assert_eq!(MessageFormat::from_magic(1), Some(MessageFormat::Legacy));
    }

    #[test]
    fn test_message_format_from_magic_record_batch() {
        assert_eq!(
            MessageFormat::from_magic(2),
            Some(MessageFormat::RecordBatch)
        );
    }

    #[test]
    fn test_message_format_from_magic_unknown() {
        assert_eq!(MessageFormat::from_magic(3), None);
        assert_eq!(MessageFormat::from_magic(255), None);
    }

    #[test]
    fn test_message_format_equality() {
        assert_eq!(MessageFormat::Legacy, MessageFormat::Legacy);
        assert_eq!(MessageFormat::RecordBatch, MessageFormat::RecordBatch);
        assert_ne!(MessageFormat::Legacy, MessageFormat::RecordBatch);
    }

    #[test]
    // Intentionally exercises both the derived `Copy` and `Clone` impls.
    #[allow(clippy::clone_on_copy)]
    fn test_message_format_clone_copy() {
        let fmt = MessageFormat::RecordBatch;
        let copied = fmt;
        let cloned = fmt.clone();
        assert_eq!(fmt, copied);
        assert_eq!(fmt, cloned);
    }

    #[test]
    fn test_message_format_debug() {
        assert_eq!(format!("{:?}", MessageFormat::Legacy), "Legacy");
        assert_eq!(format!("{:?}", MessageFormat::RecordBatch), "RecordBatch");
    }

    // ── Additional round-trip tests ──────────────────────────────────

    #[test]
    fn test_unsubscribe_roundtrip() {
        let msg = BrowserMessage::Unsubscribe {
            topic: "unsub-rt".into(),
        };
        let json = msg.to_json().unwrap();
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Unsubscribe { topic } => assert_eq!(topic, "unsub-rt"),
            _ => panic!("expected Unsubscribe"),
        }
    }

    #[test]
    fn test_admin_list_topics_roundtrip() {
        let msg = BrowserMessage::Admin {
            action: AdminAction::ListTopics,
        };
        let json = msg.to_json().unwrap();
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Admin { action } => match action {
                AdminAction::ListTopics => {}
                _ => panic!("expected ListTopics"),
            },
            _ => panic!("expected Admin"),
        }
    }

    #[test]
    fn test_admin_delete_topic_roundtrip() {
        let msg = BrowserMessage::Admin {
            action: AdminAction::DeleteTopic {
                name: "del-rt".into(),
            },
        };
        let json = msg.to_json().unwrap();
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Admin { action } => match action {
                AdminAction::DeleteTopic { name } => assert_eq!(name, "del-rt"),
                _ => panic!("expected DeleteTopic"),
            },
            _ => panic!("expected Admin"),
        }
    }

    #[test]
    fn test_response_ack_roundtrip() {
        let resp = BrowserResponse::Ack {
            topic: Some("ack-rt".into()),
            offset: Some(99),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deser = BrowserResponse::from_json(&json).unwrap();
        match deser {
            BrowserResponse::Ack { topic, offset } => {
                assert_eq!(topic.unwrap(), "ack-rt");
                assert_eq!(offset.unwrap(), 99);
            }
            _ => panic!("expected Ack"),
        }
    }

    #[test]
    fn test_response_topic_list_roundtrip() {
        let resp = BrowserResponse::TopicList {
            topics: vec![
                TopicInfo {
                    name: "a".into(),
                    partitions: 1,
                },
                TopicInfo {
                    name: "b".into(),
                    partitions: 8,
                },
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deser = BrowserResponse::from_json(&json).unwrap();
        match deser {
            BrowserResponse::TopicList { topics } => {
                assert_eq!(topics.len(), 2);
                assert_eq!(topics[0].name, "a");
                assert_eq!(topics[1].partitions, 8);
            }
            _ => panic!("expected TopicList"),
        }
    }

    // ── Additional edge cases ────────────────────────────────────────

    #[test]
    fn test_produce_with_empty_key() {
        let msg = BrowserMessage::Produce {
            topic: "t".into(),
            key: Some("".into()),
            value: "v".into(),
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("\"key\":\"\""));
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Produce { key, .. } => assert_eq!(key.unwrap(), ""),
            _ => panic!("expected Produce"),
        }
    }

    #[test]
    fn test_consume_without_group_roundtrip() {
        let msg = BrowserMessage::Consume {
            topic: "c-rt".into(),
            group_id: None,
        };
        let json = msg.to_json().unwrap();
        let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
        match deser {
            BrowserMessage::Consume { topic, group_id } => {
                assert_eq!(topic, "c-rt");
                assert!(group_id.is_none());
            }
            _ => panic!("expected Consume"),
        }
    }

    #[test]
    fn test_topic_info_debug_format() {
        let info = TopicInfo {
            name: "debug".into(),
            partitions: 2,
        };
        let dbg = format!("{:?}", info);
        assert!(dbg.contains("debug"));
        assert!(dbg.contains("2"));
    }

    #[test]
    fn test_admin_action_debug_format() {
        let action = AdminAction::CreateTopic {
            name: "dbg".into(),
            partitions: Some(1),
        };
        let dbg = format!("{:?}", action);
        assert!(dbg.contains("CreateTopic"));
        assert!(dbg.contains("dbg"));
    }

    #[test]
    fn test_browser_message_debug_format() {
        let msg = BrowserMessage::Subscribe { topic: "t".into() };
        let dbg = format!("{:?}", msg);
        assert!(dbg.contains("Subscribe"));
    }

    #[test]
    fn test_browser_response_debug_format() {
        let resp = BrowserResponse::Error {
            code: 400,
            message: "bad request".into(),
        };
        let dbg = format!("{:?}", resp);
        assert!(dbg.contains("Error"));
        assert!(dbg.contains("400"));
    }

    #[test]
    fn test_missing_required_field_returns_error() {
        // Missing "value" in produce
        let json = r#"{"type":"produce","topic":"t"}"#;
        assert!(serde_json::from_str::<BrowserMessage>(json).is_err());
    }

    #[test]
    fn test_topic_info_many_partitions() {
        let info = TopicInfo {
            name: "big".into(),
            partitions: u32::MAX,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deser: TopicInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.partitions, u32::MAX);
    }

    #[test]
    fn test_response_message_with_zero_offsets() {
        let json = r#"{"type":"message","topic":"t","value":"v","offset":0,"timestamp":0}"#;
        let resp = BrowserResponse::from_json(json).unwrap();
        match resp {
            BrowserResponse::Message {
                offset, timestamp, ..
            } => {
                assert_eq!(offset, 0);
                assert_eq!(timestamp, 0);
            }
            _ => panic!("expected Message"),
        }
    }
}

/// Wire protocol message format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MessageFormat {
    /// Legacy message format (v0/v1)
    Legacy,
    /// Record batch format (v2+)
    RecordBatch,
}

impl MessageFormat {
    /// Returns the format for a given magic byte value.
    #[allow(dead_code)]
    pub fn from_magic(magic: u8) -> Option<Self> {
        match magic {
            0 | 1 => Some(Self::Legacy),
            2 => Some(Self::RecordBatch),
            _ => None,
        }
    }
}
