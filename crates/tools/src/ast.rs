use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use async_trait::async_trait;
use pr_core::{PrError, PrResult, ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstSymbol {
    pub name: String,
    pub kind: String, // "function", "struct", "enum", "trait", "class", "interface"
    pub line: usize,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct AstQuery {
    /// File path or directory to analyze (optional, defaults to workspace root)
    #[serde(default)]
    pub path: Option<String>,
    /// Mode: "outline" (single file symbols), "repomap" (PageRank ranked repo map), "references" (find usages)
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Maximum symbols or ranked files to return (default 30)
    #[serde(default = "default_limit")]
    pub max_results: usize,
}

fn default_mode() -> String {
    "outline".to_string()
}

fn default_limit() -> usize {
    30
}

/// AST Code Intelligence and PageRank Repo-Map tool.
pub struct AstIntelligenceTool;

#[async_trait]
impl Tool for AstIntelligenceTool {
    fn name(&self) -> &str {
        "code_ast"
    }

    fn description(&self) -> &str {
        "AST code intelligence and repository map ranking tool.

Modes:
- `mode: 'outline'` — extract high-level AST signatures and symbols from a file without body clutter.
- `mode: 'repomap'` — compute a PageRank-weighted architectural map of the most referenced files and interfaces in the repository.
- `mode: 'references'` — find identifier references and call hierarchy across the workspace."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(AstQuery).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: AstQuery = serde_json::from_value(args)?;
        let target_path = params
            .path
            .as_ref()
            .map(|p| crate::file::resolve_path(&ctx.working_dir, p))
            .unwrap_or_else(|| ctx.working_dir.clone());

        match params.mode.as_str() {
            "outline" => {
                if !target_path.is_file() {
                    return Ok(ToolOutput::err(format!(
                        "Outline mode requires a file path, got: {}",
                        target_path.display()
                    )));
                }

                let content = tokio::fs::read_to_string(&target_path).await?;
                let symbols = extract_symbols(&content);

                if symbols.is_empty() {
                    return Ok(ToolOutput::ok(format!("No top-level AST symbols found in {}", target_path.display())));
                }

                let mut lines = Vec::new();
                lines.push(format!("AST Outline for {}:", target_path.display()));
                for sym in symbols.iter().take(params.max_results) {
                    lines.push(format!("  {:4} | [{}] {}", sym.line, sym.kind, sym.signature));
                }

                Ok(ToolOutput::ok(lines.join("\n")))
            }

            "repomap" => {
                let repomap_entries = compute_pagerank_repomap(&ctx.working_dir, params.max_results).await?;
                let mut lines = Vec::new();
                lines.push("PageRank Architectural Repo-Map (Most Referenced Components):".to_string());
                for (rank, path, score, symbol_count) in repomap_entries {
                    lines.push(format!(
                        "  #{:2} [{:.3}] {} ({} public symbols)",
                        rank, score, path, symbol_count
                    ));
                }
                Ok(ToolOutput::ok(lines.join("\n")))
            }

            "references" => {
                let identifier = params.path.unwrap_or_default();
                if identifier.is_empty() {
                    return Ok(ToolOutput::err("References mode requires identifier in 'path' parameter"));
                }

                let hits = search_workspace_references(&ctx.working_dir, &identifier, params.max_results).await?;
                Ok(ToolOutput::ok(format!("References for '{}':\n{}", identifier, hits.join("\n"))))
            }

            other => Ok(ToolOutput::err(format!("Unsupported mode '{}'", other))),
        }
    }
}

/// Extract structural AST symbols from text content using deterministic signature matching.
fn extract_symbols(content: &str) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = idx + 1;

        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") {
            continue;
        }

        let is_pub = trimmed.starts_with("pub ") || trimmed.starts_with("export ");
        let rest = if is_pub {
            trimmed.strip_prefix("pub ").or_else(|| trimmed.strip_prefix("export ")).unwrap_or(trimmed).trim()
        } else {
            trimmed
        };

        if rest.starts_with("fn ") || rest.starts_with("async fn ") || rest.starts_with("function ") {
            let name = extract_name_token(rest);
            symbols.push(AstSymbol {
                name,
                kind: "function".to_string(),
                line: line_num,
                signature: line.trim_end().to_string(),
            });
        } else if rest.starts_with("struct ") {
            let name = extract_name_token(rest);
            symbols.push(AstSymbol {
                name,
                kind: "struct".to_string(),
                line: line_num,
                signature: line.trim_end().to_string(),
            });
        } else if rest.starts_with("enum ") {
            let name = extract_name_token(rest);
            symbols.push(AstSymbol {
                name,
                kind: "enum".to_string(),
                line: line_num,
                signature: line.trim_end().to_string(),
            });
        } else if rest.starts_with("trait ") || rest.starts_with("interface ") {
            let name = extract_name_token(rest);
            symbols.push(AstSymbol {
                name,
                kind: "trait".to_string(),
                line: line_num,
                signature: line.trim_end().to_string(),
            });
        } else if rest.starts_with("class ") {
            let name = extract_name_token(rest);
            symbols.push(AstSymbol {
                name,
                kind: "class".to_string(),
                line: line_num,
                signature: line.trim_end().to_string(),
            });
        }
    }

    symbols
}

