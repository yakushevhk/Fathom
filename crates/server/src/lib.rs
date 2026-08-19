//! HTTP API server for the Parallel Research agent.
//!
//! Exposes a JSON API for creating and monitoring research sessions, plus
//! health and Prometheus metrics endpoints:
//!
//! - `POST   /api/v1/sessions`            — create a research session
//! - `GET    /api/v1/sessions`            — list all sessions
//! - `GET    /api/v1/sessions/:id`        — get session status
//! - `GET    /api/v1/sessions/:id/results`— get session results
//! - `DELETE /api/v1/sessions/:id`        — cancel a running session
//! - `POST   /api/v1/sessions/:id/steer`  — inject a mid-run instruction
//! - `POST   /api/v1/sessions/:id/answer` — answer a pending `question` tool
//! - `POST   /api/v1/sessions/:id/approve`— allow/deny a pending side-effect tool
//! - `GET    /api/v1/agents`              — list all agents
//! - `GET    /api/v1/agents/:id`          — get agent status
//! - `GET    /api/v1/events`              — SSE stream of all agent events
//! - `GET    /api/v1/sessions/:id/events` — SSE stream filtered to a session
//! - `POST   /api/v1/jobs`                — submit a durable background job
//! - `GET    /api/v1/jobs`                — list all jobs
//! - `GET    /api/v1/jobs/:id`            — get job status
//! - `GET    /api/v1/jobs/:id/log`        — tail the job log
//! - `DELETE /api/v1/jobs/:id`            — cancel an active job
//! - `POST   /api/v1/jobs/:id/rerun`      — re-run a finished/stale job
//! - `GET    /health`                     — health check
//! - `GET    /metrics`                    — Prometheus metrics

pub mod auth;
pub mod metrics;
mod jobs_api;
mod memory_api;

use auth::{auth_middleware, rate_limit_middleware, ApiKeyAuth, RateLimiter};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use metrics::Metrics;
use pr_agent::Coordinator;
use pr_core::{AgentEvent, AppConfig, SessionId};
use pr_llm::{DeepSeekProvider, LlmProvider};
use pr_persistence::{JobsDb, Persistence, SessionRow};
use pr_tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Environment variable overriding the per-client rate limit
/// (requests per minute).
pub const RATE_LIMIT_ENV: &str = "PARALLEL_RESEARCH_RATE_LIMIT";
/// Default per-client rate limit: requests per minute.
pub const DEFAULT_RATE_LIMIT: usize = 120;

fn rate_limit_from_env() -> usize {
    std::env::var(RATE_LIMIT_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_RATE_LIMIT)
}

/// A research session currently tracked by the server.
struct RunningSession {
    handle: JoinHandle<()>,
    /// Set exactly once when the session leaves the "active" state, so the
    /// active-sessions gauge is decremented exactly once regardless of which
    /// side (background task completion or DELETE) gets there first.
    finished: Arc<AtomicBool>,
    /// Mid-run steering channel (fleet E1).
    steer_tx: tokio::sync::mpsc::UnboundedSender<String>,
    /// Session-wide cancel token (DELETE cancels the whole agent tree).
    cancel: tokio_util::sync::CancellationToken,
}

/// Spawns the detached runner process for a job: `(job_id, log_path) ->
/// child pid`. Injectable so tests can observe submissions without spawning
/// real processes.
pub(crate) type JobSpawner =
    Arc<dyn Fn(&str, &std::path::Path) -> std::io::Result<u32> + Send + Sync>;

fn default_job_spawner() -> JobSpawner {
    Arc::new(|job_id: &str, log_path: &std::path::Path| {
        let exe = std::env::current_exe()?;
        pr_persistence::spawn_detached_runner(&exe, job_id, Some(log_path))
    })
}

/// Shared application state for all handlers.
pub struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) db: Arc<Persistence>,
    pub(crate) llm: Arc<dyn LlmProvider>,
    pub(crate) tools: Arc<ToolRegistry>,
    pub(crate) metrics: Arc<Metrics>,
    pub(crate) auth: ApiKeyAuth,
    pub(crate) rate_limiter: Mutex<RateLimiter>,
    pub(crate) active_sessions: Mutex<HashMap<String, RunningSession>>,
    pub(crate) event_tx: broadcast::Sender<AgentEvent>,
    pub(crate) jobs: Arc<JobsDb>,
    pub(crate) jobs_root: PathBuf,
    pub(crate) job_spawner: JobSpawner,
    /// Long-term semantic memory (None when `[memory] enabled = false` or
    /// the store failed to open).
    pub(crate) memory: Option<Arc<pr_memory::Memory>>,
    /// Operator control plane: pending questions/approvals keyed by
    /// request_id (answered via POST /sessions/:id/answer|approve).
    pub(crate) pending_controls: Arc<Mutex<HashMap<String, PendingControl>>>,
}

/// A pending operator round-trip waiting for an HTTP answer.
pub(crate) enum PendingControl {
    Question(tokio::sync::oneshot::Sender<String>),
    Approval(tokio::sync::oneshot::Sender<bool>),
}

impl AppState {
    /// Build state from config, opening the SQLite database under the
    /// configured output directory.
    pub fn new(config: AppConfig) -> anyhow::Result<Arc<Self>> {
        let output_dir = PathBuf::from(&config.output.dir);
        std::fs::create_dir_all(&output_dir)?;
        let db = Arc::new(Persistence::open(&output_dir.join(".research.db"))?);
        let jobs = match JobsDb::open(&pr_persistence::default_jobs_db_path()) {
            Ok(jobs) => Arc::new(jobs),
            Err(e) => {
                tracing::warn!("jobs database unavailable, using in-memory registry: {e}");
                Arc::new(JobsDb::in_memory()?)
            }
        };
        Ok(Self::with_db_and_jobs(config, db, jobs))
    }

    /// Build state around an already-open database (used by tests and for
    /// custom storage setups). Jobs get a private in-memory registry.
    pub fn with_db(config: AppConfig, db: Arc<Persistence>) -> Arc<Self> {
        let jobs = Arc::new(JobsDb::in_memory().expect("in-memory jobs db"));
        Self::with_db_and_jobs(config, db, jobs)
    }

