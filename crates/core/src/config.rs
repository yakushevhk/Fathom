use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_llm")]
    pub llm: LlmConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub export: ExportConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
    #[serde(default)]
    pub contacts: ContactsConfig,
    #[serde(default)]
    pub crm: CrmConfig,
    /// Long-term semantic memory (`[memory]` section).
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Lifecycle hooks (fleet E3, ZCode pattern): subprocesses invoked with
    /// JSON on stdin at PreToolUse / PostToolUse / Stop points.
    #[serde(default)]
    pub hooks: Vec<HookConfig>,
}

fn default_llm() -> LlmConfig {
    LlmConfig::default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// Optional cheap/fast model (same endpoint & credentials) for
    /// high-volume auxiliary calls: entity extraction, memory absorb
    /// classification and search reranking. Empty/unset = reuse `model`.
    #[serde(default)]
    pub fast_model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_tokens() -> u32 { 8192 }
fn default_temperature() -> f32 { 0.7 }

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "deepseek".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            api_key: String::new(),
            model: "deepseek-chat".to_string(),
            fast_model: String::new(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_max_agents")]
    pub max_agents: u32,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub use_multiprocess: bool,
    /// Max children of one parent running concurrently (spawn_agent batches).
    #[serde(default = "default_max_concurrent_children")]
    pub max_concurrent_children: u32,
    /// Seconds of zero progress before a stall warning (0 disables).
    #[serde(default = "default_stall_warn")]
    pub stall_warn_seconds: u64,
    /// Seconds of zero progress before the agent is cancelled (0 disables).
    #[serde(default = "default_stall_kill")]
    pub stall_kill_seconds: u64,
    /// Per-role tool deny lists (fleet E5): role name -> tool names the role
    /// must not see or execute. Example: verifier = ["shell", "save_contacts"].
    #[serde(default)]
    pub deny_tools: std::collections::HashMap<String, Vec<String>>,
    /// Per-role model overrides (fleet E8): role name -> model id on the same
    /// LLM endpoint. Example: researcher = "deepseek-chat" (cheap),
    /// analyst = "deepseek-reasoner" (strong).
    #[serde(default)]
    pub role_models: std::collections::HashMap<String, String>,
    /// Session-wide token budget; 0 disables. Fan-out stops spawning new
    /// agents once the limit is reached.
    #[serde(default)]
    pub session_token_limit: u64,
    /// Full Goal Mode: after the initial fan-out, an LLM judge reviews the
    /// collected results against the original goal and may spawn up to this
    /// many gap-filling replan rounds. 0 disables replanning.
    #[serde(default = "default_replan_rounds")]
    pub replan_rounds: u32,
    /// Tools that require operator approval before executing (exact names).
    /// With no operator connected (headless runs) the `approval_fallback`
    /// applies instead.
    #[serde(default = "default_approval_tools")]
    pub approval_tools: Vec<String>,
    /// Verdict used when no operator is connected or none answers in time:
    /// "allow" keeps classic autonomous behavior, "deny" is fail-safe.
    #[serde(default = "default_approval_fallback")]
    pub approval_fallback: String,
    /// Seconds to wait for an operator approval before the fallback applies
    /// (0 = no waiting, fallback immediately).
    #[serde(default = "default_approval_timeout")]
    pub approval_timeout_seconds: u64,
}

fn default_max_depth() -> u32 { 2 }
fn default_max_agents() -> u32 { 20 }
fn default_max_iterations() -> u32 { 50 }
fn default_timeout() -> u64 { 600 }
fn default_max_concurrent_children() -> u32 { 4 }
fn default_stall_warn() -> u64 { 450 }
fn default_stall_kill() -> u64 { 1200 }
fn default_replan_rounds() -> u32 { 1 }
fn default_approval_tools() -> Vec<String> {
    vec!["save_contacts".to_string(), "git_push".to_string()]
}
fn default_approval_fallback() -> String { "allow".to_string() }
fn default_approval_timeout() -> u64 { 300 }

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_depth: default_max_depth(),
            max_agents: default_max_agents(),
            max_iterations: default_max_iterations(),
            timeout_seconds: default_timeout(),
            use_multiprocess: false,
            max_concurrent_children: default_max_concurrent_children(),
            stall_warn_seconds: default_stall_warn(),
            stall_kill_seconds: default_stall_kill(),
            deny_tools: std::collections::HashMap::new(),
            role_models: std::collections::HashMap::new(),
            session_token_limit: 0,
            replan_rounds: default_replan_rounds(),
            approval_tools: default_approval_tools(),
            approval_fallback: default_approval_fallback(),
            approval_timeout_seconds: default_approval_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Search backend: `linkup`, `exa`, `tavily`, `serper`, `brave`,
    /// `parallel`, `duckduckgo`, `hybrid` (first configured backend that
    /// returns results) or `smart` (all configured backends in parallel,
    /// merged and ranked).
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default)]
    pub linkup: Option<LinkupConfig>,
    #[serde(default)]
    pub parallel: Option<ParallelConfig>,
    #[serde(default)]
    pub exa: Option<ExaConfig>,
    #[serde(default)]
    pub tavily: Option<TavilyConfig>,
    #[serde(default)]
    pub serper: Option<SerperConfig>,
    #[serde(default)]
    pub brave: Option<BraveConfig>,
}

