mod bench;

use clap::{Parser, Subcommand};
use pr_core::{AppConfig, SessionId};
use pr_persistence::Persistence;
use pr_agent::Coordinator;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "fathom",
    about = "Universal autonomous AI worker — research, outreach, code, computer use",
    version,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a task (headless, autonomous)
    Run {
        /// The research query (optional when --task-file is given)
        query: Option<String>,
        /// Read the task from a file instead of the positional argument
        /// (long instructions with quotes/newlines, e.g. Terminal-Bench).
        /// The file content wins over `query` when both are given.
        #[arg(long)]
        task_file: Option<std::path::PathBuf>,
        /// Output directory
        #[arg(short, long)]
        output: Option<String>,
        /// Re-run the same query every N seconds (scheduled harvesting).
        /// Runs forever until interrupted.
        #[arg(long)]
        repeat: Option<u64>,
        /// Persona/profile to apply (hunter | analyst | validator | file
        /// name in ~/.fathom/profiles | path to a .toml).
        #[arg(long)]
        profile: Option<String>,
    },
    /// Worker mode (internal - spawned by coordinator)
    Worker {
        /// Session ID
        #[arg(long)]
        session_id: String,
        /// Agent ID
        #[arg(long)]
        agent_id: String,
        /// Task description
        #[arg(long)]
        task: String,
        /// Socket path for IPC
        #[arg(long)]
        socket: String,
        /// Agent role
        #[arg(long, default_value = "researcher")]
        role: String,
    },
    /// Interactive TUI mode
    Tui {
        /// Optional initial query
        query: Option<String>,
        /// Persona/profile to apply to sessions started from the TUI.
        #[arg(long)]
        profile: Option<String>,
        /// Replay a stored session instead of starting a live run
        /// (session id; prefix match is accepted).
        #[arg(long)]
        replay: Option<String>,
    },
    /// Server mode (Phase 4)
    Serve {
        #[arg(long, default_value = "8080")]
        port: u16,
        /// Bind address. Loopback by default; a non-loopback address requires
        /// FATHOM_API_KEYS to be set.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// Expose the agent's tools over MCP (stdio) for external clients
    /// (Claude, ZCode, ...). Add to an MCP client config as:
    /// `{"command": "fathom", "args": ["mcp-serve"]}`.
    McpServe,
    /// Contact database operations (OSINT / lead generation)
    Contacts {
        #[command(subcommand)]
        action: ContactsAction,
    },
    /// Long-term semantic memory operations (search / list / stats / distill)
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Browse past sessions stored in the database
    Sessions {
        /// Session database directory (contains .research.db). Defaults to
        /// the configured output dir.
        #[arg(short, long)]
        output: Option<String>,
        #[command(subcommand)]
        action: SessionsAction,
    },
    /// Resume an interrupted session (re-runs its unfinished sub-tasks)
    Resume {
        /// Output directory of the session (contains .research.db).
        /// Defaults to the configured output dir.
        #[arg(short, long)]
        output: Option<String>,
        /// Session id to resume. Defaults to the most recent interrupted one.
        #[arg(short, long)]
        session_id: Option<String>,
    },
    /// Show configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Benchmark the tool-execution layer (no LLM, no network)
    Bench {
        /// Scenario: all | dispatch | parallel-io | parallel-cpu | mixed | parse-scale | extract-json | feed-parse | code-map
        #[arg(short, long, default_value = "all")]
        scenario: String,
        /// Number of parallel-safe calls / data files in the batch scenarios
        #[arg(short, long, default_value = "16")]
        n: usize,
        /// Also write the markdown report to this file
        #[arg(long)]
        save: Option<String>,
    },
    /// Show tool-call statistics for a recorded session
    Stats {
        /// Session output directory (contains .research.db).
        /// Defaults to the configured output dir.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Background jobs: submit tasks that run detached, check status any
    /// time, read logs, cancel. Failed attempts are retried automatically
    /// with a self-healing task that carries the previous error.
    Jobs {
        #[command(subcommand)]
        action: JobsAction,
    },
    /// Internal runner for a background job (spawned detached by `jobs submit`)
    #[command(hide = true)]
    JobRun {
        /// Job id
        id: String,
    },
    /// Personas/profiles: list, show or create task presets
    /// (--profile on run/tui).
    Profiles {
        #[command(subcommand)]
        action: ProfilesAction,
    },
}

#[derive(Subcommand)]
enum ProfilesAction {
    /// List available profiles (user files + built-ins)
    List,
    /// Show one profile's definition
    Show { name: String },
    /// Create a template profile file in ~/.fathom/profiles/
    New { name: String },
}

#[derive(Subcommand)]
enum JobsAction {
    /// Submit a task to run in the background
    Submit {
        /// The task to run
        task: String,
        /// Max attempts: on failure the job retries with a self-healing
        /// task augmented with the previous error
        #[arg(long, default_value = "3")]
        attempts: i64,
    },
    /// List all jobs
    List,
    /// Show detailed status of one job
    Status {
        /// Job id (or unique prefix)
        id: String,
        /// Keep refreshing every N seconds until the job reaches a terminal state
        #[arg(long)]
        watch: Option<u64>,
    },
    /// Show the job's log (stdout+stderr of all attempts)
    Logs {
        /// Job id (or unique prefix)
        id: String,
        /// Number of trailing lines
        #[arg(short = 'n', long, default_value = "50")]
        lines: usize,
    },
    /// Cancel a queued or running job
    Cancel {
        /// Job id (or unique prefix)
        id: String,
    },
    /// Re-run a failed/cancelled/completed (or stale) job from scratch
    Rerun {
        /// Job id (or unique prefix)
        id: String,
    },
}

#[derive(Subcommand)]
enum ContactsAction {
    /// List stored contacts
    List {
        #[arg(long, default_value = "50")]
        limit: i64,
    },
    /// Export contacts to a file
    Export {
        /// csv | vcf | json | xlsx
        #[arg(long, default_value = "csv")]
        format: String,
        /// Output directory (default: configured output dir)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Find duplicate contacts (same normalized email or phone); with
    /// --merge fold each group into its most complete row.
    Dedup {
        /// Actually merge duplicates (default: dry run, list groups only)
        #[arg(long)]
        merge: bool,
    },
    /// Push all stored contacts to the configured CRM
    PushCrm,
}

#[derive(Subcommand)]
enum ConfigAction {
    Show,
    Set {
        key: String,
        value: String,
    },
}

#[derive(Subcommand)]
enum SessionsAction {
    /// List recent sessions (newest first)
    List {
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
        /// Filter sessions whose query contains this substring
        #[arg(short, long)]
        search: Option<String>,
    },
    /// Show one session in detail (agents + findings)
    Show {
        /// Session id (or unique prefix)
        id: String,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Hybrid (semantic + keyword) search over stored facts
    Search {
        query: String,
        #[arg(long, default_value = "10")]
        top_k: usize,
        /// Scope filter: agent | user | run | all (default all-persistent)
        #[arg(long, default_value = "persistent")]
        scope: String,
    },
    /// List stored memories (newest first)
    List {
        #[arg(long, default_value = "persistent")]
        scope: String,
        /// active | superseded | archived | all
        #[arg(long, default_value = "active")]
        status: String,
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
    },
    /// Show one memory by id, optionally resolving its version chain
    Get {
        id: String,
        /// active | latest | full_history
        #[arg(long, default_value = "latest")]
        follow: String,
    },
    /// Store statistics: counts by scope/status, entities, db size
    Stats,
    /// Re-embed all memories with the current embedding model
    Rebuild,
    /// Distill run-scoped session facts into persistent agent knowledge
    Distill {
        /// Only this session's run facts (scope_key); default: all
        #[arg(long)]
        session: Option<String>,
        /// Show the plan without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Garbage-collect the store: archive expired & stale run facts,
    /// compact oversized scope groups (N→1). Nothing is deleted.
    Gc {
        /// Override [memory].gc_ttl_days for this pass
        #[arg(long)]
        ttl_days: Option<u32>,
        /// Show the plan without writing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Permanently delete memories (requires --yes)
    Nuke {
        /// Restrict to a scope: agent | user | run | all
        #[arg(long, default_value = "run")]
        scope: String,
        /// Actually perform the deletion
        #[arg(long)]
        yes: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging. Stderr, not stdout: `mcp-serve` owns stdout for
    // the JSON-RPC protocol, and structured output of other commands must
    // not be polluted by log lines either.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            query,
            task_file,
            output,
            repeat,
            profile,
        } => {
            // Resolve the task text: a file (when given) wins over the
            // positional query — Terminal-Bench hands us long instructions
            // that would be awkward to shell-quote.
            let query = match task_file {
                Some(path) => std::fs::read_to_string(&path).map_err(|e| {
                    anyhow::anyhow!("reading task file {}: {e}", path.display())
                })?,
                None => query.ok_or_else(|| {
                    anyhow::anyhow!("no task: pass <QUERY> or --task-file <FILE>")
                })?,
            };
            match repeat {
                Some(secs) if secs > 0 => {
                    // Scheduled harvesting ("watch") loop: rerun the same
                    // query every `secs` seconds until interrupted. Between
                    // runs we diff the contact store and alert about new
                    // findings ("new people at company X").
                    let watch_config = AppConfig::load()?;
                    let notifier =
                        pr_core::Notifier::from_config(&watch_config.notifications);
                    let contact_store =
                        match pr_persistence::open_contact_store(&watch_config.contacts).await {
                            Ok(s) => Some(s),
                            Err(e) => {
                                eprintln!("  ⚠️  Watch diff disabled (no contact store): {e}");
                                None
                            }
                        };

                    // Baseline: everything already known before we started.
                    let mut known_keys = match &contact_store {
                        Some(store) => {
                            let existing = store.list_all(i64::MAX, 0).await.unwrap_or_default();
                            contact_key_set(&existing)
                        }
                        None => std::collections::HashSet::new(),
                    };
                    let mut iteration: u64 = 0;

                    loop {
                        iteration += 1;
                        println!("── Watch iteration {iteration} ──");
                        let started = std::time::Instant::now();
                        if let Err(e) =
                            run_research(query.clone(), output.clone(), profile.clone()).await
                        {
                            eprintln!("Run failed: {e}");
                        }

                        // Diff the contact store against the previous run.
                        if let Some(store) = &contact_store {
                            let current = store.list_all(i64::MAX, 0).await.unwrap_or_default();
                            let new_contacts = watch_new_contacts(&known_keys, &current);
                            if !new_contacts.is_empty() {
                                let report = watch_report(&query, &new_contacts);
                                println!("\n🔔 {report}\n");
                                notifier
                                    .notify_alert(
                                        "watch.new_contacts",
                                        &format!(
                                            "Watch: {} new contact(s) for \"{}\"",
                                            new_contacts.len(),
                                            query
                                        ),
                                        &report,
                                    )
                                    .await;
                            } else {
                                println!("── Watch: no new contacts this run ──");
                            }
                            known_keys = contact_key_set(&current);
                        }

                        let elapsed = started.elapsed().as_secs();
                        let sleep = secs.saturating_sub(elapsed).max(5);
                        println!("Next run in {sleep}s (Ctrl+C to stop)");
                        tokio::time::sleep(std::time::Duration::from_secs(sleep)).await;
                    }
                }
                _ => run_research(query, output, profile).await?,
            }
        }
        Commands::Worker { session_id, agent_id, task, socket, role } => {
            run_worker(session_id, agent_id, task, socket, role).await?;
        }
        Commands::Tui { query, profile, replay } => {
            run_tui(query, profile, replay).await?;
        }
        Commands::Serve { port, host } => {
            pr_server::run_server(host, port).await?;
        }
        Commands::McpServe => {
            run_mcp_serve().await?;
        }
        Commands::Contacts { action } => {
            cmd_contacts(action).await?;
        }
        Commands::Memory { action } => {
            cmd_memory(action).await?;
        }
        Commands::Sessions { output, action } => {
            cmd_sessions(output, action)?;
        }
        Commands::Resume { output, session_id } => {
            cmd_resume(output, session_id).await?;
        }
        Commands::Config { action } => {
            match action {
                ConfigAction::Show => {
                    let config = AppConfig::load()?;
                    println!("{}", toml::to_string_pretty(&config)?);
                }
                ConfigAction::Set { key, value } => {
                    pr_core::set_config_value(&key, &value)?;
                    println!("Set {key} = {value}");
                }
            }
        }
        Commands::Bench { scenario, n, save } => {
            bench::run_bench(&scenario, n, save).await?;
        }
        Commands::Stats { output } => {
            bench::run_stats(output)?;
        }
        Commands::Jobs { action } => {
            cmd_jobs(action).await?;
        }
        Commands::JobRun { id } => {
            cmd_job_run(id).await?;
        }
        Commands::Profiles { action } => {
            cmd_profiles(action)?;
        }
    }

    Ok(())
}

/// Build the tool registry with built-ins plus tools from all configured
/// MCP servers (best effort: unreachable servers are skipped).
async fn build_registry(config: &AppConfig) -> std::sync::Arc<pr_tools::ToolRegistry> {
    let mut registry = pr_tools::ToolRegistry::with_builtins();
    if !config.mcp.servers.is_empty() {
        println!(
            "  🔌 Connecting to {} MCP server(s)...",
            config.mcp.servers.len()
        );
        let _client = pr_mcp::connect_and_register(&mut registry, &config.mcp).await;
    }
    std::sync::Arc::new(registry)
}

/// Open the contact pipeline attachments for a run: the configured contact
/// store (SQLite, or PostgreSQL when `[contacts] pg_url` is set) and the CRM
/// sync when `[crm]` is configured. Both are optional/best-effort.
async fn open_contact_attachments(
    config: &AppConfig,
) -> (
    Option<std::sync::Arc<dyn pr_persistence::ContactStore>>,
    Option<std::sync::Arc<pr_core::CrmSync>>,
) {
    let store = match pr_persistence::open_contact_store(&config.contacts).await {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("  ⚠️  Contact store unavailable: {e}");
            None
        }
    };
    let crm = pr_core::CrmSync::from_config(&config.crm).map(std::sync::Arc::new);
    (store, crm)
}

/// Open the long-term semantic memory store when `[memory] enabled` is set.
/// Best-effort: a failure here must never block the run.
fn open_memory(config: &AppConfig) -> Option<std::sync::Arc<pr_memory::Memory>> {
    if !config.memory.enabled {
        return None;
    }
    match pr_memory::Memory::open(&config.memory, &config.llm) {
        Ok(mem) => Some(std::sync::Arc::new(mem)),
        Err(e) => {
            eprintln!("  ⚠️  Memory store unavailable: {e}");
            None
        }
    }
}

/// Parse a CLI scope selector into a ScopeFilter.
/// `persistent` = agent+user, `all` = no filter, else a single scope.
fn memory_scope_filter(scope: &str) -> anyhow::Result<pr_memory::ScopeFilter> {
    match scope.to_lowercase().as_str() {
        "persistent" | "" => Ok(pr_memory::ScopeFilter::persistent()),
        "all" => Ok(pr_memory::ScopeFilter::new()),
        s => {
            let parsed: pr_memory::Scope = s.parse()?;
            Ok(pr_memory::ScopeFilter::new().add(parsed, ""))
        }
    }
}

/// `fathom memory ...` — manage the long-term memory store
/// without running an agent.
async fn cmd_memory(action: MemoryAction) -> anyhow::Result<()> {
    let config = AppConfig::load()?;
    let Some(mem) = open_memory(&config) else {
        anyhow::bail!("memory subsystem disabled ([memory] enabled = false)");
    };

    match action {
        MemoryAction::Search { query, top_k, scope } => {
            let filter = memory_scope_filter(&scope)?;
            let hits = mem.search(&query, &filter, Some(top_k)).await?;
            if hits.is_empty() {
                println!("No matching memories.");
                return Ok(());
            }
            for h in &hits {
                println!(
                    "[{}] score={:.2} ({}, scope={}, source={}, conf {:.2}) {}",
                    &h.memory.id,
                    h.score,
                    &h.memory.created_at[..h.memory.created_at.len().min(10)],
                    h.memory.scope,
                    if h.memory.source.is_empty() { "-" } else { &h.memory.source },
                    h.memory.confidence,
                    h.memory.content
                );
            }
        }
        MemoryAction::List { scope, status, limit } => {
            let filter = memory_scope_filter(&scope)?;
            let status_arg = match status.as_str() {
                "all" => None,
                s => Some(s.to_string()),
            };
            let rows = mem.db.list(&filter, status_arg.as_deref(), limit)?;
            if rows.is_empty() {
                println!("No memories.");
                return Ok(());
            }
            for r in &rows {
                println!(
                    "[{}] ({}, {}, scope={}{}) {}",
                    &r.id,
                    r.status,
                    &r.created_at[..r.created_at.len().min(10)],
                    r.scope,
                    if r.scope_key.is_empty() { String::new() } else { format!(":{}", r.scope_key) },
                    r.content
                );
            }
            println!("{} memory(ies)", rows.len());
        }
        MemoryAction::Get { id, follow } => {
            let follow_mode: pr_memory::Follow = follow.parse()?;
            let rows = pr_memory::resolve_follow(&mem.db, &id, follow_mode)?;
            if rows.is_empty() {
                anyhow::bail!("no memory found for '{id}' (follow={follow})");
            }
            for r in &rows {
                println!(
                    "[{}] status={} confidence={:.2} importance={:.2} source={} created={}",
                    r.id, r.status, r.confidence, r.importance, r.source, r.created_at
                );
                if !r.tags.is_empty() {
                    println!("  tags: {}", r.tags.join(", "));
                }
                if r.metadata.is_object() && !r.metadata.as_object().unwrap().is_empty() {
                    println!("  metadata: {}", r.metadata);
                }
                println!("  {}", r.content);
            }
        }
        MemoryAction::Stats => {
            let scopes = [
                ("agent", pr_memory::Scope::Agent),
                ("user", pr_memory::Scope::User),
                ("run", pr_memory::Scope::Run),
            ];
            println!("Memory store: {}", config.memory.db_path.if_empty_then_default());
            println!(
                "  embedding model: {} (backend: {})",
                mem.embedder.model_name(),
                config.memory.embeddings
            );
            for (label, scope) in scopes {
                let filter = pr_memory::ScopeFilter::new().add(scope, "");
                let active = mem.db.list(&filter, Some("active"), usize::MAX)?.len();
                let superseded = mem.db.list(&filter, Some("superseded"), usize::MAX)?.len();
                let archived = mem.db.list(&filter, Some("archived"), usize::MAX)?.len();
                println!(
                    "  {label:<6} active={active} superseded={superseded} archived={archived}"
                );
            }
            let (nodes, edges) = mem.db.count_entities()?;
            println!("  entity graph: {nodes} nodes, {edges} relations");
            let path = if config.memory.db_path.is_empty() {
                pr_memory::default_memory_db_path()
            } else {
                std::path::PathBuf::from(&config.memory.db_path)
            };
            if let Ok(meta) = std::fs::metadata(&path) {
                println!("  db size: {:.1} MB ({})", meta.len() as f64 / 1_048_576.0, path.display());
            }
        }
        MemoryAction::Rebuild => {
            let n = mem.rebuild_embeddings().await?;
            println!("Re-embedded {n} memory(ies) with model '{}'", mem.embedder.model_name());
        }
        MemoryAction::Distill { session, dry_run } => {
            println!(
                "Distilling run-scoped facts{}{}",
                session.as_deref().map(|s| format!(" of session {s}")).unwrap_or_default(),
                if dry_run { " (dry run)" } else { "" }
            );
            let report = mem.distill(session.as_deref(), dry_run).await?;
            println!("  {}", report.summary_line());
        }
        MemoryAction::Gc { ttl_days, dry_run } => {
            let opts = pr_memory::GcOptions {
                ttl_days: ttl_days.unwrap_or(config.memory.gc_ttl_days),
                compact_above: config.memory.gc_compact_above as usize,
                dry_run,
                ..Default::default()
            };
            println!(
                "Running memory GC (ttl {}d, compact above {}){}",
                opts.ttl_days,
                opts.compact_above,
                if dry_run { " (dry run)" } else { "" }
            );
            let report = mem.gc(&opts).await?;
            println!("  {}", report.summary_line());
        }
        MemoryAction::Nuke { scope, yes } => {
            let filter = memory_scope_filter(&scope)?;
            let rows = mem.db.list(&filter, None, usize::MAX)?;
            if rows.is_empty() {
                println!("Nothing to delete.");
                return Ok(());
            }
            if !yes {
                println!(
                    "Would permanently delete {} memory(ies) from scope '{scope}'. Re-run with --yes to confirm.",
                    rows.len()
                );
                return Ok(());
            }
            let mut n = 0usize;
            for r in &rows {
                if mem.db.delete(&r.id)? {
                    n += 1;
                }
            }
            println!("Permanently deleted {n} memory(ies).");
        }
    }
    Ok(())
}

/// Small helper to render the effective db path in `memory stats`.
trait IfEmptyDefault {
    fn if_empty_then_default(&self) -> String;
}
impl IfEmptyDefault for String {
    fn if_empty_then_default(&self) -> String {
        if self.is_empty() {
            pr_memory::default_memory_db_path().display().to_string()
        } else {
            self.clone()
        }
    }
}

/// Open the session database for history commands.
fn open_history_db(output: Option<String>) -> anyhow::Result<pr_persistence::SessionHistory> {
    let config = AppConfig::load()?;
    let dir = output
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(&config.output.dir));
    let db_path = dir.join(".research.db");
    let db = Arc::new(Persistence::open(&db_path)?);
    Ok(pr_persistence::SessionHistory::new(db))
}

/// `fathom sessions ...` — browse the session history.
fn cmd_sessions(output: Option<String>, action: SessionsAction) -> anyhow::Result<()> {
    let history = open_history_db(output)?;
    match action {
        SessionsAction::List { limit, search } => {
            let rows = match search {
                Some(q) => history.search_sessions(&q),
                None => history.list_sessions(limit),
            };
            if rows.is_empty() {
                println!("No sessions found.");
                return Ok(());
            }
            for r in rows.iter().take(limit.max(1)) {
                println!(
                    "[{}] {:9} agents={:<3} tokens={:<7} {} — {}",
                    &r.id.0,
                    r.status,
                    r.total_agents,
                    r.total_tokens,
                    &r.created_at[..r.created_at.len().min(16)],
                    r.query.chars().take(60).collect::<String>()
                );
            }
            println!("{} session(s)", rows.len().min(limit.max(1)));
        }
        SessionsAction::Show { id } => {
            // Resolve unique prefix.
            let candidates = history.search_sessions("");
            let matches: Vec<_> = candidates
                .iter()
                .filter(|s| s.id.0.starts_with(&id))
                .collect();
            if matches.is_empty() {
                anyhow::bail!("no session with id/prefix '{id}'");
            }
            if matches.len() > 1 {
                anyhow::bail!("ambiguous session prefix '{id}' ({} matches)", matches.len());
            }
            let sid = SessionId(matches[0].id.0.clone());
            let Some(details) = history.get_session_details(&sid) else {
                anyhow::bail!("session '{}' has no details", sid.0);
            };
            let s = &details.session;
            println!("Session {}", s.id.0);
            println!("  query:   {}", s.query);
            println!("  status:  {}", s.status);
            println!("  created: {}", s.created_at);
            println!("  agents:  {} | tokens: {}", s.total_agents, s.total_tokens);
            if let Some(dir) = &s.output_dir {
                println!("  output:  {dir}");
            }
            if !details.agents.is_empty() {
                println!("\n  Agents:");
                for a in &details.agents {
                    println!(
                        "    [{}] {:10} depth={} tokens={} — {}",
                        &a.id,
                        a.role,
                        a.depth,
                        a.tokens_used,
                        a.task.chars().take(50).collect::<String>()
                    );
                }
            }
            if !details.findings.is_empty() {
                println!("\n  Findings:");
                for f in details.findings.iter().take(20) {
                    println!(
                        "    - {} (conf {:.2})",
                        f.title.chars().take(70).collect::<String>(),
                        f.confidence
                    );
                }
            }
        }
    }
    Ok(())
}

/// Expose the built-in tools as an MCP server over stdio. Only built-ins
/// are listed (re-exporting externally configured MCP servers would create
/// loops); nothing may print to stdout — the protocol owns it.
async fn run_mcp_serve() -> anyhow::Result<()> {
    let config = AppConfig::load()?;

    let registry = Arc::new(pr_tools::ToolRegistry::with_builtins());

    let mut ctx = pr_tools::ToolContext::new(
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        config.search.clone(),
    );
    if let Ok(llm) = pr_llm::build_provider(&config.llm) {
        ctx = ctx.with_llm(llm);
    }
    let (contact_db, crm) = open_contact_attachments(&config).await;
    if let Some(store) = contact_db {
        ctx = ctx.with_contact_db(store);
    }
    if let Some(crm) = crm {
        ctx = ctx.with_crm(crm);
    }
    if let Some(mem) = open_memory(&config) {
        ctx = ctx.with_memory(mem);
    }

    let tool_count = registry.tool_names().len();
    let server = pr_mcp::McpServer::with_executor(registry, Arc::new(ctx));
    tracing::info!("MCP server ready (stdio), {tool_count} tools");
    server.run_stdio().await
}

async fn run_tui(
    initial_query: Option<String>,
    profile: Option<String>,
    replay: Option<String>,
) -> anyhow::Result<()> {
    use pr_tui::{App, app::InputMode};
    use pr_tui::event::{EventHandler, spawn_terminal_reader, spawn_agent_reader};
    
    use crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new();
    if let Some(query) = initial_query {
        app.input = query;
        app.input_mode = InputMode::Insert;
    }

    // Replay mode: load a stored session into the UI instead of a live run.
    if let Some(session_ref) = replay {
        match open_history_db(None) {
            Ok(history) => {
                let found = history
                    .list_sessions(200)
                    .into_iter()
                    .find(|s| s.id.0 == session_ref || s.id.0.starts_with(&session_ref));
                match found {
                    Some(summary) => {
                        if let Some(details) = history.get_session_details(&summary.id) {
                            app.load_replay(&details);
                        }
                    }
                    None => {
                        eprintln!("Session '{session_ref}' not found in history");
                    }
                }
            }
            Err(e) => eprintln!("Session history unavailable: {e}"),
        }
    }

    // Attach the long-term memory store (if enabled) for the Memory panel.
    if let Ok(config) = AppConfig::load() {
        app.memory = open_memory(&config);
        if let Some(mem) = &app.memory {
            app.memory_snapshot = pr_tui::MemorySnapshot::refresh(mem);
        }
    }

    // Create event channels
    let (event_handler, event_tx) = EventHandler::new();
    
    // Spawn terminal event reader
    spawn_terminal_reader(event_tx.clone());

    // Create agent event channel (will be connected when research starts)
    let (agent_event_tx, agent_event_rx) = broadcast::channel(1024);
    spawn_agent_reader(event_tx.clone(), agent_event_rx);

    // Jobs registry for the Jobs panel (best-effort; empty panel on failure).
    let jobs_db = open_jobs_db().ok();

    // Main event loop
    let result = run_tui_loop(
        &mut terminal,
        &mut app,
        event_handler,
        event_tx,
        agent_event_tx,
        jobs_db.as_ref(),
        profile,
    )
    .await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_tui_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut pr_tui::App,
    mut event_handler: pr_tui::event::EventHandler,
    _event_tx: tokio::sync::mpsc::UnboundedSender<pr_tui::event::AppEvent>,
    agent_event_tx: tokio::sync::broadcast::Sender<pr_core::AgentEvent>,
    jobs_db: Option<&pr_persistence::JobsDb>,
    profile: Option<String>,
) -> anyhow::Result<()> {
    let mut research_task: Option<tokio::task::JoinHandle<()>> = None;
    // Operator control plane for the active session (created per session).
    let mut control_rx: Option<(
        tokio::sync::mpsc::UnboundedReceiver<pr_agent::QuestionRequest>,
        tokio::sync::mpsc::UnboundedReceiver<pr_agent::ApprovalRequest>,
    )> = None;
    let mut jobs_refresh = tokio::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(2))
        .unwrap_or_else(tokio::time::Instant::now);

    loop {
        // Drain the operator control plane: surface pending questions and
        // approval gates to the UI.
        if let Some((q_rx, a_rx)) = &mut control_rx {
            while let Ok(req) = q_rx.try_recv() {
                app.pending_question = Some(pr_tui::PendingQuestion {
                    request_id: req.request_id,
                    agent_id: req.agent_id,
                    question: req.question,
                    reply: req.reply,
                });
            }
            while let Ok(req) = a_rx.try_recv() {
                app.pending_approval = Some(pr_tui::PendingApproval {
                    request_id: req.request_id,
                    agent_id: req.agent_id,
                    tool: req.tool,
                    args_preview: req.args_preview,
                    reply: req.reply,
                });
            }
        }

        // Refresh the Jobs panel at most every 2 seconds.
        if let Some(db) = jobs_db {
            if jobs_refresh.elapsed() >= std::time::Duration::from_secs(2) {
                if let Ok(rows) = db.list() {
                    app.jobs = rows;
                }
                // Memory panel refreshes on the same cadence.
                if let Some(mem) = &app.memory {
                    app.memory_snapshot = pr_tui::MemorySnapshot::refresh(mem);
                }
                jobs_refresh = tokio::time::Instant::now();
            }
        }

        // Draw
        terminal.draw(|f| pr_tui::ui::draw(f, app))?;

        // Handle events
        if let Some(event) = event_handler.next().await {
            match event {
                pr_tui::event::AppEvent::Terminal(term_event) => {
                    if let crossterm::event::Event::Key(key) = term_event {
                        app.handle_key(key);
                        
                        // Check if the user submitted input.
                        // No active session -> start one; active session ->
                        // steer it mid-run (fleet E1).
                        if !app.query.is_empty() {
                            if research_task.is_none() {
                                let query = app.query.clone();
                                app.query.clear();

                                let (steer_tx, steer_rx) =
                                    tokio::sync::mpsc::unbounded_channel::<String>();
                                app.steer_tx = Some(steer_tx);

                                // Operator control plane for this session.
                                let (q_tx, q_rx) =
                                    tokio::sync::mpsc::unbounded_channel::<pr_agent::QuestionRequest>();
                                let (a_tx, a_rx) =
                                    tokio::sync::mpsc::unbounded_channel::<pr_agent::ApprovalRequest>();
                                control_rx = Some((q_rx, a_rx));

                                let tx = agent_event_tx.clone();
                                let prof = profile.clone();
                                research_task = Some(tokio::spawn(async move {
                                    if let Err(e) = run_research_with_events(
                                        query,
                                        None,
                                        tx,
                                        Some(steer_rx),
                                        Some((q_tx, a_tx)),
                                        prof,
                                    )
                                    .await
                                    {
                                        eprintln!("Research error: {e}");
                                    }
                                }));
                            } else if let Some(steer_tx) = &app.steer_tx {
                                let msg = app.query.clone();
                                app.query.clear();
                                if steer_tx.send(msg).is_ok() {
                                    app.event_log.push(pr_tui::EventLogEntry {
                                        time: chrono::Local::now(),
                                        message: "steering instruction sent to the running session".to_string(),
                                        level: pr_tui::LogLevel::Info,
                                    });
                                }
                            }
                        }
                    }
                }
                pr_tui::event::AppEvent::Agent(agent_event) => {
                    app.handle_agent_event(agent_event);
                }
                pr_tui::event::AppEvent::Quit => {
                    app.should_quit = true;
                }
                _ => {}
            }
        }

        // Reap a finished research task so the NEXT query starts a new
        // session instead of falling into the (dead) steering channel.
        if let Some(task) = &research_task {
            if task.is_finished() {
                let _ = research_task.take();
                app.steer_tx = None;
                control_rx = None;
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Wait for research task to complete
    if let Some(task) = research_task {
        let _ = task.await;
    }

    Ok(())
}

/// Load a persona/profile and apply its overrides onto the config.
/// Returns the profile's system-prompt block (when non-empty) so the caller
/// can inject it into every agent of the session.
fn apply_profile(
    config: &mut AppConfig,
    profile: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let Some(name) = profile else {
        return Ok(None);
    };
    let p = pr_core::profile::load(name)?;
    p.apply(config);
    println!("👤 Profile '{}': {}", p.name, p.description);
    Ok(Some(p.prompt).filter(|s| !s.trim().is_empty()))
}

// ── Watch diff (scheduled harvesting) ───────────────────────────────────────

/// Identity keys of a contact for watch-diff purposes: normalized email
/// and/or phone. Contacts with neither fall back to a name+company key so
/// brand-new person entries still show up in diffs.
fn contact_keys(c: &pr_core::Contact) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(e) = c.normalized_email() {
        keys.push(format!("email:{e}"));
    }
    if let Some(p) = c.normalized_phone() {
        keys.push(format!("phone:{p}"));
    }
    if keys.is_empty() {
        let name = c.name.as_deref().unwrap_or("?").to_lowercase();
        let company = c.company.as_deref().unwrap_or("?").to_lowercase();
        keys.push(format!("person:{name}@{company}"));
    }
    keys
}

/// All identity keys of a contact list.
fn contact_key_set(contacts: &[pr_core::Contact]) -> std::collections::HashSet<String> {
    contacts.iter().flat_map(contact_keys).collect()
}

/// Pure diff: contacts from `current` whose identity keys are all absent
/// from `previous_keys` (a contact counts as new only when none of its
/// keys were known before).
fn watch_new_contacts<'a>(
    previous_keys: &std::collections::HashSet<String>,
    current: &'a [pr_core::Contact],
) -> Vec<&'a pr_core::Contact> {
    current
        .iter()
        .filter(|c| {
            let keys = contact_keys(c);
            !keys.is_empty() && keys.iter().all(|k| !previous_keys.contains(k))
        })
        .collect()
}

/// Render a short human-readable watch report (used for stdout and alert
/// bodies).
fn watch_report(query: &str, new_contacts: &[&pr_core::Contact]) -> String {
    let mut out = format!(
        "Watch diff for \"{}\": {} new contact(s)",
        query,
        new_contacts.len()
    );
    for c in new_contacts.iter().take(20) {
        let mut bits: Vec<String> = Vec::new();
        if let Some(n) = &c.name {
            bits.push(n.clone());
        }
        if let Some(t) = &c.title {
            bits.push(format!("({t})"));
        }
        if let Some(comp) = &c.company {
            bits.push(format!("@ {comp}"));
        }
        if let Some(e) = &c.email {
            bits.push(format!("<{e}>"));
        }
        if let Some(p) = &c.phone {
            bits.push(p.clone());
        }
        out.push_str(&format!("\n  + {}", bits.join(" ")));
    }
    if new_contacts.len() > 20 {
        out.push_str(&format!("\n  … and {} more", new_contacts.len() - 20));
    }
    out
}

async fn run_research_with_events(
    query: String,
    output: Option<String>,
    event_tx: tokio::sync::broadcast::Sender<pr_core::AgentEvent>,
    steer_rx: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
    control: Option<(pr_agent::QuestionTx, pr_agent::ApprovalTx)>,
    profile: Option<String>,
) -> anyhow::Result<()> {
    let started_at = chrono::Utc::now();
    let mut config = AppConfig::load()?;
    let profile_prompt = apply_profile(&mut config, profile.as_deref())?;

    let llm = pr_llm::build_provider(&config.llm)?;

    let output_dir = output
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(&config.output.dir));

    let session_id = SessionId::new();

    let tools = build_registry(&config).await;
    let db_path = output_dir.join(".research.db");
    std::fs::create_dir_all(&output_dir)?;
    let db = Arc::new(Persistence::open(&db_path)?);

    db.create_session(&session_id, &query)?;
    db.set_session_output_dir(&session_id, &output_dir.display().to_string())?;

    let (contact_db, crm) = open_contact_attachments(&config).await;
    let memory = open_memory(&config);

    let final_config = config.clone();
    let query_for_failure = query.clone();
    let mut coordinator = Coordinator::new(
        session_id,
        query,
        llm,
        tools,
        event_tx,
        db,
        output_dir,
        config,
    );
    if let Some(rx) = steer_rx {
        coordinator = coordinator.with_steer_rx(rx);
    }
    if let Some((q_tx, a_tx)) = control {
        coordinator = coordinator.with_control_plane(q_tx, a_tx);
    }
    if let Some(store) = contact_db {
        coordinator = coordinator.with_contact_db(store);
    }
    if let Some(crm) = crm {
        coordinator = coordinator.with_crm(crm);
    }
    if let Some(mem) = memory {
        coordinator = coordinator.with_memory(mem);
    }
    if let Some(prompt) = profile_prompt {
        coordinator = coordinator.with_profile_prompt(prompt);
    }

    let output = match coordinator.execute().await {
        Ok(out) => out,
        Err(e) => {
            notify_failure(&final_config, &query_for_failure, &e).await;
            return Err(e);
        }
    };
    finalize_session(&output, &final_config, started_at).await;
    Ok(())
}

