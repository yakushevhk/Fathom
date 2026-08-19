use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Information about a detected programming language and its LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    /// Language name (e.g. "rust", "python", "typescript")
    pub language: String,
    /// File extension that matched (e.g. "rs", "py", "ts")
    pub extension: String,
    /// LSP server command (e.g. "rust-analyzer", "pyright-langserver")
    pub lsp_command: String,
    /// Arguments to pass to the LSP server
    pub lsp_args: Vec<String>,
    /// Whether the server needs to be installed
    pub needs_install: bool,
}

/// Mapping from file extension to language/LSP server info.
fn extension_map() -> HashMap<&'static str, LanguageInfo> {
    let mut m = HashMap::new();

    // Rust
    m.insert("rs", LanguageInfo {
        language: "rust".into(),
        extension: "rs".into(),
        lsp_command: "rust-analyzer".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Python
    m.insert("py", LanguageInfo {
        language: "python".into(),
        extension: "py".into(),
        lsp_command: "pyright-langserver".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });
    m.insert("pyi", LanguageInfo {
        language: "python".into(),
        extension: "pyi".into(),
        lsp_command: "pyright-langserver".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });

    // TypeScript / JavaScript
    m.insert("ts", LanguageInfo {
        language: "typescript".into(),
        extension: "ts".into(),
        lsp_command: "typescript-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });
    m.insert("tsx", LanguageInfo {
        language: "typescript".into(),
        extension: "tsx".into(),
        lsp_command: "typescript-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });
    m.insert("js", LanguageInfo {
        language: "javascript".into(),
        extension: "js".into(),
        lsp_command: "typescript-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });
    m.insert("jsx", LanguageInfo {
        language: "javascript".into(),
        extension: "jsx".into(),
        lsp_command: "typescript-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });

    // Go
    m.insert("go", LanguageInfo {
        language: "go".into(),
        extension: "go".into(),
        lsp_command: "gopls".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // C/C++
    m.insert("c", LanguageInfo {
        language: "c".into(),
        extension: "c".into(),
        lsp_command: "clangd".into(),
        lsp_args: vec![],
        needs_install: false,
    });
    m.insert("cpp", LanguageInfo {
        language: "cpp".into(),
        extension: "cpp".into(),
        lsp_command: "clangd".into(),
        lsp_args: vec![],
        needs_install: false,
    });
    m.insert("h", LanguageInfo {
        language: "c".into(),
        extension: "h".into(),
        lsp_command: "clangd".into(),
        lsp_args: vec![],
        needs_install: false,
    });
    m.insert("hpp", LanguageInfo {
        language: "cpp".into(),
        extension: "hpp".into(),
        lsp_command: "clangd".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Java
    m.insert("java", LanguageInfo {
        language: "java".into(),
        extension: "java".into(),
        lsp_command: "jdtls".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Ruby
    m.insert("rb", LanguageInfo {
        language: "ruby".into(),
        extension: "rb".into(),
        lsp_command: "solargraph".into(),
        lsp_args: vec!["stdio".into()],
        needs_install: false,
    });

    // PHP
    m.insert("php", LanguageInfo {
        language: "php".into(),
        extension: "php".into(),
        lsp_command: "intelephense".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });

    // Lua
    m.insert("lua", LanguageInfo {
        language: "lua".into(),
        extension: "lua".into(),
        lsp_command: "lua-language-server".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Zig
    m.insert("zig", LanguageInfo {
        language: "zig".into(),
        extension: "zig".into(),
        lsp_command: "zls".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Haskell
    m.insert("hs", LanguageInfo {
        language: "haskell".into(),
        extension: "hs".into(),
        lsp_command: "haskell-language-server-wrapper".into(),
        lsp_args: vec!["--lsp".into()],
        needs_install: false,
    });

    // Elixir
    m.insert("ex", LanguageInfo {
        language: "elixir".into(),
        extension: "ex".into(),
        lsp_command: "elixir-ls".into(),
        lsp_args: vec![],
        needs_install: false,
    });
    m.insert("exs", LanguageInfo {
        language: "elixir".into(),
        extension: "exs".into(),
        lsp_command: "elixir-ls".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Kotlin
    m.insert("kt", LanguageInfo {
        language: "kotlin".into(),
        extension: "kt".into(),
        lsp_command: "kotlin-language-server".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Swift
    m.insert("swift", LanguageInfo {
        language: "swift".into(),
        extension: "swift".into(),
        lsp_command: "sourcekit-lsp".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Dart
    m.insert("dart", LanguageInfo {
        language: "dart".into(),
        extension: "dart".into(),
        lsp_command: "dart".into(),
        lsp_args: vec!["language-server".into()],
        needs_install: false,
    });

    // OCaml
    m.insert("ml", LanguageInfo {
        language: "ocaml".into(),
        extension: "ml".into(),
        lsp_command: "ocamllsp".into(),
        lsp_args: vec![],
        needs_install: false,
    });
    m.insert("mli", LanguageInfo {
        language: "ocaml".into(),
        extension: "mli".into(),
        lsp_command: "ocamllsp".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Nix
    m.insert("nix", LanguageInfo {
        language: "nix".into(),
        extension: "nix".into(),
        lsp_command: "nil".into(),
        lsp_args: vec![],
        needs_install: false,
    });

    // Terraform
    m.insert("tf", LanguageInfo {
        language: "terraform".into(),
        extension: "tf".into(),
        lsp_command: "terraform-ls".into(),
        lsp_args: vec!["serve".into()],
        needs_install: false,
    });

    // YAML
    m.insert("yaml", LanguageInfo {
        language: "yaml".into(),
        extension: "yaml".into(),
        lsp_command: "yaml-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });
    m.insert("yml", LanguageInfo {
        language: "yaml".into(),
        extension: "yml".into(),
        lsp_command: "yaml-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });

    // JSON
    m.insert("json", LanguageInfo {
        language: "json".into(),
        extension: "json".into(),
        lsp_command: "vscode-json-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });

    // CSS
    m.insert("css", LanguageInfo {
        language: "css".into(),
        extension: "css".into(),
        lsp_command: "vscode-css-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });

    // HTML
    m.insert("html", LanguageInfo {
        language: "html".into(),
        extension: "html".into(),
        lsp_command: "vscode-html-language-server".into(),
        lsp_args: vec!["--stdio".into()],
        needs_install: false,
    });

    m
}

/// Detect the language of a file by its extension.
pub fn detect_language(path: &Path) -> Option<LanguageInfo> {
    let ext = path.extension()?.to_str()?;
    extension_map().get(ext).cloned()
}

/// Detect the primary language of a project by scanning file extensions.
/// Returns the most common language found in the directory (up to `max_depth` levels).
pub fn detect_project_language(root: &Path, max_depth: usize) -> Option<LanguageInfo> {
    let map = extension_map();
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut best_info: Option<LanguageInfo> = None;

    scan_dir(root, 0, max_depth, &map, &mut counts);

    let (best_lang, _count) = counts.iter().max_by_key(|(_, c)| **c)?;
    // Find the LanguageInfo for this language
    for info in map.values() {
        if &info.language == best_lang {
            best_info = Some(info.clone());
            break;
        }
    }
    best_info
}

fn scan_dir(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    map: &HashMap<&str, LanguageInfo>,
    counts: &mut HashMap<String, usize>,
) {
    if depth > max_depth {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            // Skip common non-source directories
            if matches!(
                name.as_ref(),
                "node_modules"
                    | ".git"
                    | "target"
                    | "dist"
                    | "build"
                    | "__pycache__"
                    | ".venv"
                    | "venv"
                    | ".idea"
                    | ".vscode"
                    | "vendor"
            ) {
                continue;
            }
            scan_dir(&path, depth + 1, max_depth, map, counts);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(info) = map.get(ext) {
                *counts.entry(info.language.clone()).or_insert(0) += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_rust_file() {
        let info = detect_language(Path::new("src/main.rs")).unwrap();
        assert_eq!(info.language, "rust");
        assert_eq!(info.lsp_command, "rust-analyzer");
    }

    #[test]
    fn test_detect_python_file() {
        let info = detect_language(Path::new("app.py")).unwrap();
        assert_eq!(info.language, "python");
        assert_eq!(info.lsp_command, "pyright-langserver");
    }

    #[test]
    fn test_detect_typescript_file() {
        let info = detect_language(Path::new("index.ts")).unwrap();
        assert_eq!(info.language, "typescript");
    }

    #[test]
    fn test_detect_go_file() {
        let info = detect_language(Path::new("main.go")).unwrap();
        assert_eq!(info.language, "go");
        assert_eq!(info.lsp_command, "gopls");
    }

    #[test]
    fn test_detect_unknown_extension() {
        assert!(detect_language(Path::new("file.xyz")).is_none());
    }

    #[test]
    fn test_detect_no_extension() {
        assert!(detect_language(Path::new("Makefile")).is_none());
    }

    #[test]
    fn test_detect_project_language_rust_project() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        fs::write(root.join("Cargo.toml"), "").unwrap();

        let info = detect_project_language(root, 3).unwrap();
        assert_eq!(info.language, "rust");
    }

    #[test]
    fn test_detect_project_language_mixed_project() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        // More Python files than Rust
        fs::write(root.join("main.py"), "").unwrap();
        fs::write(root.join("utils.py"), "").unwrap();
        fs::write(root.join("app.py"), "").unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();

        let info = detect_project_language(root, 3).unwrap();
        assert_eq!(info.language, "python");
    }

    #[test]
    fn test_detect_project_language_skips_node_modules() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        // 100 files in node_modules should be skipped
        for i in 0..100 {
            fs::write(root.join(format!("node_modules/pkg/f{i}.ts")), "").unwrap();
        }
        fs::write(root.join("index.py"), "").unwrap();

        let info = detect_project_language(root, 3).unwrap();
        assert_eq!(info.language, "python");
    }
}
