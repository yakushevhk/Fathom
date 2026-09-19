//! SQLite persistence — the whole app state lives in `data_dir/fathom.db`.
//! Local-first: transcripts, bots, rooms, approvals and settings stay on this
//! machine. Fast-growing wire fields ride in `extra` JSON columns so the
//! schema stays stable.

use crate::types::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
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

/// Bot fields that live in the `extra` JSON column.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BotExtra {
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    avatar: Option<String>,
    #[serde(default)]
    approval_mode: Option<ApprovalMode>,
    #[serde(default)]
    always_allow: Vec<String>,
    #[serde(default = "default_true")]
    notifications: bool,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    section: Option<String>,
    #[serde(default = "default_true")]
    park_dms: bool,
}

fn default_true() -> bool {
    true
}

/// Message fields that live in the `extra` JSON column.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MsgExtra {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(default)]
    attachments: Vec<Attachment>,
    #[serde(default)]
    reactions: Vec<Reaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    from_bot: Option<FromBot>,
    #[serde(default)]
    via_api: bool,
    #[serde(default)]
    steered: bool,
    #[serde(default)]
    queued: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
}

impl Store {
    pub fn open(data_dir: PathBuf) -> Result<Self, StoreError> {
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(data_dir.join("souls"))?;
        std::fs::create_dir_all(data_dir.join("files"))?;
        std::fs::create_dir_all(data_dir.join("attachments"))?;
        let conn = Connection::open(data_dir.join("fathom.db"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir,
        };
        store.migrate()?;
        Ok(store)
    }

    /// Additive migrations for DBs created by older beta builds.
    fn migrate(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        ensure_column(&conn, "bots", "extra", "TEXT NOT NULL DEFAULT '{}'")?;
        ensure_column(&conn, "threads", "pinned_message_id", "TEXT")?;
        ensure_column(&conn, "messages", "extra", "TEXT NOT NULL DEFAULT '{}'")?;
        ensure_column(&conn, "approvals", "tool", "TEXT")?;
        ensure_column(&conn, "approvals", "held", "TEXT")?;
        // Legacy auto_approve=1 becomes approval_mode='auto' in the JSON blob.
        conn.execute(
            "UPDATE bots SET extra = json_set(extra, '$.approval_mode', 'auto')
             WHERE auto_approve=1 AND json_extract(extra,'$.approval_mode') IS NULL",
            [],
        )?;
        Ok(())
    }

    // ---- bots ------------------------------------------------------------

    pub fn upsert_bot(&self, bot: &Bot) -> Result<(), StoreError> {
        let extra = serde_json::to_string(&BotExtra {
            title: bot.title.clone(),
            description: bot.description.clone(),
            effort: bot.effort.clone(),
            avatar: bot.avatar.clone(),
            approval_mode: Some(bot.approval_mode),
            always_allow: bot.always_allow.clone(),
            notifications: bot.notifications,
            pinned: bot.pinned,
            hidden: bot.hidden,
            section: bot.section.clone(),
            park_dms: bot.park_dms,
        })?;
        self.conn.lock().unwrap().execute(
            "INSERT INTO bots (id,name,engine,model,soul,avatar_seed,cwd,auto_approve,archived,computer_id,created_at,updated_at,last_message,last_activity_at,unread,working,extra)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, engine=excluded.engine, model=excluded.model,
               soul=excluded.soul, avatar_seed=excluded.avatar_seed, cwd=excluded.cwd,
               archived=excluded.archived,
               computer_id=excluded.computer_id, updated_at=excluded.updated_at,
               last_message=excluded.last_message, last_activity_at=excluded.last_activity_at,
               unread=excluded.unread, working=excluded.working, extra=excluded.extra",
            params![
                bot.id, bot.name, bot.engine.to_string(), bot.model, bot.soul,
                bot.avatar_seed, bot.cwd,
                matches!(bot.approval_mode, ApprovalMode::Auto | ApprovalMode::Full),
                bot.archived, bot.computer_id,
                bot.created_at, bot.updated_at, bot.last_message, bot.last_activity_at,
                bot.unread, bot.working, extra,
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
                "SELECT id,name,engine,model,soul,avatar_seed,cwd,auto_approve,archived,computer_id,created_at,updated_at,last_message,last_activity_at,unread,working,extra FROM bots WHERE id=?1",
                params![id],
                row_to_bot,
            )
            .optional()?)
    }

    /// `archived=None` lists visible bots, `Some(true)` the archive,
    /// `Some(false)` is the same as None (kept for callers passing a flag).
    pub fn list_bots(&self) -> Result<Vec<Bot>, StoreError> {
        self.list_bots_filtered(false)
    }

    pub fn list_bots_filtered(&self, archived: bool) -> Result<Vec<Bot>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,name,engine,model,soul,avatar_seed,cwd,auto_approve,archived,computer_id,created_at,updated_at,last_message,last_activity_at,unread,working,extra FROM bots
             WHERE archived=?1 ORDER BY COALESCE(last_activity_at, created_at) DESC",
        )?;
        let rows = stmt.query_map(params![archived], row_to_bot)?;
        let mut bots = rows.collect::<Result<Vec<_>, _>>()?;
        if !archived {
            bots.retain(|b| !b.hidden);
        }
        bots.sort_by(|a, b| {
            b.pinned.cmp(&a.pinned).then(
                b.last_activity_at
                    .unwrap_or(b.created_at)
                    .cmp(&a.last_activity_at.unwrap_or(a.created_at)),
            )
        });
        Ok(bots)
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
            "INSERT INTO threads (id,kind,bot_id,title,pinned_message_id,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, pinned_message_id=excluded.pinned_message_id, updated_at=excluded.updated_at",
            params![t.id, t.kind, t.bot_id, t.title, t.pinned_message_id, t.created_at, t.updated_at],
        )?;
        Ok(())
    }

    pub fn get_thread(&self, id: &str) -> Result<Option<Thread>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,kind,bot_id,title,pinned_message_id,created_at,updated_at FROM threads WHERE id=?1",
                params![id],
                row_to_thread,
            )
            .optional()?)
    }

    /// All direct threads ("tasks") a bot owns, newest first.
    pub fn bot_threads(&self, bot_id: &str) -> Result<Vec<Thread>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,kind,bot_id,title,pinned_message_id,created_at,updated_at FROM threads WHERE bot_id=?1 AND kind='direct' ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map(params![bot_id], row_to_thread)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Create an additional direct thread (task) for a bot.
    pub fn new_task_thread(
        &self,
        bot_id: &str,
        title: Option<String>,
    ) -> Result<Thread, StoreError> {
        let now = now_ms();
        let t = Thread {
            id: new_id("th"),
            kind: "direct".into(),
            bot_id: Some(bot_id.into()),
            title,
            pinned_message_id: None,
            created_at: now,
            updated_at: now,
        };
        self.upsert_thread(&t)?;
        Ok(t)
    }

    pub fn delete_thread(&self, id: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM messages WHERE thread_id=?1", params![id])?;
        conn.execute("DELETE FROM approvals WHERE thread_id=?1", params![id])?;
        conn.execute("DELETE FROM threads WHERE id=?1", params![id])?;
        Ok(())
    }

    /// The primary DM thread for a bot — created on first access.
    pub fn direct_thread(&self, bot_id: &str) -> Result<Thread, StoreError> {
        if let Some(t) = self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,kind,bot_id,title,pinned_message_id,created_at,updated_at FROM threads WHERE bot_id=?1 AND kind='direct' ORDER BY created_at LIMIT 1",
                params![bot_id],
                row_to_thread,
            )
            .optional()?
        {
            return Ok(t);
        }
        self.new_task_thread(bot_id, None)
    }

    // ---- rooms --------------------------------------------------------------

    pub fn upsert_room(&self, r: &Room) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO rooms (id,name,member_ids,thread_id,responder,unread,last_message,last_activity_at,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, member_ids=excluded.member_ids,
               responder=excluded.responder, unread=excluded.unread, last_message=excluded.last_message,
               last_activity_at=excluded.last_activity_at, updated_at=excluded.updated_at",
            params![
                r.id, r.name, serde_json::to_string(&r.member_ids)?, r.thread_id,
                serde_json::to_string(&r.responder)?,
                r.unread, r.last_message, r.last_activity_at, r.created_at, r.updated_at,
            ],
        )?;
        Ok(())
    }

    fn room_from_row(r: &rusqlite::Row) -> rusqlite::Result<Room> {
        let members: String = r.get(2)?;
        let responder: String = r.get(4)?;
        Ok(Room {
            id: r.get(0)?,
            name: r.get(1)?,
            member_ids: serde_json::from_str(&members).unwrap_or_default(),
            thread_id: r.get(3)?,
            responder: serde_json::from_str(&responder).unwrap_or_default(),
            working: false,
            unread: r.get(5)?,
            last_message: r.get(6)?,
            last_activity_at: r.get(7)?,
            created_at: r.get(8)?,
            updated_at: r.get(9)?,
        })
    }

    pub fn get_room(&self, id: &str) -> Result<Option<Room>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,name,member_ids,thread_id,responder,unread,last_message,last_activity_at,created_at,updated_at FROM rooms WHERE id=?1",
                params![id],
                Self::room_from_row,
            )
            .optional()?)
    }

    pub fn get_room_by_thread(&self, thread_id: &str) -> Result<Option<Room>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,name,member_ids,thread_id,responder,unread,last_message,last_activity_at,created_at,updated_at FROM rooms WHERE thread_id=?1",
                params![thread_id],
                Self::room_from_row,
            )
            .optional()?)
    }

    pub fn list_rooms(&self) -> Result<Vec<Room>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,name,member_ids,thread_id,responder,unread,last_message,last_activity_at,created_at,updated_at FROM rooms ORDER BY COALESCE(last_activity_at, created_at) DESC",
        )?;
        let rows = stmt.query_map([], Self::room_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn delete_room(&self, id: &str) -> Result<(), StoreError> {
        let room = self.get_room(id)?;
        let conn = self.conn.lock().unwrap();
        if let Some(room) = room {
            conn.execute(
                "DELETE FROM messages WHERE thread_id=?1",
                params![room.thread_id],
            )?;
            conn.execute(
                "DELETE FROM approvals WHERE thread_id=?1",
                params![room.thread_id],
            )?;
            conn.execute("DELETE FROM threads WHERE id=?1", params![room.thread_id])?;
        }
        conn.execute("DELETE FROM rooms WHERE id=?1", params![id])?;
        Ok(())
    }

    pub fn touch_room_activity(
        &self,
        room_id: &str,
        snippet: &str,
        mark_unread: bool,
    ) -> Result<(), StoreError> {
        self.conn.lock().unwrap().execute(
            "UPDATE rooms SET last_message=?2, last_activity_at=?3,
               unread = unread + CASE WHEN ?4 THEN 1 ELSE 0 END, updated_at=?3 WHERE id=?1",
            params![
                room_id,
                snippet.chars().take(120).collect::<String>(),
                now_ms(),
                mark_unread
            ],
        )?;
        Ok(())
    }

    pub fn mark_room_read(&self, room_id: &str) -> Result<(), StoreError> {
        self.conn
            .lock()
            .unwrap()
            .execute("UPDATE rooms SET unread=0 WHERE id=?1", params![room_id])?;
        Ok(())
    }

    // ---- messages ----------------------------------------------------------

    pub fn upsert_message(&self, m: &Message) -> Result<(), StoreError> {
        let extra = serde_json::to_string(&MsgExtra {
            kind: Some(m.kind.clone()),
            attachments: m.attachments.clone(),
            reactions: m.reactions.clone(),
            reply_to: m.reply_to.clone(),
            sender: m.sender.clone(),
            from_bot: m.from_bot.clone(),
            via_api: m.via_api,
            steered: m.steered,
            queued: m.queued,
            turn_id: m.turn_id.clone(),
        })?;
        self.conn.lock().unwrap().execute(
            "INSERT INTO messages (id,thread_id,role,text,engine,model,segments,pending,error,created_at,extra)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(id) DO UPDATE SET text=excluded.text, segments=excluded.segments,
               pending=excluded.pending, error=excluded.error, extra=excluded.extra",
            params![
                m.id, m.thread_id,
                match m.role { Role::User => "user", Role::Bot => "bot", Role::System => "system", Role::Tool => "tool" },
                m.text, m.engine.map(|e| e.to_string()), m.model,
                serde_json::to_string(&m.segments)?, m.pending, m.error, m.created_at, extra,
            ],
        )?;
        Ok(())
    }

    pub fn get_message(&self, id: &str) -> Result<Option<Message>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,thread_id,role,text,engine,model,segments,pending,error,created_at,extra FROM messages WHERE id=?1",
                params![id],
                row_to_message,
            )
            .optional()?)
    }

    pub fn list_messages(
        &self,
        thread_id: &str,
        limit: i64,
        before: Option<i64>,
    ) -> Result<Vec<Message>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,thread_id,role,text,engine,model,segments,pending,error,created_at,extra FROM messages
             WHERE thread_id=?1 AND (?2 IS NULL OR created_at<?2) ORDER BY created_at DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![thread_id, before, limit], row_to_message)?;
        let mut out = rows.collect::<Result<Vec<_>, _>>()?;
        out.reverse();
        Ok(out)
    }

    pub fn delete_message(&self, id: &str) -> Result<(), StoreError> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM messages WHERE id=?1", params![id])?;
        Ok(())
    }

    /// Toggle an emoji reaction on a message (by = "user" or a bot id).
    pub fn toggle_reaction(
        &self,
        id: &str,
        emoji: &str,
        by: &str,
    ) -> Result<Option<Message>, StoreError> {
        let Some(mut m) = self.get_message(id)? else {
            return Ok(None);
        };
        if let Some(pos) = m
            .reactions
            .iter()
            .position(|r| r.emoji == emoji && r.by == by)
        {
            m.reactions.remove(pos);
        } else {
            m.reactions.push(Reaction {
                emoji: emoji.into(),
                by: by.into(),
            });
        }
        self.upsert_message(&m)?;
        Ok(Some(m))
    }

    /// Full-text-ish search across transcripts (LIKE on text + segments).
    pub fn search_messages(&self, q: &str, limit: i64) -> Result<Vec<Message>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let like = format!("%{}%", q.replace('%', "").replace('_', ""));
        let mut stmt = conn.prepare(
            "SELECT id,thread_id,role,text,engine,model,segments,pending,error,created_at,extra FROM messages
             WHERE text LIKE ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like, limit], row_to_message)?;
        Ok(rows.collect::<Result<_, _>>()?)
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
            "INSERT INTO approvals (id,bot_id,thread_id,request_id,kind,title,detail,tool,held,options,status,response,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(id) DO UPDATE SET status=excluded.status, response=excluded.response, held=excluded.held",
            params![
                a.id, a.bot_id, a.thread_id, a.request_id, a.kind, a.title, a.detail,
                a.tool, a.held,
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
                "SELECT id,bot_id,thread_id,request_id,kind,title,detail,tool,held,options,status,response,created_at FROM approvals WHERE id=?1",
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
            Some(b) => ("SELECT id,bot_id,thread_id,request_id,kind,title,detail,tool,held,options,status,response,created_at FROM approvals WHERE status='pending' AND bot_id=?1 ORDER BY created_at", vec![Box::new(b.to_string())]),
            None => ("SELECT id,bot_id,thread_id,request_id,kind,title,detail,tool,held,options,status,response,created_at FROM approvals WHERE status='pending' ORDER BY created_at", vec![]),
        };
        let mut stmt = conn.prepare(sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), row_to_approval)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- attachments ----------------------------------------------------------

    pub fn attachments_dir(&self) -> PathBuf {
        self.data_dir.join("attachments")
    }

    /// Persist an uploaded blob; returns the registry entry.
    pub fn save_attachment(
        &self,
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<Attachment, StoreError> {
        let id = new_id("att");
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let file = format!("{}-{}", id, safe.trim_matches('_'));
        let path = self.attachments_dir().join(&file);
        std::fs::write(&path, bytes)?;
        let a = Attachment {
            id,
            name: name.to_string(),
            mime: mime.to_string(),
            kind: if mime.starts_with("image/") {
                "image"
            } else {
                "file"
            }
            .into(),
            size: bytes.len() as i64,
            created_at: now_ms(),
            path: path.display().to_string(),
        };
        self.conn.lock().unwrap().execute(
            "INSERT INTO attachments (id,name,mime,kind,size,path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![a.id, a.name, a.mime, a.kind, a.size, a.path, a.created_at],
        )?;
        Ok(a)
    }

    pub fn get_attachment(&self, id: &str) -> Result<Option<Attachment>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,name,mime,kind,size,path,created_at FROM attachments WHERE id=?1",
                params![id],
                |r| {
                    Ok(Attachment {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        mime: r.get(2)?,
                        kind: r.get(3)?,
                        size: r.get(4)?,
                        path: r.get(5)?,
                        created_at: r.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn attachments_by_ids(&self, ids: &[String]) -> Vec<Attachment> {
        ids.iter()
            .filter_map(|id| self.get_attachment(id).ok().flatten())
            .collect()
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

    /// Per-thread composer draft persistence.
    pub fn get_draft(&self, thread_id: &str) -> Result<Option<String>, StoreError> {
        self.get_kv(&format!("draft:{thread_id}"))
    }

    pub fn set_draft(&self, thread_id: &str, text: &str) -> Result<(), StoreError> {
        self.set_kv(&format!("draft:{thread_id}"), text)
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

fn ensure_column(conn: &Connection, table: &str, col: &str, def: &str) -> Result<(), StoreError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;
    if !cols.iter().any(|c| c == col) {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {col} {def}"), [])?;
    }
    Ok(())
}

fn row_to_bot(r: &rusqlite::Row) -> rusqlite::Result<Bot> {
    let engine: String = r.get(2)?;
    let extra_raw: Option<String> = r.get(16).ok();
    let extra: BotExtra = extra_raw
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let legacy_auto: bool = r.get(7)?;
    Ok(Bot {
        id: r.get(0)?,
        name: r.get(1)?,
        title: extra.title,
        description: extra.description,
        engine: engine.parse().unwrap_or(EngineKind::Claude),
        model: r.get(3)?,
        effort: extra.effort,
        soul: r.get(4)?,
        avatar_seed: r.get(5)?,
        avatar: extra.avatar,
        cwd: r.get(6)?,
        approval_mode: extra.approval_mode.unwrap_or(if legacy_auto {
            ApprovalMode::Auto
        } else {
            ApprovalMode::Ask
        }),
        always_allow: extra.always_allow,
        notifications: extra.notifications,
        pinned: extra.pinned,
        hidden: extra.hidden,
        section: extra.section,
        park_dms: extra.park_dms,
        archived: r.get(8)?,
        computer_id: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        last_message: r.get(12)?,
        last_activity_at: r.get(13)?,
        unread: r.get(14)?,
        working: r.get(15)?,
        waiting_on_you: false,
    })
}

fn row_to_thread(r: &rusqlite::Row) -> rusqlite::Result<Thread> {
    Ok(Thread {
        id: r.get(0)?,
        kind: r.get(1)?,
        bot_id: r.get(2)?,
        title: r.get(3)?,
        pinned_message_id: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

fn row_to_message(r: &rusqlite::Row) -> rusqlite::Result<Message> {
    let role: String = r.get(2)?;
    let engine: Option<String> = r.get(4)?;
    let segments: String = r.get(6)?;
    let extra_raw: Option<String> = r.get(10).ok();
    let extra: MsgExtra = extra_raw
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    Ok(Message {
        id: r.get(0)?,
        thread_id: r.get(1)?,
        role: match role.as_str() {
            "bot" => Role::Bot,
            "system" => Role::System,
            "tool" => Role::Tool,
            _ => Role::User,
        },
        kind: extra.kind.unwrap_or_else(|| "text".into()),
        text: r.get(3)?,
        engine: engine.and_then(|e| e.parse().ok()),
        model: r.get(5)?,
        segments: serde_json::from_str(&segments).unwrap_or_default(),
        attachments: extra.attachments,
        reactions: extra.reactions,
        reply_to: extra.reply_to,
        sender: extra.sender,
        from_bot: extra.from_bot,
        via_api: extra.via_api,
        steered: extra.steered,
        queued: extra.queued,
        turn_id: extra.turn_id,
        pending: r.get(7)?,
        error: r.get(8)?,
        created_at: r.get(9)?,
    })
}

fn row_to_approval(r: &rusqlite::Row) -> rusqlite::Result<ApprovalRequest> {
    let status: String = r.get(10)?;
    let options: String = r.get(9)?;
    Ok(ApprovalRequest {
        id: r.get(0)?,
        bot_id: r.get(1)?,
        thread_id: r.get(2)?,
        request_id: r.get(3)?,
        kind: r.get(4)?,
        title: r.get(5)?,
        detail: r.get(6)?,
        tool: r.get(7)?,
        held: r.get(8)?,
        options: serde_json::from_str(&options).unwrap_or_default(),
        status: match status.as_str() {
            "allowed" => ApprovalStatus::Allowed,
            "denied" => ApprovalStatus::Denied,
            "answered" => ApprovalStatus::Answered,
            _ => ApprovalStatus::Pending,
        },
        response: r.get(11)?,
        created_at: r.get(12)?,
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
  working INTEGER NOT NULL DEFAULT 0,
  extra TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS threads (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL DEFAULT 'direct',
  bot_id TEXT,
  title TEXT,
  pinned_message_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS rooms (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  member_ids TEXT NOT NULL DEFAULT '[]',
  thread_id TEXT NOT NULL,
  responder TEXT NOT NULL DEFAULT '{"kind":"mentions"}',
  unread INTEGER NOT NULL DEFAULT 0,
  last_message TEXT,
  last_activity_at INTEGER,
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
  created_at INTEGER NOT NULL,
  extra TEXT NOT NULL DEFAULT '{}'
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
  tool TEXT,
  held TEXT,
  options TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL DEFAULT 'pending',
  response TEXT,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS attachments (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  mime TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'file',
  size INTEGER NOT NULL DEFAULT 0,
  path TEXT NOT NULL,
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
