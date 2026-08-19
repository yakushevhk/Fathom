//! Bridge that exposes tools of connected MCP servers through the
//! [`pr_tools::ToolRegistry`], so agents can call them like built-in tools.
//!
//! Tool names are namespaced as `mcp__{server}__{tool}` to avoid collisions
//! with built-in tools and between servers.

use async_trait::async_trait;
use pr_core::{McpConfig, ToolOutput, ToolSchema};
use pr_tools::{Tool, ToolContext, ToolRegistry};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::client::McpClient;

/// Separator between the `mcp`, server and tool parts of a bridged name.
const NAME_SEP: &str = "__";

/// A single MCP tool exposed through the registry.
pub struct McpBridgeTool {
    server: String,
    tool_name: String,
    bridged: String,
    description: String,
    parameters: serde_json::Value,
    client: Arc<Mutex<McpClient>>,
}

impl McpBridgeTool {
    pub fn bridged_name(server: &str, tool: &str) -> String {
        format!("mcp{NAME_SEP}{server}{NAME_SEP}{tool}")
    }

    fn new(server: String, schema: ToolSchema, client: Arc<Mutex<McpClient>>) -> Self {
        let bridged = Self::bridged_name(&server, &schema.name);
        Self {
            server,
            tool_name: schema.name,
            bridged,
            description: schema.description,
            parameters: schema.parameters,
            client,
        }
    }
}

#[async_trait]
impl Tool for McpBridgeTool {
    fn name(&self) -> &str {
        &self.bridged
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: format!("[MCP {}] {}", self.server, self.description),
            parameters: self.parameters.clone(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let result = {
            let mut client = self.client.lock().await;
            match client.call_tool(&self.server, &self.tool_name, args.clone()).await {
                Ok(v) => Ok(v),
                // A dead stdio server fails every subsequent call; try to
                // re-establish the connection once before giving up.
                Err(e) if e.to_string().contains("closed the connection") => {
                    tracing::warn!(
                        "MCP server {} went away; attempting reconnect",
                        self.server
                    );
                    match client.reconnect(&self.server).await {
                        Ok(()) => client.call_tool(&self.server, &self.tool_name, args).await,
                        Err(re) => Err(anyhow::anyhow!("{e} (reconnect failed: {re})")),
                    }
                }
                Err(e) => Err(e),
            }
        };

        match result {
            Ok(value) => Ok(mcp_result_to_output(&value)),
            Err(e) => Ok(ToolOutput::err(format!(
                "MCP tool {}.{} failed: {e}",
                self.server, self.tool_name
            ))),
        }
    }
}

/// Convert an MCP `tools/call` result into a [`ToolOutput`].
///
/// Handles the standard shape `{content: [{type:"text", text}], isError}`,
/// falling back to pretty-printed JSON for anything else.
fn mcp_result_to_output(value: &serde_json::Value) -> ToolOutput {
    let content = value.get("content").and_then(|c| c.as_array());
    if let Some(items) = content {
        let text: Vec<String> = items
            .iter()
            .filter_map(|item| {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    item.get("text").and_then(|t| t.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect();
        if !text.is_empty() {
            let is_error = value
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);
            let joined = text.join("\n");
            return if is_error {
                ToolOutput::err(joined)
            } else {
                ToolOutput::ok(joined)
            };
        }
    }
    ToolOutput::ok(serde_json::to_string_pretty(value).unwrap_or_default())
}

/// Connect to all configured MCP servers and register their tools in the
/// registry. Returns the shared client (kept alive for tool calls) or `None`
/// when no servers are configured.
///
/// Best-effort: unreachable servers are skipped with a warning.
pub async fn connect_and_register(
    registry: &mut ToolRegistry,
    config: &McpConfig,
) -> Option<Arc<Mutex<McpClient>>> {
    if config.servers.is_empty() {
        return None;
    }

    let mut client = McpClient::new();
    if let Err(e) = client.connect_all(config).await {
        tracing::warn!("MCP connect_all reported errors: {e}");
    }

    let servers = client.connected_servers();
    if servers.is_empty() {
        tracing::warn!("no MCP servers could be connected");
        return None;
    }

    let client = Arc::new(Mutex::new(client));
    let mut registered = 0usize;
    let server_count = servers.len();

    for server in servers {
        let tools = {
            let mut c = client.lock().await;
            match c.discover_tools(&server).await {
                Ok(tools) => tools,
                Err(e) => {
                    tracing::warn!("MCP tool discovery failed for {server}: {e}");
                    continue;
                }
            }
        };
        for schema in tools {
            registry.register(Arc::new(McpBridgeTool::new(
                server.clone(),
                schema,
                client.clone(),
            )));
            registered += 1;
        }
    }

    tracing::info!("registered {registered} MCP tool(s) from {server_count} server(s)");
    Some(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridged_name() {
        assert_eq!(
            McpBridgeTool::bridged_name("web-search", "query"),
            "mcp__web-search__query"
        );
    }

    #[test]
    fn test_mcp_result_to_output_text_content() {
        let value = serde_json::json!({
            "content": [
                {"type": "text", "text": "hello"},
                {"type": "text", "text": "world"}
            ]
        });
        let out = mcp_result_to_output(&value);
        assert!(out.success);
        assert_eq!(out.content, "hello\nworld");
    }

    #[test]
    fn test_mcp_result_to_output_error_flag() {
        let value = serde_json::json!({
            "content": [{"type": "text", "text": "boom"}],
            "isError": true
        });
        let out = mcp_result_to_output(&value);
        assert!(!out.success);
        assert_eq!(out.content, "boom");
    }

    #[test]
    fn test_mcp_result_to_output_fallback_json() {
        let value = serde_json::json!({"answer": 42});
        let out = mcp_result_to_output(&value);
        assert!(out.success);
        assert!(out.content.contains("42"));
    }

    #[tokio::test]
    async fn test_connect_and_register_empty_config() {
        let mut registry = ToolRegistry::new();
        let config = McpConfig::default();
        assert!(connect_and_register(&mut registry, &config).await.is_none());
        assert!(registry.tool_names().is_empty());
    }
}