async fn run_research(
    query: String,
    output: Option<String>,
    profile: Option<String>,
) -> anyhow::Result<()> {
    let started_at = chrono::Utc::now();
    let mut config = AppConfig::load()?;

    // Apply a persona/profile on top of the config, when requested.
    let profile_prompt = apply_profile(&mut config, profile.as_deref())?;

    if config.llm.api_key.is_empty() {
        anyhow::bail!(
            "No API key configured. Set it in ~/.fathom/config.toml:\n\
            [llm]\napi_key = \"your-key\""
        );
    }

    let output_dir = output
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(&config.output.dir));

    let session_id = SessionId::new();

    println!("╔══════════════════════════════════════════╗");
    println!("║        Fathom Agent           ║");
    println!("╚══════════════════════════════════════════╝");
    println!();
    println!("  Query:  {}", query);
    println!("  Model:  {} ({})", config.llm.model, config.llm.provider);
    println!("  Output: {}", output_dir.display());
    println!("  Session: {}", session_id);
    println!();

    let llm = pr_llm::build_provider(&config.llm)?;

    let tools = build_registry(&config).await;
    let (event_tx, _) = broadcast::channel(1024);

    let db_path = output_dir.join(".research.db");
    std::fs::create_dir_all(&output_dir)?;
    let db = Arc::new(Persistence::open(&db_path)?);

    db.create_session(&session_id, &query)?;
    db.set_session_output_dir(&session_id, &output_dir.display().to_string())?;

    let (contact_db, crm) = open_contact_attachments(&config).await;
    let memory = open_memory(&config);
    if let Some(store) = &contact_db {
        println!("  👤 Contacts: {} ({})", config.contacts.db_path, store.backend());
    }
    if config.crm.is_configured() {
        println!("  🔄 CRM sync: {}", config.crm.provider);
    }
    if memory.is_some() {
        println!("  🧠 Long-term memory: enabled");
    }

    // Subscribe to events for live output
    let mut event_rx = event_tx.subscribe();
    let event_handle = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match &event {
                pr_core::AgentEvent::AgentSpawned { id, task, depth, .. } => {
                    println!("  🚀 Agent {} spawned (depth {}): {}", id, depth, task.chars().take(60).collect::<String>());
                }
                pr_core::AgentEvent::ToolCallStarted { agent_id, tool, .. } => {
                    println!("  🔧 [{}] calling: {}", agent_id, tool);
                }
                pr_core::AgentEvent::ToolCallCompleted { tool, duration_ms, .. } => {
                    println!("  ✅ {} completed ({}ms)", tool, duration_ms);
                }
                pr_core::AgentEvent::AgentCompleted { id, tokens_used, .. } => {
                    println!("  🏁 Agent {} completed ({} tokens)", id, tokens_used);
                }
                pr_core::AgentEvent::AgentFailed { id, error } => {
                    println!("  ❌ Agent {} failed: {}", id, error);
                }
                pr_core::AgentEvent::SessionCompleted { total_tokens, total_agents, .. } => {
                    println!();
                    println!("  ══════════════════════════════════════");
                    println!("  ✅ Session completed!");
                    println!("     Agents: {}", total_agents);
                    println!("     Tokens: {}", total_tokens);
                }
                _ => {}
            }
        }
    });

    let final_config = config.clone();
    let query_for_failure = query.clone();
    let mut coordinator = Coordinator::new(
        session_id.clone(),
        query,
        llm,
        tools,
        event_tx,
        db,
        output_dir.clone(),
        config,
    );
    if let Some(store) = contact_db {
        coordinator = coordinator.with_contact_db(store);
    }
    if let Some(crm) = crm {
        coordinator = coordinator.with_crm(crm);
    }
    if let Some(mem) = memory {
        coordinator = coordinator.with_memory(mem);
    }
    if let Some(prompt) = profile_prompt.clone() {
        coordinator = coordinator.with_profile_prompt(prompt);
    }

    let result = coordinator.execute().await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    event_handle.abort();

    match result {
        Ok(output) => {
            println!();
            println!("  📁 Results: {}", output.output_dir.display());
            println!("  📝 Summary preview:");
            println!("  {}", output.synthesis.lines().take(5).collect::<Vec<_>>().join("\n  "));
            finalize_session(&output, &final_config, started_at).await;
        }
        Err(e) => {
            eprintln!("  ❌ Research failed: {e}");
            notify_failure(&final_config, &query_for_failure, &e).await;
            return Err(e);
        }
    }

    Ok(())
}

