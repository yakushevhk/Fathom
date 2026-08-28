use std::path::{Path, PathBuf};
use async_trait::async_trait;
use pr_core::{PrError, PrResult, ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action")]
enum WorktreeAction {
    /// Create a new isolated git worktree branch for concurrent subagent execution.
    #[serde(rename = "create")]
    Create {
        /// Branch / worktree name identifier (e.g. "feat-auth-refactor" or "subagent-123")
        name: String,
        /// Optional base commit/branch to branch off (defaults to HEAD)
        #[serde(default)]
        base: Option<String>,
    },
    /// List all active git worktrees in the repository.
    #[serde(rename = "list")]
    List,
    /// Merge/squash changes from an agent worktree branch back into the target branch.
    #[serde(rename = "merge")]
    Merge {
        /// Name of the worktree branch to merge from
        branch: String,
        /// Target branch to merge into (defaults to current active branch)
        #[serde(default)]
        into: Option<String>,
        /// Commit message for the merge
        #[serde(default)]
        message: Option<String>,
        /// Squash merge into a single clean commit (default true)
        #[serde(default = "default_true")]
        squash: bool,
    },
    /// Clean up and remove an isolated worktree.
    #[serde(rename = "remove")]
    Remove {
        /// Name or path of the worktree to remove
        name: String,
        /// Force removal even if worktree has uncommitted changes (default true)
        #[serde(default = "default_true")]
        force: bool,
    },
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WorktreeParams {
    #[serde(flatten)]
    action: WorktreeAction,
}

/// Git Worktree Isolation tool for concurrent, conflict-free multi-agent code modification.
pub struct GitWorktreeTool;

#[async_trait]
impl Tool for GitWorktreeTool {
    fn name(&self) -> &str {
        "git_worktree"
    }

    fn description(&self) -> &str {
        "Manage isolated git worktrees for concurrent subagents.

- `action: 'create'` — create an isolated `.fathom/worktrees/<name>` worktree and branch.
- `action: 'list'` — list all active worktrees and paths.
- `action: 'merge'` — merge/squash changes from a subagent worktree into target branch.
- `action: 'remove'` — clean up and remove a worktree."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(WorktreeParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: WorktreeParams = serde_json::from_value(args)?;
        let repo_root = &ctx.working_dir;

        match params.action {
            WorktreeAction::Create { name, base } => {
                let worktrees_dir = repo_root.join(".fathom").join("worktrees");
                tokio::fs::create_dir_all(&worktrees_dir).await?;
                let target_path = worktrees_dir.join(&name);

                let branch_name = format!("fathom/{}", name);
                let base_ref = base.as_deref().unwrap_or("HEAD");

                let mut cmd = tokio::process::Command::new("git");
                cmd.current_dir(repo_root)
                    .arg("worktree")
                    .arg("add")
                    .arg("-b")
                    .arg(&branch_name)
                    .arg(&target_path)
                    .arg(base_ref);

                let output = cmd.output().await?;
                if !output.status.success() {
                    let err = String::from_utf8_lossy(&output.stderr);
                    return Ok(ToolOutput::err(format!("Failed to create worktree: {}", err)));
                }

                Ok(ToolOutput::ok(format!(
                    "Created isolated worktree '{}' at path {} on branch {}",
                    name,
                    target_path.display(),
                    branch_name
                )))
            }

            WorktreeAction::List => {
                let mut cmd = tokio::process::Command::new("git");
                cmd.current_dir(repo_root)
                    .arg("worktree")
                    .arg("list")
                    .arg("--porcelain");

                let output = cmd.output().await?;
                if !output.status.success() {
                    let err = String::from_utf8_lossy(&output.stderr);
                    return Ok(ToolOutput::err(format!("Failed to list worktrees: {}", err)));
                }

                let text = String::from_utf8_lossy(&output.stdout);
                Ok(ToolOutput::ok(format!("Git worktrees:\n{}", text)))
            }

            WorktreeAction::Merge { branch, into, message, squash } => {
                let branch_name = if branch.starts_with("fathom/") {
                    branch.clone()
                } else {
                    format!("fathom/{}", branch)
                };

                let target_branch = into.as_deref().unwrap_or("HEAD");
                let msg = message.unwrap_or_else(|| format!("Merge subagent worktree branch {}", branch_name));

                let mut cmd = tokio::process::Command::new("git");
                cmd.current_dir(repo_root).arg("merge");
                if squash {
                    cmd.arg("--squash");
                }
                cmd.arg(&branch_name);

                let output = cmd.output().await?;
                if !output.status.success() {
                    let err = String::from_utf8_lossy(&output.stderr);
                    return Ok(ToolOutput::err(format!("Merge conflict or error merging {}: {}", branch_name, err)));
                }

                if squash {
                    let mut commit_cmd = tokio::process::Command::new("git");
                    commit_cmd.current_dir(repo_root)
                        .arg("commit")
                        .arg("-m")
                        .arg(&msg);
                    let _ = commit_cmd.output().await;
                }

                Ok(ToolOutput::ok(format!(
                    "Successfully merged {} into {} (squash={})",
                    branch_name, target_branch, squash
                )))
            }

            WorktreeAction::Remove { name, force } => {
                let worktrees_dir = repo_root.join(".fathom").join("worktrees");
                let target_path = if name.contains('/') || name.contains('\\') {
                    PathBuf::from(&name)
                } else {
                    worktrees_dir.join(&name)
                };

                let mut cmd = tokio::process::Command::new("git");
                cmd.current_dir(repo_root)
                    .arg("worktree")
                    .arg("remove");
                if force {
                    cmd.arg("--force");
                }
                cmd.arg(&target_path);

                let output = cmd.output().await?;
                if !output.status.success() {
                    let err = String::from_utf8_lossy(&output.stderr);
                    return Ok(ToolOutput::err(format!("Failed to remove worktree: {}", err)));
                }

                Ok(ToolOutput::ok(format!("Removed worktree at {}", target_path.display())))
            }
        }
    }
}
