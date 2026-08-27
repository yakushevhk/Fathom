# Fathom Desktop Client

Tauri v2 desktop client for the **Fathom** autonomous AI worker platform, built with React 19, Vite, and Rust.

## Overview

Fathom Desktop provides an integrated, native desktop application for running and orchestrating autonomous AI worker swarms with:
- **Embedded Engine Management**: Start, monitor, and configure the local Fathom daemon directly from the desktop shell.
- **Native IPC & Secure Storage**: Local OS keychain integration, secure file system access, and fast native IPC bridges.
- **Real-Time Live Event Feeds**: Full visual session trees, tool invocation logs, token budget telemetry, and prompt steering.
- **Computer Use Viewport**: Seamless screen observation and control interface.

## Prerequisites

- **Rust** 1.80+ (`rustup default stable`)
- **Node.js** 20+ (or Bun / pnpm)
- Platform-specific Tauri v2 dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
  - **Windows**: Microsoft Visual Studio C++ Build Tools & WebView2

## Development

```bash
# Install frontend dependencies
npm install

# Run desktop app in development mode with hot-reloading
npm run tauri dev
```

## Production Build

```bash
# Build production bundle and platform installers (.dmg, .deb, .msi)
npm run tauri build
```

## Architecture

- `src/` — React 19 UI components (Chat, Coworker, Governance, System metrics).
- `src-tauri/` — Tauri v2 Rust backend handling window lifecycle, tray icon, local process supervision, and native API bridges.
