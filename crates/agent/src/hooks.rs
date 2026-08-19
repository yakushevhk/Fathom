//! Lifecycle hooks (fleet E3, ZCode pattern).
//!
//! Hooks are subprocesses invoked with a JSON payload on stdin at defined
//! points of the agent loop. They answer with a JSON verdict on stdout:
//!
//! - **PreToolUse**  `{"tool", "args"}` → `{"decision": "allow"|"deny", "reason"?}`
//!   A `deny` verdict refuses the call and feeds the reason back to the model.
//! - **PostToolUse** `{"tool", "args", "result", "success"}` →
//!   `{"append_context"?}` — extra context appended to the tool result.
//! - **Stop**        `{"final_summary"}` → `{"continue": bool, "reason"?}` —
//!   when `continue` is true the agent gets the reason as a follow-up
//!   instruction instead of stopping (bounded by MAX_STOP_CONTINUATIONS).
//!
//! Hooks are best-effort: a timeout, spawn failure or unparsable verdict is
//! treated as "allow / no-op" and only logged.

use pr_core::HookConfig;
use serde_json::Value;
use std::time::Duration;

/// Maximum times Stop hooks may force a continuation per run.
pub const MAX_STOP_CONTINUATIONS: u32 = 3;

/// Outcome of running the PreToolUse hooks for one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreToolVerdict {
    Allow,
    Deny(String),
}

/// Outcome of running the Stop hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopVerdict {
    Stop,
    Continue(String),
}

fn hooks_for<'a>(hooks: &'a [HookConfig], event: &str, tool: Option<&str>) -> Vec<&'a HookConfig> {
    hooks
        .iter()
        .filter(|h| h.event.eq_ignore_ascii_case(event))
        .filter(|h| {
            h.tool.is_empty()
                || tool.map(|t| t.eq_ignore_ascii_case(&h.tool)).unwrap_or(false)
        })
        .collect()
}

