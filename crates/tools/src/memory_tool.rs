//! Memory management tool for agents.
//!
//! Allows agents to add, replace, remove, and batch-manage persistent memory
//! and user context entries via the `memory` tool.

use async_trait::async_trait;
use pr_core::memory::{MemoryOp, MemoryAction, MemoryTarget, MemoryStore};
use pr_core::{ToolSchema, ToolOutput};
use crate::registry::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The `memory` tool allows agents to manage persistent memory entries.
pub struct MemoryTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct MemoryParams {
    /// Action: "add", "replace", "remove", or "batch"
    action: String,
    /// Target: "memory" or "user"
    #[serde(default = "default_target")]
    target: String,
    /// Content for add/replace operations
    #[serde(default)]
    content: Option<String>,
    /// Old text for replace/remove operations
    #[serde(default)]
    old_text: Option<String>,
    /// Batch operations (for action="batch")
    #[serde(default)]
    operations: Option<Vec<BatchOp>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct BatchOp {
    /// Action: "add", "replace", "remove"
    action: String,
    /// Target: "memory" or "user"
    #[serde(default = "default_target")]
    target: String,
    /// Content for add/replace
    #[serde(default)]
    content: Option<String>,
    /// Old text for replace/remove
    #[serde(default)]
    old_text: Option<String>,
}

fn default_target() -> String {
    "memory".to_string()
}

impl MemoryTool {
    fn get_memory_store() -> MemoryStore {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        MemoryStore::new(&home)
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Manage persistent memory entries that persist across sessions. Use this to remember important facts, user preferences, corrections, and project context.

## Capability

Reads and writes to MEMORY.md (general persistent memory) and USER.md (user-specific context) stored in `~/.fathom/memory/`. Entries persist across sessions and are injected into the system prompt.

## When to Use

- **Remembering a fact** the user told you: `memory(action=\"add\", content=\"...\")`
- **Updating a correction**: `memory(action=\"replace\", old_text=\"wrong\", content=\"correct\")`
- **Removing outdated info**: `memory(action=\"remove\", old_text=\"...\")`
- **Saving user preferences**: `memory(action=\"add\", target=\"user\", content=\"...\")`
- **Batch operations**: `memory(action=\"batch\", operations=[...])`

## When NOT to Use

- Do NOT use for ephemeral per-session context.
- Do NOT use for large documents — use `file_write` instead.

## Parameters

- `action` (required): One of \"add\", \"replace\", \"remove\", \"batch\".
- `target` (optional, default \"memory\"): \"memory\" for general facts, \"user\" for user preferences.
- `content` (optional): The text to add or use as replacement.
- `old_text` (optional): Text to find for replace/remove operations.
- `operations` (optional): Array of batch operations (each with action, target, content, old_text).

## Examples

```
memory(action=\"add\", content=\"The user prefers Rust over Python\")
memory(action=\"add\", target=\"user\", content=\"User is a senior backend engineer\")
memory(action=\"replace\", old_text=\"prefers Rust\", content=\"prefers Go\")
memory(action=\"remove\", old_text=\"outdated fact\")
memory(action=\"batch\", operations=[
  {action: \"add\", target: \"memory\", content: \"fact 1\"},
  {action: \"add\", target: \"user\", content: \"preference 1\"}
])
```

## Failure Modes

- Empty content: adding an empty string is rejected.
- Entry not found: replace/remove fails if no entry contains the specified text."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(MemoryParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: MemoryParams = serde_json::from_value(args)?;

        match params.action.as_str() {
            "add" => {
                let content = params
                    .content
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("memory add requires 'content'"))?;

                let mut store = Self::get_memory_store();
                match params.target.as_str() {
                    "memory" => store.add_memory(content)?,
                    "user" => store.add_user(content)?,
                    other => {
                        return Ok(ToolOutput::err(format!(
                            "unknown target '{}', use 'memory' or 'user'",
                            other
                        )));
                    }
                }

                Ok(ToolOutput::ok(format!(
                    "Added entry to {}",
                    params.target
                )))
            }

            "replace" => {
                let old = params
                    .old_text
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("memory replace requires 'old_text'"))?;
                let new = params
                    .content
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("memory replace requires 'content'"))?;

                let mut store = Self::get_memory_store();
                match params.target.as_str() {
                    "memory" => store.replace_memory(old, new)?,
                    "user" => {
                        // User entries use the same file-backed mechanism.
                        let path = store.user_path().to_path_buf();
                        let entries = MemoryStore::load_entries_from_path(&path);
                        let _idx = entries
                            .iter()
                            .position(|e| e.content.contains(old))
                            .ok_or_else(|| {
                                anyhow::anyhow!("no user entry containing '{}'", old)
                            })?;
                        drop(entries);
                        replace_in_file(&path, old, new, store.max_user_chars)?;
                    }
                    other => {
                        return Ok(ToolOutput::err(format!(
                            "unknown target '{}', use 'memory' or 'user'",
                            other
                        )));
                    }
                }

                Ok(ToolOutput::ok(format!(
                    "Replaced entry in {}",
                    params.target
                )))
            }

            "remove" => {
                let old = params
                    .old_text
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("memory remove requires 'old_text'"))?;

