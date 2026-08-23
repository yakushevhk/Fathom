# Crate `persistence` — detailed documentation

## Overview

The `pr-persistence` crate is responsible for persistent storage of research sessions, agents, messages, findings, tool results, subtasks, contacts, credentials, coworkers, channels, jobs, replay, and schedules. It supports two backends:

- **SQLite** (default, via `rusqlite`) — the primary backend for `Persistence`, `ContactDb`, `JobsDb`
- **PostgreSQL** (optional, feature `postgres`, via `deadpool_postgres` + `tokio_postgres`) — `PgContactDb`

---

## File structure

| File | Purpose |
|------|-----------|
| `lib.rs` | Module declarations and re-exports |
| `db.rs` | `Persistence` — the main sessions/agents store (SQLite) |
| `contacts.rs` | `ContactDb` — contact store (SQLite) |
| `pg.rs` | `PgContactDb` — contact store (PostgreSQL) |
| `store.rs` | `ContactStore` — unified async trait + `open_contact_store` factory |
| `history.rs` | `SessionHistory` — facade for reading session history |
| `credentials.rs` | `CredentialRow` — AES-256-GCM encrypted credential vault on `Persistence` |
| `coworkers.rs` | `CoworkerRow`, `ChannelRow` — coworker and channel CRUD on `Persistence` |
| `jobs.rs` | `JobsDb` — standalone durable background-job registry |
| `replay.rs` | `ReplayActionRow` — redacted governed-action replay timeline on `Persistence` |
| `schedules.rs` | `ScheduleRow` — cron schedule CRUD and atomic due-claim on `Persistence` |

---

## 1. `lib.rs` — module entry point

```rust
pub mod contacts;
pub mod credentials;
pub mod coworkers;
pub mod db;
pub mod history;
pub mod jobs;
pub mod replay;
pub mod store;
pub mod schedules;

#[cfg(feature = "postgres")]
pub mod pg;

pub use contacts::*;
pub use credentials::*;
pub use coworkers::*;
pub use db::*;
pub use history::*;
pub use jobs::*;
pub use replay::*;
pub use schedules::*;
pub use store::*;

#[cfg(feature = "postgres")]
pub use pg::*;
```

The `pg` module is compiled only when the `postgres` feature is enabled. All main types are re-exported via `pub use`, so external crates can import `pr_persistence::Persistence`, `pr_persistence::ContactDb`, etc.

---

## 2. `db.rs` — `Persistence` (SQLite)

### 2.1 Structure

```rust
pub struct Persistence {
    pub(crate) conn: ConnPool,
}
```

`ConnPool` is an internal round-robin pool of `Mutex<Connection>` slots (default pool size configured via `POOL_SIZE`). WAL mode allows concurrent reads across pooled connections; writers serialize through their slot's mutex. The pool is grown in `open()` after the schema exists; `in_memory()` uses a single-slot pool. All methods acquire a lock via `self.conn.lock()` before executing SQL.

### 2.2 Constructors

#### `open(path: &Path) -> anyhow::Result<Self>`

Opens or creates the SQLite database at the given path:

1. Creates the parent directory (`create_dir_all`) if it does not exist
2. Opens `Connection::open(path)`
3. Sets the PRAGMA modes:
   ```sql
   PRAGMA journal_mode=WAL;
   PRAGMA synchronous=NORMAL;
   PRAGMA foreign_keys=ON;
   PRAGMA busy_timeout=5000;
   ```
   - **WAL** (Write-Ahead Logging) — allows concurrent reads during writes
   - **synchronous=NORMAL** — a balance between speed and reliability
   - **foreign_keys=ON** — enables foreign key enforcement
   - **busy_timeout=5000** — waits 5 seconds when blocked by another transaction
4. Calls `init_schema()`

#### `in_memory() -> anyhow::Result<Self>`

Creates an in-memory database (for tests). PRAGMA settings are not applied (not needed for in-memory). The schema is initialized the same way.

### 2.3 Schema initialization — `init_schema()`

Executes `CREATE TABLE IF NOT EXISTS` for all tables + indexes + migration:

#### `sessions` table
```sql
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    query TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'running',
    output_dir TEXT,
    error TEXT,
    total_tokens INTEGER DEFAULT 0,
    total_agents INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

#### `agents` table
```sql
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    parent_id TEXT REFERENCES agents(id),
    role TEXT NOT NULL,
    task TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'spawned',
    depth INTEGER NOT NULL DEFAULT 0,
    tokens_used INTEGER DEFAULT 0,
    summary TEXT,
    created_at TEXT NOT NULL,
    completed_at TEXT
);
```

#### `messages` table
```sql
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL REFERENCES agents(id),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tool_calls TEXT,
    created_at TEXT NOT NULL
);
```

#### `findings` table
```sql
CREATE TABLE IF NOT EXISTS findings (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agents(id),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    sources TEXT,
    confidence REAL DEFAULT 0.5,
    created_at TEXT NOT NULL
);
```

#### `tool_results` table
```sql
CREATE TABLE IF NOT EXISTS tool_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL REFERENCES agents(id),
    tool_name TEXT NOT NULL,
    input TEXT NOT NULL,
    output TEXT NOT NULL,
    success INTEGER NOT NULL,
    duration_ms INTEGER,
    created_at TEXT NOT NULL
);
```

#### `subtasks` table
```sql
CREATE TABLE IF NOT EXISTS subtasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    task TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    result TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

#### Indexes
```sql
CREATE INDEX IF NOT EXISTS idx_agents_session ON agents(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_agent ON messages(agent_id);
CREATE INDEX IF NOT EXISTS idx_findings_agent ON findings(agent_id);
CREATE INDEX IF NOT EXISTS idx_subtasks_session ON subtasks(session_id);
```

