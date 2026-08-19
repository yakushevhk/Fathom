# `crates/mcp` Crate Documentation

The `mcp` crate implements a client and server for the **MCP (Model Context Protocol)** — a JSON-RPC-based standard for connecting external tools to LLM agents. Developed by Anthropic, MCP defines a lifecycle where a client and server negotiate capabilities during initialization, then exchange JSON-RPC request/response messages for tool discovery, tool invocation, and real-time notifications.

This crate supports two transports: **stdio** (running the server as a child process and communicating over stdin/stdout with newline-delimited JSON-RPC) and **Streamable HTTP** (POST requests to a remote endpoint with optional OAuth 2.0 Client Credentials authentication, where responses may be plain JSON or Server-Sent Events). It also includes a bridge (`bridge`) for registering remote MCP tools in the agent's local tool registry, making them callable as if they were built-in tools.

The architecture follows a layered design:
- **`McpClient`** manages multiple connections (`McpConnection`) indexed by server name, each supporting either `Stdio` or `Http` transport. The client handles JSON-RPC framing, notification interception, tool caching with dynamic invalidation, and reconnection.
- **`McpServer`** exposes the agent's own tools to external MCP clients (Claude, ZCode, etc.) over stdio. It supports two modes: schema-only mode (returns tool metadata but stubs execution) and executor mode (actually runs tools through the shared `ToolRegistry`).
- **`McpBridgeTool`** wraps remote MCP tools as `Tool` trait implementations, namespacing them as `mcp__{server}__{tool}` to avoid collisions with built-in tools.

---

## Table of Contents

