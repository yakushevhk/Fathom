//! SQLite persistence — the whole app state lives in `data_dir/fathom.db`.
//! Local-first: transcripts, bots, approvals and settings stay on this machine.

use crate::types::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
}

/// `~/.fathombot` — transcripts, keys and events live here, never in a cloud.
pub fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fathombot")
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
    pub data_dir: PathBuf,
}

impl Store {
    pub fn open(data_dir: PathBuf) -> Result<Self, StoreError> {
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(data_dir.join("souls"))?;
        std::fs::create_dir_all(data_dir.join("files"))?;
        let conn = Connection::open(data_dir.join("fathom.db"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir,
        })
    }

    // ---- bots ------------------------------------------------------------

    pub fn upsert_bot(&self, bot: &Bot) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO bots (id,name,engine,model,soul,avatar_seed,cwd,auto_approve,archived,computer_id,created_at,updated_at,last_message,last_activity_at,unread,working)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, engine=excluded.engine, model=excluded.model,
               soul=excluded.soul, avatar_seed=excluded.avatar_seed, cwd=excluded.cwd,
               auto_approve=excluded.auto_approve, archived=excluded.archived,
               computer_id=excluded.computer_id, updated_at=excluded.updated_at,
               last_message=excluded.last_message, last_activity_at=excluded.last_activity_at,
               unread=excluded.unread, working=excluded.working",
            params![
                bot.id, bot.name, bot.engine.to_string(), bot.model, bot.soul,
                bot.avatar_seed, bot.cwd, bot.auto_approve, bot.archived, bot.computer_id,
                bot.created_at, bot.updated_at, bot.last_message, bot.last_activity_at,
                bot.unread, bot.working,
            ],
        )?;
        Ok(())
    }

    pub fn get_bot(&self, id: &str) -> Result<Option<Bot>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,name,engine,model,soul,avatar_seed,cwd,auto_approve,archived,computer_id,created_at,updated_at,last_message,last_activity_at,unread,working FROM bots WHERE id=?1",
                params![id],
                row_to_bot,
            )
            .optional()?)
    }

    pub fn list_bots(&self) -> Result<Vec<Bot>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,name,engine,model,soul,avatar_seed,cwd,auto_approve,archived,computer_id,created_at,updated_at,last_message,last_activity_at,unread,working FROM bots
             WHERE archived=0 ORDER BY COALESCE(last_activity_at, created_at) DESC",
        )?;
        let rows = stmt.query_map([], row_to_bot)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn delete_bot(&self, id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM messages WHERE thread_id IN (SELECT id FROM threads WHERE bot_id=?1)",
            params![id],
        )?;
        conn.execute("DELETE FROM approvals WHERE bot_id=?1", params![id])?;
        conn.execute("DELETE FROM threads WHERE bot_id=?1", params![id])?;
        conn.execute("DELETE FROM bots WHERE id=?1", params![id])?;
        Ok(())
    }

    // ---- threads ----------------------------------------------------------

    pub fn upsert_thread(&self, t: &Thread) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO threads (id,kind,bot_id,title,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, updated_at=excluded.updated_at",
            params![t.id, t.kind, t.bot_id, t.title, t.created_at, t.updated_at],
        )?;
        Ok(())
    }

    pub fn get_thread(&self, id: &str) -> Result<Option<Thread>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,kind,bot_id,title,created_at,updated_at FROM threads WHERE id=?1",
                params![id],
                |r| {
                    Ok(Thread {
                        id: r.get(0)?,
                        kind: r.get(1)?,
                        bot_id: r.get(2)?,
                        title: r.get(3)?,
                        created_at: r.get(4)?,
                        updated_at: r.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    /// The primary DM thread for a bot — created on first access.
    pub fn direct_thread(&self, bot_id: &str) -> Result<Thread, StoreError> {
        if let Some(t) = self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,kind,bot_id,title,created_at,updated_at FROM threads WHERE bot_id=?1 AND kind='direct' ORDER BY created_at LIMIT 1",
                params![bot_id],
                |r| Ok(Thread { id: r.get(0)?, kind: r.get(1)?, bot_id: r.get(2)?, title: r.get(3)?, created_at: r.get(4)?, updated_at: r.get(5)? }),
            )
            .optional()?
        {
            return Ok(t);
        }
        let now = now_ms();
        let t = Thread {
            id: new_id("th"),
            kind: "direct".into(),
            bot_id: Some(bot_id.into()),
            title: None,
            created_at: now,
            updated_at: now,
        };
        self.upsert_thread(&t)?;
        Ok(t)
    }

    // ---- messages ----------------------------------------------------------

    pub fn upsert_message(&self, m: &Message) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO messages (id,thread_id,role,text,engine,model,segments,pending,error,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(id) DO UPDATE SET text=excluded.text, segments=excluded.segments,
               pending=excluded.pending, error=excluded.error",
            params![
                m.id, m.thread_id,
                match m.role { Role::User => "user", Role::Bot => "bot", Role::System => "system", Role::Tool => "tool" },
                m.text, m.engine.map(|e| e.to_string()), m.model,
                serde_json::to_string(&m.segments)?, m.pending, m.error, m.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_messages(
        &self,
        thread_id: &str,
        limit: i64,
        before: Option<i64>,
    ) -> Result<Vec<Message>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,thread_id,role,text,engine,model,segments,pending,error,created_at FROM messages
             WHERE thread_id=?1 AND (?2 IS NULL OR created_at<?2) ORDER BY created_at DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![thread_id, before, limit], row_to_message)?;
        let mut out = rows.collect::<Result<Vec<_>, _>>()?;
        out.reverse();
        Ok(out)
    }

    /// Roster bookkeeping: last snippet, activity ts, unread counter.
    pub fn touch_bot_activity(
        &self,
        bot_id: &str,
        snippet: &str,
        mark_unread: bool,
    ) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "UPDATE bots SET last_message=?2, last_activity_at=?3,
               unread = unread + CASE WHEN ?4 THEN 1 ELSE 0 END, updated_at=?3 WHERE id=?1",
            params![
                bot_id,
                snippet.chars().take(120).collect::<String>(),
                now_ms(),
                mark_unread
            ],
        )?;
        Ok(())
    }

    pub fn mark_read(&self, bot_id: &str) -> Result<(), StoreError> {
        self.conn
            .lock()
            .unwrap()
            .execute("UPDATE bots SET unread=0 WHERE id=?1", params![bot_id])?;
        Ok(())
    }

    pub fn set_bot_working(&self, bot_id: &str, working: bool) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "UPDATE bots SET working=?2 WHERE id=?1",
            params![bot_id, working],
        )?;
        Ok(())
    }

    // ---- approvals ----------------------------------------------------------

    pub fn upsert_approval(&self, a: &ApprovalRequest) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO approvals (id,bot_id,thread_id,request_id,kind,title,detail,options,status,response,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(id) DO UPDATE SET status=excluded.status, response=excluded.response",
            params![
                a.id, a.bot_id, a.thread_id, a.request_id, a.kind, a.title, a.detail,
                serde_json::to_string(&a.options)?,
                match a.status { ApprovalStatus::Pending => "pending", ApprovalStatus::Allowed => "allowed", ApprovalStatus::Denied => "denied", ApprovalStatus::Answered => "answered" },
                a.response, a.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_approval(&self, id: &str) -> Result<Option<ApprovalRequest>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,bot_id,thread_id,request_id,kind,title,detail,options,status,response,created_at FROM approvals WHERE id=?1",
                params![id],
                row_to_approval,
            )
            .optional()?)
    }

    pub fn pending_approvals(
        &self,
        bot_id: Option<&str>,
    ) -> Result<Vec<ApprovalRequest>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let (sql, p): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match bot_id {
            Some(b) => ("SELECT id,bot_id,thread_id,request_id,kind,title,detail,options,status,response,created_at FROM approvals WHERE status='pending' AND bot_id=?1 ORDER BY created_at", vec![Box::new(b.to_string())]),
            None => ("SELECT id,bot_id,thread_id,request_id,kind,title,detail,options,status,response,created_at FROM approvals WHERE status='pending' ORDER BY created_at", vec![]),
        };
        let mut stmt = conn.prepare(sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), row_to_approval)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- kv settings ----------------------------------------------------------

    pub fn get_kv(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT value FROM kv WHERE key=?1", params![key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_kv(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO kv (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- SOUL.md files ----------------------------------------------------------

    /// Path of a bot's persona file in `data_dir/souls/`.
    pub fn soul_path(&self, bot_id: &str) -> PathBuf {
        self.data_dir.join("souls").join(format!("{bot_id}.md"))
    }

    pub fn write_soul(&self, bot_id: &str, soul: &str) -> Result<(), StoreError> {
        std::fs::write(self.soul_path(bot_id), soul)?;
        Ok(())
    }
}

