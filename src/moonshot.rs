//! Browser-safe subset of the Streamline Moonshot HTTP API for WebAssembly.
//!
//! **Scope (deliberate):** only read-side moonshot operations are exposed:
//!   * [`SearchClient`]     — `POST /api/v1/search`        (M2 semantic search)
//!   * [`MemoryReadClient`] — `POST /api/v1/memory/recall` (M1 agent memory recall)
//!
//! **Excluded by design:** attestation signing, contract registration, branch
//! mutation, and `memory/remember` are admin / write / signing operations.
//! Exposing them in a browser context would either leak signing material via
//! XHR-attached headers or grant write access to any embedded JavaScript on the
//! page. Use a server-side gateway for those.
//!
//! Stability: Experimental — API may change before GA.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub agent: String,
    pub kind: String,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, rename = "timestamp_ms")]
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct RecallResponse {
    #[serde(default)]
    memories: Vec<MemoryRecord>,
}

/// Read-safe moonshot search client.
#[wasm_bindgen]
pub struct SearchClient {
    base: MoonshotHttp,
}

#[wasm_bindgen]
impl SearchClient {
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: &str) -> Self {
        Self {
            base: MoonshotHttp::new(base_url),
        }
    }

    pub fn set_auth_token(&mut self, token: &str) {
        self.base.auth_token = Some(token.to_string());
    }

    /// `topic`, `query` non-empty; `k > 0`. Returns a JS array of hits.
    pub async fn search(&self, topic: &str, query: &str, k: u32) -> Result<JsValue, JsValue> {
        if topic.is_empty() {
            return Err(JsValue::from_str("topic is required"));
        }
        if query.is_empty() {
            return Err(JsValue::from_str("query is required"));
        }
        if k == 0 {
            return Err(JsValue::from_str("k must be > 0"));
        }
        let body = serde_json::json!({ "topic": topic, "query": query, "k": k });
        let url = format!("{}/api/v1/search", self.base.base_url);
        let resp_text = self.base.http_post(&url, &body.to_string()).await?;
        let parsed: SearchResponse = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&parsed.hits).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Read-safe moonshot agent-memory client (recall only).
#[wasm_bindgen]
pub struct MemoryReadClient {
    base: MoonshotHttp,
}

#[wasm_bindgen]
impl MemoryReadClient {
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: &str) -> Self {
        Self {
            base: MoonshotHttp::new(base_url),
        }
    }

    pub fn set_auth_token(&mut self, token: &str) {
        self.base.auth_token = Some(token.to_string());
    }

    /// Recall up to `k` memories for `agent` matching `query`.
    pub async fn recall(&self, agent: &str, query: &str, k: u32) -> Result<JsValue, JsValue> {
        if agent.is_empty() {
            return Err(JsValue::from_str("agent is required"));
        }
        if query.is_empty() {
            return Err(JsValue::from_str("query is required"));
        }
        if k == 0 {
            return Err(JsValue::from_str("k must be > 0"));
        }
        let body = serde_json::json!({ "agent": agent, "query": query, "k": k });
        let url = format!("{}/api/v1/memory/recall", self.base.base_url);
        let resp_text = self.base.http_post(&url, &body.to_string()).await?;
        let parsed: RecallResponse = serde_json::from_str(&resp_text)
            .map_err(|e| JsValue::from_str(&format!("parse error: {e}")))?;
        serde_wasm_bindgen::to_value(&parsed.memories)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

struct MoonshotHttp {
    base_url: String,
    auth_token: Option<String>,
}

impl MoonshotHttp {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token: None,
        }
    }

    async fn http_post(&self, url: &str, body: &str) -> Result<String, JsValue> {
        let opts = RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(RequestMode::Cors);
        opts.set_body(&JsValue::from_str(body));

        let request = Request::new_with_str_and_init(url, &opts)?;
        request.headers().set("Accept", "application/json")?;
        request.headers().set("Content-Type", "application/json")?;
        if let Some(ref token) = self.auth_token {
            request
                .headers()
                .set("Authorization", &format!("Bearer {}", token))?;
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_response_parses() {
        let json = r#"{"hits":[{"topic":"t","partition":0,"offset":7,"score":0.9,"snippet":"hi"}]}"#;
        let r: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.hits.len(), 1);
        assert_eq!(r.hits[0].topic, "t");
        assert_eq!(r.hits[0].snippet.as_deref(), Some("hi"));
    }

    #[test]
    fn recall_response_parses_with_missing_optional_fields() {
        let json = r#"{"memories":[{"agent":"a","kind":"fact","text":"x"}]}"#;
        let r: RecallResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.memories.len(), 1);
        assert!(r.memories[0].tags.is_empty());
        assert_eq!(r.memories[0].timestamp_ms, 0);
    }

    #[test]
    fn empty_response_yields_empty_lists() {
        let s: SearchResponse = serde_json::from_str("{}").unwrap();
        let m: RecallResponse = serde_json::from_str("{}").unwrap();
        assert!(s.hits.is_empty());
        assert!(m.memories.is_empty());
    }

    #[test]
    fn base_url_trailing_slash_is_stripped() {
        let h = MoonshotHttp::new("https://example.test/");
        assert_eq!(h.base_url, "https://example.test");
    }
}
