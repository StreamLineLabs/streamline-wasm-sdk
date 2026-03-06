//! Schema Registry client for browser environments.
//!
//! Communicates with the Streamline Schema Registry HTTP API to register,
//! retrieve, and validate schemas. Supports Avro, Protobuf, and JSON Schema
//! formats.
//!
//! # Example
//!
//! ```javascript
//! import { SchemaRegistryClient } from '@streamlinelabs/streamline-wasm-sdk';
//!
//! const registry = new SchemaRegistryClient("http://localhost:9094");
//!
//! // Register a JSON schema
//! const id = await registry.register_schema("orders-value", schemaJson, "JSON");
//!
//! // Retrieve a schema by subject
//! const schema = await registry.get_latest_schema("orders-value");
//!
//! // Check compatibility before evolving
//! const compatible = await registry.check_compatibility("orders-value", newSchema);
//! ```

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

/// Schema format types supported by the Streamline Schema Registry.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFormat {
    Avro,
    Protobuf,
    Json,
}

impl SchemaFormat {
    fn as_str(&self) -> &'static str {
        match self {
            SchemaFormat::Avro => "AVRO",
            SchemaFormat::Protobuf => "PROTOBUF",
            SchemaFormat::Json => "JSON",
        }
    }
}

/// Schema metadata returned by the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    pub subject: String,
    pub id: u32,
    pub version: u32,
    pub schema_type: String,
    pub schema: String,
}

/// Compatibility check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompatibilityResult {
    is_compatible: bool,
}

/// Schema registration response.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegisterResponse {
    id: u32,
}

/// HTTP-based Schema Registry client for browser environments.
///
/// Connects to the Streamline server's Schema Registry API (default port 9094)
/// and provides schema management operations.
#[wasm_bindgen]
pub struct SchemaRegistryClient {
    base_url: String,
    auth_token: Option<String>,
}

#[wasm_bindgen]
impl SchemaRegistryClient {
    /// Create a new Schema Registry client.
    ///
    /// `base_url` should point to the Streamline HTTP API
    /// (e.g., `http://localhost:9094`).
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token: None,
        }
    }

    /// Set an authentication token for API requests.
    pub fn set_auth_token(&mut self, token: &str) {
        self.auth_token = Some(token.to_string());
    }

    /// Register a new schema under the given subject.
    ///
    /// Returns the schema ID assigned by the registry.
    pub async fn register_schema(
        &self,
        subject: &str,
        schema: &str,
        format: SchemaFormat,
    ) -> Result<u32, JsValue> {
        let url = format!(
            "{}/subjects/{}/versions",
            self.base_url,
            js_encode_uri_component(subject)
        );
        let body = serde_json::json!({
            "schemaType": format.as_str(),
            "schema": schema
        });
        let resp_text = self.http_post(&url, &body.to_string()).await?;
        let result: RegisterResponse = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        Ok(result.id)
    }

    /// Get the latest schema for a subject.
    ///
    /// Returns the schema as a JSON string with id, version, schema_type, and
    /// schema fields.
    pub async fn get_latest_schema(&self, subject: &str) -> Result<JsValue, JsValue> {
        let url = format!(
            "{}/subjects/{}/versions/latest",
            self.base_url,
            js_encode_uri_component(subject)
        );
        let resp_text = self.http_get(&url).await?;
        let info: SchemaInfo = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&info).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a specific version of a schema for a subject.
    pub async fn get_schema_version(
        &self,
        subject: &str,
        version: u32,
    ) -> Result<JsValue, JsValue> {
        let url = format!(
            "{}/subjects/{}/versions/{}",
            self.base_url,
            js_encode_uri_component(subject),
            version
        );
        let resp_text = self.http_get(&url).await?;
        let info: SchemaInfo = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&info).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a schema by its global ID.
    pub async fn get_schema_by_id(&self, id: u32) -> Result<JsValue, JsValue> {
        let url = format!("{}/schemas/ids/{}", self.base_url, id);
        let resp_text = self.http_get(&url).await?;
        let info: SchemaInfo = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&info).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all registered subjects.
    pub async fn list_subjects(&self) -> Result<JsValue, JsValue> {
        let url = format!("{}/subjects", self.base_url);
        let resp_text = self.http_get(&url).await?;
        let subjects: Vec<String> = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&subjects).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all versions for a subject.
    pub async fn list_versions(&self, subject: &str) -> Result<JsValue, JsValue> {
        let url = format!(
            "{}/subjects/{}/versions",
            self.base_url,
            js_encode_uri_component(subject)
        );
        let resp_text = self.http_get(&url).await?;
        let versions: Vec<u32> = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&versions).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Check if a schema is compatible with the latest version.
    ///
    /// Returns `true` if the new schema is backward-compatible.
    pub async fn check_compatibility(
        &self,
        subject: &str,
        schema: &str,
        format: SchemaFormat,
    ) -> Result<bool, JsValue> {
        let url = format!(
            "{}/compatibility/subjects/{}/versions/latest",
            self.base_url,
            js_encode_uri_component(subject)
        );
        let body = serde_json::json!({
            "schemaType": format.as_str(),
            "schema": schema
        });
        let resp_text = self.http_post(&url, &body.to_string()).await?;
        let result: CompatibilityResult = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        Ok(result.is_compatible)
    }

    /// Delete a subject and all its versions.
    pub async fn delete_subject(&self, subject: &str) -> Result<JsValue, JsValue> {
        let url = format!(
            "{}/subjects/{}",
            self.base_url,
            js_encode_uri_component(subject)
        );
        let resp_text = self.http_delete(&url).await?;
        let versions: Vec<u32> = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&versions).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Validate a message value against a registered schema.
    ///
    /// Client-side validation for JSON schemas only. Returns `true` if the
    /// value conforms to the schema.
    pub fn validate_json(&self, schema_str: &str, value: &str) -> Result<bool, JsValue> {
        let schema: serde_json::Value = serde_json::from_str(schema_str)
            .map_err(|e| JsValue::from_str(&format!("invalid schema JSON: {e}")))?;
        let val: serde_json::Value = serde_json::from_str(value)
            .map_err(|e| JsValue::from_str(&format!("invalid value JSON: {e}")))?;
        Ok(basic_json_validate(&schema, &val))
    }
}

