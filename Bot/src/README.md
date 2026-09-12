# Frontend Root Architecture (`src/`)

## 1. Directory Purpose & Scope

The `src/` directory contains the client-side single-page application (SPA) for the desktop and browser environments of Parallel (Parallel). Built on **React 19**, **TypeScript 5.8**, and **Vite 7**, it provides the primary operator interface for configuring, orchestrating, and observing autonomous AI agents, multi-bot team channels, computer-use environments, and scheduled background routines.

The frontend operates in two runtime contexts:
1. **Electron Desktop Shell**: Executes inside an Electron Chromium renderer process with access to the `window.ogb` desktop preload bridge (native system window controls, auto-updater, local screen capture, native view overlays, CUA driver integrations).
2. **Standard Web Browser / PWA**: Interacts with the backend server via HTTP REST and Server-Sent Events (SSE), supporting mobile and remote pairing workflows via `/pair`.

---

## 2. Architectural Role & High-Level Data Flow

```
+-------------------------------------------------------------------------+
|                              Electron Shell / Web Browser               |
|                                                                         |
|  +-------------------------------------------------------------------+  |
|  | Entry: main.tsx                                                   |  |
|  |  - Skin & Brand Bootstrap (applySkin, bootstrapBrand)             |  |
|  |  - Auth / Pairing Resolution (chooseRoot -> PairPage vs App)      |  |
|  |  - PWA Service Worker Registration                                |  |
|  +----------------------------------+--------------------------------+  |
|                                     |                                   |
|                                     v                                   |
|  +-------------------------------------------------------------------+  |
|  | Shell Architecture: App.tsx                                       |  |
|  |  +-------------------------------------------------------------+  |  |
|  |  | Providers:                                                  |  |  |
|  |  |   StoreProvider (State + SSE listener + Action Dispatch)    |  |  |
|  |  |   DesktopCapabilitiesProvider (Electron IPC bridge context) |  |  |
|  |  |   ThreadRefsProvider (Cross-thread link routing)            |  |  |
|  |  +-------------------------------------------------------------+  |  |
|  |  | Main Layout:                                                |  |  |
|  |  |   - Sidebar (Navigation, Bot/Channel tree, Sections)       |  |  |
|  |  |   - Viewport: ChatView | GroupView | SplitWorkspace         |  |  |
|  |  |               RoutinesPage | TeamMapPage                    |  |  |
|  |  |   - Docked Panels: ComputerPanel, RemoteDesktopPanel        |  |  |
|  |  |   - Overlays: CommandPalette, InspectorPanel, Settings      |  |  |
|  |  +-------------------------------------------------------------+  |  |
|  +----------------------------------+--------------------------------+  |
|                                     |                                   |
|                                     v                                   |
|  +-------------------------------------------------------------------+  |
|  | State & Communication:                                            |  |
|  |   - SSE Stream: /api/events (live-events.ts supervisor)           |  |
|  |   - HTTP REST: /api/bots, /api/groups, /api/config, etc.          |  |
|  |   - Delta Buffer: Token streaming batching via rAF                |  |
|  |   - Patch Queue: Debounced bot profile persistence                |  |
|  +-------------------------------------------------------------------+  |
+-------------------------------------------------------------------------+
```

### Data Flow Lifecycle
1. **Pre-Paint Bootstrapping**:
   - `applySkin(readSkin())` runs synchronously before DOM rendering to prevent dark/light theme flash.
   - `bootstrapBrand()` fetches remote brand labels, window title, and accent color.
   - `readSessionState()` checks authentication: loopback requests (local machine) succeed immediately; remote sessions require an authenticated session cookie or redirect to `/pair`.
2. **State Hydration & Event Ingestion**:
   - `StoreProvider` initializes a pure `useReducer` state store.
   - `openLiveEvents()` connects an SSE stream to `/api/events` with resume cursors (`?since=...`).
   - Server pushes snapshots and mutations: `snapshot`, `bot_patch`, `group_patch`, `task_switched`, `runtime` (assistant and reasoning token deltas), and `screen_frame`.
