//! Code REPL tools: execute Python and Node.js snippets and capture
//! stdout/stderr. Code is written to a temp file and run with the system
//! interpreter (`python3`/`python`, `node`).

use std::time::Duration;

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};

/// Default execution timeout for REPL snippets.
const DEFAULT_REPL_TIMEOUT_SECS: u64 = 30;
/// Maximum characters of interpreter output returned to the model.
const REPL_OUTPUT_MAX_CHARS: usize = 50_000;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ExecParams {
    /// Source code to execute
    code: String,
    /// Timeout in seconds (default: 30)
    #[serde(default = "default_repl_timeout")]
    timeout: u64,
}

fn default_repl_timeout() -> u64 {
    DEFAULT_REPL_TIMEOUT_SECS
}

/// Write `code` to a temp file, run it with the first interpreter from
/// `candidates` that exists, and return the captured output.
async fn run_code(
    ctx: &ToolContext,
    code: &str,
    timeout_secs: u64,
    candidates: &[&str],
    extension: &str,
) -> ToolOutput {
    let id = uuid::Uuid::now_v7();
    let file = std::env::temp_dir().join(format!("pr_repl_{id}.{extension}"));
    if let Err(e) = tokio::fs::write(&file, code).await {
        return ToolOutput::err(format!("failed to write temp script {}: {e}", file.display()));
    }

    let timeout = Duration::from_secs(timeout_secs.clamp(1, 600));
    let mut tried = Vec::new();

    for binary in candidates {
        tried.push(*binary);
        let result = tokio::time::timeout(
            timeout,
            tokio::process::Command::new(binary)
                .arg(&file)
                .current_dir(&ctx.working_dir)
                .stdin(std::process::Stdio::null())
                .output(),
        )
        .await;

        match result {
            Ok(Ok(out)) => {
                let _ = tokio::fs::remove_file(&file).await;
                return format_exec_output(&out, binary);
            }
            // Interpreter not installed: try the next candidate.
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(Err(e)) => {
                let _ = tokio::fs::remove_file(&file).await;
                return ToolOutput::err(format!("failed to start {binary}: {e}"));
            }
            Err(_) => {
                let _ = tokio::fs::remove_file(&file).await;
                return ToolOutput::err(format!(
                    "{binary} execution timed out after {}s",
                    timeout_secs.max(1)
                ));
            }
        }
    }

    let _ = tokio::fs::remove_file(&file).await;
    ToolOutput::err(format!(
        "no interpreter found (tried: {})",
        tried.join(", ")
    ))
}

fn format_exec_output(out: &std::process::Output, binary: &str) -> ToolOutput {
    let stdout = truncate_output(&String::from_utf8_lossy(&out.stdout));
    let stderr = truncate_output(&String::from_utf8_lossy(&out.stderr));
    let exit_code = out.status.code().unwrap_or(-1);

    let mut result = String::new();
    if !stdout.is_empty() {
        result.push_str(&format!("STDOUT:\n{stdout}\n"));
    }
    if !stderr.is_empty() {
        result.push_str(&format!("STDERR:\n{stderr}\n"));
    }
    result.push_str(&format!("Exit code: {exit_code} (interpreter: {binary})"));

    if out.status.success() {
        ToolOutput::ok(result)
    } else {
        ToolOutput::err(result)
    }
}

fn truncate_output(s: &str) -> String {
    let count = s.chars().count();
    if count <= REPL_OUTPUT_MAX_CHARS {
        return s.to_string();
    }
    let truncated: String = s.chars().take(REPL_OUTPUT_MAX_CHARS).collect();
    format!("{truncated}...\n[Output truncated at {REPL_OUTPUT_MAX_CHARS} characters]")
}

// ─── python_exec ───

pub struct PythonExecTool;

#[async_trait]
impl Tool for PythonExecTool {
    fn name(&self) -> &str {
        "python_exec"
    }
    fn description(&self) -> &str {
        "Execute Python code and return stdout/stderr.

## Capability

Runs the given Python source with the system interpreter (`python3`, falling back to `python`) in the working directory. Returns STDOUT, STDERR, and the exit code. Default timeout is 30 seconds (max 600).

## When to Use

- Quick calculations, data parsing/transformation, statistics.
- Prototyping logic or validating assumptions with code.
- Processing structured data (JSON/CSV) fetched by other tools.

## When NOT to Use

- Long-running or interactive programs: they will hit the timeout.
- Code requiring third-party packages that may not be installed — check first or keep to the standard library.

## Notes

- The code runs as a script file; top-level prints appear in STDOUT.
- Uncaught exceptions appear in STDERR with a non-zero exit code."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ExecParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: ExecParams = serde_json::from_value(args)?;
        if params.code.trim().is_empty() {
            return Ok(ToolOutput::err("python_exec requires non-empty code"));
        }
        Ok(run_code(ctx, &params.code, params.timeout, &["python3", "python"], "py").await)
    }
}

// ─── node_exec ───

pub struct NodeExecTool;

