//! Browser automation tools driven over the Chrome DevTools Protocol (CDP).
//!
//! The tools talk to a locally running Chrome/Chromium instance that exposes
//! the CDP remote-debugging endpoint (default `http://localhost:9222`,
//! override with the `PARALLEL_CDP_ENDPOINT` environment variable).
//!
//! Transport:
//! - Target management (`/json/list`, `/json/new`) uses plain HTTP via the
//!   shared reqwest client from [`ToolContext`].
//! - Commands (navigate, screenshot, evaluate) are sent as CDP JSON-RPC
//!   messages over the target's `webSocketDebuggerUrl` WebSocket.
//!
//! Launch a compatible browser with:
//! `chrome --headless --remote-debugging-port=9222 --user-data-dir=/tmp/cdp`

use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use crate::registry::{Tool, ToolContext};

/// Default CDP endpoint used when `PARALLEL_CDP_ENDPOINT` is not set.
pub const DEFAULT_CDP_ENDPOINT: &str = "http://localhost:9222";

/// Environment variable that overrides the CDP endpoint.
pub const CDP_ENDPOINT_ENV: &str = "PARALLEL_CDP_ENDPOINT";

/// Timeout for a single CDP command round-trip.
const CDP_CALL_TIMEOUT_SECS: u64 = 30;
/// Timeout for the WebSocket connection handshake.
const CDP_CONNECT_TIMEOUT_SECS: u64 = 10;
/// Maximum time to wait for the page load event after a navigation.
const PAGE_LOAD_TIMEOUT_SECS: u64 = 30;
/// Maximum characters returned by `browser_extract`.
const DEFAULT_EXTRACT_MAX_CHARS: usize = 50_000;
/// Base64 screenshot payloads up to this size are inlined in `content`.
const SCREENSHOT_INLINE_LIMIT: usize = 60_000;

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Read the CDP endpoint from the environment (with default fallback).
pub fn cdp_endpoint_from_env() -> String {
    std::env::var(CDP_ENDPOINT_ENV).unwrap_or_else(|_| DEFAULT_CDP_ENDPOINT.to_string())
}

/// Best-effort synchronous check that a CDP endpoint accepts TCP connections.
/// Used by the registry to decide whether browser tools should be registered.
pub fn cdp_available(endpoint: &str) -> bool {
    let Ok(url) = url::Url::parse(endpoint) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let port = url.port_or_known_default().unwrap_or(9222);
    let Ok(addrs) = format!("{host}:{port}").to_socket_addrs() else {
        return false;
    };
    for addr in addrs {
        if std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok() {
            return true;
        }
    }
    false
}

/// A CDP target (browser tab) as returned by the `/json` HTTP endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct CdpTarget {
    pub id: String,
    #[serde(default, rename = "type")]
    pub target_type: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default, rename = "webSocketDebuggerUrl")]
    pub ws_url: Option<String>,
}

/// Shared CDP client state and helpers used by all browser tools.
#[derive(Debug, Clone)]
pub struct BrowserTool {
    /// Base URL of the CDP HTTP endpoint, e.g. `http://localhost:9222`.
    pub cdp_endpoint: String,
}

