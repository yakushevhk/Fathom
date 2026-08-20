//! Shared wire types between backend and UI (serde, camelCase on the JS side).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStatus {
    /// Whether a fathom engine is reachable (health OK).
    pub running: bool,
    /// Base URL of the fathom HTTP API, e.g. http://127.0.0.1:4157.
    pub url: Option<String>,
    /// Port the engine listens on.
    pub port: Option<u16>,
    /// Path to the fathom binary we manage.
    pub binary: Option<String>,
    /// Engine version string reported by the managed binary's `--version` probe when known.
    pub version: Option<String>,
    /// Human-readable phase: "starting" | "running" | "stopped" | "error".
    pub phase: String,
    /// Error message when startup failed (phase == "error").
    pub error: Option<String>,
}

impl Default for DaemonStatus {
    fn default() -> Self {
        Self {
            running: false,
            url: None,
            port: None,
            binary: None,
            version: None,
            phase: "stopped".into(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartOptions {
    /// Port to bind the engine on. None = pick a free port.
    pub port: Option<u16>,
    /// Run the engine even if one is already reachable elsewhere
    /// (default: attach to whatever is already up).
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub status: String,
    pub query: String,
    pub output_dir: String,
    pub error: Option<String>,
}