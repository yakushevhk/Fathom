//! MCP (Model Context Protocol) client.
//!
//! Supports two transports:
//! - **stdio**: spawn the server as a child process and speak newline-delimited
//!   JSON-RPC over stdin/stdout.
//! - **Streamable HTTP**: POST JSON-RPC messages to a remote endpoint, with
//!   optional OAuth 2.0 client-credentials authentication. Responses may be
//!   plain JSON or Server-Sent Events (`text/event-stream`).
//!
//! The client keeps the server configs it connected with, so a dropped
//! connection can be re-established via [`McpClient::reconnect`]. Discovered
//! tool lists are cached and invalidated when the server sends the
//! `notifications/tools/list_changed` notification (dynamic tool discovery).

use pr_core::{McpConfig, McpServerConfig, ToolSchema};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Notification sent by an MCP server when its tool list has changed.
pub const TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";

/// Transport selection for connecting to an MCP server.
#[derive(Debug, Clone, PartialEq)]
pub enum McpTransport {
    Stdio { command: String, args: Vec<String> },
    Http { url: String, auth: Option<OAuthConfig> },
}

impl McpTransport {
    /// Derive the transport from an [`McpServerConfig`] section.
    pub fn from_config(config: &McpServerConfig) -> anyhow::Result<Self> {
        match config.transport.as_str() {
            "stdio" => {
                let command = config
                    .command
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("stdio transport requires `command`"))?;
                Ok(Self::Stdio {
                    command,
                    args: config.args.clone(),
                })
            }
            "http" | "streamable-http" | "sse" => {
                let url = config
                    .url
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("http transport requires `url`"))?;
                Ok(Self::Http { url, auth: None })
            }
            other => anyhow::bail!("unknown MCP transport: {other}"),
        }
    }
}

/// OAuth 2.0 client-credentials configuration for remote MCP servers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub token_url: String,
}

#[derive(Debug, Clone)]
struct OAuthToken {
    access_token: String,
    expires_at: Option<std::time::Instant>,
}

impl OAuthToken {
    fn is_expired(&self) -> bool {
        self.expires_at
            .map(|t| std::time::Instant::now() >= t)
            .unwrap_or(false)
    }
}

pub struct McpClient {
    connections: HashMap<String, McpConnection>,
    /// Configs of every server we connected to (enables reconnection).
    configs: HashMap<String, McpServerConfig>,
    /// Cached tool lists per server (dynamic discovery).
    tool_cache: HashMap<String, Vec<ToolSchema>>,
    /// Servers that signalled their tool list changed since the last fetch.
    tools_dirty: HashSet<String>,
    next_id: u64,
}

enum McpConnection {
    Stdio(StdioConn),
    Http(HttpConn),
}

