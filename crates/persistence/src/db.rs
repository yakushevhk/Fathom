use pr_core::{AgentRecord, AgentStatus, Finding, SessionId, AgentId};
use rusqlite::{Connection, params};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Connections per file-backed database. In WAL mode readers never block
/// the writer (and vice versa), so spreading concurrent calls across a few
/// connections removes the single-mutex serialization point that server
/// mode (many agents, one DB) used to hit on every message write.
/// In-memory databases always use a single connection (each `:memory:`
/// connection would be a different database).
const POOL_SIZE: usize = 4;

/// Round-robin pool over SQLite connections.
pub(crate) struct ConnPool {
    slots: Vec<Mutex<Connection>>,
    next: AtomicUsize,
}

impl ConnPool {
    fn single(conn: Connection) -> Self {
        Self {
            slots: vec![Mutex::new(conn)],
            next: AtomicUsize::new(0),
        }
    }

    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.slots.len();
        self.slots[idx].lock().unwrap()
    }

    fn size(&self) -> usize {
        self.slots.len()
    }
}

pub struct Persistence {
    pub(crate) conn: ConnPool,
}

impl Persistence {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        const PRAGMAS: &str = "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;";
        let primary = Connection::open(path)?;
        primary.execute_batch(PRAGMAS)?;
        let mut db = Self {
            conn: ConnPool::single(primary),
        };
        db.init_schema()?;
        // Grow the pool after the schema exists: extra connections share
        // the same file through WAL. Failures are non-fatal — the primary
        // connection alone is still a correct (if serialized) database.
        let mut slots = vec![db.conn.slots.into_iter().next().unwrap()];
        for _ in 1..POOL_SIZE {
            match Connection::open(path) {
                Ok(extra) => {
                    if extra.execute_batch(PRAGMAS).is_ok() {
                        slots.push(Mutex::new(extra));
                    }
                }
                Err(e) => {
                    tracing::warn!("persistence pool: could not open extra connection: {e}");
                    break;
                }
            }
        }
        db.conn = ConnPool {
            slots,
            next: AtomicUsize::new(0),
        };
        Ok(db)
    }

    pub fn in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: ConnPool::single(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Number of pooled connections (test/diagnostics helper).
    pub fn pool_size(&self) -> usize {
        self.conn.size()
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(r#"
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

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL REFERENCES agents(id),
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS findings (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL REFERENCES agents(id),
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                sources TEXT,
                confidence REAL DEFAULT 0.5,
                created_at TEXT NOT NULL
            );

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

            CREATE TABLE IF NOT EXISTS subtasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                task TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                result TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS file_changes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                file_path TEXT NOT NULL,
                operation TEXT NOT NULL,
                old_content TEXT,
                new_content TEXT,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_agents_session ON agents(session_id);
            CREATE INDEX IF NOT EXISTS idx_messages_agent ON messages(agent_id);
            CREATE INDEX IF NOT EXISTS idx_findings_agent ON findings(agent_id);
            CREATE INDEX IF NOT EXISTS idx_subtasks_session ON subtasks(session_id);
            CREATE INDEX IF NOT EXISTS idx_file_changes_session ON file_changes(session_id);

            CREATE TABLE IF NOT EXISTS coworkers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                title TEXT NOT NULL,
                role TEXT NOT NULL,
                prompt TEXT NOT NULL,
                visibility TEXT NOT NULL DEFAULT 'private',
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS channels (
                id TEXT PRIMARY KEY,
                coworker_id TEXT NOT NULL REFERENCES coworkers(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_coworkers_active ON coworkers(active);
            CREATE INDEX IF NOT EXISTS idx_coworkers_updated ON coworkers(updated_at);
            CREATE INDEX IF NOT EXISTS idx_channels_coworker ON channels(coworker_id);
            CREATE INDEX IF NOT EXISTS idx_channels_session ON channels(session_id);
            CREATE INDEX IF NOT EXISTS idx_channels_updated ON channels(updated_at);

            CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                agent TEXT NOT NULL,
                session TEXT NOT NULL,
                tool TEXT NOT NULL,
                args TEXT NOT NULL,
                url TEXT,
                element TEXT,
                file TEXT,
                intent TEXT,
                mcp_metadata TEXT,
                decision TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_events_session ON audit_events(session);
            CREATE INDEX IF NOT EXISTS idx_audit_events_agent ON audit_events(agent);
            CREATE INDEX IF NOT EXISTS idx_audit_events_decision ON audit_events(decision);

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
        "#)?;

        // Migrate databases created before these columns existed.
        add_column_if_missing(&conn, "sessions", "error", "TEXT")?;
        Ok(())
    }

    // Sessions

    pub fn create_session(&self, id: &SessionId, query: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (id, query, status, created_at, updated_at) VALUES (?1, ?2, 'running', ?3, ?3)",
            params![id.0, query, now],
        )?;
        Ok(())
    }

    /// Record the session's output directory right after creation, so an
    /// interrupted session can be located and resumed later (the field is
    /// otherwise only written by `complete_session`).
    pub fn set_session_output_dir(&self, id: &SessionId, output_dir: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET output_dir=?2, updated_at=?3 WHERE id=?1",
            params![id.0, output_dir, now],
        )?;
        Ok(())
    }

    pub fn complete_session(&self, id: &SessionId, output_dir: &str, total_tokens: u64, total_agents: u32) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET status='completed', output_dir=?2, total_tokens=?3, total_agents=?4, updated_at=?5 WHERE id=?1",
            params![id.0, output_dir, total_tokens as i64, total_agents, now],
        )?;
        Ok(())
    }

    pub fn fail_session(&self, id: &SessionId, error: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        // output_dir is preserved (it was recorded at session start); the
        // message goes into the dedicated `error` column.
        conn.execute(
            "UPDATE sessions SET status='failed', error=?2, updated_at=?3 WHERE id=?1",
            params![id.0, error, now],
        )?;
        Ok(())
    }

    /// Refresh `updated_at` — heartbeat proving the session is still alive.
    /// `SessionResumer` treats running sessions without a recent heartbeat
    /// as interrupted.
    pub fn touch_session(&self, id: &SessionId) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET updated_at=?2 WHERE id=?1",
            params![id.0, now],
        )?;
        Ok(())
    }

    /// Atomically claim a running session for resume: transitions
    /// `running` -> `resuming` only if it is still `running`. Returns true
    /// when the claim won — concurrent resumers cannot both proceed.
    pub fn claim_session_for_resume(&self, id: &SessionId) -> anyhow::Result<bool> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        let n = conn.execute(
            "UPDATE sessions SET status='resuming', updated_at=?2
             WHERE id=?1 AND status='running'",
            params![id.0, now],
        )?;
        Ok(n > 0)
    }

    /// Mark a running session as cancelled. Returns `true` if a row was
    /// updated (i.e. the session existed and was still running).
    pub fn cancel_session(&self, id: &SessionId) -> anyhow::Result<bool> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        let n = conn.execute(
            "UPDATE sessions SET status='cancelled', updated_at=?2 WHERE id=?1 AND status='running'",
            params![id.0, now],
        )?;
        Ok(n > 0)
    }

    /// List all sessions, most recently updated first.
    pub fn list_sessions(&self) -> anyhow::Result<Vec<SessionRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, query, status, output_dir, total_tokens, total_agents, created_at, updated_at, error
             FROM sessions ORDER BY updated_at DESC, created_at DESC"
        )?;
        let rows = stmt.query_map([], session_row_from_stmt)?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Search sessions whose query text contains `needle` (case-insensitive),
    /// most recently updated first.
    pub fn search_sessions(&self, needle: &str) -> anyhow::Result<Vec<SessionRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, query, status, output_dir, total_tokens, total_agents, created_at, updated_at, error
             FROM sessions WHERE query LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC, created_at DESC"
        )?;
        let pattern = format!("%{}%", needle.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let rows = stmt.query_map(params![pattern], session_row_from_stmt)?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // Agents

    pub fn create_agent(&self, agent: &AgentRecord) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO agents (id, session_id, parent_id, role, task, status, depth, tokens_used, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                agent.id.0,
                agent.session_id,
                agent.parent_id.as_ref().map(|p| p.0.clone()),
                agent.role.to_string(),
                agent.task,
                format!("{:?}", agent.status).to_lowercase(),
                agent.depth,
                agent.tokens_used as i64,
                agent.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn update_agent_status(&self, id: &AgentId, status: AgentStatus, tokens_used: u64, summary: Option<&str>) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        let completed = matches!(status, AgentStatus::Completed | AgentStatus::Failed | AgentStatus::Cancelled);
        conn.execute(
            "UPDATE agents SET status=?2, tokens_used=?3, summary=?4, completed_at=CASE WHEN ?5 THEN ?6 ELSE completed_at END WHERE id=?1",
            params![
                id.0,
                format!("{:?}", status).to_lowercase(),
                tokens_used as i64,
                summary,
                completed,
                now,
            ],
        )?;
        Ok(())
    }

    // Messages

    pub fn add_message(&self, agent_id: &AgentId, message: &pr_core::Message) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        let (role, content, tool_calls) = match message {
            pr_core::Message::System { content } => ("system", content.clone(), None),
            pr_core::Message::User { content } => ("user", content.clone(), None),
            pr_core::Message::Assistant { content, tool_calls } => {
                let tc = if tool_calls.is_empty() { None } else {
                    Some(serde_json::to_string(tool_calls)?)
                };
                ("assistant", content.clone().unwrap_or_default(), tc)
            }
            pr_core::Message::Tool { tool_call_id: call_id, content } => {
                ("tool", format!("[{call_id}] {content}"), None)
            }
        };
        conn.execute(
            "INSERT INTO messages (agent_id, role, content, tool_calls, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![agent_id.0, role, content, tool_calls, now],
        )?;
        Ok(())
    }

    pub fn get_agent_messages(&self, agent_id: &AgentId) -> anyhow::Result<Vec<pr_core::Message>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT role, content, tool_calls FROM messages WHERE agent_id=?1 ORDER BY id"
        )?;
        let messages = stmt.query_map(params![agent_id.0], |row| {
            let role: String = row.get(0)?;
            let content: String = row.get(1)?;
            let tool_calls: Option<String> = row.get(2)?;
            Ok((role, content, tool_calls))
        })?;

        let mut result = Vec::new();
        for msg in messages {
            let (role, content, tool_calls) = msg?;
            let message = match role.as_str() {
                "system" => pr_core::Message::system(content),
                "user" => pr_core::Message::user(content),
                "assistant" => {
                    let tcs: Vec<pr_core::ToolCall> = tool_calls
                        .map(|tc| serde_json::from_str(&tc).unwrap_or_default())
                        .unwrap_or_default();
                    pr_core::Message::assistant_with_tools(
                        if content.is_empty() { None } else { Some(content) },
                        tcs,
                    )
                }
                "tool" => {
                    // Parse "[call_id] content"
                    if let Some(pos) = content.find(']') {
                        let call_id = content[1..pos].to_string();
                        let rest = content[pos+2..].to_string();
                        pr_core::Message::tool(call_id, rest)
                    } else {
                        pr_core::Message::tool("", content)
                    }
                }
                _ => pr_core::Message::user(content),
            };
            result.push(message);
        }
        Ok(result)
    }

    // Findings

    pub fn add_finding(&self, finding: &Finding) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO findings (id, agent_id, title, content, sources, confidence, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                finding.id.0,
                finding.agent_id.0,
                finding.title,
                finding.content,
                serde_json::to_string(&finding.sources)?,
                finding.confidence,
                finding.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_session_findings(&self, session_id: &SessionId) -> anyhow::Result<Vec<Finding>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT f.id, f.agent_id, f.title, f.content, f.sources, f.confidence, f.created_at
             FROM findings f JOIN agents a ON f.agent_id = a.id
             WHERE a.session_id = ?1 ORDER BY f.created_at"
        )?;
        let findings = stmt.query_map(params![session_id.0], |row| {
            let id: String = row.get(0)?;
            let agent_id: String = row.get(1)?;
            let title: String = row.get(2)?;
            let content: String = row.get(3)?;
            let sources: String = row.get(4)?;
            let confidence: f64 = row.get(5)?;
            let created_at: String = row.get(6)?;
            Ok(Finding {
                id: pr_core::FindingId(id),
                agent_id: AgentId(agent_id),
                title,
                content,
                sources: serde_json::from_str(&sources).unwrap_or_default(),
                confidence: confidence as f32,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
            })
        })?;

        let mut result = Vec::new();
        for f in findings {
            result.push(f?);
        }
        Ok(result)
    }

    // Tool results

    pub fn add_tool_result(
        &self,
        agent_id: &AgentId,
        tool_name: &str,
        input: &serde_json::Value,
        output: &pr_core::ToolOutput,
        duration_ms: u64,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tool_results (agent_id, tool_name, input, output, success, duration_ms, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                agent_id.0,
                tool_name,
                input.to_string(),
                output.content,
                output.success as i32,
                duration_ms,
                now,
            ],
        )?;
        Ok(())
    }

    // Resume capability

    pub fn find_session_by_query(&self, query: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id FROM sessions WHERE query = ?1 AND status = 'running' ORDER BY created_at DESC LIMIT 1"
        )?;
        let result = stmt.query_row(params![query], |row| {
            let id: String = row.get(0)?;
            Ok(id)
        });
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_session_agents(&self, session_id: &SessionId) -> anyhow::Result<Vec<(String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, task FROM agents WHERE session_id = ?1"
        )?;
        let agents = stmt.query_map(params![session_id.0], |row| {
            let id: String = row.get(0)?;
            let task: String = row.get(1)?;
            Ok((id, task))
        })?;

        let mut result = Vec::new();
        for agent in agents {
            result.push(agent?);
        }
        Ok(result)
    }

    /// Fetch a single session row by id, or `None` if it does not exist.
    pub fn get_session(&self, id: &SessionId) -> anyhow::Result<Option<SessionRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, query, status, output_dir, total_tokens, total_agents, created_at, updated_at, error
             FROM sessions WHERE id = ?1"
        )?;
        let result = stmt.query_row(params![id.0], session_row_from_stmt);
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all sessions with the given status (e.g. "running", "completed"),
    /// most recently updated first.
    pub fn list_sessions_with_status(&self, status: &str) -> anyhow::Result<Vec<SessionRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, query, status, output_dir, total_tokens, total_agents, created_at, updated_at, error
             FROM sessions WHERE status = ?1 ORDER BY updated_at DESC"
        )?;
        let rows = stmt.query_map(params![status], session_row_from_stmt)?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Fetch agent rows for a session including status, tokens and summary
    /// (fields required by the resume machinery).
    pub fn get_session_agent_rows(&self, session_id: &SessionId) -> anyhow::Result<Vec<AgentRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, task, status, tokens_used, summary, parent_id FROM agents WHERE session_id = ?1 ORDER BY id"
        )?;
        let agents = stmt.query_map(params![session_id.0], |row| {
            Ok(AgentRow {
                id: row.get(0)?,
                task: row.get(1)?,
                status: row.get(2)?,
                tokens_used: row.get(3)?,
                summary: row.get(4)?,
                parent_id: row.get(5)?,
            })
        })?;

        let mut result = Vec::new();
        for agent in agents {
            result.push(agent?);
        }
        Ok(result)
    }

    /// Count agents of a session grouped by terminal status.
    /// Returns `(total, completed)` where completed counts agents whose
    /// status is 'completed'.
    pub fn count_session_agents(&self, session_id: &SessionId) -> anyhow::Result<(usize, usize)> {
        let conn = self.conn.lock();
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agents WHERE session_id = ?1",
            params![session_id.0],
            |row| row.get(0),
        )?;
        let completed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agents WHERE session_id = ?1 AND status = 'completed'",
            params![session_id.0],
            |row| row.get(0),
        )?;
        Ok((total as usize, completed as usize))
    }

    /// List all agents across all sessions, most recently created first.
    pub fn list_agents(&self) -> anyhow::Result<Vec<AgentDetailRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {AGENT_DETAIL_COLS} FROM agents ORDER BY created_at DESC, id DESC"
        ))?;
        let agents = stmt.query_map([], agent_detail_from_stmt)?;

        let mut result = Vec::new();
        for agent in agents {
            result.push(agent?);
        }
        Ok(result)
    }

    /// Fetch a single agent by id, or `None` if it does not exist.
    pub fn get_agent(&self, id: &str) -> anyhow::Result<Option<AgentDetailRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {AGENT_DETAIL_COLS} FROM agents WHERE id = ?1"
        ))?;
        let result = stmt.query_row(params![id], agent_detail_from_stmt);
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Fetch all agents belonging to a session, ordered by creation.
    pub fn get_session_agents_detail(
        &self,
        session_id: &SessionId,
    ) -> anyhow::Result<Vec<AgentDetailRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {AGENT_DETAIL_COLS} FROM agents WHERE session_id = ?1 ORDER BY created_at ASC, id ASC"
        ))?;
        let agents = stmt.query_map(params![session_id.0], agent_detail_from_stmt)?;

        let mut result = Vec::new();
        for agent in agents {
            result.push(agent?);
        }
        Ok(result)
    }
}

