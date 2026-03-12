use wasm_bindgen::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

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

/// WebSocket connection manager with automatic reconnection.
///
/// When the connection drops unexpectedly the manager schedules a reconnect
/// using exponential backoff (1 s → 2 s → 4 s … capped at 30 s).
/// Set `auto_reconnect` to `false` to disable this behaviour.
#[wasm_bindgen]
pub struct WsConnection {
    url: String,
    ws: Option<WebSocket>,
    state: ConnectionState,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    auto_reconnect: bool,
    intentional_disconnect: bool,
    #[wasm_bindgen(skip)]
    pub on_message: Option<js_sys::Function>,
    #[wasm_bindgen(skip)]
    pub on_state_change: Option<js_sys::Function>,
    #[wasm_bindgen(skip)]
    pub on_reconnect_failed: Option<js_sys::Function>,
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
            auto_reconnect: true,
            intentional_disconnect: false,
            on_message: None,
            on_state_change: None,
            on_reconnect_failed: None,
        }
    }

    /// Enable or disable automatic reconnection (default: enabled).
    pub fn set_auto_reconnect(&mut self, enabled: bool) {
        self.auto_reconnect = enabled;
    }

    /// Returns `true` when automatic reconnection is enabled.
    pub fn auto_reconnect(&self) -> bool {
        self.auto_reconnect
    }

    /// Set the maximum number of reconnection attempts (default: 5).
    /// Set to `0` for unlimited attempts.
    pub fn set_max_reconnect_attempts(&mut self, max: u32) {
        self.max_reconnect_attempts = max;
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// How many reconnection attempts have been made since the last
    /// successful connection.
    pub fn reconnect_attempts(&self) -> u32 {
        self.reconnect_attempts
    }

    /// Connect to the Streamline server.
    pub fn connect(&mut self) -> Result<(), JsValue> {
        self.intentional_disconnect = false;
        self.set_state(ConnectionState::Connecting);

        let ws = WebSocket::new(&self.url)?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        self.setup_event_handlers(&ws)?;
        self.ws = Some(ws);
        Ok(())
    }

    /// Disconnect from the server. This is intentional and will **not**
    /// trigger automatic reconnection.
    pub fn disconnect(&mut self) {
        self.intentional_disconnect = true;
        if let Some(ref ws) = self.ws {
            let _ = ws.close();
        }
        self.ws = None;
        self.reconnect_attempts = 0;
        self.set_state(ConnectionState::Disconnected);
    }

    /// Send a protocol message over the WebSocket.
    pub fn send(&self, message: &str) -> Result<(), JsValue> {
        match &self.ws {
            Some(ws) if ws.ready_state() == WebSocket::OPEN => ws.send_with_str(message),
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

    fn make_conn(attempts: u32, max: u32, auto: bool) -> WsConnection {
        WsConnection {
            url: "ws://localhost:9094/ws".into(),
            ws: None,
            state: ConnectionState::Disconnected,
            reconnect_attempts: attempts,
            max_reconnect_attempts: max,
            auto_reconnect: auto,
            intentional_disconnect: false,
            on_message: None,
            on_state_change: None,
            on_reconnect_failed: None,
        }
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
    fn test_is_connected_when_connected() {
        let mut conn = make_conn(0, 5, true);
        conn.state = ConnectionState::Connected;
        assert!(conn.is_connected());
    }

    #[test]
    fn test_state_returns_current_state() {
        let mut conn = make_conn(0, 5, true);
        conn.state = ConnectionState::Reconnecting;
        assert_eq!(conn.state(), ConnectionState::Reconnecting);
    }

    #[test]
    fn test_set_max_reconnect_attempts() {
        let mut conn = make_conn(0, 5, true);
        conn.set_max_reconnect_attempts(10);
        assert_eq!(conn.max_reconnect_attempts, 10);
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
        let mut conn = make_conn(0, 5, true);
        conn.intentional_disconnect = true;
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

// Internal helper methods (not exported to JS).
impl WsConnection {
    /// Send a typed browser message (internal only — not exported to JS).
    pub(crate) fn send_message(&self, msg: &BrowserMessage) -> Result<(), JsValue> {
        let json = msg
            .to_json()
            .map_err(|e| StreamlineError::serialization(&e.to_string()))?;
        self.send(&json)
    }

    /// Whether a reconnection attempt should be scheduled.
    #[allow(dead_code)]
    pub(crate) fn should_reconnect(&self) -> bool {
        if !self.auto_reconnect || self.intentional_disconnect {
            return false;
        }
        // max_reconnect_attempts == 0 means unlimited
        self.max_reconnect_attempts == 0 || self.reconnect_attempts < self.max_reconnect_attempts
    }

    fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
        if let Some(ref cb) = self.on_state_change {
            let _ = cb.call1(&JsValue::NULL, &JsValue::from(format!("{state:?}")));
        }
    }

    fn setup_event_handlers(&self, ws: &WebSocket) -> Result<(), JsValue> {
        // Shared mutable state so closures can coordinate reconnection.
        // Rc<RefCell<…>> is safe here because WASM runs single-threaded.
        let shared_url = Rc::new(self.url.clone());
        let shared_state = Rc::new(RefCell::new(ConnectionState::Connecting));
        let shared_attempts = Rc::new(RefCell::new(self.reconnect_attempts));
        let shared_max = self.max_reconnect_attempts;
        let shared_auto = self.auto_reconnect;
        let shared_on_state = self.on_state_change.clone();
        let shared_on_msg = self.on_message.clone();
        let shared_on_fail = self.on_reconnect_failed.clone();

        // ── onopen ────────────────────────────────────────────────
        {
            let state = Rc::clone(&shared_state);
            let attempts = Rc::clone(&shared_attempts);
            let on_state = shared_on_state.clone();
            let onopen = Closure::<dyn FnMut()>::new(move || {
                *state.borrow_mut() = ConnectionState::Connected;
                *attempts.borrow_mut() = 0; // reset on success
                if let Some(ref cb) = on_state {
                    let _ = cb.call1(&JsValue::NULL, &JsValue::from("Connected"));
                }
                web_sys::console::log_1(&"[streamline-wasm] WebSocket connected".into());
            });
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            onopen.forget();
        }

        // ── onmessage ────────────────────────────────────────────
        {
            let on_msg_cb = shared_on_msg.clone();
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
        }

        // ── onerror ──────────────────────────────────────────────
        {
            let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |e: ErrorEvent| {
                web_sys::console::error_1(
                    &format!("[streamline-wasm] WebSocket error: {:?}", e.message()).into(),
                );
            });
            ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            onerror.forget();
        }

        // ── onclose (with auto-reconnect) ────────────────────────
        {
            let state = Rc::clone(&shared_state);
            let attempts = Rc::clone(&shared_attempts);
            let url = Rc::clone(&shared_url);
            let on_state = shared_on_state.clone();
            let on_msg_for_reconnect = shared_on_msg.clone();
            let on_fail = shared_on_fail.clone();

            let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |e: CloseEvent| {
                web_sys::console::log_1(
                    &format!(
                        "[streamline-wasm] WebSocket closed: code={}, reason={}",
                        e.code(),
                        e.reason()
                    )
                    .into(),
                );

                // Normal close (1000) or intentional — don't reconnect
                let is_normal_close = e.code() == 1000;

                let current_attempts = *attempts.borrow();
                let should_reconnect = shared_auto
                    && !is_normal_close
                    && (shared_max == 0 || current_attempts < shared_max);

                if should_reconnect {
                    *state.borrow_mut() = ConnectionState::Reconnecting;
                    if let Some(ref cb) = on_state {
                        let _ = cb.call1(&JsValue::NULL, &JsValue::from("Reconnecting"));
                    }

                    let delay_ms = {
                        let base = 1000u32;
                        let max_delay = 30_000u32;
                        base.saturating_mul(2u32.saturating_pow(current_attempts))
                            .min(max_delay)
                    };

                    *attempts.borrow_mut() = current_attempts + 1;

                    web_sys::console::log_1(
                        &format!(
                            "[streamline-wasm] Reconnecting in {}ms (attempt {}/{})",
                            delay_ms,
                            current_attempts + 1,
                            if shared_max == 0 {
                                "∞".to_string()
                            } else {
                                shared_max.to_string()
                            },
                        )
                        .into(),
                    );

                    // Schedule reconnection via setTimeout
                    let url_inner = Rc::clone(&url);
                    let state_inner = Rc::clone(&state);
                    let attempts_inner = Rc::clone(&attempts);
                    let on_state_inner = on_state.clone();
                    let on_msg_inner = on_msg_for_reconnect.clone();
                    let on_fail_inner = on_fail.clone();

                    let reconnect_cb = Closure::<dyn FnMut()>::new(move || {
                        match WebSocket::new(&url_inner) {
                            Ok(new_ws) => {
                                new_ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

                                // Re-wire onopen
                                {
                                    let s = Rc::clone(&state_inner);
                                    let a = Rc::clone(&attempts_inner);
                                    let osc = on_state_inner.clone();
                                    let onopen = Closure::<dyn FnMut()>::new(move || {
                                        *s.borrow_mut() = ConnectionState::Connected;
                                        *a.borrow_mut() = 0;
                                        if let Some(ref cb) = osc {
                                            let _ = cb
                                                .call1(&JsValue::NULL, &JsValue::from("Connected"));
                                        }
                                        web_sys::console::log_1(
                                            &"[streamline-wasm] Reconnected successfully".into(),
                                        );
                                    });
                                    new_ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
                                    onopen.forget();
                                }

                                // Re-wire onmessage
                                {
                                    let cb = on_msg_inner.clone();
                                    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(
                                        move |e: MessageEvent| {
                                            if let Ok(text) =
                                                e.data().dyn_into::<js_sys::JsString>()
                                            {
                                                let s: String = text.into();
                                                if let Some(ref f) = cb {
                                                    let _ =
                                                        f.call1(&JsValue::NULL, &JsValue::from(&s));
                                                }
                                            }
                                        },
                                    );
                                    new_ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                                    onmessage.forget();
                                }

                                // onerror — just log
                                {
                                    let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(
                                        move |e: ErrorEvent| {
                                            web_sys::console::error_1(
                                                &format!(
                                                    "[streamline-wasm] WebSocket error: {:?}",
                                                    e.message()
                                                )
                                                .into(),
                                            );
                                        },
                                    );
                                    new_ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                                    onerror.forget();
                                }
                            }
                            Err(err) => {
                                web_sys::console::error_1(
                                    &format!("[streamline-wasm] Reconnect failed: {:?}", err)
                                        .into(),
                                );
                                *state_inner.borrow_mut() = ConnectionState::Disconnected;
                                if let Some(ref cb) = on_fail_inner {
                                    let _ = cb
                                        .call1(&JsValue::NULL, &JsValue::from("Reconnect failed"));
                                }
                            }
                        }
                    });

                    // Use the browser's setTimeout for the delay
                    let window = web_sys::window();
                    if let Some(win) = window {
                        let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
                            reconnect_cb.as_ref().unchecked_ref(),
                            delay_ms as i32,
                        );
                    }
                    reconnect_cb.forget();
                } else {
                    *state.borrow_mut() = ConnectionState::Disconnected;
                    if let Some(ref cb) = on_state {
                        let _ = cb.call1(&JsValue::NULL, &JsValue::from("Disconnected"));
                    }

                    // Notify if reconnection exhausted
                    if shared_auto
                        && !is_normal_close
                        && current_attempts >= shared_max
                        && shared_max > 0
                    {
                        web_sys::console::error_1(
                            &format!(
                                "[streamline-wasm] Max reconnection attempts ({}) exhausted",
                                shared_max,
                            )
                            .into(),
                        );
                        if let Some(ref cb) = on_fail {
                            let _ = cb.call1(
                                &JsValue::NULL,
                                &JsValue::from("Max reconnection attempts exhausted"),
                            );
                        }
                    }
                }
            });
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            onclose.forget();
        }

        Ok(())
    }
}