struct StdioConn {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

struct HttpConn {
    client: reqwest::Client,
    url: String,
    auth: Option<OAuthConfig>,
    token: Option<OAuthToken>,
    session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    #[serde(default)]
    jsonrpc: String,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
    /// Present on notifications and server-initiated requests.
    #[serde(default)]
    method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Classification of an incoming JSON-RPC message.
#[derive(Debug, Clone, Copy, PartialEq)]
enum IncomingKind {
    /// Has an id and no method: the answer to one of our requests.
    Response,
    /// No id, has a method: fire-and-forget server notification.
    Notification,
    /// Has both id and method: server-initiated request (unsupported).
    ServerRequest,
}

fn classify(value: &serde_json::Value) -> IncomingKind {
    let has_id = value.get("id").map(|v| !v.is_null()).unwrap_or(false);
    let has_method = value.get("method").map(|v| !v.is_null()).unwrap_or(false);
    match (has_id, has_method) {
        (true, false) => IncomingKind::Response,
        (false, true) => IncomingKind::Notification,
        (true, true) => IncomingKind::ServerRequest,
        (false, false) => IncomingKind::Response, // malformed; let caller deal with it
    }
}

/// Extract the `data:` payloads from a Server-Sent Events body.
///
/// Multi-line `data:` fields within one event are joined with newlines per
/// the SSE specification; events are separated by blank lines.
pub fn parse_sse_data_events(body: &str) -> Vec<String> {
    let mut events = Vec::new();
    let mut data_lines: Vec<&str> = Vec::new();

    for line in body.lines() {
        if line.is_empty() {
            if !data_lines.is_empty() {
                events.push(data_lines.join("\n"));
                data_lines.clear();
            }
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
        // Ignore `event:`, `id:`, `retry:` and comment lines.
    }
    if !data_lines.is_empty() {
        events.push(data_lines.join("\n"));
    }
    events
}

/// Parse an OAuth token endpoint response body.
///
/// Returns the access token and its lifetime (with a 30s safety margin).
fn parse_token_response(body: &serde_json::Value) -> anyhow::Result<(String, Option<Duration>)> {
    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("token response missing `access_token`"))?
        .to_string();
    let lifetime = body.get("expires_in").and_then(|v| v.as_u64()).map(|secs| {
        // Refresh a bit before actual expiry.
        Duration::from_secs(secs.saturating_sub(30).max(1))
    });
    Ok((access_token, lifetime))
}

impl McpConnection {
    /// Perform a request/response exchange.
    ///
    /// For the stdio transport, notifications and server requests arriving
    /// while waiting for the response are skipped; the notification methods
    /// are collected and returned alongside the response.
    async fn request(
        &mut self,
        req: &JsonRpcRequest,
    ) -> anyhow::Result<(JsonRpcResponse, Vec<String>)> {
        match self {
            Self::Stdio(c) => {
                let line = serde_json::to_string(req)?;
                c.stdin.write_all(line.as_bytes()).await?;
                c.stdin.write_all(b"\n").await?;
                c.stdin.flush().await?;

                let expected_id = req.id;
                let mut notifications = Vec::new();

                loop {
                    let mut buf = String::new();
                    // Bounded wait: a hung MCP server must not wedge the
                    // global client (and every agent using MCP tools).
                    let n = tokio::time::timeout(
                        std::time::Duration::from_secs(60),
                        c.stdout.read_line(&mut buf),
                    )
                    .await
                    .map_err(|_| {
                        anyhow::anyhow!("timed out waiting for MCP server response")
                    })??;
                    if n == 0 {
                        anyhow::bail!("MCP server closed the connection");
                    }
                    let value: serde_json::Value = match serde_json::from_str(buf.trim()) {
                        Ok(v) => v,
                        Err(_) => continue, // not JSON — ignore (e.g. server log noise)
                    };
                    match classify(&value) {
                        IncomingKind::Notification => {
                            if let Some(m) = value.get("method").and_then(|m| m.as_str()) {
                                notifications.push(m.to_string());
                            }
                        }
                        IncomingKind::ServerRequest => {
                            tracing::debug!("ignoring unsupported MCP server request");
                        }
                        IncomingKind::Response => {
                            let resp: JsonRpcResponse = serde_json::from_value(value)?;
                            if expected_id.is_some() && resp.id != expected_id {
                                continue; // stale response for a different request id
                            }
                            return Ok((resp, notifications));
                        }
                    }
                }
            }
            Self::Http(c) => {
                let resp = c.post(req).await?;
                Ok((resp, Vec::new()))
            }
        }
    }

