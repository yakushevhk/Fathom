---
name: fathom-mcp
description: Model Context Protocol (MCP) client, server, and bridge architecture in crates/mcp. Use when integrating external tools via MCP, exposing Fathom tools via mcp-serve, or extending stdio/HTTP transports.
---

# Model Context Protocol (MCP) Architecture (`crates/mcp`)

Fathom implements full bi-directional support for the Model Context Protocol (MCP), enabling external tool discovery and tool exposure.

## 1. Subsystem Structure

```
crates/mcp/src/
├── client.rs      # McpClient: Stdio (child process) & Streamable HTTP transports
├── server.rs      # McpServer: Exposes Fathom ToolRegistry via stdio JSON-RPC loop
└── bridge.rs      # McpBridgeTool: Wraps remote MCP tools into local Tool trait implementations
```

## 2. Invariants & Usage

* **CLI Command**:
  * `fathom mcp-serve`: Launches the MCP server exposing all **63 built-in tools** (and active extensions) over stdio.
* **Auto-Discovery & Dynamic Registration**:
  * External servers declared in `config.toml` (or via environment) are automatically connected on boot.
  * When an external server sends `notifications/tools/list_changed`, Fathom invalidates its tool schema cache and hot-reloads available tools without restarting the agent session.
* **Streamable HTTP & OAuth2**:
  * Streamable HTTP connections support SSE streaming and lazy token refresh (30s before expiry) via client-credentials grant.
