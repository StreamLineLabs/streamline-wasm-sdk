//! Protocol edge case tests for the Streamline WASM SDK.
//!
//! Tests serialization edge cases, malformed inputs, and boundary conditions
//! for the BrowserMessage and BrowserResponse types.

use wasm_bindgen_test::*;
use streamline_wasm_sdk::{AdminAction, BrowserMessage, BrowserResponse, TopicInfo};

wasm_bindgen_test_configure!(run_in_browser);

// ── BrowserMessage edge cases ────────────────────────────────────────

#[wasm_bindgen_test]
fn produce_with_empty_topic() {
    let msg = BrowserMessage::Produce {
        topic: "".into(),
        key: None,
        value: "data".into(),
    };
    let json = msg.to_json().unwrap();
    assert!(json.contains("\"topic\":\"\""));
}

#[wasm_bindgen_test]
fn produce_with_empty_value() {
    let msg = BrowserMessage::Produce {
        topic: "t".into(),
        key: None,
        value: "".into(),
    };
    let json = msg.to_json().unwrap();
    assert!(json.contains("\"value\":\"\""));
}

#[wasm_bindgen_test]
fn produce_with_unicode_value() {
    let msg = BrowserMessage::Produce {
        topic: "t".into(),
        key: None,
        value: "こんにちは 🌍".into(),
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Produce { value, .. } => assert_eq!(value, "こんにちは 🌍"),
        _ => panic!("expected Produce"),
    }
}

#[wasm_bindgen_test]
fn produce_with_json_value() {
    let nested = r#"{"nested":{"key":"value"},"array":[1,2,3]}"#;
    let msg = BrowserMessage::Produce {
        topic: "t".into(),
        key: None,
        value: nested.into(),
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Produce { value, .. } => assert_eq!(value, nested),
        _ => panic!("expected Produce"),
    }
}

#[wasm_bindgen_test]
fn produce_with_special_chars_in_key() {
    let msg = BrowserMessage::Produce {
        topic: "t".into(),
        key: Some("key/with:special.chars".into()),
        value: "v".into(),
    };
    let json = msg.to_json().unwrap();
    let deser: BrowserMessage = serde_json::from_str(&json).unwrap();
    match deser {
        BrowserMessage::Produce { key, .. } => {
            assert_eq!(key.unwrap(), "key/with:special.chars");
        }
        _ => panic!("expected Produce"),
    }
}

#[wasm_bindgen_test]
fn admin_create_topic_with_zero_partitions() {
    let action = AdminAction::CreateTopic {
        name: "t".into(),
        partitions: Some(0),
    };
    let json = serde_json::to_string(&action).unwrap();
    assert!(json.contains("\"partitions\":0"));
}

#[wasm_bindgen_test]
fn admin_create_topic_with_large_partition_count() {
    let action = AdminAction::CreateTopic {
        name: "t".into(),
        partitions: Some(10000),
    };
    let json = serde_json::to_string(&action).unwrap();
    let deser: AdminAction = serde_json::from_str(&json).unwrap();
    match deser {
        AdminAction::CreateTopic { partitions, .. } => assert_eq!(partitions.unwrap(), 10000),
        _ => panic!("expected CreateTopic"),
    }
}

// ── BrowserResponse edge cases ───────────────────────────────────────

#[wasm_bindgen_test]
fn response_message_with_large_offset() {
    let json = r#"{"type":"message","topic":"t","value":"v","offset":9999999999,"timestamp":0}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Message { offset, .. } => assert_eq!(offset, 9999999999),
        _ => panic!("expected Message"),
    }
}

#[wasm_bindgen_test]
fn response_error_with_zero_code() {
    let json = r#"{"type":"error","code":0,"message":"unknown"}"#;
    let resp = BrowserResponse::from_json(json).unwrap();
    match resp {
        BrowserResponse::Error { code, .. } => assert_eq!(code, 0),
        _ => panic!("expected Error"),
    }
}

#[wasm_bindgen_test]
fn response_empty_string_returns_error() {
    assert!(BrowserResponse::from_json("").is_err());
}

#[wasm_bindgen_test]
fn response_null_returns_error() {
    assert!(BrowserResponse::from_json("null").is_err());
}

#[wasm_bindgen_test]
fn response_array_returns_error() {
    assert!(BrowserResponse::from_json("[1,2,3]").is_err());
}

// ── TopicInfo edge cases ─────────────────────────────────────────────

#[wasm_bindgen_test]
fn topic_info_with_zero_partitions() {
    let info = TopicInfo {
        name: "empty".into(),
        partitions: 0,
    };
    assert_eq!(info.partitions, 0);
}

#[wasm_bindgen_test]
fn topic_info_with_empty_name() {
    let info = TopicInfo {
        name: "".into(),
        partitions: 1,
    };
    assert!(info.name.is_empty());
}

#[wasm_bindgen_test]
fn topic_info_with_unicode_name() {
    let info = TopicInfo {
        name: "topic-日本語".into(),
        partitions: 3,
    };
    let json = serde_json::to_string(&info).unwrap();
    let deser: TopicInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.name, "topic-日本語");
}
