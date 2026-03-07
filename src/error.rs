//! Typed error types exported to JavaScript via wasm_bindgen.
//!
//! These replace opaque `JsValue::from_str(...)` errors with structured objects
//! that JavaScript consumers can inspect programmatically.

use wasm_bindgen::prelude::*;

/// Error codes matching common Streamline failure modes.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// The WebSocket is not connected.
    NotConnected,
    /// A connection attempt failed.
    ConnectionFailed,
    /// Message serialization failed.
    SerializationError,
    /// The requested topic does not exist.
    TopicNotFound,
    /// Authentication was rejected by the server.
    AuthenticationFailed,
    /// The operation timed out.
    Timeout,
    /// A produce operation failed.
    ProduceError,
    /// An admin operation failed.
    AdminError,
    /// A query operation failed.
    QueryError,
    /// A schema registry operation failed.
    SchemaRegistryError,
    /// An unknown error occurred.
    Unknown,
}

/// Structured error type exposed to JavaScript.
///
/// ```js
/// try {
///   client.produce("topic", "value");
/// } catch (e) {
///   if (e.retryable) {
///     // retry logic
///   }
///   console.error(`[${e.code}] ${e.message}`);
/// }
/// ```
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct StreamlineError {
    code: ErrorCode,
    message: String,
    retryable: bool,
}

#[wasm_bindgen]
impl StreamlineError {
    /// Create a new StreamlineError.
    #[wasm_bindgen(constructor)]
    pub fn new(code: ErrorCode, message: &str, retryable: bool) -> Self {
        Self {
            code,
            message: message.to_string(),
            retryable,
        }
    }

    /// The error code identifying the failure category.
    #[wasm_bindgen(getter)]
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// A human-readable description of the error.
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    /// Whether this error is transient and the operation can be retried.
    #[wasm_bindgen(getter)]
    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

impl StreamlineError {
    pub fn not_connected() -> JsValue {
        Self::new(ErrorCode::NotConnected, "WebSocket is not connected", true).into()
    }

    pub fn serialization(detail: &str) -> JsValue {
        Self::new(
            ErrorCode::SerializationError,
            &format!("serialization error: {detail}"),
            false,
        )
        .into()
    }

    pub fn connection_failed(detail: &str) -> JsValue {
        Self::new(
            ErrorCode::ConnectionFailed,
            &format!("connection failed: {detail}"),
            true,
        )
        .into()
    }

    pub fn produce_error(detail: &str) -> JsValue {
        Self::new(
            ErrorCode::ProduceError,
            &format!("produce failed: {detail}"),
            true,
        )
        .into()
    }
}

impl From<StreamlineError> for JsValue {
    fn from(err: StreamlineError) -> JsValue {
        // Convert to a JsValue that JavaScript can catch and inspect.
        // The wasm_bindgen #[wasm_bindgen] attribute makes fields accessible.
        JsValue::from(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = StreamlineError::new(ErrorCode::NotConnected, "ws closed", true);
        assert_eq!(err.code(), ErrorCode::NotConnected);
        assert_eq!(err.message(), "ws closed");
        assert!(err.retryable());
    }

    #[test]
    fn test_non_retryable_error() {
        let err = StreamlineError::new(ErrorCode::SerializationError, "bad json", false);
        assert!(!err.retryable());
        assert_eq!(err.code(), ErrorCode::SerializationError);
    }

    #[test]
    fn test_error_codes_distinct() {
        assert_ne!(ErrorCode::NotConnected, ErrorCode::Timeout);
        assert_ne!(ErrorCode::ProduceError, ErrorCode::AdminError);
    }
}
