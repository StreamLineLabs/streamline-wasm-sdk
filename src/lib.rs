//! Browser-native WebAssembly SDK for the Streamline streaming platform.
//!
//! Provides a JavaScript-friendly API for producing and consuming messages
//! via WebSocket, plus topic administration and schema registry over HTTP.

/// SDK version constant.
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod admin;
pub mod circuit_breaker;
pub mod error;
pub mod moonshot;
mod protocol;
pub mod schema_registry;
pub mod telemetry;
mod websocket;

use wasm_bindgen::prelude::*;

pub use admin::{AdminClient, QueryClient};
pub use circuit_breaker::{CircuitBreaker, CircuitState};
pub use error::{ErrorCode, StreamlineError};
pub use moonshot::{MemoryReadClient, SearchClient};
pub use protocol::{AdminAction, BrowserMessage, BrowserResponse, TopicInfo};
pub use schema_registry::{SchemaFormat, SchemaRegistryClient};
pub use telemetry::{Telemetry, TelemetrySpan};
pub use websocket::{ConnectionState, WsConnection};

/// High-level Streamline client for browser environments.
///
/// Wraps a WebSocket connection and exposes produce / subscribe / admin helpers.
#[wasm_bindgen]
pub struct StreamlineClient {
    conn: WsConnection,
}

#[wasm_bindgen]
impl StreamlineClient {
    /// Create a new client targeting the given WebSocket URL
    /// (e.g. `ws://localhost:9094/ws`).
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str) -> Self {
        Self {
            conn: WsConnection::new(url),
        }
    }

    /// Open the WebSocket connection.
    pub fn connect(&mut self) -> Result<(), JsValue> {
        self.conn.connect()
    }

    /// Close the WebSocket connection.
    pub fn disconnect(&mut self) {
        self.conn.disconnect();
    }

    /// Returns `true` when the WebSocket is open.
    pub fn is_connected(&self) -> bool {
        self.conn.is_connected()
    }

    /// Produce a message to the given topic.
    pub fn produce(&self, topic: &str, value: &str) -> Result<(), JsValue> {
        self.produce_with_key(topic, None, value)
    }

    /// Produce a keyed message to the given topic.
    pub fn produce_with_key(
        &self,
        topic: &str,
        key: Option<String>,
        value: &str,
    ) -> Result<(), JsValue> {
        let msg = BrowserMessage::Produce {
            topic: topic.to_string(),
            key,
            value: value.to_string(),
        };
        self.conn.send_message(&msg)
    }

    /// Subscribe to messages on a topic. The provided JS callback is invoked
    /// for every incoming message.
    pub fn subscribe(&mut self, topic: &str, callback: js_sys::Function) -> Result<(), JsValue> {
        self.conn.on_message = Some(callback);
        let msg = BrowserMessage::Subscribe {
            topic: topic.to_string(),
        };
        self.conn.send_message(&msg)
    }

    /// Unsubscribe from a topic.
    pub fn unsubscribe(&self, topic: &str) -> Result<(), JsValue> {
        let msg = BrowserMessage::Unsubscribe {
            topic: topic.to_string(),
        };
        self.conn.send_message(&msg)
    }

    /// Request topic creation via admin message.
    pub fn create_topic(&self, name: &str, partitions: Option<u32>) -> Result<(), JsValue> {
        let msg = BrowserMessage::Admin {
            action: AdminAction::CreateTopic {
                name: name.to_string(),
                partitions,
            },
        };
        self.conn.send_message(&msg)
    }

    /// Request topic deletion via admin message.
    pub fn delete_topic(&self, name: &str) -> Result<(), JsValue> {
        let msg = BrowserMessage::Admin {
            action: AdminAction::DeleteTopic {
                name: name.to_string(),
            },
        };
        self.conn.send_message(&msg)
    }

    /// Request the list of topics via admin message.
    pub fn list_topics(&self) -> Result<(), JsValue> {
        let msg = BrowserMessage::Admin {
            action: AdminAction::ListTopics,
        };
        self.conn.send_message(&msg)
    }

    /// Register a callback for connection state changes.
    pub fn on_state_change(&mut self, callback: js_sys::Function) {
        self.conn.on_state_change = Some(callback);
    }

    /// Enable or disable automatic reconnection (enabled by default).
    pub fn set_auto_reconnect(&mut self, enabled: bool) {
        self.conn.set_auto_reconnect(enabled);
    }

    /// Set the maximum number of reconnection attempts (default: 5).
    /// Use `0` for unlimited attempts.
    pub fn set_max_reconnect_attempts(&mut self, max: u32) {
        self.conn.set_max_reconnect_attempts(max);
    }

    /// Register a callback invoked when all reconnection attempts are exhausted.
    pub fn on_reconnect_failed(&mut self, callback: js_sys::Function) {
        self.conn.on_reconnect_failed = Some(callback);
    }

    /// Get the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.conn.state()
    }
}