    /// Send a notification (no response expected).
    async fn notify(&mut self, method: &str) -> anyhow::Result<()> {
        let msg = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.to_string(),
            params: None,
        };
        match self {
            Self::Stdio(c) => {
                let line = serde_json::to_string(&msg)?;
                c.stdin.write_all(line.as_bytes()).await?;
                c.stdin.write_all(b"\n").await?;
                c.stdin.flush().await?;
                Ok(())
            }
            Self::Http(c) => {
                c.post(&msg).await?;
                Ok(())
            }
        }
    }
}

impl HttpConn {
    /// Return a valid access token, fetching/refreshing it when needed.
    async fn ensure_token(&mut self) -> anyhow::Result<Option<String>> {
        let Some(auth) = self.auth.clone() else {
            return Ok(None);
        };
        if let Some(token) = &self.token {
            if !token.is_expired() {
                return Ok(Some(token.access_token.clone()));
            }
        }
        let response = self
            .client
            .post(&auth.token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", auth.client_id.as_str()),
                ("client_secret", auth.client_secret.as_str()),
            ])
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OAuth token request failed (HTTP {status}): {body}");
        }
        let body: serde_json::Value = response.json().await?;
        let (access_token, lifetime) = parse_token_response(&body)?;
        let token = OAuthToken {
            access_token: access_token.clone(),
            expires_at: lifetime.map(|d| std::time::Instant::now() + d),
        };
        self.token = Some(token);
        Ok(Some(access_token))
    }

    /// POST one JSON-RPC message and parse the (JSON or SSE) response.
    async fn post(&mut self, msg: &JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        let token = self.ensure_token().await?;

        let mut builder = self
            .client
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .json(msg);
        if let Some(token) = token {
            builder = builder.bearer_auth(token);
        }
        if let Some(session_id) = &self.session_id {
            builder = builder.header("mcp-session-id", session_id);
        }

        let response = builder.send().await?;
        let status = response.status();

        // The server hands out a session id during initialization.
        if let Some(sid) = response.headers().get("mcp-session-id") {
            if let Ok(s) = sid.to_str() {
                self.session_id = Some(s.to_string());
            }
        }

        // Notifications are acknowledged with 202 and no body.
        if status == reqwest::StatusCode::ACCEPTED {
            return Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: msg.id,
                result: None,
                error: None,
                method: None,
            });
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("MCP HTTP error {status}: {body}");
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = response.text().await?;

        if content_type.contains("text/event-stream") {
            // The actual JSON-RPC response is one of the SSE data events.
            for data in parse_sse_data_events(&body).into_iter().rev() {
                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&data) {
                    return Ok(resp);
                }
            }
            anyhow::bail!("no JSON-RPC response found in SSE stream")
        }

        Ok(serde_json::from_str(&body)?)
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            configs: HashMap::new(),
            tool_cache: HashMap::new(),
            tools_dirty: HashSet::new(),
            next_id: 0,
        }
    }

    fn next_request_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Connect using the transport declared in `config`.
    ///
    /// Existing connections are reused, so calling this twice with the same
    /// server name is cheap (connection pooling).
    pub async fn connect(&mut self, config: &McpServerConfig) -> anyhow::Result<()> {
        if self.connections.contains_key(&config.name) {
            tracing::debug!("MCP server {} already connected, reusing", config.name);
            self.configs.insert(config.name.clone(), config.clone());
            return Ok(());
        }
        match McpTransport::from_config(config)? {
            McpTransport::Stdio { .. } => self.connect_stdio(config).await?,
            McpTransport::Http { url, auth } => {
                self.connect_http(&config.name, url, auth).await?
            }
        }
        self.configs.insert(config.name.clone(), config.clone());
        Ok(())
    }

    /// Connect to every server declared in the `[mcp]` config section and
    /// initialize it. Individual failures are logged, not fatal.
    pub async fn connect_all(&mut self, config: &McpConfig) -> anyhow::Result<()> {
        for server in &config.servers {
            if let Err(e) = self.connect(server).await {
                tracing::warn!("MCP connect failed for {}: {e}", server.name);
                continue;
            }
            if let Err(e) = self.initialize(&server.name).await {
                tracing::warn!("MCP initialize failed for {}: {e}", server.name);
            }
        }
        Ok(())
    }

    pub async fn connect_stdio(&mut self, config: &McpServerConfig) -> anyhow::Result<()> {
        let command = config
            .command
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("stdio transport requires command"))?;

        let mut cmd = Command::new(command);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let mut child = cmd.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to get stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to get stdout"))?;

        let conn = McpConnection::Stdio(StdioConn {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        });

        self.connections.insert(config.name.clone(), conn);
        tracing::info!("Connected to MCP server: {}", config.name);
        Ok(())
    }

    pub async fn connect_http(
        &mut self,
        name: &str,
        url: String,
        auth: Option<OAuthConfig>,
    ) -> anyhow::Result<()> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;
        let conn = McpConnection::Http(HttpConn {
            client,
            url: url.clone(),
            auth,
            token: None,
            session_id: None,
        });
        self.connections.insert(name.to_string(), conn);
        tracing::info!("Connected to MCP server (http): {name} -> {url}");
        Ok(())
    }

    pub async fn initialize(&mut self, server_name: &str) -> anyhow::Result<()> {
        let id = self.next_request_id();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "parallel-research",
                    "version": "0.1.0"
                }
            })),
        };

        let notifications = {
            let conn = self
                .connections
                .get_mut(server_name)
                .ok_or_else(|| anyhow::anyhow!("server not connected: {server_name}"))?;
            let (response, notifications) = conn.request(&request).await?;
            if let Some(error) = response.error {
                anyhow::bail!("MCP initialize error: {}", error.message);
            }
            conn.notify("notifications/initialized").await?;
            notifications
        };
        self.mark_tools_dirty(server_name, &notifications);
        Ok(())
    }

    pub async fn list_tools(&mut self, server_name: &str) -> anyhow::Result<Vec<ToolSchema>> {
        let id = self.next_request_id();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: "tools/list".to_string(),
            params: None,
        };

        let conn = self
            .connections
            .get_mut(server_name)
            .ok_or_else(|| anyhow::anyhow!("server not connected: {server_name}"))?;
        let (response, _notifications) = conn.request(&request).await?;

        if let Some(error) = response.error {
            anyhow::bail!("MCP error: {}", error.message);
        }

        let result = response
            .result
            .ok_or_else(|| anyhow::anyhow!("no result in response"))?;

        let tools: Vec<McpToolDef> = serde_json::from_value(
            result.get("tools").cloned().unwrap_or(serde_json::json!([])),
        )?;

        let schemas: Vec<ToolSchema> = tools
            .into_iter()
            .map(|t| ToolSchema {
                name: t.name,
                description: t.description.unwrap_or_default(),
                parameters: t.input_schema.unwrap_or(serde_json::json!({"type": "object"})),
            })
            .collect();

        // We just fetched a fresh list, so it becomes the new cache and the
        // staleness flag is cleared.
        self.tool_cache.insert(server_name.to_string(), schemas.clone());
        self.tools_dirty.remove(server_name);
        Ok(schemas)
    }

    /// Dynamic tool discovery: return the cached tool list when it is still
    /// valid, or fetch a fresh one via `tools/list` when the server signalled
    /// a change (or nothing was cached yet).
    pub async fn discover_tools(&mut self, server_name: &str) -> anyhow::Result<Vec<ToolSchema>> {
        if !self.tools_dirty.contains(server_name) {
            if let Some(cached) = self.tool_cache.get(server_name) {
                return Ok(cached.clone());
            }
        }
        self.list_tools(server_name).await
    }

    /// Cached tool list for a server, if one has been fetched.
    pub fn cached_tools(&self, server_name: &str) -> Option<&[ToolSchema]> {
        self.tool_cache.get(server_name).map(|v| v.as_slice())
    }

    pub async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let id = self.next_request_id();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": tool_name,
                "arguments": args,
            })),
        };

        let conn = self
            .connections
            .get_mut(server_name)
            .ok_or_else(|| anyhow::anyhow!("server not connected: {server_name}"))?;
        let (response, notifications) = conn.request(&request).await?;
        self.mark_tools_dirty(server_name, &notifications);

        if let Some(error) = response.error {
            anyhow::bail!("MCP tool call error: {}", error.message);
        }

        Ok(response.result.unwrap_or(serde_json::json!({})))
    }

    /// Re-establish a previously configured connection (e.g. after the server
    /// process died or the remote endpoint was briefly unreachable).
    pub async fn reconnect(&mut self, server_name: &str) -> anyhow::Result<()> {
        let config = self
            .configs
            .get(server_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no config known for server: {server_name}"))?;

        self.disconnect(server_name).await;
        self.tool_cache.remove(server_name);
        self.tools_dirty.remove(server_name);

        self.connect(&config).await?;
        self.initialize(server_name).await?;
        tracing::info!("Reconnected MCP server: {server_name}");
        Ok(())
    }

    /// Drop a single connection (kills the child process for stdio).
    pub async fn disconnect(&mut self, server_name: &str) {
        if let Some(conn) = self.connections.remove(server_name) {
            if let McpConnection::Stdio(mut c) = conn {
                let _ = c.child.kill().await;
            }
        }
    }

    pub fn is_connected(&self, server_name: &str) -> bool {
        self.connections.contains_key(server_name)
    }

    pub fn connected_servers(&self) -> Vec<String> {
        self.connections.keys().cloned().collect()
    }

    pub async fn shutdown(&mut self) {
        let names: Vec<String> = self.connections.keys().cloned().collect();
        for name in names {
            self.disconnect(&name).await;
        }
    }

    fn mark_tools_dirty(&mut self, server_name: &str, notifications: &[String]) {
        if notifications.iter().any(|n| n == TOOLS_LIST_CHANGED) {
            tracing::debug!("MCP server {server_name} signalled tool list change");
            self.tools_dirty.insert(server_name.to_string());
        }
    }
}

