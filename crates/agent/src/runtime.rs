use pr_core::*;
use pr_core::memory::MemoryStore;
use pr_core::skill::SkillRegistry;
use pr_llm::{CompletionRequest, CompletionResponse, LlmProvider, StreamChunk, Usage};
use pr_tools::{ToolRegistry, ToolContext, Truncated, TurnBudget, apply_turn_budget};
use pr_persistence::Persistence;
use crate::compaction::CompactionEngine;
use crate::doom_loop::DoomLoopDetector;
use crate::prompt::PromptBuilder;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub struct AgentRuntime {
    pub id: AgentId,
    pub session_id: SessionId,
    pub parent_id: Option<AgentId>,
    pub role: AgentRole,
    pub task: String,
    pub depth: u32,
    pub llm: Arc<dyn LlmProvider>,
    pub tools: Arc<ToolRegistry>,
    pub event_tx: broadcast::Sender<AgentEvent>,
    pub db: Arc<Persistence>,
    pub working_dir: std::path::PathBuf,
    pub max_iterations: u32,
    pub config: AppConfig,
    pub messages: Vec<Message>,
    pub tokens_used: u64,
    pub descendant_tokens: u64,
    pub estimated_tokens: u32,
    /// Contact database handed to tools (`save_contacts`); inherited by
    /// spawned sub-agents.
    pub contact_db: Option<Arc<dyn pr_persistence::ContactStore>>,
    /// CRM sync handed to tools (`save_contacts`); inherited by spawned
    /// sub-agents.
    pub crm: Option<Arc<pr_core::CrmSync>>,
    /// Long-term semantic memory (mem0/Memora-inspired); inherited by
    /// spawned sub-agents. Drives prompt digests, the memory_* tools and
    /// deterministic autosave of harvested contacts.
    pub memory: Option<Arc<pr_memory::Memory>>,
    /// Cheap/fast provider (`[llm] fast_model`) handed to tools for
    /// high-volume auxiliary calls; inherited by spawned sub-agents.
    pub fast_llm: Option<Arc<dyn LlmProvider>>,
    /// Optional per-agent token budget (derived by the coordinator from
    /// `[agent] session_token_limit`). The agent stops gracefully at the
    /// turn boundary when its own usage reaches the cap.
    pub token_cap: Option<u64>,
    /// Session-shared HTTP fetch cache (sibling agents reuse downloads
    /// within the TTL); inherited by spawned sub-agents.
    pub fetch_cache: Option<pr_tools::cache::FetchCache>,
    /// Session-shared MX lookup cache; inherited by spawned sub-agents.
    pub mx_cache: Option<pr_tools::cache::MxCache>,
    /// Extra system-prompt block from an active profile/persona
    /// (`--profile`); inherited by spawned sub-agents.
    pub profile_prompt: Option<String>,
    compaction_engine: CompactionEngine,
    turn_budget: TurnBudget,
    memory_store: MemoryStore,
    skill_registry: SkillRegistry,
    doom_loop: DoomLoopDetector,
    /// First doom-loop offense only nudges; the second stops the agent.
    doom_nudged: bool,
    /// Structured findings harvested from tool metadata (fleet C4).
    harvested_findings: Vec<Finding>,
    /// Cooperative cancellation (session cancel, stall kill, steering stop).
    cancel: CancellationToken,
    /// Tools this agent's role is not allowed to use (fleet E5).
    denied_tools: HashSet<String>,
    /// Mid-run user instructions (fleet E1), drained at turn boundaries.
    steer_rx: Option<Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<String>>>>,
    /// Control plane: `question` tool requests to the operator.
    pub question_tx: Option<crate::control::QuestionTx>,
    /// Control plane: approval requests for side-effect tools.
    pub approval_tx: Option<crate::control::ApprovalTx>,
    /// Completion notices from background children (fleet E2):
    /// (label, summary, subtree_tokens).
    bg_results: Arc<std::sync::Mutex<Vec<(String, Result<String, String>, u64)>>>,
    /// How many times Stop hooks already forced a continuation.
    stop_continuations: u32,
    /// How many times a truncated reasoning response (empty content,
    /// finish_reason "length") was retried with a "answer directly" nudge.
    truncation_retries: u32,
    /// Per-role LLM overrides inherited from the coordinator (fleet E8);
    /// children pick their provider by role at spawn time.
    role_llms: std::collections::HashMap<String, Arc<dyn LlmProvider>>,
    /// Receiver half of the process-global IrcBus channel for this agent.
    /// Incoming peer messages are drained at turn boundaries (fleet hub).
    irc_rx: Option<tokio::sync::mpsc::UnboundedReceiver<pr_core::IrcMessage>>,
}

/// Per-role tool deny list from config (fleet E5). Keys are lowercase role
/// names: researcher, analyst, verifier, writer, coordinator.
fn denied_tools_for_role(config: &AppConfig, role: AgentRole) -> HashSet<String> {
    let key = match role {
        AgentRole::Coordinator => "coordinator",
        AgentRole::Researcher => "researcher",
        AgentRole::Analyst => "analyst",
        AgentRole::Verifier => "verifier",
        AgentRole::Writer => "writer",
    };
    config
        .agent
        .deny_tools
        .get(key)
        .map(|tools| tools.iter().map(|t| t.to_lowercase()).collect())
        .unwrap_or_default()
}

/// How many times a truncated reasoning response (empty content with
/// finish_reason "length") may be retried with a "answer directly" nudge
/// before the agent is allowed to stop with whatever it has.
const MAX_TRUNCATION_RETRIES: u32 = 2;

/// A tool call that survived the sequential pre-pass (doom loop, role
/// permissions, PreToolUse hooks). `Exec` calls go to the batch executor
/// (parallel-safe ones run concurrently); `Immediate` calls were rejected
/// before execution and carry their final error output.
enum PreparedCall {
    Exec(ToolCall),
    Immediate(ToolCall, ToolOutput),
}

impl AgentRuntime {
    pub fn new(
        id: AgentId,
        session_id: SessionId,
        parent_id: Option<AgentId>,
        role: AgentRole,
        task: String,
        depth: u32,
        llm: Arc<dyn LlmProvider>,
        tools: Arc<ToolRegistry>,
        event_tx: broadcast::Sender<AgentEvent>,
        db: Arc<Persistence>,
        working_dir: std::path::PathBuf,
        config: AppConfig,
    ) -> Self {
        let max_iterations = config.agent.max_iterations;
        let denied = denied_tools_for_role(&config, role);
        let turn_budget = TurnBudget::new(config.context.turn_budget_bytes);
        let compaction_engine = CompactionEngine::new(config.context.clone());

        let home_dir = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        let memory_store = MemoryStore::new(&home_dir);
        let mut skill_registry = SkillRegistry::new(&home_dir);
        // Best-effort skill discovery; don't fail agent startup.
        if let Err(e) = skill_registry.discover() {
            tracing::warn!("skill discovery failed: {e}");
        }

        Self {
            id, session_id, parent_id, role, task, depth,
            llm, tools, event_tx, db, working_dir,
            max_iterations, config,
            messages: Vec::new(),
            tokens_used: 0,
            descendant_tokens: 0,
            estimated_tokens: 0,
            contact_db: None,
            crm: None,
            memory: None,
            fast_llm: None,
            token_cap: None,
            fetch_cache: None,
            mx_cache: None,
            profile_prompt: None,
            compaction_engine,
            turn_budget,
            memory_store,
            skill_registry,
            doom_loop: DoomLoopDetector::new(),
            doom_nudged: false,
            harvested_findings: Vec::new(),
            cancel: CancellationToken::new(),
            denied_tools: denied,
            steer_rx: None,
            question_tx: None,
            approval_tx: None,
            bg_results: Arc::new(std::sync::Mutex::new(Vec::new())),
            stop_continuations: 0,
            truncation_retries: 0,
            role_llms: std::collections::HashMap::new(),
            irc_rx: None,
        }
    }

    /// Register this agent on the process-global IrcBus and AgentRegistry.
    /// Call after `new()` before `run()`.
    pub fn register_with_bus(&mut self) {
        use pr_core::irc::{AgentRegistry, IrcBus, PeerStatus};

        // Register on IrcBus for peer-to-peer messaging.
        self.irc_rx = Some(IrcBus::global().register(&self.id));

        // Register in AgentRegistry for discovery.
        AgentRegistry::global().register(pr_core::AgentRef {
            id: self.id.clone(),
            role: self.role,
            parent_id: self.parent_id.clone(),
            status: PeerStatus::Running,
            activity: self.task.clone(),
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
        });
    }

    /// Unregister this agent from the process-global bus and registry.
    pub fn unregister_from_bus(&self) {
        use pr_core::irc::{AgentRegistry, IrcBus};
        IrcBus::global().unregister(&self.id);
        AgentRegistry::global().unregister(&self.id);
    }

    /// Attach per-role LLM overrides (fleet E8).
    pub fn with_role_llms(
        mut self,
        map: std::collections::HashMap<String, Arc<dyn LlmProvider>>,
    ) -> Self {
        self.role_llms = map;
        self
    }

    /// Attach the session steering channel (fleet E1).
    pub fn with_steer_rx(
        mut self,
        rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<String>>>,
    ) -> Self {
        self.steer_rx = Some(rx);
        self
    }

