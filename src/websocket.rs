use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use crate::error::StreamlineError;
use crate::protocol::BrowserMessage;

/// Connection state for the WebSocket transport.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

/// WebSocket connection manager with auto-reconnection support.
#[wasm_bindgen]
pub struct WsConnection {
    url: String,
    ws: Option<WebSocket>,
    state: ConnectionState,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    #[wasm_bindgen(skip)]
    pub on_message: Option<js_sys::Function>,
    #[wasm_bindgen(skip)]
    pub on_state_change: Option<js_sys::Function>,
}

#[wasm_bindgen]
impl WsConnection {
    /// Create a new WebSocket connection manager.
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            ws: None,
            state: ConnectionState::Disconnected,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
        on_message: None,
            on_state_change: None,
        }
    }

    /// Set the maximum number of reconnection attempts (default: 5).
    pub fn set_max_reconnect_attempts(&mut self, max: u32) {
        self.max_reconnect_attempts = max;
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Connect to the Streamline server.
    pub fn connect(&mut self) -> Result<(), JsValue> {
        self.set_state(ConnectionState::Connecting);

        let ws = WebSocket::new(&self.url)?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        self.setup_event_handlers(&ws)?;
        self.ws = Some(ws);
        Ok(())
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        if let Some(ref ws) = self.ws {
            let _ = ws.close();
        }
        self.ws = None;
        self.set_state(ConnectionState::Disconnected);
    }

    /// Send a protocol message over the WebSocket.
    pub fn send(&self, message: &str) -> Result<(), JsValue> {
        match &self.ws {
            Some(ws) if ws.ready_state() == WebSocket::OPEN => {
                ws.send_with_str(message)
            }
            _ => Err(StreamlineError::not_connected()),
        }
    }

    /// Check whether the connection is open.
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Calculate reconnection delay with exponential backoff (in ms).
    pub fn reconnect_delay_ms(&self) -> u32 {
        let base_ms = 1000u32;
        let max_ms = 30_000u32;
        let delay = base_ms.saturating_mul(2u32.saturating_pow(self.reconnect_attempts));
        delay.min(max_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ConnectionState ──────────────────────────────────────────────

    #[test]
    fn test_connection_state_equality() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_ne!(ConnectionState::Connected, ConnectionState::Disconnected);
    }

    #[test]
    fn test_connection_state_clone() {
        let state = ConnectionState::Reconnecting;
        let cloned = state;
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_connection_state_debug() {
        let dbg = format!("{:?}", ConnectionState::Connecting);
        assert_eq!(dbg, "Connecting");
    }

    // ── reconnect_delay_ms ───────────────────────────────────────────

    #[test]
    fn test_reconnect_delay_initial() {
        let conn = WsConnection {
            url: String::new(),
            ws: None,
            state: ConnectionState::Disconnected,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            on_message: None,
            on_state_change: None,
        };
        assert_eq!(conn.reconnect_delay_ms(), 1_000);
    }

    #[test]
    fn test_reconnect_delay_exponential_backoff() {
        let delays: Vec<u32> = (0..5)
            .map(|attempts| {
                let conn = WsConnection {
                    url: String::new(),
                    ws: None,
                    state: ConnectionState::Disconnected,
                    reconnect_attempts: attempts,
                    max_reconnect_attempts: 5,
                    on_message: None,
                    on_state_change: None,
                };
                conn.reconnect_delay_ms()
            })
            .collect();
        assert_eq!(delays, vec![1_000, 2_000, 4_000, 8_000, 16_000]);
    }

    #[test]
    fn test_reconnect_delay_capped_at_30s() {
        let conn = WsConnection {
            url: String::new(),
            ws: None,
            state: ConnectionState::Disconnected,
            reconnect_attempts: 10,
            max_reconnect_attempts: 5,
            on_message: None,
            on_state_change: None,
        };
        assert_eq!(conn.reconnect_delay_ms(), 30_000);
    }

    #[test]
    fn test_reconnect_delay_large_attempts_no_overflow() {
        let conn = WsConnection {
            url: String::new(),
            ws: None,
            state: ConnectionState::Disconnected,
            reconnect_attempts: 100,
            max_reconnect_attempts: 5,
            on_message: None,
            on_state_change: None,
        };
        // Should not panic — saturating arithmetic caps the value
        assert_eq!(conn.reconnect_delay_ms(), 30_000);
    }

    // ── WsConnection field defaults ──────────────────────────────────

    #[test]
    fn test_is_connected_when_disconnected() {
        let conn = WsConnection {
            url: "ws://localhost:9094/ws".into(),
            ws: None,
            state: ConnectionState::Disconnected,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            on_message: None,
            on_state_change: None,
        };
        assert!(!conn.is_connected());
    }

    #[test]
    fn test_is_connected_when_connected() {
        let conn = WsConnection {
            url: String::new(),
            ws: None,
            state: ConnectionState::Connected,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            on_message: None,
            on_state_change: None,
        };
        assert!(conn.is_connected());
    }

    #[test]
    fn test_state_returns_current_state() {
        let conn = WsConnection {
            url: String::new(),
            ws: None,
            state: ConnectionState::Reconnecting,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            on_message: None,
            on_state_change: None,
        };
        assert_eq!(conn.state(), ConnectionState::Reconnecting);
    }

    #[test]
    fn test_set_max_reconnect_attempts() {
        let mut conn = WsConnection {
            url: String::new(),
            ws: None,
            state: ConnectionState::Disconnected,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
            on_message: None,
            on_state_change: None,
        };
        conn.set_max_reconnect_attempts(10);
        assert_eq!(conn.max_reconnect_attempts, 10);
    }
}

// Internal helper methods (not exported to JS).
impl WsConnection {
    /// Send a typed browser message (internal only — not exported to JS).
    pub(crate) fn send_message(&self, msg: &BrowserMessage) -> Result<(), JsValue> {
        let json = msg
            .to_json()
            .map_err(|e| StreamlineError::serialization(&e.to_string()))?;
        self.send(&json)
    }

    fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
        if let Some(ref cb) = self.on_state_change {
            let _ = cb.call1(&JsValue::NULL, &JsValue::from(format!("{state:?}")));
        }
    }

    fn setup_event_handlers(&self, ws: &WebSocket) -> Result<(), JsValue> {
        // onopen
        let onopen = Closure::<dyn FnMut()>::new(move || {
            web_sys::console::log_1(&"[streamline-wasm] WebSocket connected".into());
        });
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();

        // onmessage
        let on_msg_cb = self.on_message.clone();
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                let s: String = text.into();
                if let Some(ref cb) = on_msg_cb {
                    let _ = cb.call1(&JsValue::NULL, &JsValue::from(&s));
                }
            }
        });
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();

        // onerror
        let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |e: ErrorEvent| {
            web_sys::console::error_1(&format!("[streamline-wasm] WebSocket error: {:?}", e.message()).into());
        });
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        // onclose
        let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |e: CloseEvent| {
            web_sys::console::log_1(
                &format!(
                    "[streamline-wasm] WebSocket closed: code={}, reason={}",
                    e.code(),
                    e.reason()
                )
                .into(),
            );
        });
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        onclose.forget();

        Ok(())
    }
}
