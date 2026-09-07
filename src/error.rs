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
    /// A configuration or validation error.
    ConfigurationError,
    /// The requested operation is not implemented by this SDK version and
    /// was refused (fail-closed) rather than silently doing something the
    /// SDK cannot verify — e.g. offset commit without a broker
    /// acknowledgement protocol.
    Unsupported,
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

    pub fn timeout(detail: &str) -> JsValue {
        Self::new(
            ErrorCode::Timeout,
            &format!("operation timed out: {detail}"),
            true,
        )
        .into()
    }

    pub fn configuration(detail: &str) -> JsValue {
        Self::new(ErrorCode::ConfigurationError, detail, false).into()
    }

    /// Fail-closed error for operations this SDK version deliberately
    /// refuses to perform rather than claim an unverifiable result (e.g.
    /// committing consumer offsets without a broker acknowledgement
    /// protocol). Never retryable — retrying will not make the operation
    /// supported.
    pub fn unsupported(detail: &str) -> JsValue {
        Self::new(ErrorCode::Unsupported, detail, false).into()
    }

    pub fn auth_failed(detail: &str) -> JsValue {
        Self::new(
            ErrorCode::AuthenticationFailed,
            &format!("authentication failed: {detail}"),
            false,
        )
        .into()
    }

    /// Returns a resolution hint based on the error code.
    pub fn hint(&self) -> String {
        match self.code {
            ErrorCode::NotConnected => "Call connect() before performing operations".to_string(),
            ErrorCode::ConnectionFailed => {
                "Check that the Streamline server is running and the URL is correct".to_string()
            }
            ErrorCode::Timeout => {
                "Consider increasing timeout settings or checking server load".to_string()
            }
            ErrorCode::AuthenticationFailed => {
                "Verify your credentials and authentication configuration".to_string()
            }
            ErrorCode::TopicNotFound => "Create the topic first using the admin API".to_string(),
            ErrorCode::SerializationError => {
                "Check that the message value is valid JSON".to_string()
            }
            ErrorCode::ProduceError => {
                "Check that the topic exists, you have write permissions, and the message size is within limits".to_string()
            }
            ErrorCode::AdminError => {
                "Verify you have admin permissions and the server is accepting admin operations".to_string()
            }
            ErrorCode::QueryError => {
                "Check your SQL syntax and ensure the queried topics exist".to_string()
            }
            ErrorCode::SchemaRegistryError => {
                "Verify the schema format is valid and the registry endpoint is configured".to_string()
            }
            ErrorCode::ConfigurationError => {
                "Check the provided configuration values (e.g. topic names, parameters)".to_string()
            }
            ErrorCode::Unsupported => {
                "This SDK version does not implement this operation; it fails closed instead of guessing. Check the CHANGELOG for protocol support status".to_string()
            }
            _ => "Check server logs for more details".to_string(),
        }
    }

    /// Returns true if this error is transient and the operation can be retried.
    pub fn is_retryable(&self) -> bool {
        self.retryable
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

    #[test]
    fn test_is_retryable() {
        let retryable = StreamlineError::new(ErrorCode::ConnectionFailed, "conn err", true);
        assert!(retryable.is_retryable());

        let not_retryable =
            StreamlineError::new(ErrorCode::AuthenticationFailed, "bad auth", false);
        assert!(!not_retryable.is_retryable());
    }

    #[test]
    fn test_hint_for_known_codes() {
        let err = StreamlineError::new(ErrorCode::NotConnected, "not connected", true);
        assert!(err.hint().contains("connect()"));

        let err = StreamlineError::new(ErrorCode::TopicNotFound, "no topic", false);
        assert!(err.hint().contains("admin API"));
    }

    #[test]
    fn test_hint_for_unknown_codes() {
        let err = StreamlineError::new(ErrorCode::Unknown, "unknown", false);
        assert!(err.hint().contains("server logs"));
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_factory_methods() {
        // These return JsValue so we can't inspect deeply, but verify they don't panic
        let _ = StreamlineError::not_connected();
        let _ = StreamlineError::serialization("bad data");
        let _ = StreamlineError::connection_failed("refused");
        let _ = StreamlineError::produce_error("full");
        let _ = StreamlineError::timeout("fetch");
        let _ = StreamlineError::auth_failed("bad creds");
        let _ = StreamlineError::unsupported("commit acknowledgement protocol not implemented");
    }

    // ── Additional coverage ──────────────────────────────────────────

    #[test]
    fn test_error_clone() {
        let err = StreamlineError::new(ErrorCode::Timeout, "timed out", true);
        let cloned = err.clone();
        assert_eq!(cloned.code(), ErrorCode::Timeout);
        assert_eq!(cloned.message(), "timed out");
        assert!(cloned.retryable());
    }

    #[test]
    fn test_error_debug_format() {
        let err = StreamlineError::new(ErrorCode::Unknown, "mystery", false);
        let dbg = format!("{:?}", err);
        assert!(dbg.contains("StreamlineError"));
        assert!(dbg.contains("Unknown"));
    }

    #[test]
    fn test_error_code_debug_format() {
        let dbg = format!("{:?}", ErrorCode::ConnectionFailed);
        assert_eq!(dbg, "ConnectionFailed");
    }

    #[test]
    // Intentionally exercises both the derived `Copy` and `Clone` impls.
    #[allow(clippy::clone_on_copy)]
    fn test_error_code_clone_copy() {
        let code = ErrorCode::ProduceError;
        let copied = code;
        let cloned = code.clone();
        assert_eq!(code, copied);
        assert_eq!(code, cloned);
    }

    #[test]
    fn test_hint_connection_failed() {
        let err = StreamlineError::new(ErrorCode::ConnectionFailed, "refused", true);
        assert!(err.hint().contains("server is running"));
    }

    #[test]
    fn test_hint_timeout() {
        let err = StreamlineError::new(ErrorCode::Timeout, "slow", true);
        assert!(err.hint().contains("timeout"));
    }

    #[test]
    fn test_hint_auth_failed() {
        let err = StreamlineError::new(ErrorCode::AuthenticationFailed, "bad", false);
        assert!(err.hint().contains("credentials"));
    }

    #[test]
    fn test_hint_serialization_error() {
        let err = StreamlineError::new(ErrorCode::SerializationError, "bad json", false);
        assert!(err.hint().contains("JSON"));
    }

    #[test]
    fn test_hint_produce_error() {
        let err = StreamlineError::new(ErrorCode::ProduceError, "full", true);
        assert!(err.hint().contains("topic exists"));
    }

    #[test]
    fn test_hint_admin_error() {
        let err = StreamlineError::new(ErrorCode::AdminError, "fail", false);
        assert!(err.hint().contains("admin permissions"));
    }

    #[test]
    fn test_hint_query_error() {
        let err = StreamlineError::new(ErrorCode::QueryError, "fail", false);
        assert!(err.hint().contains("SQL syntax"));
    }

    #[test]
    fn test_hint_schema_registry_error() {
        let err = StreamlineError::new(ErrorCode::SchemaRegistryError, "fail", false);
        assert!(err.hint().contains("schema format"));
    }

    #[test]
    fn test_hint_configuration_error() {
        let err = StreamlineError::new(ErrorCode::ConfigurationError, "bad topic", false);
        assert!(err.hint().contains("configuration values"));
    }

    #[test]
    fn test_hint_unsupported() {
        let err = StreamlineError::new(ErrorCode::Unsupported, "commit not implemented", false);
        assert!(err.hint().contains("does not implement"));
    }

    #[test]
    fn test_unsupported_not_retryable() {
        let err = StreamlineError::new(ErrorCode::Unsupported, "commit not implemented", false);
        assert!(!err.is_retryable());
        assert_eq!(err.code(), ErrorCode::Unsupported);
    }

    #[test]
    fn test_configuration_error_not_retryable() {
        let err = StreamlineError::new(ErrorCode::ConfigurationError, "bad", false);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_all_error_codes_distinct() {
        let codes = [
            ErrorCode::NotConnected,
            ErrorCode::ConnectionFailed,
            ErrorCode::SerializationError,
            ErrorCode::TopicNotFound,
            ErrorCode::AuthenticationFailed,
            ErrorCode::Timeout,
            ErrorCode::ProduceError,
            ErrorCode::AdminError,
            ErrorCode::QueryError,
            ErrorCode::SchemaRegistryError,
            ErrorCode::ConfigurationError,
            ErrorCode::Unsupported,
            ErrorCode::Unknown,
        ];
        for i in 0..codes.len() {
            for j in (i + 1)..codes.len() {
                assert_ne!(codes[i], codes[j], "codes at {i} and {j} should differ");
            }
        }
    }
}