    /// Replace the cancellation token (children derive one from the parent,
    /// coordinators pass a session-wide token).
    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel = token;
        self
    }

    /// Access to the cancellation token (for supervisors).
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Build the shared [`ToolContext`] for this run.
    ///
    /// Created once per run (not per tool call) so stateful subsystems —
    /// file locks, file history, read tracking — work across calls, and the
    /// LLM/contact/CRM attachments are available to every tool.
    async fn build_tool_context(&self) -> ToolContext {
        let mut ctx = ToolContext::new(self.working_dir.clone(), self.config.search.clone())
            .with_llm(self.llm.clone())
            .with_session_id(self.session_id.0.clone());
        // Fast auxiliary provider: inherited from the coordinator when set,
        // otherwise built lazily (covers standalone/worker runtimes).
        let fast = self
            .fast_llm
            .clone()
            .or_else(|| pr_llm::build_fast_provider(&self.config.llm).ok().flatten());
        if let Some(f) = fast {
            ctx = ctx.with_fast_llm(f);
        }
        if let Some(cache) = &self.fetch_cache {
            ctx = ctx.with_fetch_cache(cache.clone());
        }
        if let Some(cache) = &self.mx_cache {
            ctx = ctx.with_mx_cache(cache.clone());
        }
        if let Some(db) = &self.contact_db {
            ctx = ctx.with_contact_db(db.clone());
        }
        if let Some(crm) = &self.crm {
            ctx = ctx.with_crm(crm.clone());
        }
        if let Some(mem) = &self.memory {
            ctx = ctx.with_memory(mem.clone());
        }
        // Durable verification-receipt ledger: lets `verify_*` tools record
        // their conclusions and `autosave`/persistence tag contacts honestly.
        // Opened best-effort — if the home dir / ledger path is unavailable we
        // simply run without receipts for this session.
        if let Ok(ledger) = pr_tools::receipt::open_default_ledger().await {
            ctx = ctx.with_receipt_ledger(ledger);
        }
        ctx = ctx.with_agent_id(self.id.clone());
        ctx
    }

    fn emit(&self, event: AgentEvent) {
        let _ = self.event_tx.send(event);
    }

    fn emit_tool_hook_denied(&self, tool: &str) {
        tracing::warn!("tool {tool} denied by PreToolUse hook (agent {})", self.id);
    }

    fn build_system_prompt(&self, memory_digest: &str) -> String {
        let mut builder = PromptBuilder::new(
            self.role,
            &self.task,
            self.depth,
            self.config.agent.max_depth,
            &self.config.llm.model,
        );

        // Context tier: environment info
        builder.add_env(&self.config, &self.working_dir);

        // Volatile tier: tool schemas (denied tools hidden from the model)
        let tool_schemas: Vec<ToolSchema> = self
            .tools
            .list_schemas()
            .into_iter()
            .filter(|t| !self.denied_tools.contains(&t.name.to_lowercase()))
            .collect();
        builder.add_tools(&tool_schemas);

        // Volatile tier: persistent memory block (file-backed MEMORY.md)
        let memory_block = self.memory_store.to_system_prompt_block();
        if !memory_block.is_empty() {
            builder.add_volatile_block(memory_block);
        }

        // Volatile tier: long-term semantic memory digest (mem0/Memora).
        if !memory_digest.is_empty() {
            builder.add_volatile_block(memory_digest);
        }

        // Volatile tier: discovered skills
        let skill_block = self.skill_registry.to_system_prompt_block();
        if !skill_block.is_empty() {
            builder.add_volatile_block(skill_block);
        }

        // Volatile tier: active persona/profile instructions (--profile).
        if let Some(profile) = &self.profile_prompt {
            builder.add_volatile_block(profile.clone());
        }

        // Stable tier: general behavioral instructions
        builder.add_stable_instruction(
            "When you have completed your task, provide a final summary of your findings.\n\n\
             Important rules:\n\
             - Use tools to gather information, don't make things up\n\
             - Cite sources (URLs) when possible\n\
             - Be thorough but efficient\n\
             - Write findings to files when appropriate\n\
             - Use the `memory` tool to persist important facts across sessions\n\
             - Use `memory_absorb` for durable findings (verified contacts, company facts) \
             and `memory_search`/`memory_digest` to check what is already known before re-researching",
        );

        builder.build()
    }

    /// Long-term semantic memory digest for the system prompt
    /// (mem0/Memora pre-session context load). Only top-level agents get
    /// one: children receive an explicit context handoff from their parent
    /// instead, and a digest per sub-agent would multiply embedding calls.
    /// Best-effort — any failure yields an empty block.
    async fn memory_digest_block(&self) -> String {
        if self.depth != 0 || !self.config.memory.auto_digest {
            return String::new();
        }
        let Some(mem) = &self.memory else {
            return String::new();
        };
        let scope = mem.session_scope(&self.session_id.0);
        mem.digest_block(&self.task, &scope, 2500).await
    }

    /// Deterministically absorb harvested contacts into long-term memory so
    /// future sessions know them without re-harvesting. Uses the heuristic
    /// (no-LLM) pipeline to keep this hot path free and fast; secret scanning
    /// still applies, so accidental credentials never reach the store.
    async fn absorb_contacts_to_memory(&self, meta: &serde_json::Value, origin: &str) {
        if !self.config.memory.enabled {
            return;
        }
        let Some(mem) = &self.memory else {
            return;
        };

        let mut facts = Vec::new();

        // extract_contacts shape: contacts = {emails:[], phones:[], persons:[]}.
        if let Some(contacts) = meta.get("contacts") {
            if let Some(emails) = contacts.get("emails").and_then(|v| v.as_array()) {
                for e in emails {
                    let Some(email) = e.get("email").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    facts.push(pr_memory::AbsorbFact {
                        content: format!("Contact email {email} (source: {origin})"),
                        metadata: serde_json::json!({ "type": "contact" }),
                        tags: vec!["contact".to_string(), "email".to_string()],
                        confidence: Some(0.6),
                        memory_class: None,
                    });
                }
            }
            if let Some(phones) = contacts.get("phones").and_then(|v| v.as_array()) {
                for p in phones {
                    let Some(phone) = p
                        .get("normalized")
                        .and_then(|v| v.as_str())
                        .or_else(|| p.get("phone").and_then(|v| v.as_str()))
                    else {
                        continue;
                    };
                    facts.push(pr_memory::AbsorbFact {
                        content: format!("Contact phone {phone} (source: {origin})"),
                        metadata: serde_json::json!({ "type": "contact" }),
                        tags: vec!["contact".to_string(), "phone".to_string()],
                        confidence: Some(0.6),
                        memory_class: None,
                    });
                }
            }
        }

        // find_leads shape: leads = [{person:{name,role,email}, company:{name}}].
        if let Some(leads) = meta.get("leads").and_then(|v| v.as_array()) {
            for lead in leads {
                let person = lead.get("person");
                let name = person
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let role = person
                    .and_then(|p| p.get("role"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let email = person
                    .and_then(|p| p.get("email"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let company = lead
                    .get("company")
                    .and_then(|c| c.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if name.is_empty() && email.is_empty() && company.is_empty() {
                    continue;
                }
                let mut content = String::new();
                if !name.is_empty() {
                    content.push_str(name);
                    if !role.is_empty() {
                        content.push_str(&format!(" ({role})"));
                    }
                    if !company.is_empty() {
                        content.push_str(&format!(" at {company}"));
                    }
                    content.push_str(" — ");
                } else if !company.is_empty() {
                    content.push_str(&format!("{company} contact — "));
                }
                if !email.is_empty() {
                    content.push_str(&format!("email {email}"));
                }
                content.push_str(&format!(" (source: {origin})"));
                facts.push(pr_memory::AbsorbFact {
                    content,
                    metadata: serde_json::json!({ "type": "contact" }),
                    tags: vec!["contact".to_string(), "lead".to_string()],
                    confidence: Some(0.6),
                    memory_class: None,
                });
            }
        }

        if facts.is_empty() {
            return;
        }

        let req = pr_memory::AbsorbRequest {
            facts,
            source: format!("session:{}", self.session_id.0),
            scope: pr_memory::Scope::Agent,
            scope_key: String::new(),
            context: None,
            dry_run: false,
        };
        match mem.pipeline().absorb(req).await {
            Ok(report) => {
                tracing::debug!(
                    "memory: auto-absorbed contacts ({})",
                    report.summary_line()
                );
            }
            Err(e) => tracing::warn!("memory: contact absorb failed: {e}"),
        }
    }

    /// Whether a tool call must pass the operator approval gate.
    fn requires_approval(&self, tool_name: &str) -> bool {
        self.config
            .agent
            .approval_tools
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tool_name))
    }

    /// Ask the operator to approve a side-effect tool call. Blocks up to
    /// `[agent] approval_timeout_seconds`; missing operator or timeout
    /// resolves to `[agent] approval_fallback` (allow/deny).
    async fn request_approval(
        &self,
        tool_name: &str,
        tool_args: &serde_json::Value,
    ) -> crate::control::ApprovalVerdict {
        use crate::control::ApprovalVerdict;

        let fallback = if self.config.agent.approval_fallback.eq_ignore_ascii_case("deny") {
            ApprovalVerdict::Denied
        } else {
            ApprovalVerdict::Allowed
        };

        let Some(tx) = &self.approval_tx else {
            // Headless run: nobody to ask.
            tracing::info!(
                "approval[{tool_name}] agent {}: no operator connected -> {:?}",
                self.id,
                fallback
            );
            return fallback;
        };

        let request_id = uuid::Uuid::now_v7().to_string();
        let args_preview: String = serde_json::to_string(tool_args)
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect();
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if tx
            .send(crate::control::ApprovalRequest {
                agent_id: self.id.clone(),
                request_id: request_id.clone(),
                tool: tool_name.to_string(),
                args_preview: args_preview.clone(),
                reply: reply_tx,
            })
            .is_err()
        {
            tracing::warn!("approval[{tool_name}]: control channel closed -> {:?}", fallback);
            return fallback;
        }
        self.emit(AgentEvent::ApprovalRequested {
            agent_id: self.id.clone(),
            request_id,
            tool: tool_name.to_string(),
            args_preview,
        });

        let timeout = std::time::Duration::from_secs(self.config.agent.approval_timeout_seconds);
        match tokio::time::timeout(timeout, reply_rx).await {
            Ok(Ok(true)) => ApprovalVerdict::Allowed,
            Ok(Ok(false)) => ApprovalVerdict::Denied,
            Ok(Err(_)) => {
                tracing::warn!("approval[{tool_name}]: operator went away -> {:?}", fallback);
                fallback
            }
            Err(_) => {
                tracing::warn!("approval[{tool_name}]: timed out -> {:?}", fallback);
                fallback
            }
        }
    }

    /// Ask the operator a question and await the answer (up to 10 minutes).
    /// Without an operator (or on timeout) the agent is told to proceed on
    /// its own — a missing human must never deadlock the fleet.
    async fn ask_operator(&self, question: &str) -> String {
        let Some(tx) = &self.question_tx else {
            return "No operator is available to answer. Proceed using your best judgment."
                .to_string();
        };
        let request_id = uuid::Uuid::now_v7().to_string();
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if tx
            .send(crate::control::QuestionRequest {
                agent_id: self.id.clone(),
                request_id: request_id.clone(),
                question: question.to_string(),
                reply: reply_tx,
            })
            .is_err()
        {
            return "The operator channel is closed. Proceed using your best judgment."
                .to_string();
        }
        self.emit(AgentEvent::QuestionAsked {
            agent_id: self.id.clone(),
            request_id,
            question: question.to_string(),
        });
        match tokio::time::timeout(std::time::Duration::from_secs(600), reply_rx).await {
            Ok(Ok(answer)) if !answer.trim().is_empty() => answer,
            Ok(_) => "The operator did not answer. Proceed using your best judgment.".to_string(),
            Err(_) => "The operator did not answer in time. Proceed using your best judgment."
                .to_string(),
        }
    }

    /// Recalculate the estimated token count from all current messages.
    /// Only needed after bulk rewrites (compaction); appends use the
    /// incremental [`Self::track_message_tokens`] instead.
    fn recalculate_estimated_tokens(&mut self) {
        self.estimated_tokens = estimate_messages_tokens(&self.messages);
    }

    /// Add one message's tokens to the running estimate. The estimate is
    /// monotonic between compactions, so per-append full rescans (O(n^2)
    /// over a session) are pure waste.
    fn track_message_tokens(&mut self, msg: &Message) {
        self.estimated_tokens += pr_core::estimate_message_tokens(msg);
    }

    /// Run context compaction: summarise the middle of the conversation and prune
    /// old tool outputs. This mutates `self.messages` in place.
    async fn run_compaction(&mut self) {
        let tokens_before = self.estimated_tokens;
        let llm = self.llm.clone();

        let result = self
            .compaction_engine
            .compact(&mut self.messages, |prompt_messages| {
                let llm = llm.clone();
                async move {
                    let schemas = vec![];
                    let req = CompletionRequest {
                        messages: prompt_messages,
                        tools: schemas,
                        temperature: Some(0.3),
                        max_tokens: Some(2048),
                        stream: false,
                    };
                    let resp = llm.complete(&req).await?;
                    if let Message::Assistant { content, .. } = &resp.message {
                        Ok(content.clone().unwrap_or_else(|| "[No summary produced]".to_string()))
                    } else {
                        Ok("[Summarization returned non-assistant message]".to_string())
                    }
                }
            })
            .await;

        match result {
            Ok(cr) => {
                if cr.tokens_after < tokens_before {
                    tracing::info!(
                        "Context compaction: {} -> {} tokens ({}% reduction, micro_pruned={}, llm={})",
                        tokens_before,
                        cr.tokens_after,
                        if tokens_before > 0 {
                            (tokens_before - cr.tokens_after) * 100 / tokens_before
                        } else {
                            0
                        },
                        cr.micro_pruned,
                        cr.used_llm,
                    );
                    self.messages = cr.messages;
                    self.recalculate_estimated_tokens();
                } else {
                    tracing::debug!("Context compaction had no effect");
                }
            }
            Err(e) => {
                tracing::error!("Context compaction failed: {e}");
            }
        }
    }

    /// Run one LLM request with TRUE token streaming.
    ///
    /// Text deltas are forwarded as [`AgentEvent::LlmStreamChunk`] the
    /// moment they arrive (the TUI renders them live, not post-factum);
    /// tool-call fragments are reassembled by their `index`. The result is
    /// returned in the same shape as a non-streaming completion so the rest
    /// of the loop is unchanged. If the provider cannot produce a stream
    /// (or none of the deltas carried content), the call transparently
    /// falls back to `complete()` — an agent must never die because a
    /// gateway lacks SSE.
    async fn complete_streaming(
        &self,
        req: &CompletionRequest,
    ) -> pr_core::PrResult<CompletionResponse> {
        use futures::StreamExt;

        let mut stream = match self.llm.stream(req).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    "Agent {}: stream unavailable ({e}); falling back to non-streaming",
                    self.id
                );
                let mut fallback = req.clone();
                fallback.stream = false;
                return self.llm.complete(&fallback).await;
            }
        };

        let mut content = String::new();
        // Reassembly buffers keyed by delta index: (id, name, arguments).
        let mut calls: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new();
        let mut usage: Option<Usage> = None;
        let mut finish_reason: Option<String> = None;
        let mut saw_anything = false;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamChunk::Text { delta }) => {
                    if !delta.is_empty() {
                        saw_anything = true;
                        content.push_str(&delta);
                        self.emit(AgentEvent::LlmStreamChunk {
                            agent_id: self.id.clone(),
                            chunk: delta,
                        });
                    }
                }
                Ok(StreamChunk::ToolCallDelta {
                    index,
                    id,
                    name,
                    arguments_delta,
                }) => {
                    saw_anything = true;
                    let entry = calls.entry(index).or_default();
                    if !id.is_empty() {
                        entry.0 = id;
                    }
                    if !name.is_empty() {
                        entry.1 = name;
                    }
                    entry.2.push_str(&arguments_delta);
                }
                Ok(StreamChunk::Done {
                    message: _,
                    usage: u,
                    finish_reason: fr,
                }) => {
                    if u.is_some() {
                        usage = u;
                    }
                    if fr.is_some() {
                        finish_reason = fr;
                    }
                }
                Ok(StreamChunk::Error { message }) => {
                    return Err(pr_core::PrError::Llm(format!("stream error: {message}")));
                }
                Err(e) => {
                    // Mid-stream transport failure: retry the whole request
                    // non-streamed rather than losing the turn.
                    tracing::warn!(
                        "Agent {}: stream broke mid-flight ({e}); retrying non-streamed",
                        self.id
                    );
                    let mut fallback = req.clone();
                    fallback.stream = false;
                    return self.llm.complete(&fallback).await;
                }
            }
        }

        if !saw_anything {
            // Provider "streamed" nothing (some gateways do this). Retry
            // non-streamed so the turn still gets a real answer.
            tracing::warn!("Agent {}: empty stream; retrying non-streamed", self.id);
            let mut fallback = req.clone();
            fallback.stream = false;
            return self.llm.complete(&fallback).await;
        }

        let mut tool_calls: Vec<ToolCall> = Vec::new();
        for (_, (id, name, args)) in calls {
            if name.is_empty() {
                continue; // never got a tool name — unusable fragment
            }
            let parsed: serde_json::Value =
                serde_json::from_str(&args).unwrap_or(serde_json::json!({}));
            let id = if id.is_empty() {
                format!("call_{}", uuid::Uuid::now_v7())
            } else {
                id
            };
            tool_calls.push(ToolCall::new(id, name, parsed));
        }

        Ok(CompletionResponse {
            message: Message::assistant_with_tools(
                if content.is_empty() { None } else { Some(content) },
                tool_calls,
            ),
            usage,
            finish_reason,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<AgentOutput> {
        // Register on the process-global IrcBus and AgentRegistry.
        self.register_with_bus();

        // Initialize messages
        let digest = self.memory_digest_block().await;
        self.messages.push(Message::system(self.build_system_prompt(&digest)));
        self.messages.push(Message::user(&self.task));

        // Update token estimate after initial messages.
        self.recalculate_estimated_tokens();

        // Save to DB
        self.db.add_message(&self.id, &self.messages[0])?;
        self.db.add_message(&self.id, &self.messages[1])?;

        self.emit(AgentEvent::AgentStateChanged {
            id: self.id.clone(),
            state: AgentState::Researching { sub_tasks: vec![] },
        });

        let tool_schemas: Vec<ToolSchema> = self
            .tools
            .list_schemas()
            .into_iter()
            .filter(|t| !self.denied_tools.contains(&t.name.to_lowercase()))
            .collect();
        let tool_ctx = Arc::new(self.build_tool_context().await);
        let tool_executor = crate::tool_executor::ToolExecutor::new();
        let mut iterations = 0;
        let mut final_content = String::new();
        let mut doom_warning: Option<String> = None;

        'main_loop: while iterations < self.max_iterations {
            iterations += 1;

            // Steering: mid-run user instructions (fleet E1).
            if let Some(rx) = self.steer_rx.clone() {
                let mut rx = rx.lock().await;
                while let Ok(msg) = rx.try_recv() {
                    tracing::info!("Agent {} received steering instruction", self.id);
                    let steer_msg = Message::user(format!("[USER INSTRUCTION] {msg}"));
                    self.messages.push(steer_msg.clone());
                    self.db.add_message(&self.id, &steer_msg)?;
                    self.track_message_tokens(&steer_msg);
                }
            }

            // Inbox: peer-to-peer messages drained at turn boundaries.
            // Injected as system-level context so the agent can react to
            // requests from sibling agents and answer them.
            {
                let msgs: Vec<pr_core::IrcMessage> = self
                    .irc_rx
                    .as_mut()
                    .map(|rx| {
                        let mut v = Vec::new();
                        while let Ok(msg) = rx.try_recv() {
                            v.push(msg);
                        }
                        v
                    })
                    .unwrap_or_default();
                for msg in msgs {
                    tracing::info!(
                        "Agent {} received peer message from {}",
                        self.id,
                        msg.from
                    );
                    let note = Message::user(format!(
                        "[INBOX from agent {}] {}",
                        msg.from, msg.content
                    ));
                    self.messages.push(note.clone());
                    self.db.add_message(&self.id, &note)?;
                    self.track_message_tokens(&note);
                }
            }

            // Background children that finished since the last turn (fleet E2).
            {
                let mut finished = self
                    .bg_results
                    .lock()
                    .map(|mut v| std::mem::take(&mut *v))
                    .unwrap_or_default();
                for (label, res, tokens) in finished.drain(..) {
                    self.descendant_tokens += tokens;
                    let text = match res {
                        Ok(summary) => format!(
                            "[background agent {label} completed]\n{}",
                            summary.chars().take(4000).collect::<String>()
                        ),
                        Err(e) => format!("[background agent {label} failed: {e}]"),
                    };
                    let note = Message::user(text);
                    self.messages.push(note.clone());
                    self.db.add_message(&self.id, &note)?;
                    self.track_message_tokens(&note);
                }
            }

            // Cooperative cancellation (session cancel / stall kill).
            if self.cancel.is_cancelled() {
                tracing::warn!("Agent {} cancelled", self.id);
                self.emit(AgentEvent::AgentFailed {
                    id: self.id.clone(),
                    error: "cancelled".to_string(),
                });
                anyhow::bail!("agent {} cancelled", self.id);
            }

            // Per-agent token budget (session_token_limit split across the
            // batch at spawn time): stop gracefully at the turn boundary
            // instead of burning the whole session budget. The lookahead
            // uses the current context size as the next turn's prompt cost,
            // so the agent stops *before* a turn that would blow the cap
            // (one turn is atomic — this is the tightest enforceable point).
            if let Some(cap) = self.token_cap {
                let next_turn_estimate = self.tokens_used + self.estimated_tokens as u64;
                if self.tokens_used >= cap || next_turn_estimate >= cap {
                    tracing::warn!(
                        "Agent {} reached its token cap ({cap}, used {}), stopping",
                        self.id,
                        self.tokens_used
                    );
                    break 'main_loop;
                }
            }

            // Reset turn budget at the start of each LLM turn.
            self.turn_budget = TurnBudget::new(self.config.context.turn_budget_bytes);

            // ── Context compaction check ──
            self.compaction_engine.set_estimated_tokens(self.estimated_tokens);
            if self.compaction_engine.should_compact() {
                self.run_compaction().await;
            }

            // Build request. Streaming is enabled: text deltas reach the
            // TUI live via LlmStreamChunk (see complete_streaming); the
            // method falls back to a plain completion when the provider
            // cannot stream.
            let req = CompletionRequest {
                messages: self.messages.clone(),
                tools: tool_schemas.clone(),
                temperature: Some(self.config.llm.temperature),
                max_tokens: Some(self.config.llm.max_tokens),
                stream: true,
            };

            // Call LLM (true token streaming)
            let response = match self.complete_streaming(&req).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("LLM error for agent {}: {e}", self.id);
                    self.emit(AgentEvent::AgentFailed {
                        id: self.id.clone(),
                        error: e.to_string(),
                    });
                    return Err(anyhow::anyhow!("LLM error: {e}"));
                }
            };

            // Track tokens
            if let Some(usage) = &response.usage {
                self.tokens_used += usage.total_tokens as u64;
            }

            // Add assistant message
            self.messages.push(response.message.clone());
            self.db.add_message(&self.id, &response.message)?;
            self.track_message_tokens(&response.message);

            // Extract content and tool calls. Note: text already reached the
            // UI incrementally via complete_streaming — no re-emit here.
            if let Message::Assistant { content, tool_calls } = &response.message {
                if let Some(text) = content {
                    if !text.is_empty() {
                        final_content = text.clone();
                    }
                }

                // If no tool calls, the model wants to stop — Stop hooks get
                // a veto (fleet E3), bounded by MAX_STOP_CONTINUATIONS.
                if tool_calls.is_empty() {
                    // Reasoning-model truncation guard: empty content plus
                    // finish_reason "length" means chain-of-thought consumed
                    // the whole output budget and no answer was produced.
                    // Nudge the model for a direct answer instead of silently
                    // stopping with nothing.
                    let content_empty = content
                        .as_deref()
                        .map(|c| c.trim().is_empty())
                        .unwrap_or(true);
                    if content_empty
                        && response.finish_reason.as_deref() == Some("length")
                        && self.truncation_retries < MAX_TRUNCATION_RETRIES
                    {
                        self.truncation_retries += 1;
                        tracing::warn!(
                            "Agent {} produced no content (finish_reason=length, \
                             reasoning ate the budget); nudging ({}/{})",
                            self.id,
                            self.truncation_retries,
                            MAX_TRUNCATION_RETRIES
                        );
                        let nudge = Message::user(
                            "Your previous response was cut off before any answer \
                             was produced (output budget exhausted). Respond NOW \
                             with a concise final answer based on what you already \
                             have. Keep reasoning minimal — deliver the answer directly."
                                .to_string(),
                        );
                        self.messages.push(nudge.clone());
                        self.db.add_message(&self.id, &nudge)?;
                        self.track_message_tokens(&nudge);
                        continue 'main_loop;
                    }

                    let summary_so_far = final_content.clone();
                    let verdict = if self.stop_continuations
                        < crate::hooks::MAX_STOP_CONTINUATIONS
                    {
                        crate::hooks::run_stop_hooks(&self.config.hooks, &summary_so_far).await
                    } else {
                        crate::hooks::StopVerdict::Stop
                    };
                    match verdict {
                        crate::hooks::StopVerdict::Continue(reason) => {
                            self.stop_continuations += 1;
                            tracing::info!(
                                "Stop hook asked agent {} to continue ({}/{})",
                                self.id,
                                self.stop_continuations,
                                crate::hooks::MAX_STOP_CONTINUATIONS
                            );
                            let cont = Message::user(format!(
                                "[hook] Do not stop yet: {reason}"
                            ));
                            self.messages.push(cont.clone());
                            self.db.add_message(&self.id, &cont)?;
                            self.track_message_tokens(&cont);
                            continue 'main_loop;
                        }
                        crate::hooks::StopVerdict::Stop => break,
                    }
                }

                // ── Execute tool calls (parallel batch pipeline, fleet P2.8) ──
                // Independent read-only tools run CONCURRENTLY so research
                // turns that request several searches/fetches/verifications
                // at once do not pay their latency sequentially. Three phases:
                //   1. Pre-pass (sequential, cheap): doom-loop detection,
                //      role permissions, PreToolUse hooks.
                //   2. Execution: ToolExecutor runs parallel-safe tools via
                //      join_all and the rest one at a time (path-overlap
                //      detection included).
                //   3. Post-pass (sequential, original order): shell cascade,
                //      spawn interception, PostToolUse hooks, contact
                //      autosave, findings, persistence, truncation.
                let mut prepared: Vec<PreparedCall> = Vec::with_capacity(tool_calls.len());
                let mut doom_nudged_this_turn = false;

                for (idx, tool_call) in tool_calls.iter().enumerate() {
                    let tool_name = tool_call.name();
                    let tool_args = tool_call.arguments();

                    // ── Doom loop detection (nudge, then stop) ──
                    // First offense: tell the model to change strategy and let
                    // it keep its accumulated findings. Second offense: stop.
                    if self.doom_loop.record_and_check(tool_name, &tool_args) {
                        let remaining: Vec<String> = tool_calls[idx..]
                            .iter()
                            .map(|tc| tc.id.clone())
                            .collect();

                        if !self.doom_nudged {
                            self.doom_nudged = true;
                            let nudge = format!(
                                "Repeated identical '{tool_name}' call detected. Do NOT repeat \
                                 it — try different arguments, a different tool/source, or finish \
                                 with the results you already have."
                            );
                            tracing::warn!("Agent {} nudged after repeated call: {tool_name}", self.id);
                            for (i, call_id) in remaining.iter().enumerate() {
                                let msg = if i == 0 {
                                    nudge.clone()
                                } else {
                                    format!("Cancelled: {nudge}")
                                };
                                let tool_msg = Message::tool(call_id, &msg);
                                self.messages.push(tool_msg.clone());
                                self.db.add_message(&self.id, &tool_msg)?;
                                self.track_message_tokens(&tool_msg);
                            }
                            doom_nudged_this_turn = true;
                            break; // back to the LLM for a strategy change
                        }

                        let warning = format!(
                            "Doom loop detected: tool '{tool_name}' invoked repeatedly with \
                             identical arguments even after a warning. Stopping agent."
                        );
                        tracing::warn!("Agent {} stopped: {warning}", self.id);
                        self.emit(AgentEvent::AgentFailed {
                            id: self.id.clone(),
                            error: warning.clone(),
                        });

                        // Answer the current and any remaining sibling tool calls
                        // so the message history stays consistent.
                        for (i, call_id) in remaining.iter().enumerate() {
                            let msg = if i == 0 {
                                warning.clone()
                            } else {
                                format!("Cancelled: {warning}")
                            };
                            let tool_msg = Message::tool(call_id, &msg);
                            self.messages.push(tool_msg.clone());
                            self.db.add_message(&self.id, &tool_msg)?;
                            self.track_message_tokens(&tool_msg);
                        }
                        // Spawn requests are collected only AFTER the pre-pass
                        // completes, so none can be pending here — the doom
                        // check happens before any execution in the turn.

                        // Keep whatever was already produced; only fall back to
                        // the warning when there is nothing else.
                        if final_content.is_empty() {
                            final_content = warning.clone();
                        }
                        doom_warning = Some(warning);
                        break 'main_loop;
                    }

                    self.emit(AgentEvent::ToolCallStarted {
                        agent_id: self.id.clone(),
                        tool: tool_name.to_string(),
                        args: tool_args.clone(),
                    });

                    // Role permission gate (fleet E5) — rejected before any
                    // execution; the error is delivered in the post-pass so
                    // message order matches the request order.
                    if self.denied_tools.contains(&tool_name.to_lowercase()) {
                        let output = ToolOutput::err_code(
                            format!(
                                "Permission denied: role {:?} is not allowed to use '{tool_name}'",
                                self.role
                            ),
                            "permission_denied",
                        );
                        prepared.push(PreparedCall::Immediate(tool_call.clone(), output));
                        continue;
                    }

                    // Operator approval gate for side-effect tools. Denials
                    // surface to the model as a normal tool error so it can
                    // adapt (ask differently, skip, ...).
                    if self.requires_approval(tool_name) {
                        let verdict = self.request_approval(tool_name, &tool_args).await;
                        if verdict == crate::control::ApprovalVerdict::Denied {
                            let output = ToolOutput::err_code(
                                format!(
                                    "Denied by operator approval: '{tool_name}' was not allowed to run. \
                                     Do not retry the same call; adapt or ask why if needed."
                                ),
                                "approval_denied",
                            );
                            prepared.push(PreparedCall::Immediate(tool_call.clone(), output));
                            continue;
                        }
                    }

                    // PreToolUse hooks can veto the call (fleet E3).
                    match crate::hooks::run_pre_tool_hooks(
                        &self.config.hooks,
                        tool_name,
                        &tool_args,
                    )
                    .await
                    {
                        crate::hooks::PreToolVerdict::Deny(reason) => {
                            let output = ToolOutput::err_code(
                                format!("Denied by hook: {reason}"),
                                "hook_denied",
                            );
                            self.emit_tool_hook_denied(tool_name);
                            prepared.push(PreparedCall::Immediate(tool_call.clone(), output));
                        }
                        crate::hooks::PreToolVerdict::Allow => {
                            prepared.push(PreparedCall::Exec(tool_call.clone()));
                        }
                    }
                }

                if doom_nudged_this_turn {
                    continue 'main_loop;
                }

                // ── Phase 2: execution ──
                // The ToolExecutor partitions calls into parallel-safe and
                // sequential groups, serializes overlapping file paths and
                // returns results tagged with durations. Parallel-safe calls
                // are spawned as independent tasks (execute_batch_spawn) so
                // CPU-bound tools overlap on multiple worker threads, not
                // just network awaits on one thread.
                let exec_calls: Vec<ToolCall> = prepared
                    .iter()
                    .filter_map(|p| match p {
                        PreparedCall::Exec(tc) => Some(tc.clone()),
                        PreparedCall::Immediate(..) => None,
                    })
                    .collect();
                let mut batch_results: std::collections::HashMap<
                    String,
                    crate::tool_executor::ToolBatchResult,
                > = tool_executor
                    .execute_batch_spawn(exec_calls, self.tools.clone(), tool_ctx.clone())
                    .await
                    .into_iter()
                    .map(|r| (r.tool_call.id.clone(), r))
                    .collect();

                // ── Phase 3: post-processing in original call order ──
                // Cascading cancellation keeps its original-order semantics:
                // a failed shell cancels every sibling AFTER it in the batch.
                let mut shell_failed: Option<String> = None;
                // spawn_agent requests are collected during the pass and run
                // CONCURRENTLY afterwards (fleet D4).
                let mut pending_spawns: Vec<(String, serde_json::Value)> = Vec::new();

                for item in prepared {
                    let (tool_call, mut result, mut duration) = match item {
                        PreparedCall::Immediate(tc, out) => (tc, out, 0u64),
                        PreparedCall::Exec(tc) => match batch_results.remove(&tc.id) {
                            Some(r) => (tc, r.output, r.duration_ms),
                            None => (tc, ToolOutput::err("Tool was not executed"), 0u64),
                        },
                    };
                    let tool_name = tool_call.name();
                    let tool_args = tool_call.arguments();

                    let mut cascaded = false;
                    if let Some(ref shell_err) = shell_failed {
                        let err_msg = format!(
                            "Cancelled: sibling shell tool failed with: {}",
                            shell_err
                        );
                        let mut output = ToolOutput::err(&err_msg);
                        output.metadata = Some(serde_json::json!({
                            "is_error": true,
                            "cascade_cancelled": true,
                            "reason": shell_err,
                        }));
                        result = output;
                        duration = 0;
                        cascaded = true;
                    }

                    // Track shell failures for cascading (a cascade-cancelled
                    // shell keeps the ORIGINAL failure reason).
                    if !cascaded && tool_name == "shell" && !result.success {
                        shell_failed = Some(result.content.clone());
                    }

                    // ── Sub-agent delegation ──
                    // `spawn_agent` returns a marker; the child is prepared and
                    // run concurrently with sibling spawns after this pass
                    // (fleet D4). The tool message is emitted once the child
                    // completes.
                    if tool_name == "spawn_agent"
                        && result
                            .metadata
                            .as_ref()
                            .and_then(|m| m.get("spawn_request"))
                            .and_then(|v| v.as_bool())
                            == Some(true)
                    {
                        let meta = result.metadata.clone().unwrap_or_default();
                        pending_spawns.push((tool_call.id.clone(), meta));
                        continue;
                    }

                    // ── Operator question (control plane) ──
                    // `question` returns a marker; the runtime blocks until
                    // the operator answers (or the timeout tells the agent
                    // to proceed alone) and delivers the answer as the tool
                    // result.
                    if tool_name == "question"
                        && result
                            .metadata
                            .as_ref()
                            .and_then(|m| m.get("question_request"))
                            .and_then(|v| v.as_bool())
                            == Some(true)
                    {
                        let question = result
                            .metadata
                            .as_ref()
                            .and_then(|m| m.get("question"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("(empty question)")
                            .to_string();
                        let answer = self.ask_operator(&question).await;
                        result = ToolOutput::ok(format!("Operator answer: {answer}"));
                    }

                    // PostToolUse hooks may append context (fleet E3).
                    let result = if let Some(extra) = crate::hooks::run_post_tool_hooks(
                        &self.config.hooks,
                        tool_name,
                        &tool_args,
                        &result.content,
                        result.success,
                    )
                    .await
                    {
                        let mut r = result.clone();
                        r.content.push_str(&extra);
                        r
                    } else {
                        result
                    };

                    // ── Deterministic contact persistence (fleet C1) ──
                    // Harvested contacts must reach the database even if the
                    // model forgets to call save_contacts.
                    let mut result = result;
                    if result.success
                        && matches!(tool_name, "extract_contacts" | "find_leads")
                    {
                        if let Some(db) = tool_ctx.contact_db.clone() {
                            let meta = result.metadata.clone().unwrap_or_default();
                            let auto = match (tool_name, meta.get("contacts"), meta.get("leads")) {
                                ("extract_contacts", Some(c), _) => {
                                    let origin = tool_args
                                        .get("url")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("inline")
                                        .to_string();
                                    Some(
                                        pr_tools::autosave::autosave_extracted(&db, c, &origin)
                                            .await,
                                    )
                                }
                                ("find_leads", _, Some(l)) => {
                                    Some(pr_tools::autosave::autosave_leads(&db, l).await)
                                }
                                _ => None,
                            };
                            if let Some(saved) = auto {
                                if saved.saved + saved.merged > 0 {
                                    result.content.push_str(&format!(
                                        "\n\n[auto-persisted {} contact(s) to the database: {} new, {} merged]",
                                        saved.saved + saved.merged,
                                        saved.saved,
                                        saved.merged
                                    ));
                                }
                                if let Some(m) = result.metadata.as_mut() {
                                    m["auto_saved"] = serde_json::json!({
                                        "saved": saved.saved,
                                        "merged": saved.merged,
                                        "failed": saved.failed,
                                    });
                                }
                            }
                        }
                    }

                    // ── Long-term memory absorb of harvested contacts ──
                    // Persisted contacts also flow into semantic memory so
                    // future sessions know them without re-harvesting.
                    if result.success
                        && matches!(tool_name, "extract_contacts" | "find_leads")
                    {
                        let meta = result.metadata.clone().unwrap_or_default();
                        let origin = tool_args
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("inline")
                            .to_string();
                        self.absorb_contacts_to_memory(&meta, &origin).await;
                    }

                    // ── Structured findings (fleet C4) ──
                    if let Some(finding) =
                        self.harvest_finding(tool_name, &tool_args, &result)
                    {
                        self.harvested_findings.push(finding);
                    }

                    self.emit(AgentEvent::ToolCallCompleted {
                        agent_id: self.id.clone(),
                        tool: tool_name.to_string(),
                        result_preview: result.content.chars().take(200).collect(),
                        duration_ms: duration,
                    });

                    // Save tool result (full, before truncation).
                    let _ = self.db.add_tool_result(
                        &self.id, tool_name, &tool_args, &result, duration,
                    );

                    // ── Apply truncation + turn budget ──
                    let truncated = apply_turn_budget(
                        tool_name,
                        &result,
                        self.config.context.tool_output_max_bytes,
                        self.config.context.tool_output_max_lines,
                        &mut self.turn_budget,
                        &self.working_dir,
                    )?;

                    let tool_content = match &truncated {
                        Truncated::Unchanged(o) => o.content.clone(),
                        Truncated::Truncated { replacement, .. } => replacement.content.clone(),
                    };

                    // Add tool result to messages
                    let tool_msg = Message::tool(&tool_call.id, &tool_content);
                    self.messages.push(tool_msg.clone());
                    self.db.add_message(&self.id, &tool_msg)?;
                    self.track_message_tokens(&tool_msg);
                }

                // ── Run collected spawn requests concurrently (fleet D4) ──
                if !pending_spawns.is_empty() {
                    self.run_spawn_batch(&mut pending_spawns).await?;
                }
            } else {
                break;
            }
        }

        if iterations >= self.max_iterations && doom_warning.is_none() {
            tracing::warn!("Agent {} hit max iterations ({})", self.id, self.max_iterations);
        }

        if doom_warning.is_none() {
            self.emit(AgentEvent::AgentStateChanged {
                id: self.id.clone(),
                state: AgentState::Complete,
            });
        }

        // Unregister from process-global bus and registry.
        self.unregister_from_bus();

        Ok(AgentOutput {
            agent_id: self.id.clone(),
            summary: final_content,
            tokens_used: self.tokens_used,
            descendant_tokens: self.descendant_tokens,
            findings: std::mem::take(&mut self.harvested_findings),
            aborted: doom_warning.is_some(),
        })
    }

    /// Validate a spawn request, persist the child agent row, emit the
    /// spawn event and build the child runtime. Depth limits are enforced
    /// here — the runtime knows its own depth.
    fn prepare_child(
        &mut self,
        meta: &serde_json::Value,
    ) -> anyhow::Result<(AgentId, AgentRuntime)> {
        // Role gate (pr-core AgentRole::can_spawn_children): verifiers and
        // writers validate/produce — they must not grow the agent tree.
        if !self.role.can_spawn_children() {
            anyhow::bail!(
                "cannot spawn: role '{}' is not allowed to create sub-agents",
                self.role
            );
        }

        // Token-cap gate: a parent whose budget is already spent must not
        // grow the tree further.
        if let Some(cap) = self.token_cap {
            if self.tokens_used >= cap {
                anyhow::bail!(
                    "cannot spawn: token budget exhausted ({}/{} tokens used)",
                    self.tokens_used,
                    cap
                );
            }
        }

        let child_depth = self.depth + 1;
        if child_depth > self.config.agent.max_depth {
            anyhow::bail!(
                "cannot spawn: max depth {} reached (current depth {})",
                self.config.agent.max_depth,
                self.depth
            );
        }

        let task = meta
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if task.is_empty() {
            anyhow::bail!("spawn_agent request has an empty task");
        }
        let role_str = meta
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("researcher")
            .to_string();
        let role = match role_str.as_str() {
            "analyst" => AgentRole::Analyst,
            "verifier" => AgentRole::Verifier,
            "writer" => AgentRole::Writer,
            _ => AgentRole::Researcher,
        };
        let context: Vec<String> = meta
            .get("context")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let agent_id = AgentId::new();
        self.db.create_agent(&AgentRecord {
            id: agent_id.clone(),
            session_id: self.session_id.0.clone(),
            parent_id: Some(self.id.clone()),
            role,
            task: task.clone(),
            status: AgentStatus::Spawned,
            depth: child_depth,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        })?;
        self.emit(AgentEvent::AgentSpawned {
            id: agent_id.clone(),
            parent: Some(self.id.clone()),
            role: role_str,
            task: task.clone(),
            depth: child_depth,
        });

        // Explicit context handoff: the child sees only its task plus the
        // facts the parent chose to pass — nothing is inherited implicitly.
        let mut full_task = task;
        if !context.is_empty() {
            let bullets = context
                .iter()
                .map(|c| format!("- {c}"))
                .collect::<Vec<_>>()
                .join("\n");
            full_task = format!("{full_task}\n\n## Context from parent agent\n{bullets}");
        }

        // Role-specific model override (fleet E8), else inherit.
        let role_key = match role {
            AgentRole::Coordinator => "coordinator",
            AgentRole::Researcher => "researcher",
            AgentRole::Analyst => "analyst",
            AgentRole::Verifier => "verifier",
            AgentRole::Writer => "writer",
        };
        let child_llm = self
            .role_llms
            .get(role_key)
            .cloned()
            .unwrap_or_else(|| self.llm.clone());

        let mut child = AgentRuntime::new(
            agent_id.clone(),
            self.session_id.clone(),
            Some(self.id.clone()),
            role,
            full_task,
            child_depth,
            child_llm,
            self.tools.clone(),
            self.event_tx.clone(),
            self.db.clone(),
            self.working_dir.clone(),
            self.config.clone(),
        );
        child.contact_db = self.contact_db.clone();
        child.crm = self.crm.clone();
        child.memory = self.memory.clone();
        child.fast_llm = self.fast_llm.clone();
        child.fetch_cache = self.fetch_cache.clone();
        child.mx_cache = self.mx_cache.clone();
        child.profile_prompt = self.profile_prompt.clone();
        // Children inherit what is left of the parent's token cap.
        child.token_cap = self.token_cap.map(|cap| cap.saturating_sub(self.tokens_used));
        child.question_tx = self.question_tx.clone();
        child.approval_tx = self.approval_tx.clone();
        // Cancelling the parent cancels the child (and its subtree).
        let child = child.with_cancel_token(self.cancel.child_token());
        Ok((agent_id, child))
    }

    /// Run a batch of collected spawn requests CONCURRENTLY — bounded by
    /// `config.agent.max_concurrent_children` (fleet D4; previously batched
    /// spawns ran strictly one after another) — and inject each child's
    /// budget-capped summary as the corresponding tool result.
    async fn run_spawn_batch(
        &mut self,
        pending: &mut Vec<(String, serde_json::Value)>,
    ) -> anyhow::Result<()> {
        use futures::stream::StreamExt;

        let width = (self.config.agent.max_concurrent_children as usize).max(1);
        let timeout_secs = self.config.agent.timeout_seconds;
        let headroom_chars = ((self.config.context.resolved_window() as u64).saturating_mul(4))
            .saturating_sub((self.estimated_tokens as u64).saturating_mul(4))
            as usize;
        let spill_dir = self.working_dir.join(".pr-context").join("spills");
        let batch_len = pending.len().max(1);

        let db = self.db.clone();
        let tx = self.event_tx.clone();

        // Prepare children up-front (depth checks, DB rows, spawn events).
        // Background children (fleet E2) detach immediately: their results
        // are injected as notices on a later turn.
        let mut items: Vec<(String, AgentId, AgentRuntime)> = Vec::new();
        let mut early_fails: Vec<(String, String)> = Vec::new();
        let mut bg_launched: Vec<(String, String)> = Vec::new();
        for (call_id, meta) in pending.drain(..) {
            let is_background = meta
                .get("background")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            match self.prepare_child(&meta) {
                Ok((aid, child)) => {
                    if is_background {
                        let label = aid.0.clone();
                        let fut = child_wait_future(
                            child,
                            aid.clone(),
                            db.clone(),
                            tx.clone(),
                            timeout_secs,
                            headroom_chars,
                            1,
                            spill_dir.clone(),
                        );
                        let slot = self.bg_results.clone();
                        tokio::spawn(async move {
                            // Keep the subtree token count — it feeds the
                            // session budget accounting (fleet round 2).
                            let (res, tokens) = match fut.await {
                                Ok((summary, tokens)) => (Ok(summary), tokens),
                                Err(e) => (Err(e.to_string()), 0),
                            };
                            if let Ok(mut v) = slot.lock() {
                                v.push((label, res, tokens));
                            }
                        });
                        bg_launched.push((call_id, aid.0.clone()));
                    } else {
                        items.push((call_id, aid, child));
                    }
                }
                Err(e) => early_fails.push((call_id, e.to_string())),
            }
        }

        // Immediate acknowledgement for background launches.
        for (call_id, label) in bg_launched {
            let out = ToolOutput::ok(format!(
                "Background agent {label} launched. Continue working — its results will be delivered automatically when it finishes."
            ));
            self.record_spawn_result(&call_id, &out)?;
        }

        // Owned per-child wait futures; `buffered` keeps at most `width`
        // children running at once.
        let futs = items.into_iter().map(|(call_id, agent_id, child)| {
            let fut = child_wait_future(
                child,
                agent_id,
                db.clone(),
                tx.clone(),
                timeout_secs,
                headroom_chars,
                batch_len,
                spill_dir.clone(),
            );
            async move { (call_id, fut.await) }
        });

        let results: Vec<(String, anyhow::Result<(String, u64)>)> =
            futures::stream::iter(futs).buffered(width).collect().await;

        // Depth/argument refusals surface as ordinary tool errors.
        for (call_id, err) in early_fails {
            let output = ToolOutput::err(format!("Sub-agent failed: {err}"));
            self.record_spawn_result(&call_id, &output)?;
        }
        for (call_id, res) in results {
            let output = match &res {
                Ok((summary, tokens)) => {
                    self.descendant_tokens += tokens;
                    ToolOutput::ok(summary.clone())
                }
                Err(e) => ToolOutput::err(format!("Sub-agent failed: {e}")),
            };
            self.record_spawn_result(&call_id, &output)?;
        }
        Ok(())
    }

    /// Persist + inject one child result as the spawn_agent tool message.
    fn record_spawn_result(&mut self, call_id: &str, output: &ToolOutput) -> anyhow::Result<()> {
        let _ = self.db.add_tool_result(
            &self.id,
            "spawn_agent",
            &serde_json::json!({}),
            output,
            0,
        );
        let truncated = apply_turn_budget(
            "spawn_agent",
            output,
            self.config.context.tool_output_max_bytes,
            self.config.context.tool_output_max_lines,
            &mut self.turn_budget,
            &self.working_dir,
        )?;
        let content = match &truncated {
            Truncated::Unchanged(o) => o.content.clone(),
            Truncated::Truncated { replacement, .. } => replacement.content.clone(),
        };
        let tool_msg = Message::tool(call_id, &content);
        self.messages.push(tool_msg.clone());
        self.db.add_message(&self.id, &tool_msg)?;
        self.track_message_tokens(&tool_msg);
        Ok(())
    }
}