    /// Build state around an already-open database and jobs registry.
    pub fn with_db_and_jobs(
        config: AppConfig,
        db: Arc<Persistence>,
        jobs: Arc<JobsDb>,
    ) -> Arc<Self> {
        // Honor `[llm] provider`; fall back to a bare OpenAI-compatible
        // client when no key is configured yet (the server still starts and
        // answers 503 on session creation).
        let llm: Arc<dyn LlmProvider> = pr_llm::build_provider(&config.llm)
            .unwrap_or_else(|_| {
                Arc::new(DeepSeekProvider::new(
                    &config.llm.base_url,
                    &config.llm.api_key,
                    &config.llm.model,
                ))
            });
        // Long-term memory: opened once per server, shared by all sessions
        // and the /memories API. Best-effort.
        let memory = if config.memory.enabled {
            match pr_memory::Memory::open(&config.memory, &config.llm) {
                Ok(m) => Some(Arc::new(m)),
                Err(e) => {
                    tracing::warn!("memory store unavailable: {e}");
                    None
                }
            }
        } else {
            None
        };
        Arc::new(Self {
            config,
            db,
            llm,
            tools: Arc::new(ToolRegistry::with_builtins()),
            metrics: Arc::new(Metrics::new()),
            auth: ApiKeyAuth::from_env(),
            rate_limiter: Mutex::new(RateLimiter::new(
                rate_limit_from_env(),
                Duration::from_secs(60),
            )),
            active_sessions: Mutex::new(HashMap::new()),
            event_tx: broadcast::channel(1024).0,
            jobs,
            jobs_root: pr_persistence::default_jobs_root(),
            job_spawner: default_job_spawner(),
            memory,
            pending_controls: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn is_active(&self, session_id: &str) -> bool {
        self.active_sessions
            .lock()
            .map(|map| map.contains_key(session_id))
            .unwrap_or(false)
    }
}

/// Build the axum router with all routes and middleware.
pub fn build_router(state: Arc<AppState>) -> Router {
    let auth_enabled = state.auth.is_enabled();
    // API routes sit behind authentication and rate limiting. The auth layer
    // is added last, so it runs first (outermost layer) and the rate limiter
    // can key off the authenticated principal.
    let api = Router::new()
        .route("/sessions", post(create_session).get(list_sessions))
        .route("/sessions/:id", get(get_session_status).delete(cancel_session))
        .route("/sessions/:id/steer", post(steer_session))
        .route("/sessions/:id/answer", post(answer_question))
        .route("/sessions/:id/approve", post(approve_tool))
        .route("/sessions/:id/results", get(get_session_results))
        .route("/sessions/:id/events", get(session_events))
        .route("/events", get(global_events))
        .route("/agents", get(list_agents))
        .route("/agents/:id", get(get_agent_status))
        .route(
            "/jobs",
            post(jobs_api::create_job).get(jobs_api::list_jobs),
        )
        .route(
            "/jobs/:id",
            get(jobs_api::get_job).delete(jobs_api::cancel_job),
        )
        .route("/jobs/:id/log", get(jobs_api::get_job_log))
        .route("/jobs/:id/rerun", post(jobs_api::rerun_job))
        .route("/memories", get(memory_api::list_memories))
        .route("/memories/absorb", post(memory_api::absorb_memories))
        .route("/memories/stats", get(memory_api::memory_stats))
        .route("/memories/distill", post(memory_api::distill_memories))
        .route("/memories/gc", post(memory_api::gc_memories))
        .route(
            "/memories/:id",
            get(memory_api::get_memory).delete(memory_api::archive_memory),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    Router::new()
        .nest("/api/v1", api)
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_endpoint))
        // Embedded single-file dashboard (sessions, agents, memory, jobs,
        // live SSE events). It only READs /api/v1, honoring the same API
        // key the operator enters in the UI.
        .route("/dashboard", get(dashboard_page))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            metrics_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        // CORS: permissive only when API keys protect the surface. With auth
        // off (local dev), cross-origin browser requests are blocked — a
        // malicious web page must not be able to drive the agent fleet
        // (fleet round 2 CRITICAL).
        .layer(if auth_enabled {
            CorsLayer::permissive()
        } else {
            CorsLayer::new()
        })
        .with_state(state)
}

/// Run the HTTP server on `0.0.0.0:<port>` until interrupted.
pub async fn run_server(host: String, port: u16) -> anyhow::Result<()> {
    let config = AppConfig::load()?;
    if config.llm.api_key.is_empty() {
        tracing::warn!(
            "No LLM API key configured; session creation will return 503 until one is set"
        );
    }

    // Security posture (fleet round 2): loopback by default; exposing the
    // agent fleet to the network REQUIRES API keys — otherwise any browser
    // tab or LAN host could create/steer/cancel sessions on this machine.
    let ip: std::net::IpAddr = host
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid --host address: {host}"))?;
    if !ip.is_loopback() && !ApiKeyAuth::from_env().is_enabled() {
        anyhow::bail!(
            "refusing to bind non-loopback address {ip} without API keys. \
             Set PARALLEL_RESEARCH_API_KEYS (comma-separated) or use --host 127.0.0.1"
        );
    }

    let state = AppState::new(config)?;
    let app = build_router(state);

    let addr = SocketAddr::from((ip, port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Parallel Research API listening on http://{addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/// Records request duration and total request count.
pub async fn metrics_middleware(
    State(state): State<Arc<AppState>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let response = next.run(request).await;
    state
        .metrics
        .request_duration
        .observe(start.elapsed().as_secs_f64());
    state.metrics.requests_total.inc();
    response
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn json(status: StatusCode, value: serde_json::Value) -> Response {
    (status, Json(value)).into_response()
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    json(
        status,
        serde_json::json!({ "error": message.into() }),
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub query: String,
    pub status: String,
    pub output_dir: Option<String>,
    pub total_tokens: i64,
    pub total_agents: i64,
    pub created_at: String,
    pub updated_at: String,
    pub active: bool,
}

impl SessionResponse {
    fn from_row(row: SessionRow, active: bool) -> Self {
        Self {
            id: row.id,
            query: row.query,
            status: row.status,
            output_dir: row.output_dir,
            total_tokens: row.total_tokens,
            total_agents: row.total_agents,
            created_at: row.created_at,
            updated_at: row.updated_at,
            active,
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SteerRequest {
    /// The instruction to inject into the running session.
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct AnswerRequest {
    /// request_id from the `question_asked` event.
    pub request_id: String,
    /// The operator's answer.
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ApproveRequest {
    /// request_id from the `approval_requested` event.
    pub request_id: String,
    /// true = allow the tool call, false = deny it.
    pub approved: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    /// The research query to run.
    pub query: String,
    /// Optional output directory override (session results are written to
    /// `<output_dir>/<session_id>`).
    #[serde(default)]
    pub output_dir: Option<String>,
}

/// `POST /api/v1/sessions` — start a new research session.
async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSessionRequest>,
) -> Response {
    let query = body.query.trim();
    if query.is_empty() {
        return error(StatusCode::BAD_REQUEST, "query must not be empty");
    }
    if state.config.llm.api_key.is_empty() {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no LLM API key configured on the server",
        );
    }

    let session_id = SessionId::new();
    // The optional output_dir override comes from an untrusted request body:
    // allow only a single relative directory name (no separators, no ".."),
    // so it can never escape the configured base directory.
    let base_dir = match &body.output_dir {
        Some(dir) => {
            let dir = dir.trim();
            let safe = !dir.is_empty()
                && !dir.starts_with('/')
                && !dir.contains('\\')
                && !dir.contains("..")
                && !std::path::Path::new(dir).is_absolute()
                && std::path::Path::new(dir).components().count() == 1;
            if !safe {
                return error(
                    StatusCode::BAD_REQUEST,
                    "output_dir must be a single relative directory name",
                );
            }
            PathBuf::from(dir)
        }
        None => PathBuf::from(&state.config.output.dir),
    };
    let session_dir = base_dir.join(session_id.0.clone());
    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create output dir: {e}"),
        );
    }
    if let Err(e) = state.db.create_session(&session_id, query) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist session: {e}"),
        );
    }
    // Record the output dir immediately so an interrupted session can be
    // located and resumed later.
    let _ = state
        .db
        .set_session_output_dir(&session_id, &session_dir.display().to_string());

    state.metrics.sessions_total.inc();
    state.metrics.sessions_active.inc();

    let finished = Arc::new(AtomicBool::new(false));
    let (steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let cancel = tokio_util::sync::CancellationToken::new();
    let handle = spawn_session(
        state.clone(),
        session_id.clone(),
        query.to_string(),
        session_dir.clone(),
        finished.clone(),
        steer_rx,
        cancel.clone(),
    );

    if let Ok(mut map) = state.active_sessions.lock() {
        map.insert(
            session_id.0.clone(),
            RunningSession {
                handle,
                finished,
                steer_tx,
                cancel,
            },
        );
    }

    json(
        StatusCode::ACCEPTED,
        serde_json::json!({
            "id": session_id.0,
            "status": "running",
            "query": query,
            "output_dir": session_dir.display().to_string(),
        }),
    )
}

/// Spawn the background task that runs a session's coordinator and keeps
/// the metrics updated from its event stream.
fn spawn_session(
    state: Arc<AppState>,
    session_id: SessionId,
    query: String,
    output_dir: PathBuf,
    finished: Arc<AtomicBool>,
    steer_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    cancel: tokio_util::sync::CancellationToken,
) -> JoinHandle<()> {
    let llm = state.llm.clone();
    let tools = state.tools.clone();
    let db = state.db.clone();
    let config = state.config.clone();
    let event_tx = state.event_tx.clone();

    tokio::spawn(async move {
        // Track session-level metrics from the agent event stream.
        let mut rx = event_tx.subscribe();
        let m = state.metrics.clone();
        let sid = session_id.0.clone();
        let metrics_loop = async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        // The bus is shared by all server sessions — only
                        // count this session's events and exit only on this
                        // session's terminal event.
                        if let Some(eid) = event.session_id() {
                            if eid.0 != sid {
                                continue;
                            }
                        } else if let Some(aid) = event.agent_id() {
                            // Agent events: attribute via DB would be costly;
                            // metrics are approximate — count all agents of
                            // live sessions (bounded, single-process server).
                            let _ = aid;
                        }
                        match event {
                            AgentEvent::AgentSpawned { .. } => m.agents_spawned.inc(),
                            AgentEvent::ToolCallCompleted { .. } => m.tool_calls.inc(),
                            AgentEvent::AgentCompleted { tokens_used, .. } => {
                                m.tokens_used.inc_by(tokens_used)
                            }
                            AgentEvent::SessionCompleted { .. }
                            | AgentEvent::SessionFailed { .. } => break,
                            _ => {}
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!("metrics event listener lagged by {n}");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        let run = async {
            // Contact pipeline attachments (best effort): the configured
            // contact store for save_contacts and CRM sync for pushing.
            let contact_db = match pr_persistence::open_contact_store(&config.contacts).await {
                Ok(store) => Some(store),
                Err(e) => {
                    tracing::warn!("contact store unavailable: {e}");
                    None
                }
            };
            let crm = pr_core::CrmSync::from_config(&config.crm).map(Arc::new);
            let memory = state.memory.clone();

            // Operator control plane: register incoming questions/approvals
            // so HTTP clients can answer them via /answer and /approve.
            let (q_tx, mut q_rx) =
                tokio::sync::mpsc::unbounded_channel::<pr_agent::QuestionRequest>();
            let (a_tx, mut a_rx) =
                tokio::sync::mpsc::unbounded_channel::<pr_agent::ApprovalRequest>();
            let pending = state.pending_controls.clone();
            let control_loop = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        q = q_rx.recv() => {
                            let Some(req) = q else { break };
                            if let Ok(mut m) = pending.lock() {
                                m.insert(req.request_id.clone(), PendingControl::Question(req.reply));
                            }
                        }
                        a = a_rx.recv() => {
                            let Some(req) = a else { break };
                            if let Ok(mut m) = pending.lock() {
                                m.insert(req.request_id.clone(), PendingControl::Approval(req.reply));
                            }
                        }
                    }
                }
            });

            let mut coordinator = Coordinator::new(
                session_id.clone(),
                query,
                llm,
                tools,
                event_tx.clone(),
                db.clone(),
                output_dir,
                config,
            )
            .with_steer_rx(steer_rx)
            .with_control_plane(q_tx, a_tx);
            coordinator.set_cancel_token(cancel.clone());
            if let Some(store) = contact_db {
                coordinator = coordinator.with_contact_db(store);
            }
            if let Some(crm) = crm {
                coordinator = coordinator.with_crm(crm);
            }
            if let Some(mem) = memory {
                coordinator = coordinator.with_memory(mem);
            }
            if let Err(e) = coordinator.execute().await {
                tracing::error!("session {session_id} failed: {e}");
                let _ = db.fail_session(&session_id, &e.to_string());
                let _ = event_tx.send(AgentEvent::SessionFailed {
                    id: session_id.clone(),
                    error: e.to_string(),
                });
            }
            // Stop registering controls once the session is over.
            control_loop.abort();
        };

        // Cleanup runs even if the session task panics (Drop guard, fleet
        // round 2): gauge released exactly once, map entry removed.
        struct SessionCleanup {
            state: Arc<AppState>,
            session_id: SessionId,
            finished: Arc<AtomicBool>,
        }
        impl Drop for SessionCleanup {
            fn drop(&mut self) {
                if !self.finished.swap(true, Ordering::SeqCst) {
                    self.state.metrics.sessions_active.dec();
                }
                if let Ok(mut map) = self.state.active_sessions.lock() {
                    map.remove(&self.session_id.0);
                }
            }
        }
        let _cleanup = SessionCleanup {
            state: state.clone(),
            session_id: session_id.clone(),
            finished: finished.clone(),
        };

        tokio::join!(metrics_loop, run);
    })
}

/// `GET /api/v1/sessions` — list all sessions.
async fn list_sessions(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_sessions() {
        Ok(rows) => {
            let sessions: Vec<SessionResponse> = rows
                .into_iter()
                .map(|row| {
                    let active = state.is_active(&row.id);
                    SessionResponse::from_row(row, active)
                })
                .collect();
            json(
                StatusCode::OK,
                serde_json::json!({ "sessions": sessions, "count": sessions.len() }),
            )
        }
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// `GET /api/v1/sessions/:id` — get session status.
async fn get_session_status(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    match state.db.get_session(&SessionId(id.clone())) {
        Ok(Some(row)) => {
            let active = state.is_active(&id);
            json(
                StatusCode::OK,
                serde_json::to_value(SessionResponse::from_row(row, active))
                    .unwrap_or_default(),
            )
        }
        Ok(None) => error(StatusCode::NOT_FOUND, "session not found"),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// `DELETE /api/v1/sessions/:id` — cancel a running session.
async fn cancel_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    // Abort the background task if we are tracking it.
    let entry = state
        .active_sessions
        .lock()
        .ok()
        .and_then(|mut map| map.remove(&id));
    if let Some(session) = entry {
        // Cancel the whole agent tree (fan-out, children, background spawns)
        // BEFORE aborting the outer task.
        session.cancel.cancel();
        session.handle.abort();
        if !session.finished.swap(true, Ordering::SeqCst) {
            state.metrics.sessions_active.dec();
        }
        let _ = state.db.cancel_session(&SessionId(id.clone()));
        return json(
            StatusCode::OK,
            serde_json::json!({ "id": id, "status": "cancelled" }),
        );
    }

    // Not active: report whether it is unknown or simply not running.
    match state.db.get_session(&SessionId(id.clone())) {
        Ok(Some(row)) => error(
            StatusCode::CONFLICT,
            format!("session is not running (status: {})", row.status),
        ),
        Ok(None) => error(StatusCode::NOT_FOUND, "session not found"),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// `POST /api/v1/sessions/:id/steer` — inject a mid-run user instruction
/// into a running session (fleet E1). The text reaches the agents at the
/// next turn boundary.
async fn steer_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SteerRequest>,
) -> Response {
    let message = body.message.trim().to_string();
    if message.is_empty() {
        return error(StatusCode::BAD_REQUEST, "message must not be empty");
    }
    let tx = state
        .active_sessions
        .lock()
        .ok()
        .and_then(|map| map.get(&id).map(|s| s.steer_tx.clone()));
    match tx {
        Some(tx) => match tx.send(message.clone()) {
            Ok(()) => json(
                StatusCode::ACCEPTED,
                serde_json::json!({ "id": id, "steered": true, "message": message }),
            ),
            Err(_) => error(
                StatusCode::CONFLICT,
                "session is shutting down; steering channel closed",
            ),
        },
        None => match state.db.get_session(&SessionId(id.clone())) {
            Ok(Some(row)) => error(
                StatusCode::CONFLICT,
                format!("session is not running (status: {})", row.status),
            ),
            Ok(None) => error(StatusCode::NOT_FOUND, "session not found"),
            Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        },
    }
}

/// `POST /api/v1/sessions/:id/answer` — answer a pending `question` tool.
async fn answer_question(
    State(state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(body): Json<AnswerRequest>,
) -> Response {
    let pending = state
        .pending_controls
        .lock()
        .ok()
        .and_then(|mut m| m.remove(&body.request_id));
    match pending {
        Some(PendingControl::Question(reply)) => match reply.send(body.text.clone()) {
            Ok(()) => json(
                StatusCode::OK,
                serde_json::json!({ "answered": true, "request_id": body.request_id }),
            ),
            Err(_) => error(StatusCode::GONE, "the agent stopped waiting for this answer"),
        },
        Some(PendingControl::Approval(_)) => error(
            StatusCode::BAD_REQUEST,
            "request_id belongs to an approval, use /approve",
        ),
        None => error(StatusCode::NOT_FOUND, "no pending question with this request_id"),
    }
}

/// `POST /api/v1/sessions/:id/approve` — allow/deny a pending side-effect
/// tool call.
async fn approve_tool(
    State(state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(body): Json<ApproveRequest>,
) -> Response {
    let pending = state
        .pending_controls
        .lock()
        .ok()
        .and_then(|mut m| m.remove(&body.request_id));
    match pending {
        Some(PendingControl::Approval(reply)) => match reply.send(body.approved) {
            Ok(()) => json(
                StatusCode::OK,
                serde_json::json!({
                    "approved": body.approved,
                    "request_id": body.request_id
                }),
            ),
            Err(_) => error(StatusCode::GONE, "the agent stopped waiting for this approval"),
        },
        Some(PendingControl::Question(_)) => error(
            StatusCode::BAD_REQUEST,
            "request_id belongs to a question, use /answer",
        ),
        None => error(StatusCode::NOT_FOUND, "no pending approval with this request_id"),
    }
}

/// `GET /api/v1/sessions/:id/results` — get results of a completed session.
async fn get_session_results(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let row = match state.db.get_session(&SessionId(id.clone())) {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "session not found"),
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if row.status != "completed" {
        return error(
            StatusCode::CONFLICT,
            format!("session is not completed (status: {})", row.status),
        );
    }

    let dir = match &row.output_dir {
        Some(dir) => PathBuf::from(dir),
        None => return error(StatusCode::CONFLICT, "session has no output directory"),
    };

    let summary = std::fs::read_to_string(dir.join("summary.md")).unwrap_or_default();

    let mut findings: Vec<serde_json::Value> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("findings")) {
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
            .collect();
        paths.sort();
        for path in paths {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let file = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            findings.push(serde_json::json!({ "file": file, "content": content }));
        }
    }

    json(
        StatusCode::OK,
        serde_json::json!({
            "session_id": id,
            "status": row.status,
            "output_dir": row.output_dir,
            "total_tokens": row.total_tokens,
            "total_agents": row.total_agents,
            "summary": summary,
            "findings": findings,
        }),
    )
}

/// `GET /api/v1/agents` — list all agents across all sessions.
async fn list_agents(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_agents() {
        Ok(agents) => json(
            StatusCode::OK,
            serde_json::json!({ "agents": agents, "count": agents.len() }),
        ),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

/// `GET /api/v1/agents/:id` — get a single agent's status.
async fn get_agent_status(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    match state.db.get_agent(&id) {
        Ok(Some(agent)) => json(StatusCode::OK, serde_json::to_value(agent).unwrap_or_default()),
        Ok(None) => error(StatusCode::NOT_FOUND, "agent not found"),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}


// ---------------------------------------------------------------------------
// Server-Sent Events (live agent event streams)
// ---------------------------------------------------------------------------

type SseStream = Sse<
    std::pin::Pin<Box<dyn Stream<Item = Result<Event, std::convert::Infallible>> + Send>>,
>;

/// `GET /api/v1/events` — SSE stream of every agent event on this server.
///
/// Each event is a JSON object tagged with its `type`
/// (`agent_spawned`, `tool_call_started`, ...). Clients that only care about
/// one session should prefer `GET /api/v1/sessions/:id/events`.
async fn global_events(State(state): State<Arc<AppState>>) -> SseStream {
    let rx = state.event_tx.subscribe();
    let stream = event_stream(rx, None, state.db.clone());
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// `GET /api/v1/sessions/:id/events` — SSE stream filtered to one session.
///
/// The agent set is snapshotted from the database at connect time; agents
/// spawned later are picked up via a DB lookup on first sight.
async fn session_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state.db.get_session(&SessionId(id.clone())) {
        Ok(Some(_)) => {}
        Ok(None) => return error(StatusCode::NOT_FOUND, "session not found"),
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }

    let agents: std::collections::HashSet<String> = state
        .db
        .get_session_agent_rows(&SessionId(id.clone()))
        .map(|rows| rows.into_iter().map(|r| r.id).collect())
        .unwrap_or_default();

    let rx = state.event_tx.subscribe();
    let stream = event_stream(
        rx,
        Some((id, agents, std::collections::HashSet::new())),
        state.db.clone(),
    );
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

/// Build the SSE stream, optionally filtering events to one session.
fn event_stream(
    rx: broadcast::Receiver<AgentEvent>,
    filter: Option<(
        String,
        std::collections::HashSet<String>,
        std::collections::HashSet<String>,
    )>,
    db: Arc<Persistence>,
) -> std::pin::Pin<Box<dyn Stream<Item = Result<Event, std::convert::Infallible>> + Send>> {
    Box::pin(futures::stream::unfold(
        (rx, filter),
        move |(mut rx, mut filter)| {
            let db = db.clone();
            async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if let Some((session_id, agents, negative)) = &mut filter {
                                if !event_belongs_to_session(
                                    &event,
                                    session_id,
                                    agents,
                                    negative,
                                    &db,
                                ) {
                                    continue;
                                }
                            }
                            let data = serde_json::to_string(&event).unwrap_or_default();
                            return Some((Ok(Event::default().data(data)), (rx, filter)));
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!("SSE listener lagged by {n}");
                        }
                        Err(broadcast::error::RecvError::Closed) => return None,
                    }
                }
            }
        },
    ))
}

/// Whether an event belongs to a session: session-scoped events match by id;
/// agent-scoped events match against the known agent set (refreshed from the
/// DB once for agents spawned after the client connected). Agents proven to
/// belong to OTHER sessions go into the negative cache, so a busy
/// multi-session server does not re-query the DB for every foreign event.
fn event_belongs_to_session(
    event: &AgentEvent,
    session_id: &str,
    agents: &mut std::collections::HashSet<String>,
    negative: &mut std::collections::HashSet<String>,
    db: &Persistence,
) -> bool {
    if let Some(sid) = event.session_id() {
        return sid.0 == session_id;
    }
    let Some(aid) = event.agent_id() else {
        return false;
    };
    if agents.contains(&aid.0) {
        return true;
    }
    if negative.contains(&aid.0) {
        return false;
    }
    // First sighting of this agent: one DB lookup decides which cache it joins.
    let known = db
        .get_session_agent_rows(&SessionId(session_id.to_string()))
        .map(|rows| rows.iter().any(|r| r.id == aid.0))
        .unwrap_or(false);
    if known {
        agents.insert(aid.0.clone());
    } else {
        negative.insert(aid.0.clone());
    }
    known
}

/// `GET /dashboard` — embedded single-file live dashboard (read-only UI over
/// the same REST/SSE API; no extra endpoints).
async fn dashboard_page() -> impl IntoResponse {
    axum::response::Html(include_str!("../assets/dashboard.html"))
}

/// `GET /health` — health check.
async fn health_check(State(state): State<Arc<AppState>>) -> Response {
    let db_ok = state.db.list_sessions().is_ok();
    let (status, code) = if db_ok {
        ("ok", StatusCode::OK)
    } else {
        ("degraded", StatusCode::SERVICE_UNAVAILABLE)
    };
    json(
        code,
        serde_json::json!({
            "status": status,
            "service": "parallel-research",
            "version": env!("CARGO_PKG_VERSION"),
            "database": if db_ok { "ok" } else { "error" },
            "active_sessions": state.metrics.sessions_active.get(),
        }),
    )
}

/// `GET /metrics` — Prometheus text exposition format.
async fn metrics_endpoint(State(state): State<Arc<AppState>>) -> Response {
    let body = state.metrics.render_metrics();
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::path::Path;
    use tower::ServiceExt;

    fn test_state() -> Arc<AppState> {
        let db = Arc::new(Persistence::in_memory().unwrap());
        // Memory off in generic tests: they must never touch the real
        // ~/.parallel-research/memory.db. Memory API tests build their own
        // state with a temp-db config (see memory_state).
        let mut config = AppConfig::default();
        config.memory.enabled = false;
        let mut state = AppState::with_db(config, db);
        {
            let s = Arc::get_mut(&mut state).unwrap();
            // Generous limit so unrelated tests never trip the rate limiter.
            s.rate_limiter = Mutex::new(RateLimiter::new(10_000, Duration::from_secs(60)));
        }
        state
    }

    /// State with a real (temp-file) memory store for /memories API tests.
    fn memory_state(tmp: &tempfile::TempDir) -> Arc<AppState> {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let mut config = AppConfig::default();
        config.memory.enabled = true;
        config.memory.db_path = tmp.path().join("memory.db").display().to_string();
        config.memory.embeddings = "tfidf".to_string();
        let mut state = AppState::with_db(config, db);
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.rate_limiter = Mutex::new(RateLimiter::new(10_000, Duration::from_secs(60)));
        }
        state
    }

    fn app(state: Arc<AppState>) -> Router {
        build_router(state)
    }

    fn test_job_state() -> (
        Arc<AppState>,
        tempfile::TempDir,
        Arc<Mutex<Vec<(String, String)>>>,
    ) {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let jobs = Arc::new(JobsDb::in_memory().unwrap());
        let mut config = AppConfig::default();
        config.memory.enabled = false; // don't touch the real memory.db
        let mut state = AppState::with_db_and_jobs(config, db, jobs);
        let tmp = tempfile::tempdir().unwrap();
        let spawned = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.rate_limiter = Mutex::new(RateLimiter::new(10_000, Duration::from_secs(60)));
            s.jobs_root = tmp.path().to_path_buf();
            let spawned_calls = spawned.clone();
            s.job_spawner = Arc::new(move |job_id: &str, log_path: &Path| {
                spawned_calls.lock().unwrap().push((
                    job_id.to_string(),
                    log_path.display().to_string(),
                ));
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)?;
                Ok(4242)
            });
        }
        (state, tmp, spawned)
    }

    async fn send(app: Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, value)
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

    #[tokio::test]
    async fn health_returns_ok() {
        let (status, body) = send(app(test_state()), get_req("/health")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["database"], "ok");
        assert_eq!(body["service"], "parallel-research");
    }

    #[tokio::test]
    async fn dashboard_serves_embedded_html() {
        let resp = app(test_state()).oneshot(get_req("/dashboard")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ctype = resp
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap_or_default().to_string())
            .unwrap_or_default();
        assert!(ctype.contains("text/html"), "{ctype}");
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Parallel Research"));
        assert!(html.contains("/api/v1/sessions") || html.contains("api/v1"));
        assert!(html.contains("EventSource"), "live SSE wiring present");
    }

    #[tokio::test]
    async fn metrics_endpoint_exposes_prometheus_format() {
        let state = test_state();
        state.metrics.sessions_total.inc();
        let resp = app(state).oneshot(get_req("/metrics")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ctype = resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(ctype.starts_with("text/plain"));
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("pr_sessions_total 1"));
        assert!(text.contains("pr_sessions_active 0"));
        assert!(text.contains("pr_request_duration_seconds_bucket{le=\"+Inf\"}"));
    }

    #[tokio::test]
    async fn list_sessions_empty() {
        let (status, body) = send(app(test_state()), get_req("/api/v1/sessions")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 0);
        assert_eq!(body["sessions"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn get_unknown_session_returns_404() {
        let (status, body) = send(
            app(test_state()),
            get_req("/api/v1/sessions/does-not-exist"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].is_string());
    }

    #[tokio::test]
    async fn delete_unknown_session_returns_404() {
        let (status, _) = send(
            app(test_state()),
            delete_req("/api/v1/sessions/does-not-exist"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_session_rejects_empty_query() {
        let mut state = test_state();
        Arc::get_mut(&mut state).unwrap().config.llm.api_key = "test-key".into();
        let (status, body) = send(
            app(state),
            post_json("/api/v1/sessions", serde_json::json!({ "query": "   " })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("query"));
    }

    #[tokio::test]
    async fn create_session_requires_api_key() {
        // Default config has an empty api_key.
        let (status, body) = send(
            app(test_state()),
            post_json("/api/v1/sessions", serde_json::json!({ "query": "test" })),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"].as_str().unwrap().contains("API key"));
    }

    #[tokio::test]
    async fn create_and_cancel_session() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = test_state();
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.config.llm.api_key = "test-key".into();
            // Unreachable endpoint: the background coordinator will sit in
            // its retry loop until the DELETE below aborts it.
            s.config.llm.base_url = "http://127.0.0.1:1".into();
            // The base output dir comes from the server config — request-body
            // overrides are restricted to single relative names (see below).
            s.config.output.dir = tmp.path().to_string_lossy().into_owned();
        }

        let router = app(state.clone());
        let (status, body) = send(
            router,
            post_json(
                "/api/v1/sessions",
                serde_json::json!({
                    "query": "research things",
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["status"], "running");
        let id = body["id"].as_str().unwrap().to_string();
        assert_eq!(state.metrics.sessions_total.get(), 1);
        assert_eq!(state.metrics.sessions_active.get(), 1);
        assert!(state.is_active(&id));

        // Session is visible in the list with the active flag.
        let (status, body) = send(app(state.clone()), get_req("/api/v1/sessions")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 1);
        assert_eq!(body["sessions"][0]["active"], true);

        // Cancel it.
        let (status, body) = send(
            app(state.clone()),
            delete_req(&format!("/api/v1/sessions/{id}")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "cancelled");
        assert_eq!(state.metrics.sessions_active.get(), 0);
        assert!(!state.is_active(&id));

        // DB row is marked cancelled.
        let row = state.db.get_session(&SessionId(id.clone())).unwrap().unwrap();
        assert_eq!(row.status, "cancelled");

        // Cancelling again reports a conflict.
        let (status, _) = send(
            app(state.clone()),
            delete_req(&format!("/api/v1/sessions/{id}")),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn create_list_get_and_log_job() {
        let (state, _tmp, spawned) = test_job_state();
        let (status, body) = send(
            app(state.clone()),
            post_json(
                "/api/v1/jobs",
                serde_json::json!({
                    "task": "research rust sqlite",
                    "attempts": 4,
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["task"], "research rust sqlite");
        assert_eq!(body["status"], "queued");
        assert_eq!(body["max_attempts"], 4);
        let id = body["id"].as_str().unwrap().to_string();
        let output_dir = body["output_dir"].as_str().unwrap().to_string();
        let log_path = body["log"].as_str().unwrap().to_string();
        assert!(log_path.ends_with("job.log"));
        assert_eq!(spawned.lock().unwrap().len(), 1);

        let (status, body) = send(app(state.clone()), get_req("/api/v1/jobs")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 1);
        assert_eq!(body["jobs"][0]["id"], id);

        let short = &id[..8];
        let (status, body) = send(
            app(state.clone()),
            get_req(&format!("/api/v1/jobs/{short}")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["task"], "research rust sqlite");

        std::fs::write(
            std::path::Path::new(&output_dir).join("job.log"),
            "line one\nline two\n",
        )
        .unwrap();
        let (status, body) = send(
            app(state),
            get_req(&format!("/api/v1/jobs/{id}/log?lines=1")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total_lines"], 2);
        assert_eq!(body["returned"], 1);
        assert_eq!(body["lines"][0], "line two");
    }

    #[tokio::test]
    async fn cancel_job_marks_row_cancelled() {
        let (state, _tmp, _spawned) = test_job_state();
        let row = state.jobs.create("cancel me", 3, "").unwrap();
        let job_dir = state.jobs_root.join(&row.id);
        std::fs::create_dir_all(&job_dir).unwrap();
        state
            .jobs
            .set_output_dir(&row.id, &job_dir.display().to_string())
            .unwrap();

        let (status, body) = send(
            app(state.clone()),
            delete_req(&format!("/api/v1/jobs/{}", row.id)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "cancelled");
        let row = state.jobs.get(&row.id).unwrap().unwrap();
        assert_eq!(row.status, "cancelled");
    }

    #[tokio::test]
    async fn rerun_failed_job_resets_and_respawns() {
        let (state, _tmp, spawned) = test_job_state();
        let row = state.jobs.create("retry me", 5, "").unwrap();
        let job_dir = state.jobs_root.join(&row.id);
        std::fs::create_dir_all(&job_dir).unwrap();
        state
            .jobs
            .set_output_dir(&row.id, &job_dir.display().to_string())
            .unwrap();
        state.jobs.mark_failed(&row.id, "boom").unwrap();

        let (status, body) = send(
            app(state.clone()),
            post_json(&format!("/api/v1/jobs/{}/rerun", row.id), serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let rerun = state.jobs.get(&row.id).unwrap().unwrap();
        assert_eq!(rerun.status, "queued");
        assert_eq!(rerun.attempt, 0);
        assert!(rerun.error.is_none());
        assert_eq!(body["status"], "queued");
        assert_eq!(spawned.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_agents_empty_and_unknown_agent_404() {
        let (status, body) = send(app(test_state()), get_req("/api/v1/agents")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 0);

        let (status, _) = send(
            app(test_state()),
            get_req("/api/v1/agents/does-not-exist"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn agent_endpoints_return_persisted_agents() {
        use pr_core::{AgentId, AgentRecord, AgentRole, AgentStatus};

        let state = test_state();
        let session_id = SessionId::new();
        state.db.create_session(&session_id, "q").unwrap();
        let agent_id = AgentId::new();
        state
            .db
            .create_agent(&AgentRecord {
                id: agent_id.clone(),
                session_id: session_id.0.clone(),
                parent_id: None,
                role: AgentRole::Analyst,
                task: "analyze".into(),
                status: AgentStatus::Running,
                depth: 1,
                tokens_used: 42,
                created_at: chrono::Utc::now(),
                completed_at: None,
            })
            .unwrap();

        let (status, body) = send(app(state.clone()), get_req("/api/v1/agents")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["count"], 1);
        assert_eq!(body["agents"][0]["id"], agent_id.0);
        assert_eq!(body["agents"][0]["role"], "analyst");

        let (status, body) = send(
            app(state),
            get_req(&format!("/api/v1/agents/{}", agent_id.0)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["task"], "analyze");
        assert_eq!(body["tokens_used"], 42);
    }

    #[tokio::test]
    async fn results_require_completed_session() {
        let state = test_state();
        let session_id = SessionId::new();
        state.db.create_session(&session_id, "q").unwrap();

        // Unknown session -> 404.
        let (status, _) = send(
            app(state.clone()),
            get_req("/api/v1/sessions/nope/results"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Running session -> 409.
        let (status, body) = send(
            app(state),
            get_req(&format!("/api/v1/sessions/{}/results", session_id.0)),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(body["error"].as_str().unwrap().contains("not completed"));
    }

    #[tokio::test]
    async fn results_return_summary_and_findings() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("summary.md"), "# Final report").unwrap();
        let findings_dir = tmp.path().join("findings");
        std::fs::create_dir_all(&findings_dir).unwrap();
        std::fs::write(findings_dir.join("finding-1.md"), "Finding one").unwrap();

        let state = test_state();
        let session_id = SessionId::new();
        state.db.create_session(&session_id, "q").unwrap();
        state
            .db
            .complete_session(&session_id, &tmp.path().display().to_string(), 123, 2)
            .unwrap();

        let (status, body) = send(
            app(state),
            get_req(&format!("/api/v1/sessions/{}/results", session_id.0)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["summary"], "# Final report");
        assert_eq!(body["total_tokens"], 123);
        let findings = body["findings"].as_array().unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0]["file"], "finding-1.md");
        assert_eq!(findings[0]["content"], "Finding one");
    }

    #[tokio::test]
    async fn auth_blocks_requests_without_valid_key() {
        let mut state = test_state();
        Arc::get_mut(&mut state).unwrap().auth =
            ApiKeyAuth::new().with_key("secret-key", "tester");

        // No key -> 401.
        let (status, _) = send(app(state.clone()), get_req("/api/v1/sessions")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // Wrong key -> 401.
        let req = Request::builder()
            .uri("/api/v1/sessions")
            .header("x-api-key", "wrong")
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(app(state.clone()), req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // Health stays public.
        let (status, _) = send(app(state), get_req("/health")).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_accepts_valid_key_via_both_headers() {
        let mut state = test_state();
        Arc::get_mut(&mut state).unwrap().auth =
            ApiKeyAuth::new().with_key("secret-key", "tester");

        let req = Request::builder()
            .uri("/api/v1/sessions")
            .header("x-api-key", "secret-key")
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(app(state.clone()), req).await;
        assert_eq!(status, StatusCode::OK);

        let req = Request::builder()
            .uri("/api/v1/sessions")
            .header("authorization", "Bearer secret-key")
            .body(Body::empty())
            .unwrap();
        let (status, _) = send(app(state), req).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn rate_limit_returns_429_when_exceeded() {
        let mut state = test_state();
        Arc::get_mut(&mut state).unwrap().rate_limiter =
            Mutex::new(RateLimiter::new(2, Duration::from_secs(60)));

        let router = app(state);
        for _ in 0..2 {
            let (status, _) = send(router.clone(), get_req("/api/v1/sessions")).await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, _) = send(router.clone(), get_req("/api/v1/sessions")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        // Public endpoints are not rate limited.
        let (status, _) = send(router, get_req("/health")).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn request_duration_metric_is_recorded() {
        let state = test_state();
        let router = app(state.clone());
        let (status, _) = send(router, get_req("/health")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(state.metrics.requests_total.get(), 1);
        assert_eq!(state.metrics.request_duration.count(), 1);
    }

    #[tokio::test]
    async fn global_events_streams_agent_events() {
        let state = test_state();
        let router = app(state.clone());

        // Emit an event shortly after the SSE connection subscribes.
        let tx = state.event_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = tx.send(AgentEvent::SessionStarted {
                id: SessionId("s1".to_string()),
                query: "q".to_string(),
            });
        });

        let resp = router.oneshot(get_req("/api/v1/events")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(ct.starts_with("text/event-stream"), "got {ct}");

        // Read frames until our event shows up (keep-alives may come first).
        let mut body = resp.into_body();
        let mut got = String::new();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && !got.contains("session_started") {
            match tokio::time::timeout(Duration::from_millis(500), body.frame()).await {
                Ok(Some(Ok(frame))) => {
                    if let Some(chunk) = frame.data_ref() {
                        got.push_str(&String::from_utf8_lossy(chunk));
                    }
                }
                _ => break,
            }
        }
        assert!(
            got.contains("session_started"),
            "stream must deliver the event, got: {got:?}"
        );
    }


    #[tokio::test]
    async fn steer_requires_running_session() {
        let state = test_state();
        let router = app(state.clone());
        // Unknown session -> 404.
        let (status, _) = send(
            router,
            post_json(
                "/api/v1/sessions/none/steer",
                serde_json::json!({ "message": "go left" }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Empty message -> 400 even for a valid-looking session id.
        let (status, _) = send(
            app(state),
            post_json("/api/v1/sessions/x/steer", serde_json::json!({ "message": " " })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_session_rejects_unsafe_output_dir() {
        let mut state = test_state();
        {
            let s = Arc::get_mut(&mut state).unwrap();
            s.config.llm.api_key = "test-key".into();
            s.config.llm.base_url = "http://127.0.0.1:1".into();
        }
        for evil in [
            "/etc/cron.d",
            "../../escape",
            "a/b",
            "..",
        ] {
            let (status, _) = send(
                app(state.clone()),
                post_json(
                    "/api/v1/sessions",
                    serde_json::json!({ "query": "x", "output_dir": evil }),
                ),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "must reject {evil}");
        }
    }

    #[tokio::test]
    async fn session_events_returns_404_for_unknown_session() {
        let state = test_state();
        let router = app(state);
        let (status, _) = send(router, get_req("/api/v1/sessions/nope/events")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    // ── Memory API ────────────────────────────────────────────────────

    #[tokio::test]
    async fn memory_disabled_returns_503() {
        let state = test_state(); // memory off
        let (status, _) = send(app(state), get_req("/api/v1/memories")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn memory_full_crud_flow() {
        let tmp = tempfile::tempdir().unwrap();
        let state = memory_state(&tmp);
        let router = app(state);

        // Absorb a fact.
        let (status, body) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "facts": [{"content": "Acme LLC headquarters moved to Kazan in 2024", "confidence": 0.9}],
                    "source": "api-test",
                    "scope": "agent"
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["created"], 1);

        // List returns it.
        let (status, body) = send(router.clone(), get_req("/api/v1/memories")).await;
        assert_eq!(status, StatusCode::OK);
        let memories = body["memories"].as_array().unwrap();
        assert_eq!(memories.len(), 1);
        let id = memories[0]["id"].as_str().unwrap().to_string();
        assert!(memories[0]["content"].as_str().unwrap().contains("Kazan"));

        // Search finds it.
        let (status, body) = send(
            router.clone(),
            get_req("/api/v1/memories?q=Acme%20headquarters%20Kazan"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(!body["memories"].as_array().unwrap().is_empty());

        // Get by id (follow=latest).
        let (status, body) = send(
            router.clone(),
            get_req(&format!("/api/v1/memories/{id}?follow=latest")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["memories"][0]["id"], id);

        // Stats show one active agent memory.
        let (status, body) = send(router.clone(), get_req("/api/v1/memories/stats")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["scopes"]["agent"]["active"], 1);

        // Archive (soft delete).
        let (status, body) = send(router.clone(), delete_req(&format!("/api/v1/memories/{id}"))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["archived"], id);

        // After archiving the default active list is empty.
        let (_, body) = send(router.clone(), get_req("/api/v1/memories")).await;
        assert!(body["memories"].as_array().unwrap().is_empty());

        // Unknown id => 404.
        let (status, _) = send(router, get_req("/api/v1/memories/deadbeef")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn memory_distill_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let state = memory_state(&tmp);
        let router = app(state);

        // Seed a run-scoped fact via the API.
        let (status, _) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "facts": [{"content": "session learned that the vendor raised prices in q2"}],
                    "source": "session:api",
                    "scope": "run",
                    "scope_key": "api"
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = send(
            router.clone(),
            post_json("/api/v1/memories/distill?dry_run=false", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["promoted"], 1);
        assert_eq!(body["archived"], 1);
    }

    #[tokio::test]
    async fn memory_gc_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let state = memory_state(&tmp);
        let router = app(state);

        // Seed one active fact so the store is non-empty.
        let (status, _) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "facts": [{"content": "the office moved to a new building in march"}],
                    "source": "session:api",
                    "scope": "agent"
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Dry run reports the plan without touching the store.
        let (status, body) = send(
            router.clone(),
            post_json("/api/v1/memories/gc?dry_run=true", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["dry_run"], true);

        // Real pass: fresh high-importance facts survive GC (stale stage
        // only takes old, never-accessed, low-importance run facts).
        let (status, body) = send(
            router.clone(),
            post_json("/api/v1/memories/gc?ttl_days=0", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["stale_archived"], 0, "{body}");
        let (_, body) = send(router, get_req("/api/v1/memories")).await;
        assert_eq!(body["memories"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn memory_absorb_rejects_empty_facts() {
        let tmp = tempfile::tempdir().unwrap();
        let state = memory_state(&tmp);
        let (status, _) = send(
            app(state),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({"facts": [], "source": "x"}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn memory_absorb_memory_class_ephemeral_goes_to_run_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let state = memory_state(&tmp);
        let router = app(state);

        let (status, body) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "source": "api-test",
                    "facts": [{"content": "annoying bug in this session", "memory_class": "ephemeral"}],
                    "scope": "agent"
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["created"], 1);

        // В agent-scope факта нет.
        let (status, body) = send(router.clone(), get_req("/api/v1/memories?scope=agent")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["memories"].as_array().unwrap().is_empty(),
            "ephemeral-факт не должен быть в agent scope");

        // В run-scope есть.
        let (status, body) = send(router, get_req("/api/v1/memories?scope=run")).await;
        assert_eq!(status, StatusCode::OK);
        let memories = body["memories"].as_array().unwrap();
        assert_eq!(memories.len(), 1);
        assert!(memories[0]["content"].as_str().unwrap().contains("annoying bug"));
    }

    #[tokio::test]
    async fn memory_absorb_memory_class_expiring_gets_ttl() {
        let tmp = tempfile::tempdir().unwrap();
        let state = memory_state(&tmp);
        let router = app(state);

        let (status, body) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "source": "api-test",
                    "facts": [{"content": "quarterly deadline 2026-09-30", "memory_class": "expiring"}]
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");

        let (status, body) = send(router, get_req("/api/v1/memories")).await;
        assert_eq!(status, StatusCode::OK);
        let memories = body["memories"].as_array().unwrap();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].get("expires_at").and_then(|v| v.as_str()).is_some(),
            "expiring-факт должен получить expires_at: {memories:?}");
    }

    #[tokio::test]
    async fn memory_search_cyrillic_via_http() {
        let tmp = tempfile::tempdir().unwrap();
        let state = memory_state(&tmp);
        let router = app(state);

        let (status, _) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "source": "api-test",
                    "facts": [{"content": "Компания Акме разрабатывает CRM-системы для банков"}]
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = send(
            router,
            get_req("/api/v1/memories?q=%D0%9A%D0%BE%D0%BC%D0%BF%D0%B0%D0%BD%D0%B8%D1%8F%20CRM%20%D0%B1%D0%B0%D0%BD%D0%BA%D0%B8"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let memories = body["memories"].as_array().unwrap();
        assert!(!memories.is_empty(), "кириллический запрос должен находить факт");
        assert!(memories[0]["content"].as_str().unwrap().contains("Акме"));
    }

    #[tokio::test]
    async fn memory_gc_reports_confidence_decay() {
        let tmp = tempfile::tempdir().unwrap();
        let state = memory_state(&tmp);
        let router = app(state);

        let (status, _) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "source": "api-test",
                    "facts": [{"content": "fresh fact for gc confidence check"}]
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = send(
            router,
            post_json("/api/v1/memories/gc", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        // Отчёт GC включает новые поля confidence decay.
        assert!(body.get("confidence_archived").is_some(),
            "gc-отчёт должен содержать confidence_archived: {body}");
        assert!(body.get("confidence_decayed").is_some(),
            "gc-отчёт должен содержать confidence_decayed: {body}");
    }

    #[tokio::test]
    async fn memory_supersede_and_follow_latest_via_http() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = memory_state(&tmp);
        {
            // Классификация требует LLM: подставляем мок, отвечающий
            // вердиктом supersede для кандидата c0.
            let s = Arc::get_mut(&mut state).unwrap();
            s.llm = Arc::new(MockSupersedeLlm);
        }
        let router = app(state);

        let (status, body) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "source": "api-test",
                    "facts": [{"content": "Acme Corp CEO is Ivan Petrov as of 2024"}]
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v1 = body["details"][0]["memory_id"].as_str().unwrap_or("").to_string();
        assert!(!v1.is_empty(), "первый absorb должен вернуть memory_id");

        let (status, body) = send(
            router.clone(),
            post_json(
                "/api/v1/memories/absorb",
                serde_json::json!({
                    "source": "api-test",
                    "facts": [{"content": "Acme Corp CEO is Maria Ivanova as of 2025"}]
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let v2 = body["details"][0]["memory_id"].as_str().unwrap_or("").to_string();
        assert!(!v2.is_empty());

        // follow=latest от v1 должен вести к v2.
        let (status, body) = send(
            router,
            get_req(&format!("/api/v1/memories/{v1}?follow=latest")),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let mems = body["memories"].as_array().unwrap();
        assert_eq!(mems.len(), 1);
        assert!(mems[0]["content"].as_str().unwrap().contains("Maria Ivanova"),
            "follow=latest должен вернуть новую версию CEO: {mems:?}");
    }

    /// Мок-LLM для классификации absorb: всегда возвращает вердикт
    /// `supersede` на кандидата c0 (формат `parse_classify_json`).
    struct MockSupersedeLlm;

    #[async_trait::async_trait]
    impl pr_llm::LlmProvider for MockSupersedeLlm {
        fn name(&self) -> &str {
            "mock-supersede"
        }
        fn model(&self) -> &str {
            "mock"
        }
        async fn complete(
            &self,
            _req: &pr_llm::CompletionRequest,
        ) -> pr_core::PrResult<pr_llm::CompletionResponse> {
            Ok(pr_llm::CompletionResponse {
                message: pr_core::Message::assistant(
                    r#"{"candidate":"c0","verdict":"supersede","reason":"newer version in test"}"#,
                ),
                usage: None,
                finish_reason: Some("stop".into()),
            })
        }
        async fn stream(
            &self,
            _req: &pr_llm::CompletionRequest,
        ) -> pr_core::PrResult<Box<dyn futures::Stream<Item = pr_core::PrResult<pr_llm::StreamChunk>> + Send + Unpin>>
        {
            Err(pr_core::PrError::Llm("stream not used".into()))
        }
    }
}
