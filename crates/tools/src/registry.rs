use async_trait::async_trait;
use pr_core::{ToolSchema, ToolOutput, SearchConfig};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::cache::{FetchCache, MxCache};
use crate::file_history::FileHistory;
use crate::file_lock::FileLockManager;

/// Tracks which files have been read (via `file_read`) and their modification
/// time at the point of reading. Used by the validation gate to ensure files
/// are read before they are edited, and to detect stale reads.
pub struct ReadTracker {
    /// Map from canonical file path to the mtime recorded when the file was last read.
    reads: HashMap<PathBuf, std::time::SystemTime>,
}

impl ReadTracker {
    pub fn new() -> Self {
        Self {
            reads: HashMap::new(),
        }
    }

    /// Record that a file at `path` was read. Stores the file's current mtime.
    pub fn record_read(&mut self, path: &Path) -> anyhow::Result<()> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::now());
        self.reads.insert(canonical, mtime);
        Ok(())
    }

    /// Check whether a file has been read.
    pub fn has_read(&self, path: &Path) -> bool {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.reads.contains_key(&canonical)
    }

    /// Check whether the file's current mtime matches the mtime recorded at
    /// last read. Returns `Ok(true)` if the file is stale (mtime changed),
    /// `Ok(false)` if the mtime matches, or `Err` if the file has never been read.
    pub fn is_stale(&self, path: &Path) -> anyhow::Result<bool> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let recorded = self
            .reads
            .get(&canonical)
            .ok_or_else(|| anyhow::anyhow!("File has not been read: {}", path.display()))?;
        let current_mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::now());
        Ok(current_mtime != *recorded)
    }
}

impl Default for ReadTracker {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ToolContext {
    pub working_dir: PathBuf,
    pub http_client: reqwest::Client,
    pub search_config: SearchConfig,
    pub file_history: Arc<Mutex<FileHistory>>,
    pub file_locks: Arc<FileLockManager>,
    pub read_tracker: Arc<Mutex<ReadTracker>>,
    /// Base URL of the OpenAI-compatible vision API
    /// (env `PARALLEL_VISION_API_BASE`, default `https://router.y7.hk/v1`).
    pub vision_api_base: String,
    /// API key for the vision API
    /// (env `PARALLEL_VISION_API_KEY`, default `sk-haus`).
    pub vision_api_key: String,
    /// Optional LLM provider for tools that support LLM-assisted extraction
    /// (e.g. `extract_contacts` with `enrich_entities: true`).
    pub llm: Option<Arc<dyn pr_llm::LlmProvider>>,
    /// Optional cheap/fast provider (`[llm] fast_model`) preferred for
    /// high-volume auxiliary calls (entity extraction, memory classify,
    /// search rerank). Falls back to [`Self::llm`] when unset.
    pub fast_llm: Option<Arc<dyn pr_llm::LlmProvider>>,
    /// Optional contact database (SQLite or PostgreSQL) used by the
    /// `save_contacts` tool to persist harvested contacts.
    pub contact_db: Option<Arc<dyn pr_persistence::ContactStore>>,
    /// Optional CRM sync (amoCRM/Bitrix24/HubSpot) used by `save_contacts`
    /// to push saved contacts into the configured CRM.
    pub crm: Option<Arc<pr_core::CrmSync>>,
    /// Session-scoped fetch cache (fleet B15/B16): `web_fetch` checks this
    /// before hitting the network, so repeated fetches of the same URL
    /// within the TTL skip the download entirely.
    pub fetch_cache: FetchCache,
    /// Session-scoped MX cache (fleet B16): `verify_email` checks this
    /// before the DNS-over-HTTPS MX lookup, so repeated checks of the same
    /// domain skip the DNS round trip.
    pub mx_cache: MxCache,
    /// Optional long-term semantic memory store used by the `memory_*`
    /// knowledge-base tools (absorb / search / digest / boost / link).
    pub memory: Option<Arc<pr_memory::Memory>>,
    /// Session id used to scope run-level memories and to tag absorbed
    /// facts with their provenance.
    pub session_id: Option<String>,
    /// Verification-receipt ledger (ouroboros-inspired): durable typed record
    /// of which contact checks ran and what they concluded, so persistence can
    /// distinguish verified facts from guesses. Lazily attached by the runtime.
    pub receipt_ledger: Option<crate::receipt::ReceiptLedger>,
    /// Agent id of the calling agent (set by the runtime before tool execution).
    /// Needed by coordination tools like `hub` to identify the sender.
    pub agent_id: Option<pr_core::AgentId>,
}

impl ToolContext {
    /// Create a new ToolContext with all subsystems initialized.
    pub fn new(working_dir: PathBuf, search_config: SearchConfig) -> Self {
        Self {
            working_dir: working_dir.clone(),
            // Bounded timeouts: a slow/hanging server must not block a tool
            // call (and the agent loop) indefinitely.
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            search_config,
            file_history: Arc::new(Mutex::new(FileHistory::new(working_dir))),
            file_locks: Arc::new(FileLockManager::new()),
            read_tracker: Arc::new(Mutex::new(ReadTracker::new())),
            vision_api_base: crate::vision::vision_api_base_from_env(),
            vision_api_key: crate::vision::vision_api_key_from_env(),
            llm: None,
            fast_llm: None,
            contact_db: None,
            crm: None,
            fetch_cache: FetchCache::new(),
            mx_cache: MxCache::new(),
            memory: None,
            session_id: None,
            receipt_ledger: None,
            agent_id: None,
        }
    }

