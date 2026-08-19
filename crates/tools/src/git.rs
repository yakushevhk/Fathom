//! Git version-control tools built on `tokio::process::Command`.
//!
//! All commands run in the context's working directory with
//! `GIT_TERMINAL_PROMPT=0` so they never hang waiting for credentials.

use std::time::Duration;

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};

/// Maximum runtime for any single git command.
const GIT_TIMEOUT_SECS: u64 = 120;
/// Maximum characters of git output returned to the model.
const GIT_OUTPUT_MAX_CHARS: usize = 50_000;

/// Run a git command in the working directory and convert the result into a
/// `ToolOutput` (never panics; failures become `ToolOutput::err`).
pub(crate) async fn run_git(ctx: &ToolContext, args: &[String]) -> ToolOutput {
    let result = tokio::time::timeout(
        Duration::from_secs(GIT_TIMEOUT_SECS),
        tokio::process::Command::new("git")
            .args(args)
            .current_dir(&ctx.working_dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(std::process::Stdio::null())
            .output(),
    )
    .await;

    match result {
        Ok(Ok(out)) => format_git_output(&out, &args.join(" ")),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ToolOutput::err(
            "git binary not found — install git or use the shell tool to check PATH",
        ),
        Ok(Err(e)) => ToolOutput::err(format!("failed to run git: {e}")),
        Err(_) => ToolOutput::err(format!(
            "git {} timed out after {GIT_TIMEOUT_SECS}s",
            args.join(" ")
        )),
    }
}

/// Format a completed git process into a ToolOutput. Note: git writes
/// progress/info to stderr even on success (e.g. `git push`), so stderr is
/// included as context on success too.
pub(crate) fn format_git_output(out: &std::process::Output, command: &str) -> ToolOutput {
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let exit_code = out.status.code().unwrap_or(-1);

    if out.status.success() {
        let mut content = String::new();
        if !stdout.trim().is_empty() {
            content.push_str(&stdout);
        }
        if !stderr.trim().is_empty() {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&format!("STDERR:\n{stderr}"));
        }
        if content.trim().is_empty() {
            content = format!("git {command}: OK");
        }
        ToolOutput::ok(truncate_git_output(&content))
    } else {
        ToolOutput::err(truncate_git_output(&format!(
            "git {command} failed (exit code {exit_code})\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
        )))
    }
}

fn truncate_git_output(s: &str) -> String {
    let count = s.chars().count();
    if count <= GIT_OUTPUT_MAX_CHARS {
        return s.to_string();
    }
    let truncated: String = s.chars().take(GIT_OUTPUT_MAX_CHARS).collect();
    format!("{truncated}...\n\n[Output truncated at {GIT_OUTPUT_MAX_CHARS} characters]")
}

macro_rules! schema_for_tool {
    ($self:expr, $params:ty) => {
        ToolSchema {
            name: $self.name().to_string(),
            description: $self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!($params).schema)
                .unwrap_or_default(),
        }
    };
}

// ─── git_status ───

pub struct GitStatusTool;

#[async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }
    fn description(&self) -> &str {
        "Show the working tree status of the git repository in the working directory (branch, staged/unstaged changes, untracked files)."
    }
    fn schema(&self) -> ToolSchema {
        schema_for_tool!(self, GitStatusParams)
    }
    async fn execute(
        &self,
        _args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        Ok(run_git(ctx, &["status".to_string()]).await)
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GitStatusParams {}

// ─── git_diff ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GitDiffParams {
    /// Show staged changes (--staged) instead of unstaged (default: false)
    #[serde(default)]
    staged: bool,
    /// Optional path to limit the diff to
    #[serde(default)]
    path: Option<String>,
}

pub struct GitDiffTool;

#[async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }
    fn description(&self) -> &str {
        "Show changes in the git repository: unstaged working-tree changes by default, staged changes with `staged: true`, optionally limited to one path."
    }
    fn schema(&self) -> ToolSchema {
        schema_for_tool!(self, GitDiffParams)
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: GitDiffParams = serde_json::from_value(args)?;
        let mut cmd = vec!["diff".to_string(), "--no-color".to_string()];
        if params.staged {
            cmd.push("--staged".to_string());
        }
        if let Some(path) = &params.path {
            cmd.push("--".to_string());
            cmd.push(path.clone());
        }
        Ok(run_git(ctx, &cmd).await)
    }
}

// ─── git_log ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GitLogParams {
    /// Maximum number of commits to show (default: 20, max: 200)
    #[serde(default = "default_log_limit")]
    limit: u32,
}