fn default_backend() -> String { "hybrid".to_string() }

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            linkup: None,
            parallel: None,
            exa: None,
            tavily: None,
            serper: None,
            brave: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkupConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExaConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TavilyConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerperConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BraveConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_output_dir")]
    pub dir: String,
}

fn default_output_dir() -> String { "./research-output".to_string() }

impl Default for OutputConfig {
    fn default() -> Self {
        Self { dir: default_output_dir() }
    }
}

/// Result export settings (`[export]` section).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// Target format: `pdf`, `html`, `json` or `docx`.
    #[serde(default = "default_export_format")]
    pub format: String,
}

fn default_export_format() -> String {
    "html".to_string()
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self { format: default_export_format() }
    }
}

impl ExportConfig {
    /// Parse the configured format string into an [`ExportFormat`],
    /// falling back to HTML for unknown values.
    pub fn parsed_format(&self) -> crate::export::ExportFormat {
        crate::export::ExportFormat::parse(&self.format).unwrap_or(crate::export::ExportFormat::Html)
    }
}

/// Notification settings (`[notifications]` section).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub webhook_url: String,
    #[serde(default)]
    pub email_to: String,
    #[serde(default)]
    pub email_from: String,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_username: String,
    #[serde(default)]
    pub smtp_password: String,
    #[serde(default)]
    pub telegram_bot_token: String,
    #[serde(default)]
    pub telegram_chat_id: String,
}

fn default_smtp_port() -> u16 {
    587
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            webhook_url: String::new(),
            email_to: String::new(),
            email_from: String::new(),
            smtp_host: String::new(),
            smtp_port: default_smtp_port(),
            smtp_username: String::new(),
            smtp_password: String::new(),
            telegram_bot_token: String::new(),
            telegram_chat_id: String::new(),
        }
    }
}

/// Contact database settings (`[contacts]` section).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactsConfig {
    /// Path to the SQLite contact database file.
    #[serde(default = "default_contacts_db_path")]
    pub db_path: String,
    /// Optional PostgreSQL connection URL. When non-empty the PostgreSQL
    /// backend is used instead of SQLite (requires the `postgres` feature of
    /// `pr-persistence`).
    #[serde(default)]
    pub pg_url: String,
}

fn default_contacts_db_path() -> String {
    "./contacts.db".to_string()
}

impl Default for ContactsConfig {
    fn default() -> Self {
        Self {
            db_path: default_contacts_db_path(),
            pg_url: String::new(),
        }
    }
}

/// CRM synchronisation settings (`[crm]` section).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrmConfig {
    /// CRM provider name: `amocrm`, `bitrix24`, `hubspot` or empty (disabled).
    #[serde(default)]
    pub provider: String,
    /// Account domain / subdomain (amoCRM and Bitrix24 only).
    #[serde(default)]
    pub domain: String,
    /// API key / token for the provider.
    #[serde(default)]
    pub api_key: String,
}

impl Default for CrmConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            domain: String::new(),
            api_key: String::new(),
        }
    }
}

impl CrmConfig {
    /// Whether a CRM provider is (attempted to be) configured.
    pub fn is_configured(&self) -> bool {
        !self.provider.trim().is_empty()
    }
}