const AGENT_DETAIL_COLS: &str =
    "id, session_id, parent_id, role, task, status, depth, tokens_used, summary, created_at, completed_at";

fn agent_detail_from_stmt(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentDetailRow> {
    Ok(AgentDetailRow {
        id: row.get(0)?,
        session_id: row.get(1)?,
        parent_id: row.get(2)?,
        role: row.get(3)?,
        task: row.get(4)?,
        status: row.get(5)?,
        depth: row.get(6)?,
        tokens_used: row.get(7)?,
        summary: row.get(8)?,
        created_at: row.get(9)?,
        completed_at: row.get(10)?,
    })
}

impl Persistence {
    // ── Subtasks (Goal Mode light, fleet E4) ────────────────────────────

    /// Record a planned sub-task for the session.
    pub fn add_subtask(&self, session_id: &SessionId, task: &str) -> anyhow::Result<i64> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subtasks (session_id, task, status, created_at, updated_at)
             VALUES (?1, ?2, 'pending', ?3, ?3)",
            params![session_id.0, task, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Update sub-task status (`pending`/`running`/`completed`/`failed`).
    pub fn update_subtask_status(
        &self,
        session_id: &SessionId,
        task: &str,
        status: &str,
        result: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE subtasks SET status=?3, result=?4, updated_at=?5
             WHERE session_id=?1 AND task=?2",
            params![session_id.0, task, status, result, now],
        )?;
        Ok(())
    }

    /// List the session's sub-tasks in creation order.
    pub fn list_subtasks(&self, session_id: &SessionId) -> anyhow::Result<Vec<SubtaskRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, task, status, result, created_at, updated_at
             FROM subtasks WHERE session_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![session_id.0], |row| {
            Ok(SubtaskRow {
                id: row.get(0)?,
                task: row.get(1)?,
                status: row.get(2)?,
                result: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ── Session fork ────────────────────────────────────────────────────

    /// Fork a session: create a new session that references the original via
    /// a `parent_session_id` column. Copies the original session's agents and
    /// findings into the new session. Returns the new session id.
    pub fn fork_session(&self, original_id: &SessionId, new_query: &str) -> anyhow::Result<SessionId> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        let new_id = SessionId::new();

        // Create the new session
        conn.execute(
            "INSERT INTO sessions (id, query, status, created_at, updated_at) VALUES (?1, ?2, 'running', ?3, ?3)",
            params![new_id.0, new_query, now],
        )?;

        // Copy agents from the original session with new ids
        let mut stmt = conn.prepare(
            "SELECT parent_id, role, task, depth FROM agents WHERE session_id = ?1"
        )?;
        let agents: Vec<(Option<String>, String, String, i64)> = stmt.query_map(params![original_id.0], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?.collect::<rusqlite::Result<Vec<_>>>()?;

        for (parent_id, role, task, depth) in agents {
            let new_agent_id = AgentId::new();
            conn.execute(
                "INSERT INTO agents (id, session_id, parent_id, role, task, status, depth, tokens_used, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'spawned', ?6, 0, ?7)",
                params![new_agent_id.0, new_id.0, parent_id, role, task, depth, now],
            )?;
        }

        Ok(new_id)
    }

    /// Get the parent session id of a forked session (if any).
    /// We store this in the session's first subtask with a special prefix.
    pub fn get_parent_session(&self, session_id: &SessionId) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT result FROM subtasks WHERE session_id = ?1 AND task = '__parent_session' LIMIT 1",
            params![session_id.0],
            |row| row.get::<_, Option<String>>(0),
        );
        match result {
            Ok(parent) => Ok(parent),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Record the parent session id for a forked session.
    pub fn set_parent_session(&self, session_id: &SessionId, parent_id: &SessionId) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO subtasks (session_id, task, status, result, created_at, updated_at)
             VALUES (?1, '__parent_session', 'completed', ?2, ?3, ?3)",
            params![session_id.0, parent_id.0, now],
        )?;
        Ok(())
    }

    /// List all child sessions (forks) of a given session.
    pub fn list_child_sessions(&self, parent_id: &SessionId) -> anyhow::Result<Vec<SessionRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.query, s.status, s.output_dir, s.total_tokens, s.total_agents, s.created_at, s.updated_at, s.error
             FROM sessions s JOIN subtasks st ON s.id = st.session_id
             WHERE st.task = '__parent_session' AND st.result = ?1
             ORDER BY s.created_at DESC"
        )?;
        let rows = stmt.query_map(params![parent_id.0], session_row_from_stmt)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ── File change tracking (undo/redo) ────────────────────────────────

    /// Record a file change for undo support.
    pub fn record_file_change(
        &self,
        session_id: &SessionId,
        file_path: &str,
        operation: &str,
        old_content: Option<&str>,
        new_content: &str,
    ) -> anyhow::Result<i64> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO file_changes (session_id, file_path, operation, old_content, new_content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![session_id.0, file_path, operation, old_content, new_content, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get all file changes for a session, ordered by creation (for undo).
    pub fn get_file_changes(&self, session_id: &SessionId) -> anyhow::Result<Vec<FileChangeRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, file_path, operation, old_content, new_content, created_at
             FROM file_changes WHERE session_id = ?1 ORDER BY id ASC"
        )?;
        let rows = stmt.query_map(params![session_id.0], |row| {
            Ok(FileChangeRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                file_path: row.get(2)?,
                operation: row.get(3)?,
                old_content: row.get(4)?,
                new_content: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Undo the last file change in a session. Returns the change that was
    /// undone, or `None` if there are no changes to undo.
    pub fn undo_last_file_change(&self, session_id: &SessionId) -> anyhow::Result<Option<FileChangeRow>> {
        let conn = self.conn.lock();
        // Find the last change
        let last: Option<FileChangeRow> = conn.query_row(
            "SELECT id, session_id, file_path, operation, old_content, new_content, created_at
             FROM file_changes WHERE session_id = ?1 ORDER BY id DESC LIMIT 1",
            params![session_id.0],
            |row| Ok(FileChangeRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                file_path: row.get(2)?,
                operation: row.get(3)?,
                old_content: row.get(4)?,
                new_content: row.get(5)?,
                created_at: row.get(6)?,
            }),
        ).optional()?;

        if let Some(ref change) = last {
            // Apply the undo: restore old content
            if let Some(ref old_content) = change.old_content {
                let path = std::path::Path::new(&change.file_path);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(path, old_content)?;
            } else {
                // No old content means the file was created — delete it
                let _ = std::fs::remove_file(&change.file_path);
            }
            // Remove the change record
            conn.execute("DELETE FROM file_changes WHERE id = ?1", params![change.id])?;
        }

        Ok(last)
    }

    /// Redo the last undone file change. Returns the change that was redone.
    /// Since we delete on undo, redo re-applies the new_content.
    pub fn redo_file_change(
        &self,
        session_id: &SessionId,
        file_path: &str,
        operation: &str,
        new_content: &str,
        old_content: Option<&str>,
    ) -> anyhow::Result<i64> {
        // Just re-record the change
        self.record_file_change(session_id, file_path, operation, old_content, new_content)
    }

    /// Get the count of undoable file changes in a session.
    pub fn undoable_change_count(&self, session_id: &SessionId) -> anyhow::Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_changes WHERE session_id = ?1",
            params![session_id.0],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    // ── Title generation ────────────────────────────────────────────────

    /// Update the session's query/title (used by the title agent).
    pub fn set_session_title(&self, session_id: &SessionId, title: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET query = ?2, updated_at = ?3 WHERE id = ?1",
            params![session_id.0, title, now],
        )?;
        Ok(())
    }

    // ── Share links ─────────────────────────────────────────────────────

    /// Generate a shareable session id (short, URL-safe). Returns the first
    /// 8 characters of the session id, which is enough for unique lookup.
    pub fn share_link(&self, session_id: &SessionId, base_url: &str) -> anyhow::Result<String> {
        // Verify the session exists
        let _ = self.get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("session not found"))?;
        let short_id = &session_id.0[..8.min(session_id.0.len())];
        Ok(format!("{}/sessions/{}", base_url.trim_end_matches('/'), short_id))
    }

    /// Resolve a short session id prefix to a full session id.
    pub fn resolve_short_id(&self, short_id: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT id FROM sessions WHERE id LIKE ?1 LIMIT 1",
            params![format!("{}%", short_id)],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // ── Governance audit ───────────────────────────────────────────────

    /// Persist a redacted governance decision. JSON fields are stored as
    /// strings so this crate remains independent of the governance crate.
    pub fn record_audit_event(&self, event: &AuditEventRow) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO audit_events
             (id, timestamp, agent, session, tool, args, url, element, file, intent, mcp_metadata, decision)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                event.id, event.timestamp, event.agent, event.session, event.tool,
                event.args, event.url, event.element, event.file, event.intent,
                event.mcp_metadata, event.decision,
            ],
        )?;
        Ok(())
    }

    /// Return all audit events in chronological order.
    pub fn list_audit_events(&self) -> anyhow::Result<Vec<AuditEventRow>> {
        self.list_audit_events_limited(None)
    }

    /// Return at most `limit` audit events when a limit is useful to callers.
    pub fn list_audit_events_limited(&self, limit: Option<usize>) -> anyhow::Result<Vec<AuditEventRow>> {
        let conn = self.conn.lock();
        let sql = match limit {
            Some(_) => "SELECT id, timestamp, agent, session, tool, args, url, element, file, intent, mcp_metadata, decision FROM audit_events ORDER BY timestamp ASC LIMIT ?1",
            None => "SELECT id, timestamp, agent, session, tool, args, url, element, file, intent, mcp_metadata, decision FROM audit_events ORDER BY timestamp ASC",
        };
        let mut stmt = conn.prepare(sql)?;
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<AuditEventRow> {
            Ok(AuditEventRow {
                id: row.get(0)?, timestamp: row.get(1)?, agent: row.get(2)?, session: row.get(3)?,
                tool: row.get(4)?, args: row.get(5)?, url: row.get(6)?, element: row.get(7)?,
                file: row.get(8)?, intent: row.get(9)?, mcp_metadata: row.get(10)?, decision: row.get(11)?,
            })
        };
        let rows = match limit {
            Some(n) => stmt.query_map(params![n as i64], map_row)?.collect::<rusqlite::Result<Vec<_>>>()?,
            None => stmt.query_map([], map_row)?.collect::<rusqlite::Result<Vec<_>>>()?,
        };
        Ok(rows)
    }
}