    /// Attach an LLM provider for tools that support LLM-assisted extraction
    /// (e.g. `extract_contacts` with `enrich_entities: true`).
    pub fn with_llm(mut self, llm: Arc<dyn pr_llm::LlmProvider>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Attach the cheap/fast provider (`[llm] fast_model`) used for
    /// high-volume auxiliary calls.
    pub fn with_fast_llm(mut self, llm: Arc<dyn pr_llm::LlmProvider>) -> Self {
        self.fast_llm = Some(llm);
        self
    }

    /// The provider auxiliary LLM calls should use: the fast model when
    /// configured, otherwise the agent's main model.
    pub fn aux_llm(&self) -> Option<Arc<dyn pr_llm::LlmProvider>> {
        self.fast_llm.clone().or_else(|| self.llm.clone())
    }

    /// Attach the contact database for the `save_contacts` tool.
    pub fn with_contact_db(mut self, db: Arc<dyn pr_persistence::ContactStore>) -> Self {
        self.contact_db = Some(db);
        self
    }

    /// Attach CRM sync for the `save_contacts` tool.
    pub fn with_crm(mut self, crm: Arc<pr_core::CrmSync>) -> Self {
        self.crm = Some(crm);
        self
    }

    /// Attach the long-term semantic memory store for the `memory_*` tools.
    pub fn with_memory(mut self, memory: Arc<pr_memory::Memory>) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Set the session id used to scope/tag run-level memories.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Attach the verification-receipt ledger used by `verify_*` tools to
    /// record their conclusions durably, and by `save_contacts` / `autosave`
    /// to gate the "verified" flag.
    pub fn with_receipt_ledger(mut self, ledger: crate::receipt::ReceiptLedger) -> Self {
        self.receipt_ledger = Some(ledger);
        self
    }

    /// Attach the calling agent's id (needed by `hub` and other
    /// coordination tools to identify the sender).
    pub fn with_agent_id(mut self, agent_id: pr_core::AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    /// Replace the per-agent fetch cache with a session-shared instance so
    /// sibling agents don't re-download the same URLs within the TTL.
    pub fn with_fetch_cache(mut self, cache: FetchCache) -> Self {
        self.fetch_cache = cache;
        self
    }

    /// Replace the per-agent MX cache with a session-shared instance.
    pub fn with_mx_cache(mut self, cache: MxCache) -> Self {
        self.mx_cache = cache;
        self
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn list_schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|t| t.schema()).collect()
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        match self.tools.get(name) {
            Some(tool) => tool.execute(args, ctx).await,
            None => Ok(ToolOutput::err(format!("Unknown tool: {name}"))),
        }
    }

    /// Create a registry with all built-in tools
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(crate::web::WebSearchTool));
        registry.register(Arc::new(crate::web::WebFetchTool));
        registry.register(Arc::new(crate::crawl::WebCrawlTool));
        registry.register(Arc::new(crate::crawl::WebFeedTool));
        registry.register(Arc::new(crate::code::CodeSymbolsTool));
        registry.register(Arc::new(crate::code::RepoMapTool));
        registry.register(Arc::new(crate::parse::ParseHtmlTool));
        registry.register(Arc::new(crate::parse::ExtractJsonTool));
        registry.register(Arc::new(crate::file::FileReadTool));
        registry.register(Arc::new(crate::file::FileWriteTool));
        registry.register(Arc::new(crate::file::FileEditTool));
        registry.register(Arc::new(crate::file::GlobTool));
        registry.register(Arc::new(crate::file::GrepTool));
        registry.register(Arc::new(crate::shell::ShellTool));
        registry.register(Arc::new(crate::memory_tool::MemoryTool));