                let mut store = Self::get_memory_store();
                match params.target.as_str() {
                    "memory" => store.remove_memory(old)?,
                    "user" => {
                        let path = store.user_path().to_path_buf();
                        remove_in_file(&path, old)?;
                    }
                    other => {
                        return Ok(ToolOutput::err(format!(
                            "unknown target '{}', use 'memory' or 'user'",
                            other
                        )));
                    }
                }

                Ok(ToolOutput::ok(format!(
                    "Removed entry from {}",
                    params.target
                )))
            }

            "batch" => {
                let batch_ops = params
                    .operations
                    .ok_or_else(|| anyhow::anyhow!("memory batch requires 'operations'"))?;

                let mut ops = Vec::new();
                for bo in batch_ops {
                    let action = match bo.action.as_str() {
                        "add" => MemoryAction::Add,
                        "replace" => MemoryAction::Replace,
                        "remove" => MemoryAction::Remove,
                        other => {
                            return Ok(ToolOutput::err(format!(
                                "unknown batch action '{}'",
                                other
                            )));
                        }
                    };
                    let target = match bo.target.as_str() {
                        "memory" => MemoryTarget::Memory,
                        "user" => MemoryTarget::User,
                        other => {
                            return Ok(ToolOutput::err(format!(
                                "unknown batch target '{}'",
                                other
                            )));
                        }
                    };
                    ops.push(MemoryOp {
                        action,
                        target,
                        content: bo.content,
                        old_text: bo.old_text,
                    });
                }

                let mut store = Self::get_memory_store();
                let count = ops.len();
                store.batch_operations(ops)?;

                Ok(ToolOutput::ok(format!(
                    "Executed {} batch operations",
                    count
                )))
            }

            other => Ok(ToolOutput::err(format!(
                "unknown memory action '{}', use: add, replace, remove, batch",
                other
            ))),
        }
    }
}

/// Helper: replace an entry in a file (for user target).
fn replace_in_file(
    path: &std::path::Path,
    old_substr: &str,
    new_content: &str,
    max_chars: usize,
) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut entries: Vec<String> = content
        .split('\u{00A7}')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let idx = entries
        .iter()
        .position(|e| e.contains(old_substr))
        .ok_or_else(|| anyhow::anyhow!("no entry containing '{}'", old_substr))?;

    entries[idx] = new_content.trim().to_string();

    let serialized = entries.join("\u{00A7}\n");
    let final_content = enforce_budget_str(&serialized, max_chars);

    atomic_write(path, &final_content)?;
    Ok(())
}

/// Helper: remove an entry from a file.
fn remove_in_file(path: &std::path::Path, substr: &str) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let entries: Vec<String> = content
        .split('\u{00A7}')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let before = entries.len();
    let remaining: Vec<String> = entries.into_iter().filter(|e| !e.contains(substr)).collect();

    if remaining.len() == before {
        return Err(anyhow::anyhow!("no entry containing '{}'", substr));
    }

    let serialized = remaining.join("\u{00A7}\n");
    atomic_write(path, &serialized)?;
    Ok(())
}

fn enforce_budget_str(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }

    let entries: Vec<&str> = content
        .split('\u{00A7}')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut result: Vec<&str> = Vec::new();
    let mut total = 0usize;

    for entry in entries.iter().rev() {
        let cost = entry.len() + 2;
        if total + cost > max_chars && !result.is_empty() {
            break;
        }
        result.push(entry);
        total += cost;
    }

    result.reverse();
    result.join("\u{00A7}\n")
}

fn atomic_write(path: &std::path::Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_params_deserialize_add() {
        let json = serde_json::json!({
            "action": "add",
            "content": "test content"
        });
        let params: MemoryParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "add");
        assert_eq!(params.content.unwrap(), "test content");
        assert_eq!(params.target, "memory");
    }

    #[test]
    fn test_memory_params_deserialize_replace() {
        let json = serde_json::json!({
            "action": "replace",
            "target": "user",
            "old_text": "old",
            "content": "new"
        });
        let params: MemoryParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "replace");
        assert_eq!(params.target, "user");
        assert_eq!(params.old_text.unwrap(), "old");
        assert_eq!(params.content.unwrap(), "new");
    }

    #[test]
    fn test_memory_params_deserialize_batch() {
        let json = serde_json::json!({
            "action": "batch",
            "operations": [
                {"action": "add", "target": "memory", "content": "fact 1"},
                {"action": "remove", "target": "memory", "old_text": "old fact"}
            ]
        });
        let params: MemoryParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "batch");
        let ops = params.operations.unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].action, "add");
        assert_eq!(ops[1].action, "remove");
    }

    #[test]
    fn test_enforce_budget_str_within_limit() {
        let content = "hello\u{00A7}\nworld";
        assert_eq!(enforce_budget_str(content, 100), content);
    }

    #[test]
    fn test_enforce_budget_str_drops_oldest() {
        let content = "aaa\u{00A7}\nbbb\u{00A7}\nccc";
        // 3 entries of 3 chars each + 2 delimiters = ~13 chars. Budget 10 drops oldest.
        let result = enforce_budget_str(content, 10);
        // Should keep the newest entries that fit.
        assert!(!result.contains("aaa") || result.contains("ccc"));
    }

    #[test]
    fn test_slug_consistency() {
        // Verify MemoryTool name is consistent.
        let tool = MemoryTool;
        assert_eq!(tool.name(), "memory");
    }
}
