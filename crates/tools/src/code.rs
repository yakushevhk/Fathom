//! Code intelligence tools: regex-based symbol extraction (code_symbols) and
//! a compact repository map (repo_map). No LSP or tree-sitter required —
//! line-level heuristics over common languages.

use crate::registry::{Tool, ToolContext};
use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

fn default_symbol_limit() -> usize {
    200
}
fn default_map_files() -> usize {
    300
}
fn default_symbols_per_file() -> usize {
    3
}

const NOISE_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    "__pycache__",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
    ".next",
    "vendor",
    ".pytest_cache",
    ".mypy_cache",
];

const MAX_FILE_BYTES: u64 = 2_000_000;
const MAX_FILES_SCANNED: usize = 8000;

/// Regexes used by `extract_symbols`. Built once and reused across all files;
/// `regex::Regex::new` compiles a DFA and is far too expensive to run on
/// every source file (repo_map touches hundreds of files in a row).
fn js_arrow_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"^(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?\(",
        )
        .expect("bad js_arrow regex")
    })
}

fn c_fn_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"^[A-Za-z_][\w:<>,&*\s]*?[\s&*]([A-Za-z_]\w*)\s*\([^;{)]*\)\s*(?:const\s*)?\{?\s*$",
        )
        .expect("bad c_fn regex")
    })
}

