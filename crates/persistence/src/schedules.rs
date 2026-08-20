use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDateTime, Timelike, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;
use crate::Persistence;

const MAX_ID: usize = 128;
const MAX_CRON: usize = 256;
const MAX_TIMEZONE: usize = 128;
const MAX_QUERY: usize = 20_000;
const MAX_CLAIM: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

fn required(value: &str, max: usize, field: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() { bail!("{field} must not be empty"); }
    if value.chars().count() > max { bail!("{field} exceeds maximum length of {max}"); }
    Ok(value.to_owned())
}
fn valid_id(value: &str) -> Result<String> {
    let value = required(value, MAX_ID, "id")?;
    if !value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')) { bail!("id contains invalid characters"); }
    Ok(value)
}

/// Validate a five-field cron expression without executing any command.
pub fn validate_cron(value: &str) -> Result<String> {
    let value = required(value, MAX_CRON, "cron_expression")?;
    if value.starts_with('@') { bail!("cron_expression must contain five fields"); }
    let fields: Vec<_> = value.split_whitespace().collect();
    if fields.len() != 5 { bail!("cron_expression must contain five fields"); }
    for (field, (min, max)) in fields.iter().zip([(0,59),(0,23),(1,31),(1,12),(0,7)]) {
        parse_field(field, min, max).map_err(|e| anyhow!("invalid cron_expression: {e}"))?;
    }
    Ok(value)
}
fn parse_field(value: &str, min: u32, max: u32) -> Result<Vec<u32>> {
    if value.is_empty() { bail!("empty field"); }
    let mut out = Vec::new();
    for item in value.split(',') {
        let (base, step) = match item.split_once('/') {
            Some((base, step)) => (base, step.parse::<u32>().map_err(|_| anyhow!("invalid step"))?),
            None => (item, 1),
        };
        if step == 0 { bail!("step must be positive"); }
        let (lo, hi) = if base == "*" { (min, max) }
            else if let Some((a,b)) = base.split_once('-') { (a.parse().map_err(|_| anyhow!("invalid range"))?, b.parse().map_err(|_| anyhow!("invalid range"))?) }
            else { let n = base.parse::<u32>().map_err(|_| anyhow!("invalid value"))?; (n,n) };
        if lo < min || hi > max || lo > hi { bail!("value outside allowed range"); }
        let mut n = lo;
        while n <= hi { out.push(n); if hi - n < step { break; } n += step; }
    }
    if out.is_empty() { bail!("empty field"); }
    out.sort_unstable(); out.dedup(); Ok(out)
}