#[async_trait]
impl Tool for NodeExecTool {
    fn name(&self) -> &str {
        "node_exec"
    }
    fn description(&self) -> &str {
        "Execute Node.js code and return stdout/stderr.

## Capability

Runs the given JavaScript source with the system `node` interpreter in the working directory. Returns STDOUT, STDERR, and the exit code. Default timeout is 30 seconds (max 600). On recent Node versions, ESM syntax (import / top-level await) in `.js` is auto-detected; otherwise the code runs as CommonJS.

## When to Use

- JSON manipulation and quick data transformations.
- Testing JavaScript snippets or regexes.

## When NOT to Use

- Code requiring npm packages that are not installed — stick to built-ins.
- Long-running or interactive programs."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ExecParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: ExecParams = serde_json::from_value(args)?;
        if params.code.trim().is_empty() {
            return Ok(ToolOutput::err("node_exec requires non-empty code"));
        }
        Ok(run_code(ctx, &params.code, params.timeout, &["node"], "js").await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx() -> ToolContext {
        ToolContext::new(std::env::temp_dir(), pr_core::SearchConfig::default())
    }

    fn binary_available(name: &str) -> bool {
        std::process::Command::new(name)
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn working_ctx(dir: PathBuf) -> ToolContext {
        ToolContext::new(dir, pr_core::SearchConfig::default())
    }

    #[test]
    fn test_exec_params_defaults() {
        let params: ExecParams =
            serde_json::from_value(serde_json::json!({"code": "print(1)"})).unwrap();
        assert_eq!(params.timeout, DEFAULT_REPL_TIMEOUT_SECS);
    }

    #[test]
    fn test_truncate_output() {
        assert_eq!(truncate_output("small"), "small");
        let big = "y".repeat(REPL_OUTPUT_MAX_CHARS + 100);
        let t = truncate_output(&big);
        assert!(t.contains("[Output truncated"));
    }

    #[test]
    fn test_format_exec_output_failure() {
        let out = std::process::Output {
            status: {
                // Build a real failed exit status via a failing command.
                std::process::Command::new("bash")
                    .args(["-c", "exit 2"])
                    .output()
                    .unwrap()
                    .status
            },
            stdout: b"some out".to_vec(),
            stderr: b"Traceback: boom".to_vec(),
        };
        let formatted = format_exec_output(&out, "python3");
        assert!(!formatted.success);
        assert!(formatted.content.contains("Exit code: 2"));
        assert!(formatted.content.contains("Traceback: boom"));
    }

    #[test]
    fn test_tool_names_and_schemas() {
        let py = PythonExecTool;
        assert_eq!(py.name(), "python_exec");
        assert!(py.schema().parameters.is_object());
        let node = NodeExecTool;
        assert_eq!(node.name(), "node_exec");
        assert!(node.schema().parameters.is_object());
    }

    #[tokio::test]
    async fn test_python_exec_basic() {
        if !binary_available("python3") && !binary_available("python") {
            eprintln!("python not available, skipping");
            return;
        }
        let ctx = ctx();
        let out = PythonExecTool
            .execute(serde_json::json!({"code": "print(6*7)"}), &ctx)
            .await
            .unwrap();
        assert!(out.success, "python_exec failed: {}", out.content);
        assert!(out.content.contains("42"));
        assert!(out.content.contains("Exit code: 0"));
    }

    #[tokio::test]
    async fn test_python_exec_error_exit() {
        if !binary_available("python3") && !binary_available("python") {
            eprintln!("python not available, skipping");
            return;
        }
        let ctx = ctx();
        let out = PythonExecTool
            .execute(
                serde_json::json!({"code": "raise ValueError('nope')"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("ValueError"));
    }

    #[tokio::test]
    async fn test_python_exec_timeout() {
        if !binary_available("python3") && !binary_available("python") {
            eprintln!("python not available, skipping");
            return;
        }
        let ctx = ctx();
        let out = PythonExecTool
            .execute(
                serde_json::json!({"code": "import time; time.sleep(10)", "timeout": 1}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("timed out"));
    }

    #[tokio::test]
    async fn test_node_exec_basic() {
        if !binary_available("node") {
            eprintln!("node not available, skipping");
            return;
        }
        let ctx = ctx();
        let out = NodeExecTool
            .execute(serde_json::json!({"code": "console.log(6*7)"}), &ctx)
            .await
            .unwrap();
        assert!(out.success, "node_exec failed: {}", out.content);
        assert!(out.content.contains("42"));
    }

    #[tokio::test]
    async fn test_empty_code_rejected() {
        let ctx = ctx();
        let out = PythonExecTool
            .execute(serde_json::json!({"code": "   "}), &ctx)
            .await
            .unwrap();
        assert!(!out.success);
        let out = NodeExecTool
            .execute(serde_json::json!({"code": ""}), &ctx)
            .await
            .unwrap();
        assert!(!out.success);
    }

    #[tokio::test]
    async fn test_missing_interpreter_fails_gracefully() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = working_ctx(tmp.path().to_path_buf());
        // A candidate list with only bogus names must produce a graceful error.
        let out = run_code(&ctx, "print(1)", 5, &["definitely_not_an_interp_xyz"], "py").await;
        assert!(!out.success);
        assert!(out.content.contains("no interpreter found"));
    }
}
