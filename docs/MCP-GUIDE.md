# MCP Integration Guide

**Model Context Protocol** (MCP) is a JSON-RPC 2.0–based standard for connecting external tools to LLM agents. Fathom implements both sides of the protocol:

- **MCP client** — connects to external MCP servers (stdio or HTTP) and makes their tools available to agents.
- **MCP server** — exposes Fathom's own built-in tools to external MCP clients such as Claude Desktop, ZCode, or Cursor.

---

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Configuring MCP Servers](#configuring-mcp-servers)
  - [Stdio Transport](#stdio-transport)
  - [HTTP / Streamable HTTP Transport](#http--streamable-http-transport)
  - [OAuth 2.0 Authentication](#oauth-20-authentication)
- [How Tools from MCP Servers Become Available to Agents](#how-tools-from-mcp-servers-become-available-to-agents)
- [Running `fathom mcp-serve`](#running-fathom-mcp-serve)
- [How External MCP Clients Can Use Fathom's Tools](#how-external-mcp-clients-can-use-fathoms-tools)
- [SSE Protocol Handling](#sse-protocol-handling)
- [Error Handling](#error-handling)
- [Best Practices](#best-practices)

---

## Architecture Overview

```
                    ┌───────────────┐
                    │  Fathom Agent │
                    │  (ToolRegistry)│
                    └───┬───┬───┬───┘
                        │   │   │
          ┌─────────────┘   │   └──────────────┐
          ▼                 ▼                  ▼
   ┌────────────┐   ┌────────────┐   ┌──────────────────┐
   │ stdio MCP  │   │ http MCP  │   │ External MCP     │
   │ server     │   │ server    │   │ client (Claude,  │
   │ (subproc)  │   │ (remote)  │   │ ZCode, Cursor)   │
   └────────────┘   └────────────┘   └────────┬─────────┘
                                              │
                                       ┌──────┴──────┐
                                       │ fathom mcp- │
                                       │ serve       │
                                       └─────────────┘
```

The system has three layers:

1. **`McpClient`** (`crates/mcp/src/client.rs`) — manages multiple named connections, each with `Stdio` or `Http` transport. Handles JSON-RPC framing, MCP initialization handshake, tool caching with dynamic invalidation, OAuth token refresh, and reconnection.

2. **`McpBridgeTool`** (`crates/mcp/src/bridge.rs`) — wraps tools discovered from remote MCP servers as `Tool` trait implementations. Each tool is namespaced as `mcp__{server}__{name}` to avoid collisions with built-in tools.

3. **`McpServer`** (`crates/mcp/src/server.rs`) — exposes Fathom's own tools externally over stdio. Supports two modes: schema-only (returns metadata, stubs execution) and executor mode (actually runs tools through the shared `ToolRegistry`).

---

## Configuring MCP Servers

MCP servers are configured in the `[[mcp.servers]]` array of `~/.fathom/config.toml`. Each entry specifies a name, transport type, and connection parameters.

```toml
# ─────────────────────────────────────────────
# MCP Servers
# ─────────────────────────────────────────────
[[mcp.servers]]
name = "web-search"
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-web-search"]

[[mcp.servers]]
name = "remote-tools"
transport = "http"
url = "https://mcp.example.com"
```

| Field       | Type     | Required | Description                                           |
|-------------|----------|----------|-------------------------------------------------------|
| `name`      | string   | yes      | Logical server name (used in tool namespacing)        |
| `transport` | string   | yes      | `"stdio"`, `"http"`, `"streamable-http"`, or `"sse"` |
| `command`   | string   | stdio    | Executable path or binary name                        |
| `args`      | string[] | no       | Command-line arguments                                |
| `url`       | string   | http     | Server endpoint URL                                   |

### Stdio Transport

The stdio transport spawns the server as a child process and communicates over its stdin/stdout with newline-delimited JSON-RPC messages. The child process's stderr is discarded (it must not interfere with the protocol).

Fathom passes `kill_on_drop(true)` to the child process — when the connection is dropped or Fathom shuts down, the server process is killed automatically.

**Example — local Node.js MCP server:**
```toml
[[mcp.servers]]
name = "filesystem"
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/allowed/path"]
```

**Example — local Python MCP server:**
```toml
[[mcp.servers]]
name = "sqlite"
transport = "stdio"
command = "uv"
args = ["run", "mcp-server-sqlite", "--db-path", "/tmp/test.db"]
```

### HTTP / Streamable HTTP Transport

The HTTP transport sends JSON-RPC messages as POST requests to a remote endpoint. The client accepts both `"http"`, `"streamable-http"`, and `"sse"` as transport identifiers — they all map to the same `Http` transport.

The `Accept` header is set to `application/json, text/event-stream`, so the server can respond with either plain JSON or Server-Sent Events. During the initialization handshake, the server may set an `mcp-session-id` header; the client stores it and sends it back on every subsequent request.

**Example:**
```toml
[[mcp.servers]]
name = "remote-tools"
transport = "http"
url = "https://mcp.example.com/api"
```

### OAuth 2.0 Authentication

HTTP MCP servers can authenticate via the OAuth 2.0 Client Credentials Grant. When the server returns `401 Unauthorized`, the client obtains an access token from the configured token endpoint and sends it as a `Bearer` token in the `Authorization` header.

Currently, OAuth config is not wired into the `[[mcp.servers]]` config section directly. The `OAuthConfig` struct is available in the codebase and can be set programmatically on the `McpTransport::Http` variant:

```rust
McpTransport::Http {
    url: "https://mcp.example.com".to_string(),
    auth: Some(OAuthConfig {
        client_id: "my-client-id".to_string(),
        client_secret: "my-client-secret".to_string(),
        token_url: "https://auth.example.com/token".to_string(),
    }),
}
```

The token is cached and refreshed 30 seconds before it expires (based on the `expires_in` field in the OAuth response). If no `expires_in` is provided, the token is considered permanent.

---

## How Tools from MCP Servers Become Available to Agents

The entry point is `connect_and_register()` in `crates/mcp/src/bridge.rs`, called during Fathom startup:

```rust
let mut registry = ToolRegistry::with_builtins();
if !config.mcp.servers.is_empty() {
    let _client = pr_mcp::connect_and_register(&mut registry, &config.mcp).await;
}
```

The process:

1. **Connect** — `McpClient::connect_all()` iterates over every configured server, creates a connection (stdio child process or HTTP client), and runs the MCP initialization handshake (`initialize` → response → `notifications/initialized`). Failed servers log a warning but do not block startup.

2. **Discover tools** — For each connected server, `McpClient::discover_tools()` sends a `tools/list` JSON-RPC request. The returned tool schemas (name, description, input schema) are cached per server.

3. **Register bridge tools** — Each tool schema is wrapped in an `McpBridgeTool` and registered in the global `ToolRegistry` under a namespaced name:

   ```
   mcp__{server_name}__{tool_name}
   ```

   For example, a tool named `search` from server `web-search` becomes `mcp__web-search__search`.

4. **Tool execution** — When an agent calls `mcp__web-search__search`, the `McpBridgeTool::execute()` method locks the shared `McpClient`, sends a `tools/call` JSON-RPC request to the appropriate server, and converts the result into a `ToolOutput`.

5. **Dynamic tool discovery** — If the server sends a `notifications/tools/list_changed` notification (which is intercepted during any request), the tool cache for that server is marked as dirty. The next `discover_tools()` call fetches a fresh list.

### Tool Namespacing

The namespacing convention (`mcp__{server}__{tool}`) ensures:

- No collisions between MCP tools and built-in tools (e.g., `glob` vs `mcp__filesystem__glob`).
- No collisions between tools from different MCP servers with the same name.
- Clear provenance — the server name is visible in the tool description (`[MCP {server}] {description}`).

### Auto-Reconnection

When a tool call fails because the MCP server closed the connection (stdio process died), `McpBridgeTool` attempts a single reconnection:

```rust
Err(e) if e.to_string().contains("closed the connection") => {
    // reconnect and retry once
}
```

If reconnection fails, the tool returns an error — the agent sees a recoverable tool failure, not a crash.

---

## Running `fathom mcp-serve`

The `mcp-serve` subcommand runs Fathom as an MCP server over stdio, exposing all built-in tools to external MCP clients.

```bash
fathom mcp-serve
```

**What it does:**

1. Loads configuration from `~/.fathom/config.toml`.
2. Creates a `ToolRegistry` with all built-in tools registered.
3. Sets up the `ToolContext` (current directory, search config, LLM provider, contact database, CRM, memory store).
4. Creates an `McpServer` in **executor mode** — `tools/call` requests actually invoke tools through the registry.
5. Enters the stdio read loop: reads JSON-RPC requests from stdin, dispatches them, writes responses to stdout.

**Important:**

- **All logging goes to stderr.** The stdio protocol owns stdout — any output to stdout corrupts the JSON-RPC stream.
- **External MCP servers configured in `[[mcp.servers]]` are NOT re-exported.** This prevents infinite loops (an MCP server that wraps another MCP server).
- The server supports `initialize`, `ping`, `tools/list`, and `tools/call` methods. Unknown methods return a JSON-RPC error (`-32601: Method not found`).
- Notifications (requests without an `id`) receive no response — they are silently consumed.

### MCP Protocol Initialization Handshake

```
Client                         Server
  │                              │
  ├─ initialize ────────────────►│
  │                              ├─ {protocolVersion, capabilities, serverInfo}
  │◄─────────────────────────────┤
  ├─ notifications/initialized ─►│  (fire-and-forget, no response)
  │                              │
  ├─ tools/list ────────────────►│
  │◄──── {tools: [...]} ─────────┤
  │                              │
  ├─ tools/call {name,args} ────►│
  │◄──── {content, isError} ─────┤
```

---

## How External MCP Clients Can Use Fathom's Tools

Any MCP client (Claude Desktop, ZCode, Cursor, VS Code extensions, custom agents) can connect to Fathom's MCP server by launching Fathom as a subprocess.

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "fathom": {
      "command": "fathom",
      "args": ["mcp-serve"]
    }
  }
}
```

### ZCode / Cursor

Add a custom MCP server:

```json
{
  "command": "fathom",
  "args": ["mcp-serve"]
}
```

### Programmatic Client

Any JSON-RPC client can connect via stdio:

```bash
# Start Fathom MCP server
fathom mcp-serve &

# Send a tools/list request
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | fathom mcp-serve

# Or use a script
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"my-client","version":"1.0.0"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"glob","arguments":{"pattern":"*.md","path":"."}}}'
} | fathom mcp-serve
```

### What Tools Are Exposed

All built-in tools are available (51 unconditional built-ins, plus optional browser/computer tools): `file_read`, `file_write`, `file_edit`, `glob`, `grep`, `shell`, `python_exec`, `web_search`, `spawn_agent`, `hub`, and many more. The exact list depends on the build configuration and environment — inspect the `tools/list` response from `fathom mcp-serve` for the authoritative set.

---

## SSE Protocol Handling

When an HTTP MCP server responds with `Content-Type: text/event-stream`, the client parses the SSE stream to extract JSON-RPC responses.

SSE parsing in `parse_sse_data_events()`:

1. Lines with the `data:` prefix are accumulated.
2. Empty lines trigger an event — accumulated `data:` lines are joined with `\n` and emitted.
3. `event:`, `id:`, `retry:`, and comment lines (`: ...`) are ignored.
4. Unterminated events (no trailing blank line) are also returned.

The HTTP transport iterates SSE events in **reverse order** to find the JSON-RPC response efficiently — the last event in the stream is most likely the response (preceding events may be progress notifications).

**Example SSE stream:**
```
event: progress
data: {"progress": 0.5}

event: progress
data: {"progress": 1.0}

data: {"jsonrpc":"2.0","id":1,"result":{"tools":[...]}}
```

The client builds the response from the last `data:` event.

---

## Error Handling

### Client-Side Errors

| Scenario | Behavior |
|----------|----------|
| Server process fails to spawn | Warning logged, startup continues |
| Stdio server closes connection | `McpBridgeTool` attempts one reconnect; on failure, returns error to agent |
| HTTP server returns non-200 | Error message includes HTTP status and body |
| OAuth token request fails | Bail with HTTP status and response body |
| JSON-RPC error response | Returned to the bridge tool, which converts to `ToolOutput::err()` |
| 60-second timeout on stdio read | `"timed out waiting for MCP server response"` — returned as tool error |
| EOF on stdio | `"MCP server closed the connection"` — triggers reconnect attempt |

### Server-Side Errors

| Scenario | Behavior |
|----------|----------|
| Unknown tool name | `isError: true` with `"Unknown tool: {name}"` in content (not a JSON-RPC error) |
| Tool execution failure | `isError: true` with the error message in content |
| Invalid JSON on stdin | JSON-RPC parse error (`-32700`) |
| Unknown method | JSON-RPC method-not-found error (`-32601`) |
| Notification (no `id`) | Silently ignored — no response sent |

### Bridge Error Handling

The `McpBridgeTool::execute()` method returns `ToolOutput` on success **and** on failure — it never panics or causes an agent crash. An MCP tool failure is always a recoverable tool output:

```rust
// Tool succeeded
Ok(ToolOutput::ok("result text"))

// Tool failed (MCP server error, network error, etc.)
Ok(ToolOutput::err("MCP tool server.tool failed: ..."))
```

The `mcp_result_to_output()` function maps the MCP response format:

```json
{
  "content": [{"type": "text", "text": "..."}],
  "isError": false
}
```

If `isError` is `true`, the text is wrapped in `ToolOutput::err()`. If the response doesn't match the expected shape, the entire JSON is pretty-printed as a fallback.

---

## Best Practices

### Configuration

- **Use descriptive server names.** The name becomes part of the tool namespace (`mcp__{name}__{tool}`), so choose something short but meaningful (`web-search`, `db-query`, `filesystem`).
- **Start with one server.** Test connectivity before adding more. Check logs for `"Connected to MCP server: {name}"` and `"registered N MCP tool(s) from M server(s)"`.
- **Keep resource-heavy servers local.** Use stdio for servers that access local files, databases, or require low latency. Use HTTP for remote or shared services.

### Performance

- **Tool caching.** Once fetched, tool lists are cached per server. The cache is only invalidated when the server sends a `notifications/tools/list_changed` notification. This avoids unnecessary round-trips.
- **Serialized tool calls.** The `McpClient` is wrapped in `Arc<Mutex<McpClient>>`, so all tool calls through the bridge are serialized. This is a deliberate trade-off — stdio transports use a single stdin/stdout stream per process, so concurrent calls would interleave and corrupt the protocol.
- **Connection pooling.** Calling `McpClient::connect()` with the same server name reuses the existing connection instead of creating a new one.

### Reliability

- **Best-effort startup.** If an MCP server is unreachable at startup, a warning is logged and the server is skipped. The agent starts with whatever tools are available.
- **Reconnection for stdio.** If a stdio server process dies, the bridge automatically reconnects once on the next tool call. For HTTP servers, reconnection is handled by the transport layer — the client simply sends a new request.
- **Graceful shutdown.** `McpClient::shutdown()` disconnects all servers and kills child processes. This is called during Fathom shutdown.

### Security

- **Stdio server isolation.** Stderr is discarded, preventing server log output from corrupting the JSON-RPC stream. The child process is killed on drop.
- **HTTP server timeout.** The HTTP client has a 60-second timeout. All requests are sent over HTTPS in production.
- **OAuth token safety.** The 30-second safety margin on token expiry prevents mid-request authentication failures. Tokens are cached in memory only, never persisted to disk.
- **No loop re-export.** `fathom mcp-serve` does not re-export tools from externally configured MCP servers, preventing infinite recursion.

### Development

- **Test with `fathom mcp-serve`.** Use any MCP client or a simple script to verify your setup:
  ```bash
  # Quick test
  echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | \
    timeout 2 fathom mcp-serve 2>/dev/null | head -5
  ```
- **Watch logs.** All MCP activity is logged at the `info` and `debug` levels via `tracing`. Run with `RUST_LOG=debug` to see JSON-RPC messages.
- **No stdout pollution.** Never print to stdout in code that runs inside `mcp-serve` — the protocol owns stdout. Use `tracing` or `eprintln!` for debugging.
- **Extend with new skills.** MCP servers are a great way to add custom tool sets without modifying Fathom's core. Build a lightweight MCP server in your language of choice, configure it in `[[mcp.servers]]`, and it appears in the agent's tool registry automatically.

---

## Reference: MCP Methods

### Client → Server

| Method | Description |
|--------|-------------|
| `initialize` | Begin the MCP handshake. Sends protocol version, client capabilities, and client info. |
| `notifications/initialized` | Signals the client is ready to receive requests. Fire-and-forget (no `id`). |
| `tools/list` | List available tools with their schemas. |
| `tools/call` | Execute a tool by name with arguments. |
| `ping` | Health check. Returns `{}`. |

### Server → Client

| Method | Description |
|--------|-------------|
| `notifications/tools/list_changed` | Signals that the tool list has changed. The client invalidates its cache. |

### JSON-RPC Error Codes

| Code | Meaning |
|------|---------|
| `-32700` | Parse error — invalid JSON |
| `-32601` | Method not found |
| Generic | Custom errors from the server (in `error.message`) |