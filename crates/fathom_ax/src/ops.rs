//! Read-side helpers for the CLI: status snapshots, log tails, event scans.

use std::path::Path;

use crate::actor;
use crate::controller::AxController;
use crate::error::AxResult;
use crate::manifest::Task;
use crate::store::{AxStore, Event};

/// One-line task row for `fathom ax list` / `status`.
#[derive(Debug)]
pub struct TaskRow {
    pub name: String,
    pub atespace: String,
    pub phase: String,
    pub pid: i64,
    pub actor: String,
    pub created: String,
    pub forked_from: String,
}

/// Project a task list (as returned by `AxStore::list_tasks`, which
/// overlays the authoritative status column) into printable rows.
pub fn task_rows(tasks: Vec<Task>) -> Vec<TaskRow> {
    tasks.into_iter().map(task_row).collect()
}

fn task_row(t: Task) -> TaskRow {
    TaskRow {
        name: t.metadata.name.clone(),
        atespace: t.metadata.atespace_or_default().to_string(),
        phase: t.status.phase.clone(),
        pid: t.status.pid,
        actor: t.status.actor.clone(),
        created: t.metadata.creation_timestamp.clone().unwrap_or_default(),
        forked_from: t.status.forked_from.clone(),
    }
}

/// Stream a task's log from `offset`; returns new offset + chunk.
/// `follow` mode is the caller's loop over this primitive.
pub fn read_task_log(log_path: &str, offset: u64) -> AxResult<(u64, Vec<u8>)> {
    actor::read_log(Path::new(log_path), offset, 64 * 1024)
}

/// Events for an object, oldest-first.
pub fn object_events(
    store: &AxStore,
    kind: &str,
    atespace: &str,
    name: &str,
) -> AxResult<Vec<Event>> {
    store.scan_events(kind, atespace, name, 0)
}

/// Render a task as `ax describe` YAML (status + conditions + events tail).
pub fn describe_task(ctl: &AxController, atespace: &str, name: &str) -> AxResult<String> {
    let task = ctl.store().get_task(atespace, name)?;
    let events = object_events(ctl.store(), "task", atespace, name)?;
    let mut out = serde_yaml::to_string(&task).unwrap_or_default();
    out.push_str("---\n# events (last 20, oldest first)\n");
    for e in events
        .iter()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        out.push_str(&format!(
            "# seq={} {} at {}\n",
            e.seq, e.action, e.created_at
        ));
    }
    Ok(out)
}
