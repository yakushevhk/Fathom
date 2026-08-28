//! Docker-backed lifecycle supervisor for per-agent computer environments.
//!
//! The Docker client is intentionally private to this crate: callers can only
//! create, stop, reset, and inspect an agent's managed container.

use bollard::{
    container::{Config as ContainerConfig, CreateContainerOptions, ListContainersOptions, RemoveContainerOptions, StartContainerOptions, StopContainerOptions},
    secret::{HealthConfig, HostConfig, Mount, MountTypeEnum, PortBinding},
    Docker,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fmt, future::Future, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::time::timeout;

const DEFAULT_IMAGE: &str = "fathom/computer:latest";
const DEFAULT_NETWORK: &str = "fathom-computer";
const DEFAULT_BASE_PORT: u16 = 19_000;
const CONTAINER_PORT: u16 = 8765;
const OP_TIMEOUT: Duration = Duration::from_secs(20);
const VOLUME_ROOT: &str = "/data/browser";
const PROFILE_ROOT: &str = "/data/profile";
const LABEL_MANAGED: &str = "io.fathom.supervisor";
const LABEL_AGENT: &str = "io.fathom.agent-id";
pub mod host;
pub use host::*;

/// Configuration for the supervisor. Values can be supplied explicitly or
/// loaded from `COMPUTER_*` environment variables with [`Self::from_env`].
#[derive(Clone, Serialize, Deserialize)]
pub struct SupervisorConfig {
    pub image: String,
    pub network: String,
    pub token: String,
    pub base_port: u16,
    /// Container's internal HTTP port. Kept configurable for custom images.
    pub container_port: u16,
}

impl SupervisorConfig {
    pub fn new(image: impl Into<String>, network: impl Into<String>, token: impl Into<String>, base_port: u16) -> Result<Self, SupervisorError> {
        let config = Self { image: image.into(), network: network.into(), token: token.into(), base_port, container_port: CONTAINER_PORT };
        config.validate()?;
        Ok(config)
    }

    pub fn from_env() -> Result<Self, SupervisorError> {
        let image = env::var("COMPUTER_IMAGE").unwrap_or_else(|_| DEFAULT_IMAGE.to_owned());
        let network = env::var("COMPUTER_NETWORK").unwrap_or_else(|_| DEFAULT_NETWORK.to_owned());
        let token = env::var("COMPUTER_TOKEN").map_err(|_| SupervisorError::MissingConfig("COMPUTER_TOKEN"))?;
        let base_port = env::var("COMPUTER_BASE_PORT")
            .ok()
            .map(|value| value.parse().map_err(|_| SupervisorError::InvalidConfig("COMPUTER_BASE_PORT must be a valid port")))
            .transpose()?
            .unwrap_or(DEFAULT_BASE_PORT);
        Self::new(image, network, token, base_port)
    }

    fn validate(&self) -> Result<(), SupervisorError> {
        if self.image.trim().is_empty() { return Err(SupervisorError::InvalidConfig("image must not be empty")); }
        if !valid_docker_component(&self.network) { return Err(SupervisorError::InvalidConfig("network contains invalid characters")); }
        if self.token.is_empty() { return Err(SupervisorError::InvalidConfig("token must not be empty")); }
        if self.base_port == 0 { return Err(SupervisorError::InvalidConfig("base_port is invalid")); }
        if self.container_port == 0 { return Err(SupervisorError::InvalidConfig("container_port is invalid")); }
        Ok(())
    }
}

impl fmt::Debug for SupervisorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupervisorConfig")
            .field("image", &self.image)
            .field("network", &self.network)
            .field("token", &"[REDACTED]")
            .field("base_port", &self.base_port)
            .field("container_port", &self.container_port)
            .finish()
    }
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self { image: DEFAULT_IMAGE.into(), network: DEFAULT_NETWORK.into(), token: String::new(), base_port: DEFAULT_BASE_PORT, container_port: CONTAINER_PORT }
    }
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("invalid agent id: {0}")]
    InvalidAgentId(String),
    #[error("invalid supervisor configuration: {0}")]
    InvalidConfig(&'static str),
    #[error("missing supervisor configuration: {0}")]
    MissingConfig(&'static str),
    #[error("docker operation timed out")]
    Timeout,
    #[error("docker error: {0}")]
    Docker(#[from] bollard::errors::Error),
    #[error("docker response did not include a container id")]
    MissingContainerId,
}

/// Metadata returned for a managed agent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentContainer {
    pub agent_id: String,
    pub container_name: String,
    pub workspace_volume: String,
    pub profile_volume: String,
    pub port: u16,
    pub running: bool,
    pub health: Option<String>,
}

/// Supervisor owning Docker lifecycle operations for computer agents.
pub struct ComputerSupervisor {
    docker: Arc<Docker>,
    config: SupervisorConfig,
}

impl fmt::Debug for ComputerSupervisor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("ComputerSupervisor").field("config", &self.config).finish_non_exhaustive() }
}

