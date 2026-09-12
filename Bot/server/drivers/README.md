# Provider Drivers (`server/drivers/`)

## 1. Purpose and Scope

The `server/drivers/` directory implements concrete provider adapters that interface Parallel's canonical harness (`server/contracts.ts`) with upstream AI engines, agent CLIs, and protocol runtimes. Each driver translates high-level turn dispatch instructions (`SendTurnInput`) into the CLI invocations, stdio streams, or HTTP calls required by specific AI systems, and normalizes disparate provider responses into standard `RuntimeEvent` streams.

Supported providers include:
- **CLI-based Agents**: Claude Code (`claude.ts`), Codex (`codex.ts`), Antigravity (`antigravity.ts`), Pi (`pi.ts`), BoxAgent (`boxagent.ts`).
- **Direct Model APIs**: Grok / xAI API (`grok.ts`), OpenAI-compatible endpoints (`openai-compat.ts`), Minimax (`minimax.ts`).
- **ACP (Agent Client Protocol)**: Multi-agent harnesses running via JSON-RPC 2.0 over stdio (`acp/`).
- **Local / Injected Endpoints**: Proxies for routing requests to loopback models and developer servers (`local-inject.ts`, `phone-proxy.ts`, `dweb-proxy.ts`).

---

## 2. Architectural Role and Lifecycle

All drivers implement the `ProviderDriver` contract defined in `server/contracts.ts`:

```typescript
export interface ProviderDriver<Config = unknown> {
  readonly driverKind: DriverKind;
  readonly metadata: DriverMetadata;
  readonly install?: EngineInstall;
  defaultConfig(): Config;
  decodeConfig(raw: unknown): Config;
  create(input: DriverCreateInput<Config>): Promise<ProviderInstance>;
}
```

```
+-------------------------------------------------------------+
|               ProviderRegistry (harness/registry.ts)        |
+------------------------------+------------------------------+
                               | creates via create()
                               v
+-------------------------------------------------------------+
|                ProviderInstance (contracts.ts)              |
|  - instanceId: InstanceId                                   |
|  - driverKind: DriverKind                                   |
|  - adapter: ProviderAdapter                                 |
+------------------------------+------------------------------+
                               | dispatches sendTurn()
                               v
+-------------------------------------------------------------+
|                  Specific Driver Implementation             |
|          (e.g., ClaudeDriver, CodexDriver, GrokDriver)       |
+------------------------------+------------------------------+
                               | spawns / streams
                               v
+-------------------------------------------------------------+
|               Subprocess / Transport Boundary               |
|      (Claude CLI stdio, ACP JSON-RPC, REST SSE Streams)     |
+-------------------------------------------------------------+
```

### Driver Instantiation and Turn Execution
1. **Registration**: Drivers are statically registered in `server/drivers/builtIn.ts` (`BUILT_IN_DRIVERS`).
2. **Creation**: `registry.load(configs)` matches configured instances by `driverKind` and invokes `driver.create()`, producing a `ProviderInstance` containing a `ProviderAdapter`.
3. **Turn Invocations**: The harness calls `adapter.sendTurn(input, listener)`.
4. **Event Normalization**: The driver streams native events, parses JSON lines or SSE frames, and invokes `listener(event)` with canonical `RuntimeEvent` objects (`turn.started`, `content.delta`, `item.started`, `turn.completed`).
5. **Session Continuation**: Drivers persist conversation continuity across turns using resume tokens or session IDs (e.g. `claude --resume <sessionId>`).

---

## 3. Key Files and Subsystems

- **`builtIn.ts`**: Static registry array export (`BUILT_IN_DRIVERS`) compiling all available built-in provider drivers.
- **`claude.ts`**: Integration with Anthropic's Claude Code CLI. Runs per-turn CLI child processes using `--resume <sessionId>` and JSON streaming over stdio. Mounts Composio sessions and cloud computer execution bridges as MCP tools.
- **`codex.ts`**: Driver for the Codex CLI harness, supporting auth session synchronization, instruction files, and model catalog resolution.
- **`grok.ts`**: Direct REST/SSE integration with xAI's native Grok API endpoints.
- **`antigravity.ts`**: Specialized driver for internal Antigravity agent environments, managing custom runtime environments and release lifecycles.
- **`pi.ts`**: Integration for Pi agent frameworks with MCP tool extension injection (`pi-mcp-extension.ts`).
- **`openai-compat.ts`**: Universal adapter for third-party OpenAI-compatible REST endpoints (Ollama, vLLM, DeepSeek, LocalAI).
- **`minimax.ts`**: MiniMax model driver handling custom streaming responses and token tracking.
- **`retry.ts`**: Exponential backoff and retry policy engine (`classifyError`, `computeBackoff`) determining which provider failures warrant re-dispatch.
- **`local-inject.ts`**: Intercepts requests targeting special `host::model` identifier patterns and routes them to local or tunneled HTTP endpoints.
- **`phone-proxy.ts` & `dweb-proxy.ts`**: Proxies bridging companion devices and decentralized web endpoints into agent tool trees.

---

## 4. Driver Communication Protocols

Drivers in this directory operate across four primary transport protocols:

### 1. Per-Turn Stdio JSON Streams (Claude / Codex)
- The server spawns a child CLI process per turn (`spawnCli`).
- The user prompt and context are piped through standard input.
- Standard output emits structured JSON objects (NDJSON).
- Process exit or a terminal `result` event signals completion.
- Conversation continuity relies on an engine session ID passed to subsequent CLI invocations (`--resume <sessionId>`).

### 2. ACP JSON-RPC 2.0 over Stdio (`server/drivers/acp/`)
- Long-running or per-turn agent child processes communicating via JSON-RPC 2.0.
- Handled through a shared protocol core (`acp/core.ts`).
- Method calls include `initialize`, `session/new`, `session/prompt`, and `session/request_permission`.

### 3. Native REST / SSE (Grok / OpenAI-Compat / Minimax)
- Direct HTTP requests using standard fetch/streaming APIs.
- Streams responses using Server-Sent Events (`text/event-stream`).
- Handles rate limits (HTTP 429) and quota exhaustion natively.

### 4. Local Injected Endpoints (`local-inject.ts`)
- Virtual driver mappings that divert model requests to loopback servers or custom developer ports.
- Bypasses standard subscription and cloud credential validation to enable local offline debugging.

---

## 5. Security Invariants and Sandbox Isolation

- **Child Process Environment Pruning**: CLI drivers inherit sanitized environments. Workspace-sensitive variables (`AWS_*`, `GITHUB_TOKEN`, `OPENMAUS_SESSION_*`) are stripped using `stripWorkspaceCredentialEnv()` to prevent untrusted agent tools from leaking credentials.
- **Fail-Closed Permission Handling**: When a driver surfaces an interactive confirmation or permission request (`request.opened`), execution halts until explicitly approved. Unrecognized or ambiguous requests default to rejection.
- **Safe Process Termination**: Subprocess termination uses `killCliTree()` to kill the entire process tree rather than orphaning background child workers.
- **Model Selection Validation**: Untrusted client-supplied model and effort levels are verified via `isEffortLevel()` and checked against driver-supported options before execution.

---

## 6. Verification and Testing
Driver unit tests verify event transformation, session recovery, and retry behavior using Vitest:

```bash
# Run all driver tests
pnpm vitest run server/drivers/

# Test specific drivers
pnpm vitest run server/drivers/claude.test.ts
pnpm vitest run server/drivers/retry.test.ts
pnpm vitest run server/drivers/openai-compat.test.ts
```
