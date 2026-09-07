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
pub mod validation;
mod websocket;

use wasm_bindgen::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

pub use admin::{AdminClient, QueryClient};
pub use circuit_breaker::{CircuitBreaker, CircuitState};
pub use error::{ErrorCode, StreamlineError};
pub use moonshot::{MemoryReadClient, SearchClient};
pub use protocol::{AdminAction, BrowserMessage, BrowserResponse, TopicInfo};
pub use schema_registry::{SchemaFormat, SchemaRegistryClient};
pub use telemetry::{Telemetry, TelemetrySpan};
pub use validation::validate_topic_name;
pub use websocket::{ConnectionState, WsConnection};

fn subscription_key(topic: &str) -> String {
    format!("subscription:{topic}")
}

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
        validation::validate_topic_name(topic).map_err(|e| StreamlineError::configuration(&e))?;
        self.produce_with_key(topic, None, value)
    }

    /// Produce a keyed message to the given topic.
    pub fn produce_with_key(
        &self,
        topic: &str,
        key: Option<String>,
        value: &str,
    ) -> Result<(), JsValue> {
        validation::validate_topic_name(topic).map_err(|e| StreamlineError::configuration(&e))?;
        let msg = BrowserMessage::Produce {
            topic: topic.to_string(),
            key,
            value: value.to_string(),
        };
        self.conn.send_message(&msg)
    }

    /// Subscribe to messages on a topic. The provided JS callback is invoked
    /// for every incoming message on *this* topic. Callbacks are demultiplexed
    /// per topic, so subscribing to a different topic does not overwrite an
    /// earlier subscription's callback — each topic keeps its own.
    pub fn subscribe(&mut self, topic: &str, callback: js_sys::Function) -> Result<(), JsValue> {
        validation::validate_topic_name(topic).map_err(|e| StreamlineError::configuration(&e))?;
        self.conn.set_topic_callback(topic, callback);
        let msg = BrowserMessage::Subscribe {
            topic: topic.to_string(),
        };
        let json = msg
            .to_json()
            .map_err(|e| StreamlineError::serialization(&e.to_string()))?;
        self.conn
            .send_persistent_when_ready(subscription_key(topic), json)
    }

    /// Unsubscribe from a topic. Only this topic's callback and replay
    /// registration are removed; other topics' subscriptions on the same
    /// client are unaffected.
    pub fn unsubscribe(&self, topic: &str) -> Result<(), JsValue> {
        self.conn.remove_topic_callback(topic);
        self.conn
            .remove_persistent_message(&subscription_key(topic));
        let msg = BrowserMessage::Unsubscribe {
            topic: topic.to_string(),
        };
        self.conn.send_message(&msg)
    }

    /// Request topic creation via admin message.
    pub fn create_topic(&self, name: &str, partitions: Option<u32>) -> Result<(), JsValue> {
        validation::validate_topic_name(name).map_err(|e| StreamlineError::configuration(&e))?;
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
        validation::validate_topic_name(name).map_err(|e| StreamlineError::configuration(&e))?;
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
        self.conn.set_on_state_change(callback);
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
        self.conn.set_on_reconnect_failed(callback);
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
    /// The callback receives `(sent_this_call: number, failed_attempts: number)`.
    /// Records left in the backlog but not attempted are reported separately
    /// by `pending_count()`.
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
        validation::validate_topic_name(&t).map_err(|e| StreamlineError::configuration(&e))?;
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
        validation::validate_topic_name(&t).map_err(|e| StreamlineError::configuration(&e))?;
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

    /// Flush all pending messages in the batch, in order.
    ///
    /// On the first send failure, flushing stops immediately: the failed
    /// message and every message after it (never attempted this call) are
    /// preserved in `self.batch`, in their original order, so a transport
    /// error never silently drops records. Messages that were already sent
    /// successfully before the failure are *not* re-queued, so a retried
    /// `flush()` never re-sends (and thus never duplicates) them. The
    /// underlying transport error is returned so callers can observe and
    /// react to the failure instead of it being swallowed.
    ///
    /// Invokes the on_delivery callback (if set) with `(sent_this_call,
    /// failed_attempts)` counts before returning. Use `pending_count()` to
    /// inspect the retained backlog.
    pub fn flush(&mut self) -> Result<(), JsValue> {
        let mut sent = 0usize;
        let mut first_error: Option<JsValue> = None;

        for msg in &self.batch {
            match self.conn.send_message(msg) {
                Ok(()) => sent += 1,
                Err(err) => {
                    first_error = Some(err);
                    break;
                }
            }
        }

        // Drop only the successfully-sent prefix. The failed message (if
        // any) and every unattempted message after it remain in the batch,
        // in original order, ready for a subsequent flush() retry.
        self.batch.drain(0..sent);

        self.sent_count += sent as u32;
        let failed_attempts = u32::from(first_error.is_some());
        self.error_count += failed_attempts;

        if let Some(ref cb) = self.on_delivery {
            let _ = cb.call2(
                &JsValue::NULL,
                &JsValue::from(sent as u32),
                &JsValue::from(failed_attempts),
            );
        }

        match first_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
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
    pub fn send_transactional(
        &mut self,
        value: &str,
        topic: &str,
        key: Option<String>,
    ) -> Result<(), JsValue> {
        if !self.in_transaction {
            return Err(JsValue::from_str("No transaction in progress"));
        }
        self.transaction_buffer
            .push((value.to_string(), key, topic.to_string()));
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
    ///
    /// The socket is always closed, even if the final flush fails — this is
    /// an intentional disconnect and the transport is going away regardless.
    /// However a failed flush is never swallowed: any records that could not
    /// be sent remain in the batch (inspect via `pending_count()`) and the
    /// underlying transport error is returned so the caller learns the
    /// disconnect did not silently claim success for undelivered records.
    pub fn disconnect(&mut self) -> Result<(), JsValue> {
        let flush_result = self.flush();
        self.conn.disconnect();
        flush_result
    }
}

/// Local, unconfirmed offset bookkeeping shared between `Consumer`'s public
/// methods and the internal message-dispatch closure registered with the
/// WebSocket connection (see [`Consumer::start`]). This SDK does not
/// implement a broker commit-acknowledgement protocol, so `committed_offset`
/// only ever reflects a *local* value and never changes — see
/// [`Consumer::commit`].
struct ConsumerOffsetState {
    current_offset: i64,
    committed_offset: i64,
}

/// Convenience consumer handle with offset tracking and consumer group support.
///
/// **Offset commit is unsupported.** This SDK version has no wire protocol
/// for the server to acknowledge that an offset was durably committed, so
/// [`Consumer::commit`], [`Consumer::commit_offset`], and the auto-commit
/// path triggered from [`Consumer::advance_offset`] all fail closed with
/// `ErrorCode::Unsupported` rather than silently pretending the broker
/// stored the offset. `committed_offset()` therefore always reports `-1`.
/// Local offset *tracking* (`current_offset`) is still fully wired to
/// message delivery via [`Consumer::start`].
#[wasm_bindgen]
pub struct Consumer {
    conn: WsConnection,
    topic: String,
    group_id: Option<String>,
    offsets: Rc<RefCell<ConsumerOffsetState>>,
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
            offsets: Rc::new(RefCell::new(ConsumerOffsetState {
                current_offset: 0,
                committed_offset: -1,
            })),
        }
    }

    /// Set the consumer group ID for server-side offset storage and coordination.
    pub fn set_group_id(&mut self, group_id: &str) {
        self.group_id = Some(group_id.to_string());
    }

    /// Returns `true` when the underlying WebSocket is open.
    pub fn is_connected(&self) -> bool {
        self.conn.is_connected()
    }

    /// Configure automatic offset commits.
    ///
    /// `interval == 0` is accepted as the disabled state. Any non-zero value
    /// fails closed with [`ErrorCode::Unsupported`] because this SDK has no
    /// broker acknowledgement protocol for durable commits.
    pub fn set_auto_commit(&mut self, interval: u32) -> Result<(), JsValue> {
        if interval == 0 {
            return Ok(());
        }
        Err(StreamlineError::unsupported(
            "auto-commit is unavailable because offset commits have no broker acknowledgement protocol",
        ))
    }

    /// Connect and subscribe in one step, invoking `callback` for each message.
    /// If a group_id is set, the subscription includes it for server-side
    /// coordination. Each delivered message wires `current_offset` forward
    /// (see [`Consumer::advance_offset`]) *before* invoking `callback`.
    pub fn start(&mut self, callback: js_sys::Function) -> Result<(), JsValue> {
        let offsets = Rc::clone(&self.offsets);
        let dispatch = Closure::<dyn FnMut(JsValue)>::new(move |message: JsValue| {
            if let Some(text) = message.as_string() {
                if let Ok(BrowserResponse::Message { offset, .. }) =
                    BrowserResponse::from_json(&text)
                {
                    Consumer::advance_offset_shared(&offsets, offset as i64);
                }
            }
            let _ = callback.call1(&JsValue::NULL, &message);
        });
        let function: js_sys::Function = dispatch
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        dispatch.forget();
        self.conn.set_topic_callback(&self.topic, function);

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
        self.conn.connect()?;
        self.conn
            .send_persistent_when_ready(subscription_key(&self.topic), msg)
    }

    /// Stop consuming and disconnect. No offset commit is attempted because
    /// commit and auto-commit are explicitly unsupported.
    pub fn stop(&mut self) {
        self.conn.remove_topic_callback(&self.topic);
        self.conn
            .remove_persistent_message(&subscription_key(&self.topic));
        let _ = self.conn.send_message(&BrowserMessage::Unsubscribe {
            topic: self.topic.clone(),
        });
        self.conn.disconnect();
    }

    /// Get the current consumer offset (last received message offset + 1).
    #[wasm_bindgen(getter)]
    pub fn current_offset(&self) -> i64 {
        self.offsets.borrow().current_offset
    }

    /// Get the last *locally confirmed* committed offset. Always `-1`:
    /// this SDK has no broker acknowledgement protocol for commits, so no
    /// offset is ever reported as durably committed. See
    /// [`Consumer::commit`].
    #[wasm_bindgen(getter)]
    pub fn committed_offset(&self) -> i64 {
        self.offsets.borrow().committed_offset
    }

    /// Get the consumer group ID, if set.
    #[wasm_bindgen(getter)]
    pub fn group_id(&self) -> Option<String> {
        self.group_id.clone()
    }

    /// Advance the current offset (called internally when messages arrive
    /// via [`Consumer::start`], and callable directly for manual offset
    /// tracking). This never commits the offset; commit and auto-commit are
    /// separate unsupported operations.
    pub fn advance_offset(&mut self, offset: i64) -> Result<(), JsValue> {
        Self::advance_offset_shared(&self.offsets, offset);
        Ok(())
    }

    fn advance_offset_shared(offsets: &Rc<RefCell<ConsumerOffsetState>>, offset: i64) {
        let mut state = offsets.borrow_mut();
        if offset >= state.current_offset {
            state.current_offset = offset + 1;
        }
    }

    /// Commit the current offset to the server.
    ///
    /// **Always fails closed with `ErrorCode::Unsupported`.** This SDK
    /// version has no wire protocol for the server to acknowledge that an
    /// offset was durably stored, so this method refuses to claim success
    /// rather than guess. `committed_offset()` and `current_offset()` are
    /// left unchanged. Track offsets locally via `current_offset()`, or
    /// rely on server-side consumer-group coordination instead.
    pub fn commit(&mut self) -> Result<(), JsValue> {
        Self::commit_shared()
    }

    /// Commit a specific offset to the server. Always fails closed; see
    /// [`Consumer::commit`].
    pub fn commit_offset(&mut self, _offset: i64) -> Result<(), JsValue> {
        Self::commit_shared()
    }

    fn commit_shared() -> Result<(), JsValue> {
        Err(StreamlineError::unsupported(
            "offset commit has no broker acknowledgement protocol in this SDK version; \
             the broker's durable state is never confirmed, so commit() refuses to claim success",
        ))
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
        self.offsets.borrow_mut().current_offset = offset;
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
        let resp_value =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await?;
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
        serde_wasm_bindgen::to_value(&parsed.hits).map_err(|e| JsValue::from_str(&e.to_string()))
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

    /// Returns `true` when the WebSocket is open.
    pub fn is_connected(&self) -> bool {
        self.conn.is_connected()
    }

    /// Get the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.conn.state()
    }

    /// Register a callback for connection state changes.
    pub fn on_state_change(&mut self, callback: js_sys::Function) {
        self.conn.set_on_state_change(callback);
    }

    pub fn create_topic(&self, name: &str, partitions: Option<u32>) -> Result<(), JsValue> {
        validation::validate_topic_name(name).map_err(|e| StreamlineError::configuration(&e))?;
        let msg = BrowserMessage::Admin {
            action: AdminAction::CreateTopic {
                name: name.to_string(),
                partitions,
            },
        };
        self.conn.send_message(&msg)
    }

    pub fn delete_topic(&self, name: &str) -> Result<(), JsValue> {
        validation::validate_topic_name(name).map_err(|e| StreamlineError::configuration(&e))?;
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
        self.conn.set_on_message(callback);
    }

    pub fn disconnect(&mut self) {
        self.conn.disconnect();
    }
}