/// Convenience producer handle with batching and delivery acknowledgment support.
///
/// Messages are accumulated and flushed when the batch reaches `batch_size`
/// or when `flush()` is called manually. Use `set_linger_ms()` to enable
/// automatic time-based flushing via `setTimeout` in the browser.
#[wasm_bindgen]
pub struct Producer {
    conn: WsConnection,
    default_topic: Option<String>,
    batch: Vec<BrowserMessage>,
    batch_size: usize,
    on_delivery: Option<js_sys::Function>,
    sent_count: u32,
    error_count: u32,
    in_transaction: bool,
    transaction_buffer: Vec<(String, Option<String>, String)>, // (value, key, topic)
}

#[wasm_bindgen]
impl Producer {
    /// Create a producer. Optionally bind it to a default topic.
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, default_topic: Option<String>) -> Self {
        Self {
            conn: WsConnection::new(url),
            default_topic,
            batch: Vec::new(),
            batch_size: 100,
            on_delivery: None,
            sent_count: 0,
            error_count: 0,
            in_transaction: false,
            transaction_buffer: Vec::new(),
        }
    }

    /// Set the maximum batch size before auto-flush (default: 100).
    pub fn set_batch_size(&mut self, size: usize) {
        self.batch_size = size;
    }

    /// Register a callback invoked after each flush with delivery status.
    /// The callback receives `(sent_count: number, error_count: number)`.
    pub fn on_delivery(&mut self, callback: js_sys::Function) {
        self.on_delivery = Some(callback);
    }

    /// Open the underlying WebSocket.
    pub fn connect(&mut self) -> Result<(), JsValue> {
        self.conn.connect()
    }

    /// Send a message. Accumulates into the batch and auto-flushes when full.
    pub fn send(&mut self, value: &str, topic: Option<String>) -> Result<(), JsValue> {
        let t = topic
            .or_else(|| self.default_topic.clone())
            .ok_or_else(|| StreamlineError::produce_error("no topic specified"))?;
        let msg = BrowserMessage::Produce {
            topic: t,
            key: None,
            value: value.to_string(),
        };
        self.batch.push(msg);

        if self.batch.len() >= self.batch_size {
            self.flush()?;
        }
        Ok(())
    }

    /// Send a keyed message. Accumulates into the batch.
    pub fn send_keyed(
        &mut self,
        key: &str,
        value: &str,
        topic: Option<String>,
    ) -> Result<(), JsValue> {
        let t = topic
            .or_else(|| self.default_topic.clone())
            .ok_or_else(|| StreamlineError::produce_error("no topic specified"))?;
        let msg = BrowserMessage::Produce {
            topic: t,
            key: Some(key.to_string()),
            value: value.to_string(),
        };
        self.batch.push(msg);

        if self.batch.len() >= self.batch_size {
            self.flush()?;
        }
        Ok(())
    }

    /// Flush all pending messages in the batch.
    /// Invokes the on_delivery callback (if set) with delivery status.
    pub fn flush(&mut self) -> Result<(), JsValue> {
        let messages = std::mem::take(&mut self.batch);
        let mut sent = 0u32;
        let mut errors = 0u32;
        for msg in messages {
            match self.conn.send_message(&msg) {
                Ok(()) => sent += 1,
                Err(_) => errors += 1,
            }
        }
        self.sent_count += sent;
        self.error_count += errors;

        if let Some(ref cb) = self.on_delivery {
            let _ = cb.call2(
                &JsValue::NULL,
                &JsValue::from(sent),
                &JsValue::from(errors),
            );
        }
        Ok(())
    }

    /// Returns the number of messages waiting in the batch.
    pub fn pending_count(&self) -> usize {
        self.batch.len()
    }

    /// Total messages successfully sent since creation.
    #[wasm_bindgen(getter)]
    pub fn total_sent(&self) -> u32 {
        self.sent_count
    }

    /// Total send errors since creation.
    #[wasm_bindgen(getter)]
    pub fn total_errors(&self) -> u32 {
        self.error_count
    }

    /// Begin a transaction. Messages sent via `send_transactional` are
    /// buffered until `commit_transaction` or `abort_transaction`.
    pub fn begin_transaction(&mut self) -> Result<(), JsValue> {
        if self.in_transaction {
            return Err(JsValue::from_str("Transaction already in progress"));
        }
        self.in_transaction = true;
        self.transaction_buffer.clear();
        Ok(())
    }

    /// Buffer a message in the current transaction.
    pub fn send_transactional(&mut self, value: &str, topic: &str, key: Option<String>) -> Result<(), JsValue> {
        if !self.in_transaction {
            return Err(JsValue::from_str("No transaction in progress"));
        }
        self.transaction_buffer.push((value.to_string(), key, topic.to_string()));
        Ok(())
    }

    /// Commit the transaction, sending all buffered messages.
    pub fn commit_transaction(&mut self) -> Result<JsValue, JsValue> {
        if !self.in_transaction {
            return Err(JsValue::from_str("No transaction in progress"));
        }
        let buffer = std::mem::take(&mut self.transaction_buffer);
        self.in_transaction = false;

        let mut count = 0u32;
        for (value, key, topic) in &buffer {
            match key {
                Some(k) => self.send_keyed(k, value, Some(topic.clone()))?,
                None => self.send(value, Some(topic.clone()))?,
            }
            count += 1;
        }
        Ok(JsValue::from(count))
    }

    /// Abort the transaction, discarding all buffered messages.
    pub fn abort_transaction(&mut self) -> Result<(), JsValue> {
        if !self.in_transaction {
            return Err(JsValue::from_str("No transaction in progress"));
        }
        self.transaction_buffer.clear();
        self.in_transaction = false;
        Ok(())
    }

    /// Disconnect the producer, flushing pending messages first.
    pub fn disconnect(&mut self) {
        let _ = self.flush();
        self.conn.disconnect();
    }
}