#[derive(Debug, Deserialize)]
struct McpToolDef {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    input_schema: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    // ── Pure helpers ────────────────────────────────────────────────────

    #[test]
    fn test_classify_messages() {
        let resp = serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {}});
        assert_eq!(classify(&resp), IncomingKind::Response);

        let notif = serde_json::json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"});
        assert_eq!(classify(&notif), IncomingKind::Notification);

        let server_req = serde_json::json!({"jsonrpc": "2.0", "id": 9, "method": "sampling/createMessage"});
        assert_eq!(classify(&server_req), IncomingKind::ServerRequest);
    }

    #[test]
    fn test_parse_sse_single_and_multi_line_events() {
        let body = "event: message\ndata: {\"a\":1}\n\ndata: line1\ndata: line2\n\n";
        let events = parse_sse_data_events(body);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], "{\"a\":1}");
        assert_eq!(events[1], "line1\nline2");
    }

    #[test]
    fn test_parse_sse_ignores_comments_and_ids() {
        let body = ": keep-alive\nid: 7\nretry: 3000\ndata: hello\n\n";
        let events = parse_sse_data_events(body);
        assert_eq!(events, vec!["hello".to_string()]);
    }

    #[test]
    fn test_parse_sse_unterminated_event_is_returned() {
        let events = parse_sse_data_events("data: tail");
        assert_eq!(events, vec!["tail".to_string()]);
    }

    #[test]
    fn test_parse_token_response() {
        let (token, lifetime) = parse_token_response(&serde_json::json!({
            "access_token": "abc123",
            "token_type": "Bearer",
            "expires_in": 3600
        }))
        .unwrap();
        assert_eq!(token, "abc123");
        assert_eq!(lifetime, Some(Duration::from_secs(3570)));

        // Missing token -> error.
        assert!(parse_token_response(&serde_json::json!({})).is_err());

        // No expiry -> unlimited.
        let (_, lifetime) =
            parse_token_response(&serde_json::json!({"access_token": "x"})).unwrap();
        assert!(lifetime.is_none());
    }

    #[test]
    fn test_transport_from_config_stdio() {
        let config = McpServerConfig {
            name: "local".to_string(),
            transport: "stdio".to_string(),
            command: Some("mcp-server".to_string()),
            args: vec!["--flag".to_string()],
            url: None,
        };
        match McpTransport::from_config(&config).unwrap() {
            McpTransport::Stdio { command, args } => {
                assert_eq!(command, "mcp-server");
                assert_eq!(args, vec!["--flag".to_string()]);
            }
            other => panic!("expected Stdio, got {other:?}"),
        }
    }

    #[test]
    fn test_transport_from_config_http() {
        let config = McpServerConfig {
            name: "remote".to_string(),
            transport: "http".to_string(),
            command: None,
            args: vec![],
            url: Some("https://mcp.example.com/api".to_string()),
        };
        match McpTransport::from_config(&config).unwrap() {
            McpTransport::Http { url, auth } => {
                assert_eq!(url, "https://mcp.example.com/api");
                assert!(auth.is_none());
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn test_transport_from_config_errors() {
        let missing_command = McpServerConfig {
            name: "x".to_string(),
            transport: "stdio".to_string(),
            command: None,
            args: vec![],
            url: None,
        };
        assert!(McpTransport::from_config(&missing_command).is_err());

        let missing_url = McpServerConfig {
            name: "x".to_string(),
            transport: "http".to_string(),
            command: None,
            args: vec![],
            url: None,
        };
        assert!(McpTransport::from_config(&missing_url).is_err());

        let unknown = McpServerConfig {
            name: "x".to_string(),
            transport: "carrier-pigeon".to_string(),
            command: None,
            args: vec![],
            url: None,
        };
        assert!(McpTransport::from_config(&unknown).is_err());
    }

    #[test]
    fn test_oauth_config_serde_roundtrip() {
        let toml_str = r#"
client_id = "id"
client_secret = "secret"
token_url = "https://auth.example.com/token"
"#;
        let config: OAuthConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.client_id, "id");
        assert_eq!(config.token_url, "https://auth.example.com/token");
    }

    #[test]
    fn test_json_rpc_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let s = serde_json::to_string(&req).unwrap();
        // Notifications must not carry an id field.
        assert!(!s.contains("\"id\""));
        assert!(s.contains("notifications/initialized"));
    }

    #[tokio::test]
    async fn test_reconnect_unknown_server_fails() {
        let mut client = McpClient::new();
        assert!(client.reconnect("never-seen").await.is_err());
        assert!(client.connected_servers().is_empty());
    }

    #[test]
    fn test_connected_state_tracking() {
        let client = McpClient::new();
        assert!(!client.is_connected("a"));
        assert!(client.cached_tools("a").is_none());
    }

    // ── HTTP transport end-to-end (fake server) ─────────────────────────

    /// Minimal HTTP server speaking one request per connection.
    ///
    /// Routes by path: `/token` answers the OAuth token request; every other
    /// path consumes the next queued JSON-RPC response (or answers 202 when
    /// the queue is empty, i.e. for notifications). Stops after
    /// `max_requests` requests and returns everything it received.
    async fn spawn_fake_server(
        mut responses: Vec<serde_json::Value>,
        max_requests: usize,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            let mut requests = Vec::new();
            while requests.len() < max_requests {
                let accept = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    listener.accept(),
                )
                .await;
                let Ok(Ok((mut socket, _))) = accept else {
                    break;
                };

                // Read headers.
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let header_end = loop {
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        break 0;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                        break pos;
                    }
                };
                let head = String::from_utf8_lossy(&buf).to_string();
                let request_line = head.lines().next().unwrap_or("").to_string();
                let content_length = head
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().to_string())
                    })
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);

                // Read the remaining body bytes if needed.
                let body_start = header_end + 4;
                let mut body = buf[body_start.min(buf.len())..].to_vec();
                while body.len() < content_length {
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    body.extend_from_slice(&chunk[..n]);
                }
                let body_str =
                    String::from_utf8_lossy(&body[..content_length.min(body.len())]).to_string();
                requests.push(format!("{head}\n{body_str}"));

                let path = request_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .to_string();

                let response_body = if path.starts_with("/token") {
                    Some(serde_json::to_string(&serde_json::json!({
                        "access_token": "test-token",
                        "token_type": "Bearer",
                        "expires_in": 3600
                    }))
                    .unwrap())
                } else if !body_str.contains("\"id\":") {
                    // JSON-RPC notification (no id): acknowledge, consume nothing.
                    None
                } else if !responses.is_empty() {
                    Some(serde_json::to_string(&responses.remove(0)).unwrap())
                } else {
                    None
                };

                if let Some(body) = response_body {
                    let http = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    socket.write_all(http.as_bytes()).await.unwrap();
                } else {
                    // Notification: acknowledge with no body.
                    socket
                        .write_all(
                            b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .await
                        .unwrap();
                }
                let _ = socket.shutdown().await;
            }
            requests
        });

        (addr, handle)
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    #[tokio::test]
    async fn test_http_transport_full_flow() {
        let responses = vec![
            // initialize
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "result": {"protocolVersion": "2024-11-05", "capabilities": {}, "serverInfo": {"name": "fake"}}
            }),
            // tools/list
            serde_json::json!({
                "jsonrpc": "2.0", "id": 2,
                "result": {"tools": [{"name": "echo", "description": "Echoes input", "inputSchema": {"type": "object"}}]}
            }),
            // tools/call
            serde_json::json!({
                "jsonrpc": "2.0", "id": 3,
                "result": {"content": [{"type": "text", "text": "echoed"}], "isError": false}
            }),
        ];
        // initialize + notifications/initialized + tools/list + tools/call
        let (addr, server) = spawn_fake_server(responses, 4).await;

        let mut client = McpClient::new();
        let config = McpServerConfig {
            name: "fake".to_string(),
            transport: "http".to_string(),
            command: None,
            args: vec![],
            url: Some(format!("http://{addr}/mcp")),
        };
        client.connect(&config).await.unwrap();
        assert!(client.is_connected("fake"));

        // Reusing an existing connection is a no-op (pooling).
        client.connect(&config).await.unwrap();

        client.initialize("fake").await.unwrap();

        let tools = client.list_tools("fake").await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");

        // Discovery serves the cache without another round-trip.
        let cached = client.discover_tools("fake").await.unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(client.cached_tools("fake").unwrap().len(), 1);

        let result = client
            .call_tool("fake", "echo", serde_json::json!({"text": "hi"}))
            .await
            .unwrap();
        assert_eq!(result["content"][0]["text"], "echoed");

        client.shutdown().await;
        assert!(!client.is_connected("fake"));

        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 4);
        assert!(requests[0].contains("/mcp"));
        assert!(requests[1].contains("notifications/initialized"));
    }

    #[tokio::test]
    async fn test_http_transport_with_oauth() {
        let responses = vec![serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"protocolVersion": "2024-11-05", "capabilities": {}}
        })];
        // token request + initialize + notifications/initialized
        let (addr, server) = spawn_fake_server(responses, 3).await;

        let mut client = McpClient::new();
        client
            .connect_http(
                "secure",
                format!("http://{addr}/mcp"),
                Some(OAuthConfig {
                    client_id: "id".to_string(),
                    client_secret: "secret".to_string(),
                    token_url: format!("http://{addr}/token"),
                }),
            )
            .await
            .unwrap();

        client.initialize("secure").await.unwrap();
        client.shutdown().await;

        let requests = server.await.unwrap();
        // First request hits the token endpoint, then initialize goes out
        // with the bearer token.
        assert!(requests[0].starts_with("POST /token"));
        assert!(requests[0].contains("grant_type=client_credentials"));
        let mcp_req = requests
            .iter()
            .find(|r| r.contains("/mcp"))
            .expect("mcp request");
        assert!(mcp_req.contains("authorization: Bearer test-token"));
    }

    /// End-to-end stdio test against a shell-script fake MCP server. The
    /// server echoes back each request's id and emits a
    /// `notifications/tools/list_changed` after `initialized`, which also
    /// exercises the notification-skipping logic in the read loop.
    #[tokio::test]
    #[cfg(unix)]
    async fn test_stdio_transport_full_flow_with_dynamic_tools() {
        let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"method":"initialize"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"2024-11-05"}}\n' "$id" ;;
    *'"method":"notifications/initialized"'*)
      printf '{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}\n' ;;
    *'"method":"tools/list"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"ping","description":"p"}]}}\n' "$id" ;;
    *'"method":"tools/call"'*)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"ok":true}}\n' "$id" ;;
  esac