impl BrowserTool {
    pub fn new(cdp_endpoint: impl Into<String>) -> Self {
        Self {
            cdp_endpoint: cdp_endpoint.into(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(cdp_endpoint_from_env())
    }

    fn endpoint(&self) -> String {
        self.cdp_endpoint.trim_end_matches('/').to_string()
    }

    /// List all CDP targets (`/json/list`, falling back to `/json`).
    pub async fn list_targets(&self, client: &reqwest::Client) -> anyhow::Result<Vec<CdpTarget>> {
        for path in ["/json/list", "/json"] {
            let url = format!("{}{}", self.endpoint(), path);
            let Ok(resp) = client
                .get(&url)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            else {
                continue;
            };
            if !resp.status().is_success() {
                continue;
            }
            if let Ok(targets) = resp.json::<Vec<CdpTarget>>().await {
                return Ok(targets);
            }
        }
        anyhow::bail!(
            "could not list CDP targets at {} — is a browser running with --remote-debugging-port?",
            self.cdp_endpoint
        )
    }

    /// Return the first open page target, if any.
    pub async fn first_page(&self, client: &reqwest::Client) -> anyhow::Result<CdpTarget> {
        let targets = self.list_targets(client).await?;
        targets
            .into_iter()
            .find(|t| t.target_type == "page")
            .ok_or_else(|| {
                anyhow::anyhow!("no browser page is open; call browser_navigate first")
            })
    }

    /// Open a new page target via `/json/new` (PUT first, GET for older builds).
    pub async fn new_page(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> anyhow::Result<CdpTarget> {
        let encoded: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
        let target_url = format!("{}/json/new?{}", self.endpoint(), encoded);

        // Recent Chrome requires PUT for /json/new (CSRF mitigation); older
        // builds only accept GET. Try PUT, then fall back to GET.
        let resp = match client
            .put(&target_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(_) => client
                .get(&target_url)
                .timeout(Duration::from_secs(10))
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("failed to open new CDP target: {e}"))?,
            Err(e) => anyhow::bail!("failed to open new CDP target: {e}"),
        };

        if !resp.status().is_success() {
            anyhow::bail!("failed to open new CDP target: HTTP {}", resp.status());
        }
        resp.json::<CdpTarget>()
            .await
            .map_err(|e| anyhow::anyhow!("invalid CDP target response: {e}"))
    }
}

/// A single CDP WebSocket session with request/response id matching.
struct CdpSession {
    ws: WsStream,
    next_id: i64,
}

impl CdpSession {
    async fn connect(ws_url: &str) -> anyhow::Result<Self> {
        let (ws, _resp) = tokio::time::timeout(
            Duration::from_secs(CDP_CONNECT_TIMEOUT_SECS),
            tokio_tungstenite::connect_async(ws_url),
        )
        .await
        .map_err(|_| anyhow::anyhow!("timed out connecting to CDP WebSocket"))?
        .map_err(|e| anyhow::anyhow!("CDP WebSocket connection failed: {e}"))?;
        Ok(Self { ws, next_id: 1 })
    }

    /// Send a CDP command and wait for the matching response.
    async fn call(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({ "id": id, "method": method, "params": params });
        self.ws
            .send(Message::Text(msg.to_string()))
            .await
            .map_err(|e| anyhow::anyhow!("failed to send CDP command: {e}"))?;

        let deadline = Instant::now() + Duration::from_secs(CDP_CALL_TIMEOUT_SECS);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                anyhow::bail!("timed out waiting for CDP response to {method}");
            }
            let value = match self.next_json(remaining).await? {
                Some(v) => v,
                None => anyhow::bail!("CDP WebSocket closed while waiting for {method}"),
            };
            if value.get("id").and_then(Value::as_i64) == Some(id) {
                if let Some(err) = value.get("error") {
                    let message = err
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown CDP error");
                    anyhow::bail!("CDP error in {method}: {message}");
                }
                return Ok(value.get("result").cloned().unwrap_or(json!({})));
            }
            // Events (or responses for other ids) are skipped here.
        }
    }

    /// Read the next JSON message from the socket. Returns `None` when the
    /// socket is closed. Ping/Pong are handled by tungstenite internally.
    async fn next_json(&mut self, timeout: Duration) -> anyhow::Result<Option<Value>> {
        let msg = tokio::time::timeout(timeout, self.ws.next())
            .await
            .map_err(|_| anyhow::anyhow!("timed out reading from CDP WebSocket"))?;
        match msg {
            Some(Ok(Message::Text(text))) => {
                Ok(Some(serde_json::from_str(&text).unwrap_or(json!({}))))
            }
            Some(Ok(Message::Binary(bytes))) => {
                Ok(Some(serde_json::from_slice(&bytes).unwrap_or(json!({}))))
            }
            Some(Ok(Message::Close(_))) | None => Ok(None),
            // Ping/Pong/Frame placeholders: callers loop past empty objects.
            Some(Ok(_)) => Ok(Some(json!({}))),
            Some(Err(e)) => Err(anyhow::anyhow!("CDP WebSocket error: {e}")),
        }
    }
}

