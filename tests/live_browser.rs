//! Mandatory live-browser tests for a configured Streamline fixture.
//!
//! Run only through the `live-browser-tests` feature so the regular browser
//! suite remains self-contained:
//! `STREAMLINE_LIVE_WEBSOCKET_URL=ws://localhost:9094/ws wasm-pack test
//! --headless --chrome . --features live-browser-tests --test live_browser`

#![cfg(feature = "live-browser-tests")]

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use streamline_wasm_sdk::{BrowserResponse, ConnectionState, TopicAdmin};

wasm_bindgen_test_configure!(run_in_browser);

async fn sleep(milliseconds: u32) {
    let script = format!("return new Promise((resolve) => setTimeout(resolve, {milliseconds}));");
    let promise = js_sys::Function::new_no_args(&script)
        .call0(&wasm_bindgen::JsValue::NULL)
        .ok()
        .and_then(|value| value.dyn_into::<js_sys::Promise>().ok());
    if let Some(promise) = promise {
        let _ = JsFuture::from(promise).await;
    }
}

#[wasm_bindgen_test(async)]
async fn configured_fixture_accepts_browser_websocket_traffic() {
    let fixture_url = option_env!("STREAMLINE_LIVE_WEBSOCKET_URL");
    assert!(
        fixture_url.is_some(),
        "STREAMLINE_LIVE_WEBSOCKET_URL must be configured for the mandatory live-browser suite"
    );

    let mut admin = TopicAdmin::new(fixture_url.unwrap_or_default());
    let response_callback = js_sys::Function::new_with_args(
        "response",
        r#"
        try {
            if (JSON.parse(response).type === "topic_list") {
                globalThis.__streamlineLiveBrowserResponse = response;
            }
        } catch (_) {
            globalThis.__streamlineLiveBrowserInvalidResponse = response;
        }
        "#,
    );
    admin.on_response(response_callback);
    assert!(
        admin.connect().is_ok(),
        "fixture WebSocket constructor failed"
    );

    let state_callback = js_sys::Function::new_with_args(
        "state",
        "globalThis.__streamlineLiveBrowserState = state;",
    );
    admin.on_state_change(state_callback);

    for _ in 0..100 {
        if admin.is_connected() {
            break;
        }
        sleep(100).await;
    }

    let callback_state = js_sys::Reflect::get(
        &js_sys::global(),
        &wasm_bindgen::JsValue::from("__streamlineLiveBrowserState"),
    )
    .ok()
    .and_then(|value| value.as_string())
    .unwrap_or_default();

    assert!(admin.is_connected(), "fixture did not become ready in 10s");
    assert_eq!(admin.connection_state(), ConnectionState::Connected);
    assert_eq!(callback_state, "Connected");
    assert!(
        admin.list_topics().is_ok(),
        "fixture rejected a browser WebSocket protocol message"
    );

    let mut response = None;
    for _ in 0..100 {
        response = js_sys::Reflect::get(
            &js_sys::global(),
            &wasm_bindgen::JsValue::from("__streamlineLiveBrowserResponse"),
        )
        .ok()
        .and_then(|value| value.as_string());
        if response.is_some() {
            break;
        }
        sleep(100).await;
    }

    let parsed_response = response
        .as_deref()
        .map(BrowserResponse::from_json)
        .transpose();
    assert!(
        matches!(parsed_response, Ok(Some(BrowserResponse::TopicList { .. }))),
        "fixture did not return a valid topic_list response in 10s"
    );

    admin.disconnect();
    let _ = js_sys::Reflect::delete_property(
        &js_sys::global(),
        &wasm_bindgen::JsValue::from("__streamlineLiveBrowserState"),
    );
    let _ = js_sys::Reflect::delete_property(
        &js_sys::global(),
        &wasm_bindgen::JsValue::from("__streamlineLiveBrowserResponse"),
    );
    let _ = js_sys::Reflect::delete_property(
        &js_sys::global(),
        &wasm_bindgen::JsValue::from("__streamlineLiveBrowserInvalidResponse"),
    );
}
