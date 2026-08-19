use pr_core::{ToolCall, ToolOutput};
use pr_tools::{ToolRegistry, ToolContext};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of executing a single tool call within a batch.
#[derive(Debug, Clone)]
pub struct ToolBatchResult {
    /// The original tool call that was executed.
    pub tool_call: ToolCall,
    /// The output produced by the tool.
    pub output: ToolOutput,
    /// Whether this tool was executed in parallel with others.
    pub parallel: bool,
    /// Duration of the execution in milliseconds.
    pub duration_ms: u64,
}

/// A batch of tool calls partitioned into parallel-safe and sequential groups.
struct PartitionedBatch {
    /// Tool calls that can run concurrently (web_search, web_fetch, file_read, glob, grep).
    parallel_safe: Vec<ToolCall>,
    /// Tool calls that require exclusive execution (file_write, file_edit, shell, spawn_agent).
    sequential: Vec<ToolCall>,
}

/// Tool executor that supports parallel and sequential execution with
/// path-overlap detection for file tools.
///
/// Parallel-safe tools (`web_search`, `web_fetch`, `file_read`, `glob`,
/// `grep`, OSINT verify/enrich/extract) run concurrently via
/// `futures::future::join_all`. Sequential tools (`file_write`,
/// `file_edit`, `shell`, `spawn_agent`) take exclusive access and run one
/// at a time.
///
/// Path-overlap detection ensures that if two file tools operate on the same
/// path, they are serialized even if both are otherwise parallel-safe.
pub struct ToolExecutor {
    /// Tool names that are safe to run in parallel.
    pub parallel_safe: HashSet<String>,
    /// Tool names that must run sequentially.
    pub sequential_only: HashSet<String>,
}