impl SchemaRegistryClient {
    async fn http_get(&self, url: &str) -> Result<String, JsValue> {
        let mut opts = RequestInit::new();
        opts.method("GET");
        opts.mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(url, &opts)?;
        self.set_common_headers(&request)?;

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;

        if !resp.ok() {
            return Err(JsValue::from_str(&format!(
                "HTTP {}: {}",
                resp.status(),
                resp.status_text()
            )));
        }

        let text = JsFuture::from(resp.text()?).await?;
        text.as_string()
            .ok_or_else(|| JsValue::from_str("response is not a string"))
    }

    async fn http_post(&self, url: &str, body: &str) -> Result<String, JsValue> {
        let mut opts = RequestInit::new();
        opts.method("POST");
        opts.mode(RequestMode::Cors);
        opts.body(Some(&JsValue::from_str(body)));

        let request = Request::new_with_str_and_init(url, &opts)?;
        self.set_common_headers(&request)?;
        request
            .headers()
            .set("Content-Type", "application/json")?;

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;

        if !resp.ok() {
            return Err(JsValue::from_str(&format!(
                "HTTP {}: {}",
                resp.status(),
                resp.status_text()
            )));
        }

        let text = JsFuture::from(resp.text()?).await?;
        text.as_string()
            .ok_or_else(|| JsValue::from_str("response is not a string"))
    }

    async fn http_delete(&self, url: &str) -> Result<String, JsValue> {
        let mut opts = RequestInit::new();
        opts.method("DELETE");
        opts.mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(url, &opts)?;
        self.set_common_headers(&request)?;

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;

        if !resp.ok() {
            return Err(JsValue::from_str(&format!(
                "HTTP {}: {}",
                resp.status(),
                resp.status_text()
            )));
        }

        let text = JsFuture::from(resp.text()?).await?;
        text.as_string()
            .ok_or_else(|| JsValue::from_str("response is not a string"))
    }

    fn set_common_headers(&self, request: &Request) -> Result<(), JsValue> {
        request.headers().set("Accept", "application/json")?;
        if let Some(ref token) = self.auth_token {
            request
                .headers()
                .set("Authorization", &format!("Bearer {}", token))?;
        }
        Ok(())
    }
}

fn js_encode_uri_component(s: &str) -> String {
    js_sys::encode_uri_component(s).into()
}

