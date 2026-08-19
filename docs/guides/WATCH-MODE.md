# Watch Mode — Scheduled Harvesting

Watch mode turns a one-shot research query into a **repeating harvest loop** that detects new contacts across runs and alerts you via configured notification channels. It is designed for ongoing monitoring: competitor team tracking, lead-gen drip campaigns, job board surveillance, and any scenario where you want to know when new people appear in a target set.

---

## How `--repeat` Works

The `--repeat <SECS>` flag on `fathom run` puts the process into a loop:

```
fathom run "<QUERY>" --repeat <SECONDS> [--output <DIR>] [--profile <NAME>]
```

```
fathom run "Find CTOs at European fintech startups" --repeat 21600
```

### Loop lifecycle

```
┌─────────────────────────────────────────────────┐
│  Iteration 1: full research run                  │
│  → contacts saved to DB                          │
│  → known_keys = contact_key_set(all contacts)    │
│  → sleep(repeat_seconds)                         │
├─────────────────────────────────────────────────┤
│  Iteration 2: research run again                 │
│  → current = list_all(DB)                        │
│  → diff = current ─ known_keys                   │
│  → if diff not empty: print + alert              │
│  → known_keys = contact_key_set(current)         │
│  → sleep(repeat_seconds)                         │
├─────────────────────────────────────────────────┤
│  Iteration N: …                                  │
│  (until Ctrl+C)                                  │
└─────────────────────────────────────────────────┘
```

Key details:

1. **Iteration 1 establishes a baseline.** The full research runs, contacts are harvested and saved to the contact database (SQLite or PostgreSQL). The set of all known identity keys is captured.

2. **Subsequent iterations re-run the full query.** Each cycle calls `run_research()` again — the same LLM decomposition, tool execution, and synthesis. This is not a lightweight scrape; it is a full agent research session.

3. **Sleep is adaptive.** The loop waits `repeat_seconds` minus the time the research run took, with a minimum of 5 seconds. If a run takes 40 minutes and `--repeat 3600` was set, the next run starts in ~56 minutes.

4. **The loop runs forever.** Interrupt with `Ctrl+C`. The contact database persists across restarts, so resuming later does not re-alert on already-known contacts — the known-keys set is rebuilt from the database on startup.

---

## Diff Detection Logic

The diff engine compares the current contact database against the set of identity keys known from the previous iteration. It lives in `src/main.rs` (functions `contact_keys`, `contact_key_set`, `watch_new_contacts`).

### Identity keys

Each contact produces one or more **identity keys** — normalised tokens that survive formatting differences:

| Key prefix | Source | Normalisation |
|-----------|--------|---------------|
| `email:` | `Contact.email` | Trimmed, lower-cased via `normalize_email()` |
| `phone:` | `Contact.phone` | Digits-only via `normalize_phone()` |
| `person:` | `name@company` fallback | Lower-cased, `"?"` for missing fields |

A contact with both email and phone produces two keys (`email:j.doe@acme.com`, `phone:79991234567`). A contact with neither email nor phone falls back to a single `person:j doe@acme` key, so brand-new person entries still appear in diffs.

### New-contact detection

A contact is considered **new** when **none** of its identity keys exist in the previous set:

```rust
fn watch_new_contacts<'a>(
    previous_keys: &HashSet<String>,
    current: &'a [Contact],
) -> Vec<&'a Contact> {
    current.iter().filter(|c| {
        let keys = contact_keys(c);
        !keys.is_empty() && keys.iter().all(|k| !previous_keys.contains(k))
    }).collect()
}
```

This means:

- **Same email, different run** → not new (email matches).
- **Same phone, different name** → not new (phone matches).
- **New email + new phone, same person keys** → new (no prior keys match).
- **`A@x.com` in iteration 1, `A@X.com` in iteration 2** → not new (normalised email `a@x.com` matches).

### Example diffs

| Iteration 1 contacts | Iteration 2 contacts | New? | Reason |
|---|---|---|---|
| `a@x.com` | `a@x.com`, `b@x.com` | `b@x.com` | Email `b@x.com` was not in keys |
| `alice@acme.com` | `alice@acme.com`, `+7 999 123-45-67` (same person) | No new | Same email normalises to same key |
| `alice@acme.com` | `bob@acme.com` (no email, no phone, name "Bob") | New | `person:bob@acme` not in keys |
| `a@x.com` (name "Alice") | `a@x.com` (name "Alice Smith") | No new | Same email key |