1. [lib.rs — entry point](#librs)
2. [client.rs — MCP client](#clientrs)
3. [server.rs — MCP server](#serverrs)
4. [bridge.rs — tool bridge](#bridgers)

---

## lib.rs

The [lib.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/mcp/src/lib.rs) file declares three public modules and re-exports all of their contents:

```rust
pub mod client;
pub mod server;
pub mod bridge;

pub use client::*;
pub use server::*;
pub use bridge::*;
```

Any public item from `client.rs`, `server.rs`, or `bridge.rs` is available at the crate root. This means users of the crate can write `use mcp::McpClient` or `use mcp::connect_and_register` without needing to know the internal module structure.

---

## client.rs

The [client.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/mcp/src/client.rs) file is the main MCP client, supporting the stdio and HTTP transports, OAuth authentication, tool caching, and reconnection.

### Design Philosophy

The client is designed around a **connection-per-server** model: `McpClient` holds a `HashMap<String, McpConnection>` where each entry is a named server. Multiple servers can be connected simultaneously, each with its own transport. The client is wrapped in `Arc<Mutex<McpClient>>` for shared access, which means all tool calls through the bridge are serialized — a deliberate trade-off since the stdio transport uses a single stdin/stdout stream per process.

The client implements **dynamic tool discovery**: tool lists are fetched once via `tools/list`, cached per server, and invalidated when the server sends a `notifications/tools/list_changed` notification. This avoids unnecessary round-trips while supporting servers whose tool sets change over time.

### MCP Protocol Overview

MCP is built on **JSON-RPC 2.0**. Every message has the structure:

**Request (with id):**
```json
{"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}
```

**Notification (no id, fire-and-forget):**
```json
{"jsonrpc": "2.0", "method": "notifications/initialized"}
```

**Response:**
```json
{"jsonrpc": "2.0", "id": 1, "result": {"tools": [...]}}
```

**Error:**
```json
{"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "Method not found"}}
```

The **initialization handshake** has three steps:
1. Client → Server: `initialize` (sends client capabilities and protocol version)
2. Server → Client: response with server capabilities (supported protocol version, features)
3. Client → Server: `notifications/initialized` (signals readiness)

After initialization, the client can call `tools/list` to discover available tools and `tools/call` to invoke them. The server can send `notifications/tools/list_changed` at any time to signal that the tool list has changed.

### Constants

```rust
pub const TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
```

The name of the notification that the MCP server sends when its tool list has changed. The client uses it to invalidate the cache. When this notification is intercepted during a `request()` call, the server name is added to `tools_dirty`, and the next `discover_tools()` call will fetch a fresh list instead of returning the cached one.

### `McpTransport` Enum

```rust
pub enum McpTransport {
    Stdio { command: String, args: Vec<String> },
    Http { url: String, auth: Option<OAuthConfig> },
}
```

This enum selects the transport mechanism. The `Stdio` variant spawns a child process and communicates over its stdin/stdout. The `Http` variant sends POST requests to a remote URL, optionally with OAuth authentication. The `auth` field is `None` by default and can be set later via `OAuthConfig`.

#### Method `from_config(config: &McpServerConfig) -> anyhow::Result<Self>`

Algorithm:
1. Reads `config.transport` (a string).
2. If `"stdio"`: extracts `config.command` (required) and `config.args`, returns `McpTransport::Stdio`.
3. If `"http"`, `"streamable-http"`, or `"sse"`: extracts `config.url` (required), returns `McpTransport::Http { url, auth: None }`. (OAuth is configured separately.)
4. Any other value — error `"unknown MCP transport"`.

The `"sse"` alias exists because some MCP servers expose only SSE endpoints; the client handles them the same way as `"http"` — the SSE parsing logic in `post()` handles both.

### `OAuthConfig` Struct

```rust
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub token_url: String,
}
```

OAuth 2.0 Client Credentials Grant configuration. Serialized/deserialized via serde and can be read from TOML configuration. The `token_url` is the endpoint that issues access tokens. The client uses these credentials to obtain a bearer token before sending requests to the MCP server.

### `OAuthToken` Struct

```rust
struct OAuthToken {
    access_token: String,
    expires_at: Option<std::time::Instant>,
}
```

A cached token. The `expires_at` field is the absolute expiration moment (`Instant`). If `None`, the token is considered permanent (the server did not return `expires_in`). The client refreshes the token 30 seconds before actual expiry to avoid mid-request failures.

#### Method `is_expired(&self) -> bool`

Returns `true` if `Instant::now() >= expires_at`. If `expires_at == None`, returns `false` (the token never expires).

### `McpClient` Struct

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
|------|----------|
| `connections` | Active connections, indexed by server name. The server name is the logical identifier used in configuration. |
| `configs` | Server configurations (for reconnection). Stored so that `reconnect()` can re-establish a dropped connection without needing the original config. |
| `tool_cache` | Cache of tool lists per server. Avoids redundant `tools/list` calls when tools haven't changed. |
| `tools_dirty` | The set of servers that notified of changed tools. When a server sends `notifications/tools/list_changed`, its name is added here; `discover_tools()` checks this set before returning the cache. |
| `next_id` | Counter for generating unique JSON-RPC request ids. Monotonically increasing across all servers. |

### Internal Types

#### `McpConnection`

```rust
enum McpConnection {
    Stdio(StdioConn),
    Http(HttpConn),
}
```

An enum dispatching between the two transport implementations. The `request()` and `notify()` methods on `McpConnection` handle both variants transparently.

#### `StdioConn`

```rust
struct StdioConn {
    child: Child,           // child process handle
    stdin: ChildStdin,      // process stdin (for writing JSON-RPC requests)
    stdout: BufReader<ChildStdout>, // buffered stdout (for reading JSON-RPC responses)
}
```

The stdio connection holds a handle to the child process and its piped stdin/stdout. The `child` field is retained so the connection can kill the process on disconnect. `stdout` is wrapped in `BufReader` for line-by-line reading, which is the natural framing for newline-delimited JSON-RPC.

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

`session_id` is the MCP session identifier issued by the server during initialization. It is passed in the `mcp-session-id` header of every subsequent request. The `client` is a `reqwest::Client` with a 60-second timeout configured at construction time.

#### JSON-RPC structs

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
    method: Option<String>,                   // for notifications/requests from the server
}

struct JsonRpcError {
    code: i64,
    message: String,
}
```

The `JsonRpcRequest` uses `#[serde(skip_serializing_if = "Option::is_none")]` on `id` and `params` so that notifications (which have no `id`) are serialized without the `id` field, and requests without params omit it. The `JsonRpcResponse` uses `#[serde(default)]` on all optional fields so that partial responses can be deserialized without error. The `method` field on `JsonRpcResponse` captures server-initiated notifications and requests that arrive interleaved with responses on the stdio transport.

#### `IncomingKind` and `classify`

```rust
enum IncomingKind {
    Response,       // has id, no method → response to our request
    Notification,   // no id, has method → notification from the server
    ServerRequest,  // has id and method → request from the server (not supported)
}
```

The `classify(value: &serde_json::Value) -> IncomingKind` function:

1. Checks whether `id` is present (not null).
2. Checks whether `method` is present (not null).
3. Returns a variant based on the `(has_id, has_method)` combination:
   - `(true, false)` → `Response`
   - `(false, true)` → `Notification`
   - `(true, true)` → `ServerRequest`
   - `(false, false)` → `Response` (malformed — left to the calling code)

This classification is essential for the stdio transport, where the server may interleave notifications and responses on the same stream. The client must distinguish between responses (which should be returned to the caller) and notifications (which should be collected and passed along with the response). Server-initiated requests (e.g., `sampling/createMessage`) are not supported and are silently ignored with a debug log.

### Function `parse_sse_data_events(body: &str) -> Vec<String>`

Algorithm for parsing an SSE (Server-Sent Events) body according to the SSE specification:

1. Creates empty `events: Vec<String>` and `data_lines: Vec<&str>`.
2. Iterates over `body.lines()`:
   - **Empty line**: event separator. If `data_lines` is not empty — joins them with `\n` and adds to `events`. Clears `data_lines`.
   - **Line with the `data:` prefix**: extracts the content after `data:`, removes one leading space (if present), adds to `data_lines`.
   - **Any other line** (`event:`, `id:`, `retry:`, comments `:`) — ignored.
3. After the loop: if `data_lines` is not empty (the last event without a trailing empty line) — adds to `events`.
4. Returns `events`.

This function is used by the HTTP transport when the server responds with `Content-Type: text/event-stream`. The MCP server may send multiple SSE events in a single response — for example, progress notifications followed by the final JSON-RPC response. The `post()` method iterates the events in reverse order to find the response efficiently.

**Example**:
```
data: {"a":1}

data: line1
data: line2
```
→ `vec!["{\"a\":1}", "line1\nline2"]`

### Function `parse_token_response(body: &serde_json::Value) -> anyhow::Result<(String, Option<Duration>)>`

Algorithm for parsing the OAuth endpoint response:

1. Extracts `body["access_token"]` as a string. If absent — error.
2. Extracts `body["expires_in"]` as `u64` (seconds).
3. If `expires_in` is present, computes the lifetime: `Duration::from_secs(max(expires_in - 30, 1))` — refreshes 30 seconds before expiration (safety margin). If `expires_in` is absent, lifetime = `None`.
4. Returns `(access_token, lifetime)`.

The 30-second safety margin prevents the token from expiring during an in-flight request, which would cause a 401 error that the client cannot automatically retry.

### `McpConnection` Implementation

#### Method `request(&mut self, req: &JsonRpcRequest) -> anyhow::Result<(JsonRpcResponse, Vec<String>)>`

**Stdio branch:**

1. Serializes `req` into a JSON string.
2. Writes the string + `\n` to the child process's `stdin`.
3. Calls `flush()`.
4. Remembers `expected_id = req.id`.
5. Read-response loop:
   - `tokio::time::timeout(60s, stdout.read_line(&mut buf))` — a 60-second timeout for reading one line.
   - On timeout — error `"timed out waiting for MCP server response"`.
   - If 0 bytes were read (EOF) — error `"MCP server closed the connection"`.
   - Parses the line as `serde_json::Value`. If it fails to parse — skips (could be a server log).
   - Classifies via `classify(&value)`:
     - `Notification`: extracts `method` and adds to `notifications`.
     - `ServerRequest`: logs a debug message and skips (not supported).
     - `Response`: deserializes into `JsonRpcResponse`. If `expected_id` is set and `resp.id != expected_id` — skips (stale response). Otherwise — returns `(resp, notifications)`.

The notification collection is important: the `notifications/initialized` handshake and `notifications/tools/list_changed` notifications may arrive during any request. The caller inspects the collected notifications and acts on them (e.g., marking the tool cache as dirty).

**Http branch:**

1. Calls `c.post(req).await`.
2. Returns `(resp, Vec::new())` — notifications are not intercepted for HTTP (they arrive as separate responses). The `notifications` vector is empty, so `mark_tools_dirty` has no effect for HTTP responses.

#### Method `notify(&mut self, method: &str) -> anyhow::Result<()>`

Sends a JSON-RPC notification (without `id`):

1. Creates `JsonRpcRequest { jsonrpc: "2.0", id: None, method, params: None }`.
2. **Stdio**: serializes, writes to stdin + `\n`, flush.
3. **Http**: calls `c.post(&msg).await`, the result is ignored.

No response is expected or read. For the HTTP transport, the server returns HTTP 202 Accepted, which the `post()` method handles by returning an empty `JsonRpcResponse`.

### `HttpConn` Implementation

#### Method `ensure_token(&mut self) -> anyhow::Result<Option<String>>`

Lazy OAuth token acquisition/refresh:

1. If `self.auth == None` — returns `None` (no authentication).
2. If `self.token` exists and has not expired — returns `Some(token.access_token.clone())`.
3. Otherwise — requests a new token:
   - `POST {auth.token_url}` with form data:
     - `grant_type=client_credentials`
     - `client_id={auth.client_id}`
     - `client_secret={auth.client_secret}`
   - If the HTTP status is not 2xx — error with the response body.
   - Parses the JSON response via `parse_token_response`.
   - Creates `OAuthToken { access_token, expires_at: Some(Instant::now() + lifetime) }`.
   - Stores it in `self.token`.
   - Returns `Some(access_token)`.

The token is cached in `self.token` and reused across requests until it expires. The 30-second safety margin in `parse_token_response` means the token is refreshed before it actually expires, preventing 401 errors during critical tool calls.

#### Method `post(&mut self, msg: &JsonRpcRequest) -> anyhow::Result<JsonRpcResponse>`

Sends a single JSON-RPC message over HTTP. Algorithm:

1. Calls `self.ensure_token()`.
2. Builds the POST request:
   - URL: `self.url`
   - Header `Accept: application/json, text/event-stream`
   - Body: JSON-serialized `msg` via `.json(msg)`.
   - If there is a token: `Authorization: Bearer {token}`.
   - If there is a `session_id`: the `mcp-session-id` header.
3. Sends the request.
4. **session_id update**: if the response contains an `mcp-session-id` header, stores it in `self.session_id`.
5. **HTTP 202 Accepted**: returns an empty `JsonRpcResponse` (response to a notification).
6. **Unsuccessful status**: error with the body.
7. **Content-Type check**:
   - If `text/event-stream`:
     - Reads the body as a string.
     - Calls `parse_sse_data_events(&body)`.
     - Iterates over the events **in reverse order** (`.rev()`).
     - Tries to deserialize each as `JsonRpcResponse`.
     - Returns the first one that succeeds.
     - If none match — error `"no JSON-RPC response found in SSE stream"`.
   - Otherwise: deserializes the body as `JsonRpcResponse`.

**Note on SSE reverse order**: the JSON-RPC response is usually the last SSE event in the stream (after possible intermediate progress events), so iterating from the end optimizes the search. If the server sends progress notifications before the final response, they are skipped because they don't deserialize as `JsonRpcResponse` (they lack the `id` field, or have a different structure).

### `McpClient` Implementation

#### `new() -> Self`

Creates an empty client. All HashMaps are empty, `next_id = 0`.

#### `next_request_id(&mut self) -> u64`

Increments `self.next_id` and returns it. Each call yields a unique id. Ids are never reused across the lifetime of the client, preventing confusion between responses to different requests.

#### `connect(&mut self, config: &McpServerConfig) -> anyhow::Result<()>`

Connects to a server with a connection pool:

1. If `config.name` is already in `self.connections` — logs debug, updates the configuration, and returns `Ok(())` (reuse).
2. Calls `McpTransport::from_config(config)`.
3. Depending on the transport:
   - `Stdio` → `self.connect_stdio(config).await`
   - `Http` → `self.connect_http(&config.name, url, auth).await`
4. Stores the configuration in `self.configs`.

Connection pooling means that calling `connect()` twice with the same server name is idempotent — the second call just updates the config without creating a new connection.

#### `connect_all(&mut self, config: &McpConfig) -> anyhow::Result<()>`

Connects to all servers from the configuration:

1. Iterates over `config.servers`.
2. For each: `self.connect(server).await`. Errors are logged, not fatal.
3. If the connection succeeds: `self.initialize(&server.name).await`. Errors are also logged.

This is the main entry point used by `connect_and_register()` in the bridge. It connects to every configured server and initializes the MCP handshake. Individual failures don't block other servers — a best-effort approach that allows the system to work even if some servers are unreachable.

#### `connect_stdio(&mut self, config: &McpServerConfig) -> anyhow::Result<()>`

Step-by-step algorithm:

1. Extracts `config.command` (required for stdio).
2. Creates `Command::new(command)` with `config.args` as arguments.
3. Configures:
   - `stdin(Stdio::piped())` — intercept stdin.
   - `stdout(Stdio::piped())` — intercept stdout.
   - `stderr(Stdio::null())` — discard stderr.
   - `kill_on_drop(true)` — kill the process on drop.
4. `cmd.spawn()` — spawns the child process.
5. Takes `stdin` and `stdout` from the `Child` (via `.take()`).
6. Creates `StdioConn { child, stdin, stdout: BufReader::new(stdout) }`.
7. Inserts `McpConnection::Stdio(conn)` into `self.connections`.
8. Logs `"Connected to MCP server: {name}"`.

**Important**: after this call the connection is established, but **initialization has not been performed yet**. You need to call `initialize()` separately. The child process is killed when the `StdioConn` is dropped (due to `kill_on_drop(true)`), which happens automatically on disconnect.

#### `connect_http(&mut self, name, url, auth) -> anyhow::Result<()>`

Algorithm:

1. Creates `reqwest::Client::builder()` with a 60-second timeout.
2. Creates `HttpConn { client, url, auth, token: None, session_id: None }`.
3. Inserts `McpConnection::Http(conn)` into `self.connections`.
4. Logs `"Connected to MCP server (http): {name} -> {url}"`.

Similarly to stdio — initialization is not performed automatically. The `session_id` is filled in during the first `initialize()` call when the server returns the `mcp-session-id` header.

#### `initialize(&mut self, server_name: &str) -> anyhow::Result<()>`

MCP initialization protocol. Algorithm:

1. Generates `id = self.next_request_id()`.
2. Builds the JSON-RPC request:
   ```json
   {
     "jsonrpc": "2.0",
     "id": <id>,
     "method": "initialize",
     "params": {
       "protocolVersion": "2024-11-05",
       "capabilities": {},
       "clientInfo": {
         "name": "fathom",
         "version": "0.1.0"
       }
     }
   }
   ```
3. Gets a mutable reference to the connection from `self.connections`.
4. Calls `conn.request(&request).await` — gets `(response, notifications)`.
5. Checks `response.error` — if present, error with the message.
6. Sends the notification `conn.notify("notifications/initialized").await` — this completes the handshake.
7. Calls `self.mark_tools_dirty(server_name, &notifications)` — if the server sent a tools-changed notification during initialization, marks the cache as stale.

**MCP handshake protocol**:
1. Client → server: `initialize` (request with client capabilities).
2. Server → client: response with server capabilities.
3. Client → server: `notifications/initialized` (ready notification).

The `protocolVersion` of `"2024-11-05"` is the current MCP standard. The client announces `capabilities: {}` (no special capabilities) and identifies itself as `"fathom"` version `"0.1.0"`.

#### `list_tools(&mut self, server_name: &str) -> anyhow::Result<Vec<ToolSchema>>`

Gets the list of tools. Algorithm:

1. Builds a `tools/list` request with a unique `id`.
2. Sends it via `conn.request(&request)`.
3. Checks `response.error`.
4. Extracts `response.result["tools"]`, deserializes into `Vec<McpToolDef>`.
5. Maps each `McpToolDef` to `ToolSchema`:
   - `name` → `name`
   - `description` → `description` (or empty string if `None`)
   - `input_schema` → `parameters` (or `{"type": "object"}` if `None`)
6. Stores in `self.tool_cache[server_name]` and removes from `tools_dirty`.
7. Returns `schemas`.

The `McpToolDef` struct handles the MCP field naming convention (`inputSchema` → `input_schema` via `#[serde(rename = "inputSchema")]`). Tools with missing `inputSchema` default to `{"type": "object"}`, which accepts any JSON object.

#### `discover_tools(&mut self, server_name: &str) -> anyhow::Result<Vec<ToolSchema>>`

A caching wrapper around `list_tools`. Algorithm:

1. If `server_name` is **not** in `tools_dirty` **and** there is a cache — returns the cache.
2. Otherwise calls `self.list_tools(server_name).await`.

This avoids extra round-trips to the server if the tools have not changed. The `tools_dirty` set is populated by `mark_tools_dirty()` when a `notifications/tools/list_changed` notification is intercepted.

#### `cached_tools(&self, server_name: &str) -> Option<&[ToolSchema]>`

Returns a reference to the cached tool list (or `None`). Unlike `discover_tools`, this never triggers a network call — it's a pure cache lookup.

#### `call_tool(&mut self, server_name, tool_name, args) -> anyhow::Result<serde_json::Value>`

Calls a tool. Algorithm:

1. Builds the `tools/call` request:
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
2. Sends it via `conn.request(&request)`.
3. Checks notifications via `mark_tools_dirty` (the tool may have changed during the call).
4. Checks `response.error`.
5. Returns `response.result` (or `{}` if result = None).

The `mark_tools_dirty` call after `tools/call` is important: some MCP servers update their tool list as a side effect of tool execution (e.g., a tool that creates a new file might add a new tool). The bridge's `McpBridgeTool` calls this method, then checks the cache on the next `discover_tools` call.

#### `reconnect(&mut self, server_name: &str) -> anyhow::Result<()>`

Reconnection. Algorithm:

1. Extracts the configuration from `self.configs[server_name]`. If not found — error.
2. Calls `self.disconnect(server_name).await` — kills the old connection.
3. Clears `tool_cache` and `tools_dirty` for this server.
4. Calls `self.connect(&config).await` — new connection.
5. Calls `self.initialize(server_name).await` — full initialization.
6. Logs `"Reconnected MCP server: {server_name}"`.

Reconnection is triggered by the bridge when a tool call fails with `"closed the connection"`. This handles the case where a stdio server process crashes or an HTTP server becomes temporarily unavailable.

#### `disconnect(&mut self, server_name: &str)`

Disconnects one server:

1. Removes the connection from `self.connections`.
2. If it is `Stdio` — calls `c.child.kill().await` (kills the child process).

For HTTP connections, the `reqwest::Client` is simply dropped, which closes any idle connections.

#### `is_connected(&self, server_name: &str) -> bool`

Checks whether `server_name` is in `self.connections`.

#### `connected_servers(&self) -> Vec<String>`

Returns the names of all connected servers.

#### `shutdown(&mut self)`

Disconnects all servers:

1. Collects the names of all keys in `self.connections`.
2. For each, calls `self.disconnect(&name).await`.

#### `mark_tools_dirty(&mut self, server_name, notifications)`

Algorithm:
1. Checks whether `TOOLS_LIST_CHANGED` is present in the `notifications` array.
2. If yes — adds `server_name` to `self.tools_dirty`.

This is called after every `request()` call that returns notifications, and after `initialize()`.

### Error Handling

The client uses `anyhow::Result` throughout. Specific error conditions:

- **Transport errors**: `McpTransport::from_config` returns an error for unknown transport strings or missing required fields (command for stdio, URL for HTTP).
- **Connection errors**: `connect_stdio` returns an error if the child process cannot be spawned or stdin/stdout cannot be piped. `connect_http` returns an error if the HTTP client cannot be built.
- **Timeout errors**: `request()` on stdio has a 60-second timeout per line read. A hung server returns `"timed out waiting for MCP server response"`.
- **Connection closed**: `request()` on stdio returns `"MCP server closed the connection"` when stdin reaches EOF. This triggers the bridge's auto-reconnect logic.
- **JSON-RPC errors**: `initialize()`, `list_tools()`, and `call_tool()` all check `response.error` and return an error with the message.
- **HTTP errors**: `post()` returns an error for non-2xx status codes with the response body.
- **SSE parsing errors**: `post()` returns `"no JSON-RPC response found in SSE stream"` if no event in the SSE stream can be deserialized as a response.
- **OAuth errors**: `ensure_token()` returns an error for non-2xx token responses with the response body.

---

## server.rs

The [server.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/mcp/src/server.rs) file implements a simple stdio-based MCP server that exposes the agent's tools to external MCP clients (Claude Desktop, ZCode, etc.).

### Design

The server supports two modes:

1. **Schema-only mode** (`McpServer::new`): `tools/list` works and returns tool metadata, but `tools/call` returns a placeholder error. This is useful for testing, discovery-only setups, or when the server is used as a registry that other systems query.
2. **Executor mode** (`McpServer::with_executor`): `tools/call` actually runs the tool through the shared `pr_tools::ToolRegistry`. This is the production mode where external MCP clients can invoke the agent's tools.

### `McpServer` Struct

```rust
pub struct McpServer {
    tools: Vec<ToolSchema>,
    executor: Option<(Arc<pr_tools::ToolRegistry>, Arc<pr_tools::ToolContext>)>,
}
```

The `executor` field is `None` in schema-only mode and `Some` in executor mode. When `Some`, it holds a reference to the global `ToolRegistry` (which contains all registered tools) and a `ToolContext` (which provides environment context like working directory and search configuration).

#### Constructor `new(tools: Vec<ToolSchema>)`

Stores the given list of tools. The executor is `None`, so `tools/call` returns a placeholder.

#### Constructor `with_executor(registry, ctx) -> Self`

1. Calls `registry.list_schemas()` to get the full list of registered tools.
2. Stores the tools and executor.

This constructor is used when the server should actually execute tool calls. The tool list is derived from the registry at construction time.

### Method `run_stdio(&self) -> anyhow::Result<()>`

The MCP server main loop. Algorithm:

1. Gets handles to `tokio::io::stdin()` and `tokio::io::stdout()`.
2. Wraps stdin in `BufReader::new(stdin)` for line-by-line reading.
3. Infinite loop:
   - `reader.read_line(&mut line).await?` — reads one line.
   - If 0 bytes were read (EOF) — `break` (shuts down the server).
   - Trims the line (`trim()`). If empty — `continue`.
   - Calls `self.handle_request(line).await` → gets the JSON response.
   - Serializes the response into a string.
   - Writes the string + `\n` to stdout.
   - Calls `flush()`.

The server reads one JSON-RPC message per line from stdin and writes one JSON-RPC response per line to stdout. This is the standard MCP stdio transport framing.

### Method `handle_request(&self, request: &str) -> Option<serde_json::Value>`

Handles a single JSON-RPC request line. Returns `None` for notifications (which don't need a response). Algorithm:

1. **Parsing**: `serde_json::from_str(request)` into `serde_json::Value`. If it fails to parse — returns:
   ```json
   {"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}
   ```
   (Code -32700 is the standard JSON-RPC parse error code.)

2. **Notification detection**: extracts `id` from the request. If `id` is absent or `null` — returns `None` (no response). This is how JSON-RPC notifications are handled: the method is received but no response is sent.

3. **Field extraction**: `method = req["method"]` (or an empty string).

4. **Routing by method**:

   - **`"initialize"`**: returns:
     ```json
     {
       "jsonrpc": "2.0",
       "id": <id>,
       "result": {
         "protocolVersion": "2024-11-05",
         "capabilities": {"tools": {}},
         "serverInfo": {
           "name": "fathom",
           "version": "0.1.0"
         }
       }
     }
     ```
     The `{"tools": {}}` capability means the server supports tools (without additional options like `listChanged`). The server version is derived from `CARGO_PKG_VERSION` at compile time.

   - **`"tools/list"`**: maps each `ToolSchema` into:
     ```json
     {
       "name": "<tool.name>",
       "description": "<tool.description>",
       "inputSchema": "<tool.parameters>"
     }
     ```
     and returns the array in `result.tools`. All tools registered in the server are returned.

   - **`"tools/call"`**: handles tool execution:
     - Extracts `params.name` (tool name) and `params.arguments` (tool arguments).
     - If `self.executor` is `None` (schema-only mode): returns a placeholder error:
       ```json
       {"result": {"content": [{"type": "text", "text": "Tool execution not available (schema-only server)"}], "isError": true}}
       ```
     - If `self.executor` is `Some`: calls `registry.execute(name, arguments, ctx).await`:
       - On success: returns `{content: [{type: "text", text: output.content}], isError: !output.success}`
       - On error: returns `{content: [{type: "text", text: "tool execution failed: {e}"}], isError: true}`

   - **`"ping"`**: returns an empty result `{}`. This is a health-check endpoint that some MCP clients use to verify the server is alive.

   - **Any other method**: returns an error:
     ```json
     {"jsonrpc": "2.0", "id": <id>, "error": {"code": -32601, "message": "Method not found"}}
     ```
     (Code -32601 is the standard JSON-RPC "method not found" code.)

### Error Handling

The server handles several error conditions gracefully:

- **Invalid JSON**: returns JSON-RPC error code -32700 (Parse error) with `id: null`.
- **Missing method**: returns JSON-RPC error code -32601 (Method not found) with the request's `id`.
- **Unknown tool** (executor mode): `registry.execute` returns an error, which is converted to `isError: true` in the result, not a JSON-RPC error. This is important: tool execution errors are part of the result, not protocol-level errors.
- **Schema-only `tools/call`**: returns a clear error message in the result, not a JSON-RPC error.

### Tests

The server has comprehensive tests covering:

- **`handle_initialize`**: verifies protocol version, server info, and JSON-RPC structure.
- **`handle_tools_list`**: verifies tool metadata is returned correctly.
- **`handle_tools_call_schema_only_mode`**: verifies the placeholder error message.
- **`notifications_get_no_response`**: verifies that notifications (no id) return `None`.
- **`handle_unknown_method`**: verifies JSON-RPC error code -32601.
- **`handle_invalid_json`**: verifies JSON-RPC error code -32700.
- **`handle_missing_method`**: verifies that missing method also returns -32601.
- **`tools_list_empty`**: verifies empty tool list is handled.
- **`tools_list_multiple`**: verifies multiple tools are returned correctly.
- **`executor_mode_runs_real_tools`**: end-to-end test that creates a real `ToolRegistry` with built-in tools, calls `glob` (a real tool) and an unknown tool, verifying both success and error paths.

---

## bridge.rs

The [bridge.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/mcp/src/bridge.rs) file implements the bridge between MCP servers and the agent's local tool registry. Remote MCP tools are wrapped in `McpBridgeTool`, which implements the `Tool` trait from `pr_tools`. This allows agents to call remote MCP tools as if they were built-in, using the same `ToolRegistry::execute()` interface.

### Design

The bridge follows a **namespacing** approach: each remote tool is registered with a name like `mcp__{server}__{tool}`. This prevents collisions between:
- Tools from different MCP servers (e.g., two servers both exporting a tool called `search`)
- MCP tools and built-in tools (e.g., a built-in `read` tool vs. an MCP server's `read` tool)

The bridge is **best-effort**: if a server is unreachable during registration, it's skipped with a warning. The shared `Arc<Mutex<McpClient>>` keeps connections alive for the lifetime of the application.

### Constants

```rust
const NAME_SEP: &str = "__";
```

The separator between parts of a bridge tool name. Format: `mcp__{server}__{tool}`.

### `McpBridgeTool` Struct

```rust
pub struct McpBridgeTool {
    server: String,                    // MCP server name
    tool_name: String,                 // original tool name on the server
    bridged: String,                   // full bridged name: mcp__server__tool
    description: String,               // tool description
    parameters: serde_json::Value,     // JSON Schema of parameters
    client: Arc<Mutex<McpClient>>,     // shared MCP client (shared between tools)
}
```

#### Static method `bridged_name(server: &str, tool: &str) -> String`

Builds the name: `"mcp__{server}__{tool}"`.

**Example**: `bridged_name("web-search", "query")` → `"mcp__web-search__query"`.

#### Constructor `new(server, schema: ToolSchema, client) -> Self`

1. Computes `bridged = Self::bridged_name(&server, &schema.name)`.
2. Fills the fields from `schema` and `server`.

### `Tool` Trait Implementation for `McpBridgeTool`

#### `name(&self) -> &str`

Returns `self.bridged` — the full name with the `mcp__` prefix.

#### `description(&self) -> &str`

Returns `self.description`.

#### `schema(&self) -> ToolSchema`

Returns a `ToolSchema` with:
- `name` = the full bridged name
- `description` = `"[MCP {server}] {original_description}"` — adds a prefix to identify MCP tools in the UI/logs.
- `parameters` = a clone of `self.parameters`

#### `execute(&self, args, _ctx) -> anyhow::Result<ToolOutput>`

The main execution method. Algorithm:

**Step 1 — Calling the MCP tool:**

1. Acquires the lock on `self.client` (`self.client.lock().await`).
2. Calls `client.call_tool(&self.server, &self.tool_name, args.clone()).await`.
3. Error handling:
   - **Success**: `Ok(value)` → proceed to step 2.
   - **Error with the text "closed the connection"**: auto-reconnect.
     1. Logs a warning: `"MCP server {server} went away; attempting reconnect"`.
     2. Calls `client.reconnect(&self.server).await`.
     3. If the reconnect succeeds: retries `client.call_tool(...)` with the original `args`.
     4. If the reconnect fails: returns an error combining both errors.
   - **Any other error**: returns `Err(e)`.

**Step 2 — Converting the result:**

- `Ok(value)` → `Ok(mcp_result_to_output(&value))`
- `Err(e)` → `Ok(ToolOutput::err("MCP tool {server}.{tool} failed: {e}"))`

**Important**: MCP tool errors are NOT returned as `Err` — they are wrapped in `ToolOutput::err(...)` (Ok with success=false). This allows the agent to see the error as a tool result rather than a system failure. The agent can then decide how to handle the error (e.g., retry, report to user, try a different approach).

### Auto-Reconnect Logic

The auto-reconnect is a robustness feature: if a stdio server process crashes, the next tool call fails with `"closed the connection"`. The bridge catches this specific error, reconnects the server (which spawns a new process), and retries the tool call exactly once. If the reconnect itself fails, the original error is returned alongside the reconnect failure.

### Function `mcp_result_to_output(value: &serde_json::Value) -> ToolOutput`

Converts the `tools/call` result into `ToolOutput`. Algorithm:

1. Extracts `value["content"]` as an array.
2. If the array exists:
   - Filters the elements: only those with `type == "text"` and with a `text` field.
   - Extracts the text of each matching element into a `Vec<String>`.
   - If the vector is not empty:
     - Checks `value["isError"]` (boolean). If absent — `false`.
     - Joins the texts with `\n`.
     - If `isError == true` → `ToolOutput::err(joined)`.
     - If `isError == false` → `ToolOutput::ok(joined)`.
     - Returns the result.
3. **Fallback**: if `content` is absent, empty, or contains no text elements — `ToolOutput::ok(serde_json::to_string_pretty(value))` — a pretty-printed JSON of the whole object.

The MCP standard result format is `{content: [{type: "text", text: "..."}, ...], isError: false}`. The function handles both this standard format and any non-standard response by falling back to pretty-printed JSON.

### Function `connect_and_register(registry, config) -> Option<Arc<Mutex<McpClient>>>`

The main entry point for integrating MCP with the tool registry. Algorithm:

1. If `config.servers` is empty — returns `None` (no servers).
2. Creates `McpClient::new()`.
3. Calls `client.connect_all(config).await`. Errors are logged, not fatal.
4. Gets the list `client.connected_servers()`.
5. If no server connected — logs a warning, returns `None`.
6. Wraps the client in `Arc<Mutex<McpClient>>`.
7. For each connected server:
   - Acquires the lock: `client.lock().await`.
   - Calls `c.discover_tools(&server).await`. On error — logs and `continue`.
   - For each `ToolSchema` from the result:
     - Creates `McpBridgeTool::new(server, schema, client.clone())`.
     - Registers in `registry` via `registry.register(Arc::new(bridge_tool))`.
     - Increments the `registered` counter.
8. Logs: `"registered {registered} MCP tool(s) from {server_count} server(s)"`.
9. Returns `Some(client)` — a shared reference to the client is needed to keep the connections alive and for future reconnections.

**Note**: `Arc<Mutex<McpClient>>` is shared between all `McpBridgeTool`s. When the agent calls any MCP tool, it acquires the mutex, which means **sequential** execution of MCP calls (no parallelism). This is a characteristic of the current implementation, stemming from the fact that `McpClient` does not support parallel requests to the same server (especially for the stdio transport, where stdin/stdout is a single stream).

### Error Handling in the Bridge

The bridge converts all errors into `ToolOutput` with `success: false` rather than propagating them as `Err`:

- **MCP tool errors**: `call_tool` returns a JSON-RPC error → `ToolOutput::err("MCP tool {server}.{tool} failed: {error}")`.
- **Connection errors**: `"closed the connection"` triggers auto-reconnect → if reconnect fails, returns `ToolOutput::err("MCP tool {server}.{tool} failed: {error} (reconnect failed: {reconnect_error})")`.
- **Non-standard results**: `mcp_result_to_output` falls back to pretty-printed JSON, so even unexpected response formats are returned as usable text.

This design ensures that an MCP tool failure never crashes the agent — it's always a recoverable tool output.