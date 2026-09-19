//! Codex engine — `codex exec --json` one-shot turns emitting JSONL events.
//!
//! Spawn contract: `codex exec --json --sandbox <mode> [--model M] <prompt>`
//! Events: thread.started / item.started / item.completed / turn.completed.
//! Sandbox maps to the bot's approval posture: auto-approve →
//! danger-full-access, otherwise workspace-write.

use crate::engine::*;
use anyhow::Context as _;
use fathom_core::types::*;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;

const MODELS: &[(&str, &str, bool)] = &[
    ("gpt-5.2-codex", "GPT-5.2 Codex", true),
    ("gpt-5.2", "GPT-5.2", false),
    ("o4-mini", "o4-mini", false),
];

pub struct CodexDriver;

#[async_trait::async_trait]
impl EngineDriver for CodexDriver {
    fn kind(&self) -> EngineKind {
        EngineKind::Codex
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
        let cli = resolve_cli(cfg, "codex");
        let version = match &cli {
            Some(p) => cli_version(p).await,
            None => None,
        };
        EngineStatus {
            kind: EngineKind::Codex,
            label: "Codex".into(),
            available: cli.is_some(),
            resolved: cli.as_ref().map(|p| p.display().to_string()),
            version,
            reason: cli
                .is_none()
                .then(|| "codex CLI not found — install Codex or set a custom path".into()),
            models: self.model_catalog(),
            config: cfg.clone(),
        }
    }

    async fn start_session(&self, cfg: &EngineConfig) -> anyhow::Result<Box<dyn EngineSession>> {
        let cli = resolve_cli(cfg, "codex")
            .ok_or_else(|| anyhow::anyhow!("codex CLI not found on PATH — install Codex"))?;
        Ok(Box::new(CodexSession {
            cli,
            _cfg: cfg.clone(),
            running: None,
        }))
    }
}

pub struct CodexSession {
    cli: std::path::PathBuf,
    _cfg: EngineConfig,
    running: Option<tokio::task::AbortHandle>,
}

#[async_trait::async_trait]
impl EngineSession for CodexSession {
    async fn send_turn(
        &mut self,
        input: TurnInput,
        events: broadcast::Sender<TurnEvent>,
        _decisions: SharedDecisions,
    ) {
        let mut args = vec!["exec".to_string(), "--json".to_string()];
        args.push("--sandbox".into());
        args.push(if input.auto_approve {
            "danger-full-access".into()
        } else {
            "workspace-write".into()
        });
        if let Some(model) = &input.model {
            args.push("--model".into());
            args.push(model.clone());
        }
        let mut prompt = input.prompt.clone();
        if let Some(sys) = &input.system_prompt {
            prompt = format!("<persona>\n{sys}\n</persona>\n\n{prompt}");
        }
        args.push(prompt);

        let mut cmd = Command::new(&self.cli);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &input.cwd {
            cmd.current_dir(cwd);
        }
        let mut child = match cmd.spawn().context("spawn codex") {
            Ok(c) => c,
            Err(e) => {
                let _ = events.send(TurnEvent::Error(format!("{e:#}")));
                return;
            }
        };
        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = events.send(TurnEvent::Error("codex stdout missing".into()));
                return;
            }
        };

        let mut lines = BufReader::new(stdout).lines();
        let task = tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                handle_line(&v, &events);
            }
            let _ = events.send(TurnEvent::Done { text: None });
        });
        self.running = Some(task.abort_handle());
        let _ = task.await;
    }

    async fn abort(&mut self) {
        if let Some(t) = self.running.take() {
            t.abort();
        }
    }
}

fn handle_line(v: &serde_json::Value, events: &broadcast::Sender<TurnEvent>) {
    match v.get("type").and_then(|t| t.as_str()) {
        Some("item.completed") | Some("item.updated") => {
            let item = v.get("item").cloned().unwrap_or_default();
            match item.get("type").and_then(|t| t.as_str()) {
                Some("agent_message") | Some("assistant_message") => {
                    if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                        let _ = events.send(TurnEvent::Text(t.to_string()));
                    }
                }
                Some("reasoning") => {
                    if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                        let _ = events.send(TurnEvent::Thinking(t.to_string()));
                    }
                }
                Some("command_execution") | Some("local_shell_call") => {
                    let cmd = item
                        .get("command")
                        .and_then(|c| c.as_str())
                        .or_else(|| {
                            item.get("action")
                                .and_then(|a| a.get("command"))
                                .and_then(|c| c.as_str())
                        })
                        .unwrap_or("command")
                        .to_string();
                    let _ = events.send(TurnEvent::ToolUse {
                        name: "shell".into(),
                        input: cmd.clone(),
                    });
                    if let Some(out) = item.get("aggregated_output").and_then(|o| o.as_str()) {
                        let ok = item.get("status").and_then(|s| s.as_str()) != Some("failed");
                        let _ = events.send(TurnEvent::ToolResult {
                            name: "shell".into(),
                            output: out.chars().take(4000).collect(),
                            ok,
                        });
                    }
                }
                Some("file_change") | Some("file_edit") => {
                    let _ = events.send(TurnEvent::ToolUse {
                        name: "edit".into(),
                        input: item.to_string().chars().take(400).collect(),
                    });
                }
                _ => {}
            }
        }
        Some("turn.failed") | Some("error") => {
            let msg = v
                .get("error")
                .and_then(|e| {
                    e.get("message")
                        .and_then(|m| m.as_str())
                        .or_else(|| e.as_str())
                })
                .or_else(|| v.get("message").and_then(|m| m.as_str()))
                .unwrap_or("turn failed");
            let _ = events.send(TurnEvent::Error(msg.to_string()));
        }
        _ => {}
    }
}
