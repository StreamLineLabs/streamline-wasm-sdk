use wasm_bindgen::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use crate::error::StreamlineError;
use crate::protocol::BrowserMessage;

/// Best-effort extraction of the `topic` field from a raw protocol message,
/// used to demultiplex incoming messages to the right per-topic callback.
/// Returns `None` for malformed JSON or messages without a `topic` field
/// (e.g. `topic_list`/`error` admin responses).
fn extract_topic(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .get("topic")?
        .as_str()
        .map(str::to_string)
}

/// Connection state for the WebSocket transport.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

/// WebSocket connection manager with automatic reconnection.
///
/// When the connection drops unexpectedly the manager schedules a reconnect
/// using exponential backoff (1 s → 2 s → 4 s … capped at 30 s).
/// Set `auto_reconnect` to `false` to disable this behaviour.
#[wasm_bindgen]
pub struct WsConnection {
    inner: Rc<RefCell<WsConnectionInner>>,
}

struct WsConnectionInner {
    url: String,
    ws: Option<WebSocket>,
    state: ConnectionState,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    auto_reconnect: bool,
    intentional_disconnect: bool,
    persistent_messages: Vec<(String, String)>,
    /// Per-topic subscription callbacks, keyed by topic name. Demultiplexed
    /// by inspecting the `topic` field of each incoming message so that
    /// later `subscribe()` calls for a *different* topic do not clobber
    /// earlier ones on the same connection.
    topic_callbacks: Vec<(String, js_sys::Function)>,
    /// Fallback callback used for messages that carry no `topic` field
    /// (e.g. admin responses such as `topic_list`/`error`/`ack`).
    on_message: Option<js_sys::Function>,
    on_state_change: Option<js_sys::Function>,
    on_reconnect_failed: Option<js_sys::Function>,
}

