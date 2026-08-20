//! HTTP tools for a governed computer/browser service.
//!
//! The service is deliberately kept separate from the CDP browser tools.  CDP
//! remains available as a local fallback, while these tools can target a
//! remote, policy-controlled computer through `COMPUTER_URL`.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::registry::{Tool, ToolContext};

pub const COMPUTER_URL_ENV: &str = "COMPUTER_URL";
pub const COMPUTER_TOKEN_ENV: &str = "COMPUTER_TOKEN";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_ERROR_BODY: usize = 160;

/// Return the configured service URL, or `None` when the HTTP computer is not
/// configured. Empty values are treated as unset.
pub fn normalize_computer_url(value: Option<&str>) -> Option<String> {
    value
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
}

pub fn computer_url_from_env() -> Option<String> {
    let value = std::env::var(COMPUTER_URL_ENV).ok();
    normalize_computer_url(value.as_deref())
}

fn token_from_env() -> Option<String> {
    std::env::var(COMPUTER_TOKEN_ENV)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

/// Shared, bounded HTTP client for computer operations.
#[derive(Clone)]
pub struct ComputerClient {
    pub base_url: String,
    token: Option<String>,
    agent_id: Option<String>,
    http: reqwest::Client,
    /// Refs are issued by a snapshot and are required for mutating actions.
    refs: Arc<RwLock<HashSet<String>>>,
    active_tab_id: Arc<RwLock<Option<String>>>,
}

impl std::fmt::Debug for ComputerClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputerClient")
            .field("base_url", &self.base_url)
            .field("token_configured", &self.token.is_some())
            .field("agent_configured", &self.agent_id.is_some())
            .finish()
    }
}

