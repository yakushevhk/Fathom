# Coworkers, Channels & Schedules

> Persistent worker profiles, cron-like scheduling, and configured notification channels for recurring autonomous operations.

Fathom can run workers on infrastructure you control: coworkers are durable profiles with schedules and channels. This extends a one-off session into recurring operations while keeping execution, credentials, and delivery in your deployment. It is not a hosted workforce service.

---

## Coworkers

A **coworker** is a persistent agent profile stored in SQLite. Each coworker has its own identity, goals, and optional linked Fathom session or channel.

### Coworker fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID v7 | Unique identifier |
| `name` | string | Human-readable name |
| `description` | string | Natural-language description of the coworker's role and goals |
| `system_prompt` | string? | Optional system prompt override |
| `profile` | string? | Persona profile name (hunter, analyst, validator, or custom) |
| `session_id` | UUID v7? | Linked session output |
| `channel_id` | UUID v7? | Linked channel for results delivery |
| `enabled` | bool | Whether the coworker is active |
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
| `POST` | `/api/v1/coworkers/:id/run` | Trigger an immediate run |

### Run lifecycle

When a coworker is triggered (via schedule or manual run):

1. The coworker's profile and prompt are loaded
2. A new session is created from the configured coworker query/description
3. Results are delivered when a linked channel and its transport are configured
4. The session ID is stored in the coworker record for retrieval

A schedule or channel does not provision external infrastructure or credentials; those remain operator-managed configuration.

---

## Channels

**Channels** link coworkers to surfaces where they can be addressed and where results are delivered. A channel is a symbolic communication endpoint.

### Channel types

| Type | Description |
|------|-------------|
| `cli` | Command-line interface — results printed to stdout |
| `http` | HTTP webhook — results POSTed to a URL |
| `telegram` | Configured Telegram delivery |
| `email` | Configured email delivery |
| `slack` | Configured Slack/webhook delivery, when supported by the deployment |

### API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/channels` | List all channels |
| `POST` | `/api/v1/channels` | Create a new channel |
| `GET` | `/api/v1/channels/:id` | Get channel details |
| `PUT` | `/api/v1/channels/:id` | Update channel |
| `DELETE` | `/api/v1/channels/:id` | Delete channel |

### Channel configuration

```json
{
  "name": "team-alerts",
  "type": "telegram",
  "config": {
    "bot_token": "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11",
    "chat_id": "-1001234567890"
  }
}
```

---

## Schedules

**Schedules** trigger coworker runs on cron-like timers. The scheduler uses **atomic claim** to ensure that concurrent runners never execute the same task twice.

### Schedule fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID v7 | Unique identifier |
| `coworker_id` | UUID v7 | The coworker to run |
| `cron` | string | Cron expression (e.g., `0 */4 * * *` for every 4 hours) |
| `timezone` | string | Timezone (default: UTC) |
| `enabled` | bool | Whether the schedule is active |
| `last_run_at` | datetime? | Timestamp of the last run |
| `next_run_at` | datetime? | Calculated next run time |
| `created_at` | datetime | Creation timestamp |

### Atomic claim

When the scheduler fires, it **atomically claims** the schedule by updating `next_run_at` to a future time in a single SQLite transaction. This prevents two concurrent scheduler processes from running the same task:

```sql
UPDATE schedules
SET last_run_at = CURRENT_TIMESTAMP,
    next_run_at = calculate_next(cron, CURRENT_TIMESTAMP)
WHERE id = ?
  AND next_run_at <= CURRENT_TIMESTAMP
  AND enabled = 1
```

If the UPDATE affects 0 rows, another scheduler already claimed this task — the current process skips it.

### API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/schedules` | List all schedules |
| `POST` | `/api/v1/schedules` | Create a new schedule |
| `GET` | `/api/v1/schedules/:id` | Get schedule details |
| `PUT` | `/api/v1/schedules/:id` | Update schedule |
| `DELETE` | `/api/v1/schedules/:id` | Delete schedule |
| `POST` | `/api/v1/schedules/claim` | Atomic claim endpoint (internal, used by the scheduler) |

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

### Notification types

| Type | Description |
|------|-------------|
| `session.completed` | Coworker session completed successfully |
| `session.failed` | Coworker session failed |
| `watch.new_contacts` | New contacts discovered in a scheduled run |
| `schedule.missed` | A scheduled run was missed (e.g., system was down) |

### API

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/notifications/test` | Send a test notification to a channel |

### Configuration

```toml
# ~/.fathom/config.toml

[notifications]
# Global notification settings (also used by sessions)
webhook_url = ""
email_to = ""
telegram_bot_token = ""
telegram_chat_id = ""

# Coworker-specific notification channels are configured per-channel
# in the channels API and linked to coworkers via channel_id.
```

---

## Complete workflow example

```bash
# 1. Create a channel (Telegram)
curl -X POST http://localhost:8080/api/v1/channels \
  -H "Content-Type: application/json" \
  -d '{
    "name": "alerts",
    "type": "telegram",
    "config": { "bot_token": "...", "chat_id": "..." }
  }'

# 2. Create a coworker
curl -X POST http://localhost:8080/api/v1/coworkers \
  -H "Content-Type: application/json" \
  -d '{
    "name": "market-watcher",
    "description": "Monitor AI startup funding news daily",
    "profile": "analyst",
    "channel_id": "<channel-id>"
  }'

# 3. Schedule the coworker to run every 4 hours
curl -X POST http://localhost:8080/api/v1/schedules \
  -H "Content-Type: application/json" \
  -d '{
    "coworker_id": "<coworker-id>",
    "cron": "0 */4 * * *",
    "timezone": "UTC"
  }'

# 4. Test the notification channel
curl -X POST http://localhost:8080/api/v1/notifications/test \
  -H "Content-Type: application/json" \
  -d '{
    "channel_id": "<channel-id>",
    "message": "Hello from Fathom!"
  }'
```

---

## Related

- [USAGE.md](USAGE.md) — CLI commands for profiles and coworker management
- [HTTP-API.md](HTTP-API.md) — coworkers, channels, schedules API endpoints
- [CONFIGURATION.md](CONFIGURATION.md) — notification configuration