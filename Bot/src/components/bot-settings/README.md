# Bot Settings Component Suite

The `bot-settings` directory contains the modular tabbed configuration dialog for customizing AI bot personas, model providers, execution environments, autonomy permissions, tool integrations, and persistent memories within Parallel.

---

## Directory Architecture

```text
src/components/bot-settings/
├── sections.ts                # Rail section metadata, icons, and search indexing
├── useBotSettingsDerived.ts   # Central derivation hook computing engine capabilities & patch actions
├── field.ts                   # Standardized form input classnames and field wrappers
├── PromptPreview.tsx          # System prompt inspection showing rendered soul & tool prompts
│
├── OverviewSection.tsx        # High-level summary, prompt preview, and capability badges
├── IdentitySection.tsx        # Name, title, avatar picker, blurb, color accents, and mascot state
├── SoulSection.tsx            # Standing instructions (soul.md), behavioral bounds, and persona rules
├── ModelSection.tsx           # Model engine provider, model identifier, and reasoning effort
├── PermissionsSection.tsx     # Autonomy approval levels, Chief of Staff delegation, peer comms
├── AccessSection.tsx          # Working directory (cwd), compute backend (local/cloud), MCP tools, browser
├── SkillsSection.tsx          # Procedural skills and learned capabilities
├── MemorySection.tsx          # Persistent long-term memory notes, recall topics, and memory clearing
├── RoutinesSection.tsx        # Scheduled automated tasks assigned to this bot
├── VoiceSection.tsx           # Text-to-speech voice models and notification alert preferences
├── HistorySection.tsx         # Audit log of profile changes and configuration rollback
└── UsageSection.tsx           # Token consumption, context costs, and session metrics
```

---

## State Management & Derivation Architecture

Bot settings consume state from the global reactive store (`useStore`) via a dedicated derivation hook: `useBotSettingsDerived(bot: Bot)`.

### Derivation Pipeline (`useBotSettingsDerived.ts`)
Instead of duplicating engine discovery and capability detection logic across individual UI sections, `useBotSettingsDerived` aggregates:
- **Engine Capabilities**: Inspects `state.instances` matching `bot.modelSelection.instanceId` to verify feature support (`agentsMcp`, `composioMcp`, `computerMcp`, `browserMcp`).
- **Compute Selectability**: Evaluates whether local desktop automation (`localSelectable`), remote VPS (`canUseVps`), or headless browser control (`browserSelectable`) are supported on the host platform.
- **Section & Chief of Staff Mapping**: Identifies the bot's organizational section and detects whether a peer bot already holds the Chief of Staff crown.
- **Atomic Patch Dispatch**: Exposes a unified `patch(p: BotPatch)` helper that emits `{ type: "updateBot", botId: bot.id, patch: p }`.

---

## Key Functional Sections

### 1. Access & Execution Environment (`AccessSection.tsx`)
Controls where and how the bot executes operations:
- **Working Directory (`cwd`)**: Sets the execution directory for shell and filesystem tools. New tasks start here; active tasks remain pinned to their initial thread directory to avoid breaking running CLI processes.
- **Compute Backend (`computer`)**:
  - `local`: Drives the host operating system directly (triggers `LocalComputerAutoWarning` if paired with autonomous execution).
  - `cloud`: Spins up an isolated per-bot Linux container desktop.
  - `vps`: Connects to an external server via SSH.
- **Tool Grants & MCP Servers**:
  - `mcpServers`: Selects specific Model Context Protocol (MCP) servers mounted for this bot. If unconfigured (`null`), mounts all globally enabled servers.
  - `composio`: Toggles access to external connected SaaS apps (GitHub, Slack, Google Calendar, etc.).
  - `browser`: Toggles the built-in Playwright/Puppeteer browser tool.
- **Always-Allowed Tool Grants (`alwaysAllow`)**: Displays standing grants authorizing specific shell commands or destructive operations without interactive approval.

### 2. Autonomy & Permissions (`PermissionsSection.tsx`)
Controls autonomy gates and multi-agent coordination:
- **Approval Level (`approvalMode`)**:
  - `ask`: Every shell command and tool execution requires interactive human confirmation.
  - `auto`: Autonomous execution for safe commands; prompts only for privileged operations.
  - `full`: Complete autonomous execution without interactive confirmation prompts. Requires host platform support (`trustedModesAvailable`).
  - `custom`: Fine-grained allow/deny rules per tool category.
- **Chief of Staff (`chiefOfStaff`)**:
  - Designates this bot as the primary team leader for its assigned organizational section.
  - Requires multi-agent coordination capability (`engine.capabilities.agentsMcp === true`).
  - Can spawn sub-tasks, delegate work to specialized bots, and synthesize results into a final report.
- **Peer Communications (`approvePeerComms`)**:
  - When enabled, the bot must prompt for confirmation before sending cross-bot messages to teammates.

### 3. Model & Reasoning Selection (`ModelSection.tsx`)
- **Default Model**: Selects the underlying LLM provider instance (`claude-code`, `codex`, `parallel`, `openai-compat`, etc.) and specific model ID.
- **Effort Setting (`bot.modelSelection.effort`)**: Configures reasoning effort/budget (e.g., low, medium, high reasoning tokens) for supported reasoning models.

### 4. Identity & Soul (`IdentitySection.tsx`, `SoulSection.tsx`)
- **Identity**: Configures visual branding, mascot expression resting state (`happy`, `curious`, `searching`, etc.), and display bio.
- **Soul (`soul.md`)**: Standing operational guidelines and ethical guardrails injected into the system prompt across every session. The byte size is capped to prevent exhausting the context window.

### 5. Memory & Context Inspection (`MemorySection.tsx`, `PromptPreview.tsx`)
- **Memory**: Displays indexed persistent memories, user preferences, and fact stores extracted during previous interactions. Allows selective deletion or manual topic additions.
- **Prompt Preview**: Live diagnostic modal rendering the exact system prompt that the agent will see, combining base personality, soul guidelines, active tool definitions, and contextual attachments.

---

## Form Contracts & Security Rules

1. **Direct Path Validation**:
   Working directory changes are dispatched via direct HTTP `PATCH /api/bots/:id` to ensure the server validates path existence and permissions prior to local state commitment.
2. **Warning Interceptors**:
   - `LocalComputerAutoWarning`: Prevents switching execution mode to `local` while approval is set to `auto` or `full` without explicit user confirmation of host safety implications.
   - `FullAccessWarning`: Demands explicit acknowledgment before enabling unrestrained autonomous execution.
3. **Safety Isolation**:
   When switching bot tabs or closing the dialog, active modal warnings tie strictly to `bot.id` and never bleed into subsequent selections.

---

## Verification & Testing

Run the bot settings unit tests:
```sh
npm test src/components/bot-settings/
```

Specific test suites:
- `OverviewSection.test.ts`: Validates summary calculations and badge rendering.
- `PermissionsSection.tsx`: Tested via approval mode state machines in `shared/approval-mode.ts`.
- `AccessSection.test.ts`: Verifies working folder validation and MCP server list diffing.
- `MemorySection.test.ts`: Validates memory item removal and topic mutations.
- `PromptPreview.test.ts`: Ensures soul and system prompt formatting matches server injection specs.