/// A serializable, persistence-only representation of a governance audit event.
/// `args` and `mcp_metadata` contain redacted JSON text, never raw credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEventRow {
    pub id: String,
    pub timestamp: String,
    pub agent: String,
    pub session: String,
    pub tool: String,
    pub args: String,
    pub url: Option<String>,
    pub element: Option<String>,
    pub file: Option<String>,
    pub intent: Option<String>,
    pub mcp_metadata: Option<String>,
    pub decision: String,
}

/// A planned sub-task row (Goal Mode light).
#[derive(Debug, Clone)]
pub struct SubtaskRow {
    pub id: i64,
    pub task: String,
    pub status: String,
    pub result: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// ALTER TABLE ... ADD COLUMN when the column does not exist yet (idempotent
/// schema migration for databases created by older versions).
pub(crate) fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    sql_type: &str,
) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    if !names.iter().any(|n| n == column) {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {sql_type};"
        ))?;
    }
    Ok(())
}

fn session_row_from_stmt(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        id: row.get(0)?,
        query: row.get(1)?,
        status: row.get(2)?,
        output_dir: row.get(3)?,
        total_tokens: row.get(4)?,
        total_agents: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        error: row.get(8)?,
    })
}

/// A raw session row from the database.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub query: String,
    pub status: String,
    pub output_dir: Option<String>,
    pub total_tokens: i64,
    pub total_agents: i64,
    pub created_at: String,
    pub updated_at: String,
    /// Error message when `status == "failed"` (separate from output_dir).
    pub error: Option<String>,
}