/// Validate UTC, fixed offsets, or an existing IANA zoneinfo path.
pub fn validate_timezone(value: &str) -> Result<String> {
    let value = required(value, MAX_TIMEZONE, "timezone")?;
    if value == "UTC" || value == "Etc/UTC" || parse_offset(&value).is_some() { return Ok(value); }
    if value.starts_with('/') || value.contains("..") || !value.contains('/') { bail!("timezone must be UTC, a fixed offset, or an IANA timezone"); }
    if !Path::new("/usr/share/zoneinfo").join(&value).is_file() { bail!("unknown timezone"); }
    Ok(value)
}
fn parse_offset(tz: &str) -> Option<i32> {
    if let Some(rest) = tz.strip_prefix("UTC+").or_else(|| tz.strip_prefix("UTC-")) {
        let sign = if tz.as_bytes()[3] == b'+' { 1 } else { -1 };
        let (h,m) = rest.split_once(':').map_or((rest,"0"), |(h,m)|(h,m));
        let hours = h.parse::<i32>().ok()?; let mins = m.parse::<i32>().ok()?;
        if hours > 23 || mins > 59 { return None; }
        return Some(sign * (hours * 3600 + mins * 60));
    }
    if let Some(rest) = tz.strip_prefix("Etc/GMT+").or_else(|| tz.strip_prefix("Etc/GMT-")) {
        let sign = if tz.contains("GMT+") { -1 } else { 1 };
        return rest.parse::<i32>().ok().filter(|h| *h <= 14).map(|h| sign*h*3600);
    }
    None
}
fn cron_matches(local: &NaiveDateTime, fields: &[Vec<u32>]) -> bool {
    let dow = local.weekday().num_days_from_sunday();
    fields[0].contains(&local.minute()) && fields[1].contains(&local.hour()) && fields[2].contains(&local.day()) && fields[3].contains(&local.month()) && (fields[4].contains(&dow) || (dow == 0 && fields[4].contains(&7)))
}
fn next_occurrence(cron: &str, timezone: &str, after: DateTime<Utc>) -> Result<String> {
    let raw: Vec<_> = cron.split_whitespace().collect();
    let fields = [(0,59),(0,23),(1,31),(1,12),(0,7)].iter().zip(raw).map(|(&(lo,hi),v)| parse_field(v,lo,hi)).collect::<Result<Vec<_>>>()?;
    let offset = parse_offset(timezone).unwrap_or(0);
    let local_start = after.naive_utc().with_second(0).and_then(|v| v.with_nanosecond(0)).ok_or_else(|| anyhow!("invalid timestamp"))? + Duration::seconds(offset as i64) + Duration::minutes(1);
    for minute in 0..(366 * 24 * 60) {
        let candidate = local_start + Duration::minutes(minute);
        if cron_matches(&candidate, &fields) { let utc = candidate - Duration::seconds(offset as i64); return Ok(DateTime::<Utc>::from_naive_utc_and_offset(utc,Utc).to_rfc3339()); }
    }
    bail!("cron expression has no occurrence within one year")
}
fn ensure_schema(db: &Persistence) -> Result<()> {
    let conn = db.conn.lock();
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schedules (id TEXT PRIMARY KEY, coworker_id TEXT NOT NULL, cron_expression TEXT NOT NULL, timezone TEXT NOT NULL, query TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, next_run TEXT NOT NULL, last_run TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS idx_schedules_due ON schedules(enabled,next_run); CREATE INDEX IF NOT EXISTS idx_schedules_coworker ON schedules(coworker_id);")?;
    Ok(())
}
fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduleRow> { Ok(ScheduleRow { id: row.get(0)?, coworker_id: row.get(1)?, cron_expression: row.get(2)?, timezone: row.get(3)?, query: row.get(4)?, enabled: row.get::<_,i64>(5)? != 0, next_run: row.get(6)?, last_run: row.get(7)?, created_at: row.get(8)?, updated_at: row.get(9)? }) }
const SELECT: &str = "SELECT id,coworker_id,cron_expression,timezone,query,enabled,next_run,last_run,created_at,updated_at FROM schedules";

impl Persistence {
    pub fn create_schedule(&self, coworker_id: &str, cron_expression: &str, timezone: &str, query: &str, enabled: bool, next_run: Option<&str>) -> Result<ScheduleRow> {
        ensure_schema(self)?; let coworker_id = valid_id(coworker_id)?; let cron_expression = validate_cron(cron_expression)?; let timezone = validate_timezone(timezone)?; let query = required(query,MAX_QUERY,"query")?; let now = Utc::now(); let next_run = match next_run { Some(v) => DateTime::parse_from_rfc3339(v)?.with_timezone(&Utc).to_rfc3339(), None => next_occurrence(&cron_expression,&timezone,now)? }; let now_s = now.to_rfc3339(); let id = Uuid::now_v7().to_string(); let conn = self.conn.lock(); let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM coworkers WHERE id=?1)",params![coworker_id],|r|r.get(0))?; if !exists { bail!("coworker not found"); } conn.execute("INSERT INTO schedules (id,coworker_id,cron_expression,timezone,query,enabled,next_run,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)",params![id,coworker_id,cron_expression,timezone,query,enabled as i64,next_run,now_s])?; Ok(conn.query_row(&format!("{SELECT} WHERE id=?1"),params![id],from_row)?)
    }
    pub fn list_schedules(&self) -> Result<Vec<ScheduleRow>> { ensure_schema(self)?; let conn = self.conn.lock(); let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY next_run,id"))?; let rows = stmt.query_map([],from_row)?.collect::<rusqlite::Result<Vec<_>>>()?; Ok(rows) }
    pub fn get_schedule(&self,id:&str)->Result<Option<ScheduleRow>> { ensure_schema(self)?; let id=valid_id(id)?; let conn=self.conn.lock(); Ok(conn.query_row(&format!("{SELECT} WHERE id=?1"),params![id],from_row).optional()?) }
    pub fn update_schedule(&self,id:&str,coworker_id:&str,cron_expression:&str,timezone:&str,query:&str,enabled:bool,next_run:Option<&str>)->Result<Option<ScheduleRow>> { ensure_schema(self)?; let id=valid_id(id)?; let coworker_id=valid_id(coworker_id)?; let cron_expression=validate_cron(cron_expression)?; let timezone=validate_timezone(timezone)?; let query=required(query,MAX_QUERY,"query")?; let now=Utc::now(); let next_run=match next_run {Some(v)=>DateTime::parse_from_rfc3339(v)?.with_timezone(&Utc).to_rfc3339(),None=>next_occurrence(&cron_expression,&timezone,now)?}; let now_s=now.to_rfc3339(); let conn=self.conn.lock(); let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM coworkers WHERE id=?1)",params![coworker_id],|r|r.get(0))?; if !exists {bail!("coworker not found");} let changed=conn.execute("UPDATE schedules SET coworker_id=?1,cron_expression=?2,timezone=?3,query=?4,enabled=?5,next_run=?6,updated_at=?7 WHERE id=?8",params![coworker_id,cron_expression,timezone,query,enabled as i64,next_run,now_s,id])?; if changed==0{return Ok(None)} Ok(Some(conn.query_row(&format!("{SELECT} WHERE id=?1"),params![id],from_row)?)) }
    pub fn delete_schedule(&self,id:&str)->Result<bool>{ensure_schema(self)?;let id=valid_id(id)?;Ok(self.conn.lock().execute("DELETE FROM schedules WHERE id=?1",params![id])?!=0)}
    /// Claim at most `limit` due schedules, advancing each row in one transaction.
    pub fn claim_due_schedules(&self,now:DateTime<Utc>,limit:usize)->Result<Vec<ScheduleRow>> { if limit == 0 { return Ok(Vec::new()); } ensure_schema(self)?; let limit=limit.min(MAX_CLAIM); let now_s=now.to_rfc3339(); let conn=self.conn.lock(); let tx=conn.unchecked_transaction()?; let mut stmt=tx.prepare(&format!("{SELECT} WHERE enabled=1 AND next_run<=?1 ORDER BY next_run,id LIMIT ?2"))?; let rows=stmt.query_map(params![now_s,limit as i64],from_row)?.collect::<rusqlite::Result<Vec<_>>>()?; drop(stmt); let mut claimed=Vec::with_capacity(rows.len()); for row in rows { let next=next_occurrence(&row.cron_expression,&row.timezone,now)?; let changed=tx.execute("UPDATE schedules SET next_run=?1,last_run=?2,updated_at=?2 WHERE id=?3 AND next_run=?4 AND enabled=1",params![next,now_s,row.id,row.next_run])?; if changed==1 {let mut updated=row;updated.next_run=next;updated.last_run=Some(now_s.clone());updated.updated_at=now_s.clone();claimed.push(updated);}} tx.commit()?; Ok(claimed) }
}

#[cfg(test)]
mod tests { use super::*; #[test] fn schedule_crud_and_atomic_claim(){let db=Persistence::in_memory().unwrap();let c=db.create_coworker("Alice","","","prompt","private",true).unwrap();let due=(Utc::now()-Duration::minutes(2)).to_rfc3339();let row=db.create_schedule(&c.id,"* * * * *","UTC","do work",true,Some(&due)).unwrap();assert_eq!(db.list_schedules().unwrap().len(),1);let got=db.claim_due_schedules(Utc::now(),10).unwrap();assert_eq!(got.len(),1);assert_eq!(got[0].id,row.id);assert!(db.claim_due_schedules(Utc::now(),10).unwrap().is_empty());assert!(!db.update_schedule(&row.id,&c.id,"*/5 * * * *","UTC","updated",false,None).unwrap().unwrap().enabled);assert!(db.delete_schedule(&row.id).unwrap());} #[test] fn validation(){assert!(validate_cron("* * * * *").is_ok());assert!(validate_cron("/bin/sh -c x").is_err());assert!(validate_timezone("UTC").is_ok());assert!(validate_timezone("../../etc").is_err());} }
