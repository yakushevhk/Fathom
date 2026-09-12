# Frontend Core Libraries (`src/lib/`)

## 1. Directory Purpose & Scope

The `src/lib/` directory contains framework-agnostic client utilities, domain protocol handlers, browser-to-server synchronization primitives, media processing pipelines, and integration drivers. It bridges the React UI with backend APIs, system hardware (via the Electron preload bridge), and browser standards.

---

## 2. Functional Domains & Key Modules

```
+---------------------------------------------------------------------------------+
|                                    src/lib/                                     |
|                                                                                 |
|  +---------------------------+  +--------------------------+  +--------------+  |
|  |   Transport & Network     |  |   Transcript & Parsing   |  |  Media & UI  |  |
|  |  - live-events.ts (SSE)   |  |  - transcript-window.ts  |  |  - tts/      |  |
|  |  - session.ts (Auth/Pair) |  |  - composer-attach.ts    |  |  - haptics.ts|  |
|  |  - webhooks.ts            |  |  - markdown-tables.ts    |  |  - skins.ts  |  |
|  |  - mcp-servers.ts         |  |  - code-block.ts         |  |  - mascot.ts |  |
|  +---------------------------+  +--------------------------+  +--------------+  |
|                                                                                 |
|  +---------------------------+  +--------------------------+  +--------------+  |
|  |     Agent & Execution     |  |    Sidebar & Layout      |  | Localization |  |
|  |  - computer-control.ts    |  |  - sidebar-layout.ts     |  |  - i18n.ts   |  |
|  |  - local-computer.ts      |  |  - folder-order.ts       |  |              |  |
|  |  - routines.ts            |  |  - drafts.ts             |  |              |  |
|  |  - group-routing.ts       |  |  - bottom-follow.ts      |  |              |  |
|  +---------------------------+  +--------------------------+  +--------------+  |
+---------------------------------------------------------------------------------+
```

---

## 3. Deep Dive into Core Modules

### 3.1 Network & Streaming Protocols
- **`live-events.ts`**:
  - Implements a resilient supervisor for the `/api/events` SSE feed.
  - Monitors application-level heartbeat frames (`ping`) every 40s (`LIVE_EVENTS_STALE_MS`).
  - Automatically recovers half-open connections using exponential backoff with jitter (`500ms` to `10000ms`).
  - Tracks server replay cursors (`?since=...`) to prevent dropped messages across transient network cuts.
  - Exposes `onSnapshotRequired` callback when the server's event buffer has rolled past the client cursor.
- **`session.ts`**:
  - Manages authentication state detection: detects local loopback (zero-auth trusted) vs. remote sessions (cookie/token authenticated).
  - Handles `/pair` URL parameters (`#code=...` and `?email=...`), securely clearing sensitive tokens from browser history via `history.replaceState`.
- **`mcp-servers.ts`**:
  - Provides a typed client for Model Context Protocol (MCP) server status inspection, configuration fetching, and tool inventory checks.

### 3.2 Transcript Virtualization & Messaging
- **`transcript-window.ts`**:
  - Manages sliding-window virtualization for conversation histories to avoid mounting thousands of DOM nodes.
  - Standard window size: `TRANSCRIPT_WINDOW_SIZE = 60` messages, with a maximum mounted limit of `MAX_TRANSCRIPT_MOUNTED = 120`.
  - Supports bidirectional window expansion ("Show earlier" and "Show later") and clamped focal windows when deep-linking from search hits via `focusWindowRange`.
- **`composer-attachments.ts` & `image-compress.ts`**:
  - Handles multimodal file and image inputs (pasted screenshots, dropped files, large raw text pastes).
  - Automatically compresses oversized client images via canvas scaling and WebP/JPEG re-encoding before uploading.
  - Manages ephemeral blob preview URLs (`previewUrl`) with clean disposal to prevent browser memory leaks.
- **`drafts.ts`**:
  - Durable in-memory and local storage backup for unsent user drafts, active reply quotes, attachment stages, and failed-send retry queues.

### 3.3 Computer Control & Automation
- **`computer-control.ts`**:
  - Implements human-in-the-loop lease management for bot computer sessions.
  - Allows the operator to request "takeover" or release control back to the agent via `/api/bots/:id/computer/control`.
- **`local-computer.ts`**:
  - Inspects local environment capabilities (OS version, permissions, display servers like X11 or Wayland, accessibility grants) to determine if native desktop control is permissible.
- **`local-vm-workspace.ts`**:
  - Coordinates local virtual machine sandbox provisioning and aspect-ratio-fitted viewport calculations.

### 3.4 Multi-Agent Coordination & Group Routing
- **`group-routing.ts`**:
  - Implements conversation flow algorithms for shared team channels: evaluates `@mentions`, default responder policies (`member`, `everyone`, or `mentions`), and goal coordinator handoffs.
- **`mentions.ts`**:
  - Fuzzy matching engine for `@bot` and `@channel` handles in the composer.
- **`routines.ts` & `routine-calendar.ts`**:
  - Data structures, schedule parsing (Cron/Interval), and calendar projection logic for scheduled autonomous background routines.

### 3.5 Media, Haptics, & UI Enhancements
- **`tts/` & `local-voice.ts`**:
  - Client interface for ElevenLabs and native speech synthesis engines.
  - Manages audio playback queues, active utterance tracking (`useSpeech`), and push-to-talk microphone inputs.
- **`haptics.ts`**:
  - Tactile feedback router supporting native Electron Taptic Engine triggers, mobile browser `navigator.vibrate`, and fallback no-ops.
- **`skins.ts` & `brand.ts`**:
  - Dynamic runtime theming engine applying CSS custom property palettes and calculating contrast compliance.

---

## 4. Invariants & Reliability Rules

1. **Deterministic Session Extraction**:
   - `takePairingCodeFromLocation()` must consume the hash token immediately and clean browser history before the first network request, preventing token leakage in referrers or logs.
2. **Blob URL Lifecycle**:
   - Any object URL created via `URL.createObjectURL()` for attachments or voice previews must be registered and revoked (`URL.revokeObjectURL()`) when components unmount or previews settle.
3. **SSE Connection Exclusivity**:
   - Only one active `EventSource` connection to `/api/events` may exist per browser window. Reconnection routines must explicitly close and nullify stale instances before creating replacements.
4. **Draft Immutability on Failure**:
   - When a network send fails, the composer text and attachments must be preserved in the failed-send cache and never dropped.

---

## 5. Verification & Testing

All modules in `src/lib/` maintain strict unit test coverage using Vitest:
- Run all library tests:
  ```bash
  pnpm vitest run src/lib/
  ```
- Targeted domain suites:
  ```bash
  pnpm vitest run src/lib/live-events.test.ts
  pnpm vitest run src/lib/transcript-window.test.ts
  pnpm vitest run src/lib/composer-attachments.test.ts
  pnpm vitest run src/lib/drafts.test.ts
  pnpm vitest run src/lib/group-routing.test.ts
  ```