/// Long-term semantic memory settings (`[memory]` section).
///
/// The memory subsystem stores self-contained facts in a SQLite database
/// with hybrid (vector + BM25) search, append-only supersession chains and
/// an `absorb` pipeline that deduplicates and links new facts against
/// existing ones (mem0/Memora-inspired).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Master switch (default true). When false no memory DB is opened and
    /// the memory_* tools are not registered.
    #[serde(default = "default_memory_enabled")]
    pub enabled: bool,
    /// Path to the memory SQLite file. Empty = default
    /// `~/.parallel-research/memory.db` (env `PR_MEMORY_DB` overrides).
    #[serde(default)]
    pub db_path: String,
    /// Embedding backend: `auto` (OpenAI-compatible when credentials are
    /// available, else TF-IDF), `openai` or `tfidf`.
    #[serde(default = "default_embedding_backend")]
    pub embeddings: String,
    /// OpenAI-compatible embeddings endpoint. Empty = reuse `llm.base_url`.
    #[serde(default)]
    pub embedding_base_url: String,
    /// API key for the embeddings endpoint. Empty = reuse `llm.api_key`.
    #[serde(default)]
    pub embedding_api_key: String,
    /// Embedding model id (OpenAI-compatible backend).
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    /// Hybrid search weight: `score = w·semantic + (1−w)·bm25`.
    /// Lower (0.2-0.3) favours exact terms, higher (0.7) semantics.
    #[serde(default = "default_semantic_weight")]
    pub semantic_weight: f32,
    /// Default number of memories returned by search/digest.
    #[serde(default = "default_memory_top_k")]
    pub top_k: u32,
    /// Minimum hybrid score for a search hit (0.2-0.3 typical).
    #[serde(default = "default_memory_min_score")]
    pub min_score: f32,
    /// Linear freshness decay per day applied during ranking
    /// (`score × max(0, 1 − decay·days_old)`); 0 disables.
    #[serde(default = "default_temporal_decay")]
    pub temporal_decay: f32,
    /// Inject a topic digest of relevant memories into the system prompt
    /// of top-level agents.
    #[serde(default = "default_memory_enabled")]
    pub auto_digest: bool,
    /// Use the LLM to classify new facts against candidates
    /// (duplicate/supersede/contradict/related). Falls back to a
    /// similarity-threshold heuristic when no LLM is attached.
    #[serde(default = "default_memory_enabled")]
    pub llm_classify: bool,
    /// Second-pass LLM reranking of search results (better precision,
    /// +1 LLM call per search; requires an LLM in the tool context).
    #[serde(default)]
    pub rerank: bool,
    /// GC: archive untouched run-scoped facts older than this many days.
    #[serde(default = "default_gc_ttl_days")]
    pub gc_ttl_days: u32,
    /// GC: compact a scope group when it holds more than this many active
    /// rows (N→1 consolidation of the oldest/least-important surplus).
    #[serde(default = "default_gc_compact_above")]
    pub gc_compact_above: u32,
    /// GC: daily confidence decay rate for active memories that haven't
    /// been accessed.  Memories below `gc_confidence_threshold` are archived.
    #[serde(default = "default_gc_confidence_decay_rate")]
    pub gc_confidence_decay_rate: f64,
    /// GC: confidence threshold below which a memory is archived.
    #[serde(default = "default_gc_confidence_threshold")]
    pub gc_confidence_threshold: f64,
    /// Auto-run GC + distill on a background timer (hourly).
    #[serde(default)]
    pub gc_auto: bool,
}

fn default_memory_enabled() -> bool { true }
fn default_embedding_backend() -> String { "auto".to_string() }
fn default_embedding_model() -> String { "text-embedding-3-small".to_string() }
fn default_semantic_weight() -> f32 { 0.7 }
fn default_memory_top_k() -> u32 { 5 }
fn default_memory_min_score() -> f32 { 0.25 }
fn default_temporal_decay() -> f32 { 0.01 }
fn default_gc_ttl_days() -> u32 { 30 }
fn default_gc_compact_above() -> u32 { 200 }
fn default_gc_confidence_decay_rate() -> f64 { 0.02 }
fn default_gc_confidence_threshold() -> f64 { 0.15 }

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            db_path: String::new(),
            embeddings: default_embedding_backend(),
            embedding_base_url: String::new(),
            embedding_api_key: String::new(),
            embedding_model: default_embedding_model(),
            semantic_weight: default_semantic_weight(),
            top_k: default_memory_top_k(),
            min_score: default_memory_min_score(),
            temporal_decay: default_temporal_decay(),
            auto_digest: true,
            llm_classify: true,
            rerank: false,
            gc_ttl_days: default_gc_ttl_days(),
            gc_compact_above: default_gc_compact_above(),
            gc_confidence_decay_rate: default_gc_confidence_decay_rate(),
            gc_confidence_threshold: default_gc_confidence_threshold(),
            gc_auto: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