#### Migration
After tables are created, `add_column_if_missing(&conn, "sessions", "error", "TEXT")` is called for backward compatibility with older databases.

### 2.4 `add_column_if_missing()` — idempotent migration

```rust
pub(crate) fn add_column_if_missing(
    conn: &Connection, table: &str, column: &str, sql_type: &str
) -> rusqlite::Result<()>
```

Algorithm:
1. Runs `PRAGMA table_info({table})` to get the list of columns
2. Extracts column names (index 1 in the PRAGMA result)
3. If the column is missing — runs `ALTER TABLE {table} ADD COLUMN {column} {sql_type}`
4. If the column already exists — does nothing

This makes it safe to run the migration multiple times (idempotency).

### 2.5 CRUD methods for sessions

#### `create_session(id, query)`
```sql
INSERT INTO sessions (id, query, status, created_at, updated_at)
VALUES (?1, ?2, 'running', ?3, ?3)
```
The timestamp is generated via `chrono::Utc::now().to_rfc3339()`.

#### `set_session_output_dir(id, output_dir)`
```sql
UPDATE sessions SET output_dir=?2, updated_at=?3 WHERE id=?1
```
Writes the output directory right after the session is created, so an interrupted session can be found and resumed.

#### `complete_session(id, output_dir, total_tokens, total_agents)`
```sql
UPDATE sessions SET status='completed', output_dir=?2, total_tokens=?3,
       total_agents=?4, updated_at=?5 WHERE id=?1
```

#### `fail_session(id, error)`
```sql
UPDATE sessions SET status='failed', error=?2, updated_at=?3 WHERE id=?1
```
The `output_dir` field is preserved (written when the session starts).

#### `touch_session(id)` — heartbeat
```sql
UPDATE sessions SET updated_at=?2 WHERE id=?1
```
Updates `updated_at`. `SessionResumer` considers running sessions without a fresh heartbeat as interrupted.

#### `claim_session_for_resume(id)` — atomic session acquisition
```sql
UPDATE sessions SET status='resuming', updated_at=?2
WHERE id=?1 AND status='running'
```
Returns `true` if at least one row was updated (`n > 0`). Atomicity is guaranteed because the `UPDATE ... WHERE status='running'` succeeds for only one concurrent resumer — the second one gets `n == 0`.

#### `cancel_session(id)`
```sql
UPDATE sessions SET status='cancelled', updated_at=?2
WHERE id=?1 AND status='running'
```
Returns `true` if the session was running and got cancelled. A repeated cancellation returns `false`.

#### `list_sessions()`
```sql
SELECT id, query, status, output_dir, total_tokens, total_agents,
       created_at, updated_at, error
FROM sessions ORDER BY updated_at DESC, created_at DESC
```