impl ComputerSupervisor {
    /// Connect to the local Docker daemon and construct a supervisor.
    pub fn new(config: SupervisorConfig) -> Result<Self, SupervisorError> {
        config.validate()?;
        let docker = Docker::connect_with_local_defaults().map_err(SupervisorError::Docker)?;
        Ok(Self { docker: Arc::new(docker), config })
    }

    /// Construct from `COMPUTER_*` environment variables.
    pub fn from_env() -> Result<Self, SupervisorError> { Self::new(SupervisorConfig::from_env()?) }

    /// Ensure the agent container exists and is running, returning its metadata.
    pub async fn ensure(&self, agent_id: &str) -> Result<AgentContainer, SupervisorError> {
        let id = validate_agent_id(agent_id)?;
        let names = Names::for_agent(id, self.config.base_port)?;
        if let Some(summary) = self.inspect_managed(&names.container).await? {
            let running = summary.state.as_deref() == Some("running");
            if !running { self.with_timeout(self.docker.start_container(&names.container, None::<StartContainerOptions<String>>)).await?; }
            return Ok(metadata_from_summary(id, names, true, summary));
        }
        let mut labels = HashMap::new();
        labels.insert(LABEL_MANAGED.to_string(), "true".to_string());
        labels.insert(LABEL_AGENT.to_string(), id.to_string());
        let mut exposed = HashMap::new();
        exposed.insert(format!("{}/tcp", self.config.container_port), HashMap::new());
        let mut bindings = HashMap::new();
        bindings.insert(format!("{}/tcp", self.config.container_port), Some(vec![PortBinding { host_ip: Some("127.0.0.1".into()), host_port: Some(names.port.to_string()) }]));
        let config = ContainerConfig {
            image: Some(self.config.image.clone()),
            env: Some(vec![format!("COMPUTER_TOKEN={}", self.config.token), format!("COMPUTER_WORKSPACE={}", VOLUME_ROOT)]),
            labels: Some(labels),
            exposed_ports: Some(exposed),
            healthcheck: Some(HealthConfig { test: Some(vec!["CMD-SHELL".into(), format!("node -e \"fetch('http://127.0.0.1:{}/health').then(r => process.exit(r.ok ? 0 : 1)).catch(() => process.exit(1))\"", self.config.container_port)]), interval: Some(5_000_000_000), timeout: Some(2_000_000_000), retries: Some(3), start_period: Some(10_000_000_000), start_interval: None }),
            host_config: Some(HostConfig { network_mode: Some(self.config.network.clone()), port_bindings: Some(bindings), mounts: Some(vec![
                Mount { target: Some(VOLUME_ROOT.into()), source: Some(names.workspace_volume.clone()), typ: Some(MountTypeEnum::VOLUME), read_only: Some(false), ..Default::default() },
                Mount { target: Some(PROFILE_ROOT.into()), source: Some(names.profile_volume.clone()), typ: Some(MountTypeEnum::VOLUME), read_only: Some(false), ..Default::default() },
            ]), cap_drop: Some(vec!["ALL".into()]), security_opt: Some(vec!["no-new-privileges:true".into()]), ..Default::default() }),
            ..Default::default()
        };
        self.with_timeout(self.docker.create_container(Some(CreateContainerOptions { name: &names.container, platform: None }), config)).await?;
        self.with_timeout(self.docker.start_container(&names.container, None::<StartContainerOptions<String>>)).await?;
        Ok(AgentContainer { agent_id: id.to_string(), container_name: names.container, workspace_volume: names.workspace_volume, profile_volume: names.profile_volume, port: names.port, running: true, health: None })
    }

