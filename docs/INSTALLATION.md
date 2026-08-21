# Installation

Fathom is a self-hosted Rust runtime: install the binary on a workstation or a server you control, configure an OpenAI-compatible LLM endpoint, and choose which interfaces and optional services to expose. A remote deployment is the same binary running under your process manager or container; there is no hosted Fathom service assumed by these instructions.

---

## Requirements

- **Rust** 1.97+ (edition 2021; the repository pins the supported build toolchain to 1.97.1 and the Dockerfile uses `rust:1.97-bookworm`)
  - This is Fathom's declared MSRV and support baseline. Dependency metadata can advertise a lower floor (currently around 1.88), but that floor is informational and does not imply Fathom source compatibility or testing on older Rust releases.
- **macOS / Linux** (Windows via WSL)
- Optional: `pandoc` (for PDF/DOCX export), `ripgrep` (for grep), `python3`/`node` (for REPL), Chrome (for browser tools)

---

## Building from source

```bash
git clone <repo-url> fathom
cd fathom

# Debug build (fast)
cargo build

# Release build (optimized)
cargo build --release

# Binary
./target/release/fathom --help
```

The Rust workspace consists of 12 crates (`pr-core`, `pr-llm`, `pr-agent`, `pr-tools`, `pr-mcp`, `pr-persistence`, `pr-memory`, `pr-server`, `pr-tui`, `pr-lsp`, `pr-governance`, `pr-supervisor`) plus the root binary. The release profile uses LTO and symbol stripping for minimal binary size.

---

## Configuration setup

The first run auto-generates a default config file at `~/.fathom/config.toml`:

```bash
./target/release/fathom config show
```

This command creates the config directory and file if they don't exist, then prints the effective config with all defaults filled in. The default config is ready to use — you only need to add an API key.

### Manual configuration

Edit `~/.fathom/config.toml`:

```toml
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
api_key = "sk-your-key"          # REQUIRED
model = "deepseek-chat"
```

The config uses TOML with all sections optional — missing fields take sensible defaults. See [CONFIGURATION.md](CONFIGURATION.md) for the full reference.

### Config path override

The config file location can be overridden via the `PR_CONFIG` environment variable:

```bash
PR_CONFIG=/path/to/config.toml fathom run "query" --output ./test/
```

This is useful for one-off runs with different settings, CI/CD pipelines, or testing without touching your main config file.

### CLI config management

Individual config values can be set from the command line:

```bash
# Set a value by dotted key path (validated against the schema)
fathom config set llm.api_key "sk-new-key"
fathom config set agent.max_depth 3
fathom config set search.backend hybrid
```

The value is parsed as bool / integer / float / string (in that order). Unknown keys or type mismatches are rejected without modifying the file.

### First run

`run` accepts any natural-language task; research is only one workflow. Start with a harmless task after configuring the LLM endpoint:

```bash
./target/release/fathom run "Summarize the files in this project" --output ./test/
```

For a remote worker, start the server on the host you control. Loopback is the default; binding beyond loopback requires `FATHOM_API_KEYS`:

```bash
./target/release/fathom serve --host 0.0.0.0 --port 8080
```

---

## Profiles directory

Personas (profiles) are named TOML presets that tune the agent fleet for different task categories. They live in `~/.fathom/profiles/<name>.toml`.

Three built-in presets are available without any files on disk:

| Profile | Purpose |
|---------|---------|
| `hunter` | OSINT / lead generation — aggressive tool usage, broad search |
| `analyst` | Deep analysis — higher depth, slower but more thorough |
| `validator` | Fact-checking — conservative, verifies claims |

### Using profiles

```bash
# List available profiles (built-in + user-defined)
fathom profiles list

# Run with a profile
fathom run --profile hunter "Find decision makers at Acme Corp"

# Create a new profile template
fathom profiles new security-audit
```

A profile overlay can override the system prompt, main/fast model, temperature, agent depth, and tool deny-lists. User-defined files in the profiles directory override built-in presets with the same name.

---

## Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PARALLEL_CDP_ENDPOINT` | Chrome DevTools Protocol endpoint | `http://localhost:9222` |
| `PARALLEL_VISION_API_BASE` | Vision model API base | `https://router.y7.hk/v1` |
| `PARALLEL_VISION_API_KEY` | Vision API key | — |
| `PARALLEL_VISION_MODEL` | Vision model | `qwen-vl-max` |
| `FATHOM_API_KEYS` | API keys for HTTP API (comma-separated) | — (open access) |
| `FATHOM_RATE_LIMIT` | HTTP API rate limit (requests per minute) | `120` |
| `FATHOM_GOVERNANCE_ENABLED` | Enable governance policy engine | `false` |
| `FATHOM_GOVERNANCE_POLICY` | Governance policy JSON | — |
| `FATHOM_CREDENTIAL_KEY` | Encrypted credential vault key (32 bytes, 64 hex chars) | — |
| `COMPUTER_TOKEN` | Computer service authentication token | — |
| `COMPUTER_IMAGE` | Docker image for per-agent computer containers | `fathom/computer:latest` |
| `COMPUTER_NETWORK` | Docker network for computer containers | `fathom-computer` |
| `COMPUTER_BASE_PORT` | Base port for per-agent loopback ports | `19000` |
| `FATHOM_COMPUTER_SERVICE_URL` | Computer service URL (server relay) | `http://127.0.0.1:8765` |
| `COMPUTER_URL` | Computer service URL (agent tools) | `http://127.0.0.1:8765` |
| `COMPUTER_ALLOW_PRIVATE_HOSTS` | Allow private/localhost targets (dev only) | `false` |
| `RUST_LOG` | Logging level | `info` |
| `PR_CONFIG` | Config file path | `~/.fathom/config.toml` |
| `PR_MEMORY_DB` | Memory database path | `~/.fathom/memory.db` |
| `PR_OUTPUT_DIR` | Default task output directory | `./research-output` |
| `PR_JOBS_DB` | Jobs registry database path | `~/.fathom/jobs.db` |
| `PR_JOBS_DIR` | Per-job workspace root | `~/.fathom/jobs` |

### Notes on env vars

- **`PARALLEL_CDP_ENDPOINT`** — overrides the Chrome DevTools Protocol endpoint used by the CDP browser tools.
- **`PR_CONFIG`** — absolute or relative path to a TOML config file. When set, the default `~/.fathom/config.toml` is ignored entirely.
- **`PR_MEMORY_DB`** — path to the long-term memory SQLite database. When set, the `[memory] db_path` config field is ignored.
- **`PR_OUTPUT_DIR`** — overrides `[output] dir` at runtime, useful for ad-hoc runs without editing config.
- **`FATHOM_API_KEYS`** — when the HTTP API server is bound to a non-loopback address, this variable is required. Multiple keys can be comma-separated.

---

## Docker

### Building the image

```bash
docker build -t fathom .
```

The `Dockerfile` uses multi-stage build:
- **Stage 1**: `rust:1.97-bookworm` — builds the release binary from source on Fathom's supported Rust 1.97 baseline (the pinned local toolchain is 1.97.1)
- **Stage 2**: `debian:bookworm-slim` — minimal runtime image with `ca-certificates` and a non-root `researcher` user (uid 1000)

The runtime image contains only the binary, CA certificates, and the researcher home directory — no compiler toolchain, no package manager.

### Running

The container reads `~/.fathom/config.toml` by default. Mount a config volume:

```bash
docker run -it --rm \
  -v ~/.fathom:/home/researcher/.fathom \
  -v fathom-data:/data \
  fathom \
  run "Your task" --output /data/results/
```

Or mount a custom config file:

```bash
docker run -it --rm \
  -v ./my-config.toml:/home/researcher/.fathom/config.toml \
  -v fathom-data:/data \
  fathom \
  run "Your task" --output /data/results/
```

The container's working directory is `/data` — research output and database files (contacts.db, memory.db) should be placed here.

