# Installation

---

## Requirements

- **Rust** 1.75+ (edition 2021)
- **macOS / Linux** (Windows via WSL)
- Optional: `pandoc` (for PDF/DOCX export), `ripgrep` (for grep), `python3`/`node` (for REPL), Chrome (for browser tools)

---

## Building from source

```bash
git clone <repo-url> parallel-research
cd parallel-research

# Debug build (fast)
cargo build

# Release build (optimized)
cargo build --release

# Binary
./target/release/parallel-research --help
```

---

## First run

1. **Create config** (auto-generated on first run):

```bash
./target/release/parallel-research config show
```

2. **Edit** `~/.parallel-research/config.toml`:

```toml
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
api_key = "sk-your-key"          # REQUIRED
model = "deepseek-v4-flash"
```

3. **Run**:

```bash
./target/release/parallel-research run "Test query" --output ./test/
```

---

## Install script

```bash
./install.sh
```

The script:
- Builds a release binary
- Installs it to `/usr/local/bin/` (or `~/.local/bin/`)
- Creates a default config
- Optionally installs a systemd service (`INSTALL_SYSTEMD=1 ./install.sh`)

---

## Docker

### Building the image

```bash
docker build -t parallel-research .
```

The `Dockerfile` uses multi-stage build:
- **Stage 1**: `rust:1.82-bookworm` — build
- **Stage 2**: `debian:bookworm-slim` — minimal image, non-root user

### Running

```bash
docker run -it --rm \
  -e PARALLEL_LLM_API_KEY="sk-your-key" \
  -v research-data:/data \
  parallel-research \
  run "Your query" --output /data/results/
```

### Docker Compose

```bash
docker compose up -d
```

`docker-compose.yml` configures:
- Port 8080 (HTTP API)
- Volumes: `research-data`, `research-config`
- Environment variables

---

## Systemd (Linux)

Unit file: `parallel-research.service`

```bash
# Installation
sudo cp parallel-research.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable parallel-research
sudo systemctl start parallel-research

# Status
sudo systemctl status parallel-research

# Logs
journalctl -u parallel-research -f
```

The service runs the HTTP API on port 8080 with auto-restart and hardening options.

---

## Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PARALLEL_LLM_API_KEY` | LLM API key | — |
| `PARALLEL_CDP_ENDPOINT` | Chrome DevTools Protocol endpoint | `http://localhost:9222` |
| `PARALLEL_VISION_API_BASE` | Vision model API base | `https://router.y7.hk/v1` |
| `PARALLEL_VISION_API_KEY` | Vision API key | — |
| `PARALLEL_VISION_MODEL` | Vision model | `qwen-vl-max` |
| `PARALLEL_RESEARCH_API_KEYS` | API keys for HTTP API (comma-separated) | — (open access) |
| `RUST_LOG` | Logging level | `info` |

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
./target/release/parallel-research --help

# Config
./target/release/parallel-research config show

# Health check HTTP API (if serve is running)
curl http://localhost:8080/health

# Tests
cargo test --workspace
```