fn default_log_limit() -> u32 {
    20
}

pub struct GitLogTool;

#[async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }
    fn description(&self) -> &str {
        "Show the commit history of the git repository (hash, date, author, subject), newest first."
    }
    fn schema(&self) -> ToolSchema {
        schema_for_tool!(self, GitLogParams)
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: GitLogParams = serde_json::from_value(args)?;
        let limit = params.limit.clamp(1, 200);
        let cmd = vec![
            "log".to_string(),
            format!("-n{limit}"),
            "--date=short".to_string(),
            "--pretty=format:%h %ad %an: %s".to_string(),
        ];
        Ok(run_git(ctx, &cmd).await)
    }
}

// ─── git_add ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GitAddParams {
    /// Paths to stage (default: ["."] to stage everything)
    #[serde(default = "default_add_paths")]
    paths: Vec<String>,
}

fn default_add_paths() -> Vec<String> {
    vec![".".to_string()]
}

pub struct GitAddTool;

#[async_trait]
impl Tool for GitAddTool {
    fn name(&self) -> &str {
        "git_add"
    }
    fn description(&self) -> &str {
        "Stage files for the next commit. Pass specific paths or omit `paths` to stage everything (`git add .`)."
    }
    fn schema(&self) -> ToolSchema {
        schema_for_tool!(self, GitAddParams)
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: GitAddParams = serde_json::from_value(args)?;
        if params.paths.is_empty() {
            return Ok(ToolOutput::err("git_add requires at least one path"));
        }
        let mut cmd = vec!["add".to_string(), "--".to_string()];
        cmd.extend(params.paths);
        Ok(run_git(ctx, &cmd).await)
    }
}

// ─── git_commit ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GitCommitParams {
    /// Commit message
    message: String,
}

pub struct GitCommitTool;

#[async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }
    fn description(&self) -> &str {
        "Commit the staged changes with the given message. Stage files first with `git_add`. Fails when nothing is staged."
    }
    fn schema(&self) -> ToolSchema {
        schema_for_tool!(self, GitCommitParams)
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: GitCommitParams = serde_json::from_value(args)?;
        if params.message.trim().is_empty() {
            return Ok(ToolOutput::err("git_commit requires a non-empty message"));
        }
        let cmd = vec!["commit".to_string(), "-m".to_string(), params.message];
        Ok(run_git(ctx, &cmd).await)
    }
}

// ─── git_push ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GitPushParams {
    /// Remote to push to (default: "origin")
    #[serde(default = "default_remote")]
    remote: String,
    /// Branch to push (default: current branch)
    #[serde(default)]
    branch: Option<String>,
}

fn default_remote() -> String {
    "origin".to_string()
}

pub struct GitPushTool;

#[async_trait]
impl Tool for GitPushTool {
    fn name(&self) -> &str {
        "git_push"
    }
    fn description(&self) -> &str {
        "Push commits to a remote repository (default remote: origin). Optionally specify a branch. Requires network access and push permission; never prompts interactively."
    }
    fn schema(&self) -> ToolSchema {
        schema_for_tool!(self, GitPushParams)
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: GitPushParams = serde_json::from_value(args)?;
        let mut cmd = vec!["push".to_string(), params.remote];
        if let Some(branch) = params.branch {
            cmd.push(branch);
        }
        Ok(run_git(ctx, &cmd).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn git_available() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Create a fresh git repository in a temp dir with identity configured.
    fn init_repo() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();
        let run = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("git should run")
        };
        assert!(run(&["init", "-q"]).status.success());
        assert!(run(&["config", "user.email", "test@example.com"]).status.success());
        assert!(run(&["config", "user.name", "Test"]).status.success());
        assert!(run(&["config", "commit.gpgsign", "false"]).status.success());
        (tmp, dir)
    }

    fn ctx_for(dir: PathBuf) -> ToolContext {
        ToolContext::new(dir, pr_core::SearchConfig::default())
    }

    #[tokio::test]
    async fn test_format_git_output_success_with_stderr() {
        // Real command that writes to both stdout and stderr and exits 0.
        let out = tokio::process::Command::new("bash")
            .args(["-c", "echo OUT_TEXT; echo ERR_TEXT >&2"])
            .output()
            .await
            .unwrap();
        let formatted = format_git_output(&out, "push");
        assert!(formatted.success);
        assert!(formatted.content.contains("OUT_TEXT"));
        assert!(formatted.content.contains("ERR_TEXT"));
    }