### Docker Compose

If you create a `docker-compose.yml`, configure it with:
- Port 8080 (HTTP API server)
- Volumes: `research-data` (output + databases), `research-config` (config persistence)
- Environment variables for logging and API authentication; mount a Fathom config containing `[llm].api_key`

A compose file is not currently shipped in this repository; use the `docker run` examples above or provide your own compose definition.

### Exposed ports and volumes

| Item | Details |
|------|---------|
| Port | `8080` (HTTP API) |
| Volume `/data` | Research output, contacts.db, memory.db, jobs database |
| Volume `~/.fathom/` | Config file persistence |
| Entrypoint | `fathom` |
| Default command | `serve --port 8080` |

---

## Systemd (Linux)

The repository does not currently ship a `fathom.service` unit file. Create a unit that runs the release binary with your mounted config, then install it:

```bash
# Installation (after creating your own unit file)
sudo cp /path/to/fathom.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable fathom
sudo systemctl start fathom

# Status
sudo systemctl status fathom

# Logs
journalctl -u fathom -f
```

The service runs the HTTP API on port 8080 with:
- **Auto-restart** on failure (`Restart=on-failure`) — the agent process recovers from crashes automatically
- **Hardening** — `ProtectSystem=strict`, `PrivateTmp=true`, `NoNewPrivileges=true`, `CapabilityBoundingSet=` (empty) for a minimal attack surface
- **Resource limits** — CPU and memory constraints configurable in the unit file
- **Environment** — set `RUST_LOG`, `FATHOM_API_KEYS`, and other variables in the unit's `[Service]` section; the LLM API key is read from `~/.fathom/config.toml`

---

## Memory database initialization

The long-term semantic memory store is a SQLite database (mem0/Memora-inspired) that is created automatically on first use. It stores:

- Self-contained facts (append-only rows with versioning via `supersedes` edges)
- FTS5 keyword index for BM25 full-text search
- Binary embeddings for vector similarity search
- Typed edges between memories (supersedes / contradicts / related_to / ...)
- Audit history of all changes

The database is located at `~/.fathom/memory.db` by default, or at the path specified by `PR_MEMORY_DB` or `[memory] db_path` in config. It is opened on session start when `[memory] enabled = true` (the default). The embedder model (default: `text-embedding-3-small`) is auto-detected — if an OpenAI-compatible embedding endpoint is available, it's used; otherwise, a TF-IDF fallback is used.

### Memory isolation scopes

Memories are namespaced by scope for isolation:

| Scope | Purpose |
|-------|---------|
| `user` | Facts about the user/client — personal preferences, contact info |
| `agent` | General agent knowledge — reusable across sessions |
| `run` | Session-local episode facts — temporary, subject to GC |

### Garbage collection

The memory subsystem runs automatic GC based on configurable thresholds:
- `gc_ttl_days` (default: 30) — run-scoped facts older than this are archived
- `gc_compact_above` (default: 200) — compact a scope group when it exceeds this many active rows
- `gc_confidence_decay_rate` (default: 0.02) — daily confidence decay for unaccessed memories
- `gc_confidence_threshold` (default: 0.15) — memories below this confidence are archived
- `gc_auto` — auto-run GC + distill on a background timer (hourly)

---

## Contacts database

The optional contacts database stores people and companies collected by contact-oriented workflows (including OSINT / lead generation). It is a SQLite database created automatically at `./contacts.db` (or the path in `[contacts] db_path`).

### Schema

The database contains these tables:
- `contacts` — id, email, phone, name, title, company, created_at, updated_at, source
- `social_profiles` — id, contact_id, platform, url, username
- `companies` — id, name, website, industry, size, location, description
- `tags` — id, contact_id, tag
- `notes` — id, contact_id, note, created_at

### PostgreSQL backend

When `[contacts] pg_url` is set to a PostgreSQL connection string, the PostgreSQL backend is used instead of SQLite (requires the `postgres` feature of `pr-persistence`). This enables multi-process access and shared contact databases across deployments.

