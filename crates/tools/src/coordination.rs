//! `skill` and `scratchpad` tools (fleet E7 / C8).
//!
//! - `skill` loads the full instructions of a discovered skill on demand,
//!   keeping system prompts small while playbooks stay usable.
//! - `scratchpad` is a session-shared ledger all agents of a run can read
//!   and append — cheap cross-agent coordination ("companies already
//!   covered", "dead ends") that survives compaction.

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};

// ─── skill ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SkillParams {
    /// Name of the skill to load (see "Available Skills" in your prompt).
    name: String,
}

pub struct SkillTool;

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }
    fn description(&self) -> &str {
        "Load the full instructions of a skill by name.

Skills are reusable playbooks discovered from ~/.fathom/skills.
Your system prompt lists each skill's name and description; call this tool
with the skill name to read its complete workflow before following it."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(SkillParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: SkillParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(format!("Invalid arguments: {e}"))),
        };
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        let mut registry = pr_core::skill::SkillRegistry::new(&home);
        if let Err(e) = registry.discover() {
            return Ok(ToolOutput::err(format!("Skill discovery failed: {e}")));
        }
        let wanted = params.name.trim().to_lowercase();
        match registry
            .all_skills()
            .iter()
            .find(|s| s.name.to_lowercase() == wanted)
        {
            Some(skill) => Ok(ToolOutput::ok(skill.content.clone())),
            None => {
                let known: Vec<String> =
                    registry.all_skills().iter().map(|s| s.name.clone()).collect();
                Ok(ToolOutput::err(format!(
                    "Skill '{}' not found. Available: {}",
                    params.name,
                    if known.is_empty() {
                        "(none)".to_string()
                    } else {
                        known.join(", ")
                    }
                )))
            }
        }
    }
}

// ─── scratchpad ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ScratchpadParams {
    /// `read` returns the whole ledger; `append` adds a line to it.
    action: String,
    /// Line to add (required for `append`).
    #[serde(default)]
    text: String,
}

pub struct ScratchpadTool;

impl ScratchpadTool {
    fn pad_path(ctx: &ToolContext) -> std::path::PathBuf {
        ctx.working_dir.join(".pr-context").join("ledger.md")
    }
}

#[async_trait]
impl Tool for ScratchpadTool {
    fn name(&self) -> &str {
        "scratchpad"
    }
    fn description(&self) -> &str {
        "Shared session ledger for coordination between agents.

## Actions

- `read`: return the current ledger content.
- `append`: add one line (set `text`). Example: \"covered: acme.ru (2 emails found)\" or \"dead end: no team page at corp.io\".

## When to Use

Before starting collection work, `read` the ledger to see what other agents already covered. After finishing a source/company, `append` what you covered and found. This prevents duplicate work across parallel agents and survives context compaction."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ScratchpadParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: ScratchpadParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(format!("Invalid arguments: {e}"))),
        };
        let path = Self::pad_path(ctx);

        match params.action.to_lowercase().as_str() {
            "read" => {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                if content.trim().is_empty() {
                    Ok(ToolOutput::ok(
                        "(scratchpad is empty — append coordination notes as you work)",
                    ))
                } else {
                    Ok(ToolOutput::ok(content))
                }
            }
            "append" => {
                let text = params.text.trim();
                if text.is_empty() {
                    return Ok(ToolOutput::err("append requires non-empty text"));
                }
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)?;
                writeln!(
                    file,
                    "- [{}] {text}",
                    chrono::Utc::now().format("%H:%M:%S")
                )?;
                Ok(ToolOutput::ok("appended"))
            }
            other => Ok(ToolOutput::err(format!(
                "Unknown action '{other}' (expected read or append)"
            ))),
        }
    }
}

// ─── undo ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct UndoParams {
    /// Number of checkpoints to roll back (default 1).
    #[serde(default = "default_steps")]
    steps: usize,
}

fn default_steps() -> usize {
    1
}

/// Roll back file edits made by agents to an earlier checkpoint
/// (OpenCode-style undo over the existing FileHistory).
pub struct UndoTool;

