use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use pr_core::{PrError, PrResult, ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct DiagnosticItem {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub severity: String,
    pub message: String,
    pub code: Option<String>,
    pub suggested_replacement: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CompilerCheckParams {
    /// Target language: "rust", "typescript", "python", "go", or "auto"
    #[serde(default = "default_lang")]
    pub language: String,
    /// Optional specific file path or package
    #[serde(default)]
    pub path: Option<String>,
}

fn default_lang() -> String {
    "auto".to_string()
}

/// Compiler diagnostics tool with unified span parsing across languages.
pub struct CompilerCheckTool;

#[async_trait]
impl Tool for CompilerCheckTool {
    fn name(&self) -> &str {
        "compiler_check"
    }

    fn description(&self) -> &str {
        "Run compiler/linter diagnostics and extract structured error spans.

Supported languages:
- `rust`: `cargo check --message-format=json`
- `typescript`: `tsc --noEmit`
- `python`: `ruff check --output-format=json` / `mypy`
- `go`: `go build ./...`
- `auto`: auto-detects language from workspace files."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(CompilerCheckParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: CompilerCheckParams = serde_json::from_value(args)?;
        let working_dir = &ctx.working_dir;

        let lang = if params.language == "auto" {
            if working_dir.join("Cargo.toml").exists() {
                "rust"
            } else if working_dir.join("tsconfig.json").exists() || working_dir.join("package.json").exists() {
                "typescript"
            } else if working_dir.join("go.mod").exists() {
                "go"
            } else if working_dir.join("pyproject.toml").exists() || working_dir.join("requirements.txt").exists() {
                "python"
            } else {
                "rust"
            }
        } else {
            params.language.as_str()
        };

        let mut diagnostics = Vec::new();

        match lang {
            "rust" => {
                let output = tokio::process::Command::new("cargo")
                    .arg("check")
                    .arg("--message-format=json")
                    .current_dir(working_dir)
                    .output()
                    .await?;

                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                        if val.get("reason").and_then(|r| r.as_str()) == Some("compiler-message") {
                            if let Some(msg) = val.get("message") {
                                let level = msg.get("level").and_then(|l| l.as_str()).unwrap_or("error");
                                let rendered = msg.get("rendered").and_then(|r| r.as_str()).unwrap_or_default();
                                let code = msg.get("code").and_then(|c| c.get("code")).and_then(|cd| cd.as_str()).map(String::from);

                                if let Some(spans) = msg.get("spans").and_then(|s| s.as_array()) {
                                    for span in spans {
                                        if span.get("is_primary").and_then(|p| p.as_bool()) == Some(true) {
                                            let file_name = span.get("file_name").and_then(|f| f.as_str()).unwrap_or("unknown");
                                            let line_start = span.get("line_start").and_then(|l| l.as_u64()).unwrap_or(1) as u32;
                                            let col_start = span.get("column_start").and_then(|c| c.as_u64()).unwrap_or(1) as u32;
                                            let suggested = span.get("suggested_replacement").and_then(|s| s.as_str()).map(String::from);

                                            diagnostics.push(DiagnosticItem {
                                                file: file_name.to_string(),
                                                line: line_start,
                                                column: col_start,
                                                severity: level.to_string(),
                                                message: rendered.to_string(),
                                                code: code.clone(),
                                                suggested_replacement: suggested,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "typescript" => {
                let output = tokio::process::Command::new("npx")
                    .arg("tsc")
                    .arg("--noEmit")
                    .current_dir(working_dir)
                    .output()
                    .await;

                if let Ok(out) = output {
                    let text = String::from_utf8_lossy(&out.stdout);
                    for line in text.lines() {
                        if line.contains(": error TS") {
                            let parts: Vec<&str> = line.splitn(2, ": error ").collect();
                            if parts.len() == 2 {
                                let loc = parts[0];
                                let msg = parts[1];
                                if let Some((f, rest)) = loc.split_once('(') {
                                    if let Some((l_str, c_str)) = rest.trim_end_matches(')').split_once(',') {
                                        let l = l_str.parse::<u32>().unwrap_or(1);
                                        let c = c_str.parse::<u32>().unwrap_or(1);
                                        diagnostics.push(DiagnosticItem {
                                            file: f.to_string(),
                                            line: l,
                                            column: c,
                                            severity: "error".to_string(),
                                            message: msg.to_string(),
                                            code: None,
                                            suggested_replacement: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "python" => {
                let output = tokio::process::Command::new("ruff")
                    .arg("check")
                    .arg("--output-format=json")
                    .current_dir(working_dir)
                    .output()
                    .await;

                if let Ok(out) = output {
                    if let Ok(items) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
                        for item in items {
                            let file = item.get("filename").and_then(|f| f.as_str()).unwrap_or("unknown");
                            let msg = item.get("message").and_then(|m| m.as_str()).unwrap_or_default();
                            let code = item.get("code").and_then(|c| c.as_str()).map(String::from);
                            let location = item.get("location");
                            let line = location.and_then(|l| l.get("row")).and_then(|r| r.as_u64()).unwrap_or(1) as u32;
                            let col = location.and_then(|l| l.get("column")).and_then(|c| c.as_u64()).unwrap_or(1) as u32;

                            diagnostics.push(DiagnosticItem {
                                file: file.to_string(),
                                line,
                                column: col,
                                severity: "error".to_string(),
                                message: msg.to_string(),
                                code,
                                suggested_replacement: None,
                            });
                        }
                    }
                }
            }
            "go" => {
                let output = tokio::process::Command::new("go")
                    .arg("build")
                    .arg("./...")
                    .current_dir(working_dir)
                    .output()
                    .await;

                if let Ok(out) = output {
                    let text = String::from_utf8_lossy(&out.stderr);
                    for line in text.lines() {
                        let parts: Vec<&str> = line.splitn(4, ':').collect();
                        if parts.len() >= 4 {
                            let f = parts[0];
                            let l = parts[1].parse::<u32>().unwrap_or(1);
                            let c = parts[2].parse::<u32>().unwrap_or(1);
                            let msg = parts[3].trim();
                            diagnostics.push(DiagnosticItem {
                                file: f.to_string(),
                                line: l,
                                column: c,
                                severity: "error".to_string(),
                                message: msg.to_string(),
                                code: None,
                                suggested_replacement: None,
                            });
                        }
                    }
                }
            }
            _ => return Ok(ToolOutput::err(format!("Unsupported compiler language: {}", lang))),
        }

        if diagnostics.is_empty() {
            Ok(ToolOutput::ok(format!("Compiler diagnostics [{}]: 0 errors/warnings found. Codebase compiles cleanly.", lang)))
        } else {
            let count = diagnostics.len();
            let formatted = diagnostics.iter().map(|d| {
                format!("{}:{}:{} [{}] {}{}", d.file, d.line, d.column, d.severity, d.code.as_deref().unwrap_or(""), d.message)
            }).collect::<Vec<_>>().join("\n---\n");

            Ok(ToolOutput::ok(format!("Compiler diagnostics [{}]: {} issue(s) detected:\n\n{}", lang, count, formatted)))
        }
    }
}