/// Owned wait-future for one child agent (fleet D4). Lives outside the
/// `async fn` chain so the `run -> run_spawn_batch -> run` Send obligation
/// resolves through this boxed boundary.
fn child_wait_future(
    mut child: AgentRuntime,
    agent_id: AgentId,
    db: Arc<Persistence>,
    tx: broadcast::Sender<AgentEvent>,
    timeout_secs: u64,
    headroom_chars: usize,
    batch_len: usize,
    spill_dir: std::path::PathBuf,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<(String, u64)>> + Send>>
{
    Box::pin(async move {
        let run_res = if timeout_secs > 0 {
            match tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs),
                child.run(),
            )
            .await
            {
                Ok(res) => res,
                Err(_) => Err(anyhow::anyhow!(
                    "sub-agent timed out after {timeout_secs}s"
                )),
            }
        } else {
            child.run().await
        };
        match run_res {
            Ok(output) if output.aborted => {
                let _ = db.update_agent_status(
                    &agent_id,
                    AgentStatus::Failed,
                    output.tokens_used,
                    Some(&output.summary),
                );
                let _ = tx.send(AgentEvent::AgentFailed {
                    id: agent_id,
                    error: output.summary.clone(),
                });
                Err(anyhow::anyhow!("sub-agent was aborted: {}", output.summary))
            }
            Ok(output) => {
                let total = output.tokens_used + output.descendant_tokens;
                let _ = db.update_agent_status(
                    &agent_id,
                    AgentStatus::Completed,
                    output.tokens_used,
                    Some(&output.summary),
                );
                let _ = tx.send(AgentEvent::AgentCompleted {
                    id: agent_id,
                    summary: output.summary.chars().take(200).collect(),
                    tokens_used: output.tokens_used,
                });
                let budget =
                    crate::budget::ResultBudget::new(headroom_chars, batch_len, spill_dir);
                Ok((budget.cap_result(&output.summary).summary, total))
            }
            Err(e) => {
                let _ = db.update_agent_status(
                    &agent_id,
                    AgentStatus::Failed,
                    0,
                    Some(&e.to_string()),
                );
                let _ = tx.send(AgentEvent::AgentFailed {
                    id: agent_id,
                    error: e.to_string(),
                });
                Err(e)
            }
        }
    })
}

