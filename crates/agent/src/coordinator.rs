use pr_core::*;
use pr_llm::{LlmProvider, CompletionRequest};
use pr_tools::ToolRegistry;
use pr_persistence::Persistence;
use crate::prompt::role_prompt_for;
use crate::runtime::{AgentRuntime, AgentOutput};
use crate::process_manager::{ProcessManager, WorkerResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

// SessionOutput lives in pr-core so the export/notify subsystems can use it
// without depending on this crate. Re-exported here for backwards compatibility.
pub use pr_core::SessionOutput;

pub struct Coordinator {
    session_id: SessionId,
    query: String,
    llm: Arc<dyn LlmProvider>,
    tools: Arc<ToolRegistry>,
    event_tx: broadcast::Sender<AgentEvent>,
    db: Arc<Persistence>,
    output_dir: std::path::PathBuf,
    config: AppConfig,
    total_tokens: u64,
    total_agents: u32,
    use_multiprocess: bool,
    contact_db: Option<Arc<dyn pr_persistence::ContactStore>>,
    crm: Option<Arc<pr_core::CrmSync>>,
    /// Long-term semantic memory shared by all agents of the session.
    memory: Option<Arc<pr_memory::Memory>>,
    /// Detected task type: generic research vs contact harvesting.
    task_type: TaskType,
    /// Requested contact count for lead-gen tasks, when stated.
    target_count: Option<u32>,
    /// Session wall-clock start (for yield accounting / reflection).
    started_at: chrono::DateTime<chrono::Utc>,
    /// Cancels every agent of this session (DELETE / stall kill).
    session_cancel: CancellationToken,
    /// Live agents' cancel tokens, keyed by agent id (stall monitor).
    agent_tokens: Arc<std::sync::Mutex<HashMap<String, CancellationToken>>>,
    /// Mid-run user instructions shared by top-level agents (fleet E1).
    steer_rx: Option<
        Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<String>>>,
    >,
    /// Control plane channels handed to every agent (questions/approvals).
    question_tx: Option<crate::control::QuestionTx>,
    approval_tx: Option<crate::control::ApprovalTx>,
    /// Per-role LLM providers built from `[agent] role_models` (fleet E8).
    role_llms: HashMap<String, Arc<dyn pr_llm::LlmProvider>>,
    /// Session-shared HTTP fetch cache: every agent of this session reuses
    /// downloads made by siblings within the TTL.
    fetch_cache: pr_tools::cache::FetchCache,
    /// Session-shared MX lookup cache (same rationale).
    mx_cache: pr_tools::cache::MxCache,
    /// Extra system-prompt block from an active profile/persona.
    profile_prompt: Option<String>,
    /// Shared task-tree blackboard for this session: durable journal of
    /// coordination records + child→parent beacons. Lazy-created at run start.
    tree_ledger: Option<crate::task_tree::TaskTreeLedger>,
}

/// What kind of job the query describes (fleet C2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    Research,
    LeadGen,
}

/// User-scoped ledger directory: `~/.fathom/ledger`. Shared by the
/// verification-receipt ledger and per-session task-tree blackboards.
fn default_ledger_dir() -> anyhow::Result<std::path::PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
    Ok(home.join(".fathom").join("ledger"))
}

impl Coordinator {
    pub fn new(
        session_id: SessionId,
        query: String,
        llm: Arc<dyn LlmProvider>,
        tools: Arc<ToolRegistry>,
        event_tx: broadcast::Sender<AgentEvent>,
        db: Arc<Persistence>,
        output_dir: std::path::PathBuf,
        config: AppConfig,
    ) -> Self {
        let use_multiprocess = config.agent.use_multiprocess;
        let role_llms = Self::build_role_llms(&config);
        Self {
            session_id, query, llm, tools, event_tx, db, output_dir, config,
            total_tokens: 0,
            total_agents: 0,
            use_multiprocess,
            contact_db: None,
            crm: None,
            memory: None,
            task_type: TaskType::Research,
            target_count: None,
            started_at: chrono::Utc::now(),
            session_cancel: CancellationToken::new(),
            agent_tokens: Arc::new(std::sync::Mutex::new(HashMap::new())),
            steer_rx: None,
            question_tx: None,
            approval_tx: None,
            role_llms,
            fetch_cache: pr_tools::cache::FetchCache::new(),
            mx_cache: pr_tools::cache::MxCache::new(),
            profile_prompt: None,
            tree_ledger: None,
        }
    }

    /// Build role-specific providers from `config.agent.role_models`
    /// (same endpoint/credentials, different model ids). Invalid entries are
    /// logged and skipped — the default provider remains the fallback.
    fn build_role_llms(config: &AppConfig) -> HashMap<String, Arc<dyn pr_llm::LlmProvider>> {
        let mut map = HashMap::new();
        for (role, model) in &config.agent.role_models {
            let mut llm_cfg = config.llm.clone();
            llm_cfg.model = model.clone();
            match pr_llm::build_provider(&llm_cfg) {
                Ok(provider) => {
                    tracing::info!("role '{role}' uses model '{model}'");
                    map.insert(role.to_lowercase(), provider);
                }
                Err(e) => {
                    tracing::warn!("role model '{model}' for '{role}' rejected: {e}");
                }
            }
        }
        map
    }

    /// Pick the LLM for a role: override from role_models or the default.
    fn llm_for_role(&self, role: AgentRole) -> Arc<dyn LlmProvider> {
        let key = match role {
            AgentRole::Coordinator => "coordinator",
            AgentRole::Researcher => "researcher",
            AgentRole::Analyst => "analyst",
            AgentRole::Verifier => "verifier",
            AgentRole::Writer => "writer",
        };
        self.role_llms
            .get(key)
            .cloned()
            .unwrap_or_else(|| self.llm.clone())
    }

    /// Cheap auxiliary provider (`[llm] fast_model`) for the session's
    /// agents; `None` when unset (agents fall back to their main model).
    fn fast_llm(&self) -> Option<Arc<dyn pr_llm::LlmProvider>> {
        pr_llm::build_fast_provider(&self.config.llm).ok().flatten()
    }

    /// Attach an active profile's system-prompt block (injected into every
    /// agent of this session).
    pub fn with_profile_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.profile_prompt = Some(prompt.into());
        self
    }

    /// Session token budget check (0 = unlimited).
    fn budget_exhausted(&self) -> bool {
        let limit = self.config.agent.session_token_limit;
        limit > 0 && self.total_tokens >= limit
    }

    /// Per-agent token cap for a batch of `n_tasks`, derived from the
    /// remaining session budget (`[agent] session_token_limit`). Without a
    /// limit this is `None` (uncapped). The floor keeps a nearly-exhausted
    /// budget from spawning agents that die before their first turn.
    fn per_agent_cap(&self, n_tasks: usize) -> Option<u64> {
        let limit = self.config.agent.session_token_limit;
        if limit == 0 {
            return None;
        }
        let remaining = limit.saturating_sub(self.total_tokens);
        let n = (n_tasks.max(1)) as u64;
        const MIN_CAP: u64 = 4_096;
        Some((remaining / n).max(MIN_CAP))
    }

    /// Attach the session steering channel (fleet E1). Top-level agents
    /// drain it at turn boundaries.
    pub fn with_steer_rx(
        mut self,
        rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    ) -> Self {
        self.steer_rx = Some(Arc::new(tokio::sync::Mutex::new(rx)));
        self
    }

    /// Attach the operator control plane (questions + approvals). Every
    /// agent of the session receives both channels.
    pub fn with_control_plane(
        mut self,
        question_tx: crate::control::QuestionTx,
        approval_tx: crate::control::ApprovalTx,
    ) -> Self {
        self.question_tx = Some(question_tx);
        self.approval_tx = Some(approval_tx);
        self
    }

    /// Cancel all agents of this session (HTTP DELETE, CLI, ...).
    pub fn cancel(&self) {
        self.session_cancel.cancel();
    }

    /// Expose the session cancel token (e.g. to the HTTP layer so DELETE can
    /// cancel agents, not just abort the outer task).
    pub fn cancel_token(&self) -> CancellationToken {
        self.session_cancel.clone()
    }

    /// Replace the session cancel token (the server owns cancellation).
    pub fn set_cancel_token(&mut self, token: CancellationToken) {
        self.session_cancel = token;
    }

    /// Attach the contact database so agents' `save_contacts` tool can
    /// persist harvested contacts (SQLite or PostgreSQL backend).
    pub fn with_contact_db(mut self, db: Arc<dyn pr_persistence::ContactStore>) -> Self {
        self.contact_db = Some(db);
        self
    }

    /// Attach CRM sync so saved contacts are pushed to the configured CRM.
    pub fn with_crm(mut self, crm: Arc<pr_core::CrmSync>) -> Self {
        self.crm = Some(crm);
        self
    }

    /// Attach the long-term semantic memory store; agents receive it for
    /// prompt digests, the memory_* tools and deterministic autosave of
    /// harvested contacts.
    pub fn with_memory(mut self, memory: Arc<pr_memory::Memory>) -> Self {
        self.memory = Some(memory);
        self
    }

    fn emit(&self, event: AgentEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Heartbeat: refresh `sessions.updated_at` every 60s while the session
    /// runs. `SessionResumer` treats running sessions with a stale heartbeat
    /// (>5 min) as interrupted — without this, any run longer than 5 minutes
    /// would be misdetected as crashed and could be resumed a second time.
    fn start_heartbeat(db: Arc<Persistence>, session_id: SessionId) -> HeartbeatGuard {
        let handle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
            tick.tick().await; // first tick is immediate — skip it
            loop {
                tick.tick().await;
                if db.touch_session(&session_id).is_err() {
                    break;
                }
            }
        });
        HeartbeatGuard { handle }
    }

    /// Periodic memory maintenance: hourly GC + distill when `gc_auto` is
    /// enabled.  Runs as a background task alongside the heartbeat.
    fn start_memory_maintenance(memory: Arc<pr_memory::Memory>, config: AppConfig) -> Option<tokio::task::JoinHandle<()>> {
        if !config.memory.gc_auto {
            return None;
        }
        let opts = pr_memory::GcOptions {
            ttl_days: config.memory.gc_ttl_days,
            compact_above: config.memory.gc_compact_above as usize,
            confidence_decay_rate: config.memory.gc_confidence_decay_rate,
            confidence_threshold: config.memory.gc_confidence_threshold,
            ..Default::default()
        };
        let handle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
            tick.tick().await; // skip first immediate tick
            loop {
                tick.tick().await;
                if let Err(e) = memory.gc(&opts).await {
                    tracing::warn!("periodic gc failed: {e}");
                }
                if let Err(e) = memory.distill(None, false).await {
                    tracing::warn!("periodic distill failed: {e}");
                }
            }
        });
        Some(handle)
    }

    /// Stall detection (fleet D2, Hermes pattern): watch the live event bus
    /// and cancel agents that make zero progress for too long. Warns at
    /// `warn_secs`, cancels at `kill_secs` (either 0 disables that stage).
    fn start_stall_monitor(
        event_rx: broadcast::Receiver<AgentEvent>,
        tokens: Arc<std::sync::Mutex<HashMap<String, CancellationToken>>>,
        warn_secs: u64,
        kill_secs: u64,
        session_id: String,
    ) -> Option<StallMonitorGuard> {
        if warn_secs == 0 && kill_secs == 0 {
            return None;
        }
        let handle = tokio::spawn(stall_monitor_loop(
            event_rx,
            tokens,
            warn_secs,
            kill_secs,
            std::time::Duration::from_secs(30),
            session_id,
        ));
        Some(StallMonitorGuard { handle })
    }
}