/// Run `Runtime.evaluate` with `returnByValue` and return the produced value.
async fn evaluate(session: &mut CdpSession, expression: &str) -> anyhow::Result<Value> {
    let result = session
        .call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
        )
        .await?;
    if let Some(exc) = result.get("exceptionDetails") {
        let description = exc
            .get("exception")
            .and_then(|e| e.get("description"))
            .and_then(Value::as_str)
            .or_else(|| exc.get("text").and_then(Value::as_str))
            .unwrap_or("unknown JavaScript error");
        anyhow::bail!("JavaScript error: {description}");
    }
    Ok(result
        .get("result")
        .and_then(|r| r.get("value"))
        .cloned()
        .unwrap_or(Value::Null))
}

/// Produce a JavaScript string literal for `s` (JSON escaping plus the two
/// line-terminator characters that JSON does not escape).
fn js_string(s: &str) -> String {
    serde_json::to_string(s)
        .unwrap_or_else(|_| "\"\"".to_string())
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// Char-boundary-safe truncation helper.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}...\n\n[Content truncated at {max_chars} characters]")
}

/// Require the target to expose a WebSocket debugger URL.
fn require_ws_url(target: &CdpTarget) -> anyhow::Result<String> {
    target.ws_url.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "CDP target {} has no webSocketDebuggerUrl (it may be attached by another debugger)",
            target.id
        )
    })
}

// ─── browser_navigate ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct NavigateParams {
    /// URL to navigate to
    url: String,
}

pub struct BrowserNavigateTool {
    pub browser: BrowserTool,
}

impl BrowserNavigateTool {
    pub fn new(cdp_endpoint: impl Into<String>) -> Self {
        Self {
            browser: BrowserTool::new(cdp_endpoint),
        }
    }
}

#[async_trait]
impl Tool for BrowserNavigateTool {
    fn name(&self) -> &str {
        "browser_navigate"
    }
    fn description(&self) -> &str {
        "Navigate a Chrome browser (via CDP) to a URL and wait for the page to load.

## Capability

Drives a locally running Chrome/Chromium instance through the Chrome DevTools Protocol. Reuses the currently open tab when one exists, otherwise opens a new one. Waits (up to 30s) for the page load event, then returns the final URL and document title. The browser session persists across browser_* calls.

## When to Use

- Loading a page before taking a screenshot (`browser_screenshot`) or extracting rendered text (`browser_extract`).
- Reading JavaScript-heavy pages whose content `web_fetch` cannot see.
- Interacting with a page via `browser_click` / `browser_type`.

## When NOT to Use

- Static pages or APIs: prefer `web_fetch` (faster, no browser required).
- No CDP endpoint reachable: the tool errors out; ask for a browser with --remote-debugging-port."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(NavigateParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: NavigateParams = serde_json::from_value(args)?;
        match self.navigate(&params, ctx).await {
            Ok(output) => Ok(output),
            Err(e) => Ok(ToolOutput::err(format!(
                "browser_navigate failed: {e} (CDP endpoint: {})",
                self.browser.cdp_endpoint
            ))),
        }
    }
}

