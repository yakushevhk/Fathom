//! SQLite-backed long-term memory store (mem0/Memora-inspired).
//!
//! Layout follows the Memora model: one SQLite database holds the memories
//! themselves (append-only rows), an FTS5 keyword index, embeddings,
//! typed edges between memories (supersedes/contradicts/related_to/...)
//! and an audit history. Nothing is ever overwritten in place — new
//! versions are new rows linked by `supersedes` edges.

use anyhow::Context;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Environment variable overriding the memory database location.
pub const MEMORY_DB_ENV: &str = "PR_MEMORY_DB";

/// Global memory database: `~/.fathom/memory.db` unless the
/// `PR_MEMORY_DB` environment variable points elsewhere.
pub fn default_memory_db_path() -> PathBuf {
    if let Ok(p) = std::env::var(MEMORY_DB_ENV) {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".fathom")
        .join("memory.db")
}

// ── Scope ────────────────────────────────────────────────────────────────────

/// Memory isolation scope (mem0 namespacing): `user` (facts about the
/// user/client), `agent` (general agent knowledge), `run` (session-local
/// episode facts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    Agent,
    Run,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
            Self::Run => "run",
        }
    }
}

impl std::str::FromStr for Scope {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "agent" => Ok(Self::Agent),
            "run" => Ok(Self::Run),
            other => anyhow::bail!("unknown memory scope '{other}', use user/agent/run"),
        }
    }
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Row types ────────────────────────────────────────────────────────────────

/// One stored memory (a self-contained fact).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRow {
    pub id: String,
    pub content: String,
    /// Free-form JSON metadata (type, status, entity refs, ...).
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
    /// Provenance marker, e.g. `session:<id>` or `research-тема-дата`.
    pub source: String,
    pub scope: String,
    pub scope_key: String,
    /// 0.0-1.0 — how sure we are the fact is true; tie-breaker in conflicts.
    pub confidence: f64,
    /// Utility weight raised by `boost` when a memory proves helpful.
    pub importance: f64,
    pub access_count: i64,
    pub last_accessed: Option<String>,
    /// `active`, `superseded` (excluded from search) or `archived`.
    pub status: String,
    /// Optional expiry timestamp (RFC3339); expired rows leave search.
    pub expires_at: Option<String>,
    pub content_hash: String,
    pub created_at: String,
    pub updated_at: String,
}

impl MemoryRow {
    pub fn is_expired(&self) -> bool {
        match &self.expires_at {
            Some(ts) => chrono::DateTime::parse_from_rfc3339(ts)
                .map(|t| t < chrono::Utc::now())
                .unwrap_or(false),
            None => false,
        }
    }
}

/// Typed edge between two memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEdge {
    pub from_id: String,
    pub to_id: String,
    /// `supersedes`, `contradicts`, `related_to`, `implements`, `extends`,
    /// `references`.
    pub edge_type: String,
    pub reason: Option<String>,
    pub created_at: String,
}

/// One audit-history event for a memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub memory_id: String,
    pub event: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub created_at: String,
}

/// Search scope filter: which (scope, scope_key) pairs are visible.
#[derive(Debug, Clone, Default)]
pub struct ScopeFilter {
    pub pairs: Vec<(String, String)>,
}

impl ScopeFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(mut self, scope: Scope, key: impl Into<String>) -> Self {
        self.pairs.push((scope.as_str().to_string(), key.into()));
        self
    }

    /// Everything except run-scoped episode facts (cross-session search).
    pub fn persistent() -> Self {
        Self {
            pairs: vec![
                ("agent".to_string(), String::new()),
                ("user".to_string(), String::new()),
            ],
        }
    }
}

