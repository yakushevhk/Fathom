# Agent Client Protocol Drivers (`server/drivers/acp/`)

## 1. Purpose and Scope

The `server/drivers/acp/` directory implements drivers conforming to the **Agent Client Protocol (ACP)**—a standardized JSON-RPC 2.0 protocol executed over subprocess `stdio`. 

Rather than duplicating process spawning, framing, notification handling, and permission validation for every agent CLI, this directory implements a clean architectural split:
1. **Generic Protocol Core (`core.ts`)**: Contains all JSON-RPC 2.0 connection management, message serialization, request/response tracking, error mapping, and event normalization.
2. **Per-Harness Support Modules**: Specialized descriptors implementing `AcpSupport` to define CLI arguments, binary discovery, model catalogs, authentication checks, and environment configurations for specific agents.

---

## 2. Architecture and Data Flow

```
+-------------------------------------------------------------+
|             ACP Driver Definition (e.g. GrokAgentDriver)    |
|                     createAcpDriver(support)                |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                    AcpSessionRuntime (core.ts)              |
|  - Spawns CLI child process (spawnCli)                      |
|  - JSON-RPC 2.0 line-delimited stdio transport              |
|  - Dispatches methods & routes asynchronous notifications   |
+------------------------------+------------------------------+
                               |
              Standard Input / Output (JSON-RPC 2.0)
                               v
+-------------------------------------------------------------+
|                  Agent Subprocess (e.g. grok CLI)           |
|                                                             |
|   Client -> Server (Harness -> CLI):                        |
|     - initialize                                            |
|     - session/new                                           |
|     - session/prompt                                        |
|                                                             |
|   Server -> Client (CLI -> Harness Notifications):          |
|     - session/update (tokens, tool execution, output)       |
|     - session/request_permission (approval challenges)      |
+-------------------------------------------------------------+
```

### Protocol Dynamics
1. **Initialization**: On turn dispatch, `AcpSessionRuntime` launches the agent CLI with `support.spawnArgs(...)` and sends an `initialize` JSON-RPC request to negotiate protocol capabilities.
2. **Session Creation / Load**: If a resume cursor exists, it attempts `session/load`; otherwise, it sends `session/new`.
3. **Turn Execution (`session/prompt`)**:
   - The user's input, attachments, and turn directives are submitted via `session/prompt`.
   - The agent streams execution progress via incoming `session/update` notifications (`assistant_text`, `reasoning_text`, `tool_call`).
   - The completion of the turn is signaled by the `session/prompt` **RPC response result** (carrying final `stopReason` and token `usage`). ACP does not emit an explicit `turn/completed` notification.
4. **History Replay Protection**: When a session is resumed, agents may replay historical updates with `_meta.isReplay: true`. The runtime gates emissions so that no events are emitted prior to the active prompt and replay updates are filtered out.

---

## 3. Key Files and Harness Implementations

### Protocol Core
- **`core.ts`**: The canonical ACP driver engine. Exports `createAcpDriver(support: AcpSupport): ProviderDriver<AcpConfig>`, providing full lifecycle management, stdio parsing, error handling, permission resolution, and event mapping.

### Supported Agent Harnesses
- **`grok.ts`**: Official `grok` CLI agent (`grok agent stdio`) using grok.com subscription authentication (`~/.grok/auth.json`), local model discovery from `~/.grok/config.toml`, and image capability negotiation.
- **`gemini.ts`**: Google Gemini CLI agent integration, handling OAuth token validation and model routing.
- **`kimi.ts`**: Kimi CLI agent integration for Moonshot AI models.
- **`droid.ts`**: Integration for the Droid agent runtime.
- **`cursor.ts`**: Adapter for Cursor's background agent CLI.
- **`opencode-go.ts`**: OpenCode Go agent runner, adapting model selection via in-session configuration rather than CLI flags.
- **`qwen.ts`**: Alibaba Qwen CLI harness integration.
- **`hermes.ts`**: Hermes agent integration.
- **`custom.ts`**: Configurable ACP driver allowing users to attach arbitrary third-party executables supporting the ACP JSON-RPC standard.

---

## 4. Permission and Approval Security

Interactive tools executed by ACP agents must pass through the harness permission pipeline:

1. **`session/request_permission` Handling**:
   - When the agent CLI requires approval to execute a shell command, read sensitive files, or access external APIs, it sends a `session/request_permission` RPC call to the harness.
   - `core.ts` intercepts this request, formats an `OptionCardData` approval request, and emits a canonical `request.opened` runtime event.
2. **Fail-Closed Security Invariant**:
   - The request resolves only if the user explicitly confirms the action.
   - Decisions are evaluated fail-closed: options must explicitly specify an `allow` kind to be granted.
   - Option order from the agent is never treated as a semantic hint.
3. **Approval Modes**: Respects the configured `ApprovalMode` (`ask`, `auto`, `prompt`). In `auto` mode, decisions are vetted by `autoVerdict()` before bypass.

---

## 5. Adding a New ACP Harness

To integrate an agent that supports ACP over stdio:

1. Create a new definition file in `server/drivers/acp/<name>.ts`.
2. Define an `AcpSupport` specification object:
   ```typescript
   export const MyAgentSupport: AcpSupport = {
     driverKind: "myagent",
     displayName: "My Agent",
     defaultCli: "myagent",
     nativeSource: "myagent.acp",
     models: {
       default: "model-name",
       options: [{ id: "model-name", label: "Model Name" }],
     },
     loginNote: "Run `myagent login` in your terminal to sign in.",
     spawnArgs(config, turn) {
       return ["agent", "stdio", "--model", turn.model];
     },
   };

   export const MyAgentDriver = createAcpDriver(MyAgentSupport);
   ```
3. Register the driver in `server/drivers/builtIn.ts`.

---

## 6. Verification and Testing
ACP drivers and core protocol dynamics are validated via Vitest unit and approval tests:

```bash
# Test ACP core logic and driver implementations
pnpm vitest run server/drivers/acp/

# Run ACP approval matrix tests
pnpm vitest run server/drivers/acp/approval-matrix.test.ts
```
