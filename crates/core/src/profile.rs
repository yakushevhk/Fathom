//! Personas / profiles: named TOML presets that tune the fleet for a class
//! of tasks.
//!
//! A profile is a small declarative overlay applied on top of
//! `~/.fathom/config.toml`:
//!
//! - an extra system-prompt block injected into every agent;
//! - optional overrides for the main/fast model, temperature and depth;
//! - extra tools denied for every role.
//!
//! Profiles live in `~/.fathom/profiles/<name>.toml`; three
//! presets (`hunter`, `analyst`, `validator`) are built in and available
//! without any files.
//!
//! ```bash
//! fathom profiles list
//! fathom run --profile hunter "find decision makers at Acme"
//! ```

use crate::config::AppConfig;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One named persona.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Extra system-prompt block injected into every agent of the session.
    #[serde(default)]
    pub prompt: String,
    /// Override `[llm] model` (strong model: planning, reports).
    #[serde(default)]
    pub model: Option<String>,
    /// Override `[llm] fast_model` (cheap model: extraction, classify).
    #[serde(default)]
    pub fast_model: Option<String>,
    /// Override `[llm] temperature`.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Override `[agent] max_depth`.
    #[serde(default)]
    pub max_depth: Option<u32>,
    /// Override `[agent] max_agents`.
    #[serde(default)]
    pub max_agents: Option<u32>,
    /// Override `[agent] max_iterations` (turn budget per agent).
    #[serde(default)]
    pub max_iterations: Option<u32>,
    /// Override `[agent] timeout_seconds` (per-agent wall-clock timeout).
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// Override `[agent] replan_rounds` (Goal Mode gap-filling rounds; 0
    /// disables replanning entirely).
    #[serde(default)]
    pub replan_rounds: Option<u32>,
    /// Tools denied for every role (merged into `[agent] deny_tools`).
    #[serde(default)]
    pub deny_tools: Vec<String>,
}

/// Directory holding user profile files.
pub fn profiles_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".fathom").join("profiles")
}

/// Names of the built-in presets.
pub const BUILTIN_NAMES: &[&str] = &["hunter", "analyst", "validator"];

/// Built-in presets, available without any files on disk.
pub fn built_in(name: &str) -> Option<Profile> {
    match name {
        "hunter" => Some(Profile {
            name: "hunter".into(),
            description: "Aggressive lead harvesting: maximise verified contacts".into(),
            prompt: "## Persona: hunter\n\
                     You are a lead-generation hunter. Priority: harvest as many VERIFIED \
                     contacts as possible.\n\
                     - Prefer sources with direct people pages (team, about, press, LinkedIn posts).\n\
                     - For every person found: extract name, role, company, email, phone.\n\
                     - Always run extract_contacts with enrich_entities and save_contacts on results.\n\
                     - Verify emails when MX checks are cheap; mark unverified ones as such.\n\
                     - Quantity matters, but never invent data: no contact — no row."
                .into(),
            max_agents: Some(6),
            ..Default::default()
        }),
        "analyst" => Some(Profile {
            name: "analyst".into(),
            description: "Deep research & cross-checking, no side effects".into(),
            prompt: "## Persona: analyst\n\
                     You are a research analyst. Priority: depth, sourcing and cross-checking.\n\
                     - Every claim needs at least one source URL; conflicting sources get both cited.\n\
                     - Structure the final report: facts, evidence, confidence, open questions.\n\
                     - Do not collect contacts; focus on companies, markets, numbers and dates.\n\
                     - Prefer primary sources (official sites, filings, releases) over aggregators."
                .into(),
            deny_tools: vec!["save_contacts".into(), "git_push".into()],
            ..Default::default()
        }),
        "validator" => Some(Profile {
            name: "validator".into(),
            description: "Verify and enrich already-collected contacts".into(),
            prompt: "## Persona: validator\n\
                     You are a data validator. Priority: verify and enrich existing contacts.\n\
                     - Work through the provided contact list one by one.\n\
                     - Use verify_email / verify_phone / verify_social; record verdicts.\n\
                     - Enrich missing fields (role, company site, socials) from primary sources.\n\
                     - Never guess: an unverifiable field stays empty and is marked 'unverified'."
                .into(),
            deny_tools: vec!["spawn_agent".into()],
            ..Default::default()
        }),
        _ => None,
    }
}