3. **Optimistic Updates & Reconciliations**:
   - Outgoing user messages are injected optimistically into state (`optimistic-${sendId}`).
   - Fast user-driven bot edits go through `bot-patch-queue.ts`, keeping local UI responsive while coalescing PATCH requests.
   - Token deltas pass through `createStreamDeltaBuffer` which throttles re-renders to `requestAnimationFrame` ticks.

---

## 3. Key Files & Exports

| File | Type / Responsibility | Key Functions / Components |
|---|---|---|
| `main.tsx` | Application entry point | `chooseRoot()`, root React 19 mount with `<StrictMode>` |
| `App.tsx` | Root shell layout & provider container | `App()`, `Shell()`, global shortcuts listener, window events router |
| `styles.css` | Core styling & CSS token definitions | Tailwind CSS imports, color custom properties, CSS animations (`msg-in`) |
| `onboarding-preview.tsx` | Standalone preview harness for onboarding | Isolated dev harness for verifying welcome and setup cards |
| `mascot-preview.tsx` | Standalone preview harness for agent avatars | Renders mascot body SVGs, color schemes, and expressions |
| `mascot-preview.css` | Styles for mascot previewing | Animation and backdrop styling for mascot testing |

---

## 4. Subdirectory Structure & Architecture

- **`components/`**: React UI components including `ChatView`, `Composer`, `Sidebar`, `ComputerPanel`, `SplitWorkspace`, and auxiliary dialogs/modals.
- **`state/`**: Global state management (`store.tsx`), reducer, bot patch queues (`bot-patch-queue.ts`), and unit tests.
- **`lib/`**: Domain logic libraries (network protocols, SSE management, transcript windowing, attachments, draft storage, computer control, audio/TTS, i18n).
- **`hooks/`**: Specialized React hooks, notably `useNativeViewObscured` for managing Electron WebContentsView occlusion boundaries.
- **`pair/`**: Mobile and remote pairing interface (`PairPage.tsx`) for zero-friction browser-to-server linking.
- **`locales/`**: Multi-language translation dictionaries and i18n definitions (English, Chinese, German, Spanish, French, Hindi, Japanese, Portuguese).
- **`types/`**: Ambient TypeScript declarations for the desktop bridge (`ogb.d.ts`).

---

## 5. Invariants & Security Rules

1. **Pre-Paint Integrity**:
   - Never render UI components before skin and brand bootstrap complete. Doing so causes visible flash of unstyled content (FOUC).
2. **Loopback vs. Remote Authentication**:
   - Direct loopback connections (`localhost`, `127.0.0.1`, `::1`) are trusted by the local backend server without credentials.
   - Non-loopback requests must present valid session credentials. If unauthenticated, `chooseRoot()` in `main.tsx` MUST halt execution of `<App />` and route exclusively to `<PairPage />`.
3. **Electron WebContentsView Z-Index Isolation**:
   - Native WebContentsViews (used for live computer sessions and CUA integration) paint above the React DOM stack regardless of CSS `z-index`.
   - Modals, drawers, and floating panels must register through `useNativeViewObscured` or overlay state flags to hide or pause native views during interaction.
4. **Token Streaming Performance**:
   - Raw SSE token deltas must NEVER trigger direct `setState` on every incoming frame. All token stream mutations must route through `createStreamDeltaBuffer` to prevent catastrophic frame-rate drops.

---

## 6. Testing & Verification

- **Linting**:
  ```bash
  pnpm lint
  ```
- **Type Checking**:
  ```bash
  pnpm typecheck
  ```
- **Unit & Integration Tests**:
  ```bash
  pnpm test
  ```
  Runs Vitest across unit test suites in `src/state/`, `src/lib/`, and `src/hooks/`.
- **Contrast & Theme Verification**:
  ```bash
  pnpm check:contrast
  ```
- **Internationalization Consistency**:
  ```bash
  pnpm i18n:check
  ```
