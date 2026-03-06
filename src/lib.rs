//! Browser-native WebAssembly SDK for the Streamline streaming platform.
//!
//! Provides a JavaScript-friendly API for producing and consuming messages
//! via WebSocket, plus topic administration and schema registry over HTTP.

pub mod admin;
mod protocol;
pub mod schema_registry;
pub mod telemetry;
mod websocket;

use wasm_bindgen::prelude::*;

pub use admin::{AdminClient, QueryClient};
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
}

/// Convenience producer handle (wraps a shared connection).
#[wasm_bindgen]
pub struct Producer {
    conn: WsConnection,
    default_topic: Option<String>,
}

#[wasm_bindgen]
impl Producer {
    /// Create a producer. Optionally bind it to a default topic.
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, default_topic: Option<String>) -> Self {
        Self {
            conn: WsConnection::new(url),
            default_topic,
        }
    }

    /// Open the underlying WebSocket.
    pub fn connect(&mut self) -> Result<(), JsValue> {
        self.conn.connect()
    }

    /// Send a message. Uses the default topic if none is specified.
    pub fn send(&self, value: &str, topic: Option<String>) -> Result<(), JsValue> {
        let t = topic
            .or_else(|| self.default_topic.clone())
            .ok_or_else(|| JsValue::from_str("no topic specified"))?;
        let msg = BrowserMessage::Produce {
            topic: t,
            key: None,
            value: value.to_string(),
        };
        self.conn.send_message(&msg)
    }

    /// Send a keyed message.
    pub fn send_keyed(
        &self,
        key: &str,
        value: &str,
        topic: Option<String>,
    ) -> Result<(), JsValue> {
        let t = topic
            .or_else(|| self.default_topic.clone())
            .ok_or_else(|| JsValue::from_str("no topic specified"))?;
        let msg = BrowserMessage::Produce {
            topic: t,
            key: Some(key.to_string()),
            value: value.to_string(),
        };
        self.conn.send_message(&msg)
    }

    /// Disconnect the producer.
    pub fn disconnect(&mut self) {
        self.conn.disconnect();
    }
}

/// Convenience consumer handle.
#[wasm_bindgen]
pub struct Consumer {
    conn: WsConnection,
    topic: String,
}

#[wasm_bindgen]
impl Consumer {
    /// Create a consumer for the given topic.
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, topic: &str) -> Self {
        Self {
            conn: WsConnection::new(url),
            topic: topic.to_string(),
        }
    }

    /// Connect and subscribe in one step, invoking `callback` for each message.
    pub fn start(&mut self, callback: js_sys::Function) -> Result<(), JsValue> {
        self.conn.connect()?;
        self.conn.on_message = Some(callback);
        let msg = BrowserMessage::Subscribe {
            topic: self.topic.clone(),
        };
        self.conn.send_message(&msg)
    }

    /// Stop consuming and disconnect.
    pub fn stop(&mut self) {
        let _ = self.conn.send_message(&BrowserMessage::Unsubscribe {
            topic: self.topic.clone(),
        });
        self.conn.disconnect();
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
