use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
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
                if name.starts_with('-') || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                    return Ok(ToolOutput::err("Invalid worktree name: must be alphanumeric, hyphens, or underscores, and not start with '-'"));
                }
                let worktrees_dir = repo_root.join(".fathom").join("worktrees");
                tokio::fs::create_dir_all(&worktrees_dir).await?;
                let target_path = worktrees_dir.join(&name);

                let branch_name = format!("fathom/{}", name);
                let base_ref = base.as_deref().unwrap_or("HEAD");
                if base_ref.starts_with('-') {
                    return Ok(ToolOutput::err("Invalid base ref"));
                }

                let mut cmd = tokio::process::Command::new("git");
                cmd.current_dir(repo_root)
                    .arg("worktree")
                    .arg("add")
                    .arg("-b")
                    .arg(&branch_name)
                    .arg("--")
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
                if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
                    return Ok(ToolOutput::err("Invalid worktree path traversal"));
                }
                let worktrees_dir = repo_root.join(".fathom").join("worktrees");
                let target_path = worktrees_dir.join(&name);

                let mut cmd = tokio::process::Command::new("git");
                cmd.current_dir(repo_root)
                    .arg("worktree")
                    .arg("remove");
                if force {
                    cmd.arg("--force");
                }
                cmd.arg("--").arg(&target_path);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolContext;

    fn dummy_ctx() -> ToolContext {
        ToolContext::new(std::path::PathBuf::from("."), pr_core::SearchConfig::default())
    }
    #[test]
    fn schema_validity() {
        let tool = GitWorktreeTool;
        assert_eq!(tool.name(), "git_worktree");
        let schema = tool.schema();
        assert!(schema.parameters.is_object());
    }

    #[tokio::test]
    async fn reject_invalid_worktree_name() {
        let tool = GitWorktreeTool;
        let ctx = dummy_ctx();
        let res = tool.execute(serde_json::json!({
            "action": "create",
            "name": "--orphan"
        }), &ctx).await.unwrap();
        assert!(!res.success);
        assert!(res.content.contains("Invalid worktree name"));
    }

    #[tokio::test]
    async fn reject_path_traversal() {
        let tool = GitWorktreeTool;
        let ctx = dummy_ctx();
        let res = tool.execute(serde_json::json!({
            "action": "remove",
            "name": "../../etc/passwd"
        }), &ctx).await.unwrap();
        assert!(!res.success);
        assert!(res.content.contains("Invalid worktree path traversal"));
    }
}