        // Long-term semantic memory knowledge base (mem0/Memora-inspired).
        // The tools no-op with a clear error when no memory store is
        // attached to the ToolContext.
        registry.register(Arc::new(crate::memory_kb::MemoryAbsorbTool));
        registry.register(Arc::new(crate::memory_kb::MemorySearchTool));
        registry.register(Arc::new(crate::memory_kb::MemoryDigestTool));
        registry.register(Arc::new(crate::memory_kb::MemoryBoostTool));
        registry.register(Arc::new(crate::memory_kb::MemoryLinkTool));
        registry.register(Arc::new(crate::memory_kb::MemoryGraphTool));

        // Browser automation: only registered when a CDP endpoint is reachable.
        let cdp_endpoint = crate::browser::cdp_endpoint_from_env();
        if crate::browser::cdp_available(&cdp_endpoint) {
            registry.register(Arc::new(crate::browser::BrowserNavigateTool::new(
                cdp_endpoint.clone(),
            )));
            registry.register(Arc::new(crate::browser::BrowserScreenshotTool::new(
                cdp_endpoint.clone(),
            )));
            registry.register(Arc::new(crate::browser::BrowserClickTool::new(
                cdp_endpoint.clone(),
            )));
            registry.register(Arc::new(crate::browser::BrowserTypeTool::new(
                cdp_endpoint.clone(),
            )));
            registry.register(Arc::new(crate::browser::BrowserExtractTool::new(
                cdp_endpoint,
            )));
        }

        // Vision / image analysis.
        registry.register(Arc::new(crate::vision::VisionTool::new()));

        // Git operations.
        registry.register(Arc::new(crate::git::GitStatusTool));
        registry.register(Arc::new(crate::git::GitDiffTool));
        registry.register(Arc::new(crate::git::GitLogTool));
        registry.register(Arc::new(crate::git::GitAddTool));
        registry.register(Arc::new(crate::git::GitCommitTool));
        registry.register(Arc::new(crate::git::GitPushTool));

        // PDF text extraction.
        registry.register(Arc::new(crate::pdf::PdfTool));

        // Code REPLs.
        registry.register(Arc::new(crate::repl::PythonExecTool));
        registry.register(Arc::new(crate::repl::NodeExecTool));

        // Lead sources (OSINT/lead generation).
        registry.register(Arc::new(crate::directories::DirectorySearchTool));
        registry.register(Arc::new(crate::social_search::SocialSearchTool));
        registry.register(Arc::new(crate::corporate::CorporateParseTool));
        registry.register(Arc::new(crate::news::NewsSearchTool));
        registry.register(Arc::new(crate::lead_finder::LeadFinderTool));

        // Data verification and enrichment (OSINT/lead generation).
        registry.register(Arc::new(crate::verify_email::EmailVerifier));
        registry.register(Arc::new(crate::verify_email::EmailSuggester));
        registry.register(Arc::new(crate::verify_phone::PhoneVerifier));
        registry.register(Arc::new(crate::verify_social::SocialVerifier));
        registry.register(Arc::new(crate::enrich_company::CompanyEnricher));
        registry.register(Arc::new(crate::enrich_person::PersonEnricher));

        // Contact extraction (OSINT/lead generation).
        registry.register(Arc::new(crate::extract::ContactExtractor));

        // Contact persistence + CRM push (OSINT/lead generation).
        registry.register(Arc::new(crate::save_contacts::SaveContactsTool));