fn row_to_bot(r: &rusqlite::Row) -> rusqlite::Result<Bot> {
    let engine: String = r.get(2)?;
    Ok(Bot {
        id: r.get(0)?,
        name: r.get(1)?,
        engine: engine.parse().unwrap_or(EngineKind::Claude),
        model: r.get(3)?,
        soul: r.get(4)?,
        avatar_seed: r.get(5)?,
        cwd: r.get(6)?,
        auto_approve: r.get(7)?,
        archived: r.get(8)?,
        computer_id: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        last_message: r.get(12)?,
        last_activity_at: r.get(13)?,
        unread: r.get(14)?,
        working: r.get(15)?,
    })
}

fn row_to_message(r: &rusqlite::Row) -> rusqlite::Result<Message> {
    let role: String = r.get(2)?;
    let engine: Option<String> = r.get(4)?;
    let segments: String = r.get(6)?;
    Ok(Message {
        id: r.get(0)?,
        thread_id: r.get(1)?,
        role: match role.as_str() {
            "bot" => Role::Bot,
            "system" => Role::System,
            "tool" => Role::Tool,
            _ => Role::User,
        },
        text: r.get(3)?,
        engine: engine.and_then(|e| e.parse().ok()),
        model: r.get(5)?,
        segments: serde_json::from_str(&segments).unwrap_or_default(),
        pending: r.get(7)?,
        error: r.get(8)?,
        created_at: r.get(9)?,
    })
}