impl BrowserNavigateTool {
    async fn navigate(
        &self,
        params: &NavigateParams,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let client = &ctx.http_client;

        // Reuse an existing page target when available to avoid leaking tabs.
        let target = match self.browser.list_targets(client).await {
            Ok(targets) => match targets.into_iter().find(|t| t.target_type == "page") {
                Some(t) => t,
                None => self.browser.new_page(client, &params.url).await?,
            },
            Err(_) => self.browser.new_page(client, &params.url).await?,
        };
        let ws_url = require_ws_url(&target)?;
        let mut session = CdpSession::connect(&ws_url).await?;

        let _ = session.call("Page.enable", json!({})).await;
        let nav = session
            .call("Page.navigate", json!({ "url": params.url }))
            .await?;
        if let Some(err_text) = nav.get("errorText").and_then(Value::as_str) {
            if !err_text.is_empty() {
                return Ok(ToolOutput::err(format!(
                    "navigation to {} failed: {err_text}",
                    params.url
                )));
            }
        }

        // Wait (bounded) for the load event; timeouts are non-fatal.
        let deadline = Instant::now() + Duration::from_secs(PAGE_LOAD_TIMEOUT_SECS);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match session.next_json(remaining).await {
                Ok(Some(v))
                    if v.get("method").and_then(Value::as_str) == Some("Page.loadEventFired") =>
                {
                    break;
                }
                Ok(None) | Err(_) => break,
                _ => {}
            }
        }

        let final_url = evaluate(&mut session, "location.href")
            .await
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| params.url.clone());
        let title = evaluate(&mut session, "document.title")
            .await
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        Ok(ToolOutput::ok(format!(
            "Navigated to {final_url}\nTitle: {title}"
        )))
    }
}

// ─── browser_screenshot ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ScreenshotParams {
    /// Image format: "png" (default) or "jpeg"
    #[serde(default = "default_screenshot_format")]
    format: String,
    /// Capture the full scrollable page instead of the viewport (default: false)
    #[serde(default)]
    full_page: bool,
}

fn default_screenshot_format() -> String {
    "png".to_string()
}

pub struct BrowserScreenshotTool {
    pub browser: BrowserTool,
}

impl BrowserScreenshotTool {
    pub fn new(cdp_endpoint: impl Into<String>) -> Self {
        Self {
            browser: BrowserTool::new(cdp_endpoint),
        }
    }
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn name(&self) -> &str {
        "browser_screenshot"
    }
    fn description(&self) -> &str {
        "Take a screenshot of the current browser page (via CDP) and return it as base64-encoded image data.

## Capability

Captures the viewport (or the full scrollable page with `full_page: true`) of the currently open page as PNG (default) or JPEG. The base64 payload is returned in `metadata.base64`; payloads up to 60,000 characters are also included directly in the content. Call `browser_navigate` first to open a page.

## When to Use

- Verifying what a rendered page actually looks like.
- Capturing charts, images, or layouts that text extraction cannot describe.
- Producing visual evidence for findings.

## When NOT to Use

- Text content: prefer `browser_extract` (much cheaper)."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ScreenshotParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: ScreenshotParams = serde_json::from_value(args)?;
        match self.screenshot(&params, ctx).await {
            Ok(output) => Ok(output),
            Err(e) => Ok(ToolOutput::err(format!("browser_screenshot failed: {e}"))),
        }
    }
}

impl BrowserScreenshotTool {
    async fn screenshot(
        &self,
        params: &ScreenshotParams,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let format = match params.format.to_ascii_lowercase().as_str() {
            f @ ("png" | "jpeg") => f.to_string(),
            other => {
                return Ok(ToolOutput::err(format!(
                    "unsupported screenshot format '{other}', use 'png' or 'jpeg'"
                )))
            }
        };

        let target = self.browser.first_page(&ctx.http_client).await?;
        let ws_url = require_ws_url(&target)?;
        let mut session = CdpSession::connect(&ws_url).await?;

        let result = session
            .call(
                "Page.captureScreenshot",
                json!({ "format": format, "captureBeyondViewport": params.full_page }),
            )
            .await?;
        let data = result
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("CDP returned no screenshot data"))?
            .to_string();

        let meta = json!({
            "format": format,
            "full_page": params.full_page,
            "base64_bytes": data.len(),
            "base64": data,
        });

        if data.len() <= SCREENSHOT_INLINE_LIMIT {
            Ok(ToolOutput::ok_with_meta(data, meta))
        } else {
            Ok(ToolOutput::ok_with_meta(
                format!(
                    "Screenshot captured ({} format, {} base64 chars). The full base64 payload is in the metadata field under \"base64\".",
                    format,
                    data.len()
                ),
                meta,
            ))
        }
    }
}

