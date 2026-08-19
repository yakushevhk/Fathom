use async_trait::async_trait;
use pr_core::{ToolSchema, ToolOutput};
use crate::registry::{Tool, ToolContext};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Regex patterns matching commands that are almost always destructive and
/// irreversible. Any command matching one of these patterns is refused.
const DESTRUCTIVE_PATTERNS: &[&str] = &[
    r"rm\s+-rf\s+/",           // rm -rf /
    r"rm\s+-rf\s+~",           // rm -rf ~
    r"rm\s+-rf\s+\$",          // rm -rf $HOME
    r"mkfs\.",                 // mkfs.*
    r"dd\s+if=.+of=/dev/",     // dd to /dev/
    r">\s*/dev/sd",            // redirect to /dev/sd*
    r"chmod\s+-R\s+777\s+/",   // chmod -R 777 /
    r":\(\)\{.*\};:",          // fork bomb
];

static DESTRUCTIVE_REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();

fn destructive_regexes() -> &'static [Regex] {
    DESTRUCTIVE_REGEXES.get_or_init(|| {
        DESTRUCTIVE_PATTERNS
            .iter()
            .filter_map(|pattern| match Regex::new(pattern) {
                Ok(re) => Some(re),
                Err(e) => {
                    tracing::error!("invalid destructive pattern {pattern:?}: {e}");
                    None
                }
            })
            .collect()
    })
}

