//! Process-global registry of long-running daemon processes.
//!
//! Tracks child processes that serve as development servers, watchers,
//! REPLs, or any long-lived sidecar. Each daemon is assigned a unique name
//! and can be started with readiness conditions (port binding, log regex).
//!
//! This is the data layer; the [`DaemonTool`](crate::daemon_tool::DaemonTool)
//! exposes management operations to agents via the `daemon` tool.

use crate::ids::AgentId;
use std::collections::HashMap;
use std::sync::LazyLock;
use parking_lot::Mutex;
use std::time::SystemTime;

/// Status of a daemon process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    Starting,
    Running,
    Stopped,
    Failed,
}

/// Metadata about a tracked daemon.
#[derive(Debug, Clone)]
pub struct DaemonInfo {
    pub name: String,
    pub shell: String,
    pub status: DaemonStatus,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub created_by: AgentId,
    pub started_at: SystemTime,
    pub last_heartbeat: SystemTime,
}

/// Process-global registry of daemon processes.
pub struct DaemonRegistry {
    daemons: Mutex<HashMap<String, DaemonInfo>>,
}

impl DaemonRegistry {
    fn new() -> Self {
        Self {
            daemons: Mutex::new(HashMap::new()),
        }
    }

    /// Access the process-global singleton.
    pub fn global() -> &'static Self {
        static REG: LazyLock<DaemonRegistry> = LazyLock::new(DaemonRegistry::new);
        &REG
    }

    /// Register a new daemon. Returns false if the name is already active.
    pub fn try_register(&self, info: DaemonInfo) -> bool {
        let mut daemons = self.daemons.lock();
        if daemons.contains_key(&info.name) {
            return false;
        }
        daemons.insert(info.name.clone(), info);
        true
    }

    /// Register or replace a daemon (used by restart).
    pub fn register(&self, info: DaemonInfo) {
        self.daemons.lock().insert(info.name.clone(), info);
    }

    /// Update a daemon's status.
    pub fn update_status(&self, name: &str, status: DaemonStatus) {
        if let Some(d) = self.daemons.lock().get_mut(name) {
            d.status = status;
            d.last_heartbeat = SystemTime::now();
        }
    }

    /// Update a daemon's pid.
    pub fn update_pid(&self, name: &str, pid: u32) {
        if let Some(d) = self.daemons.lock().get_mut(name) {
            d.pid = Some(pid);
            d.last_heartbeat = SystemTime::now();
        }
    }

    /// Update a daemon's port.
    pub fn update_port(&self, name: &str, port: u16) {
        if let Some(d) = self.daemons.lock().get_mut(name) {
            d.port = Some(port);
            d.status = DaemonStatus::Running;
            d.last_heartbeat = SystemTime::now();
        }
    }

    /// Remove a daemon.
    pub fn unregister(&self, name: &str) {
        self.daemons.lock().remove(name);
    }

    /// Get a daemon's info.
    pub fn get(&self, name: &str) -> Option<DaemonInfo> {
        self.daemons.lock().get(name).cloned()
    }

    /// List all daemons.
    pub fn list(&self) -> Vec<DaemonInfo> {
        self.daemons.lock().values().cloned().collect()
    }

    /// List daemons owned by an agent.
    pub fn list_by_owner(&self, agent_id: &AgentId) -> Vec<DaemonInfo> {
        self.daemons
            .lock()
            .values()
            .filter(|d| d.created_by.0 == agent_id.0)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::AgentId;
    use std::time::SystemTime;

    #[test]
    fn test_register_and_list() {
        let reg = DaemonRegistry::new();
        let owner = AgentId::new();
        reg.register(DaemonInfo {
            name: "dev-server".to_string(),
            shell: "cargo run".to_string(),
            status: DaemonStatus::Starting,
            pid: None,
            port: None,
            created_by: owner.clone(),
            started_at: SystemTime::now(),
            last_heartbeat: SystemTime::now(),
        });
        assert_eq!(reg.list().len(), 1);
        assert_eq!(reg.list_by_owner(&owner).len(), 1);
    }

    #[test]
    fn test_update_status() {
        let reg = DaemonRegistry::new();
        let owner = AgentId::new();
        reg.register(DaemonInfo {
            name: "test".to_string(),
            shell: "echo".to_string(),
            status: DaemonStatus::Starting,
            pid: None,
            port: None,
            created_by: owner,
            started_at: SystemTime::now(),
            last_heartbeat: SystemTime::now(),
        });
        reg.update_status("test", DaemonStatus::Running);
        assert_eq!(reg.get("test").unwrap().status, DaemonStatus::Running);
    }

    #[test]
    fn test_update_port() {
        let reg = DaemonRegistry::new();
        let owner = AgentId::new();
        reg.register(DaemonInfo {
            name: "web".to_string(),
            shell: "python -m http.server".to_string(),
            status: DaemonStatus::Starting,
            pid: None,
            port: None,
            created_by: owner,
            started_at: SystemTime::now(),
            last_heartbeat: SystemTime::now(),
        });
        reg.update_port("web", 8080);
        let info = reg.get("web").unwrap();
        assert_eq!(info.port, Some(8080));
        assert_eq!(info.status, DaemonStatus::Running);
    }
}