fn extract_name_token(line: &str) -> String {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() > 1 {
        parts[1].trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string()
    } else {
        "unknown".to_string()
    }
}

/// Compute PageRank on workspace source files based on cross-file symbol references.
async fn compute_pagerank_repomap(
    workspace_root: &Path,
    limit: usize,
) -> PrResult<Vec<(usize, String, f64, usize)>> {
    let mut files = Vec::new();
    let mut dir_queue = vec![workspace_root.to_path_buf()];

    while let Some(dir) = dir_queue.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| PrError::Tool(e.to_string()))?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    dir_queue.push(path);
                }
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if matches!(ext, "rs" | "ts" | "tsx" | "js" | "py" | "go" | "astro") {
                        files.push(path);
                    }
                }
            }
        }
    }

    let mut symbol_map: HashMap<String, PathBuf> = HashMap::new();
    let mut file_symbols_count: HashMap<PathBuf, usize> = HashMap::new();
    let mut file_references: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();

    for f in &files {
        if let Ok(content) = tokio::fs::read_to_string(f).await {
            let symbols = extract_symbols(&content);
            file_symbols_count.insert(f.clone(), symbols.len());
            for sym in symbols {
                if !sym.name.is_empty() {
                    symbol_map.insert(sym.name, f.clone());
                }
            }
        }
    }

    for f in &files {
        if let Ok(content) = tokio::fs::read_to_string(f).await {
            let mut refs = HashSet::new();
            for (sym_name, target_file) in &symbol_map {
                if target_file != f && content.contains(sym_name) {
                    refs.insert(target_file.clone());
                }
            }
            file_references.insert(f.clone(), refs);
        }
    }

    // PageRank calculation
    let num_nodes = files.len().max(1);
    let mut ranks: HashMap<PathBuf, f64> = files.iter().map(|f| (f.clone(), 1.0 / num_nodes as f64)).collect();
    let d = 0.85;

    for _ in 0..10 {
        let mut new_ranks: HashMap<PathBuf, f64> = HashMap::new();
        for f in &files {
            let mut incoming_score = 0.0;
            for (other_f, outgoing_refs) in &file_references {
                if outgoing_refs.contains(f) {
                    let out_deg = outgoing_refs.len().max(1);
                    let other_rank = ranks.get(other_f).cloned().unwrap_or(0.0);
                    incoming_score += other_rank / out_deg as f64;
                }
            }
            let rank = (1.0 - d) / num_nodes as f64 + d * incoming_score;
            new_ranks.insert(f.clone(), rank);
        }
        ranks = new_ranks;
    }

    let mut ranked_list: Vec<(PathBuf, f64, usize)> = files
        .into_iter()
        .map(|f| {
            let score = ranks.get(&f).cloned().unwrap_or(0.0);
            let count = file_symbols_count.get(&f).cloned().unwrap_or(0);
            (f, score, count)
        })
        .collect();

    ranked_list.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let results = ranked_list
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(idx, (p, score, count))| {
            let rel = p.strip_prefix(workspace_root).unwrap_or(&p).display().to_string();
            (idx + 1, rel, score, count)
        })
        .collect();

    Ok(results)
}

async fn search_workspace_references(
    workspace_root: &Path,
    identifier: &str,
    limit: usize,
) -> PrResult<Vec<String>> {
    let mut results = Vec::new();
    let mut dir_queue = vec![workspace_root.to_path_buf()];

    while let Some(dir) = dir_queue.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| PrError::Tool(e.to_string()))?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    dir_queue.push(path);
                }
            } else if path.is_file() {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    for (i, line) in content.lines().enumerate() {
                        if line.contains(identifier) {
                            let rel = path.strip_prefix(workspace_root).unwrap_or(&path).display();
                            results.push(format!("{}:{}: {}", rel, i + 1, line.trim()));
                            if results.len() >= limit {
                                return Ok(results);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(results)
}
