# Electron Shell Architecture (`electron/`)

This directory contains the Electron main process, preload scripts, native bridges, child process supervisors, and OS integration layers for the desktop distribution.

---

## Architectural Role & Process Topology

The desktop application operates across multiple process boundaries to isolate privileged OS access, secure key storage, native drivers, and background servers from the unprivileged renderer:

```
+-----------------------------------------------------------------------------------+
|                              Electron Main Process                                |
|                                (`electron/main.mjs`)                              |
|  - Window Lifecycle & Native Chrome        - OS SafeStorage Credential Vault      |
|  - Server Process Supervisor               - Application Menu & Tray Navigation   |
|  - Native Auto-Updater Coordinator         - CUA Driver Daemon Host               |
+-----------------------------------------------------------------------------------+
       |                    |                       |                     |
 (Context Isolation)   (Process Lifecycle)    (stdio / Socket)      (Private Unix Sock)
       v                    v                       v                     v
+--------------+   +-------------------+   +------------------+   +------------------+
| Renderer UI  |   | Embedded Server   |   | Cloudflared      |   | CUA Native Host  |
| (`preload.cjs|   | (utilityProcess)  |   | Guardian         |   | (Embedded Host   |
|   window.ogb)|   | (Hono / Node API) |   | (Tunnel Daemon)  |   |   or Standalone) |
+--------------+   +-------------------+   +------------------+   +------------------+
```

---

## Process Lifecycle & Core Subsystems

### 1. Main Entrypoint & Window Management (`main.mjs`, `window-chrome.mjs`, `window-state.cjs`)
- **Single Instance Enforcement**: Uses Chromium single-instance locks (`single-instance.mjs`) to focus the primary window when a secondary instance or deep link launches.
- **Window State Preservation**: Atomic debounced state writes (`window-state.json`) with minimum bounding constraints (`MIN_BOUNDS`) ensure the application re-opens at predictable screen coordinates across restarts or display changes.
- **Window Chrome**: Custom frameless window decoration (`window-chrome.mjs`) supporting dark/light theme switching, titlebar drag regions, and dynamic unread badge overlays.

### 2. Server Process Supervisor (`server-supervisor.mjs`, `server-boot-probe.mjs`)
- Spawns the embedded backend server as an Electron `utilityProcess` or child process.
- **Boot Probing**: Probes loopback health endpoints (`/healthz` or server identity probes) with exponential backoff before displaying the UI.
- **Auto-Recovery**: If the backend crashes during runtime, `createServerSupervisor` executes bounded restart attempts (`retryDelaysMs: [1_000, 2_000, 4_000]`) on the established port without tearing down the renderer window.

### 3. IPC & Renderer Isolation (`preload.cjs`, `local-origin.cjs`)
- **Strict Context Isolation**: `contextIsolation: true` and `nodeIntegration: false` are enforced across all views.
- **Bridge Exposure (`window.ogb`)**: Only a well-defined bridge object is exposed to the renderer.
- **Origin Scoping (`local-origin.cjs`)**: Sensitive operations (screen capture, local file paths, CUA controls, credential modification) are restricted to the local origin (`http://127.0.0.1:<port>`). Remote web pages or companion viewer instances receive a minimal `REMOTE_SAFE` subset.

### 4. Computer-Use Agent (CUA) Host (`cua.mjs`, `cua-linux-runtime.cjs`, `cua-connection.cjs`)
- Manages the local computer-use daemon bridging agent commands to native OS inputs:
  - **Embedded Mode (Packaged App)**: Launches an in-process native driver host (`EmbeddedCuaDriverHost`) under the app's bundle ID (`com.parallel.app`) so OS Accessibility/TCC prompts attribute cleanly to the application.
  - **Standalone Mode (Development)**: Connects via Unix domain socket to an existing `CuaDriver.app` daemon.
  - **Connection Descriptor**: Emits connection metadata to `<userData>/cua-connection.json` consumed by backend drivers.
  - **Linux Subsystem**: Coordinates X11/Wayland input proxies and AppImage bundle staging (`cua-linux-bundle.cjs`).

### 5. Secure Credential Vault (`secure-credentials.mjs`, `secure-credential-state.mjs`)
- Leverages Electron's native `safeStorage` API to encrypt secrets at rest using OS-backed keychains (macOS Keychain, Windows DPAPI, Linux Secret Service / libsecret).
- Provides atomic fallback mechanisms and migration pipelines (`workspace-credentials.mjs`) when migrating from plaintext legacy formats.

### 6. Companion Tunnel & Remote Access (`managed-companion-tunnel.mjs`, `managed-companion-guardian.mjs`)
- Supervises the embedded `cloudflared` tunnel binary to expose the local companion server over an end-to-end encrypted Cloudflare Tunnel.
- Validates binary checksums, monitors tunnel health, isolates cloud connector tokens in OS secure storage, and fences operations with local watchdog guards.

### 7. Native Auto-Updates (`updater.mjs`, `updater-coordinator.mjs`)
- Coordinates release downloads and installation handoffs across platforms.
- Manages squirrel/AppImage/macOS native updater state machines, update-ready notifications, and graceful background staging.

---

## Security Model & Invariants

1. **Origin Verification**: Every IPC handler handling filesystem, process spawning, or security operations verifies `event.senderFrame.url` matches the trusted loopback server.
2. **Never Forward Raw Keys**: Account bearer tokens, Composio master keys, and tunnel connector tokens are held strictly in the main process and encrypted via `safeStorage`; they are never transmitted across the renderer preload bridge.
3. **No Dynamic Code Evaluation**: Navigation is locked down; external URLs are strictly passed to the OS default browser via `shell.openExternal` after URL scheme validation.
4. **Display Media Intent**: Screen capture requires explicit user interaction through `beginScreenPreviewIntent`, preventing background clickjacking of desktop frames.

---

## Testing & Verification

Run the test suite covering Electron helper logic and node-compatible modules:
```sh
pnpm test:electron
```
Specific unit tests:
- `electron/server-supervisor.node-test.mjs`: Validates crash detection, retry backoff, and exhaustion hooks.
- `electron/window-state.test.mjs`: Tests bounds resolution, display boundary clamping, and parse recovery.
- `electron/secure-credentials.test.mjs`: Tests safeStorage encryption wrappers and mock fallbacks.
- `electron/managed-companion-tunnel.test.mjs`: Tests tunnel startup arguments, binary path resolution, and exit guards.