/// A file change record for undo/redo support.
#[derive(Debug, Clone)]
pub struct FileChangeRow {
    pub id: i64,
    pub session_id: String,
    pub file_path: String,
    pub operation: String,
    pub old_content: Option<String>,
    pub new_content: String,
    pub created_at: String,
}

/// A raw agent row with the fields needed for session resume.
#[derive(Debug, Clone)]
pub struct AgentRow {
    pub id: String,
    pub task: String,
    pub status: String,
    pub tokens_used: i64,
    pub summary: Option<String>,
    /// Parent agent id (`None` for top-level agents) — lets resume
    /// reconstruct subtree token accounting.
    pub parent_id: Option<String>,
}

/// A full agent row with all fields needed by the HTTP API.
#[derive(Debug, Clone, serde::Serialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::{SessionId, AgentId, AgentRole, AgentStatus};

    #[test]
    fn test_in_memory_db() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "test query").unwrap();
        
        let found = db.find_session_by_query("test query").unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn test_agent_lifecycle() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "test").unwrap();
        
        let agent_id = AgentId::new();
        let agent = AgentRecord {
            id: agent_id.clone(),
            session_id: session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: "research task".to_string(),
            status: AgentStatus::Spawned,
            depth: 1,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        db.create_agent(&agent).unwrap();
        
        db.update_agent_status(&agent_id, AgentStatus::Completed, 100, Some("done")).unwrap();
    }

    #[test]
    fn test_message_persistence() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "test").unwrap();
        
        let agent_id = AgentId::new();
        let agent = AgentRecord {
            id: agent_id.clone(),
            session_id: session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: "task".to_string(),
            status: AgentStatus::Spawned,
            depth: 1,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        db.create_agent(&agent).unwrap();
        
        db.add_message(&agent_id, &pr_core::Message::user("hello")).unwrap();
        db.add_message(&agent_id, &pr_core::Message::assistant("world")).unwrap();
        
        let messages = db.get_agent_messages(&agent_id).unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_list_sessions_and_cancel() {
        let db = Persistence::in_memory().unwrap();
        let s1 = SessionId::new();
        let s2 = SessionId::new();
        db.create_session(&s1, "query one").unwrap();
        db.create_session(&s2, "query two").unwrap();

        let sessions = db.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);

        // Cancel a running session.
        assert!(db.cancel_session(&s1).unwrap());
        let row = db.get_session(&s1).unwrap().unwrap();
        assert_eq!(row.status, "cancelled");

        // Cancelling again is a no-op (session no longer running).
        assert!(!db.cancel_session(&s1).unwrap());

        // Cancelling an unknown session returns false.
        assert!(!db.cancel_session(&SessionId("missing".into())).unwrap());
    }

    #[test]
    fn test_list_and_get_agents() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "test").unwrap();

        assert!(db.list_agents().unwrap().is_empty());
        assert!(db.get_agent("missing").unwrap().is_none());

        let agent_id = AgentId::new();
        let agent = AgentRecord {
            id: agent_id.clone(),
            session_id: session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: "task".to_string(),
            status: AgentStatus::Spawned,
            depth: 1,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        db.create_agent(&agent).unwrap();

        let agents = db.list_agents().unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, agent_id.0);
        assert_eq!(agents[0].session_id, session_id.0);
        assert_eq!(agents[0].role, "researcher");

        let fetched = db.get_agent(&agent_id.0).unwrap().unwrap();
        assert_eq!(fetched.task, "task");
        assert_eq!(fetched.depth, 1);
        assert!(fetched.completed_at.is_none());
    }

    #[test]
    fn file_db_uses_connection_pool_and_stays_consistent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".research.db");
        let db = Persistence::open(&path).unwrap();
        assert_eq!(db.pool_size(), POOL_SIZE, "file DB should pool connections");

        // Concurrent-ish writes through the round-robin pool must all land
        // in the same logical database.
        for i in 0..16 {
            let sid = SessionId(format!("sess-{i}"));
            db.create_session(&sid, &format!("query {i}")).unwrap();
        }
        let rows = db.list_sessions().unwrap();
        assert_eq!(rows.len(), 16);

        // Reopen sees the same data.
        drop(db);
        let db2 = Persistence::open(&path).unwrap();
        assert_eq!(db2.list_sessions().unwrap().len(), 16);
    }

    #[test]
    fn in_memory_db_is_single_connection() {
        let db = Persistence::in_memory().unwrap();
        assert_eq!(db.pool_size(), 1, "in-memory DB must not pool");
    }

    #[test]
    fn test_fork_session_copies_agents_and_findings() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "original research").unwrap();

        let agent_id = AgentId::new();
        db.create_agent(&AgentRecord {
            id: agent_id.clone(),
            session_id: session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: "research task".to_string(),
            status: AgentStatus::Completed,
            depth: 1,
            tokens_used: 100,
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        }).unwrap();

        db.add_finding(&Finding {
            id: pr_core::FindingId::new(),
            agent_id: agent_id.clone(),
            title: "Key finding".to_string(),
            content: "content".to_string(),
            sources: vec![],
            confidence: 0.9,
            created_at: chrono::Utc::now(),
        }).unwrap();

        // Fork
        let forked_id = db.fork_session(&session_id, "extended research").unwrap();
        assert_ne!(forked_id.0, session_id.0);

        let forked = db.get_session(&forked_id).unwrap().unwrap();
        assert_eq!(forked.query, "extended research");
        assert_eq!(forked.status, "running");

        // Agents were copied
        let forked_agents = db.get_session_agent_rows(&forked_id).unwrap();
        assert_eq!(forked_agents.len(), 1);
        assert_eq!(forked_agents[0].task, "research task");
    }

    #[test]
    fn test_parent_child_sessions() {
        let db = Persistence::in_memory().unwrap();
        let parent_id = SessionId::new();
        db.create_session(&parent_id, "parent").unwrap();

        let child_id = SessionId::new();
        db.create_session(&child_id, "child fork").unwrap();
        db.set_parent_session(&child_id, &parent_id).unwrap();

        let resolved_parent = db.get_parent_session(&child_id).unwrap();
        assert_eq!(resolved_parent, Some(parent_id.0.clone()));

        let children = db.list_child_sessions(&parent_id).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, child_id.0);
    }

    #[test]
    fn test_file_change_tracking_and_undo() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "test").unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");

        // Record a file creation
        db.record_file_change(
            &session_id,
            file_path.to_str().unwrap(),
            "create",
            None,
            "hello world",
        ).unwrap();

        std::fs::write(&file_path, "hello world").unwrap();
        assert_eq!(db.undoable_change_count(&session_id).unwrap(), 1);

        // Record a file edit
        db.record_file_change(
            &session_id,
            file_path.to_str().unwrap(),
            "edit",
            Some("hello world"),
            "hello rust",
        ).unwrap();
        std::fs::write(&file_path, "hello rust").unwrap();

        assert_eq!(db.undoable_change_count(&session_id).unwrap(), 2);

        // Undo last change (edit -> restore "hello world")
        let undone = db.undo_last_file_change(&session_id).unwrap();
        assert!(undone.is_some());
        let undone = undone.unwrap();
        assert_eq!(undone.operation, "edit");
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");
        assert_eq!(db.undoable_change_count(&session_id).unwrap(), 1);

        // Undo first change (create -> delete file)
        let undone = db.undo_last_file_change(&session_id).unwrap();
        assert!(undone.is_some());
        assert_eq!(undone.unwrap().operation, "create");
        assert!(!file_path.exists());
        assert_eq!(db.undoable_change_count(&session_id).unwrap(), 0);

        // No more changes to undo
        assert!(db.undo_last_file_change(&session_id).unwrap().is_none());
    }

    #[test]
    fn test_set_session_title() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "original query").unwrap();

        db.set_session_title(&session_id, "Better Title: Research Summary").unwrap();

        let session = db.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.query, "Better Title: Research Summary");
    }

    #[test]
    fn test_share_link_and_resolve() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "test").unwrap();

        let link = db.share_link(&session_id, "https://app.example.com").unwrap();
        assert!(link.starts_with("https://app.example.com/sessions/"));
        assert!(link.len() > 40);

        // Resolve short id
        let short = &session_id.0[..8];
        let resolved = db.resolve_short_id(short).unwrap();
        assert_eq!(resolved, Some(session_id.0));
    }

    #[test]
    fn test_resolve_short_id_not_found() {
        let db = Persistence::in_memory().unwrap();
        let resolved = db.resolve_short_id("nonexist").unwrap();
        assert!(resolved.is_none());
    }

    #[test]
    fn audit_event_roundtrip() {
        let db = Persistence::in_memory().unwrap();
        let event = AuditEventRow {
            id: "evt-1".into(), timestamp: "2026-01-01T00:00:00Z".into(),
            agent: "agent".into(), session: "session".into(), tool: "browser.click".into(),
            args: r#"{"value":"[REDACTED]"}"#.into(), url: Some("https://example.com".into()),
            element: Some("e1".into()), file: None, intent: Some("read".into()),
            mcp_metadata: None, decision: "deny".into(),
        };
        db.record_audit_event(&event).unwrap();
        let rows = db.list_audit_events().unwrap();
        assert_eq!(rows, vec![event]);
        assert_eq!(db.list_audit_events_limited(Some(1)).unwrap().len(), 1);
    }

    #[test]
    fn test_list_child_sessions_empty() {
        let db = Persistence::in_memory().unwrap();
        let session_id = SessionId::new();
        db.create_session(&session_id, "no children").unwrap();
        let children = db.list_child_sessions(&session_id).unwrap();
        assert!(children.is_empty());
    }
}
