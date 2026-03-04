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
}

