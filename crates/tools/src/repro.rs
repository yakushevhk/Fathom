use std::path::{Path, PathBuf};
use async_trait::async_trait;
use pr_core::{PrError, PrResult, ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action")]
enum ReproAction {
    /// Create a new isolated reproduction test case file.
    #[serde(rename = "create")]
    Create {
        /// Relative path for the test file (e.g. "tests/repro_issue_42.rs" or "test_repro.py")
        file: String,
        /// Source code content for the reproduction test
        content: String,
    },
    /// Run the reproduction test and verify whether it fails (confirming bug) or passes (confirming fix).
    #[serde(rename = "run")]
    Run {
        /// Specific test name or filter pattern
        test_name: String,
        /// Runner command: "cargo test", "pytest", "npm test", "go test", "auto"
        #[serde(default = "default_runner")]
        runner: String,
    },
    /// Clean up and remove the reproduction test file once verified.
    #[serde(rename = "cleanup")]
    Cleanup {
        /// File path to delete
        file: String,
    },
}

fn default_runner() -> String {
    "auto".to_string()
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ReproParams {
    #[serde(flatten)]
    action: ReproAction,
}

/// Test-Driven Reproduction Test Runner tool.
pub struct ReproTestTool;

#[async_trait]
impl Tool for ReproTestTool {
    fn name(&self) -> &str {
        "repro_test"
    }

    fn description(&self) -> &str {
        "Test-Driven Auto-Repair: create reproduction tests, execute them to verify failures/fixes, and clean up.

- `action: 'create'` — write a minimal reproduction test file.
- `action: 'run'` — execute test suite with targeted filter.
- `action: 'cleanup'` — remove temporary reproduction harness."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ReproParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: ReproParams = serde_json::from_value(args)?;

        match params.action {
            ReproAction::Create { file, content } => {
                let target = crate::file::resolve_path(&ctx.working_dir, &file);
                if let Some(p) = target.parent() {
                    tokio::fs::create_dir_all(p).await?;
                }
                tokio::fs::write(&target, &content).await?;
                Ok(ToolOutput::ok(format!(
                    "Created reproduction test at {} ({} bytes)",
                    target.display(),
                    content.len()
                )))
            }

            ReproAction::Run { test_name, runner } => {
                let mut cmd = if runner == "pytest" || runner == "python" {
                    let mut c = tokio::process::Command::new("pytest");
                    c.arg("-k").arg(&test_name);
                    c
                } else if runner == "npm" || runner == "jest" {
                    let mut c = tokio::process::Command::new("npm");
                    c.arg("test").arg("--").arg("-t").arg(&test_name);
                    c
                } else {
                    // Default to cargo test
                    let mut c = tokio::process::Command::new("cargo");
                    c.arg("test").arg(&test_name);
                    c
                };

                cmd.current_dir(&ctx.working_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                let output = cmd.output().await?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let success = output.status.success();

                let status_label = if success { "PASSED" } else { "FAILED (Expected during reproduction)" };
                Ok(ToolOutput::ok(format!(
                    "Test execution [{}]:\nSTDOUT:\n{}\nSTDERR:\n{}",
                    status_label, stdout, stderr
                )))
            }

            ReproAction::Cleanup { file } => {
                let target = crate::file::resolve_path(&ctx.working_dir, &file);
                if target.exists() {
                    tokio::fs::remove_file(&target).await?;
                    Ok(ToolOutput::ok(format!("Cleaned up reproduction test {}", target.display())))
                } else {
                    Ok(ToolOutput::ok(format!("File {} does not exist, nothing to clean up", target.display())))
                }
            }
        }
    }
}