/// Basic JSON validation: checks that all required properties in a JSON schema
/// object are present in the value. This is intentionally simple — full JSON
/// Schema validation is too heavy for WASM. Server-side validation handles
/// the complete spec.
fn basic_json_validate(schema: &serde_json::Value, value: &serde_json::Value) -> bool {
    if let Some(schema_type) = schema.get("type").and_then(|t| t.as_str()) {
        match schema_type {
            "object" => {
                if !value.is_object() {
                    return false;
                }
                if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                    let obj = value.as_object().unwrap_or(&serde_json::Map::new());
                    for field in required {
                        if let Some(field_name) = field.as_str() {
                            if !obj.contains_key(field_name) {
                                return false;
                            }
                        }
                    }
                }
            }
            "array" => {
                if !value.is_array() {
                    return false;
                }
            }
            "string" => {
                if !value.is_string() {
                    return false;
                }
            }
            "number" | "integer" => {
                if !value.is_number() {
                    return false;
                }
            }
            "boolean" => {
                if !value.is_boolean() {
                    return false;
                }
            }
            "null" => {
                if !value.is_null() {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_format_as_str() {
        assert_eq!(SchemaFormat::Avro.as_str(), "AVRO");
        assert_eq!(SchemaFormat::Protobuf.as_str(), "PROTOBUF");
        assert_eq!(SchemaFormat::Json.as_str(), "JSON");
    }

    #[test]
    fn test_schema_format_equality() {
        assert_eq!(SchemaFormat::Avro, SchemaFormat::Avro);
        assert_ne!(SchemaFormat::Avro, SchemaFormat::Json);
    }

    #[test]
    fn test_schema_info_serialization() {
        let info = SchemaInfo {
            subject: "orders-value".into(),
            id: 1,
            version: 1,
            schema_type: "JSON".into(),
            schema: r#"{"type":"object"}"#.into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"subject\":\"orders-value\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"version\":1"));
    }

    #[test]
    fn test_schema_info_deserialization() {
        let json = r#"{"subject":"test","id":42,"version":3,"schema_type":"AVRO","schema":"{}"}"#;
        let info: SchemaInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.subject, "test");
        assert_eq!(info.id, 42);
        assert_eq!(info.version, 3);
        assert_eq!(info.schema_type, "AVRO");
    }

    #[test]
    fn test_compatibility_result_deserialization() {
        let json = r#"{"is_compatible":true}"#;
        let result: CompatibilityResult = serde_json::from_str(json).unwrap();
        assert!(result.is_compatible);
    }

    #[test]
    fn test_register_response_deserialization() {
        let json = r#"{"id":7}"#;
        let result: RegisterResponse = serde_json::from_str(json).unwrap();
        assert_eq!(result.id, 7);
    }

    #[test]
    fn test_validate_json_object_with_required_fields() {
        let schema: serde_json::Value = serde_json::json!({
            "type": "object",
            "required": ["name", "age"]
        });
        let valid = serde_json::json!({"name": "Alice", "age": 30});
        let invalid = serde_json::json!({"name": "Alice"});
        assert!(basic_json_validate(&schema, &valid));
        assert!(!basic_json_validate(&schema, &invalid));
    }

    #[test]
    fn test_validate_json_type_checks() {
        let string_schema: serde_json::Value = serde_json::json!({"type": "string"});
        assert!(basic_json_validate(&string_schema, &serde_json::json!("hello")));
        assert!(!basic_json_validate(&string_schema, &serde_json::json!(42)));

        let number_schema: serde_json::Value = serde_json::json!({"type": "number"});
        assert!(basic_json_validate(&number_schema, &serde_json::json!(3.14)));
        assert!(!basic_json_validate(&number_schema, &serde_json::json!("text")));

        let bool_schema: serde_json::Value = serde_json::json!({"type": "boolean"});
        assert!(basic_json_validate(&bool_schema, &serde_json::json!(true)));
        assert!(!basic_json_validate(&bool_schema, &serde_json::json!("true")));

        let array_schema: serde_json::Value = serde_json::json!({"type": "array"});
        assert!(basic_json_validate(&array_schema, &serde_json::json!([1, 2])));
        assert!(!basic_json_validate(&array_schema, &serde_json::json!("not array")));

        let null_schema: serde_json::Value = serde_json::json!({"type": "null"});
        assert!(basic_json_validate(&null_schema, &serde_json::Value::Null));
        assert!(!basic_json_validate(&null_schema, &serde_json::json!(0)));
    }

    #[test]
    fn test_validate_json_no_type_always_passes() {
        let schema: serde_json::Value = serde_json::json!({});
        assert!(basic_json_validate(&schema, &serde_json::json!("anything")));
        assert!(basic_json_validate(&schema, &serde_json::json!(42)));
    }

    #[test]
    fn test_client_construction() {
        let client = SchemaRegistryClient::new("http://localhost:9094");
        assert_eq!(client.base_url, "http://localhost:9094");
        assert!(client.auth_token.is_none());
    }

    #[test]
    fn test_client_trailing_slash_trimmed() {
        let client = SchemaRegistryClient::new("http://localhost:9094/");
        assert_eq!(client.base_url, "http://localhost:9094");
    }

    #[test]
    fn test_client_auth_token() {
        let mut client = SchemaRegistryClient::new("http://localhost:9094");
        client.set_auth_token("my-token");
        assert_eq!(client.auth_token.as_deref(), Some("my-token"));
    }
}
