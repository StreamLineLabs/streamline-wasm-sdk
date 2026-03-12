//! Browser-compatible telemetry for the Streamline WASM SDK.
//!
//! Provides lightweight performance instrumentation using browser-native APIs:
//!
//! - **`console.time` / `console.timeEnd`**: For timing produce and consume operations
//! - **`Performance.mark` / `Performance.measure`**: For high-resolution timing
//!   visible in browser DevTools Performance panel
//! - **W3C Trace Context headers**: Optional trace context propagation via
//!   `traceparent` / `tracestate` headers
//!
//! This module is always available (no feature gate) since it uses only browser
//! APIs that are universally supported.
//!
//! # Span Conventions
//!
//! Performance marks and measures follow the same naming conventions as the
//! server-side SDKs:
//!
//! - Name: `{topic} {operation}` (e.g., "orders produce", "events consume")
//! - Detail attributes: `messaging.system=streamline`,
//!   `messaging.destination.name={topic}`, `messaging.operation={operation}`
//!
//! # Example
//!
//! ```javascript
//! import { Telemetry } from '@streamlinelabs/streamline-wasm-sdk';
//!
//! const telemetry = new Telemetry();
//!
//! // Start timing a produce operation
//! const span = telemetry.start_produce("orders");
//! client.produce("orders", "message data");
//! telemetry.end_span(span);
//!
//! // Start timing a consume operation
//! const cspan = telemetry.start_consume("events");
//! // ... consume messages ...
//! telemetry.end_span(cspan);
//! ```

use wasm_bindgen::prelude::*;

/// A lightweight span handle for browser-based telemetry.
///
/// Holds the information needed to close the span (end the performance
/// measurement and console.timeEnd).
#[wasm_bindgen]
pub struct TelemetrySpan {
    label: String,
    mark_start: String,
    #[wasm_bindgen(skip)]
    pub topic: String,
    #[wasm_bindgen(skip)]
    pub operation: String,
}

#[wasm_bindgen]
impl TelemetrySpan {
    /// Returns the span label (e.g., "orders produce").
    #[wasm_bindgen(getter)]
    pub fn label(&self) -> String {
        self.label.clone()
    }

    /// Returns the topic associated with this span.
    #[wasm_bindgen(getter, js_name = "topic")]
    pub fn topic_js(&self) -> String {
        self.topic.clone()
    }

    /// Returns the operation type (produce, consume, process).
    #[wasm_bindgen(getter, js_name = "operation")]
    pub fn operation_js(&self) -> String {
        self.operation.clone()
    }
}

/// Browser-native telemetry for Streamline operations.
///
/// Uses `console.time`/`console.timeEnd` for console-visible timing and
/// `Performance.mark`/`Performance.measure` for DevTools integration.
#[wasm_bindgen]
pub struct Telemetry {
    enabled: bool,
}

#[wasm_bindgen]
impl Telemetry {
    /// Create a new Telemetry instance.
    ///
    /// Telemetry is enabled by default if `Performance` is available.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Create a disabled telemetry instance (all operations are no-ops).
    pub fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Returns whether telemetry is enabled.
    #[wasm_bindgen(getter)]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable telemetry.
    #[wasm_bindgen(setter)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Start timing a produce operation.
    ///
    /// Returns a `TelemetrySpan` handle that must be passed to `end_span()`
    /// when the operation completes.
    pub fn start_produce(&self, topic: &str) -> TelemetrySpan {
        self.start_span(topic, "produce")
    }

    /// Start timing a consume operation.
    pub fn start_consume(&self, topic: &str) -> TelemetrySpan {
        self.start_span(topic, "consume")
    }

    /// Start timing a record processing operation.
    pub fn start_process(&self, topic: &str) -> TelemetrySpan {
        self.start_span(topic, "process")
    }

    /// End a span, recording the performance measurement.
    ///
    /// This calls `console.timeEnd` and `Performance.measure` with the
    /// span label.
    pub fn end_span(&self, span: TelemetrySpan) {
        if !self.enabled {
            return;
        }

        // console.timeEnd
        web_sys::console::time_end_with_label(&span.label);

        // Performance.measure
        let mark_end = format!("{}-end", span.label);
        if let Some(performance) = get_performance() {
            performance.mark(&mark_end).ok();
            performance
                .measure_with_start_mark_and_end_mark(&span.label, &span.mark_start, &mark_end)
                .ok();
        }
    }

    /// End a span and record an error.
    pub fn end_span_with_error(&self, span: TelemetrySpan, error: &str) {
        if !self.enabled {
            return;
        }

        web_sys::console::time_end_with_label(&span.label);
        web_sys::console::error_1(&format!("[streamline] {} failed: {}", span.label, error).into());

        let mark_end = format!("{}-error", span.label);
        if let Some(performance) = get_performance() {
            performance.mark(&mark_end).ok();
            performance
                .measure_with_start_mark_and_end_mark(&span.label, &span.mark_start, &mark_end)
                .ok();
        }
    }

    // Internal helper to create a span.
    fn start_span(&self, topic: &str, operation: &str) -> TelemetrySpan {
        let label = format!("{} {}", topic, operation);
        let mark_start = format!("{}-start", label);

        if self.enabled {
            // console.time
            web_sys::console::time_with_label(&label);

            // Performance.mark
            if let Some(performance) = get_performance() {
                performance.mark(&mark_start).ok();
            }
        }

        TelemetrySpan {
            label,
            mark_start,
            topic: topic.to_string(),
            operation: operation.to_string(),
        }
    }
}

// ── W3C Trace Context Support ────────────────────────────────────────