#[async_trait]
impl Tool for UndoTool {
    fn name(&self) -> &str {
        "undo"
    }
    fn description(&self) -> &str {
        "Undo recent file edits made during this session by rewinding to an earlier file-history checkpoint.

Use after a batch of edits went wrong. `steps` (default 1) selects how many checkpoints back to rewind to. Files not tracked by the history are untouched."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(UndoParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: UndoParams = serde_json::from_value(args).unwrap_or(UndoParams { steps: 1 });
        let steps = params.steps.max(1);

        let history = ctx.file_history.lock().await;
        let checkpoints = history.list_checkpoints();
        if checkpoints.is_empty() {
            return Ok(ToolOutput::err("no file-history checkpoints to undo to"));
        }
        // Checkpoints are created AFTER each write, so the newest one equals
        // the current on-disk state — step past it to actually undo.
        let idx = checkpoints.len().saturating_sub(steps + 1);
        let target = checkpoints[idx].clone();
        match history.rewind(&target) {
            Ok(()) => Ok(ToolOutput::ok(format!(
                "rewound to checkpoint {} ({} of {})",
                target,
                steps,
                checkpoints.len()
            ))),
            Err(e) => Ok(ToolOutput::err(format!("undo failed: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::SearchConfig;
    use std::path::PathBuf;

    fn ctx(dir: &std::path::Path) -> ToolContext {
        ToolContext::new(dir.to_path_buf(), SearchConfig::default())
    }

    #[tokio::test]
    async fn scratchpad_append_then_read() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tool = ScratchpadTool;
        let c = ctx(tmp.path());

        let out = tool
            .execute(serde_json::json!({"action": "read"}), &c)
            .await
            .unwrap();
        assert!(out.content.contains("empty"));

        tool.execute(
            serde_json::json!({"action": "append", "text": "covered: acme.ru"}),
            &c,
        )
        .await
        .unwrap();
        tool.execute(
            serde_json::json!({"action": "append", "text": "dead end: corp.io"}),
            &c,
        )
        .await
        .unwrap();

        let out = tool
            .execute(serde_json::json!({"action": "read"}), &c)
            .await
            .unwrap();
        assert!(out.content.contains("acme.ru"));
        assert!(out.content.contains("corp.io"));
    }

    #[tokio::test]
    async fn scratchpad_rejects_bad_input() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tool = ScratchpadTool;
        let c = ctx(tmp.path());

        let out = tool
            .execute(serde_json::json!({"action": "append", "text": "  "}), &c)
            .await
            .unwrap();
        assert!(!out.success);

        let out = tool
            .execute(serde_json::json!({"action": "destroy"}), &c)
            .await
            .unwrap();
        assert!(!out.success);
    }

    #[tokio::test]
    async fn undo_rewinds_to_checkpoint() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("doc.md");
        std::fs::write(&file, "v1").unwrap();

        let c = ctx(tmp.path());
        // Track + checkpoint v1, then modify.
        {
            let mut h = c.file_history.lock().await;
            h.track_edit(&file).unwrap();
            h.make_snapshot().unwrap();
        }
        std::fs::write(&file, "v2-broken").unwrap();

        let tool = UndoTool;
        let out = tool
            .execute(serde_json::json!({"steps": 1}), &c)
            .await
            .unwrap();
        assert!(out.success, "undo failed: {}", out.content);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "v1");
    }

    #[tokio::test]
    async fn undo_without_history_reports_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let c = ctx(tmp.path());
        let tool = UndoTool;
        let out = tool
            .execute(serde_json::json!({}), &c)
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("no file-history checkpoints"));
    }

    #[tokio::test]
    async fn skill_tool_reports_unknown_skill() {
        // Discovery root is the real home dir; the name is guaranteed absent.
        let tool = SkillTool;
        let c = ctx(&PathBuf::from("/tmp"));
        let out = tool
            .execute(serde_json::json!({"name": "definitely-not-a-skill-xyz"}), &c)
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("not found"));
    }
}