impl ToolExecutor {
    /// Create a new ToolExecutor with the default classification of tools.
    pub fn new() -> Self {
        let parallel_safe: HashSet<String> = [
            "web_search",
            "web_fetch",
            "file_read",
            "glob",
            "grep",
            "pdf_extract",
            "analyze_image",
            "search_business_directory",
            "search_social",
            "parse_corporate_site",
            "search_news",
            "find_leads",
            // OSINT verification/enrichment/extraction are read-only network
            // or CPU work (contact autosave happens later, sequentially, in
            // the runtime post-pass). Lead-gen turns routinely batch 5-10 of
            // these at once, so they must not serialize.
            "verify_email",
            "verify_phone",
            "verify_social_profile",
            "suggest_emails",
            "enrich_company",
            "enrich_person",
            "extract_contacts",
            // Structured parsers are read-only (shared fetch cache is
            // mutex-guarded), so parsing jobs batch like web_fetch.
            "parse_html",
            "extract_json",
            // Crawl/feed/code tools are read-only: network fetches or pure
            // filesystem scans with no writes, so they batch like web_fetch.
            "web_crawl",
            "web_feed",
            "code_symbols",
            "repo_map",
            // Memory reads are pure SQLite lookups (pool-guarded), safe to
            // batch; writes (absorb/boost/link/graph) stay sequential.
            "memory_search",
            "memory_digest",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let sequential_only: HashSet<String> = [
            "file_write",
            "file_edit",
            "shell",
            "spawn_agent",
            "browser_navigate",
            "browser_screenshot",
            "browser_click",
            "browser_type",
            "browser_extract",
            "git_status",
            "git_diff",
            "git_log",
            "git_add",
            "git_commit",
            "git_push",
            "python_exec",
            "node_exec",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            parallel_safe,
            sequential_only,
        }
    }

    /// Execute a batch of tool calls, running parallel-safe tools concurrently
    /// and sequential tools one at a time.
    ///
    /// Returns a `Vec<ToolBatchResult>` in the same order as the input calls.
    /// If a shell tool fails, all sibling tool calls in the batch are cancelled
    /// (cascading error), and their results are replaced with error outputs.
    pub async fn execute_batch(
        &self,
        calls: Vec<ToolCall>,
        registry: &ToolRegistry,
        ctx: &ToolContext,
    ) -> Vec<ToolBatchResult> {
        if calls.is_empty() {
            return Vec::new();
        }

        // Partition into parallel-safe and sequential groups.
        let partitioned = self.partition_batch(&calls);

        // Results indexed by tool call id for ordered reassembly.
        let results: Arc<Mutex<HashMap<String, ToolBatchResult>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Track whether a shell failure occurred (for cascading cancellation).
        let shell_failed: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // Phase 1: Execute parallel-safe tools concurrently.
        if !partitioned.parallel_safe.is_empty() {
            let futs: Vec<_> = partitioned
                .parallel_safe
                .iter()
                .map(|tc| {
                    let tc = tc.clone();
                    let shell_failed = shell_failed.clone();
                    async move {
                        // Check if shell already failed before starting.
                        {
                            let sf = shell_failed.lock().await;
                            if sf.is_some() {
                                let output = ToolOutput::err(format!(
                                    "Cancelled: sibling shell tool failed with: {}",
                                    sf.as_ref().unwrap()
                                ));
                                return ToolBatchResult {
                                    tool_call: tc,
                                    output,
                                    parallel: true,
                                    duration_ms: 0,
                                };
                            }
                        }

                        let start = std::time::Instant::now();
                        let output =
                            match registry.execute(tc.name(), tc.arguments(), ctx).await {
                                Ok(o) => o,
                                // {e:#} keeps the full anyhow chain (root
                                // cause like "Operation not permitted").
                                Err(e) => ToolOutput::err(format!("Tool execution error: {e:#}")),
                            };
                        let duration_ms = start.elapsed().as_millis() as u64;

                        ToolBatchResult {
                            tool_call: tc,
                            output,
                            parallel: true,
                            duration_ms,
                        }
                    }
                })
                .collect();

            let batch_results = futures::future::join_all(futs).await;
            let mut map = results.lock().await;
            for result in batch_results {
                map.insert(result.tool_call.id.clone(), result);
            }
        }

        // Phase 2: Execute sequential tools one at a time.
        for tc in &partitioned.sequential {
            // Check if a previous shell failure should cascade.
            {
                let sf = shell_failed.lock().await;
                if sf.is_some() {
                    let output = ToolOutput::err(format!(
                        "Cancelled: sibling shell tool failed with: {}",
                        sf.as_ref().unwrap()
                    ));
                    let result = ToolBatchResult {
                        tool_call: tc.clone(),
                        output,
                        parallel: false,
                        duration_ms: 0,
                    };
                    let mut map = results.lock().await;
                    map.insert(tc.id.clone(), result);
                    continue;
                }
            }

            let start = std::time::Instant::now();
            let output = match registry.execute(tc.name(), tc.arguments(), ctx).await {
                Ok(o) => o,
                // {e:#} keeps the full anyhow chain (root cause).
                Err(e) => ToolOutput::err(format!("Tool execution error: {e:#}")),
            };
            let duration_ms = start.elapsed().as_millis() as u64;

            // If this is a shell tool and it failed, trigger cascading cancellation.
            if tc.name() == "shell" && !output.success {
                let mut sf = shell_failed.lock().await;
                *sf = Some(output.content.clone());
            }

            let result = ToolBatchResult {
                tool_call: tc.clone(),
                output,
                parallel: false,
                duration_ms,
            };
            let mut map = results.lock().await;
            map.insert(tc.id.clone(), result);
        }

        // Reassemble results in original order.
        let map = results.lock().await;
        calls
            .iter()
            .map(|tc| {
                map.get(&tc.id).cloned().unwrap_or_else(|| ToolBatchResult {
                    tool_call: tc.clone(),
                    output: ToolOutput::err("Tool was not executed"),
                    parallel: false,
                    duration_ms: 0,
                })
            })
            .collect()
    }

    /// Execute a batch with true multi-threaded parallelism.
    ///
    /// `execute_batch` polls every parallel-safe future on the calling task
    /// via `join_all`, which overlaps *awaiting* work (network I/O) but not
    /// CPU-bound work — the futures still share one thread. This variant
    /// spawns each parallel-safe call as its own tokio task, so the
    /// multi-threaded runtime spreads them across worker threads and
    /// CPU-heavy tools (parse_html, extract_json, grep) genuinely overlap.
    ///
    /// Sequential tools and shell-failure cascading behave exactly as in
    /// `execute_batch`. Results come back in the original call order.
    pub async fn execute_batch_spawn(
        &self,
        calls: Vec<ToolCall>,
        registry: Arc<ToolRegistry>,
        ctx: Arc<ToolContext>,
    ) -> Vec<ToolBatchResult> {
        if calls.is_empty() {
            return Vec::new();
        }

        let partitioned = self.partition_batch(&calls);
        let mut map: HashMap<String, ToolBatchResult> = HashMap::new();
        let shell_failed: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // Phase 1: spawn parallel-safe tools as independent tasks.
        if !partitioned.parallel_safe.is_empty() {
            let handles: Vec<(String, tokio::task::JoinHandle<ToolBatchResult>)> = partitioned
                .parallel_safe
                .iter()
                .map(|tc| {
                    let tc = tc.clone();
                    let id = tc.id.clone();
                    let registry = registry.clone();
                    let ctx = ctx.clone();
                    let shell_failed = shell_failed.clone();
                    let handle = tokio::spawn(async move {
                        {
                            let sf = shell_failed.lock().await;
                            if let Some(err) = sf.as_ref() {
                                return ToolBatchResult {
                                    tool_call: tc,
                                    output: ToolOutput::err(format!(
                                        "Cancelled: sibling shell tool failed with: {err}"
                                    )),
                                    parallel: true,
                                    duration_ms: 0,
                                };
                            }
                        }

                        let start = std::time::Instant::now();
                        let output = match registry.execute(tc.name(), tc.arguments(), &ctx).await
                        {
                            Ok(o) => o,
                            // {e:#} keeps the full anyhow chain (root cause).
                            Err(e) => ToolOutput::err(format!("Tool execution error: {e:#}")),
                        };
                        let duration_ms = start.elapsed().as_millis() as u64;

                        ToolBatchResult {
                            tool_call: tc,
                            output,
                            parallel: true,
                            duration_ms,
                        }
                    });
                    (id, handle)
                })
                .collect();

            for (id, handle) in handles {
                match handle.await {
                    Ok(result) => {
                        map.insert(result.tool_call.id.clone(), result);
                    }
                    Err(e) => {
                        if let Some(tc) = calls.iter().find(|c| c.id == id) {
                            map.insert(
                                id.clone(),
                                ToolBatchResult {
                                    tool_call: tc.clone(),
                                    output: ToolOutput::err(format!(
                                        "parallel task failed to complete: {e}"
                                    )),
                                    parallel: true,
                                    duration_ms: 0,
                                },
                            );
                        }
                    }
                }
            }
        }

        // Phase 2: sequential tools, identical semantics to execute_batch.
        for tc in &partitioned.sequential {
            {
                let sf = shell_failed.lock().await;
                if let Some(err) = sf.as_ref() {
                    map.insert(
                        tc.id.clone(),
                        ToolBatchResult {
                            tool_call: tc.clone(),
                            output: ToolOutput::err(format!(
                                "Cancelled: sibling shell tool failed with: {err}"
                            )),
                            parallel: false,
                            duration_ms: 0,
                        },
                    );
                    continue;
                }
            }

            let start = std::time::Instant::now();
            let output = match registry.execute(tc.name(), tc.arguments(), &ctx).await {
                Ok(o) => o,
                Err(e) => ToolOutput::err(format!("Tool execution error: {e:#}")),
            };
            let duration_ms = start.elapsed().as_millis() as u64;

            if tc.name() == "shell" && !output.success {
                let mut sf = shell_failed.lock().await;
                *sf = Some(output.content.clone());
            }

            map.insert(
                tc.id.clone(),
                ToolBatchResult {
                    tool_call: tc.clone(),
                    output,
                    parallel: false,
                    duration_ms,
                },
            );
        }

        calls
            .iter()
            .map(|tc| {
                map.remove(&tc.id).unwrap_or_else(|| ToolBatchResult {
                    tool_call: tc.clone(),
                    output: ToolOutput::err("Tool was not executed"),
                    parallel: false,
                    duration_ms: 0,
                })
            })
            .collect()
    }

    /// Partition a batch of tool calls into parallel-safe and sequential groups.
    ///
    /// Tools that are not explicitly classified are treated as sequential by default.
    /// Path-overlap detection: if two parallel-safe file tools target the same path,
    /// the second is moved to the sequential group.
    fn partition_batch(&self, calls: &[ToolCall]) -> PartitionedBatch {
        let mut parallel_safe = Vec::new();
        let mut sequential = Vec::new();

        // Track paths seen in parallel-safe file tools for overlap detection.
        let mut seen_paths: HashSet<PathBuf> = HashSet::new();

        for tc in calls {
            let name = tc.name();

            if self.sequential_only.contains(name) {
                sequential.push(tc.clone());
            } else if self.parallel_safe.contains(name) {
                // Check for path overlap on file tools.
                if let Some(path) = extract_file_path(tc) {
                    let canonical = canonicalize_path(&path);
                    if seen_paths.contains(&canonical) {
                        // Path overlap: move to sequential.
                        sequential.push(tc.clone());
                    } else {
                        seen_paths.insert(canonical);
                        parallel_safe.push(tc.clone());
                    }
                } else {
                    parallel_safe.push(tc.clone());
                }
            } else {
                // Unknown tools default to sequential for safety.
                sequential.push(tc.clone());
            }
        }

        PartitionedBatch {
            parallel_safe,
            sequential,
        }
    }
}

impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a file path from a tool call's arguments, if applicable.
fn extract_file_path(tc: &ToolCall) -> Option<PathBuf> {
    let args = tc.arguments();
    // Common file path argument names.
    for key in &["path", "file", "file_path", "filename", "pattern"] {
        if let Some(value) = args.get(key) {
            if let Some(s) = value.as_str() {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

/// Canonicalize a path for comparison purposes (simple normalization).
fn canonicalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_executor_default_classifications() {
        let executor = ToolExecutor::new();
        assert!(executor.parallel_safe.contains("web_search"));
        assert!(executor.parallel_safe.contains("web_fetch"));
        assert!(executor.parallel_safe.contains("file_read"));
        assert!(executor.parallel_safe.contains("glob"));
        assert!(executor.parallel_safe.contains("grep"));

        assert!(executor.sequential_only.contains("file_write"));
        assert!(executor.sequential_only.contains("file_edit"));
        assert!(executor.sequential_only.contains("shell"));
        assert!(executor.sequential_only.contains("spawn_agent"));
    }

    #[test]
    fn test_osint_readonly_tools_are_parallel_safe() {
        let executor = ToolExecutor::new();
        for tool in [
            "verify_email",
            "verify_phone",
            "verify_social_profile",
            "enrich_company",
            "enrich_person",
            "extract_contacts",
        ] {
            assert!(
                executor.parallel_safe.contains(tool),
                "{tool} should be parallel-safe"
            );
        }
        // Persisting contacts writes to the DB — must stay sequential.
        assert!(!executor.parallel_safe.contains("save_contacts"));
    }

    #[test]
    fn test_crawl_feed_code_tools_are_parallel_safe() {
        let executor = ToolExecutor::new();
        for tool in ["web_crawl", "web_feed", "code_symbols", "repo_map"] {
            assert!(
                executor.parallel_safe.contains(tool),
                "{tool} is read-only and should be parallel-safe"
            );
        }
    }

    #[test]
    fn test_partition_batch_separates_tools() {
        let executor = ToolExecutor::new();
        let calls = vec![
            ToolCall::new("c1", "web_search", json!({"query": "test"})),
            ToolCall::new("c2", "shell", json!({"command": "ls"})),
            ToolCall::new("c3", "file_read", json!({"path": "/tmp/test.txt"})),
            ToolCall::new(
                "c4",
                "file_write",
                json!({"path": "/tmp/out.txt", "content": "hi"}),
            ),
        ];

        let partitioned = executor.partition_batch(&calls);
        assert_eq!(partitioned.parallel_safe.len(), 2); // web_search, file_read
        assert_eq!(partitioned.sequential.len(), 2); // shell, file_write
    }

    #[test]
    fn test_partition_batch_unknown_tool_defaults_to_sequential() {
        let executor = ToolExecutor::new();
        let calls = vec![ToolCall::new("c1", "some_unknown_tool", json!({}))];

        let partitioned = executor.partition_batch(&calls);
        assert_eq!(partitioned.parallel_safe.len(), 0);
        assert_eq!(partitioned.sequential.len(), 1);
    }

    #[test]
    fn test_partition_batch_path_overlap_detection() {
        let executor = ToolExecutor::new();
        let calls = vec![
            ToolCall::new("c1", "file_read", json!({"path": "/tmp/test.txt"})),
            ToolCall::new("c2", "file_read", json!({"path": "/tmp/test.txt"})),
        ];

        let partitioned = executor.partition_batch(&calls);
        // First file_read is parallel, second moves to sequential due to path overlap.
        assert_eq!(partitioned.parallel_safe.len(), 1);
        assert_eq!(partitioned.sequential.len(), 1);
    }

    #[test]
    fn test_partition_batch_different_paths_stay_parallel() {
        let executor = ToolExecutor::new();
        let calls = vec![
            ToolCall::new("c1", "file_read", json!({"path": "/tmp/a.txt"})),
            ToolCall::new("c2", "file_read", json!({"path": "/tmp/b.txt"})),
        ];

        let partitioned = executor.partition_batch(&calls);
        assert_eq!(partitioned.parallel_safe.len(), 2);
        assert_eq!(partitioned.sequential.len(), 0);
    }

    #[test]
    fn test_partition_batch_empty() {
        let executor = ToolExecutor::new();
        let calls = vec![];
        let partitioned = executor.partition_batch(&calls);
        assert!(partitioned.parallel_safe.is_empty());
        assert!(partitioned.sequential.is_empty());
    }

    #[test]
    fn test_extract_file_path_common_keys() {
        let tc1 = ToolCall::new("c1", "file_read", json!({"path": "/tmp/a.txt"}));
        assert_eq!(extract_file_path(&tc1), Some(PathBuf::from("/tmp/a.txt")));

        let tc2 = ToolCall::new("c2", "glob", json!({"pattern": "*.rs"}));
        assert_eq!(extract_file_path(&tc2), Some(PathBuf::from("*.rs")));

        let tc3 = ToolCall::new("c3", "web_search", json!({"query": "test"}));
        assert_eq!(extract_file_path(&tc3), None);
    }

    #[test]
    fn test_tool_batch_result_fields() {
        let tc = ToolCall::new("c1", "web_search", json!({"query": "test"}));
        let result = ToolBatchResult {
            tool_call: tc.clone(),
            output: ToolOutput::ok("result"),
            parallel: true,
            duration_ms: 100,
        };
        assert_eq!(result.tool_call.name(), "web_search");
        assert!(result.output.success);
        assert!(result.parallel);
        assert_eq!(result.duration_ms, 100);
    }

    #[test]
    fn test_shell_in_sequential_only() {
        let executor = ToolExecutor::new();
        // Shell should never be in parallel_safe.
        assert!(!executor.parallel_safe.contains("shell"));
        assert!(executor.sequential_only.contains("shell"));
    }

    #[test]
    fn test_partition_batch_mixed_parallel_tools() {
        let executor = ToolExecutor::new();
        let calls = vec![
            ToolCall::new("c1", "web_search", json!({"query": "rust"})),
            ToolCall::new("c2", "web_fetch", json!({"url": "https://example.com"})),
            ToolCall::new("c3", "glob", json!({"pattern": "*.rs"})),
            ToolCall::new("c4", "grep", json!({"pattern": "fn main", "path": "src/"})),
        ];

        let partitioned = executor.partition_batch(&calls);
        assert_eq!(partitioned.parallel_safe.len(), 4);
        assert_eq!(partitioned.sequential.len(), 0);
    }

    #[tokio::test]
    async fn test_execute_batch_empty_calls() {
        let executor = ToolExecutor::new();
        let registry = ToolRegistry::new();
        let ctx = ToolContext::new(
            std::path::PathBuf::from("/tmp"),
            pr_core::SearchConfig::default(),
        );
        let results = executor.execute_batch(vec![], &registry, &ctx).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_execute_batch_unknown_tool_returns_error() {
        let executor = ToolExecutor::new();
        let registry = ToolRegistry::with_builtins();
        let ctx = ToolContext::new(
            std::path::PathBuf::from("/tmp"),
            pr_core::SearchConfig::default(),
        );
        let calls = vec![ToolCall::new("c1", "nonexistent", json!({}))];
        let results = executor.execute_batch(calls, &registry, &ctx).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].output.success);
    }

    #[tokio::test]
    async fn test_execute_batch_spawn_preserves_order() {
        let executor = ToolExecutor::new();
        let dir = tempfile::tempdir().unwrap();
        for i in 0..3 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), format!("content {i}")).unwrap();
        }
        let registry = Arc::new(ToolRegistry::with_builtins());
        let ctx = Arc::new(ToolContext::new(
            dir.path().to_path_buf(),
            pr_core::SearchConfig::default(),
        ));
        let calls: Vec<ToolCall> = (0..3)
            .map(|i| {
                ToolCall::new(
                    format!("c{i}"),
                    "file_read",
                    json!({"path": dir.path().join(format!("f{i}.txt")).display().to_string()}),
                )
            })
            .collect();

        let results = executor.execute_batch_spawn(calls, registry, ctx).await;
        assert_eq!(results.len(), 3);
        for (i, r) in results.iter().enumerate() {
            assert_eq!(r.tool_call.id, format!("c{i}"), "order must match input");
            assert!(r.parallel);
            assert!(r.output.success);
        }
    }