---

## Alert Channels

Watch mode uses the same `[notifications]` configuration as session completion notifications. Alerts are sent **only for new contacts** — not for every run.

### Configuration

```toml
[notifications]
# Webhook (JSON POST)
webhook_url = "https://hooks.slack.com/services/..."

# Email (SMTP)
email_to = "me@example.com"
email_from = "fathom@example.com"
smtp_host = "smtp.example.com"
smtp_port = 587
smtp_username = "fathom"
smtp_password = "..."

# Telegram
telegram_bot_token = "123456:ABC-DEF..."
telegram_chat_id = "-1001234567890"

# Enable any subset — empty fields are skipped
```

### Webhook alert

When `webhook_url` is set, the system sends an HTTP POST with the event `watch.new_contacts`:

```json
{
  "event": "watch.new_contacts",
  "subject": "Watch: 3 new contact(s) for \"CTOs at European fintech\"",
  "text": "Watch diff for \"CTOs at European fintech\": 3 new contact(s)\n  + Maria Ivanova (CEO) @ Acme <ceo@acme.ru>\n  + John Smith (CTO) @ Beacon AI <john@beacon.ai>\n  + Alice Wang (Head of Engineering) @ CloudSync <alice@cloudsync.io>"
}
```

### Email alert

Sent to `email_to` via SMTP with subject `Watch: N new contact(s) for "..."`. The body contains the full watch report with contact details.

### Telegram alert

Sent to the configured chat via the Bot API. The message format:

```
Watch: 3 new contact(s) for "CTOs at European fintech"

Watch diff for "CTOs at European fintech": 3 new contact(s)
  + Maria Ivanova (CEO) @ Acme <ceo@acme.ru>
  + John Smith (CTO) @ Beacon AI <john@beacon.ai>
  + Alice Wang (Head of Engineering) @ CloudSync <alice@cloudsync.io>
```

### Notification events in watch mode

| Event | Trigger | Delivery |
|---|---|---|
| `watch.new_contacts` | New contacts found in a watch iteration | Every configured channel |
| `session.completed` | Each research run finishes successfully | Via `finalize_session` (independent of watch) |
| `session.failed` | A research run crashes | Via `notify_failure` |

---

## Output Format

### Stdout per iteration

```
── Watch iteration 3 ──
…
── Watch: no new contacts this run ──
Next run in 21540s (Ctrl+C to stop)
```

When new contacts are found:

```
── Watch iteration 4 ──
…
🔔 Watch diff for "CTOs at European fintech": 2 new contact(s)
  + Maria Ivanova (CEO) @ Acme <ceo@acme.ru>
  + John Smith (CTO) @ Beacon AI <john@beacon.ai>

Next run in 21480s (Ctrl+C to stop)
```

### Watch report format

The `watch_report()` function renders up to 20 contacts per diff. Each line shows:

```
  + <Name> (<Title>) @ <Company> <<Email>>  <Phone>
```

If more than 20 contacts are new, a trailing line is added:

```
  … and 5 more
```

The same report is printed to stdout **and** sent through every configured notification channel.

---

## Cron / Systemd Setup

Watch mode is designed to run in a terminal or as a managed service. For production deployments, use systemd or a process supervisor.

### Systemd service unit (watch variant)

Create `/etc/systemd/system/fathom-watch.service`:

```ini
[Unit]
Description=Fathom — scheduled harvesting (watch mode)
Documentation=https://github.com/yakushev/fathom
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=researcher
WorkingDirectory=/opt/fathom
ExecStart=/usr/local/bin/fathom run \
  "Find CTOs at European fintech startups. Extract emails, LinkedIn, phone numbers." \
  --output /data/watch/fintech-ctos \
  --repeat 21600
Restart=on-failure
RestartSec=10

# Security hardening
ProtectSystem=strict
PrivateTmp=true
NoNewPrivileges=true
CapabilityBoundingSet=

# Environment
Environment=PARALLEL_LLM_API_KEY=sk-...
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

```bash
# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable fathom-watch
sudo systemctl start fathom-watch