### CRM sync

The `[crm]` section configures optional CRM synchronization:

```toml
[crm]
provider = "amocrm"       # amocrm | bitrix24 | hubspot
domain = "mycompany"      # Domain/subdomain (amoCRM, Bitrix24)
api_key = "..."           # API key/token
```

When configured, contacts saved during a run may be pushed to the selected CRM adapter. The sync is a best-effort post-processing step — failures are logged but do not fail the run itself. No CRM account or delivery is provided by Fathom.

### Contacts CLI

```bash
# List all contacts
fathom contacts list

# Search contacts
fathom contacts search "Acme Corp"

# Export contacts
fathom contacts export --format csv
```

---

## Multi-model routing setup

The LLM layer supports multiple models and providers through a factory pattern. All providers speak the OpenAI-compatible chat-completions protocol, so any endpoint implementing it works (DeepSeek, OpenAI, OpenRouter, vLLM, Ollama, LM Studio, etc.).

### Primary and fast models

```toml
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
api_key = "sk-your-key"
model = "deepseek-chat"       # Primary model for reasoning
fast_model = "deepseek-chat"       # Cheap model for auxiliary calls
```

The `fast_model` is used for high-volume, low-stakes tasks:
- Entity extraction from web pages
- Memory fact classification (deduplicate / supersede / contradict / related)
- Search result reranking
- Memory digest generation

When `fast_model` is empty, the primary model is used for everything.

### Per-role model routing

Different agent roles can use different models on the same endpoint:

```toml
[agent]
role_models = {
  researcher = "deepseek-chat",       # Cheap for broad exploration
  analyst = "deepseek-reasoner",      # Strong for deep analysis
  verifier = "deepseek-chat"          # Lightweight for fact-checking
}
```

This enables cost-efficient routing where expensive reasoning models are reserved for the roles that need them, while high-volume search and extraction roles use cheaper models.

### Provider compatibility

| Provider | `base_url` typically | Notes |
|----------|---------------------|-------|
| DeepSeek | `https://api.deepseek.com` | Native OpenAI-compatible |
| OpenAI | `https://api.openai.com/v1` | |
| OpenRouter | `https://openrouter.ai/api/v1` | Model routing |
| vLLM | `http://localhost:8000/v1` | Self-hosted |
| Ollama | `http://localhost:11434/v1` | Local models |
| LM Studio | `http://localhost:1234/v1` | Local models |

Any unknown provider name is accepted with a trace warning — genuinely new protocols can be added explicitly in the code.

---

## Install from source

The repository does not ship an installer script. Build and install the binary with Cargo:

```bash
cargo install --path . --locked
# or: cargo build --release
```

Create `~/.fathom/config.toml` from the documented configuration and run `fathom serve` or `fathom tui`.
---

## Optional dependencies

### Pandoc (PDF/DOCX export)

```bash
# macOS
brew install pandoc

# Debian/Ubuntu
sudo apt install pandoc
```

Without pandoc, PDF/DOCX export will return an error; HTML/JSON always work.

### Ripgrep (fast grep)

```bash
brew install ripgrep    # macOS
sudo apt install ripgrep # Debian/Ubuntu
```

Without ripgrep, grep uses a built-in fallback.

### Chrome (browser tools)

```bash
# Launch Chrome with CDP
google-chrome --remote-debugging-port=9222 --headless
```

Browser tools automatically detect CDP availability on `localhost:9222`.

---

## Verifying installation

```bash
# Version and help
./target/release/fathom --help

# Config
./target/release/fathom config show

# Health check HTTP API (if serve is running)
curl http://localhost:8080/health

# Tests
cargo test --workspace
```

## Multi-agent Communication

Fathom agents can communicate with each other in real-time via the built-in `hub` tool. Each agent is registered on a process-global IrcBus and can send/receive messages, broadcast to all peers, and wait for replies. Parked agents are automatically revived when a message arrives for them. The `daemon` tool lets agents manage long-running background processes (dev servers, watchers, REPLs) with port readiness checks.