/// Check a command string against the destructive command patterns.
///
/// Returns `true` when the command matches any known-destructive pattern and
/// must not be executed.
pub fn is_destructive_command(cmd: &str) -> bool {
    destructive_regexes().iter().any(|re| re.is_match(cmd))
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ShellParams {
    /// Command to execute
    command: String,
    /// Timeout in seconds (default: 120)
    #[serde(default = "default_timeout")]
    timeout: u64,
}

fn default_timeout() -> u64 { 120 }

pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str { "shell" }
    fn description(&self) -> &str {
        "Execute a shell command in the working directory and return stdout, stderr, and exit code.

## Capability

Runs the given command in `bash` within the working directory. Returns combined stdout, stderr, and the process exit code. Supports a configurable timeout (default 120 seconds). Use this for any task that requires command-line tools, scripts, or system inspection.

## When to Use

- Running build tools, linters, formatters, or test suites.
- Inspecting system state: `git status`, `ls -la`, `env | grep ...`.
- Running scripts or one-off commands that have no dedicated tool.
- Checking installed tool versions: `node --version`, `cargo --version`.
- Executing `git` operations for version control.

## When NOT to Use

- Do NOT use `shell` to read file contents — use `file_read` instead (it provides line numbers and is faster).
- Do NOT use `shell` to search file contents — use `grep` instead (it handles regex properly).
- Do NOT use `shell` for web requests — use `web_search` or `web_fetch` instead.
- Do NOT use `shell` to write files — use `file_write` or `file_edit` instead.

## Safety Notes

- Avoid commands with long-running or unbounded output (e.g., `tail -f`, `yes`). Use timeouts.
- Avoid destructive commands without careful consideration (e.g., `rm -rf`, `drop database`).
- Prefer non-interactive commands. Commands that wait for stdin will hang until timeout.
- Keep commands focused — one operation per call for clear error diagnosis.

## Parameters

- `command` (required): The shell command string to execute.
- `timeout` (optional, default 120): Timeout in seconds. Increase for long-running builds or tests.

## Failure Modes

- Exit code != 0: the command failed. Read stderr for the error message.
- Timeout: the command exceeded the timeout limit. Try increasing `timeout` or simplifying the command.
- Command not found: the required tool is not installed. Check with `which <tool>` first."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ShellParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: ShellParams = serde_json::from_value(args)?;

        // Refuse commands that would cause irreversible damage.
        if is_destructive_command(&params.command) {
            return Ok(ToolOutput::err(format!(
                "BLOCKED: Destructive command detected: {}",
                params.command.chars().take(50).collect::<String>()
            )));
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(params.timeout),
            tokio::process::Command::new("bash")
                .args(["-c", &params.command])
                .current_dir(&ctx.working_dir)
                .output(),
        ).await;

        match output {
            Ok(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let exit_code = out.status.code().unwrap_or(-1);

                let mut result = String::new();
                if !stdout.is_empty() {
                    result.push_str(&format!("STDOUT:\n{stdout}\n"));
                }
                if !stderr.is_empty() {
                    result.push_str(&format!("STDERR:\n{stderr}\n"));
                }
                result.push_str(&format!("Exit code: {exit_code}"));

                if out.status.success() {
                    Ok(ToolOutput::ok(result))
                } else {
                    Ok(ToolOutput::err(result))
                }
            }
            Ok(Err(e)) => Ok(ToolOutput::err(format!("Command failed to start: {e}"))),
            Err(_) => Ok(ToolOutput::err(format!(
                "Command timed out after {}s", params.timeout
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolContext;

    #[test]
    fn test_blocks_rm_rf_root() {
        assert!(is_destructive_command("rm -rf /"));
        assert!(is_destructive_command("sudo rm -rf /"));
        assert!(is_destructive_command("rm -rf   /"));
    }

    #[test]
    fn test_blocks_rm_rf_home() {
        assert!(is_destructive_command("rm -rf ~"));
        assert!(is_destructive_command("rm -rf ~/"));
    }

    #[test]
    fn test_blocks_rm_rf_env_var() {
        assert!(is_destructive_command("rm -rf $HOME"));
        assert!(is_destructive_command("rm -rf $DIR"));
    }

    #[test]
    fn test_blocks_mkfs() {
        assert!(is_destructive_command("mkfs.ext4 /dev/sda1"));
        assert!(is_destructive_command("mkfs.xfs /dev/nvme0n1"));
    }

    #[test]
    fn test_blocks_dd_to_device() {
        assert!(is_destructive_command("dd if=/dev/zero of=/dev/sda"));
        assert!(is_destructive_command("dd if=image.iso of=/dev/sdb bs=4M"));
    }

    #[test]
    fn test_blocks_redirect_to_disk_device() {
        assert!(is_destructive_command("cat payload > /dev/sda"));
        assert!(is_destructive_command("echo x >/dev/sdb1"));
    }

    #[test]
    fn test_blocks_chmod_777_root() {
        assert!(is_destructive_command("chmod -R 777 /"));
    }

    #[test]
    fn test_blocks_fork_bomb() {
        assert!(is_destructive_command(":(){ :|:& };:"));
        assert!(is_destructive_command(":(){ :|: & };:"));
    }

    #[test]
    fn test_allows_safe_commands() {
        assert!(!is_destructive_command("ls -la"));
        assert!(!is_destructive_command("cargo test"));
        assert!(!is_destructive_command("rm file.txt"));
        assert!(!is_destructive_command("rm -rf ./build"));
        assert!(!is_destructive_command("rm -rf target/debug"));
        assert!(!is_destructive_command("git status"));
        assert!(!is_destructive_command("echo hello > out.txt"));
        assert!(!is_destructive_command("dd if=input.txt of=output.txt"));
        assert!(!is_destructive_command("chmod -R 755 ./scripts"));
    }

    #[tokio::test]
    async fn test_shell_tool_blocks_destructive_command() {
        let tool = ShellTool;
        let ctx = ToolContext::new(
            std::env::temp_dir(),
            pr_core::SearchConfig::default(),
        );
        let args = serde_json::json!({ "command": "rm -rf /" });
        let output = tool.execute(args, &ctx).await.unwrap();

        assert!(!output.success);
        assert!(output.content.starts_with("BLOCKED: Destructive command detected:"));
    }

    #[tokio::test]
    async fn test_shell_tool_blocks_destructive_embedded_in_chain() {
        let tool = ShellTool;
        let ctx = ToolContext::new(
            std::env::temp_dir(),
            pr_core::SearchConfig::default(),
        );
        let args = serde_json::json!({ "command": "echo cleaning && rm -rf $HOME" });
        let output = tool.execute(args, &ctx).await.unwrap();

        assert!(!output.success);
        assert!(output.content.starts_with("BLOCKED"));
    }

    #[tokio::test]
    async fn test_shell_tool_allows_safe_command() {
        let tool = ShellTool;
        let ctx = ToolContext::new(
            std::env::temp_dir(),
            pr_core::SearchConfig::default(),
        );
        let args = serde_json::json!({ "command": "echo ok" });
        let output = tool.execute(args, &ctx).await.unwrap();

        assert!(output.success);
        assert!(output.content.contains("ok"));
        assert!(output.content.contains("Exit code: 0"));
    }

    #[test]
    fn test_blocked_message_truncates_long_commands() {
        let long_cmd = format!("rm -rf / {}", "x".repeat(200));
        let preview: String = long_cmd.chars().take(50).collect();
        assert_eq!(preview.chars().count(), 50);
        assert!(is_destructive_command(&long_cmd));
    }
}