impl ComputerClient {
    pub fn new(base_url: impl Into<String>, token: Option<String>) -> anyhow::Result<Self> {
        let base_url = base_url.into().trim().trim_end_matches('/').to_string();
        validate_base_url(&base_url)?;
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|e| anyhow::anyhow!("could not create computer HTTP client: {e}"))?;
        Ok(Self {
            base_url,
            token: token.filter(|t| !t.trim().is_empty()),
            agent_id: None,
            http,
            refs: Arc::new(RwLock::new(HashSet::new())),
            active_tab_id: Arc::new(RwLock::new(None)),
        })
    }

    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let Some(url) = computer_url_from_env() else {
            return Ok(None);
        };
        Ok(Some(Self::new(url, token_from_env())?))
    }

    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    fn for_context(&self, ctx: &ToolContext) -> Self {
        match &ctx.agent_id {
            Some(agent_id) => self.clone().with_agent_id(agent_id.to_string()),
            None => self.clone(),
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let request = self.http.request(method, self.endpoint(path));
        let request = match &self.token {
            Some(token) => request.bearer_auth(token).header("x-computer-token", token),
            None => request,
        };
        match &self.agent_id {
            Some(agent_id) => request.header("x-agent-id", agent_id),
            None => request,
        }
    }

    async fn json<T: DeserializeOwned>(&self, response: reqwest::Response, path: &str) -> anyhow::Result<T> {
        let status = response.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("computer service {path} returned HTTP {}", status.as_u16()));
        }
        response
            .json::<T>()
            .await
            .map_err(|_| anyhow::anyhow!("computer service {path} returned invalid JSON"))
    }

    async fn post_json<T: DeserializeOwned>(&self, path: &str, body: Value) -> anyhow::Result<T> {
        let response = self.request(reqwest::Method::POST, path).json(&body).send().await
            .map_err(|e| map_transport_error(path, e))?;
        self.json(response, path).await
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let response = self.request(reqwest::Method::GET, path).send().await
            .map_err(|e| map_transport_error(path, e))?;
        self.json(response, path).await
    }

    pub async fn session(&self, url: Option<&str>) -> anyhow::Result<ComputerResponse> {
        self.post_json("/session", json!({"url": url})).await
    }

    pub async fn navigate(&self, url: &str) -> anyhow::Result<ComputerResponse> {
        self.post_json("/navigate", json!({"url": url})).await
    }

    pub async fn list_tabs(&self) -> anyhow::Result<ComputerResponse> {
        self.get_json("/tabs").await
    }

    pub async fn tabs(&self) -> anyhow::Result<ComputerResponse> {
        self.list_tabs().await
    }

    pub async fn open_tab(&self, url: &str) -> anyhow::Result<ComputerResponse> {
        self.post_json("/tabs/open", json!({"url": url})).await
    }

    pub async fn activate_tab(&self, tab_id: &str) -> anyhow::Result<ComputerResponse> {
        validate_tab_id(tab_id)?;
        let path = format!("/tabs/{}/activate", urlencoding::encode(tab_id));
        let result = self.post_json(&path, json!({})).await?;
        *self.active_tab_id.write().await = Some(tab_id.to_string());
        self.refs.write().await.clear();
        Ok(result)
    }

    pub async fn close_tab(&self, tab_id: &str) -> anyhow::Result<ComputerResponse> {
        validate_tab_id(tab_id)?;
        let path = format!("/tabs/{}/close", urlencoding::encode(tab_id));
        let result = self.post_json(&path, json!({})).await?;
        self.refs.write().await.clear();
        Ok(result)
    }

    pub async fn snapshot(&self) -> anyhow::Result<ComputerResponse> {
        self.snapshot_tab(None).await
    }

    pub async fn snapshot_tab(&self, tab_id: Option<&str>) -> anyhow::Result<ComputerResponse> {
        let path = match tab_id {
            Some(tab_id) => {
                validate_tab_id(tab_id)?;
                format!("/snapshot?tab_id={}", urlencoding::encode(tab_id))
            }
            None => "/snapshot".to_string(),
        };
        let result: ComputerResponse = self.get_json(&path).await?;
        let response_tab_id = result.tab_id.clone();
        let refs = refs_from_response(&result);
        if let Some(tab_id) = tab_id {
            if !refs.is_empty() {
                let scoped = refs.into_iter().filter(|reference| reference.starts_with(&format!("t_{tab_id}_"))).collect();
                *self.refs.write().await = scoped;
            } else {
                self.refs.write().await.clear();
            }
        } else if let Some(tab_id) = response_tab_id {
            *self.active_tab_id.write().await = Some(tab_id.to_string());
            *self.refs.write().await = refs;
        }
        Ok(result)
    }

    pub async fn click(&self, reference: &str) -> anyhow::Result<ComputerResponse> {
        self.require_ref(reference).await?;
        self.post_json("/click", json!({"ref": reference})).await
    }

    pub async fn type_text(&self, reference: &str, text: &str) -> anyhow::Result<ComputerResponse> {
        self.require_ref(reference).await?;
        self.post_json("/type", json!({"ref": reference, "text": text})).await
    }

    pub async fn key(&self, key: &str) -> anyhow::Result<ComputerResponse> {
        self.post_json("/key", json!({"key": key})).await
    }

    pub async fn screenshot(&self) -> anyhow::Result<ComputerScreenshot> {
        let response = self.request(reqwest::Method::GET, "/screenshot").send().await
            .map_err(|e| map_transport_error("/screenshot", e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("computer service /screenshot returned HTTP {}", status.as_u16()));
        }
        let content_type = response.headers().get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
        if content_type.contains("json") || content_type.is_empty() {
            let body = response.json::<ComputerResponse>().await
                .map_err(|_| anyhow::anyhow!("computer service /screenshot returned invalid JSON"))?;
            return Ok(ComputerScreenshot { data: body.data.clone().or_else(|| body.screenshot.clone()).unwrap_or_default(), response: body });
        }
        let bytes = response.bytes().await.map_err(|_| anyhow::anyhow!("failed reading computer screenshot"))?;
        Ok(ComputerScreenshot {
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
            response: ComputerResponse::default(),
        })
    }

    async fn require_ref(&self, reference: &str) -> anyhow::Result<()> {
        validate_ref(reference)?;
        let tab_id = self.active_tab_id.read().await.clone();
        if let Some(tab_id) = tab_id {
            if !reference.starts_with(&format!("t_{tab_id}_")) {
                anyhow::bail!("ref belongs to a different tab")
            }
        }
        let refs = self.refs.read().await;
        if refs.is_empty() {
            anyhow::bail!("no snapshot refs available; call computer_snapshot first")
        }
        if !refs.contains(reference) {
            anyhow::bail!("ref is not present in the latest computer snapshot")
        }
        Ok(())
    }
}

fn validate_base_url(url: &str) -> anyhow::Result<()> {
    let parsed = url::Url::parse(url).map_err(|_| anyhow::anyhow!("COMPUTER_URL must be a valid URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        anyhow::bail!("COMPUTER_URL must use http or https")
    }
    Ok(())
}

pub fn validate_ref(reference: &str) -> anyhow::Result<()> {
    let value = reference.trim();
    if value.is_empty() || value.len() > 128 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b':' || b == b'.') {
        anyhow::bail!("ref must be a non-empty snapshot reference")
    }
    Ok(())
}

fn validate_tab_id(tab_id: &str) -> anyhow::Result<()> {
    let value = tab_id.trim();
    if value.is_empty() || value.len() > 64 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        anyhow::bail!("tab_id must be a non-empty tab identifier")
    }
    Ok(())
}

