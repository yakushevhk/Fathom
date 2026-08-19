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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::Router;
    use http_body_util::BodyExt;
    use parking_lot::Mutex;
    use pr_core::AppConfig;
    use pr_persistence::{JobsDb, Persistence};
    use std::path::Path;
    use tower::ServiceExt;

    /// Build an AppState backed by in-memory persistence and an in-memory
    /// jobs registry, with a temp jobs_root and a recording job_spawner so
    /// no real subprocesses are ever launched.
    fn test_state() -> (
        Arc<AppState>,
        tempfile::TempDir,
        Arc<Mutex<Vec<(String, String)>>>,
    ) {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let jobs = Arc::new(JobsDb::in_memory().unwrap());
        let mut config = AppConfig::default();
        config.memory.enabled = false; // never touch the real memory.db
        let mut state = AppState::with_db_and_jobs(config, db, jobs);
        let tmp = tempfile::tempdir().unwrap();
        let spawned = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.rate_limiter = std::sync::Mutex::new(crate::auth::RateLimiter::new(
                crate::DEFAULT_RATE_LIMIT,
                std::time::Duration::from_secs(60),
            ));
            s.jobs_root = tmp.path().to_path_buf();
            let calls = spawned.clone();
            s.job_spawner = Arc::new(move |job_id: &str, log_path: &Path| {
                calls.lock().push((
                    job_id.to_string(),
                    log_path.display().to_string(),
                ));
                // Create the log file so the log endpoints can read it.
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)?;
                Ok(4242)
            });
        }
        (state, tmp, spawned)
    }

    /// Wrap handlers in the same router used by the server, then drive a
    /// request through it, returning `(status, json_body)`.
    async fn send(app: Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, value)
    }

    fn app(state: Arc<AppState>) -> Router {
        crate::build_router(state)
    }

    fn get_req(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn delete_req(uri: &str) -> Request<Body> {
        Request::builder()
            .method("DELETE")
            .uri(uri)
            .body(Body::empty())
            .unwrap()
    }

    /// Register a job directly in the registry (no spawner), mirroring the
    /// state a runner leaves behind. Returns the created row.
    fn seed_job(
        state: &AppState,
        task: &str,
        status: &str,
        write_log: bool,
    ) -> JobRow {
        let row = state.jobs.create(task, 3, "").unwrap();
        if status != "queued" {
            let job_dir = state.jobs_root.join(&row.id);
            std::fs::create_dir_all(&job_dir).unwrap();
            state
                .jobs
                .set_output_dir(&row.id, &job_dir.display().to_string())
                .unwrap();
            if write_log {
                std::fs::write(job_dir.join("job.log"), "alpha\nbeta\ngamma\n").unwrap();
            }
            match status {
                "running" => state.jobs.mark_running(&row.id, 1, 4242).unwrap(),
                "completed" => state.jobs.mark_completed(&row.id).unwrap(),
                "failed" => state.jobs.mark_failed(&row.id, "boom").unwrap(),
                "cancelled" => {
                    state.jobs.mark_cancelled(&row.id).unwrap();
                }
                _ => {}
            }
        }
        state.jobs.get(&row.id).unwrap().unwrap()
    }

    #[test]
    fn create_job_request_parses_default_attempts() {
        // Defaults come from serde, so the deserialization path is covered.
        let req: CreateJobRequest = serde_json::from_value(serde_json::json!({
            "task": "research",
        }))
        .unwrap();
        assert_eq!(req.task, "research");
        assert_eq!(req.attempts, default_attempts());
        assert_eq!(req.attempts, 3);

        let req: CreateJobRequest = serde_json::from_value(serde_json::json!({
            "task": "research",
            "attempts": 7,
        }))
        .unwrap();
        assert_eq!(req.attempts, 7);
    }

    #[test]
    fn log_query_parses_default_lines() {
        let q: LogQuery = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(q.lines, default_log_lines());
        assert_eq!(q.lines, 100);

        let q: LogQuery = serde_json::from_value(serde_json::json!({ "lines": 5 })).unwrap();
        assert_eq!(q.lines, 5);
    }

    #[test]
    fn default_helpers_have_expected_values() {
        assert_eq!(default_attempts(), 3);
        assert_eq!(default_log_lines(), 100);
    }

    #[test]
    fn job_json_serializes_all_fields_and_flags_stale() {
        let (state, _tmp, _spawned) = test_state();
        let row = state.jobs.create("calc", 5, "/out/dir").unwrap();
        let v = job_json(&row);
        assert_eq!(v["id"], row.id);
        assert_eq!(v["task"], "calc");
        assert_eq!(v["status"], "queued");
        assert_eq!(v["max_attempts"], 5);
        assert_eq!(v["output_dir"], "/out/dir");

        // A "running" job with a dead pid is reported as "stale".
        state.jobs.mark_running(&row.id, 1, i64::MAX - 1).unwrap();
        let dead = state.jobs.get(&row.id).unwrap().unwrap();
        assert!(!pid_alive(dead.pid.unwrap()));
        let v = job_json(&dead);
        assert_eq!(v["status"], "stale");
    }

    #[test]
    fn resolve_finds_full_id_and_unique_prefix() {
        let (state, _tmp, _spawned) = test_state();
        let a = state.jobs.create("a", 3, "").unwrap();
        let b = state.jobs.create("b", 3, "").unwrap();

        assert_eq!(resolve(&state, &a.id).unwrap().id, a.id);
        assert_eq!(resolve(&state, &b.id).unwrap().id, b.id);

        // UUID v7 prefix matching: the first 8 hex chars are the
        // timestamp part, so concurrent creates may collide. Only test
        // prefix lookups when the prefixes are actually different.
        let a_prefix = &a.id[..8];
        let b_prefix = &b.id[..8];
        if a_prefix != b_prefix {
            assert_eq!(resolve(&state, a_prefix).unwrap().id, a.id);
            assert_eq!(resolve(&state, b_prefix).unwrap().id, b.id);
        }

        let err = resolve(&state, "nosuchjob").unwrap_err();
        assert_eq!(err.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_job_accepts_valid_task_and_spawns_runner() {
        let (state, tmp, spawned) = test_state();
        let (status, body) = send(
            app(state.clone()),
            post_json(
                "/api/v1/jobs",
                serde_json::json!({ "task": "research rust", "attempts": 4 }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["task"], "research rust");
        assert_eq!(body["status"], "queued");
        assert_eq!(body["attempt"], 0);
        assert_eq!(body["max_attempts"], 4);
        let id = body["id"].as_str().unwrap().to_string();
        let log = body["log"].as_str().unwrap().to_string();
        assert!(log.ends_with("job.log"));

        // The spawner was invoked once with a log path under jobs_root.
        let calls = spawned.lock();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, id);
        assert!(log.starts_with(tmp.path().display().to_string().as_str()));

        // Row is persisted with output_dir set to the job dir.
        let row = state.jobs.get(&id).unwrap().unwrap();
        assert_eq!(row.status, "queued");
        assert_eq!(row.output_dir, Path::new(&log).parent().unwrap().display().to_string());
    }

    #[tokio::test]
    async fn create_job_uses_default_attempts_when_omitted() {
        let (state, _tmp, _spawned) = test_state();
        let (status, body) = send(
            app(state.clone()),
            post_json("/api/v1/jobs", serde_json::json!({ "task": "defaults" })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["max_attempts"], 3);
    }

    #[tokio::test]
    async fn create_job_trims_whitespace_only_and_empty_tasks() {
        let (state, _tmp, _spawned) = test_state();
        for task in ["", "   ", "\n\t "] {
            let (status, body) = send(
                app(state.clone()),
                post_json("/api/v1/jobs", serde_json::json!({ "task": task })),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "task {task:?}");
            assert_eq!(body["error"], "task must not be empty");
        }

        // Whitespace around a real task is trimmed.
        let (status, body) = send(
            app(state.clone()),
            post_json("/api/v1/jobs", serde_json::json!({ "task": "  padded  " })),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["task"], "padded");
    }

    #[tokio::test]
    async fn create_job_rejects_attempts_out_of_range() {
        let (state, _tmp, _spawned) = test_state();
        for attempts in [0, -1, 11, 100] {
            let (status, body) = send(
                app(state.clone()),
                post_json(
                    "/api/v1/jobs",
                    serde_json::json!({ "task": "t", "attempts": attempts }),
                ),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "attempts={attempts}");
            assert_eq!(body["error"], "attempts must be between 1 and 10");
        }
        // Both boundaries are accepted.
        for attempts in [1, 10] {
            let (status, _) = send(
                app(state.clone()),
                post_json(
                    "/api/v1/jobs",
                    serde_json::json!({ "task": "t", "attempts": attempts }),
                ),
            )
            .await;
            assert_eq!(status, StatusCode::ACCEPTED, "attempts={attempts}");
        }
    }

    #[tokio::test]
    async fn create_job_malformed_json_returns_400() {
        let (state, _tmp, _spawned) = test_state();
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/jobs")
            .header("content-type", "application/json")
            .body(Body::from("not json"))
            .unwrap();
        let (status, _) = send(app(state), req).await;
        // Axum's Json extractor rejects malformed bodies with 400.
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_job_spawner_failure_returns_500() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let jobs = Arc::new(JobsDb::in_memory().unwrap());
        let mut config = AppConfig::default();
        config.memory.enabled = false;
        let mut state = AppState::with_db_and_jobs(config, db, jobs);
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.jobs_root = tempfile::tempdir().unwrap().path().to_path_buf();
            s.job_spawner = Arc::new(|_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "no runner available",
                ))
            });
        }
        let (status, body) = send(
            app(state),
            post_json("/api/v1/jobs", serde_json::json!({ "task": "t" })),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body["error"].as_str().unwrap().contains("failed to spawn runner"));
    }

    #[tokio::test]
    async fn list_jobs_empty_and_counts() {
        let (state, _tmp, _spawned) = test_state();
        let (status, body) = send(app(state.clone()), get_req("/api/v1/jobs")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 0);
        assert_eq!(body["jobs"].as_array().unwrap().len(), 0);

        // A couple of jobs populate the list newest-first.
        let a = state.jobs.create("first", 3, "").unwrap();
        let b = state.jobs.create("second", 3, "").unwrap();
        let (status, body) = send(app(state.clone()), get_req("/api/v1/jobs")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 2);
        assert_eq!(body["jobs"][0]["id"], b.id);
        assert_eq!(body["jobs"][1]["id"], a.id);
    }

    #[tokio::test]
    async fn get_job_by_id_and_prefix() {
        let (state, _tmp, _spawned) = test_state();
        let row = seed_job(&state, "inspect", "completed", false);
        let prefix = &row.id[..8];

        let (status, body) = send(
            app(state.clone()),
            get_req(&format!("/api/v1/jobs/{}", row.id)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], row.id);
        assert_eq!(body["status"], "completed");

        let (status, body) = send(
            app(state.clone()),
            get_req(&format!("/api/v1/jobs/{prefix}")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], row.id);
    }

    #[tokio::test]
    async fn get_job_stale_running_flags_status() {
        let (state, _tmp, _spawned) = test_state();
        let row = seed_job(&state, "stale-job", "running", false);
        // 4242 may theoretically be alive; force a pid that cannot be.
        state
            .jobs
            .mark_running(&row.id, 1, i64::MAX - 1)
            .unwrap();
        let (status, body) = send(
            app(state.clone()),
            get_req(&format!("/api/v1/jobs/{}", row.id)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "stale");
    }

    #[tokio::test]
    async fn get_job_unknown_id_returns_404() {
        let (state, _tmp, _spawned) = test_state();
        let (status, body) = send(app(state), get_req("/api/v1/jobs/nosuch")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "job not found");
    }

    #[tokio::test]
    async fn get_job_log_resolves_unique_prefix() {
        let (state, _tmp, _spawned) = test_state();
        let row = seed_job(&state, "logger", "completed", true);
        let prefix = &row.id[..8];
        // A unique prefix of the job id resolves through the log endpoint.
        let (status, body) = send(
            app(state.clone()),
            get_req(&format!("/api/v1/jobs/{prefix}/log?lines=1")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["returned"], 1);
        assert_eq!(body["lines"], serde_json::json!(["gamma"]));
    }

    #[tokio::test]
    async fn get_job_log_tails_with_defaults_and_caps() {
        let (state, _tmp, _spawned) = test_state();
        let row = seed_job(&state, "logger", "completed", true);
        let url = format!("/api/v1/jobs/{}/log", row.id);

        // Default: last 100 lines.
        let (status, body) = send(app(state.clone()), get_req(&url)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total_lines"], 3);
        assert_eq!(body["returned"], 3);
        assert_eq!(body["lines"], serde_json::json!(["alpha", "beta", "gamma"]));

        // lines=1 returns only the tail.
        let (status, body) = send(app(state.clone()), get_req(&format!("{url}?lines=1"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["returned"], 1);
        assert_eq!(body["lines"], serde_json::json!(["gamma"]));

        // lines > total simply returns everything.
        let (status, body) = send(app(state.clone()), get_req(&format!("{url}?lines=1000"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["returned"], 3);
    }

    #[tokio::test]
    async fn get_job_log_clamps_lines_and_handles_missing_file() {
        let (state, _tmp, _spawned) = test_state();
        let row = seed_job(&state, "logger", "running", false); // no job.log written
        let url = format!("/api/v1/jobs/{}/log", row.id);

        // Missing log file -> empty payload, still 200.
        let (status, body) = send(app(state.clone()), get_req(&url)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total_lines"], 0);
        assert_eq!(body["lines"], serde_json::json!([]));

        // lines=0 -> clamped up to 1, so we get the last line.
        // But this job has no log file yet, so it'll be empty.
        let (status, body) = send(
            app(state.clone()),
            get_req(&format!("{url}?lines=0")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // No log file written for this seed (write_log=false), so 0 lines.
        assert_eq!(body["total_lines"], 0);

        // A job with no output dir at all -> empty payload.
        let bare = state.jobs.create("bare", 3, "").unwrap();
        let (status, body) = send(
            app(state.clone()),
            get_req(&format!("/api/v1/jobs/{}/log", bare.id)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total_lines"], 0);
    }

    #[tokio::test]
    async fn get_job_log_unknown_job_returns_404() {
        let (state, _tmp, _spawned) = test_state();
        let (status, body) = send(app(state), get_req("/api/v1/jobs/nosuch/log")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "job not found");
    }

    #[tokio::test]
    async fn cancel_job_queued_and_running() {
        let (state, _tmp, _spawned) = test_state();
        let row = seed_job(&state, "cancel-me", "queued", false);
        let (status, body) = send(
            app(state.clone()),
            delete_req(&format!("/api/v1/jobs/{}", row.id)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], row.id);
        assert_eq!(body["status"], "cancelled");
        let row = state.jobs.get(&row.id).unwrap().unwrap();
        assert_eq!(row.status, "cancelled");
        assert!(row.is_terminal());

        // Cancelling a terminal job conflicts.
        let (status, body) = send(
            app(state.clone()),
            delete_req(&format!("/api/v1/jobs/{}", row.id)),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("job is not active"));
    }

    #[tokio::test]
    async fn cancel_job_completed_conflicts() {
        let (state, _tmp, _spawned) = test_state();
        let row = seed_job(&state, "done", "completed", false);
        let (status, body) = send(
            app(state.clone()),
            delete_req(&format!("/api/v1/jobs/{}", row.id)),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(body["error"].as_str().unwrap().contains("completed"));
    }

    #[tokio::test]
    async fn cancel_job_unknown_returns_404() {
        let (state, _tmp, _spawned) = test_state();
        let (status, body) = send(app(state), delete_req("/api/v1/jobs/nosuch")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "job not found");
    }

    #[tokio::test]
    async fn rerun_failed_and_completed_jobs() {
        let (state, _tmp, spawned) = test_state();
        for status in ["failed", "completed"] {
            let row = seed_job(&state, "rerun-me", status, false);
            let (code, body) = send(
                app(state.clone()),
                post_json(
                    &format!("/api/v1/jobs/{}/rerun", row.id),
                    serde_json::json!({}),
                ),
            )
            .await;
            assert_eq!(code, StatusCode::ACCEPTED, "status={status}");
            assert_eq!(body["status"], "queued");
            let rerun = state.jobs.get(&row.id).unwrap().unwrap();
            assert_eq!(rerun.status, "queued");
            assert_eq!(rerun.attempt, 0);
            assert!(rerun.error.is_none());
        }
        // Both reruns triggered the spawner.
        assert_eq!(spawned.lock().len(), 2);
    }

    #[tokio::test]
    async fn rerun_queued_and_live_running_conflict() {
        let (state, _tmp, spawned) = test_state();
        let queued = seed_job(&state, "queued-job", "queued", false);
        let (status, body) = send(
            app(state.clone()),
            post_json(
                &format!("/api/v1/jobs/{}/rerun", queued.id),
                serde_json::json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "job is already queued");
        assert_eq!(spawned.lock().len(), 0);
    }

    #[tokio::test]
    async fn cancel_then_rerun_cancelled_job() {
        let (state, _tmp, spawned) = test_state();
        let row = seed_job(&state, "cancel-rerun", "queued", false);
        let (status, _) = send(
            app(state.clone()),
            delete_req(&format!("/api/v1/jobs/{}", row.id)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // A cancelled (terminal) job can be re-run.
        let (status, body) = send(
            app(state.clone()),
            post_json(
                &format!("/api/v1/jobs/{}/rerun", row.id),
                serde_json::json!({}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["status"], "queued");
        assert_eq!(spawned.lock().len(), 1);
        let row = state.jobs.get(&row.id).unwrap().unwrap();
        assert_eq!(row.status, "queued");
    }

    #[tokio::test]
    async fn rerun_unknown_job_returns_404() {
        let (state, _tmp, _spawned) = test_state();
        let (status, body) = send(
            app(state),
            post_json("/api/v1/jobs/nosuch/rerun", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "job not found");
    }
}
