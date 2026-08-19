use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use std::collections::HashMap;
use std::sync::Arc;

/// A JSON-RPC 2.0 message sent to/from an LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Pending request waiting for a response.
struct PendingRequest {
    reply_tx: oneshot::Sender<Result<Value, String>>,
}

/// An LSP client that communicates with a language server over stdio.
pub struct LspClient {
    /// Channel to send requests to the writer task
    request_tx: mpsc::UnboundedSender<LspRequest>,
    /// Pending requests waiting for responses
    pending: Arc<Mutex<HashMap<u64, PendingRequest>>>,
    /// The child process (kept alive)
    _child: Child,
    /// Name of the LSP server command
    server_name: String,
}

/// Internal request type sent to the writer task.
struct LspRequest {
    id: u64,
    method: String,
    params: Value,
    reply_tx: oneshot::Sender<Result<Value, String>>,
}

impl LspClient {
    /// Spawn an LSP server process and connect to it via stdio.
    pub async fn spawn(
        command: &str,
        args: &[String],
        root_path: &Path,
    ) -> anyhow::Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to spawn LSP server '{}': {}", command, e))?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;

        let pending: Arc<Mutex<HashMap<u64, PendingRequest>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<LspRequest>();

        // Writer task: reads requests from channel, writes JSON-RPC to stdin
        let pending_writer = pending.clone();
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(req) = request_rx.recv().await {
                // Register pending request
                {
                    let mut map = pending_writer.lock().await;
                    map.insert(req.id, PendingRequest { reply_tx: req.reply_tx });
                }
                // Send JSON-RPC message
                let msg = JsonRpcMessage {
                    jsonrpc: "2.0".into(),
                    id: Some(req.id),
                    method: Some(req.method),
                    params: Some(req.params),
                    result: None,
                    error: None,
                };
                let body = match serde_json::to_string(&msg) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                let header = format!("Content-Length: {}\r\n\r\n", body.len());
                if stdin.write_all(header.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(body.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });

        // Reader task: reads JSON-RPC from stdout, dispatches responses
        let pending_reader = pending.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                // Read headers
                let mut content_length: Option<usize> = None;
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line).await {
                        Ok(0) => return, // EOF
                        Ok(_) => {}
                        Err(_) => return,
                    }
                    let line = line.trim();
                    if line.is_empty() {
                        break;
                    }
                    if let Some(val) = line.strip_prefix("Content-Length: ") {
                        content_length = val.parse().ok();
                    }
                }
                let len = match content_length {
                    Some(l) => l,
                    None => continue,
                };
                let mut buf = vec![0u8; len];
                if tokio::io::AsyncReadExt::read_exact(&mut reader, &mut buf).await.is_err() {
                    return;
                }
                let text = String::from_utf8_lossy(&buf);
                if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(&text) {
                    if let Some(id) = msg.id {
                        let result = if let Some(err) = msg.error {
                            Err(err.message)
                        } else {
                            Ok(msg.result.unwrap_or(Value::Null))
                        };
                        let mut map = pending_reader.lock().await;
                        if let Some(pending) = map.remove(&id) {
                            let _ = pending.reply_tx.send(result);
                        }
                    }
                }
            }
        });

        let server_name = command.to_string();
        let mut client = Self {
            request_tx,
            pending,
            _child: child,
            server_name,
        };

        // Send initialize request
        let root_uri = format!("file://{}", root_path.display());
        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "definition": { "dynamicRegistration": false },
                    "references": { "dynamicRegistration": false },
                    "documentSymbol": { "dynamicRegistration": false, "hierarchicalDocumentSymbolSupport": true },
                    "completion": { "dynamicRegistration": false }
                },
                "workspace": {
                    "symbol": { "dynamicRegistration": false }
                }
            }
        });

        let _init_result = client.request("initialize", init_params).await?;
        client.notify_raw("initialized", serde_json::json!({})).await?;

        Ok(client)
    }

    /// Send a request and wait for the response.
    pub async fn request(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let (tx, rx) = oneshot::channel();
        self.request_tx.send(LspRequest {
            id,
            method: method.to_string(),
            params,
            reply_tx: tx,
        })?;

        let result = rx.await.map_err(|_| anyhow::anyhow!("LSP channel closed"))?;
        result.map_err(|e| anyhow::anyhow!("LSP error: {}", e))
    }

    /// Send a notification (no response expected) via the writer channel.
    async fn notify_raw(&self, method: &str, params: Value) -> anyhow::Result<()> {
        // Notifications don't have an id and don't expect a response.
        // We send through the same channel but with a dummy reply_tx that we drop.
        let (tx, _rx) = oneshot::channel();
        self.request_tx.send(LspRequest {
            id: 0, // notifications use id 0
            method: method.to_string(),
            params,
            reply_tx: tx,
        })?;
        Ok(())
    }

    /// Query document symbols (functions, classes, etc.)
    pub async fn document_symbols(&self, file_uri: &str) -> anyhow::Result<Value> {
        self.request("textDocument/documentSymbol", serde_json::json!({
            "textDocument": { "uri": file_uri }
        })).await
    }

    /// Go to definition
    pub async fn goto_definition(
        &self,
        file_uri: &str,
        line: u32,
        character: u32,
    ) -> anyhow::Result<Value> {
        self.request("textDocument/definition", serde_json::json!({
            "textDocument": { "uri": file_uri },
            "position": { "line": line, "character": character }
        })).await
    }

    /// Find references
    pub async fn find_references(
        &self,
        file_uri: &str,
        line: u32,
        character: u32,
    ) -> anyhow::Result<Value> {
        self.request("textDocument/references", serde_json::json!({
            "textDocument": { "uri": file_uri },
            "position": { "line": line, "character": character },
            "context": { "includeDeclaration": true }
        })).await
    }

    /// Hover (type info, documentation)
    pub async fn hover(
        &self,
        file_uri: &str,
        line: u32,
        character: u32,
    ) -> anyhow::Result<Value> {
        self.request("textDocument/hover", serde_json::json!({
            "textDocument": { "uri": file_uri },
            "position": { "line": line, "character": character }
        })).await
    }

    /// Workspace symbol search
    pub async fn workspace_symbols(&self, query: &str) -> anyhow::Result<Value> {
        self.request("workspace/symbol", serde_json::json!({
            "query": query
        })).await
    }

    /// Shut down the LSP server gracefully.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let _ = self.request("shutdown", Value::Null).await;
        let _ = self.notify_raw("exit", Value::Null).await;
        Ok(())
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Child process will be killed when dropped
    }
}