fn map_transport_error(path: &str, error: reqwest::Error) -> anyhow::Error {
    if error.is_timeout() {
        anyhow::anyhow!("computer service {path} timed out")
    } else {
        anyhow::anyhow!("computer service {path} request failed")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComputerResponse {
    #[serde(default)] pub tab_id: Option<String>,
    #[serde(default)] pub url: Option<String>,
    #[serde(default)] pub title: Option<String>,
    #[serde(default)] pub screenshot: Option<String>,
    #[serde(default, rename = "data")] pub data: Option<String>,
    #[serde(default, rename = "mimeType")] pub mime_type: Option<String>,
    #[serde(default)] pub control_owner: Option<String>,
    #[serde(default)] pub refs: Value,
    #[serde(flatten)] pub extra: serde_json::Map<String, Value>,
}

#[derive(Debug)]
pub struct ComputerScreenshot { pub data: String, pub response: ComputerResponse }

fn refs_from_response(response: &ComputerResponse) -> HashSet<String> {
    let mut refs = match &response.refs {
        Value::Object(map) => map.keys().filter_map(|key| validate_ref(key).ok().map(|_| key.clone())).collect(),
        Value::Array(items) => items.iter().filter_map(|item| item.as_str()).filter_map(|key| validate_ref(key).ok().map(|_| key.to_string())).collect(),
        _ => HashSet::new(),
    };
    if let Some(elements) = response.extra.get("elements").and_then(Value::as_array) {
        refs.extend(elements.iter().filter_map(|item| item.get("ref").and_then(Value::as_str)).filter_map(|key| validate_ref(key).ok().map(|_| key.to_string())));
    }
    refs
}

fn metadata(response: &ComputerResponse, reference: Option<&str>) -> Value {
    json!({"tab_id": response.tab_id, "url": response.url, "owner": response.control_owner, "ref": reference})
}

fn schema<T: JsonSchema>(name: &str, description: &str) -> ToolSchema {
    ToolSchema { name: name.to_string(), description: description.to_string(), parameters: serde_json::to_value(&schemars::schema_for!(T).schema).unwrap_or_default() }
}

fn output_error(tool: &str, error: anyhow::Error) -> ToolOutput {
    let text = error.to_string();
    let code = if text.contains("timed out") { "timeout" } else if text.contains("HTTP 401") || text.contains("HTTP 403") { "unauthorized" } else if text.contains("HTTP 404") { "not_found" } else if text.contains("ref") || text.contains("required") { "invalid_arguments" } else { "network" };
    ToolOutput::err_code(format!("{tool} failed: {}", text.chars().take(MAX_ERROR_BODY).collect::<String>()), code)
}

#[derive(Debug, Deserialize, JsonSchema)] pub struct NavigateParams { pub url: Option<String> }
#[derive(Debug, Deserialize, JsonSchema)] pub struct RefParams { pub reference: String }
#[derive(Debug, Deserialize, JsonSchema)] pub struct TypeParams { pub reference: String, pub text: String }
#[derive(Debug, Deserialize, JsonSchema)] pub struct KeyParams { pub key: String }
#[derive(Debug, Deserialize, JsonSchema)] pub struct EmptyParams {}

pub struct ComputerSnapshotTool { pub client: ComputerClient }
pub struct ComputerNavigateTool { pub client: ComputerClient }
pub struct ComputerClickTool { pub client: ComputerClient }
pub struct ComputerTypeTool { pub client: ComputerClient }
pub struct ComputerKeyTool { pub client: ComputerClient }
pub struct ComputerScreenshotTool { pub client: ComputerClient }

macro_rules! tool_common {
    ($ty:ty, $name:literal, $desc:literal, $params:ty) => {
        fn name(&self) -> &str { $name }
        fn description(&self) -> &str { $desc }
        fn schema(&self) -> ToolSchema { schema::<$params>($name, $desc) }
    };
}

#[async_trait]
impl Tool for ComputerSnapshotTool {
    tool_common!(Self, "computer_snapshot", "Get the current controlled computer page and snapshot refs.", EmptyParams);
    async fn execute(&self, args: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        if !args.is_null() && !args.as_object().map(|m| m.is_empty()).unwrap_or(false) { return Ok(ToolOutput::err_code("computer_snapshot takes no arguments", "invalid_arguments")); }
        match self.client.for_context(ctx).snapshot().await { Ok(r) => Ok(ToolOutput::ok_with_meta(serde_json::to_string_pretty(&r).unwrap_or_default(), metadata(&r, None))), Err(e) => Ok(output_error(self.name(), e)) }
    }
}

#[async_trait]
impl Tool for ComputerNavigateTool {
    tool_common!(Self, "computer_navigate", "Navigate the controlled computer to a URL.", NavigateParams);
    async fn execute(&self, args: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: NavigateParams = match serde_json::from_value(args) { Ok(p) => p, Err(e) => return Ok(ToolOutput::err_code(format!("Invalid arguments: {e}"), "invalid_arguments")) };
        let url = params.url.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let Some(url) = url else { return Ok(ToolOutput::err_code("url is required", "invalid_arguments")); };
        match self.client.for_context(ctx).navigate(url).await { Ok(r) => Ok(ToolOutput::ok_with_meta(serde_json::to_string_pretty(&r).unwrap_or_else(|_| "navigated".into()), metadata(&r, None))), Err(e) => Ok(output_error(self.name(), e)) }
    }
}

#[async_trait]
impl Tool for ComputerClickTool {
    tool_common!(Self, "computer_click", "Click an element by a ref from the latest computer snapshot.", RefParams);
    async fn execute(&self, args: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: RefParams = match serde_json::from_value(args) { Ok(p) => p, Err(e) => return Ok(ToolOutput::err_code(format!("Invalid arguments: {e}"), "invalid_arguments")) };
        match self.client.for_context(ctx).click(params.reference.trim()).await { Ok(r) => Ok(ToolOutput::ok_with_meta(serde_json::to_string_pretty(&r).unwrap_or_default(), metadata(&r, Some(params.reference.trim())))), Err(e) => Ok(output_error(self.name(), e)) }
    }
}

#[async_trait]
impl Tool for ComputerTypeTool {
    tool_common!(Self, "computer_type", "Type text into an element by a ref from the latest computer snapshot.", TypeParams);
    async fn execute(&self, args: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: TypeParams = match serde_json::from_value(args) { Ok(p) => p, Err(e) => return Ok(ToolOutput::err_code(format!("Invalid arguments: {e}"), "invalid_arguments")) };
        match self.client.for_context(ctx).type_text(params.reference.trim(), &params.text).await { Ok(r) => Ok(ToolOutput::ok_with_meta(serde_json::to_string_pretty(&r).unwrap_or_default(), metadata(&r, Some(params.reference.trim())))), Err(e) => Ok(output_error(self.name(), e)) }
    }
}

#[async_trait]
impl Tool for ComputerKeyTool {
    tool_common!(Self, "computer_key", "Send a keyboard key to the controlled computer.", KeyParams);
    async fn execute(&self, args: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: KeyParams = match serde_json::from_value(args) { Ok(p) => p, Err(e) => return Ok(ToolOutput::err_code(format!("Invalid arguments: {e}"), "invalid_arguments")) };
        if params.key.trim().is_empty() { return Ok(ToolOutput::err_code("key is required", "invalid_arguments")); }
        match self.client.for_context(ctx).key(params.key.trim()).await { Ok(r) => Ok(ToolOutput::ok_with_meta(serde_json::to_string_pretty(&r).unwrap_or_default(), metadata(&r, None))), Err(e) => Ok(output_error(self.name(), e)) }
    }
}

#[async_trait]
impl Tool for ComputerScreenshotTool {
    tool_common!(Self, "computer_screenshot", "Capture a screenshot of the controlled computer.", EmptyParams);
    async fn execute(&self, args: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        if !args.is_null() && !args.as_object().map(|m| m.is_empty()).unwrap_or(false) { return Ok(ToolOutput::err_code("computer_screenshot takes no arguments", "invalid_arguments")); }
        match self.client.for_context(ctx).screenshot().await { Ok(s) => Ok(ToolOutput::ok_with_meta("Screenshot captured", json!({"base64": s.data, "url": s.response.url, "owner": s.response.control_owner}))), Err(e) => Ok(output_error(self.name(), e)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_normalization_trims_and_unsets_empty() {
        assert_eq!(normalize_computer_url(Some(" http://localhost:1234/// ")), Some("http://localhost:1234".to_string()));
        assert_eq!(normalize_computer_url(Some("  ")), None);
        assert_eq!(normalize_computer_url(None), None);
    }

    #[test]
    fn base_url_validation_rejects_bad_urls() {
        assert!(ComputerClient::new("not a url", None).is_err());
        assert!(ComputerClient::new("ftp://localhost", None).is_err());
        assert!(ComputerClient::new("http://localhost:1234/", None).is_ok());
    }

    #[test]
    fn refs_are_strictly_validated() {
        assert!(validate_ref("button-1").is_ok());
        assert!(validate_ref("").is_err());
        assert!(validate_ref("button with spaces").is_err());
        assert!(validate_ref("../secret").is_err());
    }

    #[test]
    fn response_refs_support_object_and_array() {
        let object: ComputerResponse = serde_json::from_value(json!({"refs":{"a":{"role":"button"}}})).unwrap();
        assert!(refs_from_response(&object).contains("a"));
        let array: ComputerResponse = serde_json::from_value(json!({"refs":["b"]})).unwrap();
        assert!(refs_from_response(&array).contains("b"));
    }
}
