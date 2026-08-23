# Troubleshooting Guide

Common issues encountered while running Fathom, their root causes, and how to resolve them.

---

## Table of Contents

- [API Key Not Found](#api-key-not-found)
- [LLM Model Not Responding](#llm-model-not-responding)
- [LLM Rate Limiting](#llm-rate-limiting)
- [Tool Timeout](#tool-timeout)
- [Agent Stuck in a Loop (Doom Loop)](#agent-stuck-in-a-loop-doom-loop)
- [Streaming Failures](#streaming-failures)
- [Docker Build Issues](#docker-build-issues)
- [MCP Server Not Connecting](#mcp-server-not-connecting)
- [Database Locked](#database-locked)
- [Session Not Found](#session-not-found)
- [Agent Not Spawning Children](#agent-not-spawning-children)
- [Memory Search Returns Nothing](#memory-search-returns-nothing)
- [Approval Timeout](#approval-timeout)
- [Sandboxed Container Has No Internet](#sandboxed-container-has-no-internet)
- [Development / Testing](#development--testing)
- [SSRF Loopback Escape Hatch](#ssrf-loopback-escape-hatch)

---

## API Key Not Found

### Symptom

```
Error: Config error: missing field `api_key`
```

or the agent starts but all LLM calls fail with HTTP 401.

### Root Cause

Fathom reads the LLM API key from `~/.fathom/config.toml` under the `[llm]` section:

```toml
[llm]
api_key = "sk-…"
```

The key is missing, is empty, or is set to a placeholder.

### Resolution

1. **Set the key in the config file:**

   ```bash
   fathom config set llm.api_key "sk-your-actual-key"
   ```

2. **Verify the config was written:**

   ```bash
   fathom config show
   ```

3. **If using a search API** (Parallel, LinkUp, Exa, Tavily, Serper, Brave), also configure those keys in their respective sections (`[parallel]`, `[linkup]`, `[exa]`, `[tavily]`, `[serper]`, `[brave]`).

4. **Check the config file** — the supported LLM credential is `[llm].api_key` in `~/.fathom/config.toml`; Fathom does not claim a `FATHOM_LLM_API_KEY` environment override.

---

## LLM Model Not Responding

### Symptom

```
LLM error: timeout after 300s
LLM error: connection refused
LLM error: received invalid response
```

### Root Cause

- The LLM provider endpoint is unreachable (wrong `base_url`, network down, provider outage).
- The model name is wrong or not available on the configured provider.
- The request exceeds the provider's context window (default: 128k tokens).

### Resolution

1. **Verify the endpoint and model name:**

   Inspect `~/.fathom/config.toml` or run:

   ```bash
   fathom config show | grep -E 'base_url|model'
   ```

   Defaults are `https://api.deepseek.com` and `deepseek-chat`. For OpenAI-compatible providers, point `base_url` to the correct endpoint.

2. **Test connectivity with curl:**

   ```bash
   curl -s "$(grep 'base_url' ~/.fathom/config.toml | head -1 | cut -d'"' -f2)/chat/completions" \
     -H "Authorization: Bearer $(grep 'api_key' ~/.fathom/config.toml | head -1 | cut -d'"' -f2)" \
     -H "Content-Type: application/json" \
     -d '{"model":"'"$(grep 'model' ~/.fathom/config.toml | head -1 | cut -d'"' -f2)"'","messages":[{"role":"user","content":"hi"}]}'
   ```

3. **Check the provider status page** for an ongoing outage.

4. **Reduce the context window** if you hit token limits (see `[context]` section in config, default `128_000`).

5. **Increase the HTTP timeout** (hardcoded at 300s in the provider; see `crates/llm/src/deepseek.rs`). For very large prompts, the model may need more time.

---

## LLM Rate Limiting

### Symptom

```
API error 429: Too Many Requests
```

or repeated warnings in the log:

```
model deepseek-chat in rate-limit cooldown; waiting 60s
```

### Root Cause

The LLM provider returned HTTP 429, meaning your API key has exceeded its rate quota. Fathom has two built-in defences (see `crates/llm/src/concurrency.rs`):

- **`ModelSemaphore`** — bounds concurrent requests per model (default 3). A fan-out of sub-agents queues instead of hammering the API simultaneously.
- **`FallbackCooldown`** — after a 429, the model lane is marked as cooling down for 60 seconds (30s for 5xx). Subsequent requests to the same model wait before retrying, preventing the swarm from re-hammering a rate-limited endpoint.

### Resolution

1. **Wait.** The built-in retry logic (`crates/llm/src/retry.rs`) automatically retries up to 3 times with exponential backoff (500ms → 1s → 2s, ±25% jitter). If the provider's `Retry-After` header is present, it is honoured (capped at 60s).

2. **Reduce the rate of requests.** The `ModelSemaphore` concurrency limit is hardcoded at 3 in `crates/llm/src/concurrency.rs` and is not user-configurable. Consider spacing out sub-agent fan-outs or upgrading your provider plan.

3. **Upgrade your provider plan** for a higher rate limit.

4. **Switch to a provider with higher limits** (e.g., OpenRouter, Groq) by changing `llm.base_url` and `llm.model`.

5. **Check the HTTP API rate limiter** if you are running in server mode (`fathom serve`). The server has its own per-client rate limiter (default 120 requests/minute, configurable via `$FATHOM_RATE_LIMIT`). Clients exceeding this get HTTP 429 from the server itself.

---

## Tool Timeout

### Symptom

```
Command timed out after 120s
Tool error: execution timed out
```

### Root Cause

A tool (shell command, Python script, Node.js snippet, web fetch, etc.) exceeded its timeout. Each tool has a default timeout:

| Tool          | Default Timeout | Configurable |
|---------------|-----------------|--------------|
| `shell`       | 120s            | `timeout` parameter |
| `python`      | 30s             | `timeout` parameter |
| `node`        | 30s             | `timeout` parameter |
| `web_fetch`   | 30s             | Hardcoded in tool |
| `git_*`       | 120s            | Hardcoded |
| `browser_*`   | 30s             | Hardcoded |
| Agent stall   | 450s (warn), 1200s (kill) | `[agent] stall_warn_seconds`, `stall_kill_seconds` |
| Approval      | 300s            | `[agent] approval_timeout_seconds` |

### Resolution

1. **For `shell`/`python`/`node` tools**, increase the timeout in the tool call:

   ```
   shell: command="cargo build" timeout=600
   ```

2. **For agent-level stall detection**, adjust the config:

   ```toml
   [agent]
   stall_warn_seconds = 600   # Default 450
   stall_kill_seconds = 1800  # Default 1200
   ```

3. **For `web_fetch`**, the tool has a hard 30s timeout on both connect and total time. If a site is consistently slow, try a different source or use `browser_navigate` which can handle JS-heavy slow pages.

4. **For the LLM provider**, the HTTP client has a 300s timeout (5 minutes). If the model is slow to generate, consider reducing the prompt size or switching to a faster model.

---

## Agent Stuck in a Loop (Doom Loop)

### Symptom

The agent repeats the same tool call (same tool name, same arguments) multiple times without making progress. The log shows:

```
doom loop detected: tool 'web_search' called 3 times with identical arguments
```

### Root Cause

A "doom loop" occurs when the LLM retries a failing operation with exactly the same parameters instead of adapting. Fathom detects this via `DoomLoopDetector` (see `crates/agent/src/doom_loop.rs`):

- Tracks the last N tool call signatures (tool name + hash of arguments).
- Default threshold: **3 consecutive identical calls**.
- **First offence**: the agent is nudged with a warning message telling it to adapt.
- **Second offence**: the agent is stopped to prevent exhausting the token budget.

### Resolution

1. **Let it recover.** On the first nudge, the agent receives a message explaining the loop and should try a different approach.

2. **If loops persist**, the task may be too vague or the available tools are insufficient for the task. Try:
   - Refining the query with more specific instructions.
   - Adding more tools or search sources.

3. **Adjust the threshold** (only via source code at `crates/agent/src/doom_loop.rs`):

   ```rust
   pub const DEFAULT_MAX_IDENTICAL: usize = 3;
   ```

4. **Check tool output truncation.** If the model cannot see the full error output (truncated by `[context] tool_output_max_bytes` default 50KB), it may keep retrying the same failing call. Increase the limit:

   ```toml
   [context]
   tool_output_max_bytes = 100_000
   tool_output_max_lines = 2_000
   ```

---

## Streaming Failures

### Symptom

```
LLM error: stream error: connection reset
LLM error: streaming response exceeded 52428800 byte limit
```

or garbled/partial tool calls in the agent's output.

### Root Cause

Fathom uses Server-Sent Events (SSE) for streaming LLM responses (`crates/llm/src/deepseek.rs`). Issues include:

- **Network interruption** mid-stream (connection reset, timeout).
- **Response too large** — the streaming fallback is triggered when the non-streaming response exceeds 10 MB, but the streaming path itself has a 50 MB cap.
- **Malformed SSE frames** — the parser handles multi-chunk UTF-8 correctly, but some providers emit non-standard SSE.
- **Tool call delta assembly** — streaming tool calls arrive in fragments (`id` + `name` on first delta, `arguments` in subsequent deltas). If the provider omits `id` on the first delta, the fragment may be misrouted.

### Resolution

1. **Check network stability.** Flaky connections cause mid-stream resets.

2. **Reduce the response size.** Limit the model's output with `max_tokens`:

   ```toml
   [llm]
   max_tokens = 4096  # Default 8192
   ```

3. **Disable streaming** (workaround if the provider's SSE implementation is broken). Set the model to a non-streaming fallback by patching the `complete` method in `crates/llm/src/deepseek.rs` — remove the streaming fallback logic and rely solely on the non-streaming retry path.

4. **Check provider compatibility.** Fathom's SSE parser expects OpenAI-compatible `data: {...}` lines. Some providers (Ollama, vLLM) may use different formats. Ensure the provider is fully OpenAI-compatible or override the `DeepSeekProvider` with a custom parser.

---

## Docker Build Issues

### Symptom

```
ERROR: failed to solve: process "/bin/sh -c cargo build --release --bin fathom" did not complete successfully
```

or the build succeeds but the container exits immediately.

### Root Cause

- **Missing dependencies** — the `rust:1.97-bookworm` image is minimal; native libraries (OpenSSL, libsqlite3) must be installed.
- **Out of memory** — `cargo build --release` can consume several GB of RAM on a large workspace.
- **Architecture mismatch** — building on Apple Silicon (arm64) and deploying on amd64.
- **Config file missing** — the runtime container expects `~/.fathom/config.toml`.

### Resolution

1. **Build with more memory:**

   ```bash
   docker build --memory=8g --memory-swap=8g -t fathom .
   ```

2. **Cross-compile for amd64 from Apple Silicon:**

   ```bash
   docker buildx build --platform linux/amd64 -t fathom .
   ```

3. **Mount the config file at runtime:**

   ```bash
   docker run -v ~/.fathom:/home/researcher/.fathom fathom serve
   ```

4. **Mount a data volume for the SQLite database and output:**

   ```bash
   docker run -v ~/.fathom:/home/researcher/.fathom \
              -v $(pwd)/data:/data \
              fathom serve
   ```

5. **If the build fails with a linker error**, install the missing library:

   ```dockerfile
   RUN apt-get update && apt-get install -y pkg-config libssl-dev libsqlite3-dev
   ```

6. **Use the build cache** to speed up incremental builds. The `Dockerfile` copies the entire workspace at once (no dependency-only cache layer). For faster iteration, split the copy into:

   ```dockerfile
   COPY Cargo.toml Cargo.lock ./
   COPY crates ./crates
   COPY src ./src
   ```

---

## MCP Server Not Connecting

### Symptom

```
MCP server 'my-server' failed to connect: connection refused
MCP server 'my-server' timed out after 10s
```

or tools from the MCP server are missing from the agent's tool list.

### Root Cause

Fathom connects to MCP servers via two transports (see `crates/mcp/src/client.rs`):

- **stdio**: spawns the server as a child process and speaks JSON-RPC over stdin/stdout.
- **Streamable HTTP**: POSTs JSON-RPC messages to a remote endpoint.

Common failures:

- The server binary/path is wrong or not executable.
- The server process crashes on startup (stderr is not captured in the log).
- The HTTP endpoint is unreachable or requires authentication.
- The server's `initialize` response is malformed.
- The server does not support the JSON-RPC protocol version.

### Resolution

1. **Test the server standalone:**

   For stdio servers:
   ```bash
   echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"fathom","version":"0.1"}}}' | ./path/to/mcp-server
   ```

   For HTTP servers:
   ```bash
   curl -s -X POST https://mcp-server.example.com \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"fathom","version":"0.1"}}}'
   ```

2. **Check the config:**

   ```toml
   [mcp]
   # stdio transport
   [mcp.servers.my-server]
   command = "/usr/local/bin/my-server"
   args = ["--flag"]

   # HTTP transport
   [mcp.servers.my-server]
   url = "https://mcp-server.example.com"
   ```

3. **For HTTP servers with OAuth**, note that OAuth credentials are not currently configurable through `[[mcp.servers]]` TOML entries. Configure `OAuthConfig` programmatically in the MCP client integration, or use a server endpoint that does not require OAuth.

4. **Check the server logs.** Fathom logs MCP connection attempts at `warn` level. Enable debug logging:

   ```bash
   RUST_LOG=debug fathom run "your query"
   ```

5. **Increase the connection timeout** (hardcoded in `crates/mcp/src/client.rs`). The default HTTP timeout is 10 seconds for the initial handshake.

---

## Database Locked

### Symptom

```
database is locked
Persistence error: SQLite busy
```

### Root Cause

SQLite's default journal mode has limited concurrency. Fathom uses WAL (Write-Ahead Logging) mode with a connection pool of 4 connections (see `crates/persistence/src/db.rs`), which allows concurrent reads and a single writer. However, certain operations can still collide:

- Multiple processes accessing the same database file.
- A long-running write transaction blocking reads.
- The database file resides on a network filesystem (NFS, SMB) that does not support SQLite locking.

### Resolution

1. **Ensure only one Fathom process** accesses the database file at a time. Each session gets its own database file by default (`./sessions.db`), but the jobs database and contacts database are shared.

2. **Check the filesystem.** SQLite's locking protocol requires POSIX advisory locks. Network filesystems (NFS with `nolock`, SMB) may not support these correctly. Move the database to a local filesystem.

3. **Increase the WAL pool size** (source code change in `crates/persistence/src/db.rs`):

   ```rust
   const POOL_SIZE: usize = 4; // Increase to 8
   ```

4. **Set a busy timeout** so concurrent operations wait instead of failing immediately:

   ```sql
   PRAGMA busy_timeout = 5000;
   ```

   Fathom sets this automatically on connection open.

5. **If the database is corrupted**, delete it and let Fathom recreate it:

   ```bash
   rm ~/.fathom/sessions.db
   ```

   (Contacts and jobs databases are separate; delete those too if needed.)

---

## Session Not Found

### Symptom

```
Session not found
fathom sessions get <id>: session not found
```

or resuming a session returns empty results.

### Root Cause

- The session ID is misspelled or from a different database.
- The session was never persisted (e.g., the agent crashed before the first write).
- The session database was deleted or reset.
- The session is stored in a different database file than the one being queried.

### Resolution

1. **List available sessions:**

   ```bash
   fathom sessions list
   ```

2. **Search by query text:**

   ```bash
   fathom sessions list --search "quantum computing"
   ```

3. **Check the session directory.** By default, sessions are stored in `./sessions/` (the current working directory). If you ran Fathom from a different directory, the sessions may be in a different location:

   ```bash
   fathom sessions --output /path/to/session-dir list
   ```

4. **If the session was created by a detached agent** (background job), the session is persisted in the jobs database (`~/.fathom/jobs.db`). List jobs instead:

   ```bash
   fathom jobs list
   fathom jobs logs <job-id>
   ```

5. **If the session was killed before completion**, it may exist but have no child agents. Check the session detail:

   ```bash
   fathom sessions show <id>
   ```

   A session with `status: "running"` and no agents may indicate the agent crashed during initialization.

---

## Agent Not Spawning Children

### Symptom

The agent calls `spawn_agent` but the tool returns an error:

```
Tool error: cannot spawn: max depth 2 reached (current depth 2)
Tool error: cannot spawn: role 'writer' is not allowed to create sub-agents
```

or the spawn is silently ignored and no child agent appears.

### Root Cause

Fathom enforces several constraints on child agent creation (see `crates/agent/src/runtime.rs`, `prepare_child`):

- **Depth limit** (`[agent] max_depth`, default 2): the coordinator is depth 0, its children are depth 1, grandchildren depth 2, etc. At `max_depth`, no further children can be spawned.
- **Agent count limit** (`[agent] max_agents`, default 20): total agents across the entire session tree.
- **Role gate**: only `Coordinator` and `Researcher` roles can spawn children. `Analyst`, `Verifier`, and `Writer` roles are prohibited.
- **Token budget exhaustion**: if the parent has exhausted its token cap, spawning is refused.
- **Empty task**: `spawn_agent` with an empty task string is rejected.

### Resolution

1. **Increase the depth limit:**

   ```toml
   [agent]
   max_depth = 4
   ```

2. **Increase the agent count limit:**

   ```toml
   [agent]
   max_agents = 50
   ```

3. **Check the role.** If you see "role 'writer' is not allowed to create sub-agents", the task is being assigned to a non-spawning role. Adjust the prompt to assign research tasks to the coordinator or a researcher.

4. **Check the token budget.** If spawning is refused due to token exhaustion, increase the session token limit:

   ```toml
   [agent]
   session_token_limit = 200_000  # Default unset (no limit)
   ```

5. **Verify the spawn request format.** The agent must pass a valid `task` string and a recognized `role`:

   ```json
   {
     "task": "Research quantum computing startups",
     "role": "researcher",
     "context": "Optional context handoff from parent"
   }
   ```

---

## Memory Search Returns Nothing

### Symptom

```
memory_search returned 0 results
```

or the agent reports "no relevant memories found" despite having stored facts.

### Root Cause

Memory search uses a hybrid semantic + keyword retrieval (see `crates/memory/src/search.rs`):

```
score = w · cosine_similarity + (1 − w) · bm25_normalized
score × max(0, 1 − temporal_decay · days_old)
score × (0.8 + 0.1 · reinforcement + 0.1 · confidence)
```

Common reasons for empty results:

- **Memory is disabled** — `[memory] enabled = false` (default: `true`).
- **Score below threshold** — `min_score` default is 0.25. Memories with low relevance are pruned.
- **Query too short** — BM25 keyword search requires meaningful tokens. A single word or very short query may not match.
- **Temporal decay** — `temporal_decay` default is 0.01/day. After 100 days, the score is multiplied by 0.0, effectively expiring the memory.
- **Memory expired** — `gc_ttl_days` default is 30. After 30 days without access, garbage collection may remove the memory.
- **Supersession chains** — the search resolves to `Active` status by default. If all memories have been superseded by newer versions, only the active ones appear.
- **Scope mismatch** — the search scope filter may exclude the session or agent scope the memories were stored in.

### Resolution

1. **Check memory is enabled:**

   ```bash
   fathom config show | grep -A1 '\[memory\]' | grep enabled
   ```

2. **Lower the minimum score threshold:**

   ```toml
   [memory]
   min_score = 0.1
   ```

3. **Reduce temporal decay:**

   ```toml
   [memory]
   temporal_decay = 0.001  # Slower decay
   ```

4. **Increase the TTL for garbage collection:**

   ```toml
   [memory]
   gc_ttl_days = 90
   ```

5. **List all memories to verify they exist:**

   ```bash
   fathom memory list
   ```

6. **Search with a broader query:**

   ```bash
   fathom memory search "your topic" --scope all
   ```

7. **Check the embedding backend.** If `embedding_backend = "auto"`, Fathom tries to use OpenAI's `text-embedding-3-small`. If the API key is missing or the network is down, embeddings fail silently and semantic search returns zero scores. Set `embedding_backend = "tfidf"` for offline lexical-only embeddings (no network required).

---

## Approval Timeout

### Symptom

```
approval[save_contacts]: timed out -> Denied
```

or the tool returns:

```
Denied by operator approval: 'save_contacts' was not allowed to run
```

### Root Cause

Fathom can gate side-effect tools behind operator approval (see `crates/agent/src/runtime.rs`, `request_approval`):

- Tools requiring approval are listed in `[agent] approval_tools` (default: `save_contacts`, `git_push`).
- When the agent calls one of these tools, it sends an approval request to the operator.
- The operator has `approval_timeout_seconds` (default 300s) to respond.
- If no operator is connected (headless mode), the `approval_fallback` is used (default `allow`).

### Resolution

1. **Connect an operator** — run `fathom tui` (the TUI includes an approval panel) or use the HTTP API's session control endpoint.

2. **Increase the approval timeout:**

   ```toml
   [agent]
   approval_timeout_seconds = 600
   ```

3. **Change the default fallback for headless runs:**

   ```toml
   [agent]
   approval_fallback = "allow"  # or "deny"
   ```

4. **Remove tools from the approval list if they do not need manual gates:**

   ```toml
   [agent]
   approval_tools = ["git_push"]  # Only git_push requires approval
   ```

5. **Check the control channel.** If the operator disconnects, the approval falls back to the configured default. The log shows:

   ```
   approval[save_contacts]: operator went away -> Allow
   approval[save_contacts]: control channel closed -> Deny
   ```

---

## Sandboxed Container Has No Internet

### Symptom

Tools that require network access (`web_fetch`, `web_search`, `verify_email`) fail with:

```
network error: connection refused
Failed to fetch https://…: dns error
```

### Root Cause

The Docker container or sandbox environment does not have outbound network access. In Docker, this can happen when:

- The container is run with `--network none`.
- Corporate firewalls block outbound traffic from the container.
- DNS resolution is not configured inside the container.

### Resolution

1. **Check network access inside the container** (the runtime image installs `ca-certificates` but no `curl`, so use `fathom` itself; override the entrypoint only if you add a network tool):

   ```bash
   docker run --rm fathom run "fetch https://api.deepseek.com and report the HTTP status"
   ```

2. **Run with the host network:**

   ```bash
   docker run --network host fathom run "your query"
   ```

3. **Configure custom DNS servers:**

   ```bash
   docker run --dns 8.8.8.8 --dns 1.1.1.1 fathom run "your query"
   ```

4. **If behind a corporate proxy**, set the `$HTTP_PROXY` and `$HTTPS_PROXY` environment variables. Fathom's HTTP client (`reqwest`) respects these automatically.

5. **Check the `no_proxy` setting** for internal-only resources.

6. **For tools that use local resources** (shell, python, node, git), they work without network — but any operation that clones a remote repo, downloads packages, or calls external APIs will fail.

## Cargo Build Failures

### Symptom

```
error[E0000]: … does not live long enough
error[E0277]: the trait bound … is not satisfied
error: failed to run custom build command for 'openssl-sys'
```

### Root Cause

- **Rust version mismatch** — Fathom supports and declares Rust 1.97 as its MSRV. The pinned `rust-toolchain.toml` uses Rust 1.97.1, and Docker uses the matching `rust:1.97-bookworm` builder. Cargo dependency metadata may report a lower floor (currently around 1.88); that is an informational dependency floor only, not a supported Fathom toolchain or compatibility guarantee.
- **Missing system libraries** — this workspace uses rustls for HTTPS and bundled SQLite; native OpenSSL/libsqlite3 are not required by the Rust build. Platform toolchains may still require standard compiler utilities.
- **Out of memory** — the workspace is large; `cargo build --release` can need 4+ GB.
- **Incremental compilation issues** — stale build artifacts.

### Resolution

1. **Update Rust:**

   ```bash
   rustup update stable
   rustc --version  # Should be 1.97+
   ```

2. **Install the standard Rust build toolchain.** The workspace uses rustls and bundled SQLite, so OpenSSL and a system SQLite library are not required. On Linux, install your distribution's C compiler/linker and certificate package if they are missing.

3. **Clean and rebuild:**

   ```bash
   cargo clean
   cargo build --release
   ```

4. **Build with more memory (if using Docker):**

   ```bash
   cargo build --release -j 4  # Limit parallel jobs
   ```

5. **Check for specific compile errors.** If the error mentions a missing trait, check that the feature flags are correct. Fathom uses conditional compilation for some features (e.g., `pg` for PostgreSQL contacts).

6. **If the `openssl-sys` crate fails**, try using the system OpenSSL or switch to `rustls`:

   ```bash
   # Use vendored OpenSSL
   OPENSSL_NO_VENDOR=0 cargo build --release

   # Or use rustls (requires source changes to Cargo.toml)
   ```

7. **Check for broken lockfile** — if `Cargo.lock` is stale or corrupted, delete it and regenerate:

   ```bash
   rm Cargo.lock
   cargo generate-lockfile
   cargo build

---

## Parked Agent Not Reviving

### Symptom

Agent does not respond to hub messages, despite having been completed.

### Root Cause

The file `~/.fathom/parked/<id>.json` is corrupted, deleted, or the reviver callback is not registered.

### Resolution

Delete the file manually and restart the session.

---

## Hub Messages Not Delivered

### Symptom

An agent is waiting for a reply from another agent, but the message is not delivered.

### Root Cause

The recipient is not registered in IrcBus, is parked, or the mailbox is full.

### Resolution

Check `fathom tui` — is the target agent visible in the tree? If not, it is parked and should be revived automatically.

Message priority: waiter (blocking wait) → agent channel → mailbox → reviver hook.

---

---

## Development / Testing

### SSRF Loopback Escape Hatch

By default, Fathom's SSRF guard (`crates/tools/src/guard.rs`) blocks all fetches to loopback and private-network addresses. This protects against server-side request forgery in production, but prevents integration tests and local development from reaching mock servers on `localhost`.

Set the environment variable to permit loopback fetches:

```bash
export PR_SSRF_ALLOW_LOOPBACK=1
```

| Variable | Value | Effect |
|----------|-------|--------|
| `PR_SSRF_ALLOW_LOOPBACK` | `1` | Permits `http://127.0.0.1`, `http://localhost`, and RFC 1918 addresses |
| unset / any other value | — | All loopback/private IPs rejected (default) |

**When to use:**

- Local development with a mock HTTP server (`127.0.0.1:8080`).
- Integration tests that need to verify fetch behaviour against a local service.

**Warning:** Never set this variable in production. It disables a critical security boundary. The constant `SSRF_LOOPBACK_ENV` is defined in `crates/tools/src/guard.rs` and the check `loopback_allowed_for_tests()` is evaluated on every fetch validation.