done
"#;

        let mut client = McpClient::new();
        let config = McpServerConfig {
            name: "shfake".to_string(),
            transport: "stdio".to_string(),
            command: Some("sh".to_string()),
            args: vec!["-c".to_string(), script.to_string()],
            url: None,
        };
        client.connect(&config).await.unwrap();
        client.initialize("shfake").await.unwrap();

        let tools = client.list_tools("shfake").await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "ping");

        // discover_tools serves the fresh cache without another round-trip.
        let cached = client.discover_tools("shfake").await.unwrap();
        assert_eq!(cached.len(), 1);
        assert!(client.cached_tools("shfake").is_some());

        // Force staleness: discovery must refresh via a new tools/list call.
        client.tools_dirty.insert("shfake".to_string());
        let refreshed = client.discover_tools("shfake").await.unwrap();
        assert_eq!(refreshed.len(), 1);
        assert!(!client.tools_dirty.contains("shfake"));

        let result = client
            .call_tool("shfake", "ping", serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result["ok"], true);

        // Reconnect kills the old process and spins up a new one.
        client.reconnect("shfake").await.unwrap();
        assert!(client.is_connected("shfake"));
        let tools = client.list_tools("shfake").await.unwrap();
        assert_eq!(tools.len(), 1);

        client.shutdown().await;
        assert!(!client.is_connected("shfake"));
    }

    #[tokio::test]
    async fn test_http_transport_error_status() {        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf).await;
                socket
                    .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\nConnection: close\r\n\r\nboom")
                    .await
                    .unwrap();
            }
        });

        let mut client = McpClient::new();
        client
            .connect_http("broken", format!("http://{addr}/mcp"), None)
            .await
            .unwrap();
        let err = client.initialize("broken").await.unwrap_err();
        assert!(err.to_string().contains("500"));
        server.abort();
    }
}
