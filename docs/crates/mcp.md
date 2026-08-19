# Crate documentation: `crates/mcp`

The `mcp` crate implements a client and server for the **MCP (Model Context Protocol)** — a standard for connecting external tools to LLM agents. Supports two transports: **stdio** (running the server as a child process) and **Streamable HTTP** (POST requests to a remote endpoint with optional OAuth authentication). Includes a bridge (`bridge`) for registering remote MCP tools in the local agent tool registry.

---

## Contents

1. [lib.rs — entry point](#librs)
2. [client.rs — MCP client](#clientrs)
3. [server.rs — MCP server](#serverrs)
4. [bridge.rs — tool bridge](#bridgers)

---

## lib.rs

File [lib.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/mcp/src/lib.rs) declares three public modules and re-exports all their contents:

```rust
pub mod client;
pub mod server;
pub mod bridge;

pub use client::*;
pub use server::*;
pub use bridge::*;
```

---

## client.rs

File [client.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/mcp/src/client.rs) — the main MCP client supporting stdio and HTTP transports, OAuth authentication, tool caching, and reconnection.

### Constants

```rust
pub const TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
```

The notification name that an MCP server sends when its tool list changes. The client uses this to invalidate the cache.

### Enum `McpTransport`

```rust
pub enum McpTransport {
    Stdio { command: String, args: Vec<String> },
    Http { url: String, auth: Option<OAuthConfig> },
}
```

#### Method `from_config(config: &McpServerConfig) -> anyhow::Result<Self>`

Algorithm:
1. Reads `config.transport` (string).
2. If `"stdio"`: extracts `config.command` (required) and `config.args`, returns `McpTransport::Stdio`.
3. If `"http"`, `"streamable-http"` or `"sse"`: extracts `config.url` (required), returns `McpTransport::Http { url, auth: None }`. (OAuth is configured separately.)
4. Any other value — error `"unknown MCP transport"`.

### Struct `OAuthConfig`

```rust
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub token_url: String,
}
```

OAuth 2.0 Client Credentials Grant configuration. Serialized/deserialized (serde), can be read from TOML configuration.

### Struct `OAuthToken`

```rust
struct OAuthToken {
    access_token: String,
    expires_at: Option<std::time::Instant>,
}
```

Cached token. The `expires_at` field is the absolute expiration instant. If `None` — the token is considered eternal (the server did not return `expires_in`).

#### Method `is_expired(&self) -> bool`

Returns `true` if `Instant::now() >= expires_at`. If `expires_at == None`, returns `false` (token does not expire).

### Struct `McpClient`

```rust
pub struct McpClient {
    connections: HashMap<String, McpConnection>,
    configs: HashMap<String, McpServerConfig>,
    tool_cache: HashMap<String, Vec<ToolSchema>>,
    tools_dirty: HashSet<String>,
    next_id: u64,
}
```

| Field | Description |
|-------|-------------|
| `connections` | Active connections indexed by server name. |
| `configs` | Server configurations (for reconnection). |
| `tool_cache` | Cached tool lists per server. |
| `tools_dirty` | Set of servers that have notified of tool changes. |
| `next_id` | Counter for generating unique JSON-RPC request ids. |

### Internal types

#### `McpConnection`

```rust
enum McpConnection {
    Stdio(StdioConn),
    Http(HttpConn),
}
```

#### `StdioConn`

```rust
struct StdioConn {
    child: Child,           // child process
    stdin: ChildStdin,      // process stdin (for writing)
    stdout: BufReader<ChildStdout>, // stdout with buffering (for reading)
}
```

#### `HttpConn`

```rust
struct HttpConn {
    client: reqwest::Client,
    url: String,
    auth: Option<OAuthConfig>,
    token: Option<OAuthToken>,
    session_id: Option<String>,
}
```

`session_id` is the MCP session identifier issued by the server during initialization. It is sent in the `mcp-session-id` header of every subsequent request.

#### JSON-RPC structures

```rust
struct JsonRpcRequest {
    jsonrpc: String,                          // always "2.0"
    id: Option<u64>,                          // None for notifications
    method: String,
    params: Option<serde_json::Value>,
}

struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
    method: Option<String>,                   // for server notifications/requests
}

struct JsonRpcError {
    code: i64,
    message: String,
}
```

#### `IncomingKind` and `classify`

```rust
enum IncomingKind {
    Response,       // has id, no method → response to our request
    Notification,   // no id, has method → notification from server
    ServerRequest,  // has id and method → request from server (not supported)
}
```

Function `classify(value: &serde_json::Value) -> IncomingKind`:

1. Checks for the presence of `id` (not null).
2. Checks for the presence of `method` (not null).
3. Returns the variant based on the `(has_id, has_method)` combination:
   - `(true, false)` → `Response`
   - `(false, true)` → `Notification`
   - `(true, true)` → `ServerRequest`
   - `(false, false)` → `Response` (malformed — left to the caller's discretion)

### Function `parse_sse_data_events(body: &str) -> Vec<String>`

Algorithm for parsing SSE (Server-Sent Events) body:

1. Creates empty `events: Vec<String>` and `data_lines: Vec<&str>`.
2. Iterates over `body.lines()`:
   - **Empty line**: event separator. If `data_lines` is not empty — joins them with `\n` and adds to `events`. Clears `data_lines`.
   - **Line with prefix `data:`**: extracts content after `data:`, removes one leading space (if present), adds to `data_lines`.
   - **Any other line** (`event:`, `id:`, `retry:`, comments `:`) — ignored.
3. After the loop: if `data_lines` is not empty (last event without trailing empty line) — adds to `events`.
4. Returns `events`.

**Example**:
```
data: {"a":1}

data: line1
data: line2
```
→ `vec!["{\"a\":1}", "line1\nline2"]`

### Function `parse_token_response(body: &serde_json::Value) -> anyhow::Result<(String, Option<Duration>)>`

Algorithm for parsing OAuth endpoint response:

1. Extracts `body["access_token"]` as a string. If absent — error.
2. Extracts `body["expires_in"]` as `u64` (seconds).
3. If `expires_in` is present, computes the lifetime: `Duration::from_secs(max(expires_in - 30, 1))` — refresh 30 seconds before expiration (safety margin). If `expires_in` is absent, lifetime = `None`.
4. Returns `(access_token, lifetime)`.

### Implementation of `McpConnection`

#### Method `request(&mut self, req: &JsonRpcRequest) -> anyhow::Result<(JsonRpcResponse, Vec<String>)>`

**Stdio branch:**

1. Serializes `req` to a JSON string.
2. Writes the string + `\n` to the child process's `stdin`.
3. Calls `flush()`.
4. Remembers `expected_id = req.id`.
5. Response reading loop:
   - `tokio::time::timeout(60s, stdout.read_line(&mut buf))` — 60-second timeout for reading one line.
   - If timeout — error `"timed out waiting for MCP server response"`.
   - If 0 bytes read (EOF) — error `"MCP server closed the connection"`.
   - Parses the line as `serde_json::Value`. If parsing fails — skips (may be a log line from the server).
   - Classifies via `classify(&value)`:
     - `Notification`: extracts `method` and adds to `notifications`.
     - `ServerRequest`: logs debug and skips (not supported).
     - `Response`: deserializes to `JsonRpcResponse`. If `expected_id` is set and `resp.id != expected_id` — skips (stale response). Otherwise — returns `(resp, notifications)`.

**Http branch:**

1. Calls `c.post(req).await`.
2. Returns `(resp, Vec::new())` — notifications are not intercepted for HTTP (they arrive as separate responses).

#### Method `notify(&mut self, method: &str) -> anyhow::Result<()>`

Sends a JSON-RPC notification (without `id`):

1. Creates `JsonRpcRequest { jsonrpc: "2.0", id: None, method, params: None }`.
2. **Stdio**: serializes, writes to stdin + `\n`, flush.
3. **Http**: calls `c.post(&msg).await`, result is ignored.

### Implementation of `HttpConn`

#### Method `ensure_token(&mut self) -> anyhow::Result<Option<String>>`

Lazy OAuth token acquisition/refresh:

1. If `self.auth == None` — returns `None` (no authentication).
2. If `self.token` exists and is not expired — returns `Some(token.access_token.clone())`.
3. Otherwise — requests a new token:
   - `POST {auth.token_url}` with form data:
     - `grant_type=client_credentials`
     - `client_id={auth.client_id}`
     - `client_secret={auth.client_secret}`
   - If HTTP status is not 2xx — error with response body.
   - Parses JSON response via `parse_token_response`.
   - Creates `OAuthToken { access_token, expires_at: Some(Instant::now() + lifetime) }`.
   - Saves to `self.token`.
   - Returns `Some(access_token)`.

#### Method `post(&mut self, msg: &JsonRpcRequest) -> anyhow::Result<JsonRpcResponse>`

Sending a single JSON-RPC message over HTTP. Algorithm:

1. Calls `self.ensure_token()`.
2. Builds a POST request:
   - URL: `self.url`
   - Header `Accept: application/json, text/event-stream`
   - Body: JSON serialization of `msg` via `.json(msg)`.
   - If token exists: `Authorization: Bearer {token}`.
   - If `session_id` exists: header `mcp-session-id`.
3. Sends the request.
4. **Update session_id**: if the response has an `mcp-session-id` header, saves it to `self.session_id`.
5. **HTTP 202 Accepted**: returns an empty `JsonRpcResponse` (response to a notification).
6. **Unsuccessful status**: error with body.
7. **Content-Type check**:
   - If `text/event-stream`:
     - Reads body as a string.
     - Calls `parse_sse_data_events(&body)`.
     - Iterates over events **in reverse order** (`.rev()`).
     - Tries to deserialize each as `JsonRpcResponse`.
     - First successful — returns.
     - If none matched — error `"no JSON-RPC response found in SSE stream"`.
   - Otherwise: deserializes body as `JsonRpcResponse`.

**Note on reverse SSE order**: The JSON-RPC response is usually the last SSE event in the stream (after possible intermediate progress events), so iterating from the end optimizes the search.

### Implementation of `McpClient`

#### `new() -> Self`

Creates an empty client. All HashMaps are empty, `next_id = 0`.

#### `next_request_id(&mut self) -> u64`

Increments `self.next_id` and returns it. Each call yields a unique id.

#### `connect(&mut self, config: &McpServerConfig) -> anyhow::Result<()>`

Connect to a server with connection pooling:

1. If `config.name` already exists in `self.connections` — logs debug, updates the configuration, and returns `Ok(())` (reuse).
2. Calls `McpTransport::from_config(config)`.
3. Depending on the transport:
   - `Stdio` → `self.connect_stdio(config).await`
   - `Http` → `self.connect_http(&config.name, url, auth).await`
4. Saves the configuration in `self.configs`.

#### `connect_all(&mut self, config: &McpConfig) -> anyhow::Result<()>`

Connect to all servers from the configuration:

1. Iterates over `config.servers`.
2. For each: `self.connect(server).await`. Errors are logged, not fatal.
3. If connection successful: `self.initialize(&server.name).await`. Errors are also logged.

#### `connect_stdio(&mut self, config: &McpServerConfig) -> anyhow::Result<()>`

Step-by-step algorithm:

1. Extracts `config.command` (required for stdio).
2. Creates `Command::new(command)` with `config.args` as arguments.
3. Configures:
   - `stdin(Stdio::piped())` — intercept stdin.
   - `stdout(Stdio::piped())` — intercept stdout.
   - `stderr(Stdio::null())` — discard stderr.
   - `kill_on_drop(true)` — kill process on drop.
4. `cmd.spawn()` — launches the child process.
5. Takes `stdin` and `stdout` from `Child` (via `.take()`).
6. Creates `StdioConn { child, stdin, stdout: BufReader::new(stdout) }`.
7. Inserts `McpConnection::Stdio(conn)` into `self.connections`.
8. Logs `"Connected to MCP server: {name}"`.

**Important**: after this call the connection is established, but **initialization has not yet been performed**. You need to call `initialize()` separately.

#### `connect_http(&mut self, name, url, auth) -> anyhow::Result<()>`

Algorithm:

1. Creates `reqwest::Client::builder()` with a 60-second timeout.
2. Creates `HttpConn { client, url, auth, token: None, session_id: None }`.
3. Inserts `McpConnection::Http(conn)` into `self.connections`.
4. Logs `"Connected to MCP server (http): {name} -> {url}"`.

Similar to stdio — initialization is not performed automatically.

#### `initialize(&mut self, server_name: &str) -> anyhow::Result<()>`

MCP initialization protocol. Algorithm:

1. Generates `id = self.next_request_id()`.
2. Builds a JSON-RPC request:
   ```json
   {
     "jsonrpc": "2.0",
     "id": <id>,
     "method": "initialize",
     "params": {
       "protocolVersion": "2024-11-05",
       "capabilities": {},
       "clientInfo": {
         "name": "parallel-research",
         "version": "0.1.0"
       }
     }
   }
   ```
3. Gets a mutable reference to the connection from `self.connections`.
4. Calls `conn.request(&request).await` — receives `(response, notifications)`.
5. Checks `response.error` — if present, error with message.
6. Sends notification `conn.notify("notifications/initialized").await` — this completes the handshake.
7. Calls `self.mark_tools_dirty(server_name, &notifications)` — if the server sent a tool change notification during initialization, marks the cache as stale.

**MCP handshake protocol**:
1. Client → server: `initialize` (request with client capabilities).
2. Server → client: response with server capabilities.
3. Client → server: `notifications/initialized` (ready notification).

#### `list_tools(&mut self, server_name: &str) -> anyhow::Result<Vec<ToolSchema>>`

Retrieving the tool list. Algorithm:

1. Builds a `tools/list` request with a unique `id`.
2. Sends via `conn.request(&request)`.
3. Checks `response.error`.
4. Extracts `response.result["tools"]`, deserializes into `Vec<McpToolDef>`.
5. Maps each `McpToolDef` to `ToolSchema`:
   - `name` → `name`
   - `description` → `description` (or empty string if `None`)
   - `input_schema` → `parameters` (or `{"type": "object"}` if `None`)
6. Saves in `self.tool_cache[server_name]` and removes from `tools_dirty`.
7. Returns `schemas`.

#### `discover_tools(&mut self, server_name: &str) -> anyhow::Result<Vec<ToolSchema>>`

Caching wrapper around `list_tools`. Algorithm:

1. If `server_name` is **not** in `tools_dirty` **and** cache exists → returns cache.
2. Otherwise calls `self.list_tools(server_name).await`.

This avoids unnecessary round-trips to the server if tools have not changed.

#### `cached_tools(&self, server_name: &str) -> Option<&[ToolSchema]>`

Returns a reference to the cached tool list (or `None`).

#### `call_tool(&mut self, server_name, tool_name, args) -> anyhow::Result<serde_json::Value>`

Tool invocation. Algorithm:

1. Builds a `tools/call` request:
   ```json
   {
     "jsonrpc": "2.0",
     "id": <id>,
     "method": "tools/call",
     "params": {
       "name": "<tool_name>",
       "arguments": <args>
     }
   }
   ```
2. Sends via `conn.request(&request)`.
3. Checks notifications via `mark_tools_dirty` (tool may have changed during the call).
4. Checks `response.error`.
5. Returns `response.result` (or `{}` if result = None).

#### `reconnect(&mut self, server_name: &str) -> anyhow::Result<()>`

Reconnection. Algorithm:

1. Retrieves the configuration from `self.configs[server_name]`. If not found — error.
2. Calls `self.disconnect(server_name).await` — kills the old connection.
3. Clears `tool_cache` and `tools_dirty` for this server.
4. Calls `self.connect(&config).await` — new connection.
5. Calls `self.initialize(server_name).await` — full initialization.
6. Logs `"Reconnected MCP server: {server_name}"`.

#### `disconnect(&mut self, server_name: &str)`

Disconnect a single server:

1. Removes the connection from `self.connections`.
2. If it is `Stdio` — calls `c.child.kill().await` (kills the child process).

#### `is_connected(&self, server_name: &str) -> bool`

Checks for the presence of `server_name` in `self.connections`.

#### `connected_servers(&self) -> Vec<String>`

Returns a list of names of all connected servers.

#### `shutdown(&mut self)`

Disconnects all servers:

1. Collects all keys from `self.connections`.
2. For each, calls `self.disconnect(&name).await`.

#### `mark_tools_dirty(&mut self, server_name, notifications)`

Algorithm:
1. Checks whether `TOOLS_LIST_CHANGED` is contained in the `notifications` array.
2. If yes — adds `server_name` to `self.tools_dirty`.

---

## server.rs

File [server.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/mcp/src/server.rs) implements a simple MCP server running over stdio.

### Struct `McpServer`

```rust
pub struct McpServer {
    tools: Vec<ToolSchema>,
}
```

Stores the list of tools that the server can provide.

#### Constructor `new(tools: Vec<ToolSchema>)`

Saves the provided tool list.

### Method `run_stdio(&self) -> anyhow::Result<()>`

Main MCP server loop. Algorithm:

1. Gets handles to `tokio::io::stdin()` and `tokio::io::stdout()`.
2. Wraps stdin in `BufReader::new(stdin)` for line-by-line reading.
3. Infinite loop:
   - `reader.read_line(&mut line).await?` — reads one line.
   - If 0 bytes read (EOF) — `break` (server terminates).
   - Trims the line (`trim()`). If empty — `continue`.
   - Calls `self.handle_request(line).await` → receives a JSON response.
   - Serializes the response to a string.
   - Writes the string + `\n` to stdout.
   - Calls `flush()`.

### Method `handle_request(&self, request: &str) -> serde_json::Value`

Processing a single JSON-RPC request. Algorithm:

1. **Parsing**: `serde_json::from_str(request)` into `serde_json::Value`. If parsing fails — returns:
   ```json
   {"jsonrpc": "2.0", "error": {"code": -32700, "message": "Parse error"}}
   ```
   (Code -32700 is the standard JSON-RPC parse error code.)

2. **Field extraction**: `id = req["id"]`, `method = req["method"]` (or empty string).

3. **Routing by method**:

   - **`"initialize"`**: returns:
     ```json
     {
       "jsonrpc": "2.0",
       "id": <id>,
       "result": {
         "protocolVersion": "2024-11-05",
         "capabilities": {"tools": {}},
         "serverInfo": {
           "name": "parallel-research",
           "version": "0.1.0"
         }
       }
     }
     ```
     The capability `{"tools": {}}` means the server supports tools (without additional options).

   - **`"tools/list"`**: maps each `ToolSchema` to:
     ```json
     {
       "name": "<tool.name>",
       "description": "<tool.description>",
       "inputSchema": "<tool.parameters>"
     }
     ```
     and returns the array in `result.tools`.

   - **`"tools/call"`**: returns a stub:
     ```json
     {
       "jsonrpc": "2.0",
       "id": <id>,
       "result": {
         "content": [{"type": "text", "text": "Tool execution delegated"}]
       }
     }
     ```
     This server **does not execute tools itself** — it only provides metadata. Actual execution is delegated to the caller.

   - **Any other method**: returns an error:
     ```json
     {"jsonrpc": "2.0", "id": <id>, "error": {"code": -32601, "message": "Method not found"}}
     ```
     (Code -32601 is the standard JSON-RPC "method not found" code.)

**Note**: if `id` is absent in the request (notification), `id` in the response will be `null`. For notifications, a response is usually not needed, but the server still generates one — this is not a problem because the client simply will not read it.

---

## bridge.rs

File [bridge.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/mcp/src/bridge.rs) implements a bridge between MCP servers and the local agent tool registry. Remote MCP tools are wrapped in `McpBridgeTool`, which implements the `Tool` trait from `pr_tools`.

### Constants

```rust
const NAME_SEP: &str = "__";
```

Separator for bridge tool name parts. Format: `mcp__{server}__{tool}`.

### Struct `McpBridgeTool`

```rust
pub struct McpBridgeTool {
    server: String,                    // MCP server name
    tool_name: String,                 // original tool name on the server
    bridged: String,                   // full bridge name: mcp__server__tool
    description: String,               // tool description
    parameters: serde_json::Value,     // JSON Schema of parameters
    client: Arc<Mutex<McpClient>>,     // shared MCP client (shared between tools)
}
```

#### Static method `bridged_name(server: &str, tool: &str) -> String`

Forms the name: `"mcp__{server}__{tool}"`.

**Example**: `bridged_name("web-search", "query")` → `"mcp__web-search__query"`.

#### Constructor `new(server, schema: ToolSchema, client) -> Self`

1. Computes `bridged = Self::bridged_name(&server, &schema.name)`.
2. Fills fields from `schema` and `server`.

### Implementation of the `Tool` trait for `McpBridgeTool`

#### `name(&self) -> &str`

Returns `self.bridged` — the full name with the `mcp__` prefix.

#### `description(&self) -> &str`

Returns `self.description`.

#### `schema(&self) -> ToolSchema`

Returns `ToolSchema` with:
- `name` = full bridge name
- `description` = `"[MCP {server}] {original_description}"` — adds a prefix for identifying MCP tools in UI/logs.
- `parameters` = clone of `self.parameters`

#### `execute(&self, args, _ctx) -> anyhow::Result<ToolOutput>`

Main execution method. Algorithm:

**Step 1 — MCP tool invocation:**

1. Acquires the lock on `self.client` (`self.client.lock().await`).
2. Calls `client.call_tool(&self.server, &self.tool_name, args.clone()).await`.
3. Error handling:
   - **Success**: `Ok(value)` → proceed to step 2.
   - **Error with text "closed the connection"**: auto-reconnect.
     1. Logs warning: `"MCP server {server} went away; attempting reconnect"`.
     2. Calls `client.reconnect(&self.server).await`.
     3. If reconnect succeeds: retries `client.call_tool(...)` with the original `args`.
     4. If reconnect fails: returns an error combining both errors.
   - **Any other error**: returns `Err(e)`.

**Step 2 — Result conversion:**

- `Ok(value)` → `Ok(mcp_result_to_output(&value))`
- `Err(e)` → `Ok(ToolOutput::err("MCP tool {server}.{tool} failed: {e}"))`

**Important**: MCP tool errors are NOT returned as `Err` — they are wrapped in `ToolOutput::err(...)` (Ok with success=false). This allows the agent to see the error as a tool result rather than a system failure.

### Function `mcp_result_to_output(value: &serde_json::Value) -> ToolOutput`

Converting a `tools/call` result to `ToolOutput`. Algorithm:

1. Extracts `value["content"]` as an array.
2. If the array exists:
   - Filters elements: only those with `type == "text"` and that have a `text` field.
   - Extracts the text of each matching element into `Vec<String>`.
   - If the vector is not empty:
     - Checks `value["isError"]` (boolean). If absent — `false`.
     - Joins texts with `\n`.
     - If `isError == true` → `ToolOutput::err(joined)`.
     - If `isError == false` → `ToolOutput::ok(joined)`.
     - Returns the result.
3. **Fallback**: if `content` is absent, empty, or contains no text elements — `ToolOutput::ok(serde_json::to_string_pretty(value))` — pretty-printed JSON of the entire object.

### Function `connect_and_register(registry, config) -> Option<Arc<Mutex<McpClient>>>`

Main entry point for integrating MCP with the tool registry. Algorithm:

1. If `config.servers` is empty — returns `None` (no servers).
2. Creates `McpClient::new()`.
3. Calls `client.connect_all(config).await`. Errors are logged, not fatal.
4. Gets the list of `client.connected_servers()`.
5. If no server connected — logs a warning, returns `None`.
6. Wraps the client in `Arc<Mutex<McpClient>>`.
7. For each connected server:
   - Acquires the lock: `client.lock().await`.
   - Calls `c.discover_tools(&server).await`. On error — logs and `continue`.
   - For each `ToolSchema` from the result:
     - Creates `McpBridgeTool::new(server, schema, client.clone())`.
     - Registers it in the `registry` via `registry.register(Arc::new(bridge_tool))`.
     - Increments the `registered` counter.
8. Logs: `"registered {registered} MCP tool(s) from {server_count} server(s)"`.
9. Returns `Some(client)` — the shared reference to the client is needed to keep connections alive and for future reconnections.

**Note**: `Arc<Mutex<McpClient>>` is shared across all `McpBridgeTool` instances. When an agent calls any MCP tool, it acquires the mutex, which means **sequential** execution of MCP calls (no parallelism). This is a characteristic of the current implementation, driven by the fact that `McpClient` does not support parallel requests to the same server (especially for the stdio transport, where stdin/stdout is a single stream).