//! Claude engine — the `claude` CLI in bidirectional stream-json mode.
//!
//! Spawn contract (mirrors the OpenMausBot driver):
//!   claude --output-format stream-json --input-format stream-json --verbose
//!          [--model M] [--permission-prompt-tool stdio]
//!          [--dangerously-skip-permissions]   (auto-approve)
//!
//! stdin accepts `{"type":"user","message":{"role":"user","content":[...]}}`
//! lines; stdout emits assistant/user/result/control lines. Permission prompts
//! arrive as `control_request`/`can_use_tool` and are answered with a
//! `control_response` line — that is the approvals bridge.

use crate::engine::*;
use anyhow::Context as _;
use fathom_core::types::*;
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{broadcast, mpsc};

const MODELS: &[(&str, &str, bool)] = &[
    ("claude-opus-4-1", "Claude Opus 4.1", true),
    ("claude-sonnet-4-5", "Claude Sonnet 4.5", false),
    ("claude-haiku-4-5", "Claude Haiku 4.5", false),
];

pub struct ClaudeDriver;

#[async_trait::async_trait]
impl EngineDriver for ClaudeDriver {
    fn kind(&self) -> EngineKind {
        EngineKind::Claude
    }

    fn model_catalog(&self) -> Vec<ModelInfo> {
        MODELS
            .iter()
            .map(|(id, label, def)| ModelInfo {
                id: (*id).into(),
                label: (*label).into(),
                default: *def,
            })
            .collect()
    }

    async fn probe(&self, cfg: &EngineConfig) -> EngineStatus {
        let cli = resolve_cli(cfg, "claude");
        let version = match &cli {
            Some(p) => cli_version(p).await,
            None => None,
        };
        EngineStatus {
            kind: EngineKind::Claude,
            label: "Claude Code".into(),
            available: cli.is_some(),
            resolved: cli.as_ref().map(|p| p.display().to_string()),
            version,
            reason: cli.is_none().then(|| {
                "claude CLI not found — install Claude Code or set a custom path".to_string()
            }),
            models: self.model_catalog(),
            config: cfg.clone(),
        }
    }

    async fn start_session(&self, cfg: &EngineConfig) -> anyhow::Result<Box<dyn EngineSession>> {
        let cli = resolve_cli(cfg, "claude")
            .ok_or_else(|| anyhow::anyhow!("claude CLI not found on PATH — install Claude Code"))?;
        Ok(Box::new(ClaudeSession {
            cli,
            _cfg: cfg.clone(),
            live: None,
        }))
    }
}

struct Live {
    child: Child,
    stdin: ChildStdin,
    /// Receives parsed stdout events from the reader task.
    rx: mpsc::UnboundedReceiver<serde_json::Value>,
    /// Holds the reader task's JoinHandle so abort cleans it up.
    reader: tokio::task::JoinHandle<()>,
}

pub struct ClaudeSession {
    cli: PathBuf,
    _cfg: EngineConfig,
    live: Option<Live>,
}

impl ClaudeSession {
    fn spawn(&mut self, input: &TurnInput) -> anyhow::Result<()> {
        // Tear down a stale process before respawning.
        if let Some(mut live) = self.live.take() {
            let _ = live.child.start_kill();
            live.reader.abort();
        }

        let mut args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--input-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            // Route can_use_tool through stdio so the chat UI becomes the broker.
            "--permission-prompt-tool".to_string(),
            "stdio".to_string(),
        ];
        if let Some(model) = &input.model {
            args.push("--model".into());
            args.push(model.clone());
        }
        // Approval posture → claude's own permission mode.
        match input.approval_mode {
            ApprovalMode::Ask | ApprovalMode::Custom => {}
            ApprovalMode::Edits | ApprovalMode::Auto => {
                args.push("--permission-mode".into());
                args.push("acceptEdits".into());
            }
            ApprovalMode::Full => {
                args.push("--dangerously-skip-permissions".into());
            }
        }

        let mut cmd = Command::new(&self.cli);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &input.cwd {
            cmd.current_dir(cwd);
        }
        if let Some(sys) = &input.system_prompt {
            cmd.env("FATHOM_SOUL", sys);
            cmd.args(["--append-system-prompt", sys]);
        }

        let mut child = cmd.spawn().context("spawn claude")?;
        let stdin = child.stdin.take().context("claude stdin")?;
        let stdout = child.stdout.take().context("claude stdout")?;

        let (tx, rx) = mpsc::unbounded_channel();
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                if tx.send(v).is_err() {
                    break;
                }
            }
        });

        self.live = Some(Live {
            child,
            stdin,
            rx,
            reader,
        });
        Ok(())
    }
}