/// Configuration for context management: token budgeting, tool output truncation,
/// and compaction behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Total context window in tokens (default: 128000 for DeepSeek).
    #[serde(default = "default_context_window")]
    pub context_window: u32,
    /// Window profile used when `context_window` is left at its default:
    /// `low` (conservative 128K) or `max` (256K). Explicit `context_window`
    /// always wins (capability evidence, fail-closed).
    #[serde(default)]
    pub context_window_profile: crate::capability::WindowProfile,
    /// Fraction of context window that triggers compaction (default: 0.50).
    #[serde(default = "default_compact_threshold")]
    pub compact_threshold: f32,
    /// Per-tool output cap in bytes (default: 50 KB).
    #[serde(default = "default_tool_output_max_bytes")]
    pub tool_output_max_bytes: u32,
    /// Per-tool output cap in lines (default: 2000).
    #[serde(default = "default_tool_output_max_lines")]
    pub tool_output_max_lines: u32,
    /// Per-turn aggregate tool output budget in bytes (default: 200 KB).
    #[serde(default = "default_turn_budget_bytes")]
    pub turn_budget_bytes: u32,
}

fn default_context_window() -> u32 { 128_000 }
fn default_compact_threshold() -> f32 { 0.50 }
fn default_tool_output_max_bytes() -> u32 { 50_000 }
fn default_tool_output_max_lines() -> u32 { 2_000 }
fn default_turn_budget_bytes() -> u32 { 200_000 }

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            context_window: default_context_window(),
            context_window_profile: crate::capability::WindowProfile::Low,
            compact_threshold: default_compact_threshold(),
            tool_output_max_bytes: default_tool_output_max_bytes(),
            tool_output_max_lines: default_tool_output_max_lines(),
            turn_budget_bytes: default_turn_budget_bytes(),
        }
    }
}

impl ContextConfig {
    /// The fail-closed effective context window, resolved through capability
    /// evidence (explicit window → profile → safety floor). Budgets should use
    /// this rather than `context_window` directly so an unproven large window
    /// never inflates the frame past what the model can actually hold.
    pub fn resolved_window(&self) -> u32 {
        let evidence = crate::capability::resolve_window(
            // Only an explicitly configured (non-default) window counts as
            // confirmed evidence; the default 128K is treated as "not pinned".
            if self.context_window != default_context_window() {
                Some(self.context_window)
            } else {
                None
            },
            self.context_window_profile,
        );
        crate::capability::effective_window(evidence)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: String,  // "stdio" or "http"
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig::default(),
            agent: AgentConfig::default(),
            search: SearchConfig::default(),
            output: OutputConfig::default(),
            mcp: McpConfig::default(),
            context: ContextConfig::default(),
            export: ExportConfig::default(),
            notifications: NotificationsConfig::default(),
            contacts: ContactsConfig::default(),
            crm: CrmConfig::default(),
            memory: MemoryConfig::default(),
            hooks: Vec::new(),
        }
    }
}