/// Tracks per-agent progress timestamps and enforces warn/kill thresholds.
async fn stall_monitor_loop(
    mut event_rx: broadcast::Receiver<AgentEvent>,
    tokens: Arc<std::sync::Mutex<HashMap<String, CancellationToken>>>,
    warn_secs: u64,
    kill_secs: u64,
    tick: std::time::Duration,
    session_id: String,
) {
    use std::collections::HashSet;
    let mut last_progress: HashMap<String, std::time::Instant> = HashMap::new();
    let mut warned: HashSet<String> = HashSet::new();
    let mut interval = tokio::time::interval(tick);

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Ok(e) => {
                        if let Some(aid) = e.agent_id() {
                            last_progress
                                .entry(aid.0.clone())
                                .and_modify(|t| *t = std::time::Instant::now())
                                .or_insert_with(std::time::Instant::now);
                        }
                        // Terminal events of THIS session end the monitor.
                        // In server mode the bus is shared between sessions —
                        // breaking on a foreign session would silently drop
                        // stall protection.
                        if matches!(
                            e,
                            AgentEvent::SessionCompleted { .. }
                                | AgentEvent::SessionFailed { .. }
                        ) && e.session_id().map(|s| s.0.as_str()) == Some(session_id.as_str())
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = interval.tick() => {
                let now = std::time::Instant::now();
                for (agent_id, last) in &last_progress {
                    let idle_secs = now.duration_since(*last).as_secs();
                    let token = tokens
                        .lock()
                        .ok()
                        .and_then(|map| map.get(agent_id).cloned());
                    let Some(token) = token else { continue };
                    if token.is_cancelled() {
                        continue;
                    }
                    if kill_secs > 0 && idle_secs >= kill_secs {
                        tracing::error!(
                            "agent {agent_id} stalled for {idle_secs}s — cancelling"
                        );
                        token.cancel();
                    } else if warn_secs > 0 && idle_secs >= warn_secs && !warned.contains(agent_id) {
                        tracing::warn!(
                            "agent {agent_id} made no progress for {idle_secs}s"
                        );
                        warned.insert(agent_id.clone());
                    }
                }
            }
        }
    }
}

/// Aborts the stall monitor when dropped.
struct StallMonitorGuard {
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for StallMonitorGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Aborts the heartbeat task when dropped (including on early `?` returns).
struct HeartbeatGuard {
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl Coordinator {
    /// Build an [`AgentRuntime`] for a researcher sub-task, with all
    /// attachments (contact DB, CRM) inherited.
    fn build_researcher(
        &self,
        agent_id: AgentId,
        parent_id: Option<AgentId>,
        task: String,
        depth: u32,
    ) -> AgentRuntime {
        let mut agent = AgentRuntime::new(
            agent_id,
            self.session_id.clone(),
            parent_id,
            AgentRole::Researcher,
            task,
            depth,
            self.llm_for_role(AgentRole::Researcher),
            self.tools.clone(),
            self.event_tx.clone(),
            self.db.clone(),
            self.output_dir.clone(),
            self.config.clone(),
        );
        agent.contact_db = self.contact_db.clone();
        agent.crm = self.crm.clone();
        agent.memory = self.memory.clone();
        agent.fast_llm = self.fast_llm();
        agent.fetch_cache = Some(self.fetch_cache.clone());
        agent.mx_cache = Some(self.mx_cache.clone());
        agent.profile_prompt = self.profile_prompt.clone();
        agent.question_tx = self.question_tx.clone();
        agent.approval_tx = self.approval_tx.clone();
        agent = agent.with_role_llms(self.role_llms.clone());
        agent
    }

    /// Run one agent with the configured wall-clock timeout
    /// (`config.agent.timeout_seconds`; 0 disables).
    async fn run_with_timeout(&self, mut agent: AgentRuntime) -> anyhow::Result<AgentOutput> {
        let timeout_secs = self.config.agent.timeout_seconds;
        if timeout_secs == 0 {
            return agent.run().await;
        }
        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            agent.run(),
        )
        .await
        {
            Ok(res) => res,
            Err(_) => Err(anyhow::anyhow!(
                "agent {} timed out after {timeout_secs}s",
                agent.id
            )),
        }
    }