#### `search_sessions(needle)`
```sql
SELECT ... FROM sessions
WHERE query LIKE ?1 ESCAPE '\'
ORDER BY updated_at DESC, created_at DESC
```
Pattern: `%needle%` with escaping of LIKE special characters (`%`, `_`, `\`).

#### `get_session(id)`
```sql
SELECT ... FROM sessions WHERE id = ?1
```
Returns `Option<SessionRow>` or `None` if not found.

#### `list_sessions_with_status(status)`
```sql
SELECT ... FROM sessions WHERE status = ?1 ORDER BY updated_at DESC
```

#### `find_session_by_query(query)`
```sql
SELECT id FROM sessions WHERE query = ?1 AND status = 'running'
ORDER BY created_at DESC LIMIT 1
```
Finds a running session with an exact query match (for resumption).

### 2.6 CRUD methods for agents

#### `create_agent(agent: &AgentRecord)`
```sql
INSERT INTO agents (id, session_id, parent_id, role, task, status, depth,
                    tokens_used, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
```
- `status` is serialized as `format!("{:?}", status).to_lowercase()` (e.g., `"spawned"`)
- `role` is serialized via `.to_string()` (e.g., `"researcher"`)

#### `update_agent_status(id, status, tokens_used, summary)`
```sql
UPDATE agents SET status=?2, tokens_used=?3, summary=?4,
       completed_at=CASE WHEN ?5 THEN ?6 ELSE completed_at END
WHERE id=?1
```
- Parameter `?5` — `bool completed` (true if the status is `Completed`, `Failed`, or `Cancelled`)
- `completed_at` is set only on transition to a terminal state

#### `get_session_agents(session_id)`
```sql
SELECT id, task FROM agents WHERE session_id = ?1
```
Returns `Vec<(String, String)>` — pairs of (id, task).

#### `get_session_agent_rows(session_id)`
```sql
SELECT id, task, status, tokens_used, summary
FROM agents WHERE session_id = ?1 ORDER BY id
```
Returns `Vec<AgentRow>` with fields for the resume mechanism.

#### `count_session_agents(session_id)`
Two separate queries:
```sql
SELECT COUNT(*) FROM agents WHERE session_id = ?1
SELECT COUNT(*) FROM agents WHERE session_id = ?1 AND status = 'completed'
```
Returns `(total, completed)`.

#### `list_agents()`
```sql
SELECT id, session_id, parent_id, role, task, status, depth,
       tokens_used, summary, created_at, completed_at
FROM agents ORDER BY created_at DESC, id DESC
```

#### `get_agent(id)` / `get_session_agents_detail(session_id)`
Use the `AGENT_DETAIL_COLS` constant with the full set of columns. `get_session_agents_detail` sorts by `created_at ASC, id ASC`.

### 2.7 CRUD methods for messages

#### `add_message(agent_id, message: &pr_core::Message)`
Match on the enum variant:
- `System` → `role="system"`, content = the string, tool_calls = NULL
- `User` → `role="user"`, content = the string, tool_calls = NULL
- `Assistant` → `role="assistant"`, content = `content.unwrap_or_default()`, tool_calls = `Some(json)` (if not empty)
- `Tool` → `role="tool"`, content = `"[{call_id}] {content}"`, tool_calls = NULL

```sql
INSERT INTO messages (agent_id, role, content, tool_calls, created_at)
VALUES (?1, ?2, ?3, ?4, ?5)
```

#### `get_agent_messages(agent_id)`
```sql
SELECT role, content, tool_calls FROM messages WHERE agent_id=?1 ORDER BY id
```
Reverse deserialization:
- `"system"` → `Message::system(content)`
- `"user"` → `Message::user(content)`
- `"assistant"` → parses `tool_calls` from JSON, creates `Message::assistant_with_tools`
- `"tool"` → parses `"[call_id] content"`, extracts the `call_id` and `content`

### 2.8 CRUD methods for findings

#### `add_finding(finding: &Finding)`
```sql
INSERT INTO findings (id, agent_id, title, content, sources, confidence, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
```
`sources` is serialized to JSON via `serde_json::to_string`.

#### `get_session_findings(session_id)`
```sql
SELECT f.id, f.agent_id, f.title, f.content, f.sources, f.confidence, f.created_at
FROM findings f JOIN agents a ON f.agent_id = a.id
WHERE a.session_id = ?1 ORDER BY f.created_at
```
The JOIN is needed because findings reference agents, not sessions directly.

### 2.9 Tool results

#### `add_tool_result(agent_id, tool_name, input, output, duration_ms)`
```sql
INSERT INTO tool_results (agent_id, tool_name, input, output, success, duration_ms, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
```
- `input` — `serde_json::Value`, serialized via `.to_string()`
- `success` — `bool` → `i32` (0/1)

### 2.10 Subtasks (Goal Mode light)

#### `add_subtask(session_id, task)`
```sql
INSERT INTO subtasks (session_id, task, status, created_at, updated_at)
VALUES (?1, ?2, 'pending', ?3, ?3)
```
Returns `last_insert_rowid()`.

#### `update_subtask_status(session_id, task, status, result)`
```sql
UPDATE subtasks SET status=?3, result=?4, updated_at=?5
WHERE session_id=?1 AND task=?2
```
Updates by the `(session_id, task)` pair — the task acts as a natural key within the session.

#### `list_subtasks(session_id)`
```sql
SELECT id, task, status, result, created_at, updated_at
FROM subtasks WHERE session_id = ?1 ORDER BY id
```

### 2.11 Helper data types

#### `SessionRow`
```rust
pub struct SessionRow {
    pub id: String,
    pub query: String,
    pub status: String,
    pub output_dir: Option<String>,
    pub total_tokens: i64,
    pub total_agents: i64,
    pub created_at: String,
    pub updated_at: String,
    pub error: Option<String>,
}
```

#### `AgentRow` — for resume
```rust
pub struct AgentRow {
    pub id: String,
    pub task: String,
    pub status: String,
    pub tokens_used: i64,
    pub summary: Option<String>,
}
```

#### `AgentDetailRow` — full row for the HTTP API
```rust
pub struct AgentDetailRow {
    pub id: String,
    pub session_id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub task: String,
    pub status: String,
    pub depth: i64,
    pub tokens_used: i64,
    pub summary: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}
```
The `serde::Serialize` derivation allows returning it directly as JSON.

#### `SubtaskRow`
```rust
pub struct SubtaskRow {
    pub id: i64,
    pub task: String,
    pub status: String,
    pub result: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

---

## 3. `contacts.rs` — `ContactDb` (SQLite)

### 3.1 Structure

```rust
pub struct ContactDb {
    conn: Mutex<Connection>,
}
```

Like `Persistence`, a wrapper with `Mutex<Connection>`.

### 3.2 Constructors

#### `open(path: &Path)`
1. Creates the directory (with the `!parent.as_os_str().is_empty()` check)
2. Opens the connection
3. Sets the PRAGMA: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `busy_timeout=5000`
4. Calls `init_schema()`

#### `in_memory()`
In-memory database with `foreign_keys=ON`.

### 3.3 Schema — 5 tables

#### `contacts` table
```sql
CREATE TABLE IF NOT EXISTS contacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT,
    phone TEXT,
    phone_norm TEXT,
    name TEXT,
    title TEXT,
    company TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'unknown',
    crm_id TEXT
);
```
- `phone_norm` — only the phone digits, for indexed search
- `crm_id` — CRM identifier after synchronization

#### `social_profiles` table
```sql
CREATE TABLE IF NOT EXISTS social_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    url TEXT NOT NULL DEFAULT '',
    username TEXT NOT NULL DEFAULT ''
);
```

#### `companies` table
```sql
CREATE TABLE IF NOT EXISTS companies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    website TEXT,
    industry TEXT,
    size TEXT,
    location TEXT,
    description TEXT
);
```

#### `tags` table
```sql
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    UNIQUE (contact_id, tag)
);
```
The `UNIQUE (contact_id, tag)` constraint prevents duplicate tags.

#### `notes` table
```sql
CREATE TABLE IF NOT EXISTS notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    note TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

#### Indexes
```sql
CREATE INDEX IF NOT EXISTS idx_contacts_email ON contacts(email);
CREATE INDEX IF NOT EXISTS idx_contacts_phone_norm ON contacts(phone_norm);
CREATE INDEX IF NOT EXISTS idx_social_contact ON social_profiles(contact_id);
CREATE INDEX IF NOT EXISTS idx_tags_contact ON tags(contact_id);
CREATE INDEX IF NOT EXISTS idx_notes_contact ON notes(contact_id);
```

#### Migrations
```rust
add_column_if_missing(&conn, "contacts", "crm_id", "TEXT")?;
add_column_if_missing(&conn, "contacts", "phone_norm", "TEXT")?;
```

### 3.4 `add_contact(contact: &Contact) -> i64`

Inserts a contact in a transaction:

1. Normalizes the email via `normalize_email` (trim + lowercase)
2. Trims and filters empty phone
3. Computes `phone_norm = normalize_phone(phone)` — only digits
4. INSERT into `contacts`, gets `contact_id` via `last_insert_rowid()`
5. For each `social_profile` — INSERT into `social_profiles`
6. For each tag — `INSERT OR IGNORE INTO tags` (deduplication via UNIQUE)
7. For each note — INSERT into `notes`
8. `tx.commit()`

### 3.5 `save_deduped(contact) -> (i64, bool)` — atomic deduplication

**Step-by-step algorithm:**

1. Opens a transaction (locks the `Mutex`)
2. Normalizes email and phone
3. **Looks up an existing contact by email:**
   ```sql
   SELECT id FROM contacts WHERE email = ?1 ORDER BY id LIMIT 1
   ```
4. **If not found — looks up by phone_norm:**
   ```sql
   SELECT id FROM contacts WHERE phone_norm = ?1 ORDER BY id LIMIT 1
   ```
   (skipped if `phone_norm` is empty)
5. **Inserts a new row** (always — even on duplicate)
6. Inserts social_profiles, tags, notes for the new record
7. **If no existing contact is found** — commits, returns `(new_id, false)`
8. **If an existing contact is found** — performs a merge:
   - Loads the old contact's fields (`email, phone, name, title, company, crm_id`)
   - For each field: `merged = old.or_else(|| new)` (old takes priority)
   - UPDATEs the old contact with merged values
   - Copies social_profiles from the new contact to the old one (INSERT ... SELECT ... WHERE NOT EXISTS — avoids duplicates)
   - Deletes the new contact's social_profiles
   - Copies tags (INSERT OR IGNORE)
   - Deletes the new contact's tags
   - Moves notes (UPDATE contact_id)
   - Deletes the new contact
   - Commits, returns `(old_id, true)`

The whole operation runs in a single transaction, which prevents TOCTOU races on concurrent calls.

### 3.6 `merge_contacts(primary_id, duplicate_id)`

The external version of merge (invoked by the user through the UI):

1. Checks `primary_id != duplicate_id`
2. Loads both contacts via `get_contact`
3. Opens a transaction
4. Fills primary's empty fields from duplicate (the same `or_else` logic)
5. UPDATEs primary with merged values
6. Copies unique social_profiles
7. Copies tags (INSERT OR IGNORE)
8. Moves notes
9. DELETEs duplicate
10. Commits

### 3.7 `search(query: &str) -> Vec<Contact>`

```sql
SELECT DISTINCT c.id, c.email, c.phone, c.name, c.title, c.company,
       c.created_at, c.updated_at, c.source, c.crm_id
FROM contacts c
LEFT JOIN tags t ON t.contact_id = c.id
WHERE c.name LIKE ?1 ESCAPE '\'
   OR c.email LIKE ?1 ESCAPE '\'
   OR c.phone LIKE ?1 ESCAPE '\'
   OR c.title LIKE ?1 ESCAPE '\'
   OR c.company LIKE ?1 ESCAPE '\'
   OR t.tag LIKE ?1 ESCAPE '\'
ORDER BY c.id DESC
```

- Pattern: `%escaped_needle%`
- `escape_like()` escapes `\` → `\\`, `%` → `\%`, `_` → `\_`
- Empty query → delegates to `list_all(usize::MAX, 0)`
- Results are loaded via `attach_extras` (batch loading)

### 3.8 `list_all(limit, offset)`

```sql
SELECT ... FROM contacts c ORDER BY c.id DESC LIMIT ?1 OFFSET ?2
```
Newest contacts first. Results via `attach_extras`.

### 3.9 `find_by_email(email)`

```rust
let email = normalize_email(email);
// ...
SELECT id FROM contacts WHERE email = ?1 ORDER BY id LIMIT 1
```
Returns `Option<Contact>` (loaded via `get_contact`).

### 3.10 `find_by_phone(phone)` — lazy backfill

**Fast path:**
```sql
SELECT id FROM contacts WHERE phone_norm = ?1 ORDER BY id LIMIT 1
```

**Fallback (legacy rows):**
If the index yields no results:
```sql
SELECT id, phone FROM contacts
WHERE phone IS NOT NULL AND phone_norm IS NULL
ORDER BY id
```
For each row:
1. Computes `normalize_phone(phone)` in Rust
2. UPDATEs `phone_norm` in the DB (lazy backfill)
3. If norm == needle — remembers the id

After the first call, all legacy rows have `phone_norm`, and subsequent calls use the index.

### 3.11 Batch loading — `attach_extras`

The function `attach_extras(conn, contacts) -> Vec<Contact>`:

1. Collects a `Vec<i64>` of contact ids
2. Builds a `HashMap<i64, usize>` for positioning
3. Splits ids into batches of `ID_BATCH_SIZE = 500`
4. For each batch runs **3 queries**:
   - `SELECT contact_id, platform, url, username FROM social_profiles WHERE contact_id IN (...) ORDER BY id`
   - `SELECT contact_id, tag FROM tags WHERE contact_id IN (...) ORDER BY id`
   - `SELECT contact_id, note FROM notes WHERE contact_id IN (...) ORDER BY id`
5. Distributes results to contacts via the `position` map

This replaces the N+1 pattern (3 queries per contact) with 3 queries per batch.

### 3.12 Other methods

#### `add_tag(contact_id, tag)`
```sql
INSERT OR IGNORE INTO tags (contact_id, tag) VALUES (?1, ?2)
```
Checks that the tag is not empty.

#### `delete_contact(contact_id)`
```sql
DELETE FROM contacts WHERE id = ?1
```
Cascading deletion (ON DELETE CASCADE) removes social_profiles, tags, notes.

#### `set_crm_id(contact_id, crm_id)`
```sql
UPDATE contacts SET crm_id=?2, updated_at=?3 WHERE id=?1
```

#### `add_note(contact_id, note)`
```sql
INSERT INTO notes (contact_id, note, created_at) VALUES (?1, ?2, ?3)
```

#### `upsert_company(company: &Company)`
```sql
INSERT INTO companies (name, website, industry, size, location, description)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
ON CONFLICT(name) DO UPDATE SET
    website = excluded.website,
    industry = excluded.industry,
    size = excluded.size,
    location = excluded.location,
    description = excluded.description
```
Then `SELECT id FROM companies WHERE name = ?1` to get the id.

#### `count()`
```sql
SELECT COUNT(*) FROM contacts
```

#### `list_companies()`
```sql
SELECT id, name, website, industry, size, location, description
FROM companies ORDER BY name
```

#### `find_duplicates() -> Vec<(Contact, Contact)>`

Algorithm:
1. Loads all contacts via `list_all(usize::MAX, 0)`
2. Builds a `HashMap<String, Vec<i64>>` keyed by normalized emails and phones
3. For each group with ≥2 items, generates all `(older, newer)` pairs
4. Deduplicates and sorts the pairs
5. Returns the pairs of `Contact` objects

---

## 4. `pg.rs` — `PgContactDb` (PostgreSQL)

### 4.1 Structure and connection

```rust
pub struct PgContactDb {
    pool: Pool,  // deadpool_postgres::Pool
}
```

#### `connect(url: &str)`
1. Creates a `Config` from the URL
2. Creates the pool via `cfg.create_pool(Some(Runtime::Tokio1), NoTls)`
3. Calls `init_schema().await`

#### `from_pool(pool: Pool)`
Constructor from an existing pool.

### 4.2 Schema — key differences from SQLite

- `BIGSERIAL PRIMARY KEY` types instead of `INTEGER PRIMARY KEY AUTOINCREMENT`
- `BIGINT NOT NULL REFERENCES ... ON DELETE CASCADE` instead of `INTEGER NOT NULL REFERENCES ...`
- Migration: `ALTER TABLE contacts ADD COLUMN IF NOT EXISTS crm_id TEXT` (PostgreSQL-native syntax)

```sql
CREATE TABLE IF NOT EXISTS contacts (
    id BIGSERIAL PRIMARY KEY,
    email TEXT,
    phone TEXT,
    phone_norm TEXT,
    ...
);
```
Analogous for `social_profiles`, `companies`, `tags`, `notes`.

### 4.3 Key API differences

- **All methods are async** — use `client.query().await`, `tx.execute().await`
- **Placeholders**: `$1, $2, ...` instead of `?1, ?2, ...`
- **INSERT ... RETURNING id** instead of `last_insert_rowid()`
- **INSERT ... ON CONFLICT (contact_id, tag) DO NOTHING** instead of `INSERT OR IGNORE`
- **ILIKE** instead of `LIKE` for case-insensitive search
- **LIMIT NULL** means "no limit" in PostgreSQL
- **in_placeholders(start, count)** — generates `$1, $2, ...` placeholders
- **ID_BATCH_SIZE = 1000** (vs 500 for SQLite; PostgreSQL allows up to 65535 parameters)
- **add_tag**: checks `n > 0` after INSERT; if 0 — checks that the contact exists (to distinguish "duplicate tag" from "nonexistent contact")
- **search**: uses `ILIKE` instead of `LIKE`

### 4.4 Pooling via deadpool

All queries get a client from the pool: `self.pool.get().await?`. Transactions are created via `client.transaction().await?`.

---

## 5. `store.rs` — `ContactStore` trait

### 5.1 The `ContactStore` trait

```rust
#[async_trait]
pub trait ContactStore: Send + Sync {
    async fn add_contact(&self, contact: &Contact) -> anyhow::Result<i64>;
    async fn get_contact(&self, id: i64) -> anyhow::Result<Option<Contact>>;
    async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<Contact>>;
    async fn find_by_phone(&self, phone: &str) -> anyhow::Result<Option<Contact>>;
    async fn search(&self, query: &str) -> anyhow::Result<Vec<Contact>>;
    async fn list_all(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<Contact>>;
    async fn count(&self) -> anyhow::Result<i64>;
    async fn merge_contacts(&self, primary_id: i64, duplicate_id: i64) -> anyhow::Result<()>;
    async fn set_crm_id(&self, contact_id: i64, crm_id: &str) -> anyhow::Result<()>;
    async fn delete_contact(&self, contact_id: i64) -> anyhow::Result<()>;
    async fn save_deduped(&self, contact: &Contact) -> anyhow::Result<(i64, bool)>;
    fn backend(&self) -> &'static str;
}
```

A unified async interface for both backends. Tools, CLI, and CRM synchronization work through this trait.

### 5.2 Implementation for `ContactDb` (SQLite)

Synchronous methods are wrapped in async:
```rust
async fn add_contact(&self, contact: &Contact) -> anyhow::Result<i64> {
    self.add_contact(contact)  // direct call to the sync method
}
```
`backend()` returns `"sqlite"`.

### 5.3 Implementation for `PgContactDb` (PostgreSQL)

A direct delegate to the async methods:
```rust
async fn add_contact(&self, contact: &Contact) -> anyhow::Result<i64> {
    self.add_contact(contact).await
}
```
`backend()` returns `"postgres"`.

### 5.4 `open_contact_store(cfg) -> Arc<dyn ContactStore>`

Factory function:
1. If `cfg.pg_url` is not empty:
   - With the `postgres` feature — creates `PgContactDb::connect(&cfg.pg_url).await?`
   - Without the `postgres` feature — logs a warning, falls back to SQLite
2. Otherwise — creates `ContactDb::open(Path::new(&cfg.db_path))`
3. Returns `Arc<dyn ContactStore>`

---

## 6. `history.rs` — `SessionHistory`

### 6.1 Data structures

#### `SessionSummary`
```rust
pub struct SessionSummary {
    pub id: SessionId,
    pub query: String,
    pub status: String,
    pub output_dir: Option<String>,
    pub total_tokens: i64,
    pub total_agents: i64,
    pub created_at: String,
    pub updated_at: String,
}
```
Implements `From<SessionRow>` (without `error`).

#### `SessionDetails`
```rust
pub struct SessionDetails {
    pub session: SessionSummary,
    pub agents: Vec<AgentDetailRow>,
    pub findings: Vec<Finding>,
}
```

### 6.2 `SessionHistory`

```rust
pub struct SessionHistory {
    db: Arc<Persistence>,
}
```

A read-only facade over `Persistence` for the UI session history.

#### `new(db: Arc<Persistence>)`
Constructor.

#### `list_sessions(limit: usize) -> Vec<SessionSummary>`
Delegates to `db.list_sessions()`, applies `.take(limit)`, converts `SessionRow` → `SessionSummary`. Errors are logged, an empty Vec is returned.

#### `search_sessions(query: &str) -> Vec<SessionSummary>`
Empty query → `list_sessions(usize::MAX)`. Otherwise — `db.search_sessions(query.trim())`.

#### `get_session_details(id: &SessionId) -> Option<SessionDetails>`
1. Loads the `SessionRow` via `db.get_session(id)`
2. Loads agents via `db.get_session_agents_detail(id)`
3. Loads findings via `db.get_session_findings(id)`
4. Assembles the `SessionDetails`
5. Errors on steps 2-3 are logged, empty Vecs are returned

---

## 7. `credentials.rs` — Encrypted Credential Vault

### 7.1 Overview

AES-256-GCM encrypted credential storage, exposed as methods on `Persistence`. Secrets are never stored in plaintext — every write encrypts via `ring::aead::LessSafeKey`; every read decrypts on the fly. The encryption key is loaded from the `FATHOM_CREDENTIAL_KEY` environment variable (hex, base64 standard, or base64 URL-safe, exactly 32 bytes decoded).

### 7.2 `CredentialRow`

```rust
pub struct CredentialRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub created_at: String,
    pub updated_at: String,
}
```

Note: the actual ciphertext is never exposed through `CredentialRow` — `name` is unique and acts as the human-friendly lookup key; `kind` is an arbitrary label (e.g. `"api_key"`, `"oauth_token"`).

### 7.3 Schema

```sql
CREATE TABLE IF NOT EXISTS credentials (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    ciphertext BLOB NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_credentials_updated_at ON credentials(updated_at);
```

### 7.4 Methods (on `Persistence`)

| Method | Signature | SQL / Behaviour |
|--------|-----------|-----------------|
| `store_credential` | `(&self, name, kind, secret) -> Result<CredentialRow>` | Encrypts `secret`, upserts by unique `name` (`ON CONFLICT(name) DO UPDATE`), returns the row |
| `list_credentials` | `(&self) -> Result<Vec<CredentialRow>>` | `SELECT id,name,kind,created_at,updated_at ORDER BY updated_at DESC,id` — ciphertext never returned |
| `delete_credential` | `(&self, id) -> Result<bool>` | `DELETE FROM credentials WHERE id=?1`; returns true if a row was deleted |
| `resolve_secret` | `(&self, id) -> Result<Option<String>>` | Fetches ciphertext blob, decrypts, returns plaintext or `None` if id not found |

### 7.5 Limits

| Constant | Value |
|----------|-------|
| `MAX_NAME` | 128 bytes |
| `MAX_KIND` | 64 bytes |
| `MAX_SECRET` | 65 536 bytes |
| `NONCE_LEN` | 12 bytes (AES-GCM standard) |

---

## 8. `coworkers.rs` — Coworkers & Channels

### 8.1 Overview

Named AI coworker definitions and their conversation channels, persisted as rows on the shared `Persistence` SQLite database. Channels are child rows of a coworker and optionally linked to a session.

### 8.2 Types

#### `CoworkerRow`

```rust
pub struct CoworkerRow {
    pub id: String,
    pub name: String,
    pub title: String,
    pub role: String,
    pub prompt: String,
    pub visibility: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}
```

#### `ChannelRow`

```rust
pub struct ChannelRow {
    pub id: String,
    pub coworker_id: String,
    pub title: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

### 8.3 Schema

```sql
-- (created in db.rs init_schema)
CREATE TABLE IF NOT EXISTS coworkers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    title TEXT NOT NULL,
    role TEXT NOT NULL,
    prompt TEXT NOT NULL,
    visibility TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS channels (
    id TEXT PRIMARY KEY,
    coworker_id TEXT NOT NULL REFERENCES coworkers(id),
    title TEXT NOT NULL,
    session_id TEXT REFERENCES sessions(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### 8.4 Methods (on `Persistence`)

#### Coworker CRUD

| Method | Signature | Notes |
|--------|-----------|-------|
| `create_coworker` | `(&self, name, title, role, prompt, visibility, active) -> Result<CoworkerRow>` | UUIDv7 id, timestamps set |
| `get_coworker` | `(&self, id) -> Result<Option<CoworkerRow>>` | Exact id lookup |
| `list_coworkers` | `(&self) -> Result<Vec<CoworkerRow>>` | `ORDER BY updated_at DESC, id` |
| `update_coworker` | `(&self, id, name, title, role, prompt, visibility, active) -> Result<Option<CoworkerRow>>` | Returns `None` if id not found |
| `delete_coworker` | `(&self, id) -> Result<bool>` | Deletes channels first (cascade), then coworker; returns true if deleted |

#### Channel CRUD

| Method | Signature | Notes |
|--------|-----------|-------|
| `create_channel` | `(&self, coworker_id, title, session_id) -> Result<ChannelRow>` | Validates coworker exists; validates session exists if provided |
| `get_channel` | `(&self, id) -> Result<Option<ChannelRow>>` | Exact id lookup |
| `list_channels` | `(&self, coworker_id) -> Result<Vec<ChannelRow>>` | `ORDER BY updated_at DESC, id` |
| `update_channel` | `(&self, id, title, session_id) -> Result<Option<ChannelRow>>` | Returns `None` if id not found |
| `delete_channel` | `(&self, id) -> Result<bool>` | Returns true if a row was deleted |

### 8.5 Field limits

| Field | Max length |
|-------|-----------|
| `id` | 128 |
| `name` | 200 |
| `title` | 200 |
| `role` | 100 |
| `prompt` | 32 000 |
| `visibility` | 32 |

---

## 9. `jobs.rs` — Durable Background-Job Registry

### 9.1 Overview

`JobsDb` is a **standalone** SQLite database (separate from the main `Persistence` database, default path `~/.fathom/jobs.db`, overridable via `PR_JOBS_DB` env var). Jobs survive process restarts: the runner (`fathom job-run <id>`) updates its row as attempts start, fail, and complete. Failed attempts are retried with an augmented task carrying the previous error so the agent can diagnose and fix its own failure.

### 9.2 `JobRow`

```rust
pub struct JobRow {
    pub id: String,
    pub task: String,
    pub status: String,
    pub attempt: i64,
    pub max_attempts: i64,
    pub output_dir: String,
    pub error: Option<String>,
    pub pid: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}
```

`is_terminal()` returns true when `status` is `"completed"`, `"failed"`, or `"cancelled"`.

### 9.3 Schema

```sql
CREATE TABLE IF NOT EXISTS jobs (
    id            TEXT PRIMARY KEY,
    task          TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'queued',
    attempt       INTEGER NOT NULL DEFAULT 0,
    max_attempts  INTEGER NOT NULL DEFAULT 1,
    output_dir    TEXT NOT NULL,
    error         TEXT,
    pid           INTEGER,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    started_at    TEXT,
    completed_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs (status);
```

### 9.4 `JobsDb` struct

```rust
pub struct JobsDb {
    conn: Mutex<Connection>,
}
```

Open via `JobsDb::open(path)` or `JobsDb::in_memory()` (tests).

### 9.5 Methods

| Method | Signature | Notes |
|--------|-----------|-------|
| `create` | `(&self, task, max_attempts, output_dir) -> Result<JobRow>` | Status starts as `"queued"`, attempt = 0 |
| `get` | `(&self, id) -> Result<Option<JobRow>>` | Supports exact match and unique prefix lookup; ambiguous prefix → error |
| `list` | `(&self) -> Result<Vec<JobRow>>` | `ORDER BY created_at DESC` |
| `set_output_dir` | `(&self, id, dir) -> Result<()>` | Update output dir post-creation |
| `mark_running` | `(&self, id, attempt, pid) -> Result<()>` | Sets `status='running'`, records pid; `started_at` is set once on first attempt only (`COALESCE`) |
| `mark_completed` | `(&self, id) -> Result<()>` | Sets `status='completed'`, clears pid and error |
| `mark_failed` | `(&self, id, error) -> Result<()>` | Sets `status='failed'`, records error |
| `record_attempt_error` | `(&self, id, error) -> Result<()>` | Records non-terminal error text (before retry) |
| `mark_cancelled` | `(&self, id) -> Result<bool>` | Only cancels jobs in `queued`/`running` states; returns true if cancelled |
| `reset_for_rerun` | `(&self, id) -> Result<bool>` | Resets terminal state (`failed`/`cancelled`/`completed`) back to `queued` with attempt=0 |
| `reset_running_with_pid` | `(&self, id, dead_pid) -> Result<bool>` | Resets a `running` job only if the stored pid matches `dead_pid` — guards against racing a live runner |

### 9.6 Free functions

| Function | Purpose |
|----------|---------|
| `default_jobs_db_path()` | Returns `~/.fathom/jobs.db` (or `PR_JOBS_DB` override) |
| `default_jobs_root()` | Returns `~/.fathom/jobs/` (or `PR_JOBS_DIR` override) |
| `pid_alive(pid)` | `kill -0` check |
| `terminate_pid(pid)` | Best-effort SIGTERM |
| `spawn_detached_runner(exe, job_id, log_path)` | Spawns `<exe> job-run <job_id>` fully detached, in its own session on unix; returns child pid |

---

## 10. `replay.rs` — Governed-Action Replay Timeline

### 10.1 Overview

Persists a redacted timeline of governed tool-execution actions. Rows are deliberately separate from governance *audit* decisions: audit records explain *why* an action was allowed or denied; replay rows describe the bounded execution timeline. This module never creates synthetic execution records.

All stored payloads (`args_redacted`, `result_redacted`) are defensively redacted on write via `redact_value()` — secret-key JSON fields (password, token, apikey, etc.) are replaced with `[REDACTED]`.

### 10.2 `ReplayActionRow`

```rust
pub struct ReplayActionRow {
    pub id: String,
    pub agent: String,
    pub session: String,
    pub tool: String,
    pub args_redacted: String,
    pub decision: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub result_redacted: Option<String>,
    pub screenshot_before: Option<String>,
    pub screenshot_after: Option<String>,
    pub policy_version: String,
}
```

### 10.3 Constants

| Constant | Value |
|----------|-------|
| `MAX_REPLAY_LIMIT` | 200 — max rows per query |
| `MAX_REPLAY_TEXT_BYTES` | 64 KiB — max payload size |
| `MAX_REPLAY_FIELD_BYTES` | 2 048 bytes — max identifier/reference length |

### 10.4 Schema (created in `db.rs` `init_schema`)

```sql
CREATE TABLE IF NOT EXISTS replay_actions (
    id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    session TEXT NOT NULL,
    tool TEXT NOT NULL,
    args_redacted TEXT NOT NULL,
    decision TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    duration_ms INTEGER,
    result_redacted TEXT,
    screenshot_before TEXT,
    screenshot_after TEXT,
    policy_version TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_replay_actions_started ON replay_actions(started_at);
CREATE INDEX IF NOT EXISTS idx_replay_actions_session ON replay_actions(session);
CREATE INDEX IF NOT EXISTS idx_replay_actions_agent ON replay_actions(agent);
```

### 10.5 Methods (on `Persistence`)

| Method | Signature | Notes |
|--------|-----------|-------|
| `record_replay_action` | `(&self, action: &ReplayActionRow) -> Result<()>` | Validates all required fields, redacts JSON payloads, rejects negative `duration_ms` and sensitive screenshot references; uses `INSERT OR REPLACE` |
| `list_replay_actions` | `(&self, session, agent, limit) -> Result<Vec<ReplayActionRow>>` | Optional `session`/`agent` filters; bounded by `MAX_REPLAY_LIMIT`; returns newest-first |

### 10.6 Redaction logic

`redact_value()` recursively walks JSON:
- Object keys matching `password`, `passwd`, `secret`, `token`, `apikey`, `authorization`, `credential`, `privatekey`, `accesskey`, `clientsecret`, `cookie` (case-insensitive) → replaced with `[REDACTED]`
- String values containing those markers → `[REDACTED]`
- Non-JSON results with secret markers are rejected outright (callers must pre-redact)

---

## 11. `schedules.rs` — Cron Schedules & Due-Claim

### 11.1 Overview

Cron-based schedule definitions attached to coworkers. Each schedule has a five-field cron expression, an IANA timezone (or UTC/fixed offset), and an optional explicit `next_run` timestamp. The `claim_due_schedules` method atomically fetches and advances due schedules in a single transaction, making it safe for multiple scheduler processes.

### 11.2 `ScheduleRow`

```rust
pub struct ScheduleRow {
    pub id: String,
    pub coworker_id: String,
    pub cron_expression: String,
    pub timezone: String,
    pub query: String,
    pub enabled: bool,
    pub next_run: String,
    pub last_run: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

### 11.3 Schema (created lazily in `ensure_schema`)

```sql
CREATE TABLE IF NOT EXISTS schedules (
    id TEXT PRIMARY KEY,
    coworker_id TEXT NOT NULL,
    cron_expression TEXT NOT NULL,
    timezone TEXT NOT NULL,
    query TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    next_run TEXT NOT NULL,
    last_run TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_schedules_due ON schedules(enabled, next_run);
CREATE INDEX IF NOT EXISTS idx_schedules_coworker ON schedules(coworker_id);
```

### 11.4 Methods (on `Persistence`)

| Method | Signature | Notes |
|--------|-----------|-------|
| `create_schedule` | `(&self, coworker_id, cron_expression, timezone, query, enabled, next_run) -> Result<ScheduleRow>` | Validates cron, timezone, and that `coworker_id` exists; auto-computes `next_run` if not provided |
| `list_schedules` | `(&self) -> Result<Vec<ScheduleRow>>` | `ORDER BY next_run, id` |
| `get_schedule` | `(&self, id) -> Result<Option<ScheduleRow>>` | Exact id lookup |
| `update_schedule` | `(&self, id, coworker_id, cron_expression, timezone, query, enabled, next_run) -> Result<Option<ScheduleRow>>` | Re-validates all fields; returns `None` if id not found |
| `delete_schedule` | `(&self, id) -> Result<bool>` | Returns true if a row was deleted |
| `claim_due_schedules` | `(&self, now, limit) -> Result<Vec<ScheduleRow>>` | Atomically selects enabled schedules where `next_run <= now`, advances `next_run` to the next cron occurrence, records `last_run`, all within one transaction; `limit` capped at `MAX_CLAIM = 100` |

### 11.5 Validation helpers (module-level functions)

| Function | Purpose |
|----------|---------|
| `validate_cron(expr)` | Validates a five-field cron expression (minute, hour, day-of-month, month, day-of-week); rejects `@`-prefixed shorthand; supports `*`, ranges, steps |
| `validate_timezone(tz)` | Accepts `"UTC"`, `"Etc/UTC"`, fixed offsets (`UTC+5`, `UTC-3:30`), `Etc/GMT+N`/`Etc/GMT-N`, or verifies `/usr/share/zoneinfo` path exists; rejects path traversal |

### 11.6 Field limits

| Field | Max length |
|-------|-----------|
| `id` | 128 |
| `cron_expression` | 256 |
| `timezone` | 128 |
| `query` | 20 000 |

---

## 12. Common patterns

### 12.1 Thread safety
`Persistence` uses a `ConnPool` (round-robin of `Mutex<Connection>` slots) — all methods acquire a slot lock before executing SQL. `ContactDb` and `JobsDb` each hold a single `Mutex<Connection>`.

### 12.2 Timestamps
All `created_at`/`updated_at` values are stored as ISO 8601/RFC 3339 strings (`chrono::Utc::now().to_rfc3339()`).

### 12.3 Error handling
- `db.rs` and `contacts.rs` return `anyhow::Result`
- `ContactStore` trait errors — `anyhow::Result`
- `SessionHistory` degrades gracefully: errors are logged, empty values are returned

### 12.4 Contact deduplication
Normalization via `normalize_email` (trim + lowercase) and `normalize_phone` (digits only). The index on `phone_norm` provides fast lookup.
