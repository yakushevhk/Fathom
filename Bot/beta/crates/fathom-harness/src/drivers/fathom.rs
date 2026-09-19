//! Fathom engine — the project's own autonomous worker CLI
//! (`fathom run "<prompt>"`). Streams stdout lines as text deltas; the run
//! inherits the full built-in toolset of the Fathom core runtime.

use crate::engine::*;
use anyhow::Context as _;
use fathom_core::types::*;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;

const MODELS: &[(&str, &str, bool)] = &[
    ("default", "Default (config.toml)", true),
    ("deepseek-chat", "DeepSeek Chat", false),
    ("claude-opus-4-1", "Claude Opus 4.1", false),
];

pub struct FathomDriver;

#[async_trait::async_trait]
impl EngineDriver for FathomDriver {
    fn kind(&self) -> EngineKind {
        EngineKind::Fathom
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
        let cli = resolve_cli(cfg, "fathom");
        let version = match &cli {
            Some(p) => cli_version(p).await,
            None => None,
        };
        EngineStatus {
            kind: EngineKind::Fathom,
            label: "Fathom".into(),
            available: cli.is_some(),
            resolved: cli.as_ref().map(|p| p.display().to_string()),
            version,
            reason: cli.is_none().then(|| {
                "fathom binary not found — build the core workspace or set a custom path".into()
            }),
            models: self.model_catalog(),
            config: cfg.clone(),
        }
    }

    async fn start_session(&self, cfg: &EngineConfig) -> anyhow::Result<Box<dyn EngineSession>> {
        let cli = resolve_cli(cfg, "fathom")
            .ok_or_else(|| anyhow::anyhow!("fathom binary not found — build the core workspace"))?;
        Ok(Box::new(FathomSession { cli, running: None }))
    }
}

pub struct FathomSession {
    cli: std::path::PathBuf,
    running: Option<tokio::task::AbortHandle>,
}

#[async_trait::async_trait]
impl EngineSession for FathomSession {
    async fn send_turn(
        &mut self,
        input: TurnInput,
        events: broadcast::Sender<TurnEvent>,
        _decisions: SharedDecisions,
    ) {
        let mut prompt = input.prompt.clone();
        if let Some(sys) = &input.system_prompt {
            prompt =
                format!("You are a Fathom bot contact with this persona:\n{sys}\n\nUser: {prompt}");
        }
        let mut cmd = Command::new(&self.cli);
        cmd.arg("run").arg(&prompt);
        if let Some(model) = &input.model {
            if model != "default" {
                cmd.env("LLM_MODEL", model);
            }
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &input.cwd {
            cmd.current_dir(cwd);
        }
        let mut child = match cmd.spawn().context("spawn fathom") {
            Ok(c) => c,
            Err(e) => {
                let _ = events.send(TurnEvent::Error(format!("{e:#}")));
                return;
            }
        };
        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = events.send(TurnEvent::Error("fathom stdout missing".into()));
                return;
            }
        };
        let mut lines = BufReader::new(stdout).lines();
        let task = tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = events.send(TurnEvent::Text(format!("{line}\n")));
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
