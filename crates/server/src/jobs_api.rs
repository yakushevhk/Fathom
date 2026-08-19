//! HTTP endpoints for the durable background jobs subsystem.
//!
//! Jobs are long-running research tasks executed by a fully detached runner
//! process (`job-run`), so they survive server restarts. The registry lives
//! in a SQLite database shared with the CLI, which means jobs submitted via
//! HTTP are visible to `fathom jobs list` and vice versa.

use crate::{error, json, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use pr_persistence::{pid_alive, terminate_pid, JobRow};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    /// The research task to run in the background.
    pub task: String,
    /// Max attempts (self-healing retries, default 3, cap 10).
    #[serde(default = "default_attempts")]
    pub attempts: i64,
}

fn default_attempts() -> i64 {
    3
}

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    /// How many tail lines to return (default 100, cap 2000).
    #[serde(default = "default_log_lines")]
    lines: usize,
}

fn default_log_lines() -> usize {
    100
}

fn job_json(row: &JobRow) -> serde_json::Value {
    let mut v = serde_json::to_value(row).unwrap_or_default();
    let stale = row.status == "running"
        && row.pid.map(|p| !pid_alive(p)).unwrap_or(false);
    if stale {
        v["status"] = serde_json::json!("stale");
    }
    v
}

fn resolve(state: &AppState, id: &str) -> Result<JobRow, Box<Response>> {
    state
        .jobs
        .get(id)
        .map_err(|e| Box::new(error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())))?
        .ok_or_else(|| Box::new(error(StatusCode::NOT_FOUND, "job not found")))
}

/// `POST /api/v1/jobs` — submit a durable background job.
pub(crate) async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateJobRequest>,
) -> Response {
    let task = body.task.trim();
    if task.is_empty() {
        return error(StatusCode::BAD_REQUEST, "task must not be empty");
    }
    if !(1..=10).contains(&body.attempts) {
        return error(
            StatusCode::BAD_REQUEST,
            "attempts must be between 1 and 10",
        );
    }

    let row = match state.jobs.create(task, body.attempts, "") {
        Ok(row) => row,
        Err(e) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to create job: {e}"),
            )
        }
    };
    let job_dir = state.jobs_root.join(&row.id);
    if let Err(e) = std::fs::create_dir_all(&job_dir) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create job dir: {e}"),
        );
    }
    if let Err(e) = state
        .jobs
        .set_output_dir(&row.id, &job_dir.display().to_string())
    {
        return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let log_path = job_dir.join("job.log");
    if let Err(e) = (state.job_spawner)(&row.id, &log_path) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to spawn runner: {e}"),
        );
    }

    let row = match state.jobs.get(&row.id) {
        Ok(Some(r)) => r,
        _ => row,
    };
    let mut v = job_json(&row);
    v["log"] = serde_json::json!(log_path.display().to_string());
    json(StatusCode::ACCEPTED, v)
}

/// `GET /api/v1/jobs` — list all jobs.
pub(crate) async fn list_jobs(State(state): State<Arc<AppState>>) -> Response {
    match state.jobs.list() {
        Ok(rows) => {
            let jobs: Vec<serde_json::Value> = rows.iter().map(job_json).collect();
            let count = jobs.len();
            json(
                StatusCode::OK,
                serde_json::json!({ "jobs": jobs, "count": count }),
            )
        }
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// `GET /api/v1/jobs/:id` — get job status (full id or unique prefix).
pub(crate) async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match resolve(&state, &id) {
        Ok(row) => json(StatusCode::OK, job_json(&row)),
        Err(resp) => *resp,
    }
}

/// `GET /api/v1/jobs/:id/log?lines=N` — tail the job log.
pub(crate) async fn get_job_log(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<LogQuery>,
) -> Response {
    let row = match resolve(&state, &id) {
        Ok(row) => row,
        Err(resp) => return *resp,
    };
    let empty = serde_json::json!({ "lines": [], "total_lines": 0, "returned": 0 });
    if row.output_dir.is_empty() {
        return json(StatusCode::OK, empty);
    }
    let log_path = std::path::Path::new(&row.output_dir).join("job.log");
    let content = match std::fs::read_to_string(&log_path) {
        Ok(c) => c,
        Err(_) => return json(StatusCode::OK, empty),
    };
    let all: Vec<&str> = content.lines().collect();
    let n = query.lines.clamp(1, 2000);
    let start = all.len().saturating_sub(n);
    json(
        StatusCode::OK,
        serde_json::json!({
            "lines": &all[start..],
            "total_lines": all.len(),
            "returned": all.len() - start,
        }),
    )
}

/// `DELETE /api/v1/jobs/:id` — cancel an active job.
pub(crate) async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let row = match resolve(&state, &id) {
        Ok(row) => row,
        Err(resp) => return *resp,
    };
    if row.is_terminal() {
        return error(
            StatusCode::CONFLICT,
            format!("job is not active (status: {})", row.status),
        );
    }
    if let Some(pid) = row.pid {
        if pid_alive(pid) {
            terminate_pid(pid);
        }
    }
    if let Err(e) = state.jobs.mark_cancelled(&row.id) {
        return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    json(
        StatusCode::OK,
        serde_json::json!({ "id": row.id, "status": "cancelled" }),
    )
}

/// `POST /api/v1/jobs/:id/rerun` — re-run a finished or stale job.
pub(crate) async fn rerun_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let row = match resolve(&state, &id) {
        Ok(row) => row,
        Err(resp) => return *resp,
    };
    let reset = match row.status.as_str() {
        "queued" => return error(StatusCode::CONFLICT, "job is already queued"),
        "running" => match row.pid {
            Some(pid) if !pid_alive(pid) => {
                match state.jobs.reset_running_with_pid(&row.id, pid) {
                    Ok(r) => r,
                    Err(e) => {
                        return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                    }
                }
            }
            _ => {
                return error(
                    StatusCode::CONFLICT,
                    "job is still running; cancel it first",
                )
            }
        },
        _ => match state.jobs.reset_for_rerun(&row.id) {
            Ok(r) => r,
            Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        },
    };
    if !reset {
        return error(
            StatusCode::CONFLICT,
            format!("job cannot be re-run from state '{}'", row.status),
        );
    }

    let row = match state.jobs.get(&row.id) {
        Ok(Some(r)) => r,
        _ => row,
    };
    let job_dir = if row.output_dir.is_empty() {
        let dir = state.jobs_root.join(&row.id);
        let _ = state
            .jobs
            .set_output_dir(&row.id, &dir.display().to_string());
        dir
    } else {
        std::path::PathBuf::from(&row.output_dir)
    };
    if let Err(e) = std::fs::create_dir_all(&job_dir) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create job dir: {e}"),
        );
    }
    let log_path = job_dir.join("job.log");
    if let Err(e) = (state.job_spawner)(&row.id, &log_path) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to spawn runner: {e}"),
        );
    }
    json(StatusCode::ACCEPTED, job_json(&row))
}