    #[tokio::test]
    async fn test_format_git_output_failure() {
        let out = tokio::process::Command::new("bash")
            .args(["-c", "echo boom >&2; exit 3"])
            .output()
            .await
            .unwrap();
        let formatted = format_git_output(&out, "status");
        assert!(!formatted.success);
        assert!(formatted.content.contains("exit code 3"));
        assert!(formatted.content.contains("boom"));
    }

    #[test]
    fn test_truncate_git_output() {
        assert_eq!(truncate_git_output("short"), "short");
        let big = "x".repeat(GIT_OUTPUT_MAX_CHARS + 10);
        let t = truncate_git_output(&big);
        assert!(t.contains("[Output truncated"));
    }

    #[test]
    fn test_params_defaults() {
        let diff: GitDiffParams = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!diff.staged);
        assert!(diff.path.is_none());

        let log: GitLogParams = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(log.limit, 20);

        let add: GitAddParams = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(add.paths, vec!["."]);

        let push: GitPushParams = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(push.remote, "origin");
        assert!(push.branch.is_none());
    }

    #[test]
    fn test_tool_names() {
        let tools: Vec<(Box<dyn Tool>, &str)> = vec![
            (Box::new(GitStatusTool), "git_status"),
            (Box::new(GitDiffTool), "git_diff"),
            (Box::new(GitLogTool), "git_log"),
            (Box::new(GitAddTool), "git_add"),
            (Box::new(GitCommitTool), "git_commit"),
            (Box::new(GitPushTool), "git_push"),
        ];
        for (tool, name) in tools {
            assert_eq!(tool.name(), name);
            assert!(tool.schema().parameters.is_object());
        }
    }

    #[tokio::test]
    async fn test_git_workflow_status_add_commit_log_diff() {
        if !git_available() {
            eprintln!("git not available, skipping");
            return;
        }
        let (_tmp, dir) = init_repo();
        let ctx = ctx_for(dir.clone());

        // Create a file; status should show it as untracked.
        std::fs::write(dir.join("hello.txt"), "hello world\n").unwrap();
        let status = GitStatusTool
            .execute(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(status.success, "git_status failed: {}", status.content);
        assert!(status.content.contains("hello.txt"));

        // Stage it.
        let add = GitAddTool
            .execute(serde_json::json!({"paths": ["hello.txt"]}), &ctx)
            .await
            .unwrap();
        assert!(add.success, "git_add failed: {}", add.content);

        // Commit it.
        let commit = GitCommitTool
            .execute(serde_json::json!({"message": "test commit"}), &ctx)
            .await
            .unwrap();
        assert!(commit.success, "git_commit failed: {}", commit.content);

        // Log should contain the commit.
        let log = GitLogTool
            .execute(serde_json::json!({"limit": 5}), &ctx)
            .await
            .unwrap();
        assert!(log.success, "git_log failed: {}", log.content);
        assert!(log.content.contains("test commit"));

        // Modify the file; diff should show the change.
        std::fs::write(dir.join("hello.txt"), "hello galaxy\n").unwrap();
        let diff = GitDiffTool
            .execute(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(diff.success, "git_diff failed: {}", diff.content);
        assert!(diff.content.contains("-hello world"));
        assert!(diff.content.contains("+hello galaxy"));

        // Staged diff is empty until we stage.
        let staged = GitDiffTool
            .execute(serde_json::json!({"staged": true}), &ctx)
            .await
            .unwrap();
        assert!(staged.success);
    }

    #[tokio::test]
    async fn test_git_commit_empty_message_rejected() {
        let (_tmp, dir) = init_repo();
        let ctx = ctx_for(dir);
        let out = GitCommitTool
            .execute(serde_json::json!({"message": "   "}), &ctx)
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("non-empty message"));
    }

    #[tokio::test]
    async fn test_git_status_outside_repo_returns_err_output() {
        if !git_available() {
            eprintln!("git not available, skipping");
            return;
        }
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = ctx_for(tmp.path().to_path_buf());
        let out = GitStatusTool
            .execute(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.to_lowercase().contains("not a git repository")
            || out.content.contains("failed"));
    }

    #[tokio::test]
    async fn test_git_push_without_remote_fails_gracefully() {
        if !git_available() {
            eprintln!("git not available, skipping");
            return;
        }
        let (_tmp, dir) = init_repo();
        let ctx = ctx_for(dir);
        let out = GitPushTool
            .execute(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        // No remote configured: must fail gracefully, never panic.
        assert!(!out.success);
    }
}