# Monitor
sudo systemctl status fathom-watch
journalctl -u fathom-watch -f
```

### Cron (simpler alternative)

For shorter intervals or lightweight setups, cron can run one-shot research sessions. Note that **without `--repeat`** each cron invocation is a fresh run with no diff tracking against the previous run — the contact database persists, so `save_contacts` deduplication works, but watch-mode alerts are not triggered.

```cron
# Every 6 hours
0 */6 * * * /usr/local/bin/fathom run "Find CTOs at European fintech startups" --output /data/watch/fintech-ctos >> /var/log/fathom-watch.log 2>&1
```

To use true watch-mode diff detection with cron, persist the `--repeat` loop in a screen/tmux session or use the systemd service above.

### Docker

```bash
docker run -d --restart on-failure \
  --name fathom-watch \
  -e PARALLEL_LLM_API_KEY="sk-..." \
  -v fathom-data:/data \
  fathom \
  run "Find CTOs at European fintech startups" \
  --output /data/watch/fintech-ctos \
  --repeat 21600
```

### Supervisor

```ini
[program:fathom-watch]
command=fathom run "Find CTOs at European fintech" --output /data/watch/fintech-ctos --repeat 21600
directory=/opt/fathom
user=researcher
autostart=true
autorestart=true
environment=PARALLEL_LLM_API_KEY="sk-...",RUST_LOG="info"
```

---

## Use Cases

| Use case | Interval | Rationale |
|---|---|---|
| **Competitor team tracking** | 86400 (24h) | Daily check for new hires, executive changes at competitor companies |
| **Lead generation drip** | 21600 (6h) | Catch new business listings, updated contact pages, fresh Crunchbase entries |
| **Job board monitoring** | 3600 (1h) | Track new openings at target companies for market intelligence |
| **News-driven monitoring** | 43200 (12h) | Twice-daily check for new mentions of people at target companies |
| **CRM enrichment** | 604800 (7d) | Weekly pass to find missing contacts for existing accounts |

### Example: competitor people tracking

```bash
fathom run \
  "Find new team members, executives, and engineering leads at Acme Corp, \
   Beacon AI, and CloudSync. Extract emails, LinkedIn profiles, phone numbers." \
  --output ./watch/competitors/ \
  --repeat 86400
```

First run: full extraction of all known contacts. Subsequent runs: only **new** hires or newly discovered contacts trigger alerts.

### Example: lead-gen with Goal Mode

```bash
fathom run --profile hunter \
  "Find contacts of 5 new CTOs at European fintech companies \
   (not already in my database)" \
  --output ./watch/fintech-ctos/ \
  --repeat 43200
```

The `hunter` profile with `replan_rounds` (default: 1) ensures each watch cycle does thorough gap-filling, so you do not miss a contact just because the first pass was incomplete.

---

## Caveats

- **Watch mode is resource-intensive.** Every iteration is a full LLM-powered research session with tool calls, web searches, and synthesis. Budget API costs and runtime accordingly.
- **The contact database is shared across all runs.** Watch mode diffs against the global `contacts.db` (or PostgreSQL), not against per-iteration output. This means contacts discovered in a manual `fathom run` between watch iterations will be accounted for in the next diff.
- **No in-memory state between restarts.** The known-keys set is rebuilt from the contact database on each start. If you restart the process, iteration 1 becomes a baseline again — no re-alerting for existing contacts because `save_contacts` deduplicates at the database level.
- **Minimum sleep is 5 seconds.** Even if `--repeat 1` is set, the loop sleeps at least 5 seconds to avoid tight busy-waiting.
- **Notification failures are best-effort.** A failing webhook, unreachable SMTP server, or invalid Telegram bot token does not stop the loop — errors are logged and the next iteration proceeds normally.

---

## Related

- [Lead Generation Guide](LEAD-GENERATION.md) — full pipeline for harvesting contacts
- [Research Workflow Guide](RESEARCH-WORKFLOW.md) — end-to-end research patterns including watch mode
- [Configuration Reference](../CONFIGURATION.md) — `[notifications]` and `[contacts]` sections
- [Installation Guide](../INSTALLATION.md) — systemd service, Docker deployment