/// Run one hook subprocess with `payload` on stdin; returns parsed stdout
/// JSON, or `None` on any failure (timeout, spawn error, bad JSON).
async fn run_hook(hook: &HookConfig, payload: &Value) -> Option<Value> {
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;

    let mut cmd = Command::new(&hook.command);
    cmd.args(&hook.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("hook spawn failed ({}): {e}", hook.command);
            return None;
        }
    };

    let mut stdin = child.stdin.take()?;
    let stdin_payload = payload.to_string();
    let timeout = Duration::from_millis(hook.timeout_ms.max(500));

    // Write stdin and wait for the child under ONE deadline: a hook that
    // never drains its stdin would otherwise block write_all forever
    // (payloads can exceed the OS pipe buffer).
    let exchange = async move {
        stdin.write_all(stdin_payload.as_bytes()).await?;
        stdin.flush().await?;
        drop(stdin);
        child.wait_with_output().await
    };
    let output = match tokio::time::timeout(timeout, exchange).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            tracing::warn!("hook wait failed ({}): {e}", hook.command);
            return None;
        }
        Err(_) => {
            tracing::warn!("hook timed out after {}ms ({})", hook.timeout_ms, hook.command);
            // Dropping the `exchange` future drops the child; kill_on_drop
            // kills it and tokio's process reaper collects it.
            return None;
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    // Hooks may print log lines before the JSON — take the first {...} block.
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

/// Run all matching PreToolUse hooks; the first `deny` wins.
pub async fn run_pre_tool_hooks(
    hooks: &[HookConfig],
    tool: &str,
    args: &Value,
) -> PreToolVerdict {
    for hook in hooks_for(hooks, "pretooluse", Some(tool)) {
        let payload = serde_json::json!({
            "event": "PreToolUse",
            "tool": tool,
            "args": args,
        });
        if let Some(verdict) = run_hook(hook, &payload).await {
            if verdict.get("decision").and_then(|d| d.as_str()) == Some("deny") {
                let reason = verdict
                    .get("reason")
                    .and_then(|r| r.as_str())
                    .unwrap_or("denied by hook")
                    .to_string();
                return PreToolVerdict::Deny(reason);
            }
        }
    }
    PreToolVerdict::Allow
}

/// Run all matching PostToolUse hooks; concatenates any `append_context`.
pub async fn run_post_tool_hooks(
    hooks: &[HookConfig],
    tool: &str,
    args: &Value,
    result: &str,
    success: bool,
) -> Option<String> {
    let mut appended: Vec<String> = Vec::new();
    for hook in hooks_for(hooks, "posttooluse", Some(tool)) {
        let payload = serde_json::json!({
            "event": "PostToolUse",
            "tool": tool,
            "args": args,
            "result": result.chars().take(20_000).collect::<String>(),
            "success": success,
        });
        if let Some(verdict) = run_hook(hook, &payload).await {
            if let Some(extra) = verdict.get("append_context").and_then(|c| c.as_str()) {
                if !extra.trim().is_empty() {
                    appended.push(extra.trim().to_string());
                }
            }
        }
    }
    if appended.is_empty() {
        None
    } else {
        Some(format!("\n\n[hook context]\n{}", appended.join("\n")))
    }
}

/// Run all Stop hooks; the first `continue: true` wins.
pub async fn run_stop_hooks(hooks: &[HookConfig], final_summary: &str) -> StopVerdict {
    for hook in hooks_for(hooks, "stop", None) {
        let payload = serde_json::json!({
            "event": "Stop",
            "final_summary": final_summary.chars().take(20_000).collect::<String>(),
        });
        if let Some(verdict) = run_hook(hook, &payload).await {
            if verdict.get("continue").and_then(|c| c.as_bool()) == Some(true) {
                let reason = verdict
                    .get("reason")
                    .and_then(|r| r.as_str())
                    .unwrap_or("hook requested continuation")
                    .to_string();
                return StopVerdict::Continue(reason);
            }
        }
    }
    StopVerdict::Stop
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook(event: &str, command: &str, arg: &str) -> HookConfig {
        HookConfig {
            event: event.to_string(),
            command: command.to_string(),
            args: vec![arg.to_string()],
            tool: String::new(),
            timeout_ms: 5000,
        }
    }

    #[test]
    fn hooks_for_filters_by_event_and_tool() {
        let mut h1 = hook("PreToolUse", "echo", "{}");
        h1.tool = "shell".to_string();
        let h2 = hook("PreToolUse", "echo", "{}");
        let h3 = hook("Stop", "echo", "{}");
        let hooks = vec![h1, h2, h3];

        let pre_shell = hooks_for(&hooks, "pretooluse", Some("shell"));
        assert_eq!(pre_shell.len(), 2, "tool-specific + wildcard both match");

        let pre_web = hooks_for(&hooks, "pretooluse", Some("web_fetch"));
        assert_eq!(pre_web.len(), 1, "only the wildcard hook matches");

        let stop = hooks_for(&hooks, "stop", None);
        assert_eq!(stop.len(), 1);
    }

    #[tokio::test]
    async fn pre_tool_deny_verdict() {
        // `printf` emits a deny verdict without needing a shell.
        let hooks = vec![hook(
            "PreToolUse",
            "printf",
            r#"{"decision":"deny","reason":"forbidden by policy"}"#,
        )];
        let verdict = run_pre_tool_hooks(&hooks, "shell", &serde_json::json!({})).await;
        assert_eq!(verdict, PreToolVerdict::Deny("forbidden by policy".to_string()));
    }

    #[tokio::test]
    async fn pre_tool_allows_when_hook_fails() {
        let hooks = vec![hook("PreToolUse", "/nonexistent/binary", "")];
        let verdict = run_pre_tool_hooks(&hooks, "shell", &serde_json::json!({})).await;
        assert_eq!(verdict, PreToolVerdict::Allow);
    }

    #[tokio::test]
    async fn post_tool_appends_context() {
        let hooks = vec![hook(
            "PostToolUse",
            "printf",
            r#"{"append_context":"enriched: company has 50 employees"}"#,
        )];
        let extra = run_post_tool_hooks(&hooks, "web_fetch", &serde_json::json!({}), "ok", true)
            .await
            .unwrap();
        assert!(extra.contains("enriched"));
    }

    #[tokio::test]
    async fn stop_hook_continue_and_stop() {
        let cont = vec![hook("Stop", "printf", r#"{"continue":true,"reason":"no sources"}"#)];
        assert_eq!(
            run_stop_hooks(&cont, "done").await,
            StopVerdict::Continue("no sources".to_string())
        );

        let stop = vec![hook("Stop", "printf", r#"{"continue":false}"#)];
        assert_eq!(run_stop_hooks(&stop, "done").await, StopVerdict::Stop);
    }
}
