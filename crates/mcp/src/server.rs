use pr_core::ToolSchema;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// MCP server exposing the agent's tools to external MCP clients
/// (Claude, ZCode, ...). Two modes:
///
/// - schema-only ([`McpServer::new`]): `tools/list` works, `tools/call`
///   answers with a placeholder (used by tests and discovery-only setups);
/// - executor mode ([`McpServer::with_executor`]): `tools/call` actually
///   runs the tool through the shared [`pr_tools::ToolRegistry`].
pub struct McpServer {
    tools: Vec<ToolSchema>,
    executor: Option<(Arc<pr_tools::ToolRegistry>, Arc<pr_tools::ToolContext>)>,
}

impl McpServer {
    pub fn new(tools: Vec<ToolSchema>) -> Self {
        Self { tools, executor: None }
    }

    /// Build a server that really executes tools.
    pub fn with_executor(
        registry: Arc<pr_tools::ToolRegistry>,
        ctx: Arc<pr_tools::ToolContext>,
    ) -> Self {
        let tools = registry.list_schemas();
        Self {
            tools,
            executor: Some((registry, ctx)),
        }
    }

    /// Run MCP server over stdio (one JSON-RPC message per line).
    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);

        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                break;
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Notifications (no `id`) must not receive a response.
            let Some(response) = self.handle_request(line).await else {
                continue;
            };
            let response_str = serde_json::to_string(&response)?;
            stdout.write_all(response_str.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    /// Handle one JSON-RPC request line; returns `None` for notifications.
    async fn handle_request(&self, request: &str) -> Option<serde_json::Value> {
        let req: serde_json::Value = match serde_json::from_str(request) {
            Ok(v) => v,
            Err(_) => {
                return Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": "Parse error"},
                }))
            }
        };

        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // JSON-RPC notification: no id => no response.
        if id.is_none() || id == Some(serde_json::Value::Null) {
            return None;
        }

        Some(match method {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {
                        "name": "parallel-research",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
            "tools/list" => {
                let tools: Vec<serde_json::Value> = self
                    .tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.parameters,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {"tools": tools}
                })
            }
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(serde_json::json!({}));
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                let Some((registry, ctx)) = &self.executor else {
                    return Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{"type": "text", "text": "Tool execution not available (schema-only server)"}],
                            "isError": true
                        }
                    }));
                };

                let output = registry.execute(name, arguments, ctx).await;
                match output {
                    Ok(tool_output) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{"type": "text", "text": tool_output.content}],
                            "isError": !tool_output.success
                        }
                    }),
                    Err(e) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{"type": "text", "text": format!("tool execution failed: {e}")}],
                            "isError": true
                        }
                    }),
                }
            }
            "ping" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            }),
            _ => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": "Method not found"}
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> McpServer {
        McpServer::new(vec![ToolSchema {
            name: "test_tool".into(),
            description: "A test tool".into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }])
    }

    #[tokio::test]
    async fn handle_initialize() {
        let srv = server();
        let resp = srv
            .handle_request(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#)
            .await
            .unwrap();
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(resp["result"]["serverInfo"]["name"], "parallel-research");
    }

    #[tokio::test]
    async fn handle_tools_list() {
        let srv = server();
        let resp = srv
            .handle_request(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
            .await
            .unwrap();
        assert_eq!(resp["id"], 2);
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "test_tool");
        assert_eq!(tools[0]["description"], "A test tool");
    }

    #[tokio::test]
    async fn handle_tools_call_schema_only_mode() {
        let srv = server();
        let resp = srv
            .handle_request(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"test_tool"}}"#)
            .await
            .unwrap();
        assert_eq!(resp["id"], 3);
        assert!(resp["result"]["content"].is_array());
        assert_eq!(resp["result"]["isError"], true);
    }

    #[tokio::test]
    async fn notifications_get_no_response() {
        let srv = server();
        assert!(srv
            .handle_request(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .await
            .is_none());
        assert!(srv.handle_request(r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#).await.is_none());
    }

    #[tokio::test]
    async fn handle_unknown_method() {
        let srv = server();
        let resp = srv
            .handle_request(r#"{"jsonrpc":"2.0","id":4,"method":"unknown/method"}"#)
            .await
            .unwrap();
        assert_eq!(resp["error"]["code"], -32601);
        assert_eq!(resp["error"]["message"], "Method not found");
    }

    #[tokio::test]
    async fn handle_invalid_json() {
        let srv = server();
        let resp = srv.handle_request("not json at all").await.unwrap();
        assert_eq!(resp["error"]["code"], -32700);
        assert_eq!(resp["error"]["message"], "Parse error");
    }

    #[tokio::test]
    async fn handle_missing_method() {
        let srv = server();
        let resp = srv.handle_request(r#"{"jsonrpc":"2.0","id":5}"#).await.unwrap();
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn tools_list_empty() {
        let srv = McpServer::new(vec![]);
        let resp = srv
            .handle_request(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
            .await
            .unwrap();
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn tools_list_multiple() {
        let srv = McpServer::new(vec![
            ToolSchema {
                name: "a".into(),
                description: "A".into(),
                parameters: serde_json::json!({}),
            },
            ToolSchema {
                name: "b".into(),
                description: "B".into(),
                parameters: serde_json::json!({}),
            },
        ]);
        let resp = srv
            .handle_request(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
            .await
            .unwrap();
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "a");
        assert_eq!(tools[1]["name"], "b");
    }

    #[tokio::test]
    async fn executor_mode_runs_real_tools() {
        let registry = Arc::new(pr_tools::ToolRegistry::with_builtins());
        let ctx = Arc::new(pr_tools::ToolContext::new(
            std::path::PathBuf::from("/tmp"),
            pr_core::SearchConfig::default(),
        ));
        let srv = McpServer::with_executor(registry, ctx);
        assert!(srv.tools.len() > 30, "built-ins should be listed");

        // glob in an empty dir: a real, side-effect-free execution.
        let resp = srv
            .handle_request(
                r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"glob","arguments":{"pattern":"no-such-file-xyz*","path":"/tmp"}}}"#,
            )
            .await
            .unwrap();
        assert_eq!(resp["id"], 9);
        assert!(resp["result"]["content"].is_array());

        // Unknown tool => isError result, not a JSON-RPC error.
        let resp = srv
            .handle_request(
                r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"nope","arguments":{}}}"#,
            )
            .await
            .unwrap();
        assert_eq!(resp["result"]["isError"], true);
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Unknown tool"));
    }
}