/// One lifecycle hook definition (`[[hooks]]` in config).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    /// `PreToolUse`, `PostToolUse` or `Stop`.
    pub event: String,
    /// Command to run; receives the hook payload as JSON on stdin and
    /// answers with a JSON verdict on stdout.
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional: only fire for this tool name (Pre/PostToolUse).
    #[serde(default)]
    pub tool: String,
    /// Hook timeout in milliseconds (default 5000).
    #[serde(default = "default_hook_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_hook_timeout_ms() -> u64 {
    5000
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: AppConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn config_path() -> anyhow::Result<std::path::PathBuf> {
        // Env override: lets tests and budgeted throwaway runs point at a
        // scratch config without touching the user's real one.
        if let Ok(p) = std::env::var("PR_CONFIG") {
            if !p.trim().is_empty() {
                return Ok(std::path::PathBuf::from(p));
            }
        }
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
        Ok(home.join(".parallel-research").join("config.toml"))
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Set a configuration value by dotted key path (e.g. `llm.api_key`,
/// `agent.max_depth`, `search.backend`) in the user config file.
///
/// The value string is parsed as bool / integer / float / string (in that
/// order). The updated document is validated against [`AppConfig`] before
/// being written, so an unknown key or a value with the wrong type is
/// rejected without touching the file.
pub fn set_config_value(key: &str, value: &str) -> anyhow::Result<()> {
    let path = AppConfig::config_path()?;
    let mut root: toml::Value = if path.exists() {
        toml::from_str(&std::fs::read_to_string(&path)?)?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    // If the key already exists as a string, keep it a string — otherwise
    // numeric-looking values ("42", "007") would flip the type and fail
    // AppConfig validation (e.g. telegram_chat_id).
    let existing_is_string = lookup_value(&root, key)
        .map(|v| matches!(v, toml::Value::String(_)))
        .unwrap_or(false);

    let parsed: toml::Value = if existing_is_string {
        toml::Value::String(value.to_string())
    } else if let Ok(b) = value.parse::<bool>() {
        b.into()
    } else if let Ok(i) = value.parse::<i64>() {
        i.into()
    } else if let Ok(f) = value.parse::<f64>() {
        f.into()
    } else {
        toml::Value::String(value.to_string())
    };

    set_nested_value(&mut root, key, parsed)?;

    // Validate that the updated document still parses as AppConfig.
    let _: AppConfig = toml::from_str(&toml::to_string(&root)?)
        .map_err(|e| anyhow::anyhow!("invalid key or value for '{key}': {e}"))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Atomic write: temp file + rename, so a crash mid-write cannot leave a
    // truncated config.toml behind.
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, toml::to_string_pretty(&root)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Look up a value by dotted key path without mutating anything.
fn lookup_value<'a>(root: &'a toml::Value, key: &str) -> Option<&'a toml::Value> {
    let mut cur = root;
    for part in key.split('.') {
        cur = cur.as_table()?.get(part)?;
    }
    Some(cur)
}

/// Insert `value` at the dotted `key` path inside `root`, creating
/// intermediate tables as needed.
fn set_nested_value(root: &mut toml::Value, key: &str, value: toml::Value) -> anyhow::Result<()> {
    let parts: Vec<&str> = key.split('.').collect();
    anyhow::ensure!(
        !parts.is_empty() && parts.iter().all(|p| !p.is_empty()),
        "invalid config key: '{key}'"
    );

    let mut cur = root;
    for (i, part) in parts.iter().enumerate() {
        anyhow::ensure!(
            matches!(cur, toml::Value::Table(_)),
            "config key '{key}': intermediate value is not a table"
        );
        let table = cur.as_table_mut().expect("checked above");
        if i == parts.len() - 1 {
            table.insert((*part).to_string(), value.clone());
            return Ok(());
        }
        cur = table
            .entry((*part).to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    }
    unreachable!("loop returns on the last segment")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_config_default_backend_is_hybrid() {
        let config = SearchConfig::default();
        assert_eq!(config.backend, "hybrid");
        assert!(config.linkup.is_none());
        assert!(config.parallel.is_none());
        assert!(config.exa.is_none());
        assert!(config.tavily.is_none());
        assert!(config.serper.is_none());
        assert!(config.brave.is_none());
    }

    #[test]
    fn test_search_config_parses_new_backends_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[search]
backend = "smart"

[search.exa]
api_key = "exa-key"

[search.tavily]
api_key = "tavily-key"

[search.serper]
api_key = "serper-key"

[search.brave]
api_key = "brave-key"
"#,
        )
        .unwrap();

        assert_eq!(cfg.search.backend, "smart");
        assert_eq!(cfg.search.exa.as_ref().unwrap().api_key, "exa-key");
        assert_eq!(cfg.search.tavily.as_ref().unwrap().api_key, "tavily-key");
        assert_eq!(cfg.search.serper.as_ref().unwrap().api_key, "serper-key");
        assert_eq!(cfg.search.brave.as_ref().unwrap().api_key, "brave-key");
        // Not configured in this file.
        assert!(cfg.search.linkup.is_none());
        assert!(cfg.search.parallel.is_none());
    }

    #[test]
    fn test_search_config_backward_compatible_without_new_sections() {
        // Old configs that only know linkup/parallel must still load.
        let cfg: AppConfig = toml::from_str(
            r#"
[search]
backend = "hybrid"

[search.linkup]
api_key = "linkup-key"
"#,
        )
        .unwrap();

        assert_eq!(cfg.search.backend, "hybrid");
        assert_eq!(cfg.search.linkup.as_ref().unwrap().api_key, "linkup-key");
        assert!(cfg.search.exa.is_none());
    }

    #[test]
    fn test_export_and_notifications_defaults() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.export.format, "html");
        assert_eq!(
            cfg.export.parsed_format(),
            crate::export::ExportFormat::Html
        );
        assert!(cfg.notifications.webhook_url.is_empty());
        assert!(cfg.notifications.email_to.is_empty());
        assert!(cfg.notifications.telegram_bot_token.is_empty());
        assert!(cfg.notifications.telegram_chat_id.is_empty());
        assert_eq!(cfg.notifications.smtp_port, 587);
    }

    #[test]
    fn test_export_and_notifications_parse_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[export]
