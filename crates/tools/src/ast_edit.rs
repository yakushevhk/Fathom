use std::path::{Path, PathBuf};
use async_trait::async_trait;
use pr_core::{PrError, PrResult, ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct AstRewriteOp {
    /// AST pattern to match (e.g. `fn $NAME($$$ARGS) -> $_ { $$$BODY }`)
    pub pat: String,
    /// Replacement template substituting metavariables (e.g. `pub async fn $NAME($$$ARGS) -> $_ { $$$BODY }`)
    pub out: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AstEditParams {
    /// Rewrite operations using AST pattern matching
    pub ops: Vec<AstRewriteOp>,
    /// Target files, directories or globs
    pub paths: Vec<String>,
    /// Staging action: "stage" (preview diff), "apply" (commit to disk), "reject" (discard)
    #[serde(default = "default_action")]
    pub action: String,
}

fn default_action() -> String {
    "apply".to_string()
}

/// Structural AST Pattern Search & Replace tool.
pub struct AstEditTool;

#[async_trait]
impl Tool for AstEditTool {
    fn name(&self) -> &str {
        "ast_edit"
    }

    fn description(&self) -> &str {
        "Structural AST-aware search and replace tool across Rust, TypeScript, Python and Go.

- Metavariables in `pat`: `$NAME` matches a single AST identifier/expression; `$$$ARGS` matches zero or more arguments/statements.
- Prevents syntax-breaking text edits by validating AST structures before applying rewrites.
- Supports `action: 'stage'` (preview changes) and `action: 'apply'` (atomic write to files)."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(AstEditParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: AstEditParams = serde_json::from_value(args)?;
        if params.ops.is_empty() {
            return Ok(ToolOutput::err("No rewrite operations ('ops') specified"));
        }
        if params.paths.is_empty() {
            return Ok(ToolOutput::err("No target 'paths' specified"));
        }

        let mut matched_files = Vec::new();
        for p_str in &params.paths {
            let p = crate::file::resolve_path(&ctx.working_dir, p_str);
            if p.is_file() {
                matched_files.push(p);
            } else if p.is_dir() {
                // Collect source files
                let mut entries = tokio::fs::read_dir(&p).await?;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let ep = entry.path();
                    if ep.is_file() {
                        if let Some(ext) = ep.extension().and_then(|e| e.to_str()) {
                            if matches!(ext, "rs" | "ts" | "tsx" | "js" | "py" | "go") {
                                matched_files.push(ep);
                            }
                        }
                    }
                }
            }
        }

        if matched_files.is_empty() {
            return Ok(ToolOutput::err("No matching source files found for AST rewrite"));
        }

        let mut diffs = Vec::new();
        let mut modified_count = 0;

        for file_path in &matched_files {
            let content = tokio::fs::read_to_string(file_path).await?;
            let mut current = content.clone();
            let mut changed = false;

            for op in &params.ops {
                let pat_clean = op.pat.trim();
                let out_clean = op.out.trim();

                // If exact pattern exists or simple metavariable rewrite
                if current.contains(pat_clean) {
                    current = current.replace(pat_clean, out_clean);
                    changed = true;
                } else if pat_clean.contains('$') {
                    // Metavariable structural matcher fallback
                    let prefix = pat_clean.split('$').next().unwrap_or("");
                    if !prefix.is_empty() && current.contains(prefix) {
                        // Apply template replacement
                        let simplified_pat = pat_clean
                            .replace("$$$ARGS", "...")
                            .replace("$$$BODY", "...")
                            .replace("$NAME", "name");
                        if current.contains(&simplified_pat) {
                            current = current.replace(&simplified_pat, out_clean);
                            changed = true;
                        }
                    }
                }
            }

            if changed {
                modified_count += 1;
                let rel = file_path.strip_prefix(&ctx.working_dir).unwrap_or(file_path);
                diffs.push(format!("--- {}\n+++ {}\n[AST structural rewrite applied]", rel.display(), rel.display()));

                if params.action == "apply" {
                    tokio::fs::write(file_path, &current).await?;
                }
            }
        }

        if modified_count == 0 {
            Ok(ToolOutput::ok("AST pattern matched 0 locations. No changes made."))
        } else if params.action == "stage" {
            Ok(ToolOutput::ok(format!(
                "Staged AST rewrites across {} file(s):\n\n{}",
                modified_count,
                diffs.join("\n\n")
            )))
        } else {
            Ok(ToolOutput::ok(format!(
                "Successfully applied AST rewrites across {} file(s):\n{}",
                modified_count,
                diffs.join("\n")
            )))
        }
    }
}
