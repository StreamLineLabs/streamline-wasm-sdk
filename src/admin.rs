//! HTTP-based Admin and Query client for browser environments.
//!
//! Provides full topic management, consumer group inspection, and SQL query
//! execution via the Streamline HTTP API (port 9094).
//!
//! # Example
//!
//! ```javascript
//! import { AdminClient, QueryClient } from '@streamlinelabs/streamline-wasm-sdk';
//!
//! const admin = new AdminClient("http://localhost:9094");
//!
//! // Topic management
//! const topics = await admin.list_topics();
//! await admin.create_topic("orders", 3);
//! const info = await admin.describe_topic("orders");
//! await admin.delete_topic("orders");
//!
//! // Consumer groups
//! const groups = await admin.list_consumer_groups();
//! const detail = await admin.describe_consumer_group("my-group");
//!
//! // SQL queries
//! const query = new QueryClient("http://localhost:9094");
//! const result = await query.execute("SELECT * FROM orders LIMIT 10");
//! ```

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

use crate::error::StreamlineError;

/// Topic information returned by admin operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminTopicInfo {
    pub name: String,
    pub partitions: u32,
    #[serde(default)]
    pub replication_factor: u32,
    #[serde(default)]
    pub message_count: u64,
}

/// Detailed topic description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicDescription {
    pub name: String,
    pub partitions: Vec<PartitionInfo>,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Per-partition information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub id: u32,
    #[serde(default)]
    pub leader: i32,
    #[serde(default)]
    pub replicas: Vec<i32>,
    #[serde(default)]
    pub isr: Vec<i32>,
    #[serde(default)]
    pub start_offset: u64,
    #[serde(default)]
    pub end_offset: u64,
}

/// Consumer group overview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupInfo {
    pub group_id: String,
    pub state: String,
    #[serde(default)]
    pub members: u32,
}

/// Detailed consumer group description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupDescription {
    pub group_id: String,
    pub state: String,
    #[serde(default)]
    pub protocol_type: String,
    #[serde(default)]
    pub members: Vec<ConsumerGroupMember>,
}

/// Consumer group member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupMember {
    pub member_id: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub assignments: Vec<String>,
}

/// SQL query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<serde_json::Value>>,
    #[serde(default)]
    pub row_count: u64,
}

/// Server health and info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub uptime_seconds: u64,
    #[serde(default)]
    pub topics: u32,
    #[serde(default)]
    pub messages_total: u64,
}

/// HTTP-based admin client for Streamline topic and consumer group management.
#[wasm_bindgen]
pub struct AdminClient {
    base_url: String,
    auth_token: Option<String>,
}