/// Generate a W3C traceparent header value.
///
/// This creates a new random trace ID and span ID suitable for
/// propagating context through message headers. The format is:
/// `00-{trace-id}-{span-id}-01`
///
/// Note: In a browser environment, this uses `Math.random()` for ID
/// generation since `crypto.getRandomValues` may not always be available
/// in all WASM contexts.
#[wasm_bindgen]
pub fn generate_traceparent() -> String {
    let trace_id = generate_hex_id(32);
    let span_id = generate_hex_id(16);
    format!("00-{}-{}-01", trace_id, span_id)
}

/// Parse a traceparent header into its components.
///
/// Returns an object with `trace_id`, `span_id`, and `trace_flags` fields,
/// or `null` if the header is invalid.
#[wasm_bindgen]
pub fn parse_traceparent(traceparent: &str) -> JsValue {
    let parts: Vec<&str> = traceparent.split('-').collect();
    if parts.len() != 4 || parts[0] != "00" {
        return JsValue::NULL;
    }

    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"version".into(), &parts[0].into()).ok();
    js_sys::Reflect::set(&obj, &"trace_id".into(), &parts[1].into()).ok();
    js_sys::Reflect::set(&obj, &"span_id".into(), &parts[2].into()).ok();
    js_sys::Reflect::set(&obj, &"trace_flags".into(), &parts[3].into()).ok();
    obj.into()
}

// ── Internal helpers ─────────────────────────────────────────────────

fn get_performance() -> Option<web_sys::Performance> {
    web_sys::window().and_then(|w| w.performance())
}

fn generate_hex_id(len: usize) -> String {
    // Use js_sys::Math::random to generate hex characters
    let mut id = String::with_capacity(len);
    for _ in 0..len {
        let n = (js_sys::Math::random() * 16.0) as u8;
        id.push(char::from_digit(n as u32, 16).unwrap_or('0'));
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_span_fields() {
        let span = TelemetrySpan {
            label: "orders produce".to_string(),
            mark_start: "orders produce-start".to_string(),
            topic: "orders".to_string(),
            operation: "produce".to_string(),
        };
        assert_eq!(span.label(), "orders produce");
        assert_eq!(span.topic, "orders");
        assert_eq!(span.operation, "produce");
    }

    #[test]
    fn test_telemetry_disabled() {
        let telemetry = Telemetry::disabled();
        assert!(!telemetry.enabled());
    }

    #[test]
    fn test_telemetry_enabled_by_default() {
        let telemetry = Telemetry::new();
        assert!(telemetry.enabled());
    }

    #[test]
    fn test_telemetry_toggle() {
        let mut telemetry = Telemetry::new();
        telemetry.set_enabled(false);
        assert!(!telemetry.enabled());
        telemetry.set_enabled(true);
        assert!(telemetry.enabled());
    }

    // ── Span creation ────────────────────────────────────────────────

    #[test]
    fn test_start_produce_span_fields() {
        let telemetry = Telemetry::disabled();
        let span = telemetry.start_produce("orders");
        assert_eq!(span.label(), "orders produce");
        assert_eq!(span.topic, "orders");
        assert_eq!(span.operation, "produce");
        assert_eq!(span.mark_start, "orders produce-start");
    }

    #[test]
    fn test_start_consume_span_fields() {
        let telemetry = Telemetry::disabled();
        let span = telemetry.start_consume("events");
        assert_eq!(span.label(), "events consume");
        assert_eq!(span.topic, "events");
        assert_eq!(span.operation, "consume");
        assert_eq!(span.mark_start, "events consume-start");
    }

    #[test]
    fn test_start_process_span_fields() {
        let telemetry = Telemetry::disabled();
        let span = telemetry.start_process("metrics");
        assert_eq!(span.label(), "metrics process");
        assert_eq!(span.topic, "metrics");
        assert_eq!(span.operation, "process");
        assert_eq!(span.mark_start, "metrics process-start");
    }

    #[test]
    fn test_span_label_format_with_special_chars() {
        let telemetry = Telemetry::disabled();
        let span = telemetry.start_produce("my-topic.v2");
        assert_eq!(span.label(), "my-topic.v2 produce");
    }

    #[test]
    fn test_disabled_telemetry_still_creates_spans() {
        let telemetry = Telemetry::disabled();
        let span = telemetry.start_produce("t");
        // Span is created even when disabled (browser API calls are skipped)
        assert_eq!(span.topic, "t");
        assert_eq!(span.operation, "produce");
    }

    #[test]
    fn test_end_span_disabled_is_noop() {
        let telemetry = Telemetry::disabled();
        let span = telemetry.start_produce("t");
        // Should not panic even without browser APIs
        telemetry.end_span(span);
    }

    #[test]
    fn test_end_span_with_error_disabled_is_noop() {
        let telemetry = Telemetry::disabled();
        let span = telemetry.start_produce("t");
        // Should not panic even without browser APIs
        telemetry.end_span_with_error(span, "something broke");
    }

    #[test]
    fn test_multiple_spans_independent() {
        let telemetry = Telemetry::disabled();
        let span1 = telemetry.start_produce("topic-a");
        let span2 = telemetry.start_consume("topic-b");
        assert_ne!(span1.label(), span2.label());
        assert_ne!(span1.operation, span2.operation);
    }

    #[test]
    fn test_span_topic_js_getter() {
        let span = TelemetrySpan {
            label: "t produce".to_string(),
            mark_start: "t produce-start".to_string(),
            topic: "t".to_string(),
            operation: "produce".to_string(),
        };
        assert_eq!(span.topic_js(), "t");
    }

    #[test]
    fn test_span_operation_js_getter() {
        let span = TelemetrySpan {
            label: "t consume".to_string(),
            mark_start: "t consume-start".to_string(),
            topic: "t".to_string(),
            operation: "consume".to_string(),
        };
        assert_eq!(span.operation_js(), "consume");
    }
}