impl AgentRuntime {
    /// Convert structured tool metadata into a persisted [`Finding`]
    /// (fleet C4). Only extraction/harvesting tools produce findings —
    /// cross-agent knowledge should be data, not prose.
    fn harvest_finding(
        &self,
        tool_name: &str,
        tool_args: &serde_json::Value,
        result: &ToolOutput,
    ) -> Option<Finding> {
        if !result.success {
            return None;
        }
        let meta = result.metadata.as_ref()?;
        match tool_name {
            "extract_contacts" => {
                let counts = meta.get("counts")?;
                let origin = tool_args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("inline text")
                    .to_string();
                let mut sources = Vec::new();
                if origin.starts_with("http") {
                    sources.push(pr_core::Source {
                        url: origin.clone(),
                        title: origin.clone(),
                        excerpt: String::new(),
                    });
                }
                Some(Finding {
                    id: FindingId::new(),
                    agent_id: self.id.clone(),
                    title: format!("Contacts extracted from {origin}"),
                    content: format!(
                        "emails: {}, phones: {}, social profiles: {}, persons: {}, companies: {}",
                        counts.get("emails").and_then(|v| v.as_u64()).unwrap_or(0),
                        counts.get("phones").and_then(|v| v.as_u64()).unwrap_or(0),
                        counts.get("social_profiles").and_then(|v| v.as_u64()).unwrap_or(0),
                        counts.get("persons").and_then(|v| v.as_u64()).unwrap_or(0),
                        counts.get("companies").and_then(|v| v.as_u64()).unwrap_or(0),
                    ),
                    sources,
                    confidence: 0.7,
                    created_at: chrono::Utc::now(),
                })
            }
            "find_leads" => {
                let leads = meta.get("leads")?.as_array()?;
                let count = leads.len();
                if count == 0 {
                    return None;
                }
                let top: Vec<String> = leads
                    .iter()
                    .take(5)
                    .filter_map(|l| {
                        let name = l.get("person")?.get("name")?.as_str()?;
                        let company = l
                            .get("company")
                            .and_then(|c| c.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        Some(format!("- {name} @ {company}"))
                    })
                    .collect();
                Some(Finding {
                    id: FindingId::new(),
                    agent_id: self.id.clone(),
                    title: format!("Leads harvested: {count}"),
                    content: top.join("\n"),
                    sources: Vec::new(),
                    confidence: 0.6,
                    created_at: chrono::Utc::now(),
                })
            }
            "web_search" => {
                let sources_meta = meta.get("sources")?.as_array()?;
                if sources_meta.is_empty() {
                    return None;
                }
                let query = meta
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let sources: Vec<pr_core::Source> = sources_meta
                    .iter()
                    .take(10)
                    .filter_map(|s| {
                        Some(pr_core::Source {
                            url: s.get("url")?.as_str()?.to_string(),
                            title: s
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            excerpt: s
                                .get("excerpt")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .chars()
                                .take(200)
                                .collect(),
                        })
                    })
                    .collect();
                if sources.is_empty() {
                    return None;
                }
                Some(Finding {
                    id: FindingId::new(),
                    agent_id: self.id.clone(),
                    title: format!("Web search: {query}"),
                    content: format!("{} results collected", sources.len()),
                    sources,
                    confidence: 0.5,
                    created_at: chrono::Utc::now(),
                })
            }
            "web_fetch" => {
                let url = meta.get("url")?.as_str()?.to_string();
                let title_raw = meta
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let title = if title_raw.trim().is_empty() {
                    url.clone()
                } else {
                    title_raw.trim().to_string()
                };
                Some(Finding {
                    id: FindingId::new(),
                    agent_id: self.id.clone(),
                    title: format!("Page fetched: {title}"),
                    content: url.clone(),
                    sources: vec![pr_core::Source {
                        url,
                        title,
                        excerpt: String::new(),
                    }],
                    confidence: 0.6,
                    created_at: chrono::Utc::now(),
                })
            }
            "parse_html" => {
                let source = meta.get("source")?.as_str()?.to_string();
                if !source.starts_with("http://") && !source.starts_with("https://") {
                    return None;
                }
                let title_raw = meta
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let title = if title_raw.trim().is_empty() {
                    source.clone()
                } else {
                    title_raw.trim().to_string()
                };
                Some(Finding {
                    id: FindingId::new(),
                    agent_id: self.id.clone(),
                    title: format!("Page parsed: {title}"),
                    content: source.clone(),
                    sources: vec![pr_core::Source {
                        url: source,
                        title,
                        excerpt: String::new(),
                    }],
                    confidence: 0.6,
                    created_at: chrono::Utc::now(),
                })
            }
            // OSINT/news search tools serialize a `results` array into
            // metadata; harvest whatever carries a URL as a source.
            "search_news" | "search_social" | "search_business_directory" => {
                let results = meta.get("results")?.as_array()?;
                if results.is_empty() {
                    return None;
                }
                let sources: Vec<pr_core::Source> = results
                    .iter()
                    .take(10)
                    .filter_map(|r| {
                        let url = r
                            .get("url")
                            .or_else(|| r.get("profile_url"))
                            .or_else(|| r.get("website"))
                            .and_then(|v| v.as_str())?
                            .to_string();
                        let title = r
                            .get("title")
                            .or_else(|| r.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(&url)
                            .to_string();
                        Some(pr_core::Source {
                            url,
                            title,
                            excerpt: String::new(),
                        })
                    })
                    .collect();
                if sources.is_empty() {
                    return None;
                }
                Some(Finding {
                    id: FindingId::new(),
                    agent_id: self.id.clone(),
                    title: format!("{} results: {}", tool_name, sources.len()),
                    content: String::new(),
                    sources,
                    confidence: 0.5,
                    created_at: chrono::Utc::now(),
                })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentOutput {
    pub agent_id: AgentId,
    pub summary: String,
    pub tokens_used: u64,
    /// Tokens consumed by all descendants spawned via `spawn_agent`
    /// (kept separate so events/DB rows never double-count).
    pub descendant_tokens: u64,
    pub findings: Vec<Finding>,
    /// True when the run was stopped by the doom-loop detector. The summary
    /// then contains the warning, not research results — callers must record
    /// the agent as failed instead of completed.
    pub aborted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures::Stream;
    use pr_core::{PrError, PrResult};
    use pr_llm::{CompletionRequest, CompletionResponse, StreamChunk, Usage};
    use std::collections::VecDeque;
    use tokio::sync::Mutex;

    /// Scripted LLM: returns queued responses in order, then a default.
    struct MockProvider {
        responses: Mutex<VecDeque<CompletionResponse>>,
    }

    impl MockProvider {
        fn new(responses: Vec<CompletionResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
            }
        }

        fn text(s: &str) -> CompletionResponse {
            CompletionResponse {
                message: Message::assistant(s),
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 5,
                    total_tokens: 10,
                }),
                finish_reason: Some("stop".to_string()),
            }
        }

        /// Reasoning-model truncation: empty content, budget eaten by
        /// chain-of-thought, finish_reason "length".
        fn truncated() -> CompletionResponse {
            CompletionResponse {
                message: Message::assistant(""),
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 100,
                    total_tokens: 105,
                }),
                finish_reason: Some("length".to_string()),
            }
        }

        fn spawn_call(task: &str) -> CompletionResponse {
            CompletionResponse {
                message: Message::assistant_with_tools(
                    None,
                    vec![pr_core::ToolCall::new(
                        "call_spawn",
                        "spawn_agent",
                        serde_json::json!({
                            "task": task,
                            "role": "researcher",
                            "context": ["fact from parent"]
                        }),
                    )],
                ),
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 5,
                    total_tokens: 10,
                }),
                finish_reason: Some("tool_calls".to_string()),
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
            Ok(q.pop_front().unwrap_or_else(|| Self::text("default")))
        }
        async fn stream(
            &self,
            _req: &CompletionRequest,
        ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
            Err(PrError::Llm("stream unused".into()))
        }
    }