#[wasm_bindgen]
impl AdminClient {
    /// Create a new admin client pointing to the Streamline HTTP API.
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token: None,
        }
    }

    /// Set an authentication token for API requests.
    pub fn set_auth_token(&mut self, token: &str) {
        self.auth_token = Some(token.to_string());
    }

    /// List all topics.
    pub async fn list_topics(&self) -> Result<JsValue, JsValue> {
        let resp = self
            .http_get(&format!("{}/api/topics", self.base_url))
            .await?;
        let topics: Vec<AdminTopicInfo> = serde_json::from_str(&resp)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&topics).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get detailed description of a topic.
    pub async fn describe_topic(&self, name: &str) -> Result<JsValue, JsValue> {
        let url = format!("{}/api/topics/{}", self.base_url, encode(name));
        let resp = self.http_get(&url).await?;
        let desc: TopicDescription = serde_json::from_str(&resp)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&desc).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create a topic with the given number of partitions.
    pub async fn create_topic(&self, name: &str, partitions: u32) -> Result<(), JsValue> {
        crate::validation::validate_topic_name(name)
            .map_err(|e| StreamlineError::configuration(&e))?;
        let url = format!("{}/api/topics", self.base_url);
        let body = serde_json::json!({
            "name": name,
            "partitions": partitions
        });
        self.http_post(&url, &body.to_string()).await?;
        Ok(())
    }

    /// Delete a topic by name.
    pub async fn delete_topic(&self, name: &str) -> Result<(), JsValue> {
        crate::validation::validate_topic_name(name)
            .map_err(|e| StreamlineError::configuration(&e))?;
        let url = format!("{}/api/topics/{}", self.base_url, encode(name));
        self.http_delete(&url).await?;
        Ok(())
    }

    /// List all consumer groups.
    pub async fn list_consumer_groups(&self) -> Result<JsValue, JsValue> {
        let url = format!("{}/api/consumer-groups", self.base_url);
        let resp = self.http_get(&url).await?;
        let groups: Vec<ConsumerGroupInfo> = serde_json::from_str(&resp)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&groups).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get detailed description of a consumer group.
    pub async fn describe_consumer_group(&self, group_id: &str) -> Result<JsValue, JsValue> {
        let url = format!("{}/api/consumer-groups/{}", self.base_url, encode(group_id));
        let resp = self.http_get(&url).await?;
        let desc: ConsumerGroupDescription = serde_json::from_str(&resp)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&desc).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Delete a consumer group.
    pub async fn delete_consumer_group(&self, group_id: &str) -> Result<(), JsValue> {
        let url = format!("{}/api/consumer-groups/{}", self.base_url, encode(group_id));
        self.http_delete(&url).await?;
        Ok(())
    }

    /// Get server health status.
    pub async fn health(&self) -> Result<bool, JsValue> {
        let url = format!("{}/health", self.base_url);
        match self.http_get(&url).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get server information.
    pub async fn server_info(&self) -> Result<JsValue, JsValue> {
        let url = format!("{}/api/info", self.base_url);
        let resp = self.http_get(&url).await?;
        let info: ServerInfo = serde_json::from_str(&resp)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&info).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// HTTP-based query client for executing SQL queries against Streamline topics.
#[wasm_bindgen]
pub struct QueryClient {
    base_url: String,
    auth_token: Option<String>,
}

#[wasm_bindgen]
impl QueryClient {
    /// Create a new query client.
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token: None,
        }
    }

    /// Set an authentication token for API requests.
    pub fn set_auth_token(&mut self, token: &str) {
        self.auth_token = Some(token.to_string());
    }

    /// Execute a SQL query against stream data.
    ///
    /// Returns a `QueryResult` with columns, rows, and row count.
    pub async fn execute(&self, sql: &str) -> Result<JsValue, JsValue> {
        let url = format!("{}/api/query", self.base_url);
        let body = serde_json::json!({ "query": sql });
        let resp = self.http_post(&url, &body.to_string()).await?;
        let result: QueryResult = serde_json::from_str(&resp)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Execute a query and return the result as a JSON string.
    pub async fn execute_raw(&self, sql: &str) -> Result<String, JsValue> {
        let url = format!("{}/api/query", self.base_url);
        let body = serde_json::json!({ "query": sql });
        self.http_post(&url, &body.to_string()).await
    }
}

// Shared HTTP helpers
macro_rules! impl_http_methods {
    ($t:ty) => {
        impl $t {
            #[allow(dead_code)]
            async fn http_get(&self, url: &str) -> Result<String, JsValue> {
                let opts = RequestInit::new();
                opts.set_method("GET");
                opts.set_mode(RequestMode::Cors);

                let request = Request::new_with_str_and_init(url, &opts)?;
                self.set_headers(&request)?;

                let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
                let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
                let resp: Response = resp_value.dyn_into()?;

                if !resp.ok() {
                    return Err(JsValue::from_str(&format!(
                        "HTTP {}: {}",
                        resp.status(),
                        resp.status_text()
                    )));
                }

                let text = JsFuture::from(resp.text()?).await?;
                text.as_string()
                    .ok_or_else(|| JsValue::from_str("response is not a string"))
            }

            async fn http_post(&self, url: &str, body: &str) -> Result<String, JsValue> {
                let opts = RequestInit::new();
                opts.set_method("POST");
                opts.set_mode(RequestMode::Cors);
                opts.set_body(&JsValue::from_str(body));

                let request = Request::new_with_str_and_init(url, &opts)?;
                self.set_headers(&request)?;
                request.headers().set("Content-Type", "application/json")?;

                let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
                let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
                let resp: Response = resp_value.dyn_into()?;

                if !resp.ok() {
                    return Err(JsValue::from_str(&format!(
                        "HTTP {}: {}",
                        resp.status(),
                        resp.status_text()
                    )));
                }

                let text = JsFuture::from(resp.text()?).await?;
                text.as_string()
                    .ok_or_else(|| JsValue::from_str("response is not a string"))
            }

            #[allow(dead_code)]
            async fn http_delete(&self, url: &str) -> Result<String, JsValue> {
                let opts = RequestInit::new();
                opts.set_method("DELETE");
                opts.set_mode(RequestMode::Cors);

                let request = Request::new_with_str_and_init(url, &opts)?;
                self.set_headers(&request)?;

                let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
                let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
                let resp: Response = resp_value.dyn_into()?;

                if !resp.ok() {
                    return Err(JsValue::from_str(&format!(
                        "HTTP {}: {}",
                        resp.status(),
                        resp.status_text()
                    )));
                }

                let text = JsFuture::from(resp.text()?).await?;
                text.as_string()
                    .ok_or_else(|| JsValue::from_str("response is not a string"))
            }

            fn set_headers(&self, request: &Request) -> Result<(), JsValue> {
                request.headers().set("Accept", "application/json")?;
                if let Some(ref token) = self.auth_token {
                    request
                        .headers()
                        .set("Authorization", &format!("Bearer {}", token))?;
                }
                Ok(())
            }
        }
    };
}