#[wasm_bindgen]
impl WsConnection {
    /// Create a new WebSocket connection manager.
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str) -> Self {
        Self {
            inner: Rc::new(RefCell::new(WsConnectionInner {
                url: url.to_string(),
                ws: None,
                state: ConnectionState::Disconnected,
                reconnect_attempts: 0,
                max_reconnect_attempts: 5,
                auto_reconnect: true,
                intentional_disconnect: false,
                persistent_messages: Vec::new(),
                topic_callbacks: Vec::new(),
                on_message: None,
                on_state_change: None,
                on_reconnect_failed: None,
            })),
        }
    }

    /// Enable or disable automatic reconnection (default: enabled).
    pub fn set_auto_reconnect(&mut self, enabled: bool) {
        self.inner.borrow_mut().auto_reconnect = enabled;
    }

    /// Returns `true` when automatic reconnection is enabled.
    pub fn auto_reconnect(&self) -> bool {
        self.inner.borrow().auto_reconnect
    }

    /// Set the maximum number of reconnection attempts (default: 5).
    /// Set to `0` for unlimited attempts.
    pub fn set_max_reconnect_attempts(&mut self, max: u32) {
        self.inner.borrow_mut().max_reconnect_attempts = max;
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.inner.borrow().state
    }

    /// How many reconnection attempts have been made since the last
    /// successful connection.
    pub fn reconnect_attempts(&self) -> u32 {
        self.inner.borrow().reconnect_attempts
    }

    /// Connect to the Streamline server.
    pub fn connect(&mut self) -> Result<(), JsValue> {
        {
            let mut inner = self.inner.borrow_mut();
            if inner.ws.as_ref().is_some_and(|ws| {
                matches!(ws.ready_state(), WebSocket::CONNECTING | WebSocket::OPEN)
            }) {
                return Ok(());
            }
            inner.intentional_disconnect = false;
        }
        Self::set_state(&self.inner, ConnectionState::Connecting);
        {
            let inner = self.inner.borrow();
            if inner.intentional_disconnect || inner.state != ConnectionState::Connecting {
                return Ok(());
            }
        }

        let url = self.inner.borrow().url.clone();
        let ws = match WebSocket::new(&url) {
            Ok(ws) => ws,
            Err(error) => {
                Self::set_state(&self.inner, ConnectionState::Disconnected);
                return Err(error);
            }
        };
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        Self::setup_event_handlers(&self.inner, &ws);
        self.inner.borrow_mut().ws = Some(ws);
        Ok(())
    }

    /// Disconnect from the server. This is intentional and will **not**
    /// trigger automatic reconnection.
    pub fn disconnect(&mut self) {
        let ws = {
            let mut inner = self.inner.borrow_mut();
            inner.intentional_disconnect = true;
            inner.reconnect_attempts = 0;
            inner.persistent_messages.clear();
            inner.topic_callbacks.clear();
            inner.ws.take()
        };
        if let Some(ws) = ws {
            let _ = ws.close();
        }
        Self::set_state(&self.inner, ConnectionState::Disconnected);
    }

    /// Send a protocol message over the WebSocket.
    pub fn send(&self, message: &str) -> Result<(), JsValue> {
        let ws = self.inner.borrow().ws.clone();
        match ws {
            Some(ws) if ws.ready_state() == WebSocket::OPEN => ws.send_with_str(message),
            _ => Err(StreamlineError::not_connected()),
        }
    }

    /// Check whether the connection is open.
    pub fn is_connected(&self) -> bool {
        let inner = self.inner.borrow();
        inner.state == ConnectionState::Connected
            && inner
                .ws
                .as_ref()
                .is_some_and(|ws| ws.ready_state() == WebSocket::OPEN)
    }

    /// Calculate reconnection delay with exponential backoff (in ms).
    pub fn reconnect_delay_ms(&self) -> u32 {
        let base_ms = 1000u32;
        let max_ms = 30_000u32;
        let attempts = self.inner.borrow().reconnect_attempts;
        let delay = base_ms.saturating_mul(2u32.saturating_pow(attempts));
        delay.min(max_ms)
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

    pub(crate) fn send_persistent_when_ready(
        &self,
        key: String,
        message: String,
    ) -> Result<(), JsValue> {
        let ready_socket = {
            let mut inner = self.inner.borrow_mut();
            let ready_socket = inner
                .ws
                .as_ref()
                .filter(|ws| ws.ready_state() == WebSocket::OPEN)
                .cloned();
            match inner.state {
                ConnectionState::Connected if ready_socket.is_some() => {}
                ConnectionState::Connecting | ConnectionState::Reconnecting => {}
                _ => return Err(StreamlineError::not_connected()),
            }
            if let Some((_, stored_message)) = inner
                .persistent_messages
                .iter_mut()
                .find(|(stored_key, _)| stored_key == &key)
            {
                *stored_message = message.clone();
            } else {
                inner.persistent_messages.push((key, message.clone()));
            }
            ready_socket
        };

        match ready_socket {
            Some(ws) => ws.send_with_str(&message),
            None => Ok(()),
        }
    }

    pub(crate) fn remove_persistent_message(&self, key: &str) {
        self.inner
            .borrow_mut()
            .persistent_messages
            .retain(|(stored_key, _)| stored_key != key);
    }

    pub(crate) fn set_on_message(&mut self, callback: js_sys::Function) {
        self.inner.borrow_mut().on_message = Some(callback);
    }

    /// Register (or replace) the callback for a specific topic. Unlike
    /// [`set_on_message`], multiple topics can each own a distinct callback
    /// on the same connection — registering a callback for `topic_b` does
    /// not affect a previously registered callback for `topic_a`.
    pub(crate) fn set_topic_callback(&self, topic: &str, callback: js_sys::Function) {
        let mut inner = self.inner.borrow_mut();
        if let Some((_, stored)) = inner
            .topic_callbacks
            .iter_mut()
            .find(|(stored_topic, _)| stored_topic == topic)
        {
            *stored = callback;
        } else {
            inner.topic_callbacks.push((topic.to_string(), callback));
        }
    }

    /// Remove the callback registered for a specific topic, if any. Other
    /// topics' callbacks on the same connection are left untouched.
    pub(crate) fn remove_topic_callback(&self, topic: &str) {
        self.inner
            .borrow_mut()
            .topic_callbacks
            .retain(|(stored_topic, _)| stored_topic != topic);
    }

    pub(crate) fn set_on_state_change(&mut self, callback: js_sys::Function) {
        let state = {
            let mut inner = self.inner.borrow_mut();
            inner.on_state_change = Some(callback.clone());
            inner.state
        };
        let _ = callback.call1(&JsValue::NULL, &JsValue::from(Self::state_name(state)));
    }

    pub(crate) fn set_on_reconnect_failed(&mut self, callback: js_sys::Function) {
        self.inner.borrow_mut().on_reconnect_failed = Some(callback);
    }

    /// Whether a reconnection attempt should be scheduled.
    #[allow(dead_code)]
    pub(crate) fn should_reconnect(&self) -> bool {
        let inner = self.inner.borrow();
        if !inner.auto_reconnect || inner.intentional_disconnect {
            return false;
        }
        // max_reconnect_attempts == 0 means unlimited
        inner.max_reconnect_attempts == 0 || inner.reconnect_attempts < inner.max_reconnect_attempts
    }

    fn state_name(state: ConnectionState) -> &'static str {
        match state {
            ConnectionState::Disconnected => "Disconnected",
            ConnectionState::Connecting => "Connecting",
            ConnectionState::Connected => "Connected",
            ConnectionState::Reconnecting => "Reconnecting",
        }
    }

    fn set_state(shared: &Rc<RefCell<WsConnectionInner>>, state: ConnectionState) {
        let callback = {
            let mut inner = shared.borrow_mut();
            inner.state = state;
            inner.on_state_change.clone()
        };
        if let Some(callback) = callback {
            let _ = callback.call1(&JsValue::NULL, &JsValue::from(Self::state_name(state)));
        }
    }

    fn setup_event_handlers(shared: &Rc<RefCell<WsConnectionInner>>, ws: &WebSocket) {
        // ── onopen ────────────────────────────────────────────────
        {
            let weak = Rc::downgrade(shared);
            let socket = ws.clone();
            let onopen = Closure::<dyn FnMut()>::new(move || {
                let Some(shared) = weak.upgrade() else {
                    return;
                };
                let (callback, persistent_messages) = {
                    let mut inner = shared.borrow_mut();
                    let is_current = inner.ws.as_ref().is_some_and(|ws| ws == &socket);
                    if !is_current || inner.intentional_disconnect {
                        return;
                    }
                    inner.state = ConnectionState::Connected;
                    inner.reconnect_attempts = 0;
                    (
                        inner.on_state_change.clone(),
                        inner
                            .persistent_messages
                            .iter()
                            .map(|(_, message)| message.clone())
                            .collect::<Vec<_>>(),
                    )
                };
                for message in persistent_messages {
                    if socket.ready_state() != WebSocket::OPEN {
                        break;
                    }
                    if let Err(error) = socket.send_with_str(&message) {
                        web_sys::console::error_1(
                            &format!(
                                "[streamline-wasm] Failed to restore persistent message: {:?}",
                                error
                            )
                            .into(),
                        );
                        break;
                    }
                }
                if let Some(callback) = callback {
                    let _ = callback.call1(&JsValue::NULL, &JsValue::from("Connected"));
                }
                web_sys::console::log_1(&"[streamline-wasm] WebSocket connected".into());
            });
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            onopen.forget();
        }

        // ── onmessage ────────────────────────────────────────────
        {
            let weak = Rc::downgrade(shared);
            let socket = ws.clone();
            let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                let Some(shared) = weak.upgrade() else {
                    return;
                };
                let Some(message) = e.data().as_string() else {
                    return;
                };
                // Demultiplex by the message's `topic` field so each
                // subscribed topic keeps its own callback — later
                // subscriptions never overwrite earlier ones. Messages
                // without a topic (admin/ack/list responses) fall back to
                // the connection's default callback.
                let topic = extract_topic(&message);
                let callback = {
                    let inner = shared.borrow();
                    if !inner.ws.as_ref().is_some_and(|ws| ws == &socket) {
                        return;
                    }
                    topic
                        .as_deref()
                        .and_then(|topic| {
                            inner
                                .topic_callbacks
                                .iter()
                                .find(|(stored_topic, _)| stored_topic == topic)
                                .map(|(_, callback)| callback.clone())
                        })
                        .or_else(|| inner.on_message.clone())
                };
                if let Some(callback) = callback {
                    let _ = callback.call1(&JsValue::NULL, &JsValue::from(message));
                }
            });
            ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            onmessage.forget();
        }

        // ── onerror ──────────────────────────────────────────────
        {
            let weak = Rc::downgrade(shared);
            let socket = ws.clone();
            let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |e: ErrorEvent| {
                let Some(shared) = weak.upgrade() else {
                    return;
                };
                if !shared.borrow().ws.as_ref().is_some_and(|ws| ws == &socket) {
                    return;
                }
                web_sys::console::error_1(
                    &format!("[streamline-wasm] WebSocket error: {:?}", e.message()).into(),
                );
            });
            ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            onerror.forget();
        }

        // ── onclose (with auto-reconnect) ────────────────────────
        {
            let weak = Rc::downgrade(shared);
            let socket = ws.clone();
            let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |e: CloseEvent| {
                let Some(shared) = weak.upgrade() else {
                    return;
                };
                web_sys::console::log_1(
                    &format!(
                        "[streamline-wasm] WebSocket closed: code={}, reason={}",
                        e.code(),
                        e.reason()
                    )
                    .into(),
                );

                let intentional_disconnect = {
                    let mut inner = shared.borrow_mut();
                    if !inner.ws.as_ref().is_some_and(|ws| ws == &socket) {
                        return;
                    }
                    inner.ws = None;
                    inner.intentional_disconnect
                };

                // Only *our own* call to `disconnect()` should suppress
                // reconnection. Close code 1000 ("Normal Closure") is not by
                // itself evidence of that — a peer (server restart, load
                // balancer, idle-timeout, etc.) can close cleanly with code
                // 1000 while the caller still wants the connection kept
                // alive. Treating 1000 as always-intentional silently
                // stopped `auto_reconnect` from ever kicking in for the most
                // common "server closed the socket" case.
                if intentional_disconnect {
                    Self::set_state(&shared, ConnectionState::Disconnected);
                    return;
                }

                Self::schedule_reconnect(&shared);
            });
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            onclose.forget();
        }
    }

    fn schedule_reconnect(shared: &Rc<RefCell<WsConnectionInner>>) {
        let (current_attempts, max_attempts, auto_reconnect, intentional_disconnect) = {
            let inner = shared.borrow();
            (
                inner.reconnect_attempts,
                inner.max_reconnect_attempts,
                inner.auto_reconnect,
                inner.intentional_disconnect,
            )
        };

        if !auto_reconnect || intentional_disconnect {
            Self::set_state(shared, ConnectionState::Disconnected);
            return;
        }

        if max_attempts > 0 && current_attempts >= max_attempts {
            web_sys::console::error_1(
                &format!(
                    "[streamline-wasm] Max reconnection attempts ({}) exhausted",
                    max_attempts
                )
                .into(),
            );
            Self::fail_reconnect(shared, "Max reconnection attempts exhausted");
            return;
        }

        let delay_ms = 1000u32
            .saturating_mul(2u32.saturating_pow(current_attempts))
            .min(30_000);
        let attempt = current_attempts + 1;
        {
            let mut inner = shared.borrow_mut();
            inner.reconnect_attempts = attempt;
        }
        Self::set_state(shared, ConnectionState::Reconnecting);

        web_sys::console::log_1(
            &format!(
                "[streamline-wasm] Reconnecting in {}ms (attempt {}/{})",
                delay_ms,
                attempt,
                if max_attempts == 0 {
                    "∞".to_string()
                } else {
                    max_attempts.to_string()
                },
            )
            .into(),
        );

        let weak = Rc::downgrade(shared);
        let reconnect = Closure::<dyn FnMut()>::new(move || {
            let Some(shared) = weak.upgrade() else {
                return;
            };
            let (url, should_continue, should_mark_disconnected) = {
                let inner = shared.borrow();
                let waiting_to_reconnect =
                    inner.state == ConnectionState::Reconnecting && inner.ws.is_none();
                (
                    inner.url.clone(),
                    inner.auto_reconnect && !inner.intentional_disconnect && waiting_to_reconnect,
                    waiting_to_reconnect && (!inner.auto_reconnect || inner.intentional_disconnect),
                )
            };
            if !should_continue {
                if should_mark_disconnected {
                    Self::set_state(&shared, ConnectionState::Disconnected);
                }
                return;
            }

            match WebSocket::new(&url) {
                Ok(ws) => {
                    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
                    Self::setup_event_handlers(&shared, &ws);
                    shared.borrow_mut().ws = Some(ws);
                }
                Err(error) => {
                    web_sys::console::error_1(
                        &format!("[streamline-wasm] Reconnect failed: {:?}", error).into(),
                    );
                    Self::schedule_reconnect(&shared);
                }
            }
        });

        let Some(window) = web_sys::window() else {
            Self::fail_reconnect(shared, "Browser timer unavailable");
            return;
        };
        match window.set_timeout_with_callback_and_timeout_and_arguments_0(
            reconnect.as_ref().unchecked_ref(),
            delay_ms as i32,
        ) {
            Ok(_) => reconnect.forget(),
            Err(error) => {
                web_sys::console::error_1(
                    &format!(
                        "[streamline-wasm] Failed to schedule reconnect: {:?}",
                        error
                    )
                    .into(),
                );
                Self::fail_reconnect(shared, "Failed to schedule reconnect");
            }
        }
    }

    fn fail_reconnect(shared: &Rc<RefCell<WsConnectionInner>>, message: &str) {
        let callback = shared.borrow().on_reconnect_failed.clone();
        Self::set_state(shared, ConnectionState::Disconnected);
        if let Some(callback) = callback {
            let _ = callback.call1(&JsValue::NULL, &JsValue::from(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn(attempts: u32, max: u32, auto: bool) -> WsConnection {
        let conn = WsConnection::new("ws://localhost:9094/ws");
        {
            let mut inner = conn.inner.borrow_mut();
            inner.reconnect_attempts = attempts;
            inner.max_reconnect_attempts = max;
            inner.auto_reconnect = auto;
        }
        conn
    }

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
        let conn = make_conn(0, 5, true);
        assert_eq!(conn.reconnect_delay_ms(), 1_000);
    }

    #[test]
    fn test_reconnect_delay_exponential_backoff() {
        let delays: Vec<u32> = (0..5)
            .map(|a| make_conn(a, 5, true).reconnect_delay_ms())
            .collect();
        assert_eq!(delays, vec![1_000, 2_000, 4_000, 8_000, 16_000]);
    }

    #[test]
    fn test_reconnect_delay_capped_at_30s() {
        let conn = make_conn(10, 5, true);
        assert_eq!(conn.reconnect_delay_ms(), 30_000);
    }

    #[test]
    fn test_reconnect_delay_large_attempts_no_overflow() {
        let conn = make_conn(100, 5, true);
        assert_eq!(conn.reconnect_delay_ms(), 30_000);
    }

    // ── WsConnection field defaults & auto-reconnect ─────────────────

    #[test]
    fn test_is_connected_when_disconnected() {
        let conn = make_conn(0, 5, true);
        assert!(!conn.is_connected());
    }

    #[test]
    fn test_is_connected_requires_open_socket() {
        let conn = make_conn(0, 5, true);
        conn.inner.borrow_mut().state = ConnectionState::Connected;
        assert!(!conn.is_connected());
    }

    #[test]
    fn test_state_returns_current_state() {
        let conn = make_conn(0, 5, true);
        conn.inner.borrow_mut().state = ConnectionState::Reconnecting;
        assert_eq!(conn.state(), ConnectionState::Reconnecting);
    }

    #[test]
    fn test_set_max_reconnect_attempts() {
        let mut conn = make_conn(0, 5, true);
        conn.set_max_reconnect_attempts(10);
        assert_eq!(conn.inner.borrow().max_reconnect_attempts, 10);
    }

    #[test]
    fn test_auto_reconnect_default_enabled() {
        let conn = make_conn(0, 5, true);
        assert!(conn.auto_reconnect());
    }

    #[test]
    fn test_auto_reconnect_can_be_disabled() {
        let mut conn = make_conn(0, 5, true);
        conn.set_auto_reconnect(false);
        assert!(!conn.auto_reconnect());
    }

    #[test]
    fn test_should_reconnect_when_enabled_and_under_limit() {
        let conn = make_conn(2, 5, true);
        assert!(conn.should_reconnect());
    }

    #[test]
    fn test_should_not_reconnect_when_disabled() {
        let conn = make_conn(0, 5, false);
        assert!(!conn.should_reconnect());
    }

    #[test]
    fn test_should_not_reconnect_after_intentional_disconnect() {
        let conn = make_conn(0, 5, true);
        conn.inner.borrow_mut().intentional_disconnect = true;
        assert!(!conn.should_reconnect());
    }

    #[test]
    fn test_should_not_reconnect_when_max_attempts_reached() {
        let conn = make_conn(5, 5, true);
        assert!(!conn.should_reconnect());
    }

    #[test]
    fn test_should_reconnect_unlimited_when_max_is_zero() {
        let conn = make_conn(999, 0, true);
        assert!(conn.should_reconnect());
    }

    #[test]
    fn test_reconnect_attempts_getter() {
        let conn = make_conn(3, 5, true);
        assert_eq!(conn.reconnect_attempts(), 3);
    }
}