// ─── browser_click ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ClickParams {
    /// CSS selector of the element to click
    selector: String,
}

pub struct BrowserClickTool {
    pub browser: BrowserTool,
}

impl BrowserClickTool {
    pub fn new(cdp_endpoint: impl Into<String>) -> Self {
        Self {
            browser: BrowserTool::new(cdp_endpoint),
        }
    }
}

#[async_trait]
impl Tool for BrowserClickTool {
    fn name(&self) -> &str {
        "browser_click"
    }
    fn description(&self) -> &str {
        "Click an element on the current browser page, identified by CSS selector.

## Capability

Runs `element.click()` on the first element matching the selector (scrolling it into view first). Returns the clicked element's tag and a short text snippet. Call `browser_navigate` first.

## When to Use

- Opening links, expanding accordions, dismissing dialogs, or triggering client-side actions.

## When NOT to Use

- Filling forms: use `browser_type`.
- Reading content: use `browser_extract`."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ClickParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: ClickParams = serde_json::from_value(args)?;
        match self.click(&params, ctx).await {
            Ok(output) => Ok(output),
            Err(e) => Ok(ToolOutput::err(format!("browser_click failed: {e}"))),
        }
    }
}

impl BrowserClickTool {
    async fn click(&self, params: &ClickParams, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let expr = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return {{ found: false }};
                if (el.scrollIntoView) el.scrollIntoView({{ block: "center" }});
                el.click();
                return {{
                    found: true,
                    tag: (el.tagName || "").toLowerCase(),
                    text: ((el.innerText || el.value || "") + "").slice(0, 200)
                }};
            }})()"#,
            sel = js_string(&params.selector)
        );

        let target = self.browser.first_page(&ctx.http_client).await?;
        let ws_url = require_ws_url(&target)?;
        let mut session = CdpSession::connect(&ws_url).await?;
        let value = evaluate(&mut session, &expr).await?;

        if value.get("found").and_then(Value::as_bool) != Some(true) {
            return Ok(ToolOutput::err(format!(
                "no element matches selector: {}",
                params.selector
            )));
        }
        let tag = value.get("tag").and_then(Value::as_str).unwrap_or("?");
        let text = value.get("text").and_then(Value::as_str).unwrap_or("");
        Ok(ToolOutput::ok(format!(
            "Clicked <{tag}> matching '{}'\nText: {text}",
            params.selector
        )))
    }
}

// ─── browser_type ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct TypeParams {
    /// CSS selector of the input element
    selector: String,
    /// Text to type into the element
    text: String,
    /// Submit the surrounding form after typing (default: false)
    #[serde(default)]
    submit: bool,
}

pub struct BrowserTypeTool {
    pub browser: BrowserTool,
}

impl BrowserTypeTool {
    pub fn new(cdp_endpoint: impl Into<String>) -> Self {
        Self {
            browser: BrowserTool::new(cdp_endpoint),
        }
    }
}

#[async_trait]
impl Tool for BrowserTypeTool {
    fn name(&self) -> &str {
        "browser_type"
    }
    fn description(&self) -> &str {
        "Type text into an input on the current browser page, identified by CSS selector.

## Capability

Focuses the first element matching the selector and sets its value (input/textarea via `value`, contenteditable via `textContent`), then dispatches `input` and `change` events so front-end frameworks observe the change. With `submit: true` the surrounding form is submitted afterwards. Call `browser_navigate` first.

## When to Use

- Filling search boxes, login forms, or any text input before clicking a submit control."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(TypeParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: TypeParams = serde_json::from_value(args)?;
        match self.type_text(&params, ctx).await {
            Ok(output) => Ok(output),
            Err(e) => Ok(ToolOutput::err(format!("browser_type failed: {e}"))),
        }
    }
}