/// Convenience consumer handle with offset tracking and consumer group support.
#[wasm_bindgen]
pub struct Consumer {
    conn: WsConnection,
    topic: String,
    group_id: Option<String>,
    current_offset: i64,
    committed_offset: i64,
    auto_commit: bool,
    auto_commit_count: u32,
    uncommitted_count: u32,
}

#[wasm_bindgen]
impl Consumer {
    /// Create a consumer for the given topic.
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, topic: &str) -> Self {
        Self {
            conn: WsConnection::new(url),
            topic: topic.to_string(),
            group_id: None,
            current_offset: 0,
            committed_offset: -1,
            auto_commit: false,
            auto_commit_count: 0,
            uncommitted_count: 0,
        }
    }

    /// Set the consumer group ID for server-side offset storage and coordination.
    pub fn set_group_id(&mut self, group_id: &str) {
        self.group_id = Some(group_id.to_string());
    }

    /// Enable auto-commit: offsets are committed every `interval` messages.
    /// Set to 0 to disable (default).
    pub fn set_auto_commit(&mut self, interval: u32) {
        self.auto_commit = interval > 0;
        self.auto_commit_count = interval;
    }

    /// Connect and subscribe in one step, invoking `callback` for each message.
    /// If a group_id is set, the subscription includes it for server-side coordination.
    pub fn start(&mut self, callback: js_sys::Function) -> Result<(), JsValue> {
        self.conn.connect()?;
        self.conn.on_message = Some(callback);
        let msg = if let Some(ref gid) = self.group_id {
            serde_json::json!({
                "type": "subscribe",
                "topic": self.topic,
                "group_id": gid,
            })
            .to_string()
        } else {
            serde_json::to_string(&BrowserMessage::Subscribe {
                topic: self.topic.clone(),
            })
            .map_err(|e| StreamlineError::serialization(&e.to_string()))?
        };
        self.conn.send(&msg)
    }

    /// Stop consuming and disconnect. Commits offsets if auto-commit is active.
    pub fn stop(&mut self) {
        if self.auto_commit && self.uncommitted_count > 0 {
            let _ = self.commit();
        }
        let _ = self.conn.send_message(&BrowserMessage::Unsubscribe {
            topic: self.topic.clone(),
        });
        self.conn.disconnect();
    }

    /// Get the current consumer offset (last received message offset + 1).
    #[wasm_bindgen(getter)]
    pub fn current_offset(&self) -> i64 {
        self.current_offset
    }

    /// Get the last committed offset.
    #[wasm_bindgen(getter)]
    pub fn committed_offset(&self) -> i64 {
        self.committed_offset
    }

    /// Get the consumer group ID, if set.
    #[wasm_bindgen(getter)]
    pub fn group_id(&self) -> Option<String> {
        self.group_id.clone()
    }

    /// Advance the current offset (called internally when messages arrive).
    /// Triggers auto-commit if enabled and the interval has been reached.
    pub fn advance_offset(&mut self, offset: i64) -> Result<(), JsValue> {
        if offset >= self.current_offset {
            self.current_offset = offset + 1;
        }
        self.uncommitted_count += 1;

        if self.auto_commit && self.auto_commit_count > 0 && self.uncommitted_count >= self.auto_commit_count {
            self.commit()?;
        }
        Ok(())
    }

    /// Commit the current offset to the server.
    /// Uses the group_id for server-side storage when available.
    pub fn commit(&mut self) -> Result<(), JsValue> {
        if !self.conn.is_connected() {
            return Err(StreamlineError::not_connected());
        }
        let msg = if let Some(ref gid) = self.group_id {
            serde_json::json!({
                "type": "commit_offset",
                "topic": self.topic,
                "offset": self.current_offset,
                "group_id": gid,
            })
        } else {
            serde_json::json!({
                "type": "commit_offset",
                "topic": self.topic,
                "offset": self.current_offset,
            })
        };
        self.conn.send(&msg.to_string())?;
        self.committed_offset = self.current_offset;
        self.uncommitted_count = 0;
        Ok(())
    }

    /// Commit a specific offset to the server.
    pub fn commit_offset(&mut self, offset: i64) -> Result<(), JsValue> {
        if !self.conn.is_connected() {
            return Err(StreamlineError::not_connected());
        }
        let msg = if let Some(ref gid) = self.group_id {
            serde_json::json!({
                "type": "commit_offset",
                "topic": self.topic,
                "offset": offset,
                "group_id": gid,
            })
        } else {
            serde_json::json!({
                "type": "commit_offset",
                "topic": self.topic,
                "offset": offset,
            })
        };
        self.conn.send(&msg.to_string())?;
        self.committed_offset = offset;
        self.uncommitted_count = 0;
        Ok(())
    }

    /// Seek to a specific offset. The next message received will be from this offset.
    pub fn seek(&mut self, offset: i64) -> Result<(), JsValue> {
        if !self.conn.is_connected() {
            return Err(StreamlineError::not_connected());
        }
        let msg = serde_json::json!({
            "type": "seek",
            "topic": self.topic,
            "offset": offset,
        });
        self.conn.send(&msg.to_string())?;
        self.current_offset = offset;
        Ok(())
    }

    /// Search the consumer's topic using semantic search via the HTTP API.
    ///
    /// Sends a `POST /api/v1/topics/{topic}/search` to the Streamline HTTP
    /// admin port. The `base_url` should point to the HTTP API
    /// (e.g. `http://localhost:9094`).
    pub async fn search(&self, base_url: &str, query: &str, k: u32) -> Result<JsValue, JsValue> {
        if query.is_empty() {
            return Err(JsValue::from_str("query is required"));
        }
        if k == 0 {
            return Err(JsValue::from_str("k must be > 0"));
        }

        let body = serde_json::json!({ "query": query, "k": k });
        let base = base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/topics/{}/search", base, self.topic);

        let opts = web_sys::RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(web_sys::RequestMode::Cors);
        opts.set_body(&JsValue::from_str(&body.to_string()));

        let request = web_sys::Request::new_with_str_and_init(&url, &opts)?;
        request.headers().set("Accept", "application/json")?;
        request.headers().set("Content-Type", "application/json")?;

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: web_sys::Response = resp_value.dyn_into()?;

        if !resp.ok() {
            return Err(JsValue::from_str(&format!(
                "HTTP {}: {}",
                resp.status(),
                resp.status_text()
            )));
        }

        let text = wasm_bindgen_futures::JsFuture::from(resp.text()?).await?;
        let text_str = text
            .as_string()
            .ok_or_else(|| JsValue::from_str("response is not a string"))?;

        #[derive(serde::Deserialize)]
        struct SearchResponse {
            #[serde(default)]
            hits: Vec<moonshot::SearchHit>,
        }

        let parsed: SearchResponse = serde_json::from_str(&text_str)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&parsed.hits)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Topic administration helper.
