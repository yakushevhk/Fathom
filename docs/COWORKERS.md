# Coworkers, Channels & Schedules

> Persistent worker profiles, cron-like scheduling, and configured notification channels for recurring autonomous operations.

Fathom can run workers on infrastructure you control: coworkers are durable profiles with schedules and channels. This extends a one-off session into recurring operations while keeping execution, credentials, and delivery in your deployment. It is not a hosted workforce service.

---

## Coworkers

A **coworker** is a persistent agent profile stored in SQLite. Each coworker has its own identity, role, and optional linked session.

### Coworker fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (alphanumeric, dashes, underscores, dots) |
| `name` | string | Human-readable name (max 200 chars) |
| `title` | string | Short title describing the coworker's purpose (max 200 chars) |
| `role` | string | Role identifier (max 100 chars) |
| `prompt` | string | System prompt / instructions for the coworker (max 32,000 chars) |
| `visibility` | string | Scope visibility — `"private"` (default) or `"shared"` (max 32 chars) |
| `active` | bool | Whether the coworker is enabled (default: `true`) |
| `created_at` | datetime | Creation timestamp |
| `updated_at` | datetime | Last update timestamp |

### API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/coworkers` | List all coworkers |
| `POST` | `/api/v1/coworkers` | Create a new coworker |
| `GET` | `/api/v1/coworkers/:id` | Get coworker details |
| `PUT` | `/api/v1/coworkers/:id` | Update coworker |
| `DELETE` | `/api/v1/coworkers/:id` | Delete coworker |

**Create request body:**

```json
{
  "name": "market-watcher",
  "title": "Daily market monitoring",
  "role": "analyst",
  "prompt": "Monitor AI startup funding news daily",
  "visibility": "private",
  "active": true
}
```

### Run lifecycle

Coworkers and schedules are persisted configuration. The server exposes schedule claiming, but this endpoint does not spawn jobs; an external scheduler or operator must submit the resulting task through the session or jobs API.

When an external runner starts a coworker task:

1. The coworker's prompt and role are loaded
2. A new session is created from the schedule's query text
3. Results are delivered when a linked channel and its transport are configured
4. The session ID is stored in the coworker record for retrieval

A schedule or channel does not provision external infrastructure or credentials; those remain operator-managed configuration.

---

## Channels

**Channels** link coworkers to sessions where they can be addressed and where results are delivered. A channel is a lightweight symbolic mapping — it associates a title with an optional linked session.

### Channel fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier |
| `coworker_id` | string | The coworker this channel belongs to |
| `title` | string | Human-readable channel name |
| `session_id` | string? | Optional linked Fathom session for results delivery |
| `created_at` | datetime | Creation timestamp |
| `updated_at` | datetime | Last update timestamp |

### API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/channels?coworker_id=<id>` | List channels for a coworker |
| `POST` | `/api/v1/channels` | Create a new channel |
| `PUT` | `/api/v1/channels/:id` | Update channel title/session mapping |
| `DELETE` | `/api/v1/channels/:id` | Delete channel |

**Create request body:**

```json
{
  "coworker_id": "<coworker-id>",
  "title": "team-alerts",
  "session_id": "<session-id>"
}
```

---

## Schedules

**Schedules** trigger coworker runs on cron-like timers. The scheduler uses **atomic claim** to ensure that concurrent runners never execute the same task twice.

### Schedule fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier |
| `coworker_id` | string | The coworker to run |
| `cron_expression` | string | Five-field cron expression (e.g., `0 */4 * * *` for every 4 hours) |
| `timezone` | string | IANA timezone or UTC offset (default: `"UTC"`) |
| `query` | string | The task query/prompt sent to the coworker session (max 20,000 chars) |
| `enabled` | bool | Whether the schedule is active (default: `true`) |
| `next_run` | string? | Calculated next run time (RFC 3339) |
| `last_run` | datetime? | Timestamp of the last run |
| `created_at` | datetime | Creation timestamp |
| `updated_at` | datetime | Last update timestamp |