    #[tokio::test]
    async fn test_execute_batch_spawn_mixed_phases() {
        let executor = ToolExecutor::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("in.txt"), "hello").unwrap();
        let registry = Arc::new(ToolRegistry::with_builtins());
        let ctx = Arc::new(ToolContext::new(
            dir.path().to_path_buf(),
            pr_core::SearchConfig::default(),
        ));
        let calls = vec![
            ToolCall::new(
                "r1",
                "file_read",
                json!({"path": dir.path().join("in.txt").display().to_string()}),
            ),
            ToolCall::new(
                "w1",
                "file_write",
                json!({"path": dir.path().join("out.txt").display().to_string(), "content": "x"}),
            ),
        ];

        let results = executor.execute_batch_spawn(calls, registry, ctx).await;
        assert_eq!(results.len(), 2);
        assert!(results[0].parallel, "file_read must run in parallel phase");
        assert!(!results[1].parallel, "file_write must run sequentially");
        assert!(results.iter().all(|r| r.output.success));
    }

    #[tokio::test]
    async fn test_execute_batch_spawn_empty() {
        let executor = ToolExecutor::new();
        let registry = Arc::new(ToolRegistry::with_builtins());
        let ctx = Arc::new(ToolContext::new(
            std::path::PathBuf::from("/tmp"),
            pr_core::SearchConfig::default(),
        ));
        let results = executor.execute_batch_spawn(vec![], registry, ctx).await;
        assert!(results.is_empty());
    }
}