#[wasm_bindgen]
pub struct TopicAdmin {
    conn: WsConnection,
}

#[wasm_bindgen]
impl TopicAdmin {
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str) -> Self {
        Self {
            conn: WsConnection::new(url),
        }
    }

    pub fn connect(&mut self) -> Result<(), JsValue> {
        self.conn.connect()
    }

    pub fn create_topic(&self, name: &str, partitions: Option<u32>) -> Result<(), JsValue> {
        let msg = BrowserMessage::Admin {
            action: AdminAction::CreateTopic {
                name: name.to_string(),
                partitions,
            },
        };
        self.conn.send_message(&msg)
    }

    pub fn delete_topic(&self, name: &str) -> Result<(), JsValue> {
        let msg = BrowserMessage::Admin {
            action: AdminAction::DeleteTopic {
                name: name.to_string(),
            },
        };
        self.conn.send_message(&msg)
    }

    pub fn list_topics(&self) -> Result<(), JsValue> {
        let msg = BrowserMessage::Admin {
            action: AdminAction::ListTopics,
        };
        self.conn.send_message(&msg)
    }

    /// Register a callback that receives admin responses.
    pub fn on_response(&mut self, callback: js_sys::Function) {
        self.conn.on_message = Some(callback);
    }

    pub fn disconnect(&mut self) {
        self.conn.disconnect();
    }
}
