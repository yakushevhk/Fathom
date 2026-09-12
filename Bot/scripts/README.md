# Engineering & Build Scripts (`scripts/`)

This directory houses development automation, packaging pipelines, CI smoke tests, integration verification rigs, and code generation scripts.

---

## Script Categories & Directory Overview

```
scripts/
|-- setup-router-agents.mjs       # Multi-agent LLM router & config generator
|-- mcp-server.ts                 # Local Model Context Protocol harness server
|-- bundle-server.mjs             # Bundles backend server into standalone runtime
|-- prepare-browser.mjs           # Vendor browser binaries (Chromium/Firefox)
|-- prepare-cloudflared.mjs       # Downloads & checksums platform cloudflared binaries
|-- cua-*                         # Computer-Use Agent build & packaging toolchain
|-- smoke-*                       # Headless smoke test runs for Linux/macOS/desktop
|-- verify-*                      # End-to-end subsystem verification scripts
+-- testing/                      # UI previews, sandboxes, and headless test rigs
```

---

## Key Tools & Pipelines

### 1. Agent & Router Setup (`setup-router-agents.mjs`)
Automates the local configuration of unified AI router endpoints (`router.y7.hk`) for 6 major developer agents:
- **Agents Covered**: `opencode`, `grok`, `claude`, `qwen`, `hermes`, `pi`, and `parallel`.
- **Functionality**: Writes API keys, base URLs, and curated fallback model hierarchies into standard dotfile paths (`~/.config/opencode/`, `~/.claude/settings.json`, etc.) ensuring consistent multi-model development environments.

### 2. Packaging & Binary Bundling
- **`bundle-server.mjs`**: Compiles the TypeScript Hono backend into a single self-contained JavaScript bundle embedded in the Electron production app.
- **`after-pack.mjs`**: Electron Builder post-packaging hook. Sets up executable permissions, strips debug symbols, and verifies directory structures.
- **`prepare-cloudflared.mjs`**: Fetches, validates, and stages architecture-specific `cloudflared` binaries for Darwin, Linux, and Windows packaging.
- **`prepare-browser.mjs` / `build-windows-browser-vendor.mjs`**: Manages dedicated headless browser vendor packages required for web extraction and live browsing drivers.
- **`patch-appimage-updater.mjs` & `patch-mac-updater.mjs`**: Injects delta update and feed handling patches into downstream updater dependencies.

### 3. Computer-Use Agent (CUA) Toolchain
- **`prepare-cua.mjs` & `prepare-cua-linux.mjs`**: Prepares native CUA driver dependencies for target architectures.
- **`cua-linux-release.mjs`**: Packages Linux CUA artifacts into AppImage bundles with isolated X11 input shims.
- **`generate-cua-sbom.mjs`**: Generates a Software Bill of Materials (SBOM) for CUA binary releases.

### 4. Verification & E2E Validation Rigs (`verify-*`)
Automated scripts that start sandboxed instances of the app or backend to assert component behaviors:
- **`verify-server-recovery.mjs`**: Tests backend supervisor crash handling and re-connection behavior.
- **`verify-workspace-backup.ts`**: Verifies workspace export, encryption, and import cycle fidelity.
- **`verify-routines.ts`**: Tests recurring task scheduling, cron triggers, and background execution.
- **`verify-team-backup.ts`**: Validates team backup serialization and deserialization.
- **`verify-bot-settings.ts`, `verify-sidebar.ts`, `verify-threads.ts`**: Headless DOM and state verification for specific UI panels.

### 5. Headless Smoke Tests (`smoke-*`)
Used across GitHub Actions / Linux build machines to prevent regressions:
- **`smoke-linux-package.mjs`**: Installs generated `.deb` and AppImage packages in a headless container, validating dependencies and startup.
- **`smoke-cua.mjs` & `smoke-cua-x11-input.mjs`**: Validates simulated input dispatching across display environments.
- **`smoke-approval-modes.cjs`**: Exercises the two-phase approval permission upgrade and downgrade paths.

### 6. Interactive Testing & UI Previews (`scripts/testing/`)
- Contains lightweight harness entrypoints (`cloud-preview.tsx`, `engines-preview.tsx`, `sidebar-preview.tsx`) used to develop and inspect complex UI components in isolation without launching the full desktop shell.
- Includes `firefox-sandbox.py` for headless browser sandbox isolation tests.

---

## Common Developer Workflows

### Configuring Local AI Agents
Run the router setup script to provision credentials:
```sh
node scripts/setup-router-agents.mjs
```

### Running Packaged Server Verification
Verify packaged server startup and port binding:
```sh
node scripts/smoke-packaged-server.mjs
```

### Auditing Skin & Color Contrast
Verify accessibility compliance of all app skins:
```sh
node scripts/check-skin-contrast.mjs
```