    fn make_runtime(
        llm: Arc<dyn LlmProvider>,
        db: Arc<Persistence>,
        config: AppConfig,
        depth: u32,
    ) -> AgentRuntime {
        let (event_tx, _) = broadcast::channel(64);
        AgentRuntime::new(
            AgentId::new(),
            SessionId::new(),
            None,
            AgentRole::Researcher,
            "parent task".to_string(),
            depth,
            llm,
            Arc::new(pr_tools::ToolRegistry::with_builtins()),
            event_tx,
            db,
            std::env::temp_dir(),
            config,
        )
    }


    /// Register the parent agent row the coordinator would normally create
    /// (messages have a FK to agents).
    fn register_parent(db: &Persistence, agent: &AgentRuntime) {
        db.create_agent(&AgentRecord {
            id: agent.id.clone(),
            session_id: agent.session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: agent.task.clone(),
            status: AgentStatus::Spawned,
            depth: agent.depth,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        })
        .unwrap();
    }

    #[tokio::test]
    async fn test_spawn_agent_runs_child_and_injects_summary() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        let llm = Arc::new(MockProvider::new(vec![
            // Parent turn 1: delegate via spawn_agent.
            MockProvider::spawn_call("child task"),
            // Child turn 1: final answer (no tools).
            MockProvider::text("child findings"),
            // Parent turn 2: done.
            MockProvider::text("final answer"),
        ]));