impl_http_methods!(AdminClient);
impl_http_methods!(QueryClient);

fn encode(s: &str) -> String {
    js_sys::encode_uri_component(s).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AdminTopicInfo ───────────────────────────────────────────────

    #[test]
    fn test_admin_topic_info_serialization() {
        let info = AdminTopicInfo {
            name: "orders".into(),
            partitions: 3,
            replication_factor: 1,
            message_count: 100,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"orders\""));
        assert!(json.contains("\"partitions\":3"));
    }

    #[test]
    fn test_admin_topic_info_deserialization_minimal() {
        let json = r#"{"name":"test","partitions":1}"#;
        let info: AdminTopicInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "test");
        assert_eq!(info.partitions, 1);
        assert_eq!(info.replication_factor, 0);
        assert_eq!(info.message_count, 0);
    }

    // ── TopicDescription ─────────────────────────────────────────────

    #[test]
    fn test_topic_description_deserialization() {
        let json = r#"{"name":"logs","partitions":[{"id":0,"leader":1,"replicas":[1],"isr":[1],"start_offset":0,"end_offset":50}],"config":{}}"#;
        let desc: TopicDescription = serde_json::from_str(json).unwrap();
        assert_eq!(desc.name, "logs");
        assert_eq!(desc.partitions.len(), 1);
        assert_eq!(desc.partitions[0].end_offset, 50);
    }

    // ── ConsumerGroupInfo ────────────────────────────────────────────

    #[test]
    fn test_consumer_group_info() {
        let json = r#"{"group_id":"grp-1","state":"Stable","members":3}"#;
        let info: ConsumerGroupInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.group_id, "grp-1");
        assert_eq!(info.state, "Stable");
        assert_eq!(info.members, 3);
    }

    // ── ConsumerGroupDescription ─────────────────────────────────────

    #[test]
    fn test_consumer_group_description() {
        let json = r#"{"group_id":"grp-1","state":"Stable","protocol_type":"consumer","members":[{"member_id":"m-1","client_id":"cli-1","host":"127.0.0.1","assignments":["topic-0"]}]}"#;
        let desc: ConsumerGroupDescription = serde_json::from_str(json).unwrap();
        assert_eq!(desc.members.len(), 1);
        assert_eq!(desc.members[0].member_id, "m-1");
    }

    // ── QueryResult ──────────────────────────────────────────────────

    #[test]
    fn test_query_result_deserialization() {
        let json = r#"{"columns":["id","name"],"rows":[[1,"alice"],[2,"bob"]],"row_count":2}"#;
        let result: QueryResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.columns, vec!["id", "name"]);
        assert_eq!(result.row_count, 2);
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_query_result_empty() {
        let json = r#"{"columns":[],"rows":[],"row_count":0}"#;
        let result: QueryResult = serde_json::from_str(json).unwrap();
        assert!(result.columns.is_empty());
        assert!(result.rows.is_empty());
    }

    // ── ServerInfo ───────────────────────────────────────────────────

    #[test]
    fn test_server_info_deserialization() {
        let json = r#"{"version":"0.2.0","uptime_seconds":3600,"topics":5,"messages_total":10000}"#;
        let info: ServerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.version, "0.2.0");
        assert_eq!(info.uptime_seconds, 3600);
        assert_eq!(info.topics, 5);
    }

    // ── Client construction ──────────────────────────────────────────

    #[test]
    fn test_admin_client_construction() {
        let client = AdminClient::new("http://localhost:9094");
        assert_eq!(client.base_url, "http://localhost:9094");
        assert!(client.auth_token.is_none());
    }

    #[test]
    fn test_admin_client_trailing_slash() {
        let client = AdminClient::new("http://localhost:9094/");
        assert_eq!(client.base_url, "http://localhost:9094");
    }

    #[test]
    fn test_admin_client_auth_token() {
        let mut client = AdminClient::new("http://localhost:9094");
        client.set_auth_token("secret");
        assert_eq!(client.auth_token.as_deref(), Some("secret"));
    }

    #[test]
    fn test_query_client_construction() {
        let client = QueryClient::new("http://localhost:9094");
        assert_eq!(client.base_url, "http://localhost:9094");
    }

    #[test]
    fn test_query_client_auth_token() {
        let mut client = QueryClient::new("http://localhost:9094");
        client.set_auth_token("token");
        assert_eq!(client.auth_token.as_deref(), Some("token"));
    }

    // ── URL construction ─────────────────────────────────────────────

    #[test]
    fn test_admin_client_multiple_trailing_slashes() {
        let client = AdminClient::new("http://localhost:9094///");
        // trim_end_matches removes all trailing slashes
        assert!(!client.base_url.ends_with('/'));
    }

    #[test]
    fn test_query_client_trailing_slash() {
        let client = QueryClient::new("http://localhost:9094/");
        assert_eq!(client.base_url, "http://localhost:9094");
    }

    #[test]
    fn test_admin_client_with_path() {
        let client = AdminClient::new("http://localhost:9094/api/v2");
        assert_eq!(client.base_url, "http://localhost:9094/api/v2");
    }

    // ── PartitionInfo defaults ───────────────────────────────────────

    #[test]
    fn test_partition_info_minimal_deserialization() {
        let json = r#"{"id":0}"#;
        let info: PartitionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, 0);
        assert_eq!(info.leader, 0);
        assert!(info.replicas.is_empty());
        assert!(info.isr.is_empty());
        assert_eq!(info.start_offset, 0);
        assert_eq!(info.end_offset, 0);
    }

    #[test]
    fn test_partition_info_full_deserialization() {
        let json = r#"{"id":2,"leader":1,"replicas":[1,2,3],"isr":[1,2],"start_offset":100,"end_offset":500}"#;
        let info: PartitionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, 2);
        assert_eq!(info.leader, 1);
        assert_eq!(info.replicas, vec![1, 2, 3]);
        assert_eq!(info.isr, vec![1, 2]);
        assert_eq!(info.start_offset, 100);
        assert_eq!(info.end_offset, 500);
    }

    #[test]
    fn test_partition_info_roundtrip() {
        let info = PartitionInfo {
            id: 5,
            leader: 2,
            replicas: vec![1, 2],
            isr: vec![1],
            start_offset: 0,
            end_offset: 1000,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deser: PartitionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.id, info.id);
        assert_eq!(deser.end_offset, 1000);
    }

    // ── ConsumerGroupMember defaults ─────────────────────────────────

    #[test]
    fn test_consumer_group_member_minimal() {
        let json = r#"{"member_id":"m-1"}"#;
        let member: ConsumerGroupMember = serde_json::from_str(json).unwrap();
        assert_eq!(member.member_id, "m-1");
        assert_eq!(member.client_id, "");
        assert_eq!(member.host, "");
        assert!(member.assignments.is_empty());
    }

    #[test]
    fn test_consumer_group_member_roundtrip() {
        let member = ConsumerGroupMember {
            member_id: "m-2".into(),
            client_id: "client-1".into(),
            host: "192.168.1.1".into(),
            assignments: vec!["topic-0".into(), "topic-1".into()],
        };
        let json = serde_json::to_string(&member).unwrap();
        let deser: ConsumerGroupMember = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.member_id, "m-2");
        assert_eq!(deser.assignments.len(), 2);
    }

    // ── ConsumerGroupDescription empty members ───────────────────────

    #[test]
    fn test_consumer_group_description_empty_members() {
        let json = r#"{"group_id":"empty-grp","state":"Empty","protocol_type":"","members":[]}"#;
        let desc: ConsumerGroupDescription = serde_json::from_str(json).unwrap();
        assert_eq!(desc.group_id, "empty-grp");
        assert_eq!(desc.state, "Empty");
        assert!(desc.members.is_empty());
    }

    #[test]
    fn test_consumer_group_description_minimal() {
        let json = r#"{"group_id":"g","state":"Stable"}"#;
        let desc: ConsumerGroupDescription = serde_json::from_str(json).unwrap();
        assert_eq!(desc.group_id, "g");
        assert!(desc.members.is_empty());
        assert_eq!(desc.protocol_type, "");
    }

    // ── AdminTopicInfo roundtrip ─────────────────────────────────────

    #[test]
    fn test_admin_topic_info_roundtrip() {
        let info = AdminTopicInfo {
            name: "roundtrip".into(),
            partitions: 6,
            replication_factor: 3,
            message_count: 42,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deser: AdminTopicInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.name, "roundtrip");
        assert_eq!(deser.partitions, 6);
        assert_eq!(deser.replication_factor, 3);
        assert_eq!(deser.message_count, 42);
    }

    // ── ServerInfo defaults ──────────────────────────────────────────

    #[test]
    fn test_server_info_minimal() {
        let json = r#"{}"#;
        let info: ServerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.version, "");
        assert_eq!(info.uptime_seconds, 0);
        assert_eq!(info.topics, 0);
        assert_eq!(info.messages_total, 0);
    }

    #[test]
    fn test_server_info_roundtrip() {
        let info = ServerInfo {
            version: "1.0.0".into(),
            uptime_seconds: 7200,
            topics: 10,
            messages_total: 100_000,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deser: ServerInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.version, "1.0.0");
        assert_eq!(deser.messages_total, 100_000);
    }

    // ── QueryResult edge cases ───────────────────────────────────────

    #[test]
    fn test_query_result_with_mixed_value_types() {
        let json = r#"{"columns":["id","name","active"],"rows":[[1,"alice",true],[2,"bob",false]],"row_count":2}"#;
        let result: QueryResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.rows[0][0], serde_json::json!(1));
        assert_eq!(result.rows[0][1], serde_json::json!("alice"));
        assert_eq!(result.rows[0][2], serde_json::json!(true));
    }

    #[test]
    fn test_query_result_with_null_values() {
        let json = r#"{"columns":["id","email"],"rows":[[1,null]],"row_count":1}"#;
        let result: QueryResult = serde_json::from_str(json).unwrap();
        assert!(result.rows[0][1].is_null());
    }

    #[test]
    fn test_query_result_minimal_defaults() {
        let json = r#"{}"#;
        let result: QueryResult = serde_json::from_str(json).unwrap();
        assert!(result.columns.is_empty());
        assert!(result.rows.is_empty());
        assert_eq!(result.row_count, 0);
    }

    #[test]
    fn test_query_result_roundtrip() {
        let result = QueryResult {
            columns: vec!["a".into(), "b".into()],
            rows: vec![vec![serde_json::json!(1), serde_json::json!("x")]],
            row_count: 1,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deser: QueryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.columns, result.columns);
        assert_eq!(deser.row_count, 1);
    }

    // ── TopicDescription ─────────────────────────────────────────────

    #[test]
    fn test_topic_description_empty_partitions() {
        let json = r#"{"name":"empty","partitions":[]}"#;
        let desc: TopicDescription = serde_json::from_str(json).unwrap();
        assert_eq!(desc.name, "empty");
        assert!(desc.partitions.is_empty());
    }

    #[test]
    fn test_topic_description_roundtrip() {
        let desc = TopicDescription {
            name: "rt-topic".into(),
            partitions: vec![PartitionInfo {
                id: 0,
                leader: 1,
                replicas: vec![1],
                isr: vec![1],
                start_offset: 0,
                end_offset: 100,
            }],
            config: serde_json::json!({"retention.ms": "86400000"}),
        };
        let json = serde_json::to_string(&desc).unwrap();
        let deser: TopicDescription = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.name, "rt-topic");
        assert_eq!(deser.partitions.len(), 1);
        assert_eq!(deser.partitions[0].end_offset, 100);
    }

    // ── ConsumerGroupInfo edge cases ─────────────────────────────────

    #[test]
    fn test_consumer_group_info_minimal() {
        let json = r#"{"group_id":"g","state":"Empty"}"#;
        let info: ConsumerGroupInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.group_id, "g");
        assert_eq!(info.members, 0);
    }

    #[test]
    fn test_consumer_group_info_roundtrip() {
        let info = ConsumerGroupInfo {
            group_id: "grp".into(),
            state: "Stable".into(),
            members: 5,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deser: ConsumerGroupInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.group_id, "grp");
        assert_eq!(deser.members, 5);
    }

    // ── Auth token management ────────────────────────────────────────

    #[test]
    fn test_admin_client_overwrite_auth_token() {
        let mut client = AdminClient::new("http://localhost:9094");
        client.set_auth_token("first");
        client.set_auth_token("second");
        assert_eq!(client.auth_token.as_deref(), Some("second"));
    }

    #[test]
    fn test_query_client_no_auth_by_default() {
        let client = QueryClient::new("http://localhost:9094");
        assert!(client.auth_token.is_none());
    }
}