/// All profiles visible to the CLI: user files override built-ins with the
/// same name.
pub fn list_all() -> Vec<Profile> {
    let mut out: Vec<Profile> = Vec::new();
    let dir = profiles_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                match from_file(&path) {
                    Ok(p) => out.push(p),
                    Err(e) => tracing::warn!("profile {} skipped: {e}", path.display()),
                }
            }
        }
    }
    for name in BUILTIN_NAMES {
        if !out.iter().any(|p| p.name == *name) {
            if let Some(p) = built_in(name) {
                out.push(p);
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Parse one profile file.
pub fn from_file(path: &Path) -> anyhow::Result<Profile> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading profile {}", path.display()))?;
    let mut profile: Profile =
        toml::from_str(&raw).with_context(|| format!("parsing profile {}", path.display()))?;
    if profile.name.trim().is_empty() {
        profile.name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();
    }
    Ok(profile)
}

/// Load a profile by name (user file wins over the built-in preset).
pub fn load(name_or_path: &str) -> anyhow::Result<Profile> {
    // Explicit path first.
    let as_path = PathBuf::from(name_or_path);
    if as_path.extension().is_some() && as_path.exists() {
        return from_file(&as_path);
    }
    let file = profiles_dir().join(format!("{name_or_path}.toml"));
    if file.exists() {
        return from_file(&file);
    }
    built_in(name_or_path).with_context(|| {
        format!(
            "profile '{name_or_path}' not found (checked {} and built-ins: {})",
            file.display(),
            BUILTIN_NAMES.join(", ")
        )
    })
}

impl Profile {
    /// Apply the profile's overrides onto a loaded config (in place).
    pub fn apply(&self, config: &mut AppConfig) {
        if let Some(model) = self.model.as_deref().filter(|m| !m.trim().is_empty()) {
            config.llm.model = model.to_string();
        }
        if let Some(fast) = self.fast_model.as_deref().filter(|m| !m.trim().is_empty()) {
            config.llm.fast_model = fast.to_string();
        }
        if let Some(t) = self.temperature {
            config.llm.temperature = t;
        }
        if let Some(d) = self.max_depth {
            config.agent.max_depth = d;
        }
        if let Some(n) = self.max_agents {
            config.agent.max_agents = n;
        }
        if let Some(n) = self.max_iterations {
            config.agent.max_iterations = n;
        }
        if let Some(t) = self.timeout_seconds {
            config.agent.timeout_seconds = t;
        }
        if let Some(r) = self.replan_rounds {
            config.agent.replan_rounds = r;
        }
        if !self.deny_tools.is_empty() {
            for role in ["coordinator", "researcher", "analyst", "verifier", "writer"] {
                let entry = config.agent.deny_tools.entry(role.to_string()).or_default();
                for tool in &self.deny_tools {
                    if !entry.iter().any(|t| t.eq_ignore_ascii_case(tool)) {
                        entry.push(tool.clone());
                    }
                }
            }
        }
    }

    /// Render a commented TOML template (for `profiles new`).
    pub fn template(name: &str) -> String {
        format!(
            r#"# Fathom profile '{name}'
# Selected with: fathom run --profile {name} "..."

name = "{name}"
description = "what this persona is for"

# Extra system-prompt block injected into every agent.
prompt = """
## Persona: {name}
Describe priorities, style and constraints here.
"""

# Optional overrides (uncomment to use):
# model = "deepseek-chat"        # strong model: planning, reports
# fast_model = "deepseek-chat"   # cheap model: extraction, classify, rerank
# temperature = 0.4
# max_depth = 2
# max_agents = 6
# max_iterations = 60            # turn budget per agent
# replan_rounds = 0              # Goal Mode gap-filling rounds (0 = off)
# deny_tools = ["git_push", "shell"]
"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_ins_load_and_apply() {
        for name in BUILTIN_NAMES {
            let p = built_in(name).expect(name);
            assert_eq!(p.name, *name);
            assert!(!p.prompt.is_empty());
        }
        let hunter = built_in("hunter").unwrap();
        let mut cfg = AppConfig::default();
        hunter.apply(&mut cfg);
        assert_eq!(cfg.agent.max_agents, 6);

        let analyst = built_in("analyst").unwrap();
        let mut cfg = AppConfig::default();
        analyst.apply(&mut cfg);
        assert!(cfg.agent.deny_tools["researcher"].contains(&"save_contacts".to_string()));
        assert!(cfg.agent.deny_tools["writer"].contains(&"git_push".to_string()));
    }

    #[test]
    fn load_prefers_file_over_builtin() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("custom.toml");
        std::fs::write(
            &path,
            "name = \"custom\"\ndescription = \"d\"\nmodel = \"m1\"\nmax_depth = 3\n",
        )
        .unwrap();
        let p = load(path.to_str().unwrap()).unwrap();
        assert_eq!(p.name, "custom");
        assert_eq!(p.model.as_deref(), Some("m1"));
        assert_eq!(p.max_depth, Some(3));
    }

    #[test]
    fn applies_turn_budget_and_replan_overrides() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.agent.max_iterations, 50);
        assert_eq!(cfg.agent.timeout_seconds, 600);
        assert_eq!(cfg.agent.replan_rounds, 1);
        let p = Profile {
            name: "tight".into(),
            max_iterations: Some(60),
            timeout_seconds: Some(1500),
            replan_rounds: Some(0),
            ..Default::default()
        };
        p.apply(&mut cfg);
        assert_eq!(cfg.agent.max_iterations, 60);
        assert_eq!(cfg.agent.timeout_seconds, 1500);
        assert_eq!(cfg.agent.replan_rounds, 0);
    }

    #[test]
    fn load_unknown_profile_errors_helpfully() {
        let err = load("definitely-not-a-profile-xyz").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn template_parses_back() {
        let raw = Profile::template("demo");
        let parsed: Profile = toml::from_str(&raw).unwrap();
        assert_eq!(parsed.name, "demo");
        assert!(parsed.prompt.contains("Persona: demo"));
    }
}
