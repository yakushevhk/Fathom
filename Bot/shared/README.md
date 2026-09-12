# Shared Types and Contracts (`shared/`)

This directory houses cross-cutting TypeScript models, schemas, and utility functions shared between the Node.js / Hono backend (`server/`), the Electron desktop shell (`electron/`), and the React frontend (`src/`).

It contains zero runtime framework dependencies (no React, no Electron, no DOM-specific bindings), allowing safe isomorphic imports across browser, renderer, worker, and Node processes.

---

## Architectural Role & Data Flow

```
+-------------------------------------------------------------+
|                         shared/                             |
|  - Domain Types & Schemas                                   |
|  - Request Cards (Skills, Credentials, Routines)            |
|  - Approval Mode Hierarchy & Invariants                     |
|  - Serialization & Diff Utilities                           |
+-------------------------------------------------------------+
          ^                         ^                     ^
          |                         |                     |
+-------------------+     +-------------------+   +-------------------+
|      server/      |     |     electron/     |   |       src/        |
|  - Bot Execution  |     |  - Host IPC       |   |  - React UI       |
|  - Driver Layer   |     |  - Safe Storage   |   |  - Zustand Stores |
|  - Persistence    |     |  - Native Bridge  |   |  - Settings Views |
+-------------------+     +-------------------+   +-------------------+
```

1. **Agent Interaction Cards**: When an autonomous driver or LLM attempts to install a skill, request an API token, or schedule a routine, it emits structured card payloads defined in `shared/`.
2. **Permission Model Enforcement**: `shared/approval-mode.ts` governs execution rights (Ask, Edits, Auto, Full, Custom), mapped uniformly across various driver backends (Codex, Claude, Grok, Antigravity).
3. **Backup & State Portability**: Schema validation and snapshot serialization rules for workspace export/import and team state management are centralized here.

---

## Key Modules & Exported Contracts

### 1. Permission & Approval Models (`approval-mode.ts`)
Defines the bot authorization levels and provider support matrix:
- **`APPROVAL_MODES`**: Tuple `["ask", "edits", "auto", "full", "custom"]`.
- **`supportsApprovalMode(driverKind, mode)`**: Evaluates whether a driver backend supports the requested mode (e.g., `custom` is exclusive to Codex; `edits` applies to Claude, Grok, and Antigravity).
- **`approvalModeFor(bot)`**: Fail-closed resolver that maps legacy `autoApprove` boolean flags and server two-phase `approvalGrant` states safely back to `"ask"` until explicitly elevated.
- **`isEmergencyApprovalDowngrade(current, next)`**: Validates emergency downgrade transitions (`full`/`custom` -> `ask`) which can be applied even during an active turn.

### 2. Interactive Request Cards
Defines structured cards presented in transcripts requiring user confirmation:
- **`skill-request.ts`**:
  - `SkillRequestCardData`: Card state containing `botId`, `threadId`, `stagedId`, `sha256`, and preview markdown for dynamic skill installation/updates.
  - `reviewedSkillSha256(request)`: Ensures user approval binds strictly to the exact SHA-256 digest displayed in the UI.
  - `skillRequestBehavior(answer)`: Fail-closed parser mapping affirmative responses (`"enable"`, `"apply"`, `"confirm"`) to `"allow"`.
- **`credential-request.ts`**:
  - `CREDENTIAL_TARGETS`: Whitelist of allowable credential targets (`xaiApiKey`, `boxToken`, `opencodeGoApiKey`, `ttsKey`, `openaiImageApiKey`). Agents cannot invent arbitrary config keys.
  - `credentialConfigPatch(id, value)`: Transforms secret inputs into safe structured config patches.
- **`routine-request.ts` & `routine-run.ts`**:
  - Validates automated routines, recurrence intervals, trigger types, and manual run invocations.
- **`learn-request.ts` & `profile-request.ts`**:
  - Governs agent self-directed learning items and bot persona profile mutation prompts.

### 3. Workspaces & Team Backups
- **`workspace-backup.ts` & `workspace-backup-client.ts`**:
  - Data transfer formats and HTTP client endpoints for exporting, importing, and validating encrypted workspace bundles.
- **`team-backup.ts`**:
  - Encapsulates team-wide bot settings, routine configurations, and shared skill indices for collaboration workflows.

### 4. Utilities & Presentation
- **`line-diff.ts`**:
  - Fast, clean diffing implementation used across code reviews, file edit proposals, and agent mutation cards.
- **`mascot-bodies.ts`**:
  - 2D/3D procedural body geometry, signed distance fields (SDF), and avatar generation parameters shared between client canvas rendering and backend asset generators.
- **`bot-avatar.ts` & `image-generation.ts`**:
  - Shared specs for bot profile pictures, preset avatars, and prompt templates for image generation APIs.
- **`provider-safety.ts` & `connector-availability.ts`**:
  - Sanitization checks for third-party connector credentials and provider availability states.

---

## Invariants & Design Principles

1. **Zero Node/Browser Privileges**: Never import Node builtins (`fs`, `child_process`, `net`) or browser DOM APIs (`window`, `document`) into `shared/`.
2. **Fail-Closed Security**: Any unrecognized permission mode, corrupted credential target, or unexpected approval payload must fail closed to the safest state (`"ask"`, `"deny"`, or no-op).
3. **Exact Cryptographic Binding**: Card approvals that mutate executable code (e.g., `SkillRequestCardData`) bind the approval action to an immutable SHA-256 digest of the proposed file contents.
4. **Backward Compatibility**: Deserializers must maintain fallback paths for legacy configurations (such as boolean `autoApprove`) without silently escalating user permissions.

---

## Testing & Verification

Shared modules are verified through Vitest / Node test runners:
```sh
pnpm test shared/
```
Targeted unit tests:
- `shared/line-diff.test.ts`: Validates hunk parsing, insertion/deletion tracking, and line alignment.
- `shared/image-generation.test.ts`: Verifies image prompt constructors and provider options.