        // Coordination: skill loading (E7) + shared session ledger (C8)
        // + undo over file history (OpenCode-style).
        registry.register(Arc::new(crate::coordination::SkillTool));
        registry.register(Arc::new(crate::coordination::ScratchpadTool));
        registry.register(Arc::new(crate::coordination::UndoTool));

        // Hierarchical delegation: spawn sub-agents from within an agent.
        // Depth enforcement happens in the agent runtime (it knows the
        // caller's depth); the tool only validates and packages the request.
        registry.register(Arc::new(crate::spawn::SpawnAgentTool));

        // Inter-agent coordination: hub tool for messaging, discovery,
        // and coordination.
        registry.register(Arc::new(crate::hub::HubTool));

        // Daemon process management: start/stop/restart long-running
        // processes (dev servers, watchers, REPLs).
        registry.register(Arc::new(crate::daemon::DaemonTool));

        // Operator control plane: ask the human mid-run. The actual
        // round-trip is performed by the agent runtime.
        registry.register(Arc::new(crate::question::QuestionTool));

        registry
    }

    /// Register the LSP tool for the given project root. The LSP server is
    /// lazily started on first use and auto-detected from the project's files.
    pub fn register_lsp(&mut self, project_root: std::path::PathBuf) {
        self.register(Arc::new(LspToolAdapter {
            inner: pr_lsp::LspTool::new(project_root),
        }));
    }
}

/// Adapter that wraps `pr_lsp::LspTool` to implement the `pr_tools::Tool` trait.
struct LspToolAdapter {
    inner: pr_lsp::LspTool,
}

#[async_trait]
impl Tool for LspToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn schema(&self) -> ToolSchema {
        self.inner.schema()
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        self.inner.execute(args, &ctx.working_dir).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_context_has_vision_config() {
        let ctx = ToolContext::new(PathBuf::from("/tmp"), pr_core::SearchConfig::default());
        // Defaults come from the environment or the built-in constants.
        assert!(!ctx.vision_api_base.is_empty());
        assert!(!ctx.vision_api_key.is_empty());
    }

    #[test]
    fn test_with_builtins_registers_new_tools() {
        let registry = ToolRegistry::with_builtins();
        for name in [
            "web_search", "web_fetch", "web_crawl", "web_feed",
            "code_symbols", "repo_map",
            "parse_html", "extract_json",
            "analyze_image",
            "git_status",
            "git_diff",
            "git_log",
            "git_add",
            "git_commit",
            "git_push",
            "pdf_extract",
            "python_exec",
            "node_exec",
            "search_business_directory",
            "search_social",
            "parse_corporate_site",
            "search_news",
            "find_leads",
            "verify_email",
            "verify_phone",
            "verify_social_profile",
            "enrich_company",
            "enrich_person",
            "extract_contacts",
            "save_contacts",
            "spawn_agent",
            "question",
            "memory_absorb",
            "memory_search",
            "memory_digest",
            "memory_boost",
            "memory_link",
            "memory_graph",
        ] {
            assert!(registry.get(name).is_some(), "{name} should be registered");
        }

        // Browser tools are only registered when a CDP endpoint is reachable.
        let cdp_up = crate::browser::cdp_available(&crate::browser::cdp_endpoint_from_env());
        for name in [
            "browser_navigate",
            "browser_screenshot",
            "browser_click",
            "browser_type",
            "browser_extract",
        ] {
            assert_eq!(
                registry.get(name).is_some(),
                cdp_up,
                "{name} registration should match CDP availability ({cdp_up})"
            );
        }
    }

    #[test]
    fn test_read_tracker_records_and_staleness() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("t.txt");
        std::fs::write(&file, "hello").unwrap();

        let mut tracker = ReadTracker::new();
        assert!(!tracker.has_read(&file));
        tracker.record_read(&file).unwrap();
        assert!(tracker.has_read(&file));
        assert!(!tracker.is_stale(&file).unwrap());
    }

    #[test]
    fn test_register_lsp_tool() {
        let mut registry = ToolRegistry::with_builtins();
        assert!(registry.get("lsp").is_none());
        registry.register_lsp(std::path::PathBuf::from("/tmp"));
        assert!(registry.get("lsp").is_some());
    }
}