impl BrowserTypeTool {
    async fn type_text(
        &self,
        params: &TypeParams,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let submit = if params.submit { "true" } else { "false" };
        let expr = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return {{ found: false }};
                el.focus();
                if ("value" in el) {{ el.value = {text}; }} else {{ el.textContent = {text}; }}
                el.dispatchEvent(new Event("input", {{ bubbles: true }}));
                el.dispatchEvent(new Event("change", {{ bubbles: true }}));
                if ({submit} && el.form && el.form.requestSubmit) el.form.requestSubmit();
                return {{ found: true, tag: (el.tagName || "").toLowerCase() }};
            }})()"#,
            sel = js_string(&params.selector),
            text = js_string(&params.text),
            submit = submit
        );

        let target = self.browser.first_page(&ctx.http_client).await?;
        let ws_url = require_ws_url(&target)?;
        let mut session = CdpSession::connect(&ws_url).await?;
        let value = evaluate(&mut session, &expr).await?;

        if value.get("found").and_then(Value::as_bool) != Some(true) {
            return Ok(ToolOutput::err(format!(
                "no element matches selector: {}",
                params.selector
            )));
        }
        let tag = value.get("tag").and_then(Value::as_str).unwrap_or("?");
        Ok(ToolOutput::ok(format!(
            "Typed {} characters into <{tag}> matching '{}'{}",
            params.text.chars().count(),
            params.selector,
            if params.submit { " and submitted the form" } else { "" }
        )))
    }
}

// ─── browser_extract ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ExtractParams {
    /// Optional CSS selector to scope extraction (default: whole body)
    #[serde(default)]
    selector: Option<String>,
    /// Maximum characters to return (default: 50000)
    #[serde(default = "default_extract_max_chars")]
    max_chars: usize,
}

fn default_extract_max_chars() -> usize {
    DEFAULT_EXTRACT_MAX_CHARS
}

pub struct BrowserExtractTool {
    pub browser: BrowserTool,
}

impl BrowserExtractTool {
    pub fn new(cdp_endpoint: impl Into<String>) -> Self {
        Self {
            browser: BrowserTool::new(cdp_endpoint),
        }
    }
}

#[async_trait]
impl Tool for BrowserExtractTool {
    fn name(&self) -> &str {
        "browser_extract"
    }
    fn description(&self) -> &str {
        "Extract rendered text content from the current browser page (via CDP).

## Capability

Returns the `innerText` of the whole page body, or of the first element matching an optional CSS selector. Includes the page title and URL. Content is truncated at `max_chars` (default 50,000). Call `browser_navigate` first.

## When to Use

- Reading JavaScript-rendered content that `web_fetch` cannot see (SPAs, dynamic pages).
- Extracting specific regions: articles (`article`), tables (`table`), etc.

## When NOT to Use

- Static pages: `web_fetch` is faster and needs no browser."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ExtractParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: ExtractParams = serde_json::from_value(args)?;
        match self.extract(&params, ctx).await {
            Ok(output) => Ok(output),
            Err(e) => Ok(ToolOutput::err(format!("browser_extract failed: {e}"))),
        }
    }
}

