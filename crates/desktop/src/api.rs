//! REST and SSE API client communicating with `fathom serve` (port 8080/custom).

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub query: String,
    pub status: String,
    pub output_dir: Option<String>,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub total_agents: usize,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionSummary>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coworker {
    pub id: String,
    pub name: String,
    pub title: String,
    pub role: String,
    pub prompt: String,
    pub visibility: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub coworker_id: String,
    pub title: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerSessionInfo {
    pub id: String,
    pub agent_id: String,
    pub host: String,
    pub port: u16,
    pub active: bool,
    pub human_control: bool,
    pub current_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerSnapshot {
    pub title: String,
    pub url: String,
    pub aria_tree: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerScreenshotResponse {
    pub image_base64: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub effect: String, // "allow" | "deny" | "ask"
    pub condition: String,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDocument {
    pub version: u32,
    pub rules: Vec<PolicyRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: String,
    pub agent_id: Option<String>,
    pub actor: String,
    pub tool_name: String,
    pub decision: String, // "allowed" | "denied" | "refused" | "escalated"
    pub reason: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub cron: String,
    pub prompt: String,
    pub channel_id: String,
    pub enabled: bool,
    pub last_run: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub vendor: String,
    pub granted_agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub service: String,
    pub created_at: String,
    pub is_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub id: Option<String>,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
    pub thinking: Option<String>,
    pub content: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_output: Option<serde_json::Value>,
    pub status: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    http: Client,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { base_url, http }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn health(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        let resp = self.http.get(&url).send().await?;
        Ok(resp.status().is_success())
    }

    // Sessions
    pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let url = format!("{}/api/v1/sessions", self.base_url);
        let resp = self.http.get(&url).send().await?.json::<SessionListResponse>().await?;
        Ok(resp.sessions)
    }

    pub async fn create_session(&self, query: &str) -> Result<SessionSummary> {
        let url = format!("{}/api/v1/sessions", self.base_url);
        let resp = self.http.post(&url)
            .json(&serde_json::json!({ "query": query }))
            .send().await?
            .json::<SessionSummary>().await?;
        Ok(resp)
    }

    pub async fn cancel_session(&self, id: &str) -> Result<()> {
        let url = format!("{}/api/v1/sessions/{}", self.base_url, id);
        self.http.delete(&url).send().await?;
        Ok(())
    }

    pub async fn steer_session(&self, id: &str, message: &str) -> Result<()> {
        let url = format!("{}/api/v1/sessions/{}/steer", self.base_url, id);
        self.http.post(&url)
            .json(&serde_json::json!({ "message": message }))
            .send().await?;
        Ok(())
    }

    pub async fn answer_question(&self, id: &str, request_id: &str, text: &str) -> Result<()> {
        let url = format!("{}/api/v1/sessions/{}/answer", self.base_url, id);
        self.http.post(&url)
            .json(&serde_json::json!({ "request_id": request_id, "text": text }))
            .send().await?;
        Ok(())
    }

    pub async fn approve_tool(&self, id: &str, request_id: &str, approved: bool) -> Result<()> {
        let url = format!("{}/api/v1/sessions/{}/approve", self.base_url, id);
        self.http.post(&url)
            .json(&serde_json::json!({ "request_id": request_id, "approved": approved }))
            .send().await?;
        Ok(())
    }

    // Coworkers & Channels
    pub async fn list_coworkers(&self) -> Result<Vec<Coworker>> {
        let url = format!("{}/api/v1/coworkers", self.base_url);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            let res = resp.json::<Vec<Coworker>>().await.unwrap_or_default();
            Ok(res)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn list_channels(&self) -> Result<Vec<Channel>> {
        let url = format!("{}/api/v1/channels", self.base_url);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            let res = resp.json::<Vec<Channel>>().await.unwrap_or_default();
            Ok(res)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn create_channel(&self, coworker_id: &str, title: &str) -> Result<Channel> {
        let url = format!("{}/api/v1/channels", self.base_url);
        let resp = self.http.post(&url)
            .json(&serde_json::json!({ "coworker_id": coworker_id, "title": title }))
            .send().await?
            .json::<Channel>().await?;
        Ok(resp)
    }

    // Computer
    pub async fn get_computer_screenshot(&self, agent_id: Option<&str>) -> Result<ComputerScreenshotResponse> {
        let url = match agent_id {
            Some(id) => format!("{}/api/v1/computers/{}/screenshot", self.base_url, id),
            None => format!("{}/api/v1/computers/screenshot", self.base_url),
        };
        let resp = self.http.get(&url).send().await?.json::<ComputerScreenshotResponse>().await?;
        Ok(resp)
    }

    pub async fn take_control(&self, agent_id: Option<&str>) -> Result<()> {
        let url = match agent_id {
            Some(id) => format!("{}/api/v1/computers/{}/control/take", self.base_url, id),
            None => format!("{}/api/v1/computers/control/take", self.base_url),
        };
        self.http.post(&url).send().await?;
        Ok(())
    }

    pub async fn release_control(&self, agent_id: Option<&str>) -> Result<()> {
        let url = match agent_id {
            Some(id) => format!("{}/api/v1/computers/{}/control/release", self.base_url, id),
            None => format!("{}/api/v1/computers/control/release", self.base_url),
        };
        self.http.post(&url).send().await?;
        Ok(())
    }

    pub async fn supply_secret(&self, target_ref: &str, secret: &str) -> Result<()> {
        let url = format!("{}/api/v1/computers/secret", self.base_url);
        self.http.post(&url)
            .json(&serde_json::json!({ "ref": target_ref, "secret": secret }))
            .send().await?;
        Ok(())
    }

    pub async fn navigate(&self, url: &str) -> Result<()> {
        let endpoint = format!("{}/api/v1/computers/navigate", self.base_url);
        self.http.post(&endpoint)
            .json(&serde_json::json!({ "url": url }))
            .send().await?;
        Ok(())
    }

    pub async fn computer_mouse_click(&self, x: i32, y: i32) -> Result<()> {
        let endpoint = format!("{}/api/v1/computers/click", self.base_url);
        self.http.post(&endpoint)
            .json(&serde_json::json!({ "x": x, "y": y }))
            .send().await?;
        Ok(())
    }

    // Governance & Audit
    pub async fn get_policy(&self) -> Result<PolicyDocument> {
        let url = format!("{}/api/v1/governance/policy", self.base_url);
        let resp = self.http.get(&url).send().await?.json::<PolicyDocument>().await?;
        Ok(resp)
    }

    pub async fn update_policy(&self, policy: &PolicyDocument) -> Result<()> {
        let url = format!("{}/api/v1/governance/policy", self.base_url);
        self.http.put(&url).json(policy).send().await?;
        Ok(())
    }

    pub async fn get_audit_log(&self) -> Result<Vec<AuditEntry>> {
        let url = format!("{}/api/v1/governance/audit", self.base_url);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            let res = resp.json::<Vec<AuditEntry>>().await.unwrap_or_default();
            Ok(res)
        } else {
            Ok(Vec::new())
        }
    }

    // Credentials Vault
    pub async fn list_credentials(&self) -> Result<Vec<Credential>> {
        let url = format!("{}/api/v1/credentials", self.base_url);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            let list: Vec<Credential> = resp.json().await?;
            Ok(list)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn store_credential(&self, name: &str, kind: &str, secret: &str) -> Result<()> {
        let url = format!("{}/api/v1/credentials", self.base_url);
        self.http.post(&url)
            .json(&serde_json::json!({ "name": name, "kind": kind, "secret": secret }))
            .send().await?;
        Ok(())
    }

    pub async fn delete_credential(&self, id: &str) -> Result<()> {
        let url = format!("{}/api/v1/credentials/{}", self.base_url, id);
        self.http.delete(&url).send().await?;
        Ok(())
    }

    // Schedules & Routines
    pub async fn list_schedules(&self) -> Result<Vec<Routine>> {
        let url = format!("{}/api/v1/schedules", self.base_url);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let mut routines = Vec::new();
            if let Some(arr) = body.get("schedules").and_then(|v| v.as_array()) {
                for item in arr {
                    routines.push(Routine {
                        id: item.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        name: item.get("coworker_id").and_then(|v| v.as_str()).unwrap_or("Scheduled Task").to_string(),
                        cron: item.get("cron_expression").and_then(|v| v.as_str()).unwrap_or("* * * * *").to_string(),
                        prompt: item.get("query").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                        channel_id: "general".to_string(),
                        enabled: item.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                        last_run: item.get("last_run").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    });
                }
            }
            Ok(routines)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn create_schedule(&self, coworker_id: &str, cron: &str, query: &str) -> Result<()> {
        let url = format!("{}/api/v1/schedules", self.base_url);
        self.http.post(&url)
            .json(&serde_json::json!({
                "coworker_id": coworker_id,
                "cron_expression": cron,
                "query": query,
                "enabled": true,
                "timezone": "UTC"
            }))
            .send().await?;
        Ok(())
    }

    pub async fn toggle_schedule(&self, id: &str, coworker_id: &str, cron: &str, query: &str, enabled: bool) -> Result<()> {
        let url = format!("{}/api/v1/schedules/{}", self.base_url, id);
        self.http.put(&url)
            .json(&serde_json::json!({
                "coworker_id": coworker_id,
                "cron_expression": cron,
                "query": query,
                "enabled": enabled,
                "timezone": "UTC"
            }))
            .send().await?;
        Ok(())
    }

    pub async fn delete_schedule(&self, id: &str) -> Result<()> {
        let url = format!("{}/api/v1/schedules/{}", self.base_url, id);
        self.http.delete(&url).send().await?;
        Ok(())
    }

    // MCP Plugins & Connectors
    pub async fn list_plugins(&self) -> Result<Vec<PluginInfo>> {
        let url = format!("{}/api/v1/plugins", self.base_url);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            let res = resp.json::<Vec<PluginInfo>>().await.unwrap_or_default();
            Ok(res)
        } else {
            Ok(Vec::new())
        }
    }

    // Computers Supervisor
    pub async fn list_computers(&self) -> Result<Vec<ComputerSessionInfo>> {
        let url = format!("{}/api/v1/computers", self.base_url);
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            let res = resp.json::<Vec<ComputerSessionInfo>>().await.unwrap_or_default();
            Ok(res)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn stop_computer(&self, agent_id: &str) -> Result<()> {
        let url = format!("{}/api/v1/computers/{}/stop", self.base_url, agent_id);
        self.http.post(&url).send().await?;
        Ok(())
    }

    pub async fn reset_computer(&self, agent_id: &str) -> Result<()> {
        let url = format!("{}/api/v1/computers/{}/reset", self.base_url, agent_id);
        self.http.post(&url).send().await?;
        Ok(())
    }

    /// Subscribe to real-time agent event stream (SSE) from the server.
    /// Can subscribe globally (`/api/v1/events`) or for a specific session (`/api/v1/sessions/:id/events`).
    pub fn subscribe_events(&self, session_id: Option<&str>) -> reqwest_eventsource::EventSource {
        let url = match session_id {
            Some(id) => format!("{}/api/v1/sessions/{}/events", self.base_url, id),
            None => format!("{}/api/v1/events", self.base_url),
        };
        reqwest_eventsource::EventSource::get(url)
    }
}
