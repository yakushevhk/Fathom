# Fathom Desktop (GPUI)

High-performance, GPU-accelerated native desktop client for the **Fathom** autonomous AI coworker platform, built in **Rust** using **GPUI** (Zed's GPU-accelerated UI framework) with 1:1 feature parity to [CopilotKit/openbot](https://github.com/CopilotKit/openbot).

## Overview

Fathom Desktop delivers a 60fps Metal/Vulkan native interface for orchestrating autonomous AI worker swarms:
- **Coworkers & Fleet Channels**: Real-time channels with specialized coworkers (`General Assistant`, `Risk Analyst`, `DevOps Engineer`).
- **Rich Chat & Thinking Trace**: Multi-turn conversation transcript with expandable tool call chips and a slide-out live reasoning drawer (`[t]`).
- **Live Computer Viewport**: 60fps canvas showing the worker's sandboxed Chromium browser with current URL and security state.
- **Take the Wheel**: Instant operator takeover for CAPTCHAs, multi-step logins, or sensitive manual interventions.
- **Protected Secret Dialog**: Direct injection of 2FA codes and passwords without logging or exposure in transcripts.
- **Governance & Policy Engine**: Real-time CEL action policy rules table, fail-closed security gates, and audit log with refusal tracking.
- **Routines & Standing Instructions**: Scheduled automated tasks with PostgreSQL lease management.
- **Skills & Tool Authoring**: Modular instruction sets that narrow coworker tool access.
- **Hardware Credentials Vault**: AES-256-GCM encrypted secrets store.
- **Embedded Engine Supervisor**: Spawns, monitors, and stops local `fathom serve` engine process automatically.

## Prerequisites

- **Rust 1.97+** (`rustup default stable`)
- Platform-specific GPUI prerequisites:
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)
  - **Linux**: `libxkbcommon-dev`, `libwayland-dev`, `libvulkan-dev`, `pkg-config`

## Running in Development

```bash
# Run native desktop client
cargo run -p pr-desktop

# Or build release binary
cargo build --release -p pr-desktop
```

The compiled binary will be located at `target/release/fathom-desktop` (or `target/debug/fathom-desktop`).

## Architecture

```
crates/desktop/
├── Cargo.toml                  # Package manifest (GPUI 0.2.2, tokio, reqwest, parking_lot)
└── src/
    ├── main.rs                 # Window configuration (1220x780), tracing init, app entry
    ├── theme.rs                # Glassmorphic dark metal theme, traffic lights, design tokens
    ├── state.rs                # Central reactive application state (RwLock, channels, messages)
    ├── api.rs                  # HTTP/SSE client for Fathom/OpenBot daemon API
    ├── daemon.rs               # Process lifecycle supervisor for fathom serve
    ├── app.rs                  # 3-Pane master layout and navigation tab routing
    └── components/
        ├── topbar.rs           # Glassmorphic metal topbar, traffic lights, breadcrumbs, status
        ├── sidebar.rs          # Navigation tabs, channel switcher, coworker fleet roster
        ├── chat.rs             # Conversation transcript, tool execution chips, thinking drawer
        ├── composer.rs         # Prompt input, quick trigger chips (@bot, /skill), turn dispatch
        ├── computer.rs         # Live browser viewport, take-the-wheel, secret entry modal
        ├── governance.rs       # Policy rules table, decision boundary simulator, audit log
        ├── routines.rs         # Scheduled standing instructions and cron routines
        ├── skills.rs           # Reusable coworker skills and tool binding editor
        └── vault.rs            # Encrypted hardware secrets vault view
```
