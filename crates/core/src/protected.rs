//! Protected surfaces for self-modifying agents (ouroboros
//! `runtime_mode_policy.py` inspiration).
//!
//! An agent that can edit files (and, in the future, its own code) must not be
//! able to overwrite the surfaces that keep it safe and runnable:
//!
//! - **Safety-critical**: prompts/safety definitions, secrets, the runtime's
//!   own config — corrupting these undermines the agent.
//! - **Frozen contract**: durable schemas and DB files — changing these breaks
//!   persistence.
//!
//! The guard is **case-insensitive** so a write to `.PARALLEL-RESEARCH/...`
//! (HFS+/NTFS) cannot bypass a check written against the lower-case path. When
//! a path matches a protected surface, the write is refused (fail-closed) and
//! the reason surfaced. In-flight knowledge only: this is a *content* guard;
//! it deliberately does not parse the repository it protects.

use std::path::{Path, PathBuf};

/// A named protected surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    /// Agent runtime/safety config — never editable by the agent.
    SafetyConfig,
    /// Durable semantic memory DB file.
    MemoryDb,
    /// Agent code/prompts that define its own behavior.
    OwnCode,
    /// Secrets store — must never be read back raw into a tool result.
    Secrets,
}

impl SurfaceKind {
    pub fn label(&self) -> &'static str {
        match self {
            SurfaceKind::SafetyConfig => "safety/config surface",
            SurfaceKind::MemoryDb => "memory DB surface",
            SurfaceKind::OwnCode => "own-code surface",
            SurfaceKind::Secrets => "secrets surface",
        }
    }
}

/// One matching rule.
#[derive(Debug, Clone)]
struct Rule {
    kind: SurfaceKind,
    names: &'static [&'static str],
    dir_segments: &'static [&'static str],
}

/// Protected file names (lower-cased for comparison).
const SAFETY_FILES: &[&str] = &["config.toml", "config.toml.bak"];
const MEMORY_FILES: &[&str] = &["memory.db", "memory.db-wal", "memory.db-shm"];
const OWN_CODE_FILES: &[&str] = &["bible.md", "safety.md"];
const SECRET_FILES: &[&str] = &["secrets.toml", ".env", "credentials", "credential"];

/// Directory segments that, when present anywhere in the path, mark it protected.
const SAFETY_DIRS: &[&str] = &["config"];
const MEMORY_DIRS: &[&str] = &["memory"];
const CODE_DIRS: &[&str] = &["bible", "prompts"];
const SECRET_DIRS: &[&str] = &["secrets", "env"];

fn rules() -> Vec<Rule> {
    vec![
        Rule { kind: SurfaceKind::SafetyConfig, names: SAFETY_FILES, dir_segments: SAFETY_DIRS },
        Rule { kind: SurfaceKind::MemoryDb, names: MEMORY_FILES, dir_segments: MEMORY_DIRS },
        Rule { kind: SurfaceKind::OwnCode, names: OWN_CODE_FILES, dir_segments: CODE_DIRS },
        Rule { kind: SurfaceKind::Secrets, names: SECRET_FILES, dir_segments: SECRET_DIRS },
    ]
}

/// Decision from a protected-surface check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceVerdict {
    /// Path is not protected; safe to write.
    Allowed,
    /// Path matches a protected surface — refuse the write.
    Denied(SurfaceKind),
}

/// Case-insensitive protected-surface guard. Mutable rules come from the
/// static table in [`rules`]; instances are stateless.
pub struct ProtectedSurfaces;

impl ProtectedSurfaces {
    /// Check whether `path` is protected. Case-insensitive on both file names
    /// and directory segments.
    pub fn check(path: &Path) -> SurfaceVerdict {
        let lower = normalize(path);
        for rule in rules() {
            let file_name = lower.file_name().and_then(|f| f.to_str()).unwrap_or("");
            let name_hit = rule.names.iter().any(|n| *n == file_name);
            let subpath_hit = rule.names.iter().any(|n| lower.to_string_lossy().ends_with(n));
            if name_hit || subpath_hit {
                return SurfaceVerdict::Denied(rule.kind);
            }
            for segment in rule.dir_segments {
                if lower.components().any(|c| {
                    c.as_os_str().to_str().map(|s| s == *segment).unwrap_or(false)
                }) {
                    return SurfaceVerdict::Denied(rule.kind);
                }
            }
        }
        SurfaceVerdict::Allowed
    }
}

/// Lower-case path for comparisons (no encoding of relative/absolute).
fn normalize(path: &Path) -> PathBuf {
    path.components()
        .map(|c| PathBuf::from(c.as_os_str().to_string_lossy().to_lowercase()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_config_surface() {
        let v = ProtectedSurfaces::check(Path::new("/home/u/.parallel-research/config.toml"));
        assert!(matches!(v, SurfaceVerdict::Denied(SurfaceKind::SafetyConfig)));
    }

    #[test]
    fn case_insensitive_block() {
        let v = ProtectedSurfaces::check(Path::new("/tmp/.PARALLEL-RESEARCH/CONFIG.TOML"));
        assert!(matches!(v, SurfaceVerdict::Denied(SurfaceKind::SafetyConfig)));
    }

    #[test]
    fn blocks_memory_db() {
        let v = ProtectedSurfaces::check(Path::new("/home/u/.parallel-research/memory.db"));
        assert!(matches!(v, SurfaceVerdict::Denied(SurfaceKind::MemoryDb)));
    }

    #[test]
    fn blocks_env_in_any_dir() {
        let v = ProtectedSurfaces::check(Path::new("/tmp/proj/.env"));
        assert!(matches!(v, SurfaceVerdict::Denied(SurfaceKind::Secrets)));
    }

    #[test]
    fn allows_ordinary_source_and_readme() {
        // Even though src/ is not a protected dir here, a plain project file is
        // allowed unless it matches a protected name/segment.
        assert_eq!(
            ProtectedSurfaces::check(Path::new("/tmp/proj/README.md")),
            SurfaceVerdict::Allowed
        );
        assert_eq!(
            ProtectedSurfaces::check(Path::new("/tmp/proj/src/main.rs")),
            SurfaceVerdict::Allowed
        );
    }
}