fn row_to_approval(r: &rusqlite::Row) -> rusqlite::Result<ApprovalRequest> {
    let status: String = r.get(8)?;
    let options: String = r.get(7)?;
    Ok(ApprovalRequest {
        id: r.get(0)?,
        bot_id: r.get(1)?,
        thread_id: r.get(2)?,
        request_id: r.get(3)?,
        kind: r.get(4)?,
        title: r.get(5)?,
        detail: r.get(6)?,
        options: serde_json::from_str(&options).unwrap_or_default(),
        status: match status.as_str() {
            "allowed" => ApprovalStatus::Allowed,
            "denied" => ApprovalStatus::Denied,
            "answered" => ApprovalStatus::Answered,
            _ => ApprovalStatus::Pending,
        },
        response: r.get(9)?,
        created_at: r.get(10)?,
    })
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS bots (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  engine TEXT NOT NULL,
  model TEXT,
  soul TEXT NOT NULL DEFAULT '',
  avatar_seed INTEGER NOT NULL DEFAULT 0,
  cwd TEXT,
  auto_approve INTEGER NOT NULL DEFAULT 0,
  archived INTEGER NOT NULL DEFAULT 0,
  computer_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  last_message TEXT,
  last_activity_at INTEGER,
  unread INTEGER NOT NULL DEFAULT 0,
  working INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS threads (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL DEFAULT 'direct',
  bot_id TEXT,
  title TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL,
  role TEXT NOT NULL,
  text TEXT NOT NULL DEFAULT '',
  engine TEXT,
  model TEXT,
  segments TEXT NOT NULL DEFAULT '[]',
  pending INTEGER NOT NULL DEFAULT 0,
  error TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id, created_at);
CREATE TABLE IF NOT EXISTS approvals (
  id TEXT PRIMARY KEY,
  bot_id TEXT NOT NULL,
  thread_id TEXT NOT NULL,
  request_id TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT 'tool_use',
  title TEXT NOT NULL,
  detail TEXT NOT NULL DEFAULT '',
  options TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL DEFAULT 'pending',
  response TEXT,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS computers (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'off',
  url TEXT,
  preview TEXT,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS kv (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;
