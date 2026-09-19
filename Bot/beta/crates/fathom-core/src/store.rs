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
    // --- WireBot parity fields ---
    #[serde(default)]
    projects: Vec<BotProject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_variant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mascot_expression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mascot_body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    avatar_crop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    computer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cloud_backend: Option<String>,
    #[serde(default)]
    auto_start_vps: bool,
    #[serde(default)]
    speak_replies: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    voice: Option<String>,
    #[serde(default)]
    rewound: bool,
    #[serde(default)]
    chief_of_staff: bool,
    #[serde(default)]
    managed_sections: Vec<String>,
    #[serde(default)]
    approve_peer_comms: bool,
    #[serde(default)]
    peers: Vec<String>,
    #[serde(default)]
    composio: bool,
    #[serde(default)]
    browser: bool,
    #[serde(default)]
    mcp_servers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    browser_profile: Option<String>,
    #[serde(default)]
    playbooks: Vec<InstalledPlaybook>,
}

fn default_true() -> bool {
    true
}

/// Thread fields that live in the `extra` JSON column.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ThreadExtra {
    #[serde(default)]
    title_from_first_message: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    archived_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(default)]
    rewound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_started_at: Option<i64>,
}

/// Room fields that live in the `extra` JSON column.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RoomExtra {
    #[serde(default)]
    bulletin: String,
    #[serde(default)]
    dm: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    busy_bot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_started_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    setup_completed_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    setup_skipped_at: Option<i64>,
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
    // --- WireMessage parity fields ---
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    send_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    channel_mode: Option<String>,
    #[serde(default)]
    turn_terminal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    queue_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    png: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    comm: Option<CommChip>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_ref: Option<ThreadRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    peer_post: Option<PeerRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    peer_ask: Option<PeerRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    room_request: Option<RoomRequest>,
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
        ensure_column(&conn, "threads", "room_id", "TEXT")?;
        ensure_column(&conn, "threads", "extra", "TEXT NOT NULL DEFAULT '{}'")?;
        ensure_column(&conn, "rooms", "extra", "TEXT NOT NULL DEFAULT '{}'")?;
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
            projects: bot.projects.clone(),
            model_variant: bot.model_variant.clone(),
            mascot_expression: bot.mascot_expression.clone(),
            mascot_body: bot.mascot_body.clone(),
            avatar_crop: bot.avatar_crop.clone(),
            computer: bot.computer.clone(),
            cloud_backend: bot.cloud_backend.clone(),
            auto_start_vps: bot.auto_start_vps,
            speak_replies: bot.speak_replies,
            voice: bot.voice.clone(),
            rewound: bot.rewound,
            chief_of_staff: bot.chief_of_staff,
            managed_sections: bot.managed_sections.clone(),
            approve_peer_comms: bot.approve_peer_comms,
            peers: bot.peers.clone(),
            composio: bot.composio,
            browser: bot.browser,
            mcp_servers: bot.mcp_servers.clone(),
            browser_profile: bot.browser_profile.clone(),
            playbooks: bot.playbooks.clone(),
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
        let bot = self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,name,engine,model,soul,avatar_seed,cwd,auto_approve,archived,computer_id,created_at,updated_at,last_message,last_activity_at,unread,working,extra FROM bots WHERE id=?1",
                params![id],
                row_to_bot,
            )
            .optional()?;
        Ok(bot.map(|mut b| {
            self.decorate_bot(&mut b);
            b
        }))
    }

    /// Fill read-time fields: the SOUL.md mirror hash/drift and the active
    /// thread's pinned message. Runs outside the connection lock.
    fn decorate_bot(&self, b: &mut Bot) {
        if let Ok(disk) = std::fs::read_to_string(self.soul_path(&b.id)) {
            b.soul_hash = Some(fnv64(disk.as_bytes()));
            b.soul_drift = disk != b.soul;
        }
        b.pinned_message_id = self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT pinned_message_id FROM threads WHERE bot_id=?1 AND kind='direct' ORDER BY created_at LIMIT 1",
                params![b.id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten();
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
        drop(stmt);
        drop(conn);
        for b in &mut bots {
            self.decorate_bot(b);
        }
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
        let extra = serde_json::to_string(&ThreadExtra {
            title_from_first_message: t.title_from_first_message,
            archived_at: t.archived_at,
            cwd: t.cwd.clone(),
            rewound: t.rewound,
            turn_started_at: t.turn_started_at,
        })?;
        self.conn.lock().unwrap().execute(
            "INSERT INTO threads (id,kind,bot_id,title,pinned_message_id,created_at,updated_at,room_id,extra) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title, pinned_message_id=excluded.pinned_message_id,
               room_id=excluded.room_id, extra=excluded.extra, updated_at=excluded.updated_at",
            params![t.id, t.kind, t.bot_id, t.title, t.pinned_message_id, t.created_at, t.updated_at, t.room_id, extra],
        )?;
        Ok(())
    }

    pub fn get_thread(&self, id: &str) -> Result<Option<Thread>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,kind,bot_id,title,pinned_message_id,created_at,updated_at,room_id,extra FROM threads WHERE id=?1",
                params![id],
                row_to_thread,
            )
            .optional()?)
    }

    /// All direct threads ("tasks") a bot owns, newest first.
    pub fn bot_threads(&self, bot_id: &str) -> Result<Vec<Thread>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,kind,bot_id,title,pinned_message_id,created_at,updated_at,room_id,extra FROM threads WHERE bot_id=?1 AND kind='direct' ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map(params![bot_id], row_to_thread)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// All task threads a room owns (GroupTask list), newest first.
    pub fn room_threads(&self, room_id: &str) -> Result<Vec<Thread>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,kind,bot_id,title,pinned_message_id,created_at,updated_at,room_id,extra FROM threads WHERE room_id=?1 ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map(params![room_id], row_to_thread)?;
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
            room_id: None,
            title,
            pinned_message_id: None,
            title_from_first_message: false,
            archived_at: None,
            cwd: None,
            rewound: false,
            turn_started_at: None,
            created_at: now,
            updated_at: now,
        };
        self.upsert_thread(&t)?;
        Ok(t)
    }

    /// Create a task thread inside a room (GroupTask channel).
    pub fn new_room_thread(
        &self,
        room_id: &str,
        title: Option<String>,
    ) -> Result<Thread, StoreError> {
        let now = now_ms();
        let t = Thread {
            id: new_id("th"),
            kind: "room".into(),
            bot_id: None,
            room_id: Some(room_id.into()),
            title,
            pinned_message_id: None,
            title_from_first_message: false,
            archived_at: None,
            cwd: None,
            rewound: false,
            turn_started_at: None,
            created_at: now,
            updated_at: now,
        };
        self.upsert_thread(&t)?;
        Ok(t)
    }

    /// Set the thread title from its first user message exactly once —
    /// never overwrites a title the person already set.
    pub fn title_thread_once(&self, thread_id: &str, title: &str) -> Result<bool, StoreError> {
        let changed = self.conn.lock().unwrap().execute(
            "UPDATE threads SET title=?2, extra=json_set(COALESCE(extra,'{}'),'$.title_from_first_message',1), updated_at=?3
             WHERE id=?1 AND title IS NULL",
            params![thread_id, title, now_ms()],
        )?;
        if changed == 0 {
            // Still flip the flag so later sends don't retry.
            self.conn.lock().unwrap().execute(
                "UPDATE threads SET extra=json_set(COALESCE(extra,'{}'),'$.title_from_first_message',1) WHERE id=?1",
                params![thread_id],
            )?;
        }
        Ok(changed > 0)
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
                "SELECT id,kind,bot_id,title,pinned_message_id,created_at,updated_at,room_id,extra FROM threads WHERE bot_id=?1 AND kind='direct' ORDER BY created_at LIMIT 1",
                params![bot_id],
                row_to_thread,
            )
            .optional()?
        {
            return Ok(t);
        }
        self.new_task_thread(bot_id, None)
    }

    /// Idempotent-send lookup: find the user message a retry would duplicate.
    pub fn find_by_send_id(
        &self,
        thread_id: &str,
        send_id: &str,
    ) -> Result<Option<Message>, StoreError> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id,thread_id,role,text,engine,model,segments,pending,error,created_at,extra FROM messages
                 WHERE thread_id=?1 AND json_extract(extra,'$.send_id')=?2 LIMIT 1",
                params![thread_id, send_id],
                row_to_message,
            )
            .optional()?)
    }

    // ---- rooms --------------------------------------------------------------

    pub fn upsert_room(&self, r: &Room) -> Result<(), StoreError> {
        let extra = serde_json::to_string(&RoomExtra {
            bulletin: r.bulletin.clone(),
            dm: r.dm,
            section: r.section.clone(),
            cwd: r.cwd.clone(),
            busy_bot_id: r.busy_bot_id.clone(),
            turn_started_at: r.turn_started_at,
            setup_completed_at: r.setup_completed_at,
            setup_skipped_at: r.setup_skipped_at,
        })?;
        self.conn.lock().unwrap().execute(
            "INSERT INTO rooms (id,name,member_ids,thread_id,responder,unread,last_message,last_activity_at,created_at,updated_at,extra)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, member_ids=excluded.member_ids,
               responder=excluded.responder, thread_id=excluded.thread_id, unread=excluded.unread,
               last_message=excluded.last_message,
               last_activity_at=excluded.last_activity_at, updated_at=excluded.updated_at, extra=excluded.extra",
            params![
                r.id, r.name, serde_json::to_string(&r.member_ids)?, r.thread_id,
                serde_json::to_string(&r.responder)?,
                r.unread, r.last_message, r.last_activity_at, r.created_at, r.updated_at, extra,
            ],
        )?;
        Ok(())
    }

    /// Set/clear the transient busy-speaker fields on a room.
    pub fn set_room_busy(&self, room_id: &str, bot_id: Option<&str>) -> Result<(), StoreError> {
        if let Some(mut r) = self.get_room(room_id)? {
            r.busy_bot_id = bot_id.map(|s| s.to_string());
            r.turn_started_at = bot_id.map(|_| now_ms());
            self.upsert_room(&r)?;
        }
        Ok(())
    }

    fn room_from_row(r: &rusqlite::Row) -> rusqlite::Result<Room> {
        let members: String = r.get(2)?;
        let responder: String = r.get(4)?;
        let extra_raw: Option<String> = r.get(10).ok();
        let extra: RoomExtra = extra_raw
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        Ok(Room {
            id: r.get(0)?,
            name: r.get(1)?,
            member_ids: serde_json::from_str(&members).unwrap_or_default(),
            thread_id: r.get(3)?,
            responder: serde_json::from_str(&responder).unwrap_or_default(),
            bulletin: extra.bulletin,
            dm: extra.dm,
            section: extra.section,
            cwd: extra.cwd,
            working: false,
            busy_bot_id: extra.busy_bot_id,
            turn_started_at: extra.turn_started_at,
            setup_completed_at: extra.setup_completed_at,
            setup_skipped_at: extra.setup_skipped_at,
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
                "SELECT id,name,member_ids,thread_id,responder,unread,last_message,last_activity_at,created_at,updated_at,extra FROM rooms WHERE id=?1",
                params![id],
                Self::room_from_row,
            )
            .optional()?)
    }

    /// The room that owns a thread — via the room's active thread_id or a
    /// task thread carrying room_id (GroupTask channels).
    pub fn get_room_by_thread(&self, thread_id: &str) -> Result<Option<Room>, StoreError> {
        let conn = self.conn.lock().unwrap();
        if let Some(r) = conn
            .query_row(
                "SELECT id,name,member_ids,thread_id,responder,unread,last_message,last_activity_at,created_at,updated_at,extra FROM rooms WHERE thread_id=?1",
                params![thread_id],
                Self::room_from_row,
            )
            .optional()?
        {
            return Ok(Some(r));
        }
        Ok(conn
            .query_row(
                "SELECT r.id,r.name,r.member_ids,r.thread_id,r.responder,r.unread,r.last_message,r.last_activity_at,r.created_at,r.updated_at,r.extra
                 FROM rooms r JOIN threads t ON t.room_id = r.id WHERE t.id=?1",
                params![thread_id],
                Self::room_from_row,
            )
            .optional()?)
    }

    pub fn list_rooms(&self) -> Result<Vec<Room>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,name,member_ids,thread_id,responder,unread,last_message,last_activity_at,created_at,updated_at,extra FROM rooms ORDER BY COALESCE(last_activity_at, created_at) DESC",
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
            parent_id: m.parent_id.clone(),
            send_id: m.send_id.clone(),
            channel_mode: m.channel_mode.clone(),
            turn_terminal: m.turn_terminal,
            queue_id: m.queue_id.clone(),
            png: m.png.clone(),
            mime: m.mime.clone(),
            comm: m.comm.clone(),
            thread_ref: m.thread_ref.clone(),
            peer_post: m.peer_post.clone(),
            peer_ask: m.peer_ask.clone(),
            room_request: m.room_request.clone(),
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
        let like = format!("%{}%", q.replace(['%', '_'], ""));
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

    /// Decision log — all approvals (any status), newest first (`/api/decisions`).
    pub fn recent_approvals(&self, limit: i64) -> Result<Vec<ApprovalRequest>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id,bot_id,thread_id,request_id,kind,title,detail,tool,held,options,status,response,created_at FROM approvals ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_approval)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Distinct sidebar section labels across bots and rooms.
    pub fn sidebar_sections(&self) -> Result<Vec<String>, StoreError> {
        let mut out: Vec<String> = Vec::new();
        for b in self.list_bots_filtered(false)? {
            if let Some(s) = b.section {
                if !s.is_empty() && !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        for r in self.list_rooms()? {
            if let Some(s) = r.section {
                if !s.is_empty() && !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        out.sort();
        Ok(out)
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

/// FNV-1a 64 — stable content hash for the SOUL.md mirror (drift detection).
fn fnv64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
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
        projects: extra.projects,
        model_variant: extra.model_variant,
        soul_hash: None,
        soul_drift: false,
        mascot_expression: extra.mascot_expression,
        mascot_body: extra.mascot_body,
        avatar_crop: extra.avatar_crop,
        computer: extra.computer,
        cloud_backend: extra.cloud_backend,
        auto_start_vps: extra.auto_start_vps,
        speak_replies: extra.speak_replies,
        voice: extra.voice,
        rewound: extra.rewound,
        chief_of_staff: extra.chief_of_staff,
        managed_sections: extra.managed_sections,
        approve_peer_comms: extra.approve_peer_comms,
        peers: extra.peers,
        composio: extra.composio,
        browser: extra.browser,
        mcp_servers: extra.mcp_servers,
        browser_profile: extra.browser_profile,
        playbooks: extra.playbooks,
        pinned_message_id: None,
    })
}

fn row_to_thread(r: &rusqlite::Row) -> rusqlite::Result<Thread> {
    let extra_raw: Option<String> = r.get(8).ok();
    let extra: ThreadExtra = extra_raw
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    Ok(Thread {
        id: r.get(0)?,
        kind: r.get(1)?,
        bot_id: r.get(2)?,
        title: r.get(3)?,
        pinned_message_id: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
        room_id: r.get(7)?,
        title_from_first_message: extra.title_from_first_message,
        archived_at: extra.archived_at,
        cwd: extra.cwd,
        rewound: extra.rewound,
        turn_started_at: extra.turn_started_at,
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
        parent_id: extra.parent_id,
        send_id: extra.send_id,
        channel_mode: extra.channel_mode,
        turn_terminal: extra.turn_terminal,
        queue_id: extra.queue_id,
        png: extra.png,
        mime: extra.mime,
        comm: extra.comm,
        thread_ref: extra.thread_ref,
        peer_post: extra.peer_post,
        peer_ask: extra.peer_ask,
        room_request: extra.room_request,
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
        held_code: None,
        allow_session: true,
        dismissed: false,
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
  updated_at INTEGER NOT NULL,
  room_id TEXT,
  extra TEXT NOT NULL DEFAULT '{}'
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
  updated_at INTEGER NOT NULL,
  extra TEXT NOT NULL DEFAULT '{}'
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