impl BrowserExtractTool {
    async fn extract(
        &self,
        params: &ExtractParams,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let sel_js = match &params.selector {
            Some(s) => js_string(s),
            None => "null".to_string(),
        };
        let expr = format!(
            r#"(() => {{
                const el = {sel} ? document.querySelector({sel}) : document.body;
                if (!el) return {{ found: false }};
                return {{
                    found: true,
                    title: document.title,
                    url: location.href,
                    text: el.innerText || ""
                }};
            }})()"#,
            sel = sel_js
        );

        let target = self.browser.first_page(&ctx.http_client).await?;
        let ws_url = require_ws_url(&target)?;
        let mut session = CdpSession::connect(&ws_url).await?;
        let value = evaluate(&mut session, &expr).await?;

        if value.get("found").and_then(Value::as_bool) != Some(true) {
            return Ok(ToolOutput::err(format!(
                "no element matches selector: {}",
                params.selector.as_deref().unwrap_or("body")
            )));
        }

        let title = value.get("title").and_then(Value::as_str).unwrap_or("");
        let url = value.get("url").and_then(Value::as_str).unwrap_or("");
        let text = value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let text = text
            .lines()
            .map(|l| l.trim_end())
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput::ok(truncate_chars(
            &format!("Source: {url}\nTitle: {title}\n\n{text}"),
            params.max_chars.max(1000),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_string_escaping() {
        assert_eq!(js_string("hello"), "\"hello\"");
        // Quotes and backslashes are escaped.
        assert_eq!(js_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        // JS line terminators must be escaped even though JSON allows them.
        assert_eq!(js_string("a\u{2028}b"), "\"a\\u2028b\"");
        assert_eq!(js_string("a\u{2029}b"), "\"a\\u2029b\"");
        // Newlines become \n escapes.
        assert_eq!(js_string("a\nb"), "\"a\\nb\"");
    }

    #[test]
    fn test_cdp_endpoint_from_env_default() {
        // The default must always parse as a URL with a host.
        let endpoint = std::env::var(CDP_ENDPOINT_ENV)
            .unwrap_or_else(|_| DEFAULT_CDP_ENDPOINT.to_string());
        let url = url::Url::parse(&endpoint).unwrap();
        assert!(url.host_str().is_some());
    }

    #[test]
    fn test_cdp_available_unreachable_port() {
        // Port 1 on localhost is virtually never open.
        assert!(!cdp_available("http://127.0.0.1:1"));
        // Invalid endpoint strings are never "available".
        assert!(!cdp_available("not a url"));
    }

    #[test]
    fn test_cdp_target_deserialization() {
        let json = serde_json::json!({
            "id": "ABC123",
            "type": "page",
            "title": "Example",
            "url": "https://example.com",
            "webSocketDebuggerUrl": "ws://localhost:9222/devtools/page/ABC123"
        });
        let target: CdpTarget = serde_json::from_value(json).unwrap();
        assert_eq!(target.id, "ABC123");
        assert_eq!(target.target_type, "page");
        assert_eq!(target.url, "https://example.com");
        assert!(target.ws_url.unwrap().starts_with("ws://"));
    }

    #[test]
    fn test_cdp_target_deserialization_minimal() {
        // Targets without a debugger URL (e.g. attached elsewhere).
        let json = serde_json::json!({ "id": "X", "type": "page" });
        let target: CdpTarget = serde_json::from_value(json).unwrap();
        assert!(target.ws_url.is_none());
        assert!(require_ws_url(&target).is_err());
    }

    #[test]
    fn test_truncate_chars() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        let t = truncate_chars("hello world", 5);
        assert!(t.starts_with("hello..."));
        assert!(t.contains("truncated at 5 characters"));
        // Unicode-safe.
        let t = truncate_chars("你好世界abc", 2);
        assert!(t.starts_with("你好..."));
    }

    #[test]
    fn test_navigate_params_deserialize() {
        let params: NavigateParams =
            serde_json::from_value(serde_json::json!({"url": "https://example.com"})).unwrap();
        assert_eq!(params.url, "https://example.com");
    }

    #[test]
    fn test_screenshot_params_defaults() {
        let params: ScreenshotParams = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(params.format, "png");
        assert!(!params.full_page);
    }

    #[test]
    fn test_type_params_defaults() {
        let params: TypeParams =
            serde_json::from_value(serde_json::json!({"selector": "#q", "text": "hi"})).unwrap();
        assert!(!params.submit);
    }

    #[test]
    fn test_extract_params_defaults() {
        let params: ExtractParams = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(params.selector.is_none());
        assert_eq!(params.max_chars, DEFAULT_EXTRACT_MAX_CHARS);
    }

    #[test]
    fn test_tool_names_and_schemas() {
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(BrowserNavigateTool::new("http://localhost:9222")),
            Box::new(BrowserScreenshotTool::new("http://localhost:9222")),
            Box::new(BrowserClickTool::new("http://localhost:9222")),
            Box::new(BrowserTypeTool::new("http://localhost:9222")),
            Box::new(BrowserExtractTool::new("http://localhost:9222")),
        ];
        let expected = [
            "browser_navigate",
            "browser_screenshot",
            "browser_click",
            "browser_type",
            "browser_extract",
        ];
        for (tool, name) in tools.iter().zip(expected.iter()) {
            assert_eq!(tool.name(), *name);
            let schema = tool.schema();
            assert_eq!(schema.name, *name);
            assert!(!schema.description.is_empty());
            assert!(schema.parameters.is_object());
        }
    }

    #[tokio::test]
    async fn test_list_targets_unreachable_returns_error() {
        let browser = BrowserTool::new("http://127.0.0.1:1");
        let client = reqwest::Client::new();
        let result = browser.list_targets(&client).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("could not list CDP targets"));
    }

    #[tokio::test]
    async fn test_navigate_without_browser_returns_err_output() {
        let tool = BrowserNavigateTool::new("http://127.0.0.1:1");
        let ctx = ToolContext::new(
            std::env::temp_dir(),
            pr_core::SearchConfig::default(),
        );
        let out = tool
            .execute(serde_json::json!({"url": "https://example.com"}), &ctx)
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("browser_navigate failed"));
    }

    /// Live round-trip test against a real browser. Ignored by default; run
    /// with a browser listening on the endpoint:
    ///   PR_CDP_LIVE=1 cargo test -p pr-tools live_cdp -- --ignored
    #[tokio::test]
    #[ignore]
    async fn test_live_cdp_roundtrip() {
        if std::env::var("PR_CDP_LIVE").is_err() {
            eprintln!("PR_CDP_LIVE not set, skipping live test");
            return;
        }
        let endpoint = cdp_endpoint_from_env();
        assert!(cdp_available(&endpoint), "no CDP endpoint at {endpoint}");

        let ctx = ToolContext::new(std::env::temp_dir(), pr_core::SearchConfig::default());

        // Navigate to a data URL (no external network needed).
        let nav = BrowserNavigateTool::new(endpoint.clone());
        let out = nav
            .execute(
                serde_json::json!({"url": "data:text/html,<html><head><title>PR Smoke</title></head><body><h1 id='h'>Hello CDP</h1><input id='q'/><button id='b' onclick=\"document.getElementById('h').innerText='clicked'\">go</button></body></html>"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success, "navigate failed: {}", out.content);

        // Extract full-page text.
        let extract = BrowserExtractTool::new(endpoint.clone());
        let out = extract.execute(serde_json::json!({}), &ctx).await.unwrap();
        assert!(out.success, "extract failed: {}", out.content);
        assert!(out.content.contains("Hello CDP"));

        // Type into the input, then verify the value via extract.
        let typer = BrowserTypeTool::new(endpoint.clone());
        let out = typer
            .execute(serde_json::json!({"selector": "#q", "text": "smoke"}), &ctx)
            .await
            .unwrap();
        assert!(out.success, "type failed: {}", out.content);

        // Click the button; the heading text should change.
        let click = BrowserClickTool::new(endpoint.clone());
        let out = click
            .execute(serde_json::json!({"selector": "#b"}), &ctx)
            .await
            .unwrap();
        assert!(out.success, "click failed: {}", out.content);

        let out = extract.execute(serde_json::json!({"selector": "#h"}), &ctx).await.unwrap();
        assert!(out.success);
        assert!(out.content.contains("clicked"));

        // Screenshot returns base64 image data.
        let shot = BrowserScreenshotTool::new(endpoint);
        let out = shot.execute(serde_json::json!({}), &ctx).await.unwrap();
        assert!(out.success, "screenshot failed: {}", out.content);
        let meta = out.metadata.expect("screenshot metadata");
        let b64 = meta["base64"].as_str().expect("base64 payload");
        assert!(b64.len() > 100);
    }
}