fn lang_of(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "py" | "pyi" => Some("python"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "tsx" => Some("typescript"),
        "go" => Some("go"),
        "rb" => Some("ruby"),
        "java" => Some("java"),
        "kt" => Some("kotlin"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "hpp" | "hh" => Some("cpp"),
        "cs" => Some("csharp"),
        "php" => Some("php"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SymbolHit {
    name: String,
    kind: String,
    line: usize,
    signature: String,
}

fn ident_after<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    if end == 0 {
        None
    } else {
        Some(&rest[..end])
    }
}

fn push_if_ident(
    out: &mut Vec<SymbolHit>,
    _line: &str,
    lineno: usize,
    trimmed: &str,
    kind: &str,
    keyword: &str,
) -> bool {
    if let Some(name) = ident_after(trimmed, keyword) {
        let sig: String = trimmed.chars().take(120).collect();
        out.push(SymbolHit {
            name: name.to_string(),
            kind: kind.to_string(),
            line: lineno,
            signature: sig,
        });
        true
    } else {
        false
    }
}

/// Extract symbol definitions from source using language-specific
/// line-prefix heuristics. Conservative on purpose: missing a symbol is
/// acceptable, flooding with false positives is not.
fn extract_symbols(content: &str, lang: &str) -> Vec<SymbolHit> {
    let mut out: Vec<SymbolHit> = Vec::new();

    let js_arrow = js_arrow_re();
    let c_fn = c_fn_re();

    for (idx, raw) in content.lines().enumerate() {
        let lineno = idx + 1;
        let trimmed = raw.trim_start();
        if trimmed.is_empty() {
            continue;
        }

        match lang {
            "rust" => {
                for kw in [
                    "pub async fn ",
                    "pub(crate) async fn ",
                    "async fn ",
                    "pub fn ",
                    "pub(crate) fn ",
                    "pub(super) fn ",
                    "fn ",
                ] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, "fn", kw) {
                        break;
                    }
                    if trimmed.starts_with(kw) {
                        break;
                    }
                }
                for kw in ["pub struct ", "pub(crate) struct ", "struct "] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, "struct", kw) {
                        break;
                    }
                }
                for kw in ["pub enum ", "pub(crate) enum ", "enum "] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, "enum", kw) {
                        break;
                    }
                }
                for kw in ["pub trait ", "pub(crate) trait ", "trait "] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, "trait", kw) {
                        break;
                    }
                }
                if trimmed.starts_with("impl") {
                    let rest = trimmed.trim_start_matches("impl").trim_start();
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
                        .collect();
                    if !name.is_empty() {
                        let sig: String = trimmed.chars().take(120).collect();
                        out.push(SymbolHit {
                            name,
                            kind: "impl".into(),
                            line: lineno,
                            signature: sig,
                        });
                    }
                }
                for kw in ["pub type ", "type "] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, "type", kw) {
                        break;
                    }
                }
                for kw in ["pub mod ", "mod "] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, "mod", kw) {
                        break;
                    }
                }
            }
            "python" => {
                for kw in ["async def ", "def "] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, "fn", kw) {
                        break;
                    }
                }
                push_if_ident(&mut out, raw, lineno, trimmed, "class", "class ");
            }
            "javascript" | "typescript" => {
                let mut matched = false;
                for kw in [
                    "export async function ",
                    "async function ",
                    "export function ",
                    "function ",
                ] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, "fn", kw) {
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    for kw in ["export default class ", "export class ", "class "] {
                        if push_if_ident(&mut out, raw, lineno, trimmed, "class", kw) {
                            matched = true;
                            break;
                        }
                    }
                }
                if !matched {
                    if let Some(caps) = js_arrow.captures(trimmed) {
                        let name = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                        if !name.is_empty() {
                            let sig: String = trimmed.chars().take(120).collect();
                            out.push(SymbolHit {
                                name,
                                kind: "fn".into(),
                                line: lineno,
                                signature: sig,
                            });
                        }
                    }
                }
                if trimmed.starts_with("export interface ") || trimmed.starts_with("interface ") {
                    let kw = if trimmed.starts_with("export interface ") {
                        "export interface "
                    } else {
                        "interface "
                    };
                    push_if_ident(&mut out, raw, lineno, trimmed, "interface", kw);
                }
            }
            "go" => {
                if let Some(rest) = trimmed.strip_prefix("func ") {
                    let name = if rest.starts_with('(') {
                        // method with receiver: func (r *Recv) Name(
                        rest.find(')').and_then(|close| {
                            let after = rest[close + 1..].trim_start();
                            after
                                .find('(')
                                .map(|open| after[..open].trim().to_string())
                        })
                    } else {
                        rest.find('(')
                            .map(|open| rest[..open].trim().to_string())
                    };
                    if let Some(name) = name {
                        if !name.is_empty() {
                            let sig: String = trimmed.chars().take(120).collect();
                            out.push(SymbolHit {
                                name,
                                kind: "fn".into(),
                                line: lineno,
                                signature: sig,
                            });
                        }
                    }
                }
                if let Some(rest) = trimmed.strip_prefix("type ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        let kind = if rest.contains("struct") {
                            "struct"
                        } else if rest.contains("interface") {
                            "interface"
                        } else {
                            "type"
                        };
                        let sig: String = trimmed.chars().take(120).collect();
                        out.push(SymbolHit {
                            name,
                            kind: kind.into(),
                            line: lineno,
                            signature: sig,
                        });
                    }
                }
            }
            "ruby" => {
                push_if_ident(&mut out, raw, lineno, trimmed, "fn", "def ");
                push_if_ident(&mut out, raw, lineno, trimmed, "class", "class ");
                push_if_ident(&mut out, raw, lineno, trimmed, "module", "module ");
            }
            "java" | "kotlin" | "csharp" => {
                for kw in ["class ", "interface ", "enum "] {
                    let spaced = [" ", kw].concat();
                    if trimmed.contains(&spaced) || trimmed.starts_with(kw) {
                        let pos = trimmed
                            .find(kw)
                            .map(|p| p + kw.len())
                            .unwrap_or_default();
                        let name: String = trimmed[pos..]
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() {
                            let sig: String = trimmed.chars().take(120).collect();
                            out.push(SymbolHit {
                                name,
                                kind: kw.trim().to_string(),
                                line: lineno,
                                signature: sig,
                            });
                        }
                        break;
                    }
                }
                if lang == "kotlin" {
                    for kw in ["fun ", "data class "] {
                        if push_if_ident(&mut out, raw, lineno, trimmed, "fn", kw) {
                            break;
                        }
                    }
                }
            }
            "c" | "cpp" | "php" => {
                for kw in ["class ", "struct ", "enum "] {
                    if push_if_ident(&mut out, raw, lineno, trimmed, kw.trim(), kw) {
                        break;
                    }
                }
                if !trimmed.starts_with("//")
                    && !trimmed.starts_with('#')
                    && !trimmed.starts_with("if")
                    && !trimmed.starts_with("for")
                    && !trimmed.starts_with("while")
                    && !trimmed.starts_with("switch")
                    && !trimmed.starts_with("return")
                {
                    if let Some(caps) = c_fn.captures(trimmed) {
                        if let Some(m) = caps.get(1) {
                            let name = m.as_str();
                            let control = ["if", "for", "while", "switch", "return", "else"];
                            if !control.contains(&name) && out.iter().all(|s| s.line != lineno) {
                                let sig: String = trimmed.chars().take(120).collect();
                                out.push(SymbolHit {
                                    name: name.to_string(),
                                    kind: "fn".into(),
                                    line: lineno,
                                    signature: sig,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Iterative directory walk skipping noise dirs and hidden entries.
/// Returns files with a recognized source extension, capped.
async fn collect_source_files(root: &Path) -> Vec<PathBuf> {
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut files: Vec<PathBuf> = Vec::new();
    let mut visited = 0usize;

    while let Some(dir) = stack.pop() {
        if files.len() >= MAX_FILES_SCANNED {
            break;
        }
        let Ok(mut rd) = tokio::fs::read_dir(&dir).await else {
            continue;
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            visited += 1;
            if visited > MAX_FILES_SCANNED * 4 {
                break;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let Ok(ft) = entry.file_type().await else {
                continue;
            };
            if ft.is_dir() {
                if !NOISE_DIRS.contains(&name.as_ref()) {
                    stack.push(path);
                }
            } else if ft.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
                if lang_of(ext).is_some() {
                    files.push(path);
                    if files.len() >= MAX_FILES_SCANNED {
                        break;
                    }
                }
            }
        }
    }
    files.sort();
    files
}

pub struct CodeSymbolsTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SymbolsParams {
    /// File or directory to scan.
    path: String,
    /// Case-insensitive substring filter on symbol names.
    #[serde(default)]
    query: Option<String>,
    /// Max symbols to return (default 200, cap 1000).
    #[serde(default = "default_symbol_limit")]
    limit: usize,
}

#[async_trait]
impl Tool for CodeSymbolsTool {
    fn name(&self) -> &str {
        "code_symbols"
    }

    fn description(&self) -> &str {
        "Find code definitions (functions, classes, structs, traits, methods) in a file or directory.

## Capability
- Parses source line-by-line for Rust, Python, JS/TS, Go, Ruby, Java/Kotlin,
  C/C++/C# and PHP
- Returns symbol name, kind, line number and signature for each hit
- Optional `query` filters symbol names case-insensitively (substring)
- Directory mode walks recursively, skipping .git / target / node_modules etc.

## When to use
- 'Where is function X defined?' — faster and more precise than grep
- Getting an overview of a module's API surface before reading it
- Use `repo_map` first to pick the right files, then `code_symbols` to drill in

## Notes
- Heuristic (no tree-sitter): symbols inside macros or heavily wrapped code
  may be missed
- Files larger than 2MB and binary-looking files are skipped"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(SymbolsParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: SymbolsParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(format!("Invalid parameters: {e}"))),
        };
        if params.path.trim().is_empty() {
            return Ok(ToolOutput::err("Parameter 'path' is required"));
        }
        let limit = params.limit.clamp(1, 1000);
        let root = crate::file::resolve_path(&ctx.working_dir, params.path.trim());
        let query = params.query.map(|q| q.to_lowercase());

        let md = match tokio::fs::metadata(&root).await {
            Ok(md) => md,
            Err(e) => {
                return Ok(ToolOutput::err(format!(
                    "Cannot access '{}': {e}",
                    root.display()
                )))
            }
        };

        let files: Vec<PathBuf> = if md.is_dir() {
            collect_source_files(&root).await
        } else {
            vec![root.clone()]
        };

        let mut total: Vec<(PathBuf, SymbolHit)> = Vec::new();
        let mut scanned = 0usize;
        for f in &files {
            if total.len() >= limit {
                break;
            }
            let Ok(fmd) = tokio::fs::metadata(f).await else {
                continue;
            };
            if fmd.len() > MAX_FILE_BYTES {
                continue;
            }
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or_default();
            let Some(lang) = lang_of(ext) else { continue };
            let Ok(content) = tokio::fs::read_to_string(f).await else {
                continue;
            };
            scanned += 1;
            for sym in extract_symbols(&content, lang) {
                if let Some(q) = &query {
                    if !sym.name.to_lowercase().contains(q) {
                        continue;
                    }
                }
                total.push((f.clone(), sym));
                if total.len() >= limit {
                    break;
                }
            }
        }

        let hits: Vec<serde_json::Value> = total
            .iter()
            .map(|(f, s)| {
                json!({
                    "file": f.strip_prefix(&root).map(|p| p.display().to_string())
                        .unwrap_or_else(|_| f.display().to_string()),
                    "name": s.name,
                    "kind": s.kind,
                    "line": s.line,
                    "signature": s.signature,
                })
            })
            .collect();

        Ok(ToolOutput::ok(
            serde_json::to_string_pretty(&json!({
                "path": root.display().to_string(),
                "files_scanned": scanned,
                "symbols_found": hits.len(),
                "symbols": hits,
            }))
            .unwrap_or_default(),
        ))
    }
}

pub struct RepoMapTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct MapParams {
    /// Directory to map (default: current working directory).
    #[serde(default)]
    path: Option<String>,
    /// Max files in the map (default 300, cap 2000).
    #[serde(default = "default_map_files")]
    max_files: usize,
    /// Top symbols shown per file (default 3, cap 10).
    #[serde(default = "default_symbols_per_file")]
    symbols_per_file: usize,
}

#[async_trait]
impl Tool for RepoMapTool {
    fn name(&self) -> &str {
        "repo_map"
    }

    fn description(&self) -> &str {
        "Build a compact map of a codebase: files by language plus their top symbols.

## Capability
- Walks the directory tree (skipping .git, target, node_modules, dist, ...)
- Groups files by language with counts
- For each file shows its top symbols (names + kinds) so you can see the
  API surface without reading the code
- Deterministic output, cheap enough to call at the start of any task on an
  unfamiliar repository

## When to use
- First touch with an unknown repo: understand its layout in one call
- Choosing which files to open/read next
- Pair with `code_symbols` (drill into one file) and `grep` (find usages)

## Notes
- Heuristic symbol extraction (same engine as code_symbols)
- Files larger than 2MB are counted but not parsed"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(MapParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: MapParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(format!("Invalid parameters: {e}"))),
        };
        let root = match params.path.as_deref() {
            Some(p) if !p.trim().is_empty() => {
                crate::file::resolve_path(&ctx.working_dir, p.trim())
            }
            _ => ctx.working_dir.clone(),
        };
        let max_files = params.max_files.clamp(1, 2000);
        let per_file = params.symbols_per_file.clamp(0, 10);

        if !tokio::fs::try_exists(&root).await.unwrap_or(false) {
            return Ok(ToolOutput::err(format!(
                "Path does not exist: {}",
                root.display()
            )));
        }

        let files = collect_source_files(&root).await;
        let mut by_lang: std::collections::BTreeMap<String, usize> = Default::default();

        // Pre-filter to source files and their languages so we read only what
        // we need, then scan them in parallel (file I/O bound) keeping order.
        let mut targets: Vec<(PathBuf, &'static str)> = Vec::new();
        for f in files.iter().take(max_files) {
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or_default();
            if let Some(lang) = lang_of(ext) {
                *by_lang.entry(lang.to_string()).or_insert(0) += 1;
                targets.push((f.clone(), lang));
            }
        }

        let entries: Vec<serde_json::Value> = if per_file == 0 {
            // Fast path: only a file inventory, no per-file content reads.
            targets
                .iter()
                .map(|(f, lang)| {
                    let rel = f
                        .strip_prefix(&root)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| f.display().to_string());
                    json!({ "file": rel, "lang": lang, "top_symbols": Vec::<serde_json::Value>::new() })
                })
                .collect()
        } else {
            let tasks: Vec<_> = targets
                .iter()
                .map(|(f, lang)| {
                    let root = root.clone();
                    let f = f.clone();
                    let lang: &'static str = lang;
                    tokio::spawn(async move {
                        // Skip oversized files (mirrors the previous contract).
                        let too_big = tokio::fs::metadata(&f)
                            .await
                            .map(|md| md.len() > MAX_FILE_BYTES)
                            .unwrap_or(false);
                        let top: Vec<serde_json::Value> = if too_big {
                            Vec::new()
                        } else {
                            match tokio::fs::read_to_string(&f).await {
                                Ok(content) => extract_symbols(&content, lang)
                                    .into_iter()
                                    .take(per_file)
                                    .map(|sym| json!({"name": sym.name, "kind": sym.kind}))
                                    .collect(),
                                Err(_) => Vec::new(),
                            }
                        };
                        let rel = f
                            .strip_prefix(&root)
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|_| f.display().to_string());
                        (rel, top)
                    })
                })
                .collect();

            let results = futures::future::join_all(tasks).await;
            results
                .into_iter()
                .enumerate()
                .filter_map(|(i, res)| {
                    let lang = targets[i].1;
                    let (rel, top) = res.ok()?;
                    Some(json!({ "file": rel, "lang": lang, "top_symbols": top }))
                })
                .collect()
        };

        Ok(ToolOutput::ok(
            serde_json::to_string_pretty(&json!({
                "path": root.display().to_string(),
                "files_mapped": entries.len(),
                "files_total": files.len(),
                "by_language": by_lang,
                "entries": entries,
            }))
            .unwrap_or_default(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_symbols_extracted() {
        let src = r#"
pub struct Config {
    name: String,
}

impl Config {
    pub fn load(path: &str) -> Self { todo!() }
    fn private_helper(&self) {}
}

pub trait Store: Send + Sync {}

enum Kind { A, B }

async fn fetch_all() {}
"#;
        let syms = extract_symbols(src, "rust");
        let names: Vec<_> = syms.iter().map(|s| (s.name.as_str(), s.kind.as_str())).collect();
        assert!(names.contains(&("Config", "struct")));
        assert!(names.contains(&("Config", "impl")));
        assert!(names.contains(&("load", "fn")));
        assert!(names.contains(&("private_helper", "fn")));
        assert!(names.contains(&("Store", "trait")));
        assert!(names.contains(&("Kind", "enum")));
        assert!(names.contains(&("fetch_all", "fn")));
    }

    #[test]
    fn python_symbols_extracted() {
        let src = "class Foo:\n    def bar(self):\n        pass\n\nasync def baz():\n    pass\n";
        let syms = extract_symbols(src, "python");
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str().to_string()).collect();
        assert!(names.contains(&"Foo".to_string()));
        assert!(names.contains(&"bar".to_string()));
        assert!(names.contains(&"baz".to_string()));
    }

    #[test]
    fn js_symbols_extracted() {
        let src = "function alpha() {}\nexport const beta = async (x) => x;\nclass Gamma {}\n";
        let syms = extract_symbols(src, "javascript");
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str().to_string()).collect();
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
        assert!(names.contains(&"Gamma".to_string()));
    }

    #[test]
    fn go_symbols_extracted() {
        let src = "func main() {}\n\nfunc (s *Server) Start(port int) error { return nil }\n\ntype Server struct{}\n";
        let syms = extract_symbols(src, "go");
        let names: Vec<_> = syms.iter().map(|s| (s.name.as_str().to_string(), s.kind.clone())).collect();
        assert!(names.contains(&("main".to_string(), "fn".to_string())));
        assert!(names.contains(&("Start".to_string(), "fn".to_string())));
        assert!(names.contains(&("Server".to_string(), "struct".to_string())));
    }

    #[test]
    fn lang_detection() {
        assert_eq!(lang_of("rs"), Some("rust"));
        assert_eq!(lang_of("py"), Some("python"));
        assert_eq!(lang_of("tsx"), Some("typescript"));
        assert_eq!(lang_of("md"), None);
    }

    #[test]
    fn line_numbers_are_one_based() {
        let src = "fn first() {}\nfn second() {}\n";
        let syms = extract_symbols(src, "rust");
        assert_eq!(syms[0].line, 1);
        assert_eq!(syms[1].line, 2);
    }
}
