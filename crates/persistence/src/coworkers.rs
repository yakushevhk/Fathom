use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Persistence;

const MAX_ID: usize = 128;
const MAX_NAME: usize = 200;
const MAX_TITLE: usize = 200;
const MAX_ROLE: usize = 100;
const MAX_PROMPT: usize = 32_000;
const MAX_VISIBILITY: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelRow {
    pub id: String,
    pub coworker_id: String,
    pub title: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn bounded(value: &str, max: usize, field: &str) -> anyhow::Result<String> {
    let value = value.trim();
    if value.chars().count() > max {
        anyhow::bail!("{field} exceeds maximum length of {max}");
    }
    Ok(value.to_owned())
}

fn required(value: &str, max: usize, field: &str) -> anyhow::Result<String> {
    let value = bounded(value, max, field)?;
    if value.is_empty() {
        anyhow::bail!("{field} must not be empty");
    }
    Ok(value)
}

fn id(value: &str) -> anyhow::Result<String> {
    required(value, MAX_ID, "id")
}

fn coworker_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CoworkerRow> {
    Ok(CoworkerRow {
        id: row.get(0)?,
        name: row.get(1)?,
        title: row.get(2)?,
        role: row.get(3)?,
        prompt: row.get(4)?,
        visibility: row.get(5)?,
        active: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn channel_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChannelRow> {
    Ok(ChannelRow {
        id: row.get(0)?,
        coworker_id: row.get(1)?,
        title: row.get(2)?,
        session_id: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

impl Persistence {
    pub fn create_coworker(&self, name: &str, title: &str, role: &str, prompt: &str, visibility: &str, active: bool) -> anyhow::Result<CoworkerRow> {
        let name = required(name, MAX_NAME, "name")?;
        let title = bounded(title, MAX_TITLE, "title")?;
        let role = bounded(role, MAX_ROLE, "role")?;
        let prompt = required(prompt, MAX_PROMPT, "prompt")?;
        let visibility = required(visibility, MAX_VISIBILITY, "visibility")?;
        let id = Uuid::now_v7().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute("INSERT INTO coworkers (id,name,title,role,prompt,visibility,active,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)", params![id, name, title, role, prompt, visibility, active as i64, now])?;
        Ok(CoworkerRow { id, name, title, role, prompt, visibility, active, created_at: now.clone(), updated_at: now })
    }

    pub fn get_coworker(&self, coworker_id: &str) -> anyhow::Result<Option<CoworkerRow>> {
        let coworker_id = id(coworker_id)?;
        let conn = self.conn.lock();
        Ok(conn.query_row("SELECT id,name,title,role,prompt,visibility,active,created_at,updated_at FROM coworkers WHERE id=?1", params![coworker_id], coworker_from_row).optional()?)
    }

    pub fn list_coworkers(&self) -> anyhow::Result<Vec<CoworkerRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id,name,title,role,prompt,visibility,active,created_at,updated_at FROM coworkers ORDER BY updated_at DESC, id")?;
        let rows = stmt.query_map([], coworker_from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn update_coworker(&self, coworker_id: &str, name: &str, title: &str, role: &str, prompt: &str, visibility: &str, active: bool) -> anyhow::Result<Option<CoworkerRow>> {
        let coworker_id = id(coworker_id)?;
        let name = required(name, MAX_NAME, "name")?;
        let title = bounded(title, MAX_TITLE, "title")?;
        let role = bounded(role, MAX_ROLE, "role")?;
        let prompt = required(prompt, MAX_PROMPT, "prompt")?;
        let visibility = required(visibility, MAX_VISIBILITY, "visibility")?;
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        let changed = conn.execute("UPDATE coworkers SET name=?1,title=?2,role=?3,prompt=?4,visibility=?5,active=?6,updated_at=?7 WHERE id=?8", params![name,title,role,prompt,visibility,active as i64,now,coworker_id])?;
        if changed == 0 { return Ok(None); }
        Ok(Some(conn.query_row("SELECT id,name,title,role,prompt,visibility,active,created_at,updated_at FROM coworkers WHERE id=?1", params![coworker_id], coworker_from_row)?))
    }

    pub fn delete_coworker(&self, coworker_id: &str) -> anyhow::Result<bool> {
        let coworker_id = id(coworker_id)?;
        let mut conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM channels WHERE coworker_id=?1", params![coworker_id])?;
        let deleted = tx.execute("DELETE FROM coworkers WHERE id=?1", params![coworker_id])? != 0;
        tx.commit()?;
        Ok(deleted)
    }

    pub fn create_channel(&self, coworker_id: &str, title: &str, session_id: Option<&str>) -> anyhow::Result<ChannelRow> {
        let coworker_id = id(coworker_id)?;
        let title = required(title, MAX_TITLE, "title")?;
        let session_id = session_id.map(|v| required(v, MAX_ID, "session_id")).transpose()?;
        let channel_id = Uuid::now_v7().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM coworkers WHERE id=?1)", params![coworker_id], |row| row.get(0))?;
        if !exists { anyhow::bail!("coworker not found"); }
        if let Some(session_id) = &session_id {
            let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)", params![session_id], |row| row.get(0))?;
            if !exists { anyhow::bail!("session not found"); }
        }
        conn.execute("INSERT INTO channels (id,coworker_id,title,session_id,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?5)", params![channel_id,coworker_id,title,session_id,now])?;
        Ok(ChannelRow { id: channel_id, coworker_id, title, session_id, created_at: now.clone(), updated_at: now })
    }

    pub fn get_channel(&self, channel_id: &str) -> anyhow::Result<Option<ChannelRow>> {
        let channel_id = id(channel_id)?;
        let conn = self.conn.lock();
        Ok(conn.query_row("SELECT id,coworker_id,title,session_id,created_at,updated_at FROM channels WHERE id=?1", params![channel_id], channel_from_row).optional()?)
    }

    pub fn list_channels(&self, coworker_id: &str) -> anyhow::Result<Vec<ChannelRow>> {
        let coworker_id = id(coworker_id)?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id,coworker_id,title,session_id,created_at,updated_at FROM channels WHERE coworker_id=?1 ORDER BY updated_at DESC,id")?;
        let rows = stmt.query_map(params![coworker_id], channel_from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn update_channel(&self, channel_id: &str, title: &str, session_id: Option<&str>) -> anyhow::Result<Option<ChannelRow>> {
        let channel_id = id(channel_id)?;
        let title = required(title, MAX_TITLE, "title")?;
        let session_id = session_id.map(|v| required(v, MAX_ID, "session_id")).transpose()?;
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        if let Some(session_id) = &session_id {
            let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)", params![session_id], |row| row.get(0))?;
            if !exists { anyhow::bail!("session not found"); }
        }
        let changed = conn.execute("UPDATE channels SET title=?1,session_id=?2,updated_at=?3 WHERE id=?4", params![title,session_id,now,channel_id])?;
        if changed == 0 { return Ok(None); }
        Ok(Some(conn.query_row("SELECT id,coworker_id,title,session_id,created_at,updated_at FROM channels WHERE id=?1", params![channel_id], channel_from_row)?))
    }

    pub fn delete_channel(&self, channel_id: &str) -> anyhow::Result<bool> {
        let channel_id = id(channel_id)?;
        Ok(self.conn.lock().execute("DELETE FROM channels WHERE id=?1", params![channel_id])? != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coworker_channel_crud_and_linkage() {
        let db = Persistence::in_memory().unwrap();
        let c = db.create_coworker("Alice", "Title", "assistant", "Prompt", "private", true).unwrap();
        assert_eq!(db.list_coworkers().unwrap().len(), 1);
        let c = db.update_coworker(&c.id, "Alicia", "Lead", "assistant", "Updated prompt", "team", false).unwrap().unwrap();
        assert_eq!(c.name, "Alicia");
        assert!(!c.active);
        let ch = db.create_channel(&c.id, "General", None).unwrap();
        assert_eq!(db.list_channels(&c.id).unwrap()[0].id, ch.id);
        let session = pr_core::SessionId::new();
        db.create_session(&session, "channel session").unwrap();
        let updated = db.update_channel(&ch.id, "Renamed", Some(&session.0)).unwrap().unwrap();
        assert_eq!(updated.title, "Renamed");
        assert!(db.delete_channel(&ch.id).unwrap());
        assert!(db.delete_coworker(&c.id).unwrap());
        assert!(db.get_coworker(&c.id).unwrap().is_none());
    }
}