    pub async fn stop(&self, agent_id: &str) -> Result<(), SupervisorError> {
        let id = validate_agent_id(agent_id)?;
        let name = Names::for_agent(id, self.config.base_port)?.container;
        match self.with_timeout(self.docker.stop_container(&name, Some(StopContainerOptions { t: 10 }))).await {
            Ok(()) => Ok(()),
            Err(SupervisorError::Docker(bollard::errors::Error::DockerResponseServerError { status_code: 404, .. })) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Stop and remove a container while retaining the named workspace/profile volumes.
    pub async fn reset(&self, agent_id: &str) -> Result<(), SupervisorError> {
        let id = validate_agent_id(agent_id)?;
        let name = Names::for_agent(id, self.config.base_port)?.container;
        match self.with_timeout(self.docker.remove_container(&name, Some(RemoveContainerOptions { force: true, ..Default::default() }))).await {
            Ok(()) | Err(SupervisorError::Docker(bollard::errors::Error::DockerResponseServerError { status_code: 404, .. })) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn list(&self) -> Result<Vec<AgentContainer>, SupervisorError> {
        let mut filters = HashMap::new();
        filters.insert("label".to_string(), vec![format!("{}=true", LABEL_MANAGED)]);
        let summaries = self.with_timeout(self.docker.list_containers(Some(ListContainersOptions { all: true, filters, ..Default::default() }))).await?;
        Ok(summaries.into_iter().filter_map(|summary| {
            let labels = summary.labels.as_ref()?;
            let agent_id = labels.get(LABEL_AGENT)?.clone();
            let names = Names::for_agent(&agent_id, self.config.base_port).ok()?;
            Some(metadata_from_summary(&agent_id, names, summary.state.as_deref() == Some("running"), summary))
        }).collect())
    }

    async fn inspect_managed(&self, name: &str) -> Result<Option<bollard::models::ContainerSummary>, SupervisorError> {
        let mut filters = HashMap::new();
        filters.insert("name".to_string(), vec![name.to_string()]);
        let entries = self.with_timeout(self.docker.list_containers(Some(ListContainersOptions { all: true, filters, ..Default::default() }))).await?;
        Ok(entries.into_iter().find(|entry| entry.names.as_ref().map(|ns| ns.iter().any(|n| n.trim_start_matches('/') == name)).unwrap_or(false)))
    }

    async fn with_timeout<T>(&self, operation: impl Future<Output = Result<T, bollard::errors::Error>>) -> Result<T, SupervisorError> {
        timeout(OP_TIMEOUT, operation).await.map_err(|_| SupervisorError::Timeout)?.map_err(SupervisorError::Docker)
    }
}

#[derive(Clone, Debug)]
struct Names { container: String, workspace_volume: String, profile_volume: String, port: u16 }
impl Names {
    fn for_agent(agent_id: &str, base_port: u16) -> Result<Self, SupervisorError> {
        validate_agent_id(agent_id)?;
        let hash = stable_agent_hash(agent_id);
        let offset = u16::from_str_radix(&hash[..4], 16).unwrap_or(0) % 1000;
        let port = base_port.checked_add(offset).ok_or(SupervisorError::InvalidConfig("base port range exhausted"))?;
        Ok(Self { container: format!("fathom-computer-{hash}"), workspace_volume: format!("fathom-workspace-{hash}"), profile_volume: format!("fathom-profile-{hash}"), port })
    }
}

fn metadata_from_summary(agent_id: &str, names: Names, running: bool, summary: bollard::models::ContainerSummary) -> AgentContainer {
    AgentContainer { agent_id: agent_id.to_string(), container_name: names.container, workspace_volume: names.workspace_volume, profile_volume: names.profile_volume, port: names.port, running, health: summary.status }
}

/// Validate the externally supplied identifier before it reaches Docker names or labels.
pub fn validate_agent_id(agent_id: &str) -> Result<&str, SupervisorError> {
    if agent_id.is_empty() || agent_id.len() > 64 || !agent_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.') || agent_id.starts_with('.') || agent_id.ends_with('.') {
        return Err(SupervisorError::InvalidAgentId(agent_id.to_string()));
    }
    Ok(agent_id)
}

fn valid_docker_component(value: &str) -> bool { !value.is_empty() && value.len() <= 63 && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.') }
fn stable_agent_hash(agent_id: &str) -> String {
    // FNV-1a gives deterministic, allocation-free naming without exposing the raw id.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in agent_id.bytes() { hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3); }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn validates_ids() { assert!(validate_agent_id("agent-1.alpha").is_ok()); assert!(validate_agent_id("").is_err()); assert!(validate_agent_id("../escape").is_err()); assert!(validate_agent_id("agent/name").is_err()); }
    #[test] fn names_are_deterministic_and_private() { let a = Names::for_agent("agent-1", 19000).unwrap(); let b = Names::for_agent("agent-1", 19000).unwrap(); assert_eq!(a.container, b.container); assert!(!a.container.contains("agent-1")); assert!(a.port >= 19000); }
}