/// Best-effort failure alert on the configured notification channels
/// (`[notifications]`). Never propagates errors.
async fn notify_failure(config: &AppConfig, query: &str, err: &impl std::fmt::Display) {
    let notifier = pr_core::Notifier::from_config(&config.notifications);
    if notifier.is_empty() {
        return;
    }
    let text = format!("Query: {query}\nError: {err}");
    notifier
        .notify_alert("session.failed", &format!("Research session failed: {query}"), &text)
        .await;
}

/// Post-processing after a successful research run: export the report in the
/// configured format, export any contacts saved during the run, and deliver
/// completion notifications. All steps are best-effort — failures are
/// reported but never fail the run itself.
async fn finalize_session(
    output: &pr_core::SessionOutput,
    config: &AppConfig,
    started_at: chrono::DateTime<chrono::Utc>,
) {
    // Export the report.
    let format = config.export.parsed_format();
    let exporter = pr_core::Exporter::new(output.output_dir.clone());
    match exporter.export(output, format).await {
        Ok(path) => println!("  📦 Exported report ({format}): {}", path.display()),
        Err(e) => eprintln!("  ⚠️  Export failed: {e}"),
    }

    // Export contacts saved during THIS run only (CSV next to the report) —
    // dumping the whole global DB into every session dir leaks other runs.
    if let Ok(store) = pr_persistence::open_contact_store(&config.contacts).await {
        match store.list_all(i64::MAX, 0).await {
            Ok(all) => {
                let contacts: Vec<pr_core::Contact> = all
                    .into_iter()
                    .filter(|c| c.created_at >= started_at)
                    .collect();
                if !contacts.is_empty() {
                    match exporter
                        .export_contacts(&contacts, pr_core::ContactExportFormat::Csv)
                        .await
                    {
                        Ok(path) => println!(
                            "  👤 Exported {} contact(s): {}",
                            contacts.len(),
                            path.display()
                        ),
                        Err(e) => eprintln!("  ⚠️  Contact export failed: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("  ⚠️  Contact listing failed: {e}"),
        }
    }

    // Yield report: how many contacts the run produced vs the whole DB
    // (measurement baseline for lead-gen improvements, fleet report C9).
    if let Ok(store) = pr_persistence::open_contact_store(&config.contacts).await {
        if let Ok(total) = store.count().await {
            println!("  📈 Yield: contact database now holds {total} contact(s)");
        }
    }

    // Send completion notifications, if any channels are configured.
    let notifier = pr_core::Notifier::from_config(&config.notifications);
    if !notifier.is_empty() {
        match notifier.notify_completion(output).await {
            Ok(()) => println!("  🔔 Completion notifications sent"),
            Err(e) => eprintln!("  ⚠️  Notification delivery failed: {e}"),
        }
    }

    // Post-session absorb (Memora agent-workflow): persist what this session
    // concluded so future sessions can see it was already researched.
    if config.memory.enabled {
        match pr_memory::Memory::open(&config.memory, &config.llm) {
            Ok(mem) => {
                let preview = output.synthesis_preview(400);
                if !preview.trim().is_empty() {
                    let req = pr_memory::AbsorbRequest {
                        facts: vec![pr_memory::AbsorbFact {
                            content: format!(
                                "Research session concluded ({} agent(s), {} tokens): {}",
                                output.total_agents, output.total_tokens, preview
                            ),
                            metadata: serde_json::json!({ "type": "session-summary" }),
                            tags: vec!["session-summary".to_string()],
                            confidence: Some(0.7),
                            memory_class: None,
                        }],
                        source: format!("session:{}", output.session_id),
                        scope: pr_memory::Scope::Agent,
                        scope_key: String::new(),
                        context: None,
                        dry_run: false,
                    };
                    match mem.pipeline().absorb(req).await {
                        Ok(report) => println!("  🧠 Memory: {}", report.summary_line()),
                        Err(e) => eprintln!("  ⚠️  Memory absorb failed: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("  ⚠️  Memory store unavailable: {e}"),
        }
    }
}

async fn run_worker(
    session_id: String,
    agent_id: String,
    task: String,
    socket_path: String,
    role: String,
) -> anyhow::Result<()> {
    use pr_agent::ipc::{agent_event_to_ipc, IpcMessage};
    use pr_agent::AgentRuntime;
    use pr_core::{AgentId, AgentRecord, AgentState, AgentStatus};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixListener;

    let agent_id = AgentId(agent_id);
    let role = match role.to_lowercase().as_str() {
        "coordinator" => pr_core::AgentRole::Coordinator,
        "analyst" => pr_core::AgentRole::Analyst,
        "verifier" => pr_core::AgentRole::Verifier,
        "writer" => pr_core::AgentRole::Writer,
        _ => pr_core::AgentRole::Researcher,
    };

    // Create Unix socket listener; the coordinator connects to it.
    let listener = UnixListener::bind(&socket_path)?;
    let (socket, _) = listener.accept().await?;
    drop(listener);

    let (socket_read, socket_write) = socket.into_split();
    let socket_write = Arc::new(tokio::sync::Mutex::new(socket_write));

    /// Write one IPC message to the coordinator socket.
    async fn send(
        w: &tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>,
        msg: &IpcMessage,
    ) -> anyhow::Result<()> {
        let mut writer = w.lock().await;
        writer.write_all(msg.to_line().as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }

    // Load config; report failure over the socket so the coordinator sees a
    // proper Failed message instead of a silent disconnect.
    let config = match AppConfig::load() {
        Ok(c) if !c.llm.api_key.is_empty() => c,
        Ok(_) => {
            let _ = send(
                &socket_write,
                &IpcMessage::Failed {
                    agent_id: agent_id.clone(),
                    error: "worker has no LLM api_key configured".to_string(),
                },
            )
            .await;
            let _ = std::fs::remove_file(&socket_path);
            anyhow::bail!("No API key configured for worker");
        }
        Err(e) => {
            let _ = send(
                &socket_write,
                &IpcMessage::Failed {
                    agent_id: agent_id.clone(),
                    error: format!("worker failed to load config: {e}"),
                },
            )
            .await;
            let _ = std::fs::remove_file(&socket_path);
            return Err(e);
        }
    };

    // The coordinator passes the session output dir via PR_OUTPUT_DIR so the
    // worker opens the same SQLite database.
    let output_dir = std::env::var("PR_OUTPUT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(&config.output.dir));
    std::fs::create_dir_all(&output_dir)?;
    let db = Arc::new(Persistence::open(&output_dir.join(".research.db"))?);

    // Ensure the agent record exists. The coordinator normally creates it
    // before spawning us; ignore the error if it already exists (or if the
    // session row is missing, e.g. when run manually for debugging).
    let _ = db.create_agent(&AgentRecord {
        id: agent_id.clone(),
        session_id: session_id.clone(),
        parent_id: None,
        role,
        task: task.clone(),
        status: AgentStatus::Spawned,
        depth: 1,
        tokens_used: 0,
        created_at: chrono::Utc::now(),
        completed_at: None,
    });

    // Build the runtime.
    let llm = match pr_llm::build_provider(&config.llm) {
        Ok(llm) => llm,
        Err(e) => {
            let _ = send(
                &socket_write,
                &IpcMessage::Failed {
                    agent_id: agent_id.clone(),
                    error: format!("worker has no usable LLM config: {e}"),
                },
            )
            .await;
            let _ = std::fs::remove_file(&socket_path);
            anyhow::bail!("Worker LLM config error: {e}");
        }
    };
    let tools = build_registry(&config).await;
    let (contact_db, crm) = open_contact_attachments(&config).await;
    let memory = open_memory(&config);
    let (event_tx, _) = broadcast::channel(1024);

    // Announce start.
    let _ = send(
        &socket_write,
        &IpcMessage::Progress {
            agent_id: agent_id.clone(),
            state: AgentState::Researching { sub_tasks: vec![] },
        },
    )
    .await;

    // Forward runtime events (tool calls, LLM chunks, state changes) to the
    // coordinator while the agent runs.
    let mut event_rx = event_tx.subscribe();
    let fwd_id = agent_id.clone();
    let fwd_write = socket_write.clone();
    let forwarder = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let Some(msg) = agent_event_to_ipc(&event, &fwd_id) {
                if send(&fwd_write, &msg).await.is_err() {
                    break; // coordinator went away
                }
            }
        }
    });

    // Watch for Cancel messages from the coordinator.
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let cancel_id = agent_id.clone();
    let cancel_write = socket_write.clone();
    let cancel_task = tokio::spawn(async move {
        let mut reader = BufReader::new(socket_read);
        let mut line = String::new();
        let mut cancel_tx = Some(cancel_tx);
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break, // coordinator closed the socket
                Ok(_) => {
                    if matches!(IpcMessage::from_line(&line), Some(IpcMessage::Cancel)) {
                        let _ = send(
                            &cancel_write,
                            &IpcMessage::Failed {
                                agent_id: cancel_id.clone(),
                                error: "cancelled by coordinator".to_string(),
                            },
                        )
                        .await;
                        if let Some(tx) = cancel_tx.take() {
                            let _ = tx.send(());
                        }
                        break;
                    }
                }
            }
        }
    });

    // Run the agent, interruptible by coordinator cancellation.
    // Use the session output dir as the working dir so multiprocess workers
    // behave identically to single-process sub-agents (which are constructed
    // with the coordinator's output_dir as their working directory).
    let working_dir = output_dir.clone();
    // Read before `config` moves into the runtime.
    let timeout_secs = config.agent.timeout_seconds;
    let mut agent = AgentRuntime::new(
        agent_id.clone(),
        SessionId(session_id),
        None,
        role,
        task,
        1,
        llm,
        tools,
        event_tx,
        db.clone(),
        working_dir,
        config,
    );
    agent.contact_db = contact_db;
    agent.crm = crm;
    agent.memory = memory;

    // Workers enforce the same wall-clock timeout as in-process agents —
    // previously a hung LLM call in a worker ran forever.
    let run_fut = async move {
        if timeout_secs == 0 {
            return agent.run().await;
        }
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), agent.run()).await
        {
            Ok(res) => res,
            Err(_) => Err(anyhow::anyhow!("agent timed out after {timeout_secs}s")),
        }
    };

    let cancelled = tokio::select! {
        res = run_fut => {
            // Report the outcome to the coordinator and the database.
            match res {
                Ok(output) => {
                    let _ = db.update_agent_status(
                        &agent_id,
                        AgentStatus::Completed,
                        output.tokens_used,
                        Some(&output.summary),
                    );
                    let _ = send(
                        &socket_write,
                        &IpcMessage::Completed {
                            agent_id: agent_id.clone(),
                            summary: output.summary,
                            // Session accounting includes descendants; the
                            // worker's DB row keeps its own share only.
                            tokens_used: output.tokens_used + output.descendant_tokens,
                        },
                    )
                    .await;
                }
                Err(e) => {
                    let _ = db.update_agent_status(
                        &agent_id,
                        AgentStatus::Failed,
                        0,
                        Some(&e.to_string()),
                    );
                    let _ = send(
                        &socket_write,
                        &IpcMessage::Failed {
                            agent_id: agent_id.clone(),
                            error: e.to_string(),
                        },
                    )
                    .await;
                }
            }
            false
        }
        _ = cancel_rx => true,
    };

    forwarder.abort();
    cancel_task.abort();

    // Cleanup
    let _ = std::fs::remove_file(&socket_path);

    if cancelled {
        tracing::info!("Worker {} cancelled by coordinator", agent_id);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Contacts subcommand
// ---------------------------------------------------------------------------

async fn cmd_contacts(action: ContactsAction) -> anyhow::Result<()> {
    let config = AppConfig::load()?;
    let store = pr_persistence::open_contact_store(&config.contacts).await?;

    match action {
        ContactsAction::List { limit } => {
            let contacts = store.list_all(limit, 0).await?;
            println!(
                "Contact database: {} ({} contacts total)\n",
                config.contacts.db_path,
                store.count().await?
            );
            for c in &contacts {
                let email = c.email.as_deref().unwrap_or("-");
                let phone = c.phone.as_deref().unwrap_or("-");
                let name = c.name.as_deref().unwrap_or("-");
                let title = c.title.as_deref().unwrap_or("");
                let company = c.company.as_deref().unwrap_or("");
                println!(
                    "  #{:<4} {:<28} {:<30} {:<18} {} {}",
                    c.id.unwrap_or(0),
                    name,
                    email,
                    phone,
                    title,
                    company
                );
            }
            if contacts.is_empty() {
                println!("  (empty)");
            }
        }
        ContactsAction::Export { format, output } => {
            let fmt = pr_core::ContactExportFormat::parse(&format)
                .ok_or_else(|| anyhow::anyhow!(
                    "unknown format '{format}' (expected csv, vcf, json or xlsx)"
                ))?;
            let dir = output
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(&config.output.dir));
            let contacts = store.list_all(i64::MAX, 0).await?;
            if contacts.is_empty() {
                println!("No contacts to export");
                return Ok(());
            }
            // Export-time dedup: email / phone / name+domain.
            let (contacts, dropped) = pr_core::dedup_contacts(contacts);
            if dropped > 0 {
                println!("Deduplicated: {dropped} duplicate row(s) skipped");
            }
            let exporter = pr_core::Exporter::new(dir);
            let path = exporter.export_contacts(&contacts, fmt).await?;
            println!("Exported {} contact(s) to {}", contacts.len(), path.display());
        }
        ContactsAction::Dedup { merge } => {
            let contacts = store.list_all(i64::MAX, 0).await?;
            // Group by normalized email / phone.
            let mut groups: std::collections::HashMap<String, Vec<&pr_core::Contact>> =
                std::collections::HashMap::new();
            for c in &contacts {
                if let Some(e) = c.normalized_email() {
                    groups.entry(format!("email:{e}")).or_default().push(c);
                }
                if let Some(p) = c.normalized_phone() {
                    groups.entry(format!("phone:{p}")).or_default().push(c);
                }
            }
            let mut dup_groups: Vec<(String, Vec<&pr_core::Contact>)> = groups
                .into_iter()
                .filter(|(_, v)| v.len() > 1)
                .collect();
            dup_groups.sort_by(|a, b| a.0.cmp(&b.0));
            if dup_groups.is_empty() {
                println!("No duplicates found ({} contacts checked).", contacts.len());
                return Ok(());
            }
            let mut merged_total = 0usize;
            for (key, members) in &dup_groups {
                println!("Duplicate group [{key}]:");
                for m in members {
                    println!(
                        "  #{} {} <{}> {}",
                        m.id.unwrap_or(0),
                        m.name.as_deref().unwrap_or("-"),
                        m.email.as_deref().unwrap_or("-"),
                        m.company.as_deref().unwrap_or("")
                    );
                }
                if merge {
                    // Primary: the row with the most populated fields; the
                    // rest are folded into it (fills blanks, moves extras).
                    let filled = |c: &&pr_core::Contact| {
                        [
                            c.name.is_some(),
                            c.title.is_some(),
                            c.company.is_some(),
                            c.email.is_some(),
                            c.phone.is_some(),
                        ]
                        .iter()
                        .filter(|b| **b)
                        .count()
                    };
                    let mut sorted: Vec<&&pr_core::Contact> = members.iter().collect();
                    sorted.sort_by(|a, b| filled(b).cmp(&filled(a)));
                    let primary = sorted[0];
                    for dup in &sorted[1..] {
                        if let (Some(pid), Some(did)) = (primary.id, dup.id) {
                            match store.merge_contacts(pid, did).await {
                                Ok(()) => merged_total += 1,
                                Err(e) => eprintln!("  merge #{did} → #{pid} failed: {e}"),
                            }
                        }
                    }
                }
            }
            println!(
                "{} duplicate group(s){}.",
                dup_groups.len(),
                if merge {
                    format!(" — merged {merged_total} row(s)")
                } else {
                    " (dry run; re-run with --merge to fold them)".to_string()
                }
            );
        }
        ContactsAction::PushCrm => {
            let crm = pr_core::CrmSync::from_config(&config.crm)
                .ok_or_else(|| anyhow::anyhow!(
                    "CRM is not configured ([crm] provider/domain/api_key in config.toml)"
                ))?;
            let contacts = store.list_all(i64::MAX, 0).await?;
            if contacts.is_empty() {
                println!("No contacts to push");
                return Ok(());
            }
            println!(
                "Pushing {} contact(s) to {} ...",
                contacts.len(),
                crm.provider().name()
            );
            let mut ok = 0usize;
            let mut skipped = 0usize;
            for c in &contacts {
                // Contacts pushed earlier carry a crm_id — skip to avoid
                // creating duplicates in the CRM.
                if c.crm_id.is_some() {
                    skipped += 1;
                    continue;
                }
                match crm.push_contact(c).await {
                    Ok(id) => {
                        ok += 1;
                        if let Some(cid) = c.id {
                            let _ = store.set_crm_id(cid, &id).await;
                        }
                        println!("  ✅ {}: CRM id {id}", c.display_label());
                    }
                    Err(e) => println!("  ❌ {}: {e}", c.display_label()),
                }
            }
            println!(
                "Done: {ok} pushed, {skipped} already synced (of {} total)",
                contacts.len()
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Resume subcommand
// ---------------------------------------------------------------------------

async fn cmd_resume(output: Option<String>, session_id: Option<String>) -> anyhow::Result<()> {
    let started_at = chrono::Utc::now();
    let config = AppConfig::load()?;

    let output_dir = output
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(&config.output.dir));
    let db_path = output_dir.join(".research.db");
    anyhow::ensure!(
        db_path.exists(),
        "no session database at {} (use --output <dir>)",
        db_path.display()
    );
    let db = Arc::new(Persistence::open(&db_path)?);

    let resumer = pr_agent::SessionResumer::new(db.clone());

    // Locate the session to resume.
    let session_id = match session_id {
        Some(id) => SessionId(id),
        None => {
            let interrupted = resumer.find_interrupted_sessions();
            if interrupted.is_empty() {
                println!("No interrupted sessions found in {}", db_path.display());
                return Ok(());
            }
            println!("Interrupted sessions:");
            for s in &interrupted {
                println!(
                    "  {} — \"{}\" ({} agents, {} completed) — updated {}",
                    s.session_id,
                    s.query,
                    s.total_agents,
                    s.completed_agents,
                    s.updated_at
                );
            }
            let latest = interrupted
                .into_iter()
                .max_by_key(|s| s.updated_at)
                .expect("checked non-empty");
            println!("\nResuming most recent: {}", latest.session_id);
            latest.session_id
        }
    };

    // Only interrupted (still "running") sessions may be resumed — resuming
    // a completed/cancelled one would clobber its accounting and outputs.
    let row = db
        .get_session(&session_id)?
        .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
    anyhow::ensure!(
        row.status == "running",
        "session {session_id} has status '{}' — only interrupted (running) sessions can be resumed",
        row.status
    );
    // Atomic claim: two concurrent resumers cannot both proceed.
    anyhow::ensure!(
        db.claim_session_for_resume(&session_id)?,
        "session {session_id} was claimed by another resume process"
    );

    let state = resumer.resume_session(&session_id).await?;
    println!(
        "  Recovered {} completed agent(s), {} pending task(s)",
        state.completed_agents.len(),
        state.pending_tasks.len()
    );

    let llm = pr_llm::build_provider(&config.llm)?;
    let tools = build_registry(&config).await;
    let (event_tx, _) = broadcast::channel(1024);
    let (contact_db, crm) = open_contact_attachments(&config).await;
    let memory = open_memory(&config);

    let mut coordinator = Coordinator::new(
        session_id.clone(),
        state.query.clone(),
        llm,
        tools,
        event_tx,
        db,
        output_dir.clone(),
        config.clone(),
    );
    if let Some(store) = contact_db {
        coordinator = coordinator.with_contact_db(store);
    }
    if let Some(crm) = crm {
        coordinator = coordinator.with_crm(crm);
    }
    if let Some(mem) = memory {
        coordinator = coordinator.with_memory(mem);
    }

    let result = coordinator.execute_resume(state).await?;
    println!(
        "\n✅ Session {} resumed and completed\n  📁 Results: {}",
        session_id,
        result.output_dir.display()
    );
    // Export contacts saved across the WHOLE session (the interrupted first
    // half too), so use the session's original created_at.
    let session_started = chrono::DateTime::parse_from_rfc3339(&row.created_at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or(started_at);
    finalize_session(&result, &config, session_started).await;

    Ok(())
}

// ---------------------------------------------------------------------------
// Jobs subcommand (durable background jobs with self-healing retries)
// ---------------------------------------------------------------------------

fn open_jobs_db() -> anyhow::Result<pr_persistence::JobsDb> {
    pr_persistence::JobsDb::open(&pr_persistence::default_jobs_db_path())
}

fn resolve_job(db: &pr_persistence::JobsDb, id: &str) -> anyhow::Result<pr_persistence::JobRow> {
    db.get(id)?
        .ok_or_else(|| anyhow::anyhow!("job not found: {id}"))
}

fn short_id(id: &str) -> &str {
    id.split('-').next().unwrap_or(id)
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    let chars: Vec<char> = first_line.chars().take(max + 1).collect();
    if chars.len() > max {
        let mut out: String = chars[..max].iter().collect();
        out.push('…');
        out
    } else {
        chars.iter().collect()
    }
}

fn print_job_status(job: &pr_persistence::JobRow) {
    let status = match (job.status.as_str(), job.pid.map(pr_persistence::pid_alive)) {
        ("running", Some(true)) => format!("running (pid {})", job.pid.unwrap_or(0)),
        ("running", Some(false)) => "running, but the process is gone (crashed or killed)"
            .to_string(),
        (s, _) => s.to_string(),
    };
    println!("Job:       {} ({})", short_id(&job.id), job.id);
    println!("Task:      {}", truncate(&job.task, 100));
    println!("Status:    {status}");
    println!("Attempts:  {}/{}", job.attempt, job.max_attempts);
    println!("Created:   {}", job.created_at);
    if let Some(t) = &job.started_at {
        println!("Started:   {t}");
    }
    if let Some(t) = &job.completed_at {
        println!("Completed: {t}");
    }
    println!("Output:    {}", job.output_dir);
    println!(
        "Log:       {}",
        std::path::Path::new(&job.output_dir).join("job.log").display()
    );
    if let Some(err) = &job.error {
        println!("Error:     {}", truncate(err, 400));
    }
}

/// `fathom profiles ...` — manage personas/profiles.
fn cmd_profiles(action: ProfilesAction) -> anyhow::Result<()> {
    match action {
        ProfilesAction::List => {
            let profiles = pr_core::profile::list_all();
            if profiles.is_empty() {
                println!("No profiles available.");
                return Ok(());
            }
            println!("{:<12} {}", "NAME", "DESCRIPTION");
            for p in &profiles {
                let builtin = pr_core::profile::built_in(&p.name).is_some()
                    && !pr_core::profile::profiles_dir()
                        .join(format!("{}.toml", p.name))
                        .exists();
                let mark = if builtin { " (built-in)" } else { "" };
                println!("{:<12} {}{}", p.name, p.description, mark);
            }
            println!();
            println!("Use: fathom run --profile <name> \"...\"");
        }
        ProfilesAction::Show { name } => {
            let p = pr_core::profile::load(&name)?;
            println!("Profile:     {}", p.name);
            println!("Description: {}", p.description);
            if let Some(m) = &p.model {
                println!("Model:       {m}");
            }
            if let Some(m) = &p.fast_model {
                println!("Fast model:  {m}");
            }
            if let Some(t) = p.temperature {
                println!("Temperature: {t}");
            }
            if let Some(d) = p.max_depth {
                println!("Max depth:   {d}");
            }
            if let Some(n) = p.max_agents {
                println!("Max agents:  {n}");
            }
            if !p.deny_tools.is_empty() {
                println!("Deny tools:  {}", p.deny_tools.join(", "));
            }
            println!();
            println!("{}", p.prompt);
        }
        ProfilesAction::New { name } => {
            if pr_core::profile::built_in(&name).is_some() {
                anyhow::bail!("'{name}' is a built-in profile; choose another name");
            }
            let dir = pr_core::profile::profiles_dir();
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{name}.toml"));
            if path.exists() {
                anyhow::bail!("{} already exists", path.display());
            }
            std::fs::write(&path, pr_core::Profile::template(&name))?;
            println!("Created {}. Edit it, then run with --profile {name}.", path.display());
        }
    }
    Ok(())
}

async fn cmd_jobs(action: JobsAction) -> anyhow::Result<()> {
    match action {
        JobsAction::Submit { task, attempts } => cmd_jobs_submit(task, attempts).await,
        JobsAction::List => cmd_jobs_list(),
        JobsAction::Status { id, watch } => cmd_jobs_status(id, watch).await,
        JobsAction::Logs { id, lines } => cmd_jobs_logs(id, lines),
        JobsAction::Cancel { id } => cmd_jobs_cancel(id),
        JobsAction::Rerun { id } => cmd_jobs_rerun(id),
    }
}

async fn cmd_jobs_submit(task: String, attempts: i64) -> anyhow::Result<()> {
    anyhow::ensure!(attempts >= 1, "--attempts must be >= 1");
    let db = open_jobs_db()?;
    let job = db.create(&task, attempts, "")?;
    let job_dir = pr_persistence::default_jobs_root().join(&job.id);
    std::fs::create_dir_all(&job_dir)?;
    db.set_output_dir(&job.id, &job_dir.display().to_string())?;

    let log_path = job_dir.join("job.log");
    let exe = std::env::current_exe()?;
    pr_persistence::spawn_detached_runner(&exe, &job.id, Some(&log_path))?;

    println!("Submitted job {}", short_id(&job.id));
    println!("  Task:     {}", truncate(&task, 80));
    println!("  Attempts: {attempts}");
    println!("  Dir:      {}", job_dir.display());
    println!("  Log:      {}", log_path.display());
    println!();
    println!("  Watch it live:  fathom jobs status {} --watch 5", short_id(&job.id));
    println!("  Tail the log:   fathom jobs logs {}", short_id(&job.id));
    Ok(())
}

fn cmd_jobs_list() -> anyhow::Result<()> {
    let db = open_jobs_db()?;
    let jobs = db.list()?;
    if jobs.is_empty() {
        println!("No jobs yet. Submit one with: fathom jobs submit \"<task>\"");
        return Ok(());
    }
    println!(
        "{:<10} {:<11} {:<9} {:<20} TASK",
        "ID", "STATUS", "ATTEMPT", "CREATED"
    );
    for j in jobs {
        let status = match (j.status.as_str(), j.pid.map(pr_persistence::pid_alive)) {
            ("running", Some(false)) => "stale",
            (s, _) => s,
        };
        println!(
            "{:<10} {:<11} {}/{} {:<20} {}",
            short_id(&j.id),
            status,
            j.attempt,
            j.max_attempts,
            j.created_at.chars().take(19).collect::<String>(),
            truncate(&j.task, 60)
        );
    }
    Ok(())
}

async fn cmd_jobs_status(id: String, watch: Option<u64>) -> anyhow::Result<()> {
    loop {
        let db = open_jobs_db()?;
        let job = resolve_job(&db, &id)?;
        print_job_status(&job);
        if job.is_terminal() || watch.is_none() {
            return Ok(());
        }
        let secs = watch.unwrap_or(5).clamp(1, 3600);
        tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
        println!("{}", "─".repeat(60));
    }
}

fn cmd_jobs_logs(id: String, lines: usize) -> anyhow::Result<()> {
    let db = open_jobs_db()?;
    let job = resolve_job(&db, &id)?;
    let log_path = std::path::Path::new(&job.output_dir).join("job.log");
    let content = std::fs::read_to_string(&log_path)
        .map_err(|e| anyhow::anyhow!("cannot read log {}: {e}", log_path.display()))?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    if start > 0 {
        println!("… ({start} earlier line(s) omitted)");
    }
    for line in &all[start..] {
        println!("{line}");
    }
    Ok(())
}

fn cmd_jobs_cancel(id: String) -> anyhow::Result<()> {
    let db = open_jobs_db()?;
    let job = resolve_job(&db, &id)?;
    if job.is_terminal() {
        println!("Job {} is already {}", short_id(&job.id), job.status);
        return Ok(());
    }
    if let Some(pid) = job.pid {
        if pr_persistence::pid_alive(pid) {
            pr_persistence::terminate_pid(pid);
        }
    }
    db.mark_cancelled(&job.id)?;
    println!("Cancelled job {}", short_id(&job.id));
    Ok(())
}

/// Re-run a failed/cancelled/completed job, or a stale one (marked running
/// but the process is gone). Resets attempt counters and error, then spawns
/// a fresh detached runner.
fn cmd_jobs_rerun(id: String) -> anyhow::Result<()> {
    let db = open_jobs_db()?;
    let job = resolve_job(&db, &id)?;
    let reset = match job.status.as_str() {
        "queued" => {
            println!("Job {} is already queued", short_id(&job.id));
            false
        }
        "running" => match job.pid {
            Some(pid) if !pr_persistence::pid_alive(pid) => {
                println!("Job {} is stale (pid {pid} is gone); resetting", short_id(&job.id));
                db.reset_running_with_pid(&job.id, pid)?
            }
            _ => anyhow::bail!(
                "job {} is still running; cancel it first if you want to restart",
                short_id(&job.id)
            ),
        },
        _ => db.reset_for_rerun(&job.id)?,
    };
    if !reset {
        anyhow::bail!("job {} cannot be re-run from state '{}'", short_id(&job.id), job.status);
    }

    let job_dir = std::path::PathBuf::from(&job.output_dir);
    std::fs::create_dir_all(&job_dir)?;
    let log_path = job_dir.join("job.log");
    let exe = std::env::current_exe()?;
    pr_persistence::spawn_detached_runner(&exe, &job.id, Some(&log_path))?;

    println!("Re-run started for job {}", short_id(&job.id));
    println!("  Watch it live:  fathom jobs status {} --watch 5", short_id(&job.id));
    Ok(())
}

/// Retry-task augmentation: from the second attempt on, the agent receives
/// the original task PLUS the previous failure, so it can diagnose the
/// partial workspace and fix its own mistake instead of blindly rerunning.
fn augment_task_for_retry(task: &str, prev_error: Option<&str>) -> String {
    let Some(err) = prev_error else {
        return task.to_string();
    };
    format!(
        "{task}\n\n---\n\
         The previous attempt to complete this task FAILED with the following error:\n\
         {err}\n\n\
         The output directory contains partial artifacts from that attempt. \
         Inspect them, diagnose the root cause of the failure, fix the problem, \
         and finish the original task."
    )
}

/// Background job runner. Loops over attempts until the task succeeds,
/// attempts are exhausted, or the job is cancelled. Runs detached (spawned
/// by `jobs submit` with stdout/stderr redirected into <job_dir>/job.log).
async fn cmd_job_run(id: String) -> anyhow::Result<()> {
    let db = open_jobs_db()?;
    let job = resolve_job(&db, &id)?;
    anyhow::ensure!(!job.output_dir.is_empty(), "job {id} has no output dir yet");
    let out_dir = job.output_dir.clone();

    println!(
        "══ Job {} starting (max {} attempt(s)) ══",
        short_id(&job.id),
        job.max_attempts
    );

    for attempt in 1..=job.max_attempts {
        let fresh = db
            .get(&job.id)?
            .ok_or_else(|| anyhow::anyhow!("job row disappeared mid-run"))?;
        if fresh.status == "cancelled" {
            println!("Job cancelled before attempt {attempt}; stopping.");
            return Ok(());
        }
        db.mark_running(&job.id, attempt, std::process::id() as i64)?;

        let task = if attempt == 1 {
            job.task.clone()
        } else {
            augment_task_for_retry(&job.task, fresh.error.as_deref())
        };

        println!();
        println!("══ Attempt {}/{} ══", attempt, job.max_attempts);
        match run_research(task, Some(out_dir.clone()), None).await {
            Ok(()) => {
                db.mark_completed(&job.id)?;
                println!();
                println!("✅ Job {} completed on attempt {attempt}", short_id(&job.id));
                return Ok(());
            }
            Err(e) => {
                let err = format!("{e:#}");
                eprintln!("❌ Attempt {attempt} failed: {err}");
                db.record_attempt_error(&job.id, &err)?;
                if attempt < job.max_attempts {
                    let backoff = 5 * attempt as u64;
                    println!("Next attempt in {backoff}s (jobs cancel to stop)");
                    for _ in 0..backoff {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if let Ok(Some(j)) = db.get(&job.id) {
                            if j.status == "cancelled" {
                                println!("Job cancelled during backoff; stopping.");
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    let last_error = db
        .get(&job.id)?
        .and_then(|j| j.error)
        .unwrap_or_else(|| "unknown error".to_string());
    db.mark_failed(&job.id, &last_error)?;
    eprintln!();
    eprintln!(
        "💥 Job {} failed after {} attempt(s): {}",
        short_id(&job.id),
        job.max_attempts,
        truncate(&last_error, 300)
    );
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // ─── Run subcommand ───

    #[test]
    fn parse_run_basic() {
        let cli = Cli::try_parse_from(["pr", "run", "test query"]).unwrap();
        match cli.command {
            Commands::Run { query, output, repeat, profile, task_file } => {
                assert_eq!(query.as_deref(), Some("test query"));
                assert!(output.is_none());
                assert!(repeat.is_none());
                assert!(profile.is_none());
                assert!(task_file.is_none());
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_run_with_task_file() {
        let cli = Cli::try_parse_from(["pr", "run", "--task-file", "/tmp/task.txt"]).unwrap();
        match cli.command {
            Commands::Run { query, task_file, .. } => {
                assert!(query.is_none());
                assert_eq!(task_file.as_deref(), Some(std::path::Path::new("/tmp/task.txt")));
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_run_with_output() {
        let cli = Cli::try_parse_from(["pr", "run", "query", "-o", "/tmp/out"]).unwrap();
        match cli.command {
            Commands::Run { output, .. } => assert_eq!(output.as_deref(), Some("/tmp/out")),
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_run_with_repeat() {
        let cli = Cli::try_parse_from(["pr", "run", "query", "--repeat", "60"]).unwrap();
        match cli.command {
            Commands::Run { repeat, .. } => assert_eq!(repeat, Some(60)),
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_run_with_all_flags() {
        let cli = Cli::try_parse_from(["pr", "run", "query", "--output", "/tmp/o", "--repeat", "30"]).unwrap();
        match cli.command {
            Commands::Run { query, output, repeat, profile, .. } => {
                assert_eq!(query.as_deref(), Some("query"));
                assert_eq!(output.as_deref(), Some("/tmp/o"));
                assert_eq!(repeat, Some(30));
                assert!(profile.is_none());
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_run_without_query_is_allowed() {
        // `--task-file` makes the positional query optional; the "no task"
        // error is raised at dispatch time, not by clap.
        let cli = Cli::try_parse_from(["pr", "run"]).unwrap();
        match cli.command {
            Commands::Run { query, task_file, .. } => {
                assert!(query.is_none());
                assert!(task_file.is_none());
            }
            _ => panic!("expected Run"),
        }
    }

    // ─── Worker subcommand ───

    #[test]
    fn parse_worker() {
        let cli = Cli::try_parse_from([
            "pr", "worker",
            "--session-id", "sess-1",
            "--agent-id", "agent-1",
            "--task", "find info",
            "--socket", "/tmp/s.sock",
            "--role", "analyst",
        ]).unwrap();
        match cli.command {
            Commands::Worker { session_id, agent_id, task, socket, role } => {
                assert_eq!(session_id, "sess-1");
                assert_eq!(agent_id, "agent-1");
                assert_eq!(task, "find info");
                assert_eq!(socket, "/tmp/s.sock");
                assert_eq!(role, "analyst");
            }
            _ => panic!("expected Worker"),
        }
    }

    #[test]
    fn parse_worker_default_role() {
        let cli = Cli::try_parse_from([
            "pr", "worker",
            "--session-id", "s", "--agent-id", "a", "--task", "t", "--socket", "/s",
        ]).unwrap();
        match cli.command {
            Commands::Worker { role, .. } => assert_eq!(role, "researcher"),
            _ => panic!("expected Worker"),
        }
    }

    #[test]
    fn parse_worker_missing_required() {
        assert!(Cli::try_parse_from(["pr", "worker"]).is_err());
    }

    // ─── Tui subcommand ───

    #[test]
    fn parse_tui_no_query() {
        let cli = Cli::try_parse_from(["pr", "tui"]).unwrap();
        match cli.command {
            Commands::Tui { query, profile, replay } => {
                assert!(query.is_none());
                assert!(profile.is_none());
                assert!(replay.is_none());
            }
            _ => panic!("expected Tui"),
        }
    }

    #[test]
    fn parse_tui_with_query() {
        let cli = Cli::try_parse_from(["pr", "tui", "hello"]).unwrap();
        match cli.command {
            Commands::Tui { query, profile, replay } => {
                assert_eq!(query.as_deref(), Some("hello"));
                assert!(profile.is_none());
                assert!(replay.is_none());
            }
            _ => panic!("expected Tui"),
        }
    }

    #[test]
    fn parse_tui_with_replay() {
        let cli = Cli::try_parse_from(["pr", "tui", "--replay", "01abc"]).unwrap();
        match cli.command {
            Commands::Tui { replay, .. } => assert_eq!(replay.as_deref(), Some("01abc")),
            _ => panic!("expected Tui"),
        }
    }

    #[test]
    fn parse_run_with_profile() {
        let cli = Cli::try_parse_from(["pr", "run", "query", "--profile", "hunter"]).unwrap();
        match cli.command {
            Commands::Run { profile, .. } => assert_eq!(profile.as_deref(), Some("hunter")),
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_profiles_subcommand() {
        let cli = Cli::try_parse_from(["pr", "profiles", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Profiles {
                action: ProfilesAction::List
            }
        ));
        let cli = Cli::try_parse_from(["pr", "profiles", "new", "my-persona"]).unwrap();
        match cli.command {
            Commands::Profiles {
                action: ProfilesAction::New { name },
            } => assert_eq!(name, "my-persona"),
            _ => panic!("expected Profiles new"),
        }
    }

    // ─── Watch diff ───

    fn contact(email: &str, name: &str) -> pr_core::Contact {
        let mut c = pr_core::Contact::default();
        c.email = Some(email.to_string());
        c.name = Some(name.to_string());
        c
    }

    #[test]
    fn watch_diff_detects_only_new_contacts() {
        let before = vec![contact("a@x.com", "A")];
        let keys = contact_key_set(&before);
        let after = vec![
            contact("a@x.com", "A"),        // known
            contact("new@x.com", "Newbie"), // new
        ];
        let new = watch_new_contacts(&keys, &after);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].email.as_deref(), Some("new@x.com"));
    }

    #[test]
    fn watch_diff_empty_when_nothing_new() {
        let before = vec![contact("a@x.com", "A")];
        let keys = contact_key_set(&before);
        let after = vec![contact("A@X.com", "A")]; // same normalized email
        assert!(watch_new_contacts(&keys, &after).is_empty());
    }

    #[test]
    fn watch_report_lists_new_contacts() {
        let mut c = contact("ceo@acme.ru", "Maria Ivanova");
        c.company = Some("Acme".to_string());
        let report = watch_report("acme team", &[&c]);
        assert!(report.contains("1 new contact"));
        assert!(report.contains("Maria Ivanova"));
        assert!(report.contains("ceo@acme.ru"));
    }

    #[test]
    fn contact_keys_falls_back_to_person() {
        let mut c = pr_core::Contact::default();
        c.name = Some("No Email".to_string());
        let keys = contact_keys(&c);
        assert_eq!(keys.len(), 1);
        assert!(keys[0].starts_with("person:"));
    }

    // ─── Serve subcommand ───

    #[test]
    fn parse_serve_defaults() {
        let cli = Cli::try_parse_from(["pr", "serve"]).unwrap();
        match cli.command {
            Commands::Serve { port, host } => {
                assert_eq!(port, 8080);
                assert_eq!(host, "127.0.0.1");
            }
            _ => panic!("expected Serve"),
        }
    }

    #[test]
    fn parse_serve_custom() {
        let cli = Cli::try_parse_from(["pr", "serve", "--port", "9090", "--host", "0.0.0.0"]).unwrap();
        match cli.command {
            Commands::Serve { port, host } => {
                assert_eq!(port, 9090);
                assert_eq!(host, "0.0.0.0");
            }
            _ => panic!("expected Serve"),
        }
    }

    // ─── Contacts subcommand ───

    #[test]
    fn parse_contacts_list() {
        let cli = Cli::try_parse_from(["pr", "contacts", "list"]).unwrap();
        match cli.command {
            Commands::Contacts { action } => match action {
                ContactsAction::List { limit } => assert_eq!(limit, 50),
                _ => panic!("expected List"),
            },
            _ => panic!("expected Contacts"),
        }
    }

    #[test]
    fn parse_contacts_list_limit() {
        let cli = Cli::try_parse_from(["pr", "contacts", "list", "--limit", "100"]).unwrap();
        match cli.command {
            Commands::Contacts { action } => match action {
                ContactsAction::List { limit } => assert_eq!(limit, 100),
                _ => panic!("expected List"),
            },
            _ => panic!("expected Contacts"),
        }
    }

    #[test]
    fn parse_contacts_export() {
        let cli = Cli::try_parse_from(["pr", "contacts", "export", "--format", "json"]).unwrap();
        match cli.command {
            Commands::Contacts { action } => match action {
                ContactsAction::Export { format, output } => {
                    assert_eq!(format, "json");
                    assert!(output.is_none());
                }
                _ => panic!("expected Export"),
            },
            _ => panic!("expected Contacts"),
        }
    }

    #[test]
    fn parse_contacts_export_with_output() {
        let cli = Cli::try_parse_from(["pr", "contacts", "export", "--format", "csv", "-o", "/tmp"]).unwrap();
        match cli.command {
            Commands::Contacts { action } => match action {
                ContactsAction::Export { format, output } => {
                    assert_eq!(format, "csv");
                    assert_eq!(output.as_deref(), Some("/tmp"));
                }
                _ => panic!("expected Export"),
            },
            _ => panic!("expected Contacts"),
        }
    }

    #[test]
    fn parse_contacts_push_crm() {
        let cli = Cli::try_parse_from(["pr", "contacts", "push-crm"]).unwrap();
        match cli.command {
            Commands::Contacts { action } => matches!(action, ContactsAction::PushCrm),
            _ => panic!("expected Contacts"),
        };
    }

    #[test]
    fn parse_contacts_dedup() {
        let cli = Cli::try_parse_from(["pr", "contacts", "dedup"]).unwrap();
        match cli.command {
            Commands::Contacts {
                action: ContactsAction::Dedup { merge },
            } => assert!(!merge),
            _ => panic!("expected Contacts dedup"),
        }
        let cli = Cli::try_parse_from(["pr", "contacts", "dedup", "--merge"]).unwrap();
        match cli.command {
            Commands::Contacts {
                action: ContactsAction::Dedup { merge },
            } => assert!(merge),
            _ => panic!("expected Contacts dedup --merge"),
        }
    }

    // ─── Resume subcommand ───

    #[test]
    fn parse_resume_no_args() {
        let cli = Cli::try_parse_from(["pr", "resume"]).unwrap();
        match cli.command {
            Commands::Resume { output, session_id } => {
                assert!(output.is_none());
                assert!(session_id.is_none());
            }
            _ => panic!("expected Resume"),
        }
    }

    #[test]
    fn parse_resume_with_args() {
        let cli = Cli::try_parse_from(["pr", "resume", "-o", "/tmp", "-s", "sess-42"]).unwrap();
        match cli.command {
            Commands::Resume { output, session_id } => {
                assert_eq!(output.as_deref(), Some("/tmp"));
                assert_eq!(session_id.as_deref(), Some("sess-42"));
            }
            _ => panic!("expected Resume"),
        }
    }

    // ─── Config subcommand ───

    #[test]
    fn parse_config_show() {
        let cli = Cli::try_parse_from(["pr", "config", "show"]).unwrap();
        match cli.command {
            Commands::Config { action } => matches!(action, ConfigAction::Show),
            _ => panic!("expected Config"),
        };
    }

    #[test]
    fn parse_config_set() {
        let cli = Cli::try_parse_from(["pr", "config", "set", "llm.model", "gpt-4o"]).unwrap();
        match cli.command {
            Commands::Config { action } => match action {
                ConfigAction::Set { key, value } => {
                    assert_eq!(key, "llm.model");
                    assert_eq!(value, "gpt-4o");
                }
                _ => panic!("expected Set"),
            },
            _ => panic!("expected Config"),
        }
    }

    // ─── Bench / Stats subcommands ───

    #[test]
    fn parse_bench_defaults() {
        let cli = Cli::try_parse_from(["pr", "bench"]).unwrap();
        match cli.command {
            Commands::Bench { scenario, n, save } => {
                assert_eq!(scenario, "all");
                assert_eq!(n, 16);
                assert!(save.is_none());
            }
            _ => panic!("expected Bench"),
        }
    }

    #[test]
    fn parse_bench_with_args() {
        let cli = Cli::try_parse_from([
            "pr", "bench", "-s", "parallel-io", "-n", "8", "--save", "/tmp/report.md",
        ])
        .unwrap();
        match cli.command {
            Commands::Bench { scenario, n, save } => {
                assert_eq!(scenario, "parallel-io");
                assert_eq!(n, 8);
                assert_eq!(save.as_deref(), Some("/tmp/report.md"));
            }
            _ => panic!("expected Bench"),
        }
    }

    #[test]
    fn parse_stats() {
        let cli = Cli::try_parse_from(["pr", "stats", "-o", "/tmp/session"]).unwrap();
        match cli.command {
            Commands::Stats { output } => assert_eq!(output.as_deref(), Some("/tmp/session")),
            _ => panic!("expected Stats"),
        }
    }

    // ─── Jobs subcommand ───

    #[test]
    fn parse_jobs_submit_defaults() {
        let cli = Cli::try_parse_from(["pr", "jobs", "submit", "do research"]).unwrap();
        match cli.command {
            Commands::Jobs { action } => match action {
                JobsAction::Submit { task, attempts } => {
                    assert_eq!(task, "do research");
                    assert_eq!(attempts, 3);
                }
                _ => panic!("expected Submit"),
            },
            _ => panic!("expected Jobs"),
        }
    }

    #[test]
    fn parse_jobs_submit_with_attempts() {
        let cli = Cli::try_parse_from([
            "pr", "jobs", "submit", "task", "--attempts", "5",
        ])
        .unwrap();
        match cli.command {
            Commands::Jobs { action } => match action {
                JobsAction::Submit { attempts, .. } => assert_eq!(attempts, 5),
                _ => panic!("expected Submit"),
            },
            _ => panic!("expected Jobs"),
        }
    }

    #[test]
    fn parse_jobs_list() {
        let cli = Cli::try_parse_from(["pr", "jobs", "list"]).unwrap();
        match cli.command {
            Commands::Jobs { action } => matches!(action, JobsAction::List),
            _ => panic!("expected Jobs"),
        };
    }

    #[test]
    fn parse_jobs_status_with_watch() {
        let cli = Cli::try_parse_from(["pr", "jobs", "status", "abc123", "--watch", "5"]).unwrap();
        match cli.command {
            Commands::Jobs { action } => match action {
                JobsAction::Status { id, watch } => {
                    assert_eq!(id, "abc123");
                    assert_eq!(watch, Some(5));
                }
                _ => panic!("expected Status"),
            },
            _ => panic!("expected Jobs"),
        }
    }

    #[test]
    fn parse_jobs_logs_with_lines() {
        let cli = Cli::try_parse_from(["pr", "jobs", "logs", "abc123", "-n", "100"]).unwrap();
        match cli.command {
            Commands::Jobs { action } => match action {
                JobsAction::Logs { id, lines } => {
                    assert_eq!(id, "abc123");
                    assert_eq!(lines, 100);
                }
                _ => panic!("expected Logs"),
            },
            _ => panic!("expected Jobs"),
        }
    }

    #[test]
    fn parse_jobs_cancel() {
        let cli = Cli::try_parse_from(["pr", "jobs", "cancel", "abc123"]).unwrap();
        match cli.command {
            Commands::Jobs { action } => match action {
                JobsAction::Cancel { id } => assert_eq!(id, "abc123"),
                _ => panic!("expected Cancel"),
            },
            _ => panic!("expected Jobs"),
        }
    }

    #[test]
    fn parse_jobs_rerun() {
        let cli = Cli::try_parse_from(["pr", "jobs", "rerun", "abc123"]).unwrap();
        match cli.command {
            Commands::Jobs { action } => match action {
                JobsAction::Rerun { id } => assert_eq!(id, "abc123"),
                _ => panic!("expected Rerun"),
            },
            _ => panic!("expected Jobs"),
        }
    }

    #[test]
    fn parse_job_run_hidden() {
        let cli = Cli::try_parse_from(["pr", "job-run", "some-id"]).unwrap();
        match cli.command {
            Commands::JobRun { id } => assert_eq!(id, "some-id"),
            _ => panic!("expected JobRun"),
        }
    }

    // ─── Retry augmentation ───

    #[test]
    fn augment_task_without_error_unchanged() {
        assert_eq!(augment_task_for_retry("do the thing", None), "do the thing");
    }

    #[test]
    fn augment_task_carries_previous_error() {
        let out = augment_task_for_retry("do the thing", Some("boom: rate limit"));
        assert!(out.starts_with("do the thing"));
        assert!(out.contains("boom: rate limit"));
        assert!(out.contains("FAILED"));
    }

    #[test]
    fn truncate_respects_char_boundary() {
        assert_eq!(truncate("привет мир", 6), "привет…");
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("line1\nline2", 20), "line1");
    }

    // ─── No command ───

    #[test]
    fn parse_no_command_fails() {
        assert!(Cli::try_parse_from(["pr"]).is_err());
    }

    #[test]
    fn parse_unknown_command_fails() {
        assert!(Cli::try_parse_from(["pr", "foobar"]).is_err());
    }
}
