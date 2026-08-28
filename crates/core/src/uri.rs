use std::path::{Component, Path, PathBuf};
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
    /// SQLite virtual table query: `db.sqlite:<table>[:<pk>]`
    Sqlite {
        db_path: PathBuf,
        table: String,
        pk: Option<String>,
        query: Option<String>,
    },
    /// Archive member virtual reader: `bundle.zip:path/inside`
    Archive {
        archive_path: PathBuf,
        member: String,
    },
    /// Standard filesystem path fallback
    File(PathBuf),
}

fn sanitize_relative_path(p: &str) -> Option<PathBuf> {
    let raw = Path::new(p);
    let mut safe = PathBuf::new();
    for comp in raw.components() {
        match comp {
            Component::Normal(n) => safe.push(n),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if safe.as_os_str().is_empty() {
        None
    } else {
        Some(safe)
    }
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
        } else if (trimmed.ends_with(".sqlite") || trimmed.ends_with(".db") || trimmed.contains(".sqlite:") || trimmed.contains(".db:"))
            && trimmed.contains(':')
        {
            let parts: Vec<&str> = trimmed.split(':').collect();
            let db_path = PathBuf::from(parts[0]);
            let table = parts.get(1).unwrap_or(&"").to_string();
            let pk = parts.get(2).map(|s| s.to_string());
            VirtualUri::Sqlite { db_path, table, pk, query: None }
        } else if trimmed.contains(".zip:") || trimmed.contains(".tar.gz:") || trimmed.contains(".asar:") {
            let (arch, member) = if let Some(pos) = trimmed.find(".zip:") {
                (&trimmed[..pos + 4], &trimmed[pos + 5..])
            } else if let Some(pos) = trimmed.find(".tar.gz:") {
                (&trimmed[..pos + 7], &trimmed[pos + 8..])
            } else if let Some(pos) = trimmed.find(".asar:") {
                (&trimmed[..pos + 5], &trimmed[pos + 6..])
            } else {
                (trimmed, "")
            };
            VirtualUri::Archive {
                archive_path: PathBuf::from(arch),
                member: member.to_string(),
            }
        } else {
            VirtualUri::File(PathBuf::from(trimmed))
        }
    }

    /// Check if the URI represents a virtual scheme rather than a local file path.
    pub fn is_virtual(&self) -> bool {
        !matches!(self, VirtualUri::File(_))
    }

    /// Resolve virtual URI to a canonical host filesystem path if backed by disk.
    /// Hardened against directory traversal (`..`).
    pub fn resolve_to_path(&self, workspace_root: &Path) -> Option<PathBuf> {
        let fathom_home = dirs::home_dir().map(|h| h.join(".fathom")).unwrap_or_else(|| PathBuf::from(".fathom"));
        match self {
            VirtualUri::Skill { name, path } => {
                let safe_name = sanitize_relative_path(name)?;
                let base = fathom_home.join("skills").join(safe_name);
                if let Some(sub) = path {
                    let safe_sub = sanitize_relative_path(sub)?;
                    Some(base.join(safe_sub))
                } else {
                    Some(base.join("SKILL.md"))
                }
            }
            VirtualUri::Rule { name } => {
                let safe_name = sanitize_relative_path(name)?;
                Some(fathom_home.join("rules").join(format!("{}.md", safe_name.display())))
            }
            VirtualUri::Memory { section } => {
                if section == "root" || section.is_empty() {
                    Some(fathom_home.join("memory").join("MEMORY.md"))
                } else {
                    let safe_sec = sanitize_relative_path(section)?;
                    Some(fathom_home.join("memory").join(format!("{}.md", safe_sec.display())))
                }
            }
            VirtualUri::Artifact { id } => {
                let safe_id = sanitize_relative_path(id)?;
                Some(workspace_root.join(".pr-context").join(format!("{}.txt", safe_id.display())))
            }
            VirtualUri::Local { name } => {
                let safe_name = sanitize_relative_path(name)?;
                Some(workspace_root.join(".fathom-local").join(safe_name))
            }
            VirtualUri::File(p) => {
                if p.is_absolute() {
                    Some(p.clone())
                } else {
                    Some(workspace_root.join(p))
                }
            }
            VirtualUri::Agent { .. }
            | VirtualUri::History { .. }
            | VirtualUri::Device { .. }
            | VirtualUri::Sqlite { .. }
            | VirtualUri::Archive { .. } => None,
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
            VirtualUri::parse("skill://git-flow/scripts/test.sh"),
            VirtualUri::Skill {
                name: "git-flow".into(),
                path: Some("scripts/test.sh".into())
            }
        );
        assert_eq!(
            VirtualUri::parse("memory://root"),
            VirtualUri::Memory { section: "root".into() }
        );
        assert_eq!(
            VirtualUri::parse("agent://coder_123/output"),
            VirtualUri::Agent { id: "coder_123".into(), path: Some("output".into()) }
        );
        assert_eq!(
            VirtualUri::parse("local://plan.md"),
            VirtualUri::Local { name: "plan.md".into() }
        );
        assert_eq!(
            VirtualUri::parse("xd://ast_edit"),
            VirtualUri::Device { name: "ast_edit".into() }
        );
    }

    #[test]
    fn test_path_traversal_protection() {
        let ws = Path::new("/workspace");
        let malicious = VirtualUri::parse("local://../../../../etc/passwd");
        assert_eq!(malicious.resolve_to_path(ws), None);

        let safe = VirtualUri::parse("local://plan.md");
        assert_eq!(safe.resolve_to_path(ws), Some(PathBuf::from("/workspace/.fathom-local/plan.md")));
    }
}
