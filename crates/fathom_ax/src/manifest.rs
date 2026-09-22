//! AX resource manifests (`ax.io/v1alpha1`).
//!
//! Faithful Rust port of the kinds defined by google/ax (`pkg/apis/v1alpha1`):
//! `Task`, `Gateway`, `Workspace` and `Model`, plus `TaskStatus` and
//! `Condition`. Multi-document YAML streams are decoded strictly — unknown or
//! misspelled fields are rejected, mirroring the upstream protojson bridge.

use serde::{Deserialize, Serialize};

use crate::error::{AxError, AxResult};

pub const API_VERSION: &str = "ax.io/v1alpha1";
pub const DEFAULT_ATESPACE: &str = "default";

pub mod kind {
    pub const TASK: &str = "Task";
    pub const GATEWAY: &str = "Gateway";
    pub const WORKSPACE: &str = "Workspace";
    pub const MODEL: &str = "Model";
}

pub mod phase {
    pub const PENDING: &str = "Pending";
    pub const RUNNING: &str = "Running";
    pub const SUSPENDED: &str = "Suspended";
    pub const FAILED: &str = "Failed";
    pub const COMPLETED: &str = "Completed";
    pub const INTERRUPTED: &str = "Interrupted";
    pub const TERMINATING: &str = "Terminating";

    pub fn is_terminal(p: &str) -> bool {
        matches!(p, COMPLETED | FAILED)
    }
}

// ── ObjectMeta ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectMeta {
    pub name: String,
    #[serde(default)]
    pub atespace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_timestamp: Option<String>,
}

impl ObjectMeta {
    pub fn key(&self) -> String {
        format!("{}/{}", self.atespace_or_default(), self.name)
    }
    pub fn atespace_or_default(&self) -> &str {
        if self.atespace.is_empty() {
            DEFAULT_ATESPACE
        } else {
            &self.atespace
        }
    }
}

