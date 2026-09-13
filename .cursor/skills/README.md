# Fathom AI Skills Catalog (`.cursor/skills/`)

This directory contains standardized, public-safe Agent Skills providing authoritative architectural, security, and engineering guidance for AI assistants working in the Fathom repository.

## Available Skills

| Skill | Directory | Scope & Purpose |
|---|---|---|
| **`fathom-core`** | `.cursor/skills/fathom-core/` | 13 Rust workspace crates, 63 built-in tools (up to 75), ToolRegistry, server, and execution invariants. |
| **`fathom-swarm`** | `.cursor/skills/fathom-swarm/` | Multi-agent coordination, Tokio JoinSet DAG execution trees, 14-step main loop, steering, and compaction. |
| **`fathom-memory`** | `.cursor/skills/fathom-memory/` | Long-term semantic memory, SQLite FTS5 BM25 + vector graph, secret scrubbing, and HNSW indexing. |
| **`fathom-security`** | `.cursor/skills/fathom-security/` | Sovereignty guarantees, HostSandbox (bwrap/sandbox-exec), air-gapped deployment, AES-256 vault, and ЗРУ-547. |
| **`fathom-mcp`** | `.cursor/skills/fathom-mcp/` | Model Context Protocol bi-directional client/server, `fathom mcp-serve`, and dynamic tool hot-reloading. |
| **`fathom-bot`** | `.cursor/skills/fathom-bot/` | Fathom Bot desktop client (Electron/React 19), Node harness server (`127.0.0.1:8799`), UI components, and action cards. |
| **`fathom-website`** | `.cursor/skills/fathom-website/` | Astro website architecture, bilingual i18n build generator (`i18n-generate.mjs`), MDX docs, and screenshots. |
| **`fathom-whitepaper`** | `.cursor/skills/fathom-whitepaper/` | 42-page technical whitepaper, 4:3 pitch deck design standards, benchmark metrics, and OneID accreditation (#1924). |

## Safety & Public Repository Policy

All skills in this directory are strictly curated for public open-source distribution:
* **Zero Secrets**: No internal tokens, private SSH keys, server root passwords, or internal tunnel endpoints.
* **Public Specifications Only**: Focuses strictly on architecture, coding invariants, algorithms, and official startup accreditation data.