### Atomic claim

When the scheduler fires, it **atomically claims** due schedules by updating `next_run` to a future time in a single SQLite transaction. This prevents two concurrent scheduler processes from running the same task. If the claim affects 0 rows for a given schedule, another scheduler already claimed it.

### API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/schedules` | List all schedules |
| `POST` | `/api/v1/schedules` | Create a new schedule |
| `GET` | `/api/v1/schedules/:id` | Get schedule details |
| `PUT` | `/api/v1/schedules/:id` | Update schedule |
| `DELETE` | `/api/v1/schedules/:id` | Delete schedule |
| `POST` | `/api/v1/schedules/claim` | Atomic claim due schedules (internal, used by the scheduler) |

**Create request body:**

```json
{
  "coworker_id": "<coworker-id>",
  "cron_expression": "0 */4 * * *",
  "timezone": "UTC",
  "query": "Monitor AI startup funding news today",
  "enabled": true
}
```

### Example cron expressions

| Expression | Description |
|------------|-------------|
| `0 */4 * * *` | Every 4 hours |
| `0 9 * * 1-5` | Weekdays at 9 AM |
| `0 0 * * 0` | Every Sunday at midnight |
| `*/30 * * * *` | Every 30 minutes |
| `0 0 1 * *` | First day of every month |

---

## Notifications

Notifications deliver results to configured symbolic channels. Delivery still requires the corresponding `[notifications]` settings, credentials, and reachable external transport; Fathom does not provide those services.

### Supported channels

| Channel | Description |
|---------|-------------|
| `webhook` | HTTP webhook — sends to the configured `webhook_url` |
| `email` | SMTP email delivery (configured via `smtp_host`, `email_to`, etc.) |
| `telegram` | Telegram bot delivery (configured via `telegram_bot_token`, `telegram_chat_id`) |

### API

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/notifications/test` | Send a test notification through a configured channel |

**Test notification request body:**

```json
{
  "channel": "telegram"
}
```

The `channel` field accepts `"webhook"`, `"email"`, or `"telegram"` — it references a channel already configured in `[notifications]`, not an arbitrary destination.

### Configuration

```toml
# ~/.fathom/config.toml

[notifications]
webhook_url = ""
email_to = ""
smtp_host = "localhost"
smtp_port = 25
email_from = "fathom@localhost"
telegram_bot_token = ""
telegram_chat_id = ""
```

---

## Complete workflow example

```bash
# 1. Create a coworker
curl -X POST http://localhost:8080/api/v1/coworkers \
  -H "Content-Type: application/json" \
  -d '{
    "name": "market-watcher",
    "title": "Daily market monitoring",
    "role": "analyst",
    "prompt": "Monitor AI startup funding news daily"
  }'

# 2. Create a channel linked to a session
curl -X POST http://localhost:8080/api/v1/channels \
  -H "Content-Type: application/json" \
  -d '{
    "coworker_id": "<coworker-id>",
    "title": "team-alerts"
  }'

# 3. Schedule the coworker to run every 4 hours
curl -X POST http://localhost:8080/api/v1/schedules \
  -H "Content-Type: application/json" \
  -d '{
    "coworker_id": "<coworker-id>",
    "cron_expression": "0 */4 * * *",
    "timezone": "UTC",
    "query": "Monitor AI startup funding news today"
  }'

# 4. Test the notification channel
curl -X POST http://localhost:8080/api/v1/notifications/test \
  -H "Content-Type: application/json" \
  -d '{
    "channel": "webhook"
  }'
```

---

## Related

- [USAGE.md](USAGE.md) — CLI commands for profiles and coworker management
- [HTTP-API.md](HTTP-API.md) — coworkers, channels, schedules API endpoints
- [CONFIGURATION.md](CONFIGURATION.md) — notification configuration
