//! OpenAI-compatible chat driver — backs `grok` (api.x.ai/v1) and the generic
//! `openai-compat` engine. SSE `chat/completions` streaming, Bearer key from a
//! configurable env var.

use crate::engine::*;
use fathom_core::types::*;
use futures::StreamExt;
use serde_json::json;
use tokio::sync::broadcast;

pub struct OpenAiChatDriver {
    kind: EngineKind,
    label: &'static str,
    default_url: &'static str,
    default_key_env: &'static str,
    models: &'static [(&'static str, &'static str, bool)],
}

pub fn grok_driver() -> OpenAiChatDriver {
    OpenAiChatDriver {
        kind: EngineKind::Grok,
        label: "Grok",
        default_url: "https://api.x.ai/v1",
        default_key_env: "XAI_API_KEY",
        models: &[
            ("grok-4", "Grok 4", true),
            ("grok-4-fast", "Grok 4 Fast", false),
            ("grok-3-mini", "Grok 3 Mini", false),
        ],
    }
}

pub fn openai_compat_driver() -> OpenAiChatDriver {
    OpenAiChatDriver {
        kind: EngineKind::OpenAiCompat,
        label: "OpenAI-compatible",
        default_url: "https://api.openai.com/v1",
        default_key_env: "OPENAI_API_KEY",
        models: &[
            ("gpt-4o", "GPT-4o", true),
            ("gpt-4o-mini", "GPT-4o mini", false),
        ],
    }
}

#[async_trait::async_trait]
impl EngineDriver for OpenAiChatDriver {
    fn kind(&self) -> EngineKind {
        self.kind
    }

    fn model_catalog(&self) -> Vec<ModelInfo> {
        self.models
            .iter()
            .map(|(id, label, def)| ModelInfo {
                id: (*id).into(),
                label: (*label).into(),
                default: *def,
            })
            .collect()
    }

    async fn probe(&self, cfg: &EngineConfig) -> EngineStatus {
        let url = cfg.url.clone().unwrap_or_else(|| self.default_url.into());
        let key_env = cfg
            .api_key_env
            .clone()
            .unwrap_or_else(|| self.default_key_env.into());
        let key_set = std::env::var(&key_env)
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        EngineStatus {
            kind: self.kind,
            label: self.label.into(),
            available: key_set,
            resolved: Some(url),
            version: None,
            reason: (!key_set)
                .then(|| format!("{key_env} not set — add the key to use {0}", self.label)),
            models: self.model_catalog(),
            config: cfg.clone(),
        }
    }

    async fn start_session(&self, cfg: &EngineConfig) -> anyhow::Result<Box<dyn EngineSession>> {
        let url = cfg.url.clone().unwrap_or_else(|| self.default_url.into());
        let key_env = cfg
            .api_key_env
            .clone()
            .unwrap_or_else(|| self.default_key_env.into());
        let model = cfg
            .model
            .clone()
            .or_else(|| self.models.iter().find(|m| m.2).map(|m| m.0.to_string()))
            .unwrap_or_else(|| self.models[0].0.to_string());
        Ok(Box::new(OpenAiChatSession {
            url,
            key_env,
            default_model: model,
            history: Vec::new(),
            client: reqwest::Client::new(),
        }))
    }
}

pub struct OpenAiChatSession {
    url: String,
    key_env: String,
    default_model: String,
    /// Chat history for this thread (server persists transcripts too; this is
    /// the short in-session context window sent to the endpoint).
    history: Vec<serde_json::Value>,
    client: reqwest::Client,
}

#[async_trait::async_trait]
impl EngineSession for OpenAiChatSession {
    async fn send_turn(
        &mut self,
        input: TurnInput,
        events: broadcast::Sender<TurnEvent>,
        _decisions: SharedDecisions,
    ) {
        let key = match std::env::var(&self.key_env) {
            Ok(k) if !k.is_empty() => k,
            _ => {
                let _ = events.send(TurnEvent::Error(format!("{} is not set", self.key_env)));
                return;
            }
        };

        let mut messages = Vec::new();
        if let Some(sys) = &input.system_prompt {
            messages.push(json!({ "role": "system", "content": sys }));
        }
        // Re-send recent context so a fresh process keeps continuity.
        messages.extend(self.history.iter().cloned());
        messages.push(json!({ "role": "user", "content": input.prompt }));

        let body = json!({
            "model": input.model.clone().unwrap_or_else(|| self.default_model.clone()),
            "messages": messages,
            "stream": true,
        });

        let resp = match self
            .client
            .post(format!(
                "{}/chat/completions",
                self.url.trim_end_matches('/')
            ))
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = events.send(TurnEvent::Error(format!("request failed: {e}")));
                return;
            }
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let _ = events.send(TurnEvent::Error(format!(
                "HTTP {status}: {}",
                body.chars().take(400).collect::<String>()
            )));
            return;
        }

        let mut acc = String::new();
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        while let Some(chunk) = stream.next().await {
            let Ok(bytes) = chunk else { break };
            buf.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim_end().to_string();
                buf.drain(..pos + 1);
                if !line.starts_with("data:") {
                    continue;
                }
                let data = line[5..].trim();
                if data == "[DONE]" {
                    break;
                }
                let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };
                if let Some(delta) = v
                    .pointer("/choices/0/delta/content")
                    .and_then(|d| d.as_str())
                {
                    acc.push_str(delta);
                    let _ = events.send(TurnEvent::Text(delta.to_string()));
                }
                if let Some(rc) = v
                    .pointer("/choices/0/delta/reasoning_content")
                    .and_then(|d| d.as_str())
                {
                    let _ = events.send(TurnEvent::Thinking(rc.to_string()));
                }
            }
        }

        self.history
            .push(json!({ "role": "user", "content": input.prompt }));
        self.history
            .push(json!({ "role": "assistant", "content": acc }));
        // Cap context at ~40 messages.
        let keep = self.history.len().min(40);
        self.history = self.history.split_off(self.history.len() - keep);

        let _ = events.send(TurnEvent::Done { text: None });
    }

    async fn abort(&mut self) {}
}