format = "pdf"

[notifications]
webhook_url = "https://hooks.example.com/x"
email_to = "user@example.com"
telegram_bot_token = "123:abc"
telegram_chat_id = "42"
"#,
        )
        .unwrap();

        assert_eq!(cfg.export.format, "pdf");
        assert_eq!(
            cfg.export.parsed_format(),
            crate::export::ExportFormat::Pdf
        );
        assert_eq!(cfg.notifications.webhook_url, "https://hooks.example.com/x");
        assert_eq!(cfg.notifications.email_to, "user@example.com");
        assert_eq!(cfg.notifications.telegram_bot_token, "123:abc");
        assert_eq!(cfg.notifications.telegram_chat_id, "42");
    }

    #[test]
    fn test_export_format_unknown_falls_back_to_html() {
        let cfg: AppConfig = toml::from_str(
            r#"
[export]
format = "rtf"
"#,
        )
        .unwrap();
        assert_eq!(
            cfg.export.parsed_format(),
            crate::export::ExportFormat::Html
        );
    }

    #[test]
    fn test_old_config_without_export_notifications_still_loads() {
        let cfg: AppConfig = toml::from_str(
            r#"
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
api_key = "k"
model = "deepseek-chat"
"#,
        )
        .unwrap();
        assert_eq!(cfg.export.format, "html");
        assert!(cfg.notifications.email_to.is_empty());
        // New sections fall back to defaults for old configs.
        assert_eq!(cfg.contacts.db_path, "./contacts.db");
        assert!(cfg.contacts.pg_url.is_empty());
        assert!(cfg.crm.provider.is_empty());
        assert!(!cfg.crm.is_configured());
    }

    #[test]
    fn test_contacts_and_crm_config_defaults() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.contacts.db_path, "./contacts.db");
        assert!(cfg.contacts.pg_url.is_empty());
        assert!(cfg.crm.provider.is_empty());
        assert!(cfg.crm.domain.is_empty());
        assert!(cfg.crm.api_key.is_empty());
    }

    #[test]
    fn test_contacts_and_crm_config_parse_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[contacts]
db_path = "/data/contacts.db"
pg_url = "postgres://user:pass@localhost/contacts"

