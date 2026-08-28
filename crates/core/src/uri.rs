use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Parsed virtual URI scheme used across Fathom for zero-copy routing,
/// sandboxed scratchpads, agent coordination, and tool devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VirtualUri {
    /// Skill definition: `skill://<name>` or `skill://<name>/<subpath>`
    Skill { name: String, path: Option<String> },
    /// Declarative governance/behavioral rule: `rule://<name>`
    Rule { name: String },
    /// Memory root / KB selector: `memory://root` or `memory://<section>`
    Memory { section: String },
    /// Subagent output / artifact: `agent://<id>` or `agent://<id>/<child_or_path>`
    Agent { id: String, path: Option<String> },
    /// Subagent execution transcript: `history://<id>` or `history://` (all)
    History { id: Option<String> },
    /// Spilled output artifact from disk buffer: `artifact://<id>`
    Artifact { id: String },
    /// Shared inter-agent scratchpad / markdown plan: `local://<name>.md`
    Local { name: String },
    /// Virtual tool device: `xd://<device_name>`
    Device { name: String },
    /// Standard filesystem path fallback
    File(PathBuf),
}

impl VirtualUri {
    /// Parse a string URI into a VirtualUri enum.
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        if let Some(rest) = trimmed.strip_prefix("skill://") {
            let parts: Vec<&str> = rest.splitn(2, '/').collect();
            let name = parts[0].to_string();
            let path = if parts.len() > 1 && !parts[1].is_empty() {
                Some(parts[1].to_string())
            } else {
                None
            };
            VirtualUri::Skill { name, path }
        } else if let Some(rest) = trimmed.strip_prefix("rule://") {
            VirtualUri::Rule { name: rest.trim_matches('/').to_string() }
        } else if let Some(rest) = trimmed.strip_prefix("memory://") {
            VirtualUri::Memory { section: rest.trim_matches('/').to_string() }
        } else if let Some(rest) = trimmed.strip_prefix("agent://") {
            let parts: Vec<&str> = rest.splitn(2, '/').collect();
            let id = parts[0].to_string();
            let path = if parts.len() > 1 && !parts[1].is_empty() {
                Some(parts[1].to_string())
            } else {
                None
            };
            VirtualUri::Agent { id, path }
        } else if let Some(rest) = trimmed.strip_prefix("history://") {
            let id = rest.trim_matches('/');
            VirtualUri::History {
                id: if id.is_empty() { None } else { Some(id.to_string()) },
            }
        } else if let Some(rest) = trimmed.strip_prefix("artifact://") {
            VirtualUri::Artifact { id: rest.trim_matches('/').to_string() }
        } else if let Some(rest) = trimmed.strip_prefix("local://") {
            VirtualUri::Local { name: rest.trim_matches('/').to_string() }
        } else if let Some(rest) = trimmed.strip_prefix("xd://") {
            VirtualUri::Device { name: rest.trim_matches('/').to_string() }
        } else {
            VirtualUri::File(PathBuf::from(trimmed))
        }
    }

    /// Check if the URI represents a virtual scheme rather than a local file path.
    pub fn is_virtual(&self) -> bool {
        !matches!(self, VirtualUri::File(_))
    }

    /// Resolve virtual URI to a canonical host filesystem path if backed by disk.
    pub fn resolve_to_path(&self, workspace_root: &Path) -> Option<PathBuf> {
        let fathom_home = dirs::home_dir().map(|h| h.join(".fathom")).unwrap_or_else(|| PathBuf::from(".fathom"));
        match self {
            VirtualUri::Skill { name, path } => {
                let base = fathom_home.join("skills").join(name);
                if let Some(sub) = path {
                    Some(base.join(sub))
                } else {
                    Some(base.join("SKILL.md"))
                }
            }
            VirtualUri::Rule { name } => {
                Some(fathom_home.join("rules").join(format!("{}.md", name)))
            }
            VirtualUri::Memory { section } => {
                if section == "root" || section.is_empty() {
                    Some(fathom_home.join("memory").join("MEMORY.md"))
                } else {
                    Some(fathom_home.join("memory").join(format!("{}.md", section)))
                }
            }
            VirtualUri::Artifact { id } => {
                Some(workspace_root.join(".pr-context").join(format!("{}.txt", id)))
            }
            VirtualUri::Local { name } => {
                Some(workspace_root.join(".fathom-local").join(name))
            }
            VirtualUri::File(p) => {
                if p.is_absolute() {
                    Some(p.clone())
                } else {
                    Some(workspace_root.join(p))
                }
            }
            VirtualUri::Agent { .. } | VirtualUri::History { .. } | VirtualUri::Device { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_virtual_uris() {
        assert_eq!(
            VirtualUri::parse("skill://git-flow"),
            VirtualUri::Skill { name: "git-flow".into(), path: None }
        );
        assert_eq!(
            VirtualUri::parse("skill://git-flow/docs/guide.md"),
            VirtualUri::Skill { name: "git-flow".into(), path: Some("docs/guide.md".into()) }
        );
        assert_eq!(
            VirtualUri::parse("rule://security"),
            VirtualUri::Rule { name: "security".into() }
        );
        assert_eq!(
            VirtualUri::parse("memory://root"),
            VirtualUri::Memory { section: "root".into() }
        );
        assert_eq!(
            VirtualUri::parse("agent://coder_123/diff"),
            VirtualUri::Agent { id: "coder_123".into(), path: Some("diff".into()) }
        );
        assert_eq!(
            VirtualUri::parse("history://subagent_456"),
            VirtualUri::History { id: Some("subagent_456".into()) }
        );
        assert_eq!(
            VirtualUri::parse("history://"),
            VirtualUri::History { id: None }
        );
        assert_eq!(
            VirtualUri::parse("artifact://art_789"),
            VirtualUri::Artifact { id: "art_789".into() }
        );
        assert_eq!(
            VirtualUri::parse("local://plan.md"),
            VirtualUri::Local { name: "plan.md".into() }
        );
        assert_eq!(
            VirtualUri::parse("xd://ast_edit"),
            VirtualUri::Device { name: "ast_edit".into() }
        );
        assert_eq!(
            VirtualUri::parse("src/main.rs"),
            VirtualUri::File(PathBuf::from("src/main.rs"))
        );
    }
}