#[async_trait::async_trait]
impl EngineSession for ClaudeSession {
    async fn send_turn(
        &mut self,
        input: TurnInput,
        events: broadcast::Sender<TurnEvent>,
        decisions: SharedDecisions,
    ) {
        if self.live.is_none() {
            if let Err(e) = self.spawn(&input) {
                let _ = events.send(TurnEvent::Error(format!("{e:#}")));
                return;
            }
        }
        let live = self.live.as_mut().unwrap();

        if !write_user(&mut live.stdin, &input.prompt).await {
            let _ = events.send(TurnEvent::Error("claude stdin closed".into()));
            self.live = None;
            return;
        }

        // Mid-turn steer: queued user messages feed stdin while the turn runs.
        let mut steer_rx = input.steer_rx;

        loop {
            tokio::select! {
                line = live.rx.recv() => {
                    match line {
                        None => {
                            let _ = events.send(TurnEvent::Error("claude process exited".into()));
                            self.live = None;
                            return;
                        }
                        Some(v) => {
                            if !handle_event(&v, live, &events, &decisions).await {
                                return;
                            }
                            if v.get("type").and_then(|t| t.as_str()) == Some("result") {
                                let _ = events.send(TurnEvent::Done {
                                    text: result_text(&v),
                                });
                                return;
                            }
                        }
                    }
                }
                steer = async {
                    match steer_rx.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match steer {
                        // Steered mid-turn: the reply keeps streaming into the
                        // same pending message — keep folding.
                        Some(text) => {
                            if !write_user(&mut live.stdin, &text).await {
                                let _ = events.send(TurnEvent::Error("claude stdin closed".into()));
                                self.live = None;
                                return;
                            }
                        }
                        None => steer_rx = None,
                    }
                }
            }
        }
    }

    async fn abort(&mut self) {
        if let Some(mut live) = self.live.take() {
            let _ = live.child.start_kill();
            live.reader.abort();
        }
    }
}

/// Write one user-message line into claude's stdin stream.
async fn write_user(stdin: &mut ChildStdin, text: &str) -> bool {
    let user = json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": text }] },
    });
    stdin.write_all(user.to_string().as_bytes()).await.is_ok()
        && stdin.write_all(b"\n").await.is_ok()
        && stdin.flush().await.is_ok()
}

/// Handle one stdout line. Returns false when the turn should stop early.
async fn handle_event(
    v: &serde_json::Value,
    live: &mut Live,
    events: &broadcast::Sender<TurnEvent>,
    decisions: &SharedDecisions,
) -> bool {
    match v.get("type").and_then(|t| t.as_str()) {
        Some("assistant") => {
            for block in v
                .pointer("/message/content")
                .and_then(|c| c.as_array())
                .into_iter()
                .flatten()
            {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                            let _ = events.send(TurnEvent::Text(t.to_string()));
                        }
                    }
                    Some("thinking") => {
                        if let Some(t) = block.get("thinking").and_then(|t| t.as_str()) {
                            let _ = events.send(TurnEvent::Thinking(t.to_string()));
                        }
                    }
                    Some("tool_use") => {
                        let _ = events.send(TurnEvent::ToolUse {
                            name: block
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("tool")
                                .into(),
                            input: block
                                .get("input")
                                .map(|i| i.to_string())
                                .unwrap_or_default(),
                        });
                    }
                    _ => {}
                }
            }
        }
        Some("user") => {
            // tool_result blocks feed back as user messages in stream-json.
            for block in v
                .pointer("/message/content")
                .and_then(|c| c.as_array())
                .into_iter()
                .flatten()
            {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                    let content = match block.get("content") {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                        None => String::new(),
                    };
                    let ok = !block
                        .get("is_error")
                        .and_then(|e| e.as_bool())
                        .unwrap_or(false);
                    let _ = events.send(TurnEvent::ToolResult {
                        name: block
                            .get("tool_use_id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("tool")
                            .into(),
                        output: content.chars().take(4000).collect(),
                        ok,
                    });
                }
            }
        }
        Some("control_request") => {
            let request_id = v
                .get("request_id")
                .and_then(|r| r.as_str())
                .unwrap_or_default()
                .to_string();
            let req = v.get("request").cloned().unwrap_or_default();
            let subtype = req.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
            if subtype == "can_use_tool" {
                let tool = req
                    .get("tool_name")
                    .and_then(|t| t.as_str())
                    .unwrap_or("tool");
                let input = req.get("input").cloned().unwrap_or(json!({}));
                let detail = input
                    .get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| input.to_string());
                let (tx, rx) = tokio::sync::oneshot::channel();
                decisions.lock().await.insert(request_id.clone(), tx);
                let _ = events.send(TurnEvent::PermissionRequest {
                    request_id: request_id.clone(),
                    title: format!("{tool} wants to run"),
                    detail: detail.chars().take(800).collect(),
                    tool: Some(tool.to_string()),
                });
                let decision = rx.await.unwrap_or(ApprovalDecision::Deny("closed".into()));
                decisions.lock().await.remove(&request_id);
                let response = match decision {
                    ApprovalDecision::Allow => json!({
                        "type": "control_response",
                        "request_id": request_id,
                        "response": { "subtype": "success", "behavior": "allow", "updatedInput": input },
                    }),
                    ApprovalDecision::Deny(reason) => json!({
                        "type": "control_response",
                        "request_id": request_id,
                        "response": { "subtype": "success", "behavior": "deny", "message": reason },
                    }),
                };
                let _ = live.stdin.write_all(response.to_string().as_bytes()).await;
                let _ = live.stdin.write_all(b"\n").await;
                let _ = live.stdin.flush().await;
            }
        }
        _ => {}
    }
    true
}

fn result_text(v: &serde_json::Value) -> Option<String> {
    v.get("result")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}