[crm]
provider = "amocrm"
domain = "mycompany"
api_key = "secret-key"
"#,
        )
        .unwrap();

        assert_eq!(cfg.contacts.db_path, "/data/contacts.db");
        assert_eq!(cfg.contacts.pg_url, "postgres://user:pass@localhost/contacts");
        assert_eq!(cfg.crm.provider, "amocrm");
        assert_eq!(cfg.crm.domain, "mycompany");
        assert_eq!(cfg.crm.api_key, "secret-key");
        assert!(cfg.crm.is_configured());
    }

    #[test]
    fn test_memory_config_defaults() {
        let cfg = AppConfig::default();
        assert!(cfg.memory.enabled);
        assert!(cfg.memory.db_path.is_empty());
        assert_eq!(cfg.memory.embeddings, "auto");
        assert_eq!(cfg.memory.embedding_model, "text-embedding-3-small");
        assert!((cfg.memory.semantic_weight - 0.7).abs() < f32::EPSILON);
        assert_eq!(cfg.memory.top_k, 5);
        assert!(cfg.memory.auto_digest);
        assert!(cfg.memory.llm_classify);
    }

    #[test]
    fn test_memory_config_parse_from_toml() {
        let cfg: AppConfig = toml::from_str(
            r#"
[memory]
enabled = true
db_path = "/data/memory.db"
embeddings = "tfidf"
semantic_weight = 0.5
top_k = 10
auto_digest = false
"#,
        )
        .unwrap();
        assert_eq!(cfg.memory.db_path, "/data/memory.db");
        assert_eq!(cfg.memory.embeddings, "tfidf");
        assert!((cfg.memory.semantic_weight - 0.5).abs() < f32::EPSILON);
        assert_eq!(cfg.memory.top_k, 10);
        assert!(!cfg.memory.auto_digest);
        // Fields not present fall back to defaults.
        assert!(cfg.memory.llm_classify);
        assert_eq!(cfg.memory.embedding_model, "text-embedding-3-small");
    }

    #[test]
    fn test_old_config_without_memory_still_loads() {
        let cfg: AppConfig = toml::from_str(
            r#"
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
api_key = "k"
model = "deepseek-chat"
"#,
        )
        .unwrap();
        assert!(cfg.memory.enabled);
        assert!(cfg.memory.auto_digest);
    }

    #[test]
    fn test_set_nested_value_creates_tables_and_types() {
        let mut root = toml::Value::Table(toml::map::Map::new());

        // Required [llm] fields first (they have no serde defaults)...
        set_nested_value(&mut root, "llm.provider", "deepseek".into()).unwrap();
        set_nested_value(&mut root, "llm.base_url", "https://api.deepseek.com".into()).unwrap();
        set_nested_value(&mut root, "llm.model", "deepseek-chat".into()).unwrap();
        // ...then the values under test.
        set_nested_value(&mut root, "llm.api_key", "sk-test".into()).unwrap();
        set_nested_value(&mut root, "agent.max_depth", 3.into()).unwrap();
        set_nested_value(&mut root, "llm.temperature", 0.5.into()).unwrap();
        set_nested_value(&mut root, "agent.use_multiprocess", true.into()).unwrap();

        let cfg: AppConfig = toml::from_str(&toml::to_string(&root).unwrap()).unwrap();
        assert_eq!(cfg.llm.api_key, "sk-test");
        assert_eq!(cfg.agent.max_depth, 3);
        assert_eq!(cfg.llm.temperature, 0.5);
        assert!(cfg.agent.use_multiprocess);
    }

    #[test]
    fn test_role_models_and_token_limit_parse() {
        let cfg: AppConfig = toml::from_str(
            r#"
[agent]
session_token_limit = 500000

[agent.role_models]
researcher = "cheap-model"
analyst = "strong-model"
"#,
        )
        .unwrap();
        assert_eq!(cfg.agent.session_token_limit, 500000);
        assert_eq!(cfg.agent.role_models.get("researcher").map(String::as_str), Some("cheap-model"));
        assert_eq!(cfg.agent.role_models.get("analyst").map(String::as_str), Some("strong-model"));
    }

    #[test]
    fn test_set_nested_value_rejects_bad_keys() {
        let mut root = toml::Value::Table(toml::map::Map::new());
        assert!(set_nested_value(&mut root, "", 1.into()).is_err());
        assert!(set_nested_value(&mut root, "a..b", 1.into()).is_err());
        assert!(set_nested_value(&mut root, ".a", 1.into()).is_err());
    }

    #[test]
    fn test_set_nested_value_rejects_scalar_intermediate() {
        let mut root: toml::Value = toml::from_str("llm = 5").unwrap();
        assert!(set_nested_value(&mut root, "llm.api_key", "x".into()).is_err());
    }

    #[test]
    fn test_config_value_parsing_order() {
        // Mirrors the parse order used by set_config_value.
        fn parse_like_cli(value: &str) -> toml::Value {
            if let Ok(b) = value.parse::<bool>() {
                b.into()
            } else if let Ok(i) = value.parse::<i64>() {
                i.into()
            } else if let Ok(f) = value.parse::<f64>() {
                f.into()
            } else {
                toml::Value::String(value.to_string())
            }
        }

        assert!(matches!(parse_like_cli("true"), toml::Value::Boolean(true)));
        assert!(matches!(parse_like_cli("42"), toml::Value::Integer(42)));
        assert!(matches!(parse_like_cli("0.7"), toml::Value::Float(_)));
        assert!(matches!(parse_like_cli("deepseek"), toml::Value::String(_)));
    }
}