// ── Task ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvVar {
    pub name: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceList {
    #[serde(default)]
    pub cpu: String,
    #[serde(default)]
    pub memory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceReqs {
    #[serde(default)]
    pub requests: ResourceList,
    #[serde(default)]
    pub limits: ResourceList,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceRef {
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub goal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayRef {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskSpec {
    /// When true the actor is created but kept suspended.
    #[serde(default)]
    pub suspend: bool,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvVar>,
    #[serde(default)]
    pub resources: ResourceReqs,
    #[serde(default)]
    pub workspaces: Vec<WorkspaceRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<GatewayRef>,
    /// Serve guest services (process exec / file access) — maps to the
    /// interactive `ax logs`/`describe` surfaces in this runtime.
    #[serde(default)]
    pub debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingApproval {
    pub id: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub requested_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageStats {
    #[serde(default)]
    pub prompt_tokens: i64,
    #[serde(default)]
    pub completion_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Condition {
    #[serde(rename = "type")]
    pub cond_type: String,
    pub status: String,
    #[serde(default)]
    pub last_transition_time: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskStatus {
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub actor: String,
    #[serde(rename = "workerIP", default)]
    pub worker_ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_approval: Option<PendingApproval>,
    #[serde(default)]
    pub usage: UsageStats,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// PID of the actor process (runtime-internal bookkeeping).
    #[serde(default)]
    pub pid: i64,
    /// Absolute path of the actor's log file.
    #[serde(default)]
    pub log_path: String,
    /// Sequence number of the last event applied to this task.
    #[serde(default)]
    pub last_seq: i64,
    /// Fork provenance: source task key + event sequence the chain
    /// diverged at.
    #[serde(default)]
    pub forked_from: String,
    #[serde(default)]
    pub fork_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Task {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    #[serde(default)]
    pub spec: TaskSpec,
    #[serde(default)]
    pub status: TaskStatus,
}

// ── Gateway ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Listener {
    pub name: String,
    pub port: i32,
    #[serde(default)]
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostRule {
    pub host: String,
    #[serde(default)]
    pub port: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EgressAllowlist {
    #[serde(default)]
    pub hosts: Vec<HostRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EgressConfig {
    #[serde(default)]
    pub allowlist: EgressAllowlist,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewaySpec {
    #[serde(default)]
    pub listeners: Vec<Listener>,
    #[serde(default)]
    pub egress: EgressConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Gateway {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    #[serde(default)]
    pub spec: GatewaySpec,
}

// ── Workspace ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitRepo {
    pub name: String,
    pub repo: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub dir: String,
    #[serde(default)]
    pub depth: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServer {
    pub name: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRegistry {
    pub provider: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub servers: Vec<McpServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpConfig {
    #[serde(default)]
    pub registries: Vec<McpRegistry>,
    #[serde(default)]
    pub servers: Vec<McpServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillRegistry {
    pub provider: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillsConfig {
    #[serde(default)]
    pub registries: Vec<SkillRegistry>,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSpec {
    #[serde(default)]
    pub git: Vec<GitRepo>,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub skills: SkillsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workspace {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    #[serde(default)]
    pub spec: WorkspaceSpec,
}

// ── Model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecretKeyRef {
    pub name: String,
    #[serde(default)]
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelSpec {
    pub provider: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<SecretKeyRef>,
    /// Free-form provider parameters passed through verbatim
    /// (e.g. temperature, maxOutputTokens).
    #[serde(default)]
    pub parameters: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Model {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    #[serde(default)]
    pub spec: ModelSpec,
}

// ── Top-level manifest envelope ─────────────────────────────────────────

// Task carries the full spec+status (~600B): manifests are created once per
// apply on the single-writer path, not in hot loops — an allow is cheaper
// than Box-ing every task reference through the call graph.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AxManifest {
    Task(Task),
    Gateway(Gateway),
    Workspace(Workspace),
    Model(Model),
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "apiVersion")]
    api_version: String,
    kind: String,
}

impl AxManifest {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Task(_) => kind::TASK,
            Self::Gateway(_) => kind::GATEWAY,
            Self::Workspace(_) => kind::WORKSPACE,
            Self::Model(_) => kind::MODEL,
        }
    }

    pub fn metadata(&self) -> &ObjectMeta {
        match self {
            Self::Task(t) => &t.metadata,
            Self::Gateway(g) => &g.metadata,
            Self::Workspace(w) => &w.metadata,
            Self::Model(m) => &m.metadata,
        }
    }

    pub fn metadata_mut(&mut self) -> &mut ObjectMeta {
        match self {
            Self::Task(t) => &mut t.metadata,
            Self::Gateway(g) => &mut g.metadata,
            Self::Workspace(w) => &mut w.metadata,
            Self::Model(m) => &mut m.metadata,
        }
    }

    pub fn api_version(&self) -> &str {
        match self {
            Self::Task(t) => &t.api_version,
            Self::Gateway(g) => &g.api_version,
            Self::Workspace(w) => &w.api_version,
            Self::Model(m) => &m.api_version,
        }
    }

    /// Parse a possibly multi-document YAML stream into manifests.
    /// Empty documents are skipped.
    pub fn parse_documents(yaml: &str) -> AxResult<Vec<AxManifest>> {
        let mut out = Vec::new();
        for doc in serde_yaml::Deserializer::from_str(yaml) {
            let value = serde_yaml::Value::deserialize(doc)?;
            if matches!(value, serde_yaml::Value::Null) {
                continue;
            }
            let env: Envelope = serde_yaml::from_value(value.clone())?;
            if env.api_version != API_VERSION {
                return Err(AxError::Validation(format!(
                    "unsupported apiVersion '{}' (expected '{API_VERSION}')",
                    env.api_version
                )));
            }
            let manifest = match env.kind.as_str() {
                kind::TASK => AxManifest::Task(serde_yaml::from_value(value)?),
                kind::GATEWAY => AxManifest::Gateway(serde_yaml::from_value(value)?),
                kind::WORKSPACE => AxManifest::Workspace(serde_yaml::from_value(value)?),
                kind::MODEL => AxManifest::Model(serde_yaml::from_value(value)?),
                other => {
                    return Err(AxError::Validation(format!(
                        "unknown manifest kind '{other}'"
                    )))
                }
            };
            manifest.validate()?;
            out.push(manifest);
        }
        Ok(out)
    }

    /// Validate a manifest document beyond schema shape: cross-field and
    /// semantic rules enforced by the upstream controller.
    pub fn validate(&self) -> AxResult<()> {
        if self.api_version() != API_VERSION {
            return Err(AxError::Validation(format!(
                "unsupported apiVersion '{}' (expected '{API_VERSION}')",
                self.api_version()
            )));
        }
        let meta = self.metadata();
        if meta.name.is_empty() {
            return Err(AxError::Validation("metadata.name is required".into()));
        }
        if meta.name.len() > 253
            || !meta
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
        {
            return Err(AxError::Validation(format!(
                "invalid metadata.name '{}' (alphanumerics, '-', '.', '_' only)",
                meta.name
            )));
        }
        match self {
            Self::Task(t) => {
                for ws in &t.spec.workspaces {
                    if ws.name.is_empty() {
                        return Err(AxError::Validation(
                            "spec.workspaces[].name is required".into(),
                        ));
                    }
                }
                for e in &t.spec.env {
                    if e.name.is_empty() {
                        return Err(AxError::Validation("spec.env[].name is required".into()));
                    }
                }
            }
            Self::Gateway(g) => {
                for l in &g.spec.listeners {
                    if !(1..=65535).contains(&l.port) {
                        return Err(AxError::Validation(format!(
                            "listener '{}' port {} out of range",
                            l.name, l.port
                        )));
                    }
                    if !l.protocol.is_empty()
                        && !matches!(
                            l.protocol.to_ascii_lowercase().as_str(),
                            "grpc" | "http" | "https" | "tcp"
                        )
                    {
                        return Err(AxError::Validation(format!(
                            "listener '{}' unknown protocol '{}'",
                            l.name, l.protocol
                        )));
                    }
                }
                for h in &g.spec.egress.allowlist.hosts {
                    if h.host.is_empty() {
                        return Err(AxError::Validation(
                            "egress allowlist host must not be empty".into(),
                        ));
                    }
                    if !(0..=65535).contains(&h.port) {
                        return Err(AxError::Validation(format!(
                            "egress host '{}' port {} out of range",
                            h.host, h.port
                        )));
                    }
                }
            }
            Self::Workspace(w) => {
                for g in &w.spec.git {
                    if g.repo.is_empty() {
                        return Err(AxError::Validation(format!(
                            "git repo '{}' requires a repo URL",
                            g.name
                        )));
                    }
                }
            }
            Self::Model(m) => {
                if m.spec.provider.is_empty() || m.spec.model.is_empty() {
                    return Err(AxError::Validation(
                        "model requires spec.provider and spec.model".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}