        let mut config = AppConfig::default();
        config.agent.max_depth = 2;
        config.agent.timeout_seconds = 0; // disable timeout in tests

        let mut agent = make_runtime(llm, db.clone(), config, 0);
        agent.session_id = session_id.clone();
        register_parent(&db, &agent);

        let output = agent.run().await.unwrap();
        assert_eq!(output.summary, "final answer");

        // The child's summary was injected as the spawn_agent tool result.
        let has_child_summary = agent.messages.iter().any(|m| {
            matches!(m, Message::Tool { content, .. } if content.contains("child findings"))
        });
        assert!(has_child_summary, "child summary must reach the parent");

        // Agent tree: parent + child with parent_id and depth set.
        let agents = db.get_session_agents_detail(&session_id).unwrap();
        assert_eq!(agents.len(), 2);
        let child = agents.iter().find(|a| a.task == "child task").unwrap();
        assert_eq!(child.depth, 1);
        assert_eq!(child.status, "completed");
        assert_eq!(child.parent_id.as_deref(), Some(agent.id.0.as_str()));

        // Child got the context handoff in its task.
        assert!(child.task == "child task");
    }

    #[tokio::test]
    async fn test_spawn_agent_depth_limit_refused() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        let llm = Arc::new(MockProvider::new(vec![
            MockProvider::spawn_call("child task"),
            MockProvider::text("done anyway"),
        ]));

        let mut config = AppConfig::default();
        config.agent.max_depth = 0; // no nesting allowed
        config.agent.timeout_seconds = 0;

        let mut agent = make_runtime(llm, db.clone(), config, 0);
        agent.session_id = session_id.clone();
        register_parent(&db, &agent);

        let output = agent.run().await.unwrap();
        assert_eq!(output.summary, "done anyway");

        // The spawn was refused: the tool result is an error, no child agent.
        let refused = agent.messages.iter().any(|m| {
            matches!(m, Message::Tool { content, .. } if content.contains("max depth"))
        });
        assert!(refused, "depth refusal must be reported to the model");

        let agents = db.get_session_agents_detail(&session_id).unwrap();
        assert_eq!(agents.len(), 1, "no child agent may be created");
    }

    #[tokio::test]
    async fn test_parallel_batch_executes_all_calls() {
        // Two parallel-safe file_read calls in ONE turn must both execute
        // through the batch pipeline (fleet P2.8) and both results must
        // reach the message history in the original call order.
        let tmp = tempfile::TempDir::new().unwrap();
        let file_a = tmp.path().join("a.txt");
        let file_b = tmp.path().join("b.txt");
        std::fs::write(&file_a, "alpha content").unwrap();
        std::fs::write(&file_b, "beta content").unwrap();

        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        let llm = Arc::new(MockProvider::new(vec![
            CompletionResponse {
                message: Message::assistant_with_tools(
                    None,
                    vec![
                        pr_core::ToolCall::new(
                            "c1",
                            "file_read",
                            serde_json::json!({"path": file_a.to_str().unwrap()}),
                        ),
                        pr_core::ToolCall::new(
                            "c2",
                            "file_read",
                            serde_json::json!({"path": file_b.to_str().unwrap()}),
                        ),
                    ],
                ),
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 5,
                    total_tokens: 10,
                }),
                finish_reason: Some("tool_calls".to_string()),
            },
            MockProvider::text("batch done"),
        ]));

        let mut config = AppConfig::default();
        config.agent.timeout_seconds = 0;

        let mut agent = make_runtime(llm, db.clone(), config, 0);
        agent.session_id = session_id.clone();
        register_parent(&db, &agent);

        let output = agent.run().await.unwrap();
        assert_eq!(output.summary, "batch done");

        let tool_msgs: Vec<(String, String)> = agent
            .messages
            .iter()
            .filter_map(|m| match m {
                Message::Tool {
                    tool_call_id,
                    content,
                } => Some((tool_call_id.clone(), content.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(tool_msgs.len(), 2, "both tool results must be present");
        assert_eq!(tool_msgs[0].0, "c1");
        assert_eq!(tool_msgs[1].0, "c2");
        assert!(tool_msgs[0].1.contains("alpha content"));
        assert!(tool_msgs[1].1.contains("beta content"));
    }

    #[tokio::test]
    async fn test_shell_failure_cascades_across_batch_classes() {
        // [shell(exit 1), file_read] — file_read is parallel-safe, but the
        // shell failure precedes it in the ORIGINAL order, so its result
        // must be replaced with the cascade-cancel message.
        let tmp = tempfile::TempDir::new().unwrap();
        let file_a = tmp.path().join("a.txt");
        std::fs::write(&file_a, "alpha content").unwrap();

        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        let llm = Arc::new(MockProvider::new(vec![
            CompletionResponse {
                message: Message::assistant_with_tools(
                    None,
                    vec![
                        pr_core::ToolCall::new("s1", "shell", serde_json::json!({"command": "exit 1"})),
                        pr_core::ToolCall::new(
                            "f1",
                            "file_read",
                            serde_json::json!({"path": file_a.to_str().unwrap()}),
                        ),
                    ],
                ),
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 5,
                    total_tokens: 10,
                }),
                finish_reason: Some("tool_calls".to_string()),
            },
            MockProvider::text("cascade handled"),
        ]));

        let mut config = AppConfig::default();
        config.agent.timeout_seconds = 0;

        let mut agent = make_runtime(llm, db.clone(), config, 0);
        agent.session_id = session_id.clone();
        register_parent(&db, &agent);

        let output = agent.run().await.unwrap();
        assert_eq!(output.summary, "cascade handled");

        let file_read_msg = agent
            .messages
            .iter()
            .find(|m| matches!(m, Message::Tool { tool_call_id, .. } if tool_call_id == "f1"))
            .expect("file_read tool message must exist");
        if let Message::Tool { content, .. } = file_read_msg {
            assert!(
                content.contains("Cancelled: sibling shell tool failed"),
                "file_read after a failed shell must be cascade-cancelled, got: {content}"
            );
        }
    }

    #[tokio::test]
    async fn test_role_deny_tools_gates_execution_and_schema() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        let mut config = AppConfig::default();
        config.agent.deny_tools.insert(
            "researcher".to_string(),
            vec!["shell".to_string(), "save_contacts".to_string()],
        );

        let llm = Arc::new(MockProvider::new(vec![
            CompletionResponse {
                message: Message::assistant_with_tools(
                    None,
                    vec![pr_core::ToolCall::new(
                        "c1",
                        "shell",
                        serde_json::json!({"command": "echo hi"}),
                    )],
                ),
                usage: Some(Usage {
                    prompt_tokens: 5,
                    completion_tokens: 5,
                    total_tokens: 10,
                }),
                finish_reason: Some("tool_calls".to_string()),
            },
            MockProvider::text("done"),
        ]));

        let mut agent = make_runtime(llm, db.clone(), config, 0);
        agent.session_id = session_id.clone();
        register_parent(&db, &agent);

        // Denied tools are hidden from the model's schema list.
        let schemas = agent.tools.list_schemas();
        assert!(schemas.iter().any(|t| t.name == "shell")); // registry has it
        let prompt = agent.build_system_prompt("");
        assert!(!prompt.contains("Execute a shell command"), "denied tool must be hidden");

        let output = agent.run().await.unwrap();
        assert_eq!(output.summary, "done");
        // Execution was refused with a permission error.
        let denied = agent.messages.iter().any(|m| {
            matches!(m, Message::Tool { content, .. } if content.contains("Permission denied"))
        });
        assert!(denied);
    }

    #[tokio::test]
    async fn test_harvest_finding_from_extract_contacts() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();
        let agent = make_runtime(
            Arc::new(MockProvider::new(vec![])),
            db,
            AppConfig::default(),
            0,
        );

        let args = serde_json::json!({"url": "https://acme.ru/contacts"});
        let output = pr_core::ToolOutput::ok_with_meta(
            "extracted",
            serde_json::json!({
                "counts": {"emails": 3, "phones": 1, "social_profiles": 2, "persons": 2, "companies": 1}
            }),
        );
        let finding = agent.harvest_finding("extract_contacts", &args, &output);
        assert!(finding.is_some());
        let f = finding.unwrap();
        assert!(f.title.contains("acme.ru"));
        assert!(f.content.contains("emails: 3"));
        assert_eq!(f.sources.len(), 1);
        assert_eq!(f.sources[0].url, "https://acme.ru/contacts");
    }

    #[tokio::test]
    async fn test_harvest_finding_skips_failures_and_other_tools() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let agent = make_runtime(
            Arc::new(MockProvider::new(vec![])),
            db,
            AppConfig::default(),
            0,
        );
        let failed = pr_core::ToolOutput::err("boom");
        assert!(agent
            .harvest_finding("extract_contacts", &serde_json::json!({}), &failed)
            .is_none());

        let ok = pr_core::ToolOutput::ok("page text");
        assert!(agent
            .harvest_finding("web_fetch", &serde_json::json!({}), &ok)
            .is_none());
    }

    #[tokio::test]
    async fn test_harvest_finding_from_find_leads() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let agent = make_runtime(
            Arc::new(MockProvider::new(vec![])),
            db,
            AppConfig::default(),
            0,
        );
        let output = pr_core::ToolOutput::ok_with_meta(
            "leads",
            serde_json::json!({
                "leads": [
                    {"person": {"name": "Ann"}, "company": {"name": "Corp"}},
                    {"person": {"name": "Bob"}, "company": {"name": "Ltd"}}
                ],
                "count": 2
            }),
        );
        let finding = agent
            .harvest_finding("find_leads", &serde_json::json!({}), &output)
            .unwrap();
        assert!(finding.title.contains("2"));
        assert!(finding.content.contains("Ann @ Corp"));
    }

    #[tokio::test]
    async fn test_context_handed_to_child() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        let llm = Arc::new(MockProvider::new(vec![
            MockProvider::spawn_call("child task"),
            MockProvider::text("ok"),
            MockProvider::text("final"),
        ]));

        let mut config = AppConfig::default();
        config.agent.max_depth = 2;
        config.agent.timeout_seconds = 0;

        let mut agent = make_runtime(llm, db.clone(), config, 0);
        agent.session_id = session_id.clone();

        // The child's system+user messages live in the DB; the child agent's
        // first user message must contain the parent's context bullets.
        register_parent(&db, &agent);
        agent.run().await.unwrap();

        let agents = db.get_session_agents_detail(&session_id).unwrap();
        let child = agents.iter().find(|a| a.task == "child task").unwrap();
        let child_id = AgentId(child.id.clone());
        let history = db.get_agent_messages(&child_id).unwrap();
        let user_msg = history
            .iter()
            .find(|m| matches!(m, Message::User { .. }))
            .expect("child has a user message");
        match user_msg {
            Message::User { content } => {
                assert!(content.contains("fact from parent"));
                assert!(content.contains("Context from parent agent"));
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn test_truncated_reasoning_response_is_retried() {
        // Empty content + finish_reason "length" means the reasoning model
        // burned its whole output budget on chain-of-thought. The runtime
        // must nudge the model for a direct answer instead of stopping empty.
        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        let llm = Arc::new(MockProvider::new(vec![
            MockProvider::truncated(),
            MockProvider::text("recovered answer"),
        ]));

        let mut config = AppConfig::default();
        config.agent.timeout_seconds = 0;

        let mut agent = make_runtime(llm, db.clone(), config, 0);
        agent.session_id = session_id.clone();
        register_parent(&db, &agent);

        let output = agent.run().await.unwrap();
        assert_eq!(output.summary, "recovered answer");

        let nudged = agent.messages.iter().any(|m| {
            matches!(m, Message::User { content }
                if content.contains("output budget exhausted"))
        });
        assert!(nudged, "truncation nudge must be injected");
    }

    #[tokio::test]
    async fn test_truncated_reasoning_retry_is_bounded() {
        // Perpetual truncation must terminate: after MAX_TRUNCATION_RETRIES
        // nudges the agent stops (with an empty summary) instead of looping.
        let db = Arc::new(Persistence::in_memory().unwrap());
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        let llm = Arc::new(MockProvider::new(vec![
            MockProvider::truncated(),
            MockProvider::truncated(),
            MockProvider::truncated(),
            MockProvider::truncated(),
        ]));

        let mut config = AppConfig::default();
        config.agent.timeout_seconds = 0;

        let mut agent = make_runtime(llm, db.clone(), config, 0);
        agent.session_id = session_id.clone();
        register_parent(&db, &agent);

        let output = agent.run().await.unwrap();
        assert!(output.summary.is_empty());

        let nudges = agent
            .messages
            .iter()
            .filter(|m| matches!(m, Message::User { content }
                if content.contains("output budget exhausted")))
            .count();
        assert_eq!(nudges, MAX_TRUNCATION_RETRIES as usize);
    }

    fn harvest_runtime() -> AgentRuntime {
        let db = Arc::new(Persistence::in_memory().unwrap());
        make_runtime(
            Arc::new(MockProvider::new(vec![])),
            db,
            AppConfig::default(),
            0,
        )
    }

    #[test]
    fn test_harvest_finding_from_web_search() {
        let agent = harvest_runtime();
        let output = ToolOutput::ok_with_meta(
            "results",
            serde_json::json!({
                "query": "rust async runtime",
                "sources": [
                    {"title": "Tokio", "url": "https://tokio.rs", "excerpt": "async runtime"},
                    {"title": "Async std", "url": "https://async.rs", "excerpt": "alt runtime"},
                ]
            }),
        );
        let finding = agent
            .harvest_finding("web_search", &serde_json::json!({"query": "rust async runtime"}), &output)
            .expect("web_search with sources must produce a finding");
        assert_eq!(finding.title, "Web search: rust async runtime");
        assert_eq!(finding.sources.len(), 2);
        assert_eq!(finding.sources[0].url, "https://tokio.rs");
        assert_eq!(finding.sources[0].title, "Tokio");
    }

    #[test]
    fn test_harvest_finding_from_web_fetch() {
        let agent = harvest_runtime();
        let output = ToolOutput::ok_with_meta(
            "page text",
            serde_json::json!({"url": "https://example.com/page", "title": "Example Page"}),
        );
        let finding = agent
            .harvest_finding("web_fetch", &serde_json::json!({"url": "https://example.com/page"}), &output)
            .expect("web_fetch must produce a finding");
        assert_eq!(finding.title, "Page fetched: Example Page");
        assert_eq!(finding.sources.len(), 1);
        assert_eq!(finding.sources[0].url, "https://example.com/page");
    }

    #[test]
    fn test_harvest_finding_from_search_news_results() {
        let agent = harvest_runtime();
        let output = ToolOutput::ok_with_meta(
            "news",
            serde_json::json!({
                "count": 1,
                "results": [
                    {"title": "Big news", "url": "https://news.example/a", "snippet": "s"}
                ]
            }),
        );
        let finding = agent
            .harvest_finding("search_news", &serde_json::json!({"query": "news"}), &output)
            .expect("search_news results must produce a finding");
        assert_eq!(finding.sources.len(), 1);
        assert_eq!(finding.sources[0].title, "Big news");
    }

    #[test]
    fn test_harvest_finding_skips_empty_or_metadataless() {
        let agent = harvest_runtime();

        let no_meta = ToolOutput::ok("plain");
        assert!(agent
            .harvest_finding("web_search", &serde_json::json!({}), &no_meta)
            .is_none());

        let empty_sources = ToolOutput::ok_with_meta(
            "none",
            serde_json::json!({"query": "q", "sources": []}),
        );
        assert!(agent
            .harvest_finding("web_search", &serde_json::json!({}), &empty_sources)
            .is_none());

        let failed = ToolOutput::err_code("boom", "network");
        let with_meta = ToolOutput {
            metadata: Some(serde_json::json!({"url": "https://x", "title": "t"})),
            ..failed
        };
        assert!(agent
            .harvest_finding("web_fetch", &serde_json::json!({}), &with_meta)
            .is_none());
    }

    // ── True streaming (complete_streaming) ───────────────────────────

    /// Provider whose stream() emits a scripted sequence of chunks.
    struct StreamingMock {
        chunks: Vec<StreamChunk>,
    }

    #[async_trait]
    impl LlmProvider for StreamingMock {
        fn name(&self) -> &str {
            "stream-mock"
        }
        fn model(&self) -> &str {
            "stream-model"
        }
        async fn complete(&self, _req: &CompletionRequest) -> PrResult<CompletionResponse> {
            // Should not be reached when the stream delivers content.
            Err(PrError::Llm("complete() must not be called".into()))
        }
        async fn stream(
            &self,
            _req: &CompletionRequest,
        ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
            let items: Vec<PrResult<StreamChunk>> =
                self.chunks.clone().into_iter().map(Ok).collect();
            Ok(Box::new(futures::stream::iter(items)))
        }
    }

    fn text_chunks(parts: &[&str]) -> Vec<StreamChunk> {
        let mut v: Vec<StreamChunk> = parts
            .iter()
            .map(|p| StreamChunk::Text { delta: p.to_string() })
            .collect();
        v.push(StreamChunk::Done {
            message: Message::assistant(""),
            usage: Some(Usage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            }),
            finish_reason: Some("stop".into()),
        });
        v
    }

    #[tokio::test]
    async fn streaming_assembles_text_and_emits_chunks() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(StreamingMock {
            chunks: text_chunks(&["Hello ", "streaming ", "world"]),
        });
        let agent = make_runtime(llm.clone(), db, AppConfig::default(), 0);
        let mut rx = agent.event_tx.subscribe();

        let req = CompletionRequest {
            messages: vec![Message::user("hi")],
            tools: vec![],
            temperature: Some(0.7),
            max_tokens: Some(100),
            stream: true,
        };
        let resp = agent.complete_streaming(&req).await.unwrap();

        // Final message is the assembled text.
        match &resp.message {
            Message::Assistant { content, tool_calls } => {
                assert_eq!(content.as_deref(), Some("Hello streaming world"));
                assert!(tool_calls.is_empty());
            }
            other => panic!("expected Assistant, got {:?}", other),
        }
        assert_eq!(resp.usage.as_ref().unwrap().total_tokens, 3);

        // Every delta was emitted as its own LlmStreamChunk event.
        let mut deltas = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let AgentEvent::LlmStreamChunk { chunk, .. } = ev {
                deltas.push(chunk);
            }
        }
        assert_eq!(deltas, vec!["Hello ", "streaming ", "world"]);
    }

    #[tokio::test]
    async fn streaming_reassembles_tool_call_fragments() {
        let chunks = vec![
            StreamChunk::ToolCallDelta {
                index: 0,
                id: "call_9".into(),
                name: "web_search".into(),
                arguments_delta: "".into(),
            },
            StreamChunk::ToolCallDelta {
                index: 0,
                id: "".into(),
                name: "".into(),
                arguments_delta: "{\"query\":".into(),
            },
            StreamChunk::ToolCallDelta {
                index: 0,
                id: "".into(),
                name: "".into(),
                arguments_delta: "\"rust\"}".into(),
            },
            StreamChunk::Done {
                message: Message::assistant(""),
                usage: Some(Usage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                }),
                finish_reason: Some("tool_calls".into()),
            },
        ];
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(StreamingMock { chunks });
        let agent = make_runtime(llm, db, AppConfig::default(), 0);

        let req = CompletionRequest {
            messages: vec![Message::user("search")],
            tools: vec![],
            temperature: Some(0.7),
            max_tokens: Some(100),
            stream: true,
        };
        let resp = agent.complete_streaming(&req).await.unwrap();
        match &resp.message {
            Message::Assistant { content, tool_calls } => {
                assert!(content.is_none());
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name(), "web_search");
                assert_eq!(tool_calls[0].id, "call_9");
                assert_eq!(tool_calls[0].arguments(), serde_json::json!({"query": "rust"}));
            }
            other => panic!("expected Assistant, got {:?}", other),
        }
        assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
    }

    /// A provider whose stream yields nothing must fall back to complete()
    /// instead of returning an empty turn.
    struct EmptyStreamThenComplete;

    #[async_trait]
    impl LlmProvider for EmptyStreamThenComplete {
        fn name(&self) -> &str {
            "empty-stream"
        }
        fn model(&self) -> &str {
            "empty-stream-model"
        }
        async fn complete(&self, _req: &CompletionRequest) -> PrResult<CompletionResponse> {
            Ok(CompletionResponse {
                message: Message::assistant("fallback answer"),
                usage: Some(Usage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                }),
                finish_reason: Some("stop".into()),
            })
        }
        async fn stream(
            &self,
            _req: &CompletionRequest,
        ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
            Ok(Box::new(futures::stream::empty()))
        }
    }

    #[tokio::test]
    async fn streaming_empty_falls_back_to_complete() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(EmptyStreamThenComplete);
        let agent = make_runtime(llm, db, AppConfig::default(), 0);

        let req = CompletionRequest {
            messages: vec![Message::user("hi")],
            tools: vec![],
            temperature: Some(0.7),
            max_tokens: Some(100),
            stream: true,
        };
        let resp = agent.complete_streaming(&req).await.unwrap();
        match &resp.message {
            Message::Assistant { content, .. } => {
                assert_eq!(content.as_deref(), Some("fallback answer"));
            }
            other => panic!("expected Assistant, got {:?}", other),
        }
    }

    // ── Control plane (questions + approvals) ─────────────────────────

    #[tokio::test]
    async fn approval_allowed_let_call_through() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(MockProvider::new(vec![]));
        let mut agent = make_runtime(llm, db, AppConfig::default(), 0);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        agent.approval_tx = Some(tx);

        // Operator auto-approves whatever is asked.
        let responder = tokio::spawn(async move {
            if let Some(req) = rx.recv().await {
                assert_eq!(req.tool, "save_contacts");
                let _ = req.reply.send(true);
            }
        });

        let verdict = agent
            .request_approval("save_contacts", &serde_json::json!({"contacts": []}))
            .await;
        responder.await.unwrap();
        assert_eq!(verdict, crate::control::ApprovalVerdict::Allowed);
    }

    #[tokio::test]
    async fn approval_denied_blocks_call() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(MockProvider::new(vec![]));
        let mut agent = make_runtime(llm, db, AppConfig::default(), 0);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        agent.approval_tx = Some(tx);

        let responder = tokio::spawn(async move {
            if let Some(req) = rx.recv().await {
                let _ = req.reply.send(false);
            }
        });

        let verdict = agent
            .request_approval("git_push", &serde_json::json!({}))
            .await;
        responder.await.unwrap();
        assert_eq!(verdict, crate::control::ApprovalVerdict::Denied);
    }

    #[tokio::test]
    async fn approval_without_channel_uses_fallback() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(MockProvider::new(vec![]));

        // Default fallback is "allow".
        let agent = make_runtime(llm.clone(), db.clone(), AppConfig::default(), 0);
        assert_eq!(
            agent.request_approval("save_contacts", &serde_json::json!({})).await,
            crate::control::ApprovalVerdict::Allowed
        );

        // Configured "deny" fallback.
        let mut config = AppConfig::default();
        config.agent.approval_fallback = "deny".to_string();
        let agent = make_runtime(llm, db, config, 0);
        assert_eq!(
            agent.request_approval("save_contacts", &serde_json::json!({})).await,
            crate::control::ApprovalVerdict::Denied
        );
    }

    #[tokio::test]
    async fn requires_approval_matches_config_list() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(MockProvider::new(vec![]));
        let mut config = AppConfig::default();
        config.agent.approval_tools = vec!["git_push".to_string(), "save_contacts".to_string()];
        let agent = make_runtime(llm, db, config, 0);

        assert!(agent.requires_approval("git_push"));
        assert!(agent.requires_approval("GIT_PUSH")); // case-insensitive
        assert!(agent.requires_approval("save_contacts"));
        assert!(!agent.requires_approval("web_search"));
    }

    #[tokio::test]
    async fn question_returns_operator_answer() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(MockProvider::new(vec![]));
        let mut agent = make_runtime(llm, db, AppConfig::default(), 0);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        agent.question_tx = Some(tx);

        let responder = tokio::spawn(async move {
            if let Some(req) = rx.recv().await {
                assert!(req.question.contains("region"));
                let _ = req.reply.send("Focus on the EU region.".to_string());
            }
        });

        let answer = agent.ask_operator("Which region should I focus on?").await;
        responder.await.unwrap();
        assert_eq!(answer, "Focus on the EU region.");
    }

    #[tokio::test]
    async fn question_without_channel_tells_agent_to_proceed() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(MockProvider::new(vec![]));
        let agent = make_runtime(llm, db, AppConfig::default(), 0);

        let answer = agent.ask_operator("Anyone there?").await;
        assert!(answer.contains("best judgment"));
    }

    #[tokio::test]
    async fn question_dropped_reply_falls_back() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let llm = Arc::new(MockProvider::new(vec![]));
        let mut agent = make_runtime(llm, db, AppConfig::default(), 0);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        agent.question_tx = Some(tx);

        // Receive the request but drop the reply sender without answering.
        let responder = tokio::spawn(async move {
            let req = rx.recv().await.unwrap();
            drop(req.reply);
        });

        let answer = agent.ask_operator("Anything?").await;
        responder.await.unwrap();
        assert!(answer.contains("did not answer"));
    }
}