    /// Fan out sub-tasks to in-process researcher agents and collect their
    /// outputs. Respects `max_agents`; failed agents are skipped.
    async fn spawn_researchers(&mut self, sub_tasks: &[String]) -> Vec<AgentOutput> {
        let mut findings: Vec<AgentOutput> = Vec::new();
        let mut join_set: JoinSet<anyhow::Result<AgentOutput>> = JoinSet::new();

        // Publish the delegation contract to the shared tree blackboard: every
        // row below belongs to one root, so a blocked child's beacon is visible
        // to the parent (and to any later-reflected sub-agent).
        if let Some(tree) = &self.tree_ledger {
            let _ = tree
                .append(
                    "contract",
                    &format!("Distribute {} sub-task(s) to researchers", sub_tasks.len()),
                    &self.session_id.0,
                    "coordinator",
                    false,
                    None,
                )
                .await;
        }

        // Split the remaining session token budget across this batch so a
        // single parallel round cannot blow far past session_token_limit.
        let per_agent_cap = self.per_agent_cap(sub_tasks.len());

        for task_desc in sub_tasks {
            if self.total_agents >= self.config.agent.max_agents {
                tracing::warn!("Max agents reached, skipping remaining tasks");
                let _ = self.db.update_subtask_status(
                    &self.session_id, task_desc, "skipped",
                    Some("max agents reached"),
                );
                break;
            }
            if self.budget_exhausted() {
                tracing::warn!(
                    "session token budget reached ({}), skipping remaining tasks",
                    self.config.agent.session_token_limit
                );
                let _ = self.db.update_subtask_status(
                    &self.session_id, task_desc, "skipped",
                    Some("token budget exhausted"),
                );
                break;
            }

            let agent_id = AgentId::new();
            self.total_agents += 1;

            let agent_record = AgentRecord {
                id: agent_id.clone(),
                session_id: self.session_id.0.clone(),
                parent_id: None,
                role: AgentRole::Researcher,
                task: task_desc.clone(),
                status: AgentStatus::Spawned,
                depth: 1,
                tokens_used: 0,
                created_at: chrono::Utc::now(),
                completed_at: None,
            };
            if let Err(e) = self.db.create_agent(&agent_record) {
                tracing::error!("Failed to persist agent record: {e}");
            }

            self.emit(AgentEvent::AgentSpawned {
                id: agent_id.clone(),
                parent: None,
                role: "researcher".to_string(),
                task: task_desc.clone(),
                depth: 1,
            });

            let mut agent = self.build_researcher(agent_id.clone(), None, task_desc.clone(), 1);
            agent.token_cap = per_agent_cap;
            if let Some(rx) = &self.steer_rx {
                agent = agent.with_steer_rx(rx.clone());
            }
            // Register for stall detection + session-wide cancel (regression
            // fix: fan-out agents were missing both).
            let token = self.session_cancel.child_token();
            if let Ok(mut map) = self.agent_tokens.lock() {
                map.insert(agent_id.0.clone(), token.clone());
            }
            let mut agent = agent.with_cancel_token(token.clone());
            let timeout_secs = self.config.agent.timeout_seconds;
            let db = self.db.clone();
            let aid = agent_id.clone();
            join_set.spawn(async move {
                let run = async move {
                    if timeout_secs == 0 {
                        agent.run().await
                    } else {
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(timeout_secs),
                            agent.run(),
                        )
                        .await
                        {
                            Ok(res) => res,
                            Err(_) => Err(anyhow::anyhow!(
                                "agent timed out after {timeout_secs}s"
                            )),
                        }
                    }
                };
                tokio::select! {
                    res = run => res,
                    _ = token.cancelled() => {
                        let _ = db.update_agent_status(
                            &aid,
                            AgentStatus::Cancelled,
                            0,
                            Some("cancelled"),
                        );
                        Err(anyhow::anyhow!("agent cancelled"))
                    }
                }
            });
        }

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(output)) => {
                    self.total_tokens += output.tokens_used + output.descendant_tokens;
                    if output.aborted {
                        // Doom-loop stop: the summary is a warning, not a finding.
                        let _ = self.db.update_agent_status(
                            &output.agent_id,
                            AgentStatus::Failed,
                            output.tokens_used,
                            Some(&output.summary),
                        );
                        self.emit(AgentEvent::AgentFailed {
                            id: output.agent_id.clone(),
                            error: output.summary.clone(),
                        });
                    } else {
                        let _ = self.db.update_agent_status(
                            &output.agent_id,
                            AgentStatus::Completed,
                            output.tokens_used,
                            Some(&output.summary),
                        );
                        self.emit(AgentEvent::AgentCompleted {
                            id: output.agent_id.clone(),
                            summary: output.summary.chars().take(200).collect(),
                            tokens_used: output.tokens_used,
                        });
                        // "Letters home": mirror a high-signal line into the
                        // shared tree ledger so the parent's synthesis sees the
                        // finding even after per-agent contexts are GC'd.
                        if let Some(tree) = &self.tree_ledger {
                            let snippet: String = output
                                .summary
                                .lines()
                                .find(|l| !l.trim().is_empty())
                                .map(|l| l.chars().take(300).collect())
                                .unwrap_or_else(|| "no summary".to_string());
                            let _ = tree
                                .append(
                                    "partial_finding",
                                    &snippet,
                                    &output.agent_id.0,
                                    "researcher",
                                    false,
                                    None,
                                )
                                .await;
                        }
                        findings.push(output);
                    }
                }
                Ok(Err(e)) => {
                    tracing::error!("Agent failed: {e}");
                }
                Err(e) => {
                    tracing::error!("Agent task panicked: {e}");
                }
            }
        }
        if let Ok(mut map) = self.agent_tokens.lock() {
            map.clear();
        }

        findings
    }

    /// Post-run reflection (best-effort): when a run produced substantial
    /// findings, record an observation in the durable reflection log, fold a
    /// signal into the pattern register, and offer a capability idea to the
    /// improvement backlog. Any failure here is logged, never fatal to the run.
    async fn run_post_run_reflection(&self, findings: &[AgentOutput], synthesis: &str) {
        if findings.is_empty() {
            return;
        }
        let dir = match default_ledger_dir() {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("post-run reflection skipped: {e}");
                return;
            }
        };

        let pattern_reg = crate::reflection::PatternRegister::new(&dir);
        let _ = pattern_reg.load().await;
        let reflection_log = crate::reflection::ReflectionLog::new(&dir);
        let backlog = crate::improvement::ImprovementBacklog::new(
            dir.join(crate::improvement::BACKLOG_REL_PATH),
        );
        let _ = backlog.load().await;

        // Observations: describe the run surface so a future session knows what
        // was attempted.
        let mut observations = Vec::new();
        for f in findings.iter().take(8) {
            let first = f.summary.lines().find(|l| !l.trim().is_empty()).unwrap_or_default();
            observations.push(format!("[{}] {}", f.agent_id.0, first.chars().take(200).collect::<String>()));
        }

        // Pattern: if this was a lead-gen run and we still have findings, note
        // the harvest worked; the register is what turns recurring errors into
        // structural fixes later.
        let _ = pattern_reg
            .upsert(
                "run_yielded_findings",
                "researcher returned output",
                "capture findings into blackboard + report",
            )
            .await;

        let rec = crate::reflection::ReflectionRecord {
            ts: chrono::Utc::now().to_rfc3339(),
            session_id: self.session_id.0.clone(),
            task_summary: self.query.chars().take(200).collect(),
            observations,
            pattern_upserts: vec![],
            backlog_candidates: vec![],
        };
        let _ = reflection_log.append(&rec).await;

        // Capability idea for the improvement backlog: offer to reuse the
        // harvest structure next time (self-hardening across sessions).
        let _ = backlog
            .add(
                "Reuse research structure for lead-gen across sessions",
                "capability",
                &self.query.chars().take(80).collect::<String>(),
                "capability_idea",
                crate::improvement::Priority::Medium,
                false,
                Some(synthesis.chars().take(300).collect()),
                None,
                None,
            )
            .await;
    }

    pub async fn execute(&mut self) -> anyhow::Result<SessionOutput> {
        let _heartbeat = Self::start_heartbeat(self.db.clone(), self.session_id.clone());
        let _memory_maintenance = self.memory.as_ref().and_then(|m| {
            Self::start_memory_maintenance(m.clone(), self.config.clone())
        });
        // Open the shared task-tree blackboard for this session (best-effort:
        // absent a usable data dir we run without durable coordination).
        if let Ok(dir) = default_ledger_dir() {
            let path = crate::task_tree::TaskTreeLedger::for_session(&dir, &self.session_id.0);
            let ledger = crate::task_tree::TaskTreeLedger::new(&path);
            let _ = ledger.load().await;
            self.tree_ledger = Some(ledger);
        }
        let _stall = Self::start_stall_monitor(
            self.event_tx.subscribe(),
            self.agent_tokens.clone(),
            self.config.agent.stall_warn_seconds,
            self.config.agent.stall_kill_seconds,
            self.session_id.0.clone(),
        );
        self.emit(AgentEvent::SessionStarted {
            id: self.session_id.clone(),
            query: self.query.clone(),
        });

        // Step 1: Plan — decompose the query into sub-tasks
        let sub_tasks = self.plan().await?;
        tracing::info!("Planned {} sub-tasks", sub_tasks.len());

        // Goal Mode light (fleet E4): persist the plan as subtask rows so
        // progress is observable and resumable.
        for task in &sub_tasks {
            let _ = self.db.add_subtask(&self.session_id, task);
        }

        // Step 2: Fan-out — spawn researcher agents
        let mut findings: Vec<AgentOutput> = Vec::new();

        if sub_tasks.is_empty() {
            // No sub-tasks, run as single agent
            let output = self.run_single_agent().await?;
            findings.push(output);
        } else if self.use_multiprocess {
            // Spawn each researcher as a separate OS process
            findings.extend(self.run_multiprocess_fanout(&sub_tasks).await?);
        } else {
            // Spawn sub-agents in parallel (in-process)
            findings.extend(self.spawn_researchers(&sub_tasks).await);
        }

        // Goal Mode light: sync subtask statuses with agent outcomes.
        self.sync_subtask_statuses();

        // Step 2.5: persist structured findings (fleet C4).
        for output in &findings {
            for finding in &output.findings {
                if let Err(e) = self.db.add_finding(finding) {
                    tracing::warn!("failed to persist finding: {e}");
                }
            }
        }

        // Step 2.6: reflection round for lead-gen (fleet C3).
        // If the query stated a target and the harvest came up short while
        // agent budget remains, run ONE gap-filling round aimed at the
        // shortfall.
        let mut findings = findings;
        if self.task_type == TaskType::LeadGen {
            if let Some(target) = self.target_count {
                let saved = self.contacts_saved_so_far().await.unwrap_or(0);
                if saved < target
                    && self.total_agents < self.config.agent.max_agents
                {
                    let gap = target - saved;
                    tracing::info!(
                        "reflection: {saved}/{target} contacts collected, running gap-filling round ({gap} missing)"
                    );
                    let gap_task = format!(
                        "GAP-FILLING ROUND: the team collected {saved} of {target} requested contacts so far. \
                         Find at least {gap} MORE contacts matching the original query: {}. \
                         Use DIFFERENT sources/companies than the obvious ones already covered. \
                         Extract and verify emails/phones; extraction results are auto-persisted.",
                        self.query
                    );
                    let extra = self.spawn_researchers(&[gap_task]).await;
                    for output in &extra {
                        for finding in &output.findings {
                            if let Err(e) = self.db.add_finding(finding) {
                                tracing::warn!("failed to persist finding: {e}");
                            }
                        }
                    }
                    findings.extend(extra);
                }
            }
        }

        // Step 2.7: Full Goal Mode — an LLM judge reviews coverage of the
        // original goal and runs up to `replan_rounds` gap-filling rounds
        // (each capped by max_agents / session token budget).
        let was_planned = !sub_tasks.is_empty();
        if was_planned && self.config.agent.replan_rounds > 0 {
            for round in 0..self.config.agent.replan_rounds {
                let Some(extra_tasks) = self.evaluate_and_replan(&findings).await else {
                    tracing::info!("goal mode: goal satisfied before round {}", round + 1);
                    break;
                };
                tracing::info!(
                    "goal mode: replan round {} — {} gap-filling task(s)",
                    round + 1,
                    extra_tasks.len()
                );
                for task in &extra_tasks {
                    let _ = self.db.add_subtask(&self.session_id, task);
                }
                let extra = self.spawn_researchers(&extra_tasks).await;
                self.sync_subtask_statuses();
                for output in &extra {
                    for finding in &output.findings {
                        if let Err(e) = self.db.add_finding(finding) {
                            tracing::warn!("failed to persist finding: {e}");
                        }
                    }
                }
                findings.extend(extra);
            }
        }

        // Step 3: Synthesize — combine all findings into a final report
        let synthesis = self.synthesize(&findings).await?;

        // Step 4: Write output files
        self.write_output(&synthesis, &findings)?;

        // Mark session complete
        self.db.complete_session(
            &self.session_id,
            &self.output_dir.display().to_string(),
            self.total_tokens,
            self.total_agents,
        )?;

        self.emit(AgentEvent::SessionCompleted {
            id: self.session_id.clone(),
            output_dir: self.output_dir.display().to_string(),
            total_tokens: self.total_tokens,
            total_agents: self.total_agents,
        });

        // Post-run reflection: fold non-trivial runs into the durable pattern
        // register + improvement backlog so the agent self-hardens across
        // sessions (best-effort).
        self.run_post_run_reflection(&findings, &synthesis).await;

        Ok(SessionOutput {
            session_id: self.session_id.clone(),
            output_dir: self.output_dir.clone(),
            synthesis,
            total_tokens: self.total_tokens,
            total_agents: self.total_agents,
        })
    }

    /// Resume an interrupted session from reconstructed state.
    ///
    /// Outputs of agents that finished before the interruption are kept;
    /// pending sub-tasks are re-executed (in-process only — workers from the
    /// previous run are gone), then everything is synthesized as usual and
    /// the session is marked complete.
    pub async fn execute_resume(
        &mut self,
        state: crate::resume::ResumeState,
    ) -> anyhow::Result<SessionOutput> {
        let _heartbeat = Self::start_heartbeat(self.db.clone(), self.session_id.clone());
        let _stall = Self::start_stall_monitor(
            self.event_tx.subscribe(),
            self.agent_tokens.clone(),
            self.config.agent.stall_warn_seconds,
            self.config.agent.stall_kill_seconds,
            self.session_id.0.clone(),
        );
        self.emit(AgentEvent::SessionStarted {
            id: self.session_id.clone(),
            query: format!("[resume] {}", self.query),
        });

        tracing::info!(
            completed = state.completed_agents.len(),
            pending = state.pending_tasks.len(),
            "resuming session"
        );

        let mut findings = state.completed_agents;
        // Recovered work counts toward the session totals — otherwise
        // resume overwrites stored counters with only the re-run share.
        for f in &findings {
            self.total_tokens += f.tokens_used + f.descendant_tokens;
        }
        self.total_agents += findings.len() as u32;
        if !state.pending_tasks.is_empty() {
            findings.extend(self.spawn_researchers(&state.pending_tasks).await);
        }

        let synthesis = self.synthesize(&findings).await?;
        self.write_output(&synthesis, &findings)?;

        self.db.complete_session(
            &self.session_id,
            &self.output_dir.display().to_string(),
            self.total_tokens,
            self.total_agents,
        )?;

        self.emit(AgentEvent::SessionCompleted {
            id: self.session_id.clone(),
            output_dir: self.output_dir.display().to_string(),
            total_tokens: self.total_tokens,
            total_agents: self.total_agents,
        });

        // Post-run reflection: fold non-trivial runs into the durable pattern
        // register + improvement backlog so the agent self-hardens across
        // sessions (best-effort).
        self.run_post_run_reflection(&findings, &synthesis).await;

        Ok(SessionOutput {
            session_id: self.session_id.clone(),
            output_dir: self.output_dir.clone(),
            synthesis,
            total_tokens: self.total_tokens,
            total_agents: self.total_agents,
        })
    }

    /// Keyword pre-classification of the query (zero-cost, deterministic).
    fn detect_task_type(query: &str) -> TaskType {
        let q = query.to_lowercase();
        const LEADGEN_MARKERS: &[&str] = &[
            "email", "e-mail", "emails", "телефон", "телефоны", "phone", "контакт",
            "контакты", "contact", "contacts", "лид", "лиды", "lead", "leads",
            "ceo", "cto", "cfo", "директор", "руководител", "linkedin",
            "соцсет", "сотрудник", "employees", "decision maker",
        ];
        if LEADGEN_MARKERS.iter().any(|m| q.contains(m)) {
            TaskType::LeadGen
        } else {
            TaskType::Research
        }
    }

    /// Extract an explicit target count ("найди 20 email", "find 15 leads").
    fn detect_target_count(query: &str) -> Option<u32> {
        let q = query.to_lowercase();
        let markers = ["email", "e-mail", "контакт", "contact", "лид", "lead", "телефон", "phone"];
        let bytes = q.as_bytes();
        let mut num = String::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_digit() {
                num.clear();
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    num.push(bytes[i] as char);
                    i += 1;
                }
                // Look a short window ahead for a contact-like noun
                // (tight enough that unrelated earlier numbers don't match).
                let mut end = (i + 16).min(q.len());
                while end > i && !q.is_char_boundary(end) {
                    end -= 1;
                }
                let window = &q[i..end];
                if markers.iter().any(|m| window.contains(m)) {
                    if let Ok(n) = num.parse::<u32>() {
                        if n > 0 && n <= 10_000 {
                            return Some(n);
                        }
                    }
                }
            } else {
                i += 1;
            }
        }
        None
    }

    async fn plan(&mut self) -> anyhow::Result<Vec<String>> {
        self.task_type = Self::detect_task_type(&self.query);
        self.target_count = Self::detect_target_count(&self.query);

        let plan_prompt = if self.task_type == TaskType::LeadGen {
            format!(
                r#"You are planning an OSINT / lead-generation task: harvesting contacts (emails, phones, persons, companies).

Query: {}

Decompose it into 2-5 NON-OVERLAPPING collection sub-tasks that can run in parallel. Partition by a dimension that avoids duplicate work: company industry, company name range, city district, source type (directories vs social vs corporate sites), or role.

Each sub-task MUST be self-contained and include: the exact target description, the preferred tools (search_business_directory, find_leads, parse_corporate_site, extract_contacts, search_social), and — if the query states an overall target — a per-task quota (roughly total / number of tasks).

Respond with ONLY a JSON array of strings. Example:
["Find CEO/CTO contacts of Moscow IT companies A-M via search_business_directory + parse_corporate_site; quota: 5 verified emails", "Find CEO/CTO contacts of Moscow IT companies N-Z via search_business_directory + parse_corporate_site; quota: 5 verified emails"]

Do NOT include any explanation, just the JSON array."#,
                self.query
            )
        } else {
            format!(
                r#"You are planning a research task. Decompose the following query into 2-5 independent sub-tasks that can be researched in parallel.

Query: {}

Respond with ONLY a JSON array of strings, where each string is a self-contained research task description. Each task should be specific enough for a researcher agent to complete independently.

Example format:
["Research the history of X", "Find current applications of Y", "Analyze the limitations of Z"]

Do NOT include any explanation, just the JSON array."#,
                self.query
            )
        };

        let req = CompletionRequest {
            messages: vec![
                Message::system(format!(
                    "You are a research planner.\n\n{}\n\nOutput only valid JSON.",
                    role_prompt_for(AgentRole::Coordinator)
                )),
                Message::user(plan_prompt),
            ],
            tools: vec![],
            temperature: Some(0.3),
            max_tokens: Some(2048),
            stream: false,
        };

        // Planning is a coordinator-level call: honor `[agent.role_models]
        // coordinator` when configured.
        let response = self.llm_for_role(AgentRole::Coordinator).complete(&req).await?;

        if let Message::Assistant { content, .. } = &response.message {
            if let Some(text) = content {
                // Try to parse JSON array from the response
                if let Ok(tasks) = serde_json::from_str::<Vec<String>>(text) {
                    return Ok(tasks);
                }
                // Try to extract JSON array from markdown code block
                if let Some(start) = text.find('[') {
                    if let Some(end) = text.rfind(']') {
                        let json_str = &text[start..=end];
                        if let Ok(tasks) = serde_json::from_str::<Vec<String>>(json_str) {
                            return Ok(tasks);
                        }
                    }
                }
            }
        }

        // Fallback: single task
        Ok(vec![self.query.clone()])
    }

    /// Full Goal Mode judge: review the collected results against the
    /// original goal and, if concrete gaps remain, propose up to 3
    /// gap-filling sub-tasks for an extra replan round. Returns `None`
    /// when the goal is satisfied, the judge is unsure, or no agent budget
    /// remains — the caller treats all of these as "stop replanning".
    async fn evaluate_and_replan(&self, findings: &[AgentOutput]) -> Option<Vec<String>> {
        // Budget gates: never replan into an exhausted session.
        if self.budget_exhausted() {
            tracing::info!("replan skipped: session token budget exhausted");
            return None;
        }
        if self.total_agents >= self.config.agent.max_agents {
            tracing::info!("replan skipped: max_agents reached");
            return None;
        }

        // Compact per-agent result digest for the judge.
        let mut digest = String::new();
        for (i, out) in findings.iter().enumerate() {
            let summary: String = out.summary.chars().take(800).collect();
            digest.push_str(&format!("--- Result {} ---\n{}\n", i + 1, summary));
        }
        if digest.is_empty() {
            digest = "(no results collected)".to_string();
        }

        let prompt = format!(
            r#"You are the goal-checker for a research session.

ORIGINAL GOAL:
{}

RESULTS COLLECTED SO FAR:
{}

Decide whether the collected results FULLY satisfy the original goal. Be strict but fair: only flag a gap if something concrete and important is missing — not nice-to-haves.

Respond with ONLY JSON:
{{"complete": true, "new_subtasks": []}}
or, if concrete gaps remain:
{{"complete": false, "new_subtasks": ["<self-contained gap-filling task>", ...]}}

Rules: at most 3 new_subtasks; each must be independently executable by a researcher and target a specific gap; if the goal is met return complete=true with an empty array. No explanation outside the JSON."#,
            self.query, digest
        );

        let req = CompletionRequest {
            messages: vec![
                Message::system("You are a rigorous research goal-checker. Output only valid JSON.".to_string()),
                Message::user(prompt),
            ],
            tools: vec![],
            temperature: Some(0.2),
            max_tokens: Some(1024),
            stream: false,
        };

        let response = match self.llm_for_role(AgentRole::Coordinator).complete(&req).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("replan judge call failed: {e}");
                return None;
            }
        };
        let Message::Assistant { content: Some(text), .. } = &response.message else {
            return None;
        };

        #[derive(serde::Deserialize)]
        struct Verdict {
            #[serde(default)]
            complete: bool,
            #[serde(default)]
            new_subtasks: Vec<String>,
        }

        let parse = |s: &str| serde_json::from_str::<Verdict>(s).ok();
        let verdict = parse(text)
            .or_else(|| {
                let start = text.find('{')?;
                let end = text.rfind('}')?;
                if end <= start {
                    return None;
                }
                parse(&text[start..=end])
            });

        match verdict {
            Some(v) if !v.complete && !v.new_subtasks.is_empty() => {
                let tasks: Vec<String> = v
                    .new_subtasks
                    .into_iter()
                    .filter(|t| !t.trim().is_empty())
                    .take(3)
                    .collect();
                if tasks.is_empty() {
                    None
                } else {
                    Some(tasks)
                }
            }
            _ => None,
        }
    }

    async fn run_single_agent(&mut self) -> anyhow::Result<AgentOutput> {
        let agent_id = AgentId::new();
        self.total_agents += 1;

        let agent_record = AgentRecord {
            id: agent_id.clone(),
            session_id: self.session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: self.query.clone(),
            status: AgentStatus::Spawned,
            depth: 0,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        self.db.create_agent(&agent_record)?;

        let mut agent = self.build_researcher(agent_id.clone(), None, self.query.clone(), 0);
        agent.token_cap = self.per_agent_cap(1);
        if let Some(rx) = &self.steer_rx {
            agent = agent.with_steer_rx(rx.clone());
        }
        let token = self.session_cancel.child_token();
        if let Ok(mut map) = self.agent_tokens.lock() {
            map.insert(agent_id.0.clone(), token.clone());
        }
        let agent = agent.with_cancel_token(token.clone());
        let run = self.run_with_timeout(agent);
        let output = tokio::select! {
            res = run => res,
            _ = token.cancelled() => {
                // Cancel-vs-complete race: even if the run had just finished,
                // this branch must leave a terminal DB row — otherwise the
                // agent stays "spawned" forever.
                let _ = self.db.update_agent_status(
                    &agent_id,
                    AgentStatus::Cancelled,
                    0,
                    Some("cancelled"),
                );
                self.emit(AgentEvent::AgentFailed {
                    id: agent_id.clone(),
                    error: "cancelled".to_string(),
                });
                anyhow::bail!("agent cancelled");
            }
        }?;
        self.total_tokens += output.tokens_used + output.descendant_tokens;

        // The single-agent path must record a terminal status like fan-out
        // does — otherwise the row stays "spawned" forever.
        if output.aborted {
            let _ = self.db.update_agent_status(
                &agent_id,
                AgentStatus::Failed,
                output.tokens_used,
                Some(&output.summary),
            );
            self.emit(AgentEvent::AgentFailed {
                id: agent_id,
                error: output.summary.clone(),
            });
        } else {
            let _ = self.db.update_agent_status(
                &agent_id,
                AgentStatus::Completed,
                output.tokens_used,
                Some(&output.summary),
            );
            self.emit(AgentEvent::AgentCompleted {
                id: agent_id,
                summary: output.summary.chars().take(200).collect(),
                tokens_used: output.tokens_used,
            });
        }
        Ok(output)
    }

    /// Fan out sub-tasks to separate worker OS processes via [`ProcessManager`].
    ///
    /// Each worker is spawned as `fathom worker ...` and reports
    /// progress over a per-agent Unix domain socket. Intermediate events
    /// (tool calls, LLM chunks) are re-emitted on the local event bus so the
    /// TUI/headless progress output works identically to single-process mode.
    async fn run_multiprocess_fanout(
        &mut self,
        sub_tasks: &[String],
    ) -> anyhow::Result<Vec<AgentOutput>> {
        let socket_dir = self.output_dir.join(".sockets");
        let mut pm = ProcessManager::new(socket_dir);
        let mut worker_ids: Vec<AgentId> = Vec::new();
        let mut findings: Vec<AgentOutput> = Vec::new();

        // Step 1: spawn all workers (subject to the max_agents cap).
        for task_desc in sub_tasks {
            if self.total_agents >= self.config.agent.max_agents {
                tracing::warn!("Max agents reached, skipping remaining tasks");
                let _ = self.db.update_subtask_status(
                    &self.session_id, task_desc, "skipped",
                    Some("max agents reached"),
                );
                break;
            }
            if self.budget_exhausted() {
                tracing::warn!(
                    "session token budget reached ({}), skipping remaining tasks",
                    self.config.agent.session_token_limit
                );
                let _ = self.db.update_subtask_status(
                    &self.session_id, task_desc, "skipped",
                    Some("token budget exhausted"),
                );
                break;
            }

            let agent_id = AgentId::new();
            self.total_agents += 1;

            let agent_record = AgentRecord {
                id: agent_id.clone(),
                session_id: self.session_id.0.clone(),
                parent_id: None,
                role: AgentRole::Researcher,
                task: task_desc.clone(),
                status: AgentStatus::Spawned,
                depth: 1,
                tokens_used: 0,
                created_at: chrono::Utc::now(),
                completed_at: None,
            };
            self.db.create_agent(&agent_record)?;

            self.emit(AgentEvent::AgentSpawned {
                id: agent_id.clone(),
                parent: None,
                role: "researcher".to_string(),
                task: task_desc.clone(),
                depth: 1,
            });

            match pm
                .spawn_worker(
                    agent_id.clone(),
                    &self.session_id.0,
                    task_desc.clone(),
                    AgentRole::Researcher,
                )
                .await
            {
                Ok(()) => {
                    worker_ids.push(agent_id);
                }
                Err(e) => {
                    tracing::error!("Failed to spawn worker {agent_id}: {e}");
                    let _ = self.db.update_agent_status(
                        &agent_id,
                        AgentStatus::Failed,
                        0,
                        Some(&format!("worker spawn failed: {e}")),
                    );
                    self.emit(AgentEvent::AgentFailed {
                        id: agent_id,
                        error: format!("worker spawn failed: {e}"),
                    });
                }
            }
        }

        // Step 2: wait for every worker and collect results. Workers run
        // concurrently; we wait on them one at a time, but that does not
        // serialize their actual work.
        for agent_id in worker_ids {
            let result = pm
                .wait_for_completion_with_events(&agent_id, Some(&self.event_tx))
                .await;

            match result {
                Ok(WorkerResult::Completed { summary, tokens_used }) => {
                    self.total_tokens += tokens_used; // worker incl. its own only
                    self.db.update_agent_status(
                        &agent_id,
                        AgentStatus::Completed,
                        tokens_used,
                        Some(&summary),
                    )?;
                    self.emit(AgentEvent::AgentCompleted {
                        id: agent_id.clone(),
                        summary: summary.chars().take(200).collect(),
                        tokens_used,
                    });
                    findings.push(AgentOutput {
                        agent_id,
                        summary,
                        tokens_used,
                        descendant_tokens: 0,
                        findings: vec![],
                        aborted: false,
                    });
                }
                Ok(WorkerResult::Failed { error }) => {
                    tracing::error!("Worker {agent_id} failed: {error}");
                    let _ = self.db.update_agent_status(
                        &agent_id,
                        AgentStatus::Failed,
                        0,
                        Some(&error),
                    );
                    self.emit(AgentEvent::AgentFailed {
                        id: agent_id,
                        error,
                    });
                }
                Ok(WorkerResult::Disconnected) => {
                    tracing::error!("Worker {agent_id} disconnected before completing");
                    let _ = self.db.update_agent_status(
                        &agent_id,
                        AgentStatus::Failed,
                        0,
                        Some("worker disconnected before completing"),
                    );
                    self.emit(AgentEvent::AgentFailed {
                        id: agent_id,
                        error: "worker disconnected before completing".to_string(),
                    });
                }
                Err(e) => {
                    tracing::error!("Error waiting for worker {agent_id}: {e}");
                    let _ = self.db.update_agent_status(
                        &agent_id,
                        AgentStatus::Failed,
                        0,
                        Some(&format!("wait error: {e}")),
                    );
                    self.emit(AgentEvent::AgentFailed {
                        id: agent_id,
                        error: format!("wait error: {e}"),
                    });
                }
            }
        }

        // Step 3: clean up processes and sockets.
        pm.shutdown_all().await;

        Ok(findings)
    }

    async fn synthesize(&self, findings: &[AgentOutput]) -> anyhow::Result<String> {
        if findings.is_empty() {
            return Ok("No findings were collected.".to_string());
        }

        // Cap each summary to a fair share of the synthesis context so a
        // large batch of findings cannot overflow the single LLM call
        // (overshoot spills to disk, same mechanism as spawn results).
        let headroom = (self.config.context.resolved_window() as usize).saturating_mul(2);
        let budget = crate::budget::ResultBudget::new(
            headroom,
            findings.len().max(1),
            self.output_dir.join(".pr-context").join("spills"),
        );
        let findings_text: Vec<String> = findings.iter().enumerate().map(|(i, f)| {
            let capped = budget.cap_result(&f.summary);
            format!("### Finding {}\n{}", i + 1, capped.summary)
        }).collect();

        // Pull attention-worthy beacons from the shared tree blackboard so the
        // synthesis considers blockers/questions children flagged mid-run, not
        // just their final summaries.
        let mut coordinate_note = String::new();
        if let Some(tree) = &self.tree_ledger {
            let rows = tree.tail(40).await;
            let attn: Vec<&crate::task_tree::TreeRow> = rows
                .iter()
                .filter(|r| r.is_attention_kind())
                .take(12)
                .collect();
            if !attn.is_empty() {
                let mut lines: Vec<String> = attn
                    .iter()
                    .map(|r| format!("- [{}] {}", r.kind, r.text))
                    .collect();
                lines.insert(0, "Children flagged these coordinate signals:".to_string());
                coordinate_note = lines.join("\n");
            }
        }

        let synth_prompt = format!(
            concat!(
                "You are synthesizing research findings into a final report.\n",
                "\n",
                "Original query: {}\n",
                "\n",
                "Findings from sub-agents:\n",
                "\n",
                "{}\n",
                "\n",
                "{}\n",
                "\n",
                "Write a comprehensive, well-structured markdown report that:\n",
                "1. Answers the original query\n",
                "2. Integrates all findings coherently\n",
                "3. Notes any contradictions between sources\n",
                "4. Lists key sources/references\n",
                "5. Identifies gaps or areas for further research\n",
                "\n",
                "Write in a clear, informative style. Use markdown headers, bullet points, and emphasis where appropriate."
            ),
            self.query,
            findings_text.join("\n\n"),
            coordinate_note,
        );

        // Reasoning models spend completion tokens on chain-of-thought too,
        // so a long report can exhaust the configured max_tokens before any
        // answer text is produced (empty content, finish_reason "length").
        // Give synthesis a generous floor, then escalate once if the result
        // still comes back empty.
        let base_budget = self.config.llm.max_tokens.max(16_384);
        let budgets = [base_budget, base_budget.saturating_mul(2).min(32_768)];

        for (attempt, budget) in budgets.iter().enumerate() {
            let req = CompletionRequest {
                messages: vec![
                    Message::system(format!(
                        "You are a research synthesizer.\n\n{}",
                        role_prompt_for(AgentRole::Writer)
                    )),
                    Message::user(synth_prompt.clone()),
                ],
                tools: vec![],
                temperature: Some(0.5),
                max_tokens: Some(*budget),
                stream: false,
            };

            // The final report is a writer-level call: honor
            // `[agent.role_models] writer` when configured.
            let response = self.llm_for_role(AgentRole::Writer).complete(&req).await?;

            if let Message::Assistant { content, .. } = &response.message {
                let text = content.clone().unwrap_or_default();
                if !text.trim().is_empty() {
                    return Ok(text);
                }
                tracing::warn!(
                    attempt = attempt + 1,
                    max_tokens = budget,
                    finish_reason = response.finish_reason.as_deref().unwrap_or("?"),
                    "Synthesis returned empty content, retrying with larger budget"
                );
            }
        }

        // Never ship an empty report: assemble a deterministic fallback from
        // the raw findings so the user always gets the collected data.
        tracing::error!("Synthesis failed after retries; using fallback assembly");
        Ok(format!(
            "# Research Report (fallback)\n\n\
             The LLM synthesis call returned no content (the model likely \
             exhausted its output budget on reasoning). Below are the raw \
             findings collected from sub-agents.\n\n\
             **Original query:** {}\n\n{}\n",
            self.query,
            findings_text.join("\n\n")
        ))
    }

    /// Match subtask rows against agent outcomes (Goal Mode light, fleet E4).
    fn sync_subtask_statuses(&self) {
        let agents = match self.db.get_session_agents_detail(&self.session_id) {
            Ok(rows) => rows,
            Err(_) => return,
        };
        for agent in &agents {
            let status = match agent.status.as_str() {
                "completed" => "completed",
                "failed" => "failed",
                _ => "running",
            };
            let _ = self.db.update_subtask_status(
                &self.session_id,
                &agent.task,
                status,
                agent.summary.as_deref(),
            );
        }
    }

    /// Count contacts persisted during this session (created since start).
    async fn contacts_saved_so_far(&self) -> Option<u32> {
        let store = self.contact_db.as_ref()?;
        let all = store.list_all(i64::MAX, 0).await.ok()?;
        Some(
            all.iter()
                .filter(|c| c.created_at >= self.started_at)
                .count() as u32,
        )
    }

    fn write_output(&self, synthesis: &str, findings: &[AgentOutput]) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.output_dir)?;

        // Write summary.md
        let summary_path = self.output_dir.join("summary.md");
        std::fs::write(&summary_path, synthesis)?;

        // Write index.md
        let index_content = format!(
            r#"# Research: {}

**Date**: {}
**Agents**: {}
**Tokens used**: {}

## Files

- [Summary](summary.md)
{}
"#,
            self.query,
            chrono::Utc::now().to_rfc3339(),
            self.total_agents,
            self.total_tokens,
            findings.iter().enumerate().map(|(i, _f)| {
                format!("- [Finding {}](findings/finding-{}.md)", i + 1, i + 1)
            }).collect::<Vec<_>>().join("\n")
        );
        std::fs::write(self.output_dir.join("index.md"), index_content)?;

        // Write individual findings
        let findings_dir = self.output_dir.join("findings");
        std::fs::create_dir_all(&findings_dir)?;
        for (i, finding) in findings.iter().enumerate() {
            let path = findings_dir.join(format!("finding-{}.md", i + 1));
            std::fs::write(&path, &finding.summary)?;
        }

        // Write sources.md from the structured findings' sources (fleet C4).
        let mut sources_content = String::from("# Sources\n\n");
        let mut seen = std::collections::HashSet::new();
        let mut count = 0usize;
        for output in findings {
            for finding in &output.findings {
                for src in &finding.sources {
                    if seen.insert(src.url.clone()) {
                        sources_content.push_str(&format!("- [{}]({})\n", src.title, src.url));
                        count += 1;
                    }
                }
            }
        }
        if count == 0 {
            sources_content.push_str("_No structured sources were recorded._\n");
        }
        std::fs::write(self.output_dir.join("sources.md"), sources_content)?;

        tracing::info!("Output written to {}", self.output_dir.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures::Stream;
    use pr_core::{PrError, PrResult};
    use pr_llm::{CompletionResponse, StreamChunk, Usage};
    use std::collections::VecDeque;
    use tokio::sync::Mutex;

    /// Scripted LLM provider: returns queued responses in order, then a
    /// default response once the queue is exhausted.
    struct MockProvider {
        responses: Mutex<VecDeque<CompletionResponse>>,
    }

    impl MockProvider {
        fn new(responses: Vec<CompletionResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
            }
        }

        fn assistant(text: &str) -> CompletionResponse {
            CompletionResponse {
                message: Message::assistant(text),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
                finish_reason: Some("stop".to_string()),
            }
        }

        /// Reasoning-model truncation: empty content, finish_reason "length".
        fn empty_truncated() -> CompletionResponse {
            CompletionResponse {
                message: Message::assistant(""),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 8192,
                    total_tokens: 8202,
                }),
                finish_reason: Some("length".to_string()),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        fn model(&self) -> &str {
            "mock-model"
        }

        async fn complete(&self, _req: &CompletionRequest) -> PrResult<CompletionResponse> {
            let mut q = self.responses.lock().await;
            Ok(q.pop_front().unwrap_or_else(|| Self::assistant("default answer")))
        }

        async fn stream(
            &self,
            _req: &CompletionRequest,
        ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
            Err(PrError::Llm("stream not used in tests".into()))
        }
    }

    fn make_coordinator(
        llm: Arc<dyn LlmProvider>,
        output_dir: std::path::PathBuf,
        config: AppConfig,
    ) -> (Coordinator, broadcast::Receiver<AgentEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);
        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "test query").unwrap();
        let coordinator = Coordinator::new(
            session_id,
            "test query".to_string(),
            llm,
            Arc::new(ToolRegistry::new()),
            event_tx,
            db,
            output_dir,
            config,
        );
        (coordinator, event_rx)
    }

    #[test]
    fn test_multiprocess_flag_read_from_config() {
        let mut config = AppConfig::default();
        assert!(!config.agent.use_multiprocess);

        let tmp = tempfile::tempdir().unwrap();
        let (coordinator, _) = make_coordinator(
            Arc::new(MockProvider::new(vec![])),
            tmp.path().to_path_buf(),
            config.clone(),
        );
        assert!(!coordinator.use_multiprocess);

        config.agent.use_multiprocess = true;
        let (coordinator, _) = make_coordinator(
            Arc::new(MockProvider::new(vec![])),
            tmp.path().to_path_buf(),
            config,
        );
        assert!(coordinator.use_multiprocess);
    }

    #[test]
    fn test_per_agent_cap_splits_remaining_budget() {
        let mut config = AppConfig::default();
        config.agent.session_token_limit = 100_000;
        let tmp = tempfile::tempdir().unwrap();
        let (mut coordinator, _) = make_coordinator(
            Arc::new(MockProvider::new(vec![])),
            tmp.path().to_path_buf(),
            config,
        );
        // No usage yet: 100k split across 4 tasks = 25k each.
        assert_eq!(coordinator.per_agent_cap(4), Some(25_000));
        // After 60k used, 40k left across 2 tasks = 20k each.
        coordinator.total_tokens = 60_000;
        assert_eq!(coordinator.per_agent_cap(2), Some(20_000));
        // Floor: nearly-exhausted budgets still allow a minimal turn.
        coordinator.total_tokens = 99_999;
        assert_eq!(coordinator.per_agent_cap(3), Some(4_096));
    }

    #[test]
    fn test_per_agent_cap_none_without_limit() {
        let config = AppConfig::default(); // session_token_limit = 0
        let tmp = tempfile::tempdir().unwrap();
        let (coordinator, _) = make_coordinator(
            Arc::new(MockProvider::new(vec![])),
            tmp.path().to_path_buf(),
            config,
        );
        assert_eq!(coordinator.per_agent_cap(5), None);
        assert!(!coordinator.budget_exhausted());
    }

    #[tokio::test]
    async fn test_plan_parses_json_array() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut coordinator, _) = make_coordinator(
            Arc::new(MockProvider::new(vec![MockProvider::assistant(
                "[\"Task one\", \"Task two\", \"Task three\"]",
            )])),
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );

        let tasks = coordinator.plan().await.unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0], "Task one");
        assert_eq!(tasks[2], "Task three");
    }

    #[tokio::test]
    async fn test_plan_extracts_json_from_markdown_fence() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut coordinator, _) = make_coordinator(
            Arc::new(MockProvider::new(vec![MockProvider::assistant(
                "Here you go:\n```json\n[\"A\", \"B\"]\n```",
            )])),
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );

        let tasks = coordinator.plan().await.unwrap();
        assert_eq!(tasks, vec!["A".to_string(), "B".to_string()]);
    }

    #[tokio::test]
    async fn test_plan_falls_back_to_single_task() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut coordinator, _) = make_coordinator(
            Arc::new(MockProvider::new(vec![MockProvider::assistant(
                "I cannot decompose this.",
            )])),
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );

        let tasks = coordinator.plan().await.unwrap();
        assert_eq!(tasks, vec!["test query".to_string()]);
    }

    #[tokio::test]
    async fn test_execute_single_process_end_to_end() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            // plan
            MockProvider::assistant("[\"Subtask A\", \"Subtask B\"]"),
            // researcher agents (order of the two runs is nondeterministic)
            MockProvider::assistant("Finding A"),
            MockProvider::assistant("Finding B"),
            // synthesis
            MockProvider::assistant("# Final report"),
        ]));

        // Goal Mode judge is out of scope for this test (it would consume a
        // scripted response); disable replanning.
        let mut config = AppConfig::default();
        config.agent.replan_rounds = 0;
        let (mut coordinator, mut event_rx) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            config,
        );

        let output = coordinator.execute().await.unwrap();

        assert_eq!(output.total_agents, 2);
        assert_eq!(output.total_tokens, 60); // 2 agents x 30 tokens
        assert_eq!(output.synthesis, "# Final report");

        // Output files written
        assert!(tmp.path().join("summary.md").exists());
        assert!(tmp.path().join("index.md").exists());
        assert!(tmp.path().join("sources.md").exists());
        assert!(tmp.path().join("findings/finding-1.md").exists());
        assert!(tmp.path().join("findings/finding-2.md").exists());

        // Events emitted on the bus
        let mut events = Vec::new();
        while let Ok(e) = event_rx.try_recv() {
            events.push(e);
        }
        assert!(events.iter().any(|e| matches!(e, AgentEvent::SessionStarted { .. })));
        let spawned = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::AgentSpawned { .. }))
            .count();
        let completed = events
            .iter()
            .filter(|e| matches!(e, AgentEvent::AgentCompleted { .. }))
            .count();
        assert_eq!(spawned, 2);
        assert_eq!(completed, 2);
        assert!(events.iter().any(|e| matches!(e, AgentEvent::SessionCompleted { .. })));
    }

    #[tokio::test]
    async fn test_execute_no_subtasks_runs_single_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            // plan returns an empty array -> single-agent path
            MockProvider::assistant("[]"),
            // single agent run
            MockProvider::assistant("Single finding"),
            // synthesis
            MockProvider::assistant("Report"),
        ]));

        let (mut coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );

        let output = coordinator.execute().await.unwrap();
        assert_eq!(output.total_agents, 1);
        assert!(tmp.path().join("findings/finding-1.md").exists());
        let finding = std::fs::read_to_string(tmp.path().join("findings/finding-1.md")).unwrap();
        assert_eq!(finding, "Single finding");
    }


    #[tokio::test]
    async fn test_execute_resume_reruns_pending_and_merges_completed() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            // rerun of the pending task
            MockProvider::assistant("rerun result"),
            // synthesis
            MockProvider::assistant("# Resumed report"),
        ]));

        let (mut coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );

        let state = crate::resume::ResumeState {
            session_id: SessionId::new(),
            query: "interrupted research".to_string(),
            completed_agents: vec![AgentOutput {
                agent_id: AgentId::new(),
                summary: "done before crash".to_string(),
                tokens_used: 30,
                        descendant_tokens: 0,
                findings: vec![],
            
                aborted: false,
            }],
            pending_tasks: vec!["pending task".to_string()],
        };

        let output = coordinator.execute_resume(state).await.unwrap();

        // Recovered agent + rerun are both accounted in the session totals.
        assert_eq!(output.total_agents, 2); // 1 recovered + 1 rerun
        assert_eq!(output.total_tokens, 60); // recovered (30) + rerun (30)
        assert_eq!(output.synthesis, "# Resumed report");

        // Both findings are written: recovered + rerun.
        assert!(tmp.path().join("findings/finding-1.md").exists());
        assert!(tmp.path().join("findings/finding-2.md").exists());
        assert!(tmp.path().join("summary.md").exists());
    }


    #[test]
    fn test_detect_task_type() {
        assert_eq!(Coordinator::detect_task_type("Найди контакты CEO IT-компаний"), TaskType::LeadGen);
        assert_eq!(Coordinator::detect_task_type("find emails of decision makers"), TaskType::LeadGen);
        assert_eq!(Coordinator::detect_task_type("Что такое квантовые компьютеры?"), TaskType::Research);
    }

    #[test]
    fn test_detect_target_count() {
        assert_eq!(Coordinator::detect_target_count("Найди 20 email CEO в Москве"), Some(20));
        assert_eq!(Coordinator::detect_target_count("find 15 leads in Berlin"), Some(15));
        assert_eq!(Coordinator::detect_target_count("найди контакты без числа"), None);
        assert_eq!(Coordinator::detect_target_count("компания работает 20 лет, найди контакты"), None);
    }

    #[tokio::test]
    async fn test_plan_leadgen_sets_task_type() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![MockProvider::assistant(
            "[\"Task A: find emails via directories; quota: 5\", \"Task B: find emails via social; quota: 5\"]",
        )]));
        let (mut coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );
        coordinator.query = "Найди 10 email CEO".to_string();

        let tasks = coordinator.plan().await.unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(coordinator.task_type, TaskType::LeadGen);
        assert_eq!(coordinator.target_count, Some(10));
        assert!(tasks[0].contains("quota"));
    }

    #[tokio::test]
    async fn test_reflection_runs_gap_round_when_short() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            // plan (leadgen)
            MockProvider::assistant("[\"collect emails\"]"),
            // first-round researcher: done immediately
            MockProvider::assistant("found nothing useful"),
            // gap-round researcher
            MockProvider::assistant("found the rest"),
            // synthesis
            MockProvider::assistant("# Report"),
        ]));

        // This test exercises the lead-gen reflection round; the Goal Mode
        // judge would consume a scripted response, so disable replanning.
        let mut config = AppConfig::default();
        config.agent.replan_rounds = 0;
        let (mut coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            config,
        );
        coordinator.query = "Найди 5 email".to_string();
        coordinator.task_type = TaskType::LeadGen;
        coordinator.target_count = Some(5);
        // No contact DB attached -> contacts_saved_so_far() = None -> no
        // reflection; attach an EMPTY store to trigger the gap round.
        let store: Arc<dyn pr_persistence::ContactStore> =
            Arc::new(pr_persistence::ContactDb::in_memory().unwrap());
        coordinator.contact_db = Some(store);

        let output = coordinator.execute().await.unwrap();
        // 1 planned + 1 gap-filling agent ran.
        assert_eq!(output.total_agents, 2);
        assert!(output.synthesis.contains("# Report"));
    }

    #[tokio::test]
    async fn test_no_reflection_when_target_met() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            MockProvider::assistant("[\"collect emails\"]"),
            MockProvider::assistant("found them all"),
            MockProvider::assistant("# Report"),
        ]));

        let (mut coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );
        coordinator.query = "Найди 1 email".to_string();
        coordinator.task_type = TaskType::LeadGen;
        coordinator.target_count = Some(1);
        let store = Arc::new(pr_persistence::ContactDb::in_memory().unwrap());
        // Pre-populate one contact so the target is already met.
        let contact = pr_core::Contact {
            email: Some("x@y.z".into()),
            ..pr_core::Contact::new()
        };
        store.add_contact(&contact).unwrap();
        // started_at must precede the contact creation.
        coordinator.started_at = chrono::Utc::now() - chrono::Duration::minutes(1);
        coordinator.contact_db = Some(store);

        let output = coordinator.execute().await.unwrap();
        assert_eq!(output.total_agents, 1, "no gap round when target met");
    }


    #[tokio::test]
    async fn test_stall_monitor_cancels_idle_agent() {
        use std::sync::Mutex as StdMutex;

        let (event_tx, event_rx) = broadcast::channel::<AgentEvent>(32);
        let token = CancellationToken::new();
        let tokens: Arc<StdMutex<HashMap<String, CancellationToken>>> =
            Arc::new(StdMutex::new(HashMap::new()));
        tokens
            .lock()
            .unwrap()
            .insert("agent-1".to_string(), token.clone());

        // Register the agent on the bus so the monitor tracks it.
        event_tx
            .send(AgentEvent::AgentSpawned {
                id: AgentId("agent-1".to_string()),
                parent: None,
                role: "researcher".to_string(),
                task: "t".to_string(),
                depth: 1,
            })
            .unwrap();

        // kill after 1s idle, checked every 50ms.
        let handle = tokio::spawn(stall_monitor_loop(
            event_rx,
            tokens,
            0,
            1,
            std::time::Duration::from_millis(50),
            "sess-test".to_string(),
        ));

        tokio::time::timeout(std::time::Duration::from_secs(3), token.cancelled())
            .await
            .expect("stalled agent must be cancelled within 3s");
        handle.abort();
    }

    #[tokio::test]
    async fn test_execute_respects_max_agents() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            MockProvider::assistant("[\"A\", \"B\", \"C\", \"D\"]"),
            MockProvider::assistant("r1"),
            MockProvider::assistant("r2"),
            MockProvider::assistant("report"),
        ]));

        let mut config = AppConfig::default();
        config.agent.max_agents = 2;

        let (mut coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            config,
        );

        let output = coordinator.execute().await.unwrap();
        // Only 2 of the 4 planned sub-tasks were executed.
        assert_eq!(output.total_agents, 2);
    }

    fn sample_finding(summary: &str) -> AgentOutput {
        AgentOutput {
            agent_id: AgentId::new(),
            summary: summary.to_string(),
            tokens_used: 10,
            descendant_tokens: 0,
            findings: vec![],
            aborted: false,
        }
    }

    #[tokio::test]
    async fn test_synthesize_retries_on_empty_content() {
        // Reasoning model burned the first budget on chain-of-thought and
        // returned empty content (finish_reason "length"). Synthesis must
        // retry with a larger budget instead of emitting an empty report.
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            MockProvider::empty_truncated(),
            MockProvider::assistant("# Recovered report"),
        ]));
        let (coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );

        let findings = vec![sample_finding("finding one")];
        let result = coordinator.synthesize(&findings).await.unwrap();
        assert_eq!(result, "# Recovered report");
    }

    #[tokio::test]
    async fn test_synthesize_falls_back_to_raw_findings() {
        // Both attempts return empty content: the user must still receive a
        // non-empty report assembled from the raw findings.
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            MockProvider::empty_truncated(),
            MockProvider::empty_truncated(),
        ]));
        let (coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            AppConfig::default(),
        );

        let findings = vec![sample_finding("finding one")];
        let result = coordinator.synthesize(&findings).await.unwrap();
        assert!(result.contains("Research Report (fallback)"));
        assert!(result.contains("finding one"));
        assert!(result.contains("test query"));
    }

    #[tokio::test]
    async fn test_synthesize_budget_floor_for_reasoning_models() {
        // Even when config max_tokens is small (8192 in production), the
        // synthesis call must request at least 16384 output tokens so a
        // reasoning model has room for both thinking and the report.
        let tmp = tempfile::tempdir().unwrap();

        struct BudgetSpy {
            seen: Mutex<Vec<u32>>,
        }
        #[async_trait]
        impl LlmProvider for BudgetSpy {
            fn name(&self) -> &str {
                "spy"
            }
            fn model(&self) -> &str {
                "spy-model"
            }
            async fn complete(&self, req: &CompletionRequest) -> PrResult<CompletionResponse> {
                self.seen.lock().await.push(req.max_tokens.unwrap_or(0));
                Ok(MockProvider::assistant("# Report"))
            }
            async fn stream(
                &self,
                _req: &CompletionRequest,
            ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
                Err(PrError::Llm("unused".into()))
            }
        }

        let spy = Arc::new(BudgetSpy {
            seen: Mutex::new(Vec::new()),
        });
        let mut config = AppConfig::default();
        config.llm.max_tokens = 8192;
        let (coordinator, _) = make_coordinator(
            spy.clone(),
            tmp.path().to_path_buf(),
            config,
        );

        let findings = vec![sample_finding("finding one")];
        coordinator.synthesize(&findings).await.unwrap();

        let seen = spy.seen.lock().await;
        assert_eq!(seen.len(), 1);
        assert!(
            seen[0] >= 16_384,
            "synthesis budget must be at least 16384, got {}",
            seen[0]
        );
    }

    // ── Full Goal Mode (evaluate_and_replan) ──────────────────────────

    #[tokio::test]
    async fn test_replan_returns_gap_tasks_when_incomplete() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![MockProvider::assistant(
            r#"{"complete": false, "new_subtasks": ["Find pricing details", "Find support SLA"]}"#,
        )]));
        let (coordinator, _) = make_coordinator(llm, tmp.path().to_path_buf(), AppConfig::default());

        let findings = vec![sample_finding("found the company overview")];
        let tasks = coordinator.evaluate_and_replan(&findings).await;
        assert_eq!(
            tasks,
            Some(vec![
                "Find pricing details".to_string(),
                "Find support SLA".to_string()
            ])
        );
    }

    #[tokio::test]
    async fn test_replan_none_when_goal_complete() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![MockProvider::assistant(
            r#"{"complete": true, "new_subtasks": []}"#,
        )]));
        let (coordinator, _) = make_coordinator(llm, tmp.path().to_path_buf(), AppConfig::default());

        let findings = vec![sample_finding("everything covered")];
        assert_eq!(coordinator.evaluate_and_replan(&findings).await, None);
    }

    #[tokio::test]
    async fn test_replan_parses_json_embedded_in_prose() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![MockProvider::assistant(
            r#"After review: {"complete": false, "new_subtasks": ["Gap task one"]} — that's my verdict."#,
        )]));
        let (coordinator, _) = make_coordinator(llm, tmp.path().to_path_buf(), AppConfig::default());

        let findings = vec![sample_finding("partial coverage")];
        assert_eq!(
            coordinator.evaluate_and_replan(&findings).await,
            Some(vec!["Gap task one".to_string()])
        );
    }

    #[tokio::test]
    async fn test_replan_caps_at_three_tasks_and_skips_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![MockProvider::assistant(
            r#"{"complete": false, "new_subtasks": ["a", "", "b", "c", "d"]}"#,
        )]));
        let (coordinator, _) = make_coordinator(llm, tmp.path().to_path_buf(), AppConfig::default());

        let findings = vec![sample_finding("x")];
        let tasks = coordinator.evaluate_and_replan(&findings).await.unwrap();
        assert_eq!(tasks, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn test_replan_none_on_garbage_verdict() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![MockProvider::assistant(
            "I cannot decide, this is hard.",
        )]));
        let (coordinator, _) = make_coordinator(llm, tmp.path().to_path_buf(), AppConfig::default());

        let findings = vec![sample_finding("x")];
        assert_eq!(coordinator.evaluate_and_replan(&findings).await, None);
    }

    #[tokio::test]
    async fn test_replan_skipped_when_agent_budget_reached() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = AppConfig::default();
        config.agent.max_agents = 1; // already "spent"
        // Even a judge that wants more tasks must be ignored.
        let llm = Arc::new(MockProvider::new(vec![MockProvider::assistant(
            r#"{"complete": false, "new_subtasks": ["should not run"]}"#,
        )]));
        let (mut coordinator, _) =
            make_coordinator(llm, tmp.path().to_path_buf(), config);
        coordinator.total_agents = 1;

        let findings = vec![sample_finding("x")];
        assert_eq!(coordinator.evaluate_and_replan(&findings).await, None);
    }

    #[tokio::test]
    async fn test_execute_runs_goal_mode_replan_round() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            // plan
            MockProvider::assistant("[\"Subtask A\"]"),
            // researcher A
            MockProvider::assistant("Finding A"),
            // goal-mode judge: one concrete gap
            MockProvider::assistant(
                r#"{"complete": false, "new_subtasks": ["Gap task"]}"#,
            ),
            // gap-filling researcher
            MockProvider::assistant("Gap finding"),
            // synthesis
            MockProvider::assistant("# Report with gap covered"),
        ]));

        let mut config = AppConfig::default();
        config.agent.replan_rounds = 1;
        let (mut coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            config,
        );

        let output = coordinator.execute().await.unwrap();
        // 1 planned researcher + 1 gap-filling researcher.
        assert_eq!(output.total_agents, 2);
        assert_eq!(output.synthesis, "# Report with gap covered");
    }

    #[tokio::test]
    async fn test_execute_stops_replanning_when_judge_satisfied() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = Arc::new(MockProvider::new(vec![
            // plan
            MockProvider::assistant("[\"Subtask A\"]"),
            // researcher A
            MockProvider::assistant("Finding A"),
            // goal-mode judge round 1: satisfied immediately
            MockProvider::assistant(r#"{"complete": true, "new_subtasks": []}"#),
            // synthesis
            MockProvider::assistant("# Done"),
        ]));

        let mut config = AppConfig::default();
        config.agent.replan_rounds = 3; // generous budget, unused
        let (mut coordinator, _) = make_coordinator(
            llm,
            tmp.path().to_path_buf(),
            config,
        );

        let output = coordinator.execute().await.unwrap();
        assert_eq!(output.total_agents, 1, "no gap agents when judge is satisfied");
        assert_eq!(output.synthesis, "# Done");
    }
}