// ── MemoryDb ─────────────────────────────────────────────────────────────────

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRow> {
    let metadata: String = r.get(2)?;
    let tags: String = r.get(3)?;
    Ok(MemoryRow {
        id: r.get(0)?,
        content: r.get(1)?,
        metadata: serde_json::from_str(&metadata).unwrap_or(serde_json::json!({})),
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        source: r.get(4)?,
        scope: r.get(5)?,
        scope_key: r.get(6)?,
        confidence: r.get(7)?,
        importance: r.get(8)?,
        access_count: r.get(9)?,
        last_accessed: r.get(10)?,
        status: r.get(11)?,
        expires_at: r.get(12)?,
        content_hash: r.get(13)?,
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

pub struct MemoryDb {
    conn: Mutex<Connection>,
    /// FTS5 may be missing from exotic SQLite builds; search degrades to
    /// LIKE-based keyword matching instead of failing.
    fts_available: bool,
}

impl MemoryDb {
    const SELECT_COLS: &'static str = "SELECT id, content, metadata, tags, source, scope, \
        scope_key, confidence, importance, access_count, last_accessed, status, expires_at, \
        content_hash, created_at, updated_at FROM memories";

    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;",
        )?;
        Self::from_connection(conn)
    }

    pub fn in_memory() -> anyhow::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> anyhow::Result<Self> {
        let fts_available = conn
            .execute_batch(
                "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts \
                 USING fts5(memory_id UNINDEXED, content, tags);",
            )
            .is_ok();
        if !fts_available {
            tracing::warn!("FTS5 unavailable in this SQLite build; falling back to LIKE keyword search");
        }
        let db = Self {
            conn: Mutex::new(conn),
            fts_available,
        };
        db.init_schema()?;
        db.init_graph_schema()?;
        Ok(db)
    }

    pub fn fts_available(&self) -> bool {
        self.fts_available
    }

    /// Crate-internal access to the connection guard (graph, distill).
    pub(crate) fn conn_lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS memories (
                id            TEXT PRIMARY KEY,
                content       TEXT NOT NULL,
                metadata      TEXT NOT NULL DEFAULT '{}',
                tags          TEXT NOT NULL DEFAULT '[]',
                source        TEXT NOT NULL DEFAULT '',
                scope         TEXT NOT NULL DEFAULT 'agent',
                scope_key     TEXT NOT NULL DEFAULT '',
                confidence    REAL NOT NULL DEFAULT 0.8,
                importance    REAL NOT NULL DEFAULT 1.0,
                access_count  INTEGER NOT NULL DEFAULT 0,
                last_accessed TEXT,
                status        TEXT NOT NULL DEFAULT 'active',
                expires_at    TEXT,
                content_hash  TEXT NOT NULL,
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories (scope, scope_key, status);
            CREATE INDEX IF NOT EXISTS idx_memories_status ON memories (status);
            CREATE INDEX IF NOT EXISTS idx_memories_hash ON memories (content_hash);

            CREATE TABLE IF NOT EXISTS memories_embeddings (
                memory_id TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
                model     TEXT NOT NULL,
                dim       INTEGER NOT NULL,
                vector    BLOB NOT NULL
            );

            CREATE TABLE IF NOT EXISTS memory_edges (
                from_id    TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
                to_id      TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
                edge_type  TEXT NOT NULL,
                reason     TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (from_id, to_id, edge_type)
            );

            CREATE TABLE IF NOT EXISTS memory_history (
                seq        INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id  TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
                event      TEXT NOT NULL,
                old_value  TEXT,
                new_value  TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_history_memory ON memory_history (memory_id);

            CREATE TABLE IF NOT EXISTS memory_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    // ── CRUD ────────────────────────────────────────────────────────────

    pub fn insert(&self, row: &MemoryRow) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memories (id, content, metadata, tags, source, scope, scope_key,
             confidence, importance, access_count, last_accessed, status, expires_at,
             content_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                row.id,
                row.content,
                row.metadata.to_string(),
                serde_json::to_string(&row.tags).unwrap_or_else(|_| "[]".into()),
                row.source,
                row.scope,
                row.scope_key,
                row.confidence,
                row.importance,
                row.access_count,
                row.last_accessed,
                row.status,
                row.expires_at,
                row.content_hash,
                row.created_at,
                row.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Option<MemoryRow>> {
        let conn = self.conn.lock().unwrap();
        // Exact id first...
        let exact = conn
            .prepare(&format!("{} WHERE id = ?1", Self::SELECT_COLS))?
            .query_row(params![id], map_row)
            .optional()?;
        if exact.is_some() {
            return Ok(exact);
        }
        // ...then a unique prefix...
        let mut stmt = conn.prepare(&format!("{} WHERE id LIKE ?1", Self::SELECT_COLS))?;
        let mut rows = stmt.query_map(params![format!("{id}%")], map_row)?;
        match rows.next().transpose()? {
            Some(row) => {
                if rows.next().is_some() {
                    anyhow::bail!("ambiguous memory id prefix: {id}");
                }
                return Ok(Some(row));
            }
            None => {}
        }
        // ...then a unique suffix. UUIDv7 ids lead with a timestamp, so the
        // random TAIL is the discriminating part — short ids shown to the
        // model are suffixes, not prefixes.
        let mut stmt = conn.prepare(&format!("{} WHERE id LIKE ?1", Self::SELECT_COLS))?;
        let mut rows = stmt.query_map(params![format!("%{id}")], map_row)?;
        match rows.next().transpose()? {
            Some(row) => {
                if rows.next().is_some() {
                    anyhow::bail!("ambiguous memory id suffix: {id}");
                }
                Ok(Some(row))
            }
            None => Ok(None),
        }
    }

    /// All memories visible under `filter`, newest first.
    pub fn list(&self, filter: &ScopeFilter, status: Option<&str>, limit: usize) -> anyhow::Result<Vec<MemoryRow>> {
        let conn = self.conn.lock().unwrap();
        let mut sql = format!("{} WHERE 1=1", Self::SELECT_COLS);
        let mut args: Vec<String> = Vec::new();
        if !filter.pairs.is_empty() {
            let clauses: Vec<String> = filter
                .pairs
                .iter()
                .map(|(scope, key)| {
                    if key.is_empty() {
                        format!("scope = '{}'", scope.replace('\'', "''"))
                    } else {
                        format!(
                            "(scope = '{}' AND scope_key = '{}')",
                            scope.replace('\'', "''"),
                            key.replace('\'', "''")
                        )
                    }
                })
                .collect();
            sql.push_str(&format!(" AND ({})", clauses.join(" OR ")));
        }
        if let Some(s) = status {
            sql.push_str(" AND status = ?");
            args.push(s.to_string());
        }
        // usize::MAX would overflow SQLite's signed 64-bit LIMIT.
        let limit = (limit as u64).min(i64::MAX as u64);
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {limit}"));
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), map_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn find_by_hash(&self, hash: &str, scope: &str, scope_key: &str) -> anyhow::Result<Option<MemoryRow>> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .prepare(&format!(
                "{} WHERE content_hash = ?1 AND scope = ?2 AND scope_key = ?3 AND status != 'archived' LIMIT 1",
                Self::SELECT_COLS
            ))?
            .query_row(params![hash, scope, scope_key], map_row)
            .optional()?;
        Ok(row)
    }

    pub fn set_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memories SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status, now],
        )?;
        Ok(())
    }

    /// Replace the content of a memory (re-embedding is the caller's job).
    pub fn update_content(&self, id: &str, content: &str, hash: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memories SET content = ?2, content_hash = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, content, hash, now],
        )?;
        Ok(())
    }

    /// Raise (or lower) importance when a memory proves useful.
    pub fn boost(&self, id: &str, amount: f64) -> anyhow::Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE memories SET importance = MAX(0.0, importance + ?2), updated_at = ?3 \
             WHERE id = ?1 AND status = 'active'",
            params![id, amount, now],
        )?;
        Ok(n > 0)
    }

    /// Update confidence directly (used by GC decay).
    /// Does not touch `updated_at` so idle-time calculations remain valid.
    pub fn update_confidence(&self, id: &str, confidence: f64) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE memories SET confidence = ?2 WHERE id = ?1",
            params![id, confidence],
        )?;
        Ok(n > 0)
    }

    /// Merge new content/tags into an existing memory (cross-call consolidation).
    /// Content is semicolon-joined, tags are unioned, confidence takes the max.
    pub fn merge_into(
        &self,
        id: &str,
        new_content: &str,
        new_tags: &[String],
        new_confidence: f64,
    ) -> anyhow::Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        // Read current row to merge content and tags.
        let existing: Option<(String, String, f64)> = conn
            .query_row(
                "SELECT content, tags, confidence FROM memories WHERE id = ?1",
                params![id],
                |row| {
                    let content: String = row.get(0)?;
                    let tags_json: String = row.get(1)?;
                    let conf: f64 = row.get(2)?;
                    Ok((content, tags_json, conf))
                },
            )
            .optional()?;
        let Some((old_content, old_tags_json, old_conf)) = existing else {
            return Ok(false);
        };
        let merged_content = format!("{old_content}; {new_content}");
        let merged_hash = content_hash(&merged_content);
        let merged_conf = old_conf.max(new_confidence);
        // Union tags.
        let mut tags: Vec<String> =
            serde_json::from_str::<Vec<String>>(&old_tags_json).unwrap_or_default();
        for t in new_tags {
            if !tags.contains(t) {
                tags.push(t.clone());
            }
        }
        let tags_json = serde_json::to_string(&tags)?;
        conn.execute(
            "UPDATE memories SET content = ?2, content_hash = ?3, tags = ?4, \
             confidence = ?5, updated_at = ?6 WHERE id = ?1",
            params![id, merged_content, merged_hash, tags_json, merged_conf, now],
        )?;
        Ok(true)
    }

    /// Hard-delete a memory and its embeddings/edges (for poison data);
    /// prefer `set_status(id, "archived")` for ordinary forgetting.
    pub fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        conn.execute("DELETE FROM memories_embeddings WHERE memory_id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM memory_edges WHERE from_id = ?1 OR to_id = ?1",
            params![id],
        )?;
        if self.fts_available {
            let _ = conn.execute("DELETE FROM memories_fts WHERE memory_id = ?1", params![id]);
        }
        Ok(n > 0)
    }

    pub fn count(&self, filter: &ScopeFilter) -> anyhow::Result<i64> {
        let rows = self.list(filter, Some("active"), i64::MAX as usize)?;
        Ok(rows.len() as i64)
    }

    // ── Access tracking ─────────────────────────────────────────────────

    pub fn record_access(&self, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        for id in ids {
            let _ = conn.execute(
                "UPDATE memories SET access_count = access_count + 1, last_accessed = ?2 \
                 WHERE id = ?1",
                params![id, now],
            );
        }
    }

    // ── FTS ─────────────────────────────────────────────────────────────

    pub fn fts_insert(&self, id: &str, content: &str, tags: &[String]) {
        if !self.fts_available {
            return;
        }
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO memories_fts (memory_id, content, tags) VALUES (?1, ?2, ?3)",
            params![id, content, tags.join(" ")],
        );
    }

    pub fn fts_remove(&self, id: &str) {
        if !self.fts_available {
            return;
        }
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM memories_fts WHERE memory_id = ?1", params![id]);
    }

    /// Keyword search. Returns (memory_id, relevance) with higher = better.
    /// Uses FTS5 BM25 when available, else a LIKE fallback.
    pub fn keyword_search(&self, query: &str, filter: &ScopeFilter, limit: usize) -> anyhow::Result<Vec<(String, f64)>> {
        let conn = self.conn.lock().unwrap();
        if self.fts_available {
            let match_expr = fts_match_expr(query);
            if match_expr.is_empty() {
                return Ok(Vec::new());
            }
            let scope_clause = scope_clause_sql(filter);
            let sql = format!(
                "SELECT f.memory_id, bm25(memories_fts) AS rank
                 FROM memories_fts f
                 JOIN memories m ON m.id = f.memory_id
                 WHERE memories_fts MATCH ?1 {scope_clause} AND m.status = 'active'
                 ORDER BY rank LIMIT ?2"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![match_expr, limit as i64], |r| {
                    Ok((r.get::<_, String>(0)?, -r.get::<_, f64>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            return Ok(rows);
        }

        // LIKE fallback: count matching query terms in content.
        let terms: Vec<String> = query
            .split_whitespace()
            .filter(|t| t.len() >= 2)
            .map(|t| t.to_lowercase())
            .collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let scope_clause = scope_clause_sql(filter);
        let sql = format!(
            "SELECT id, content, tags FROM memories WHERE status = 'active' {scope_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut scored: Vec<(String, f64)> = Vec::new();
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            let id: String = r.get(0)?;
            let content: String = r.get::<_, String>(1)?.to_lowercase();
            let tags: String = r.get::<_, String>(2)?.to_lowercase();
            let hits = terms
                .iter()
                .filter(|t| content.contains(t.as_str()) || tags.contains(t.as_str()))
                .count();
            if hits > 0 {
                scored.push((id, hits as f64));
            }
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }

    // ── Embeddings ──────────────────────────────────────────────────────

    pub fn put_embedding(&self, memory_id: &str, model: &str, vector: &[f32]) -> anyhow::Result<()> {
        let bytes = f32_vec_to_bytes(vector);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO memories_embeddings (memory_id, model, dim, vector)
             VALUES (?1, ?2, ?3, ?4)",
            params![memory_id, model, vector.len() as i64, bytes],
        )?;
        Ok(())
    }

    /// All embeddings for memories visible under `filter`, restricted to
    /// rows embedded with `model` (vectors from different models are not
    /// comparable). Expired/superseded rows are excluded.
    pub fn load_embeddings(&self, filter: &ScopeFilter, model: &str) -> anyhow::Result<Vec<(String, Vec<f32>)>> {
        let conn = self.conn.lock().unwrap();
        let scope_clause = scope_clause_sql(filter);
        let sql = format!(
            "SELECT e.memory_id, e.vector FROM memories_embeddings e
             JOIN memories m ON m.id = e.memory_id
             WHERE e.model = ?1 {scope_clause} AND m.status = 'active'
             AND (m.expires_at IS NULL OR m.expires_at > datetime('now'))"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![model], |r| {
                let id: String = r.get(0)?;
                let blob: Vec<u8> = r.get(1)?;
                Ok((id, blob))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, blob)| bytes_to_f32_vec(&blob).map(|v| (id, v)))
            .collect())
    }

    /// Ids of memories missing an embedding for `model` (for rebuilds).
    pub fn ids_without_embedding(&self, model: &str) -> anyhow::Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT m.id, m.content FROM memories m
             WHERE m.status != 'archived'
             AND m.id NOT IN (SELECT memory_id FROM memories_embeddings WHERE model = ?1)",
        )?;
        let rows = stmt
            .query_map(params![model], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    // ── Edges ───────────────────────────────────────────────────────────

    pub fn add_edge(&self, from: &str, to: &str, edge_type: &str, reason: Option<&str>) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO memory_edges (from_id, to_id, edge_type, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![from, to, edge_type, reason, now],
        )?;
        Ok(())
    }

    /// Edges touching `id` in either direction.
    pub fn edges_of(&self, id: &str) -> anyhow::Result<Vec<MemoryEdge>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT from_id, to_id, edge_type, reason, created_at FROM memory_edges
             WHERE from_id = ?1 OR to_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map(params![id], |r| {
                Ok(MemoryEdge {
                    from_id: r.get(0)?,
                    to_id: r.get(1)?,
                    edge_type: r.get(2)?,
                    reason: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The memory (if any) that supersedes `id`.
    pub fn superseded_by(&self, id: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .prepare(
                "SELECT from_id FROM memory_edges WHERE to_id = ?1 AND edge_type = 'supersedes'
                 ORDER BY created_at DESC LIMIT 1",
            )?
            .query_row(params![id], |r| r.get::<_, String>(0))
            .optional()?;
        Ok(row)
    }

    // ── History & meta ──────────────────────────────────────────────────

    pub fn log_history(&self, memory_id: &str, event: &str, old_value: Option<&str>, new_value: Option<&str>) {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO memory_history (memory_id, event, old_value, new_value, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![memory_id, event, old_value, new_value, now],
        );
    }

    pub fn history(&self, memory_id: &str) -> anyhow::Result<Vec<HistoryEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT memory_id, event, old_value, new_value, created_at FROM memory_history
             WHERE memory_id = ?1 ORDER BY seq ASC",
        )?;
        let rows = stmt
            .query_map(params![memory_id], |r| {
                Ok(HistoryEvent {
                    memory_id: r.get(0)?,
                    event: r.get(1)?,
                    old_value: r.get(2)?,
                    new_value: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn meta_get(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value FROM memory_meta WHERE key = ?1")
            .ok()?;
        stmt.query_row(params![key], |r| r.get::<_, String>(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn meta_set(&self, key: &str, value: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO memory_meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        );
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build an FTS5 MATCH expression from a free-form query: quote each term
/// and OR them together (any term may hit).
fn fts_match_expr(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|t| t.len() >= 2)
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// SQL fragment restricting `m.scope`/`m.scope_key` to the filter pairs.
/// Safe: values are escaped by doubling single quotes (no user SQL).
fn scope_clause_sql(filter: &ScopeFilter) -> String {
    if filter.pairs.is_empty() {
        return String::new();
    }
    let clauses: Vec<String> = filter
        .pairs
        .iter()
        .map(|(scope, key)| {
            if key.is_empty() {
                format!("m.scope = '{}'", scope.replace('\'', "''"))
            } else {
                format!(
                    "(m.scope = '{}' AND m.scope_key = '{}')",
                    scope.replace('\'', "''"),
                    key.replace('\'', "''")
                )
            }
        })
        .collect();
    format!(" AND ({})", clauses.join(" OR "))
}

fn f32_vec_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn bytes_to_f32_vec(b: &[u8]) -> Option<Vec<f32>> {
    if b.len() % 4 != 0 {
        return None;
    }
    Some(
        b.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
    )
}

/// Stable, dependency-free content hash for cheap pre-LLM dedup.
pub fn content_hash(content: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    content.trim().to_lowercase().hash(&mut h);
    format!("{:016x}", h.finish())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> MemoryDb {
        MemoryDb::in_memory().unwrap()
    }

    fn row(content: &str) -> MemoryRow {
        let now = chrono::Utc::now().to_rfc3339();
        MemoryRow {
            id: uuid::Uuid::now_v7().to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            tags: vec![],
            source: "test".into(),
            scope: "agent".into(),
            scope_key: String::new(),
            confidence: 0.9,
            importance: 1.0,
            access_count: 0,
            last_accessed: None,
            status: "active".into(),
            expires_at: None,
            content_hash: content_hash(content),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[test]
    fn insert_get_roundtrip() {
        let db = db();
        let r = row("Acme LLC uses Postgres 16 in production");
        db.insert(&r).unwrap();
        let got = db.get(&r.id).unwrap().unwrap();
        assert_eq!(got.content, r.content);
        assert_eq!(got.scope, "agent");
    }

    #[test]
    fn get_by_unique_prefix() {
        let db = db();
        let r = row("only memory");
        db.insert(&r).unwrap();
        let got = db.get(&r.id[..8]).unwrap().unwrap();
        assert_eq!(got.id, r.id);
        assert!(db.get("zzz").unwrap().is_none());
    }

    #[test]
    fn scope_filter_list() {
        let db = db();
        let a = row("agent fact");
        let mut u = row("user fact");
        u.scope = "user".into();
        let mut run = row("run fact");
        run.scope = "run".into();
        run.scope_key = "sess-1".into();
        db.insert(&a).unwrap();
        db.insert(&u).unwrap();
        db.insert(&run).unwrap();

        let persistent = db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
        assert_eq!(persistent.len(), 2);
        assert!(persistent.iter().all(|m| m.scope != "run"));

        let run_only = db
            .list(&ScopeFilter::new().add(Scope::Run, "sess-1"), None, 100)
            .unwrap();
        assert_eq!(run_only.len(), 1);
        assert_eq!(run_only[0].content, "run fact");
    }

    #[test]
    fn find_by_hash_dedup() {
        let db = db();
        let r = row("dup fact");
        db.insert(&r).unwrap();
        let found = db.find_by_hash(&content_hash("dup fact"), "agent", "").unwrap();
        assert!(found.is_some());
        assert!(db.find_by_hash(&content_hash("other"), "agent", "").unwrap().is_none());
    }

    #[test]
    fn supersession_chain_lookup() {
        let db = db();
        let v1 = row("CEO is Alice");
        let v2 = row("CEO is Bob since 2025");
        db.insert(&v1).unwrap();
        db.insert(&v2).unwrap();
        db.add_edge(&v2.id, &v1.id, "supersedes", Some("management change")).unwrap();
        db.set_status(&v1.id, "superseded").unwrap();

        assert_eq!(db.superseded_by(&v1.id).unwrap(), Some(v2.id.clone()));
        assert_eq!(db.superseded_by(&v2.id).unwrap(), None);

        let edges = db.edges_of(&v1.id).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_type, "supersedes");

        // Superseded rows leave active listings.
        let active = db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, v2.id);
    }

    #[test]
    fn boost_and_history() {
        let db = db();
        let r = row("useful fact");
        db.insert(&r).unwrap();
        assert!(db.boost(&r.id, 0.5).unwrap());
        assert!((db.get(&r.id).unwrap().unwrap().importance - 1.5).abs() < 1e-9);
        // Boosting unknown ids is a no-op, not an error.
        assert!(!db.boost("nope", 1.0).unwrap());

        db.log_history(&r.id, "add", None, Some("useful fact"));
        db.log_history(&r.id, "boost", Some("1.0"), Some("1.5"));
        let h = db.history(&r.id).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].event, "add");
        assert_eq!(h[1].old_value.as_deref(), Some("1.0"));
    }

    #[test]
    fn keyword_search_hits() {
        let db = db();
        let r = row("PostgreSQL 16 powers the billing service");
        db.insert(&r).unwrap();
        db.fts_insert(&r.id, &r.content, &r.tags);

        let hits = db.keyword_search("postgres billing", &ScopeFilter::persistent(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, r.id);

        // Superseded rows are excluded.
        db.set_status(&r.id, "superseded").unwrap();
        let hits = db.keyword_search("postgres", &ScopeFilter::persistent(), 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn embeddings_roundtrip_and_model_filter() {
        let db = db();
        let r = row("vector fact");
        db.insert(&r).unwrap();
        db.put_embedding(&r.id, "tfidf-hash-512", &[0.1, 0.2, 0.3]).unwrap();

        let same = db.load_embeddings(&ScopeFilter::persistent(), "tfidf-hash-512").unwrap();
        assert_eq!(same.len(), 1);
        assert!((same[0].1[1] - 0.2).abs() < 1e-6);

        // Different model name => not comparable => not returned.
        let other = db.load_embeddings(&ScopeFilter::persistent(), "text-embedding-3-small").unwrap();
        assert!(other.is_empty());

        assert_eq!(db.ids_without_embedding("text-embedding-3-small").unwrap().len(), 1);
        assert!(db.ids_without_embedding("tfidf-hash-512").unwrap().is_empty());
    }

    #[test]
    fn delete_removes_everything() {
        let db = db();
        let r = row("delete me");
        db.insert(&r).unwrap();
        db.put_embedding(&r.id, "m", &[1.0]).unwrap();
        db.fts_insert(&r.id, &r.content, &r.tags);
        db.log_history(&r.id, "add", None, Some("x"));
        assert!(db.delete(&r.id).unwrap());
        assert!(db.get(&r.id).unwrap().is_none());
        assert!(db.load_embeddings(&ScopeFilter::persistent(), "m").unwrap().is_empty());
        assert!(db.keyword_search("delete", &ScopeFilter::persistent(), 10).unwrap().is_empty());
    }

    #[test]
    fn expired_rows_leave_embedding_search() {
        let db = db();
        let mut r = row("ephemeral");
        r.expires_at = Some((chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339());
        db.insert(&r).unwrap();
        db.put_embedding(&r.id, "m", &[1.0]).unwrap();
        assert!(db.load_embeddings(&ScopeFilter::persistent(), "m").unwrap().is_empty());
        assert!(r.is_expired());
    }

    #[test]
    fn meta_roundtrip() {
        let db = db();
        assert!(db.meta_get("embedding_model").is_none());
        db.meta_set("embedding_model", "tfidf-hash-512");
        assert_eq!(db.meta_get("embedding_model").as_deref(), Some("tfidf-hash-512"));
        db.meta_set("embedding_model", "other");
        assert_eq!(db.meta_get("embedding_model").as_deref(), Some("other"));
    }

    #[test]
    fn open_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.db");
        let db = MemoryDb::open(&path).unwrap();
        let r = row("persistent fact");
        db.insert(&r).unwrap();
        db.fts_insert(&r.id, &r.content, &r.tags);
        drop(db);

        let db2 = MemoryDb::open(&path).unwrap();
        assert!(db2.get(&r.id).unwrap().is_some());
        let hits = db2.keyword_search("persistent", &ScopeFilter::persistent(), 5).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn content_hash_stable_and_case_insensitive() {
        assert_eq!(content_hash("Hello World"), content_hash("  hello world  "));
        assert_ne!(content_hash("a"), content_hash("b"));
    }

    #[test]
    fn fts_match_expr_quotes_terms() {
        assert_eq!(fts_match_expr("postgres 16"), "\"postgres\" OR \"16\"");
        assert_eq!(fts_match_expr(""), "");
        assert_eq!(fts_match_expr("a"), ""); // too short
    }
}
