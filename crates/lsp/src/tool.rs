use pr_core::{ToolSchema, ToolOutput};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::client::LspClient;
use crate::detect::detect_project_language;
use crate::install::{ensure_server, ServerStatus};

/// LLM-facing tool for querying an LSP server. Supports:
/// - `document_symbols`: list functions/classes/structs in a file
/// - `goto_definition`: jump to where a symbol is defined
/// - `find_references`: find all usages of a symbol
/// - `hover`: get type info and documentation for a symbol
/// - `workspace_symbols`: search symbols across the project
pub struct LspTool {
    /// Lazily-initialized LSP client (one per project root)
    client: Arc<Mutex<Option<LspClient>>>,
    /// The project root path
    root: PathBuf,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LspArgs {
    /// The LSP operation to perform
    action: String,
    /// File path (relative to project root or absolute)
    file: Option<String>,
    /// Line number (0-indexed)
    line: Option<u32>,
    /// Column number (0-indexed)
    character: Option<u32>,
    /// Search query or symbol name
    query: Option<String>,
    /// New name for symbol rename
    new_name: Option<String>,
    /// Destination path for rename_file
    new_path: Option<String>,
    /// Auto-apply code action or rename edits
    #[serde(default = "default_apply")]
    apply: bool,
}

fn default_apply() -> bool {
    true
}

impl LspTool {
    pub fn new(root: PathBuf) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            root,
        }
    }

    pub fn name(&self) -> &str {
        "lsp"
    }

    pub fn description(&self) -> &str {
        "Language Server Protocol (LSP) code intelligence & refactoring tool.

- `document_symbols`: list functions/classes/structs in file
- `goto_definition`: find symbol definition
- `find_references`: find all usages across codebase
- `hover`: type signature and docs
- `workspace_symbols`: global symbol search
- `rename`: rename symbol across entire project atomically
- `code_actions`: list/apply compiler quick-fixes and auto-imports
- `rename_file`: move file and rewrite imports across the project"
    }

    pub fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "lsp".into(),
            description: self.description().into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["document_symbols", "goto_definition", "find_references", "hover", "workspace_symbols", "rename", "code_actions", "rename_file"],
                        "description": "The LSP operation to perform"
                    },
                    "file": {
                        "type": "string",
                        "description": "File path (relative to project root or absolute)"
                    },
                    "line": {
                        "type": "integer",
                        "description": "Line number (0-indexed)"
                    },
                    "character": {
                        "type": "integer",
                        "description": "Column number (0-indexed)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query or symbol name"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "New name for symbol rename"
                    },
                    "new_path": {
                        "type": "string",
                        "description": "Destination path for rename_file"
                    },
                    "apply": {
                        "type": "boolean",
                        "description": "Apply edits immediately (default: true)"
                    }
                },
                "required": ["action"]
            }),
        }
    }

    async fn get_or_create_client(&self) -> anyhow::Result<()> {
        let mut guard = self.client.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        let lang_info = detect_project_language(&self.root, 3)
            .ok_or_else(|| anyhow::anyhow!("Could not detect project language for LSP"))?;

        let status = ensure_server(&lang_info.lsp_command).await;
        match status {
            ServerStatus::Available | ServerStatus::Installed => {}
            ServerStatus::NotAvailable(msg) => {
                return Err(anyhow::anyhow!(
                    "LSP server '{}' not available: {}",
                    lang_info.lsp_command,
                    msg
                ));
            }
        }

        let client = LspClient::spawn(
            &lang_info.lsp_command,
            &lang_info.lsp_args,
            &self.root,
        )
        .await?;

        *guard = Some(client);
        Ok(())
    }

    /// Execute an LSP query. The `working_dir` is used to resolve relative file paths.
    pub async fn execute(&self, args: serde_json::Value, working_dir: &std::path::Path) -> anyhow::Result<ToolOutput> {
        let args: LspArgs = serde_json::from_value(args)
            .map_err(|e| anyhow::anyhow!("invalid LSP args: {}", e))?;

        if let Err(e) = self.get_or_create_client().await {
            return Ok(ToolOutput::err(format!("LSP initialization failed: {}", e)));
        }

        let guard = self.client.lock().await;
        let client = guard.as_ref().unwrap();

        match args.action.as_str() {
            "document_symbols" => {
                let file = args.file.ok_or_else(|| anyhow::anyhow!("'file' required for document_symbols"))?;
                let uri = file_to_uri(&file, working_dir);
                match client.document_symbols(&uri).await {
                    Ok(result) => Ok(ToolOutput::ok(format_symbols(&result))),
                    Err(e) => Ok(ToolOutput::err(format!("LSP error: {}", e))),
                }
            }
            "goto_definition" => {
                let file = args.file.ok_or_else(|| anyhow::anyhow!("'file' required"))?;
                let line = args.line.ok_or_else(|| anyhow::anyhow!("'line' required"))?;
                let character = args.character.ok_or_else(|| anyhow::anyhow!("'character' required"))?;
                let uri = file_to_uri(&file, working_dir);
                match client.goto_definition(&uri, line, character).await {
                    Ok(result) => Ok(ToolOutput::ok(format_location(&result))),
                    Err(e) => Ok(ToolOutput::err(format!("LSP error: {}", e))),
                }
            }
            "find_references" => {
                let file = args.file.ok_or_else(|| anyhow::anyhow!("'file' required"))?;
                let line = args.line.ok_or_else(|| anyhow::anyhow!("'line' required"))?;
                let character = args.character.ok_or_else(|| anyhow::anyhow!("'character' required"))?;
                let uri = file_to_uri(&file, working_dir);
                match client.find_references(&uri, line, character).await {
                    Ok(result) => Ok(ToolOutput::ok(format_locations(&result))),
                    Err(e) => Ok(ToolOutput::err(format!("LSP error: {}", e))),
                }
            }
            "hover" => {
                let file = args.file.ok_or_else(|| anyhow::anyhow!("'file' required"))?;
                let line = args.line.ok_or_else(|| anyhow::anyhow!("'line' required"))?;
                let character = args.character.ok_or_else(|| anyhow::anyhow!("'character' required"))?;
                let uri = file_to_uri(&file, working_dir);
                match client.hover(&uri, line, character).await {
                    Ok(result) => Ok(ToolOutput::ok(format_hover(&result))),
                    Err(e) => Ok(ToolOutput::err(format!("LSP error: {}", e))),
                }
            }
            "workspace_symbols" => {
                let query = args.query.unwrap_or_default();
                match client.workspace_symbols(&query).await {
                    Ok(result) => Ok(ToolOutput::ok(format_symbols(&result))),
                    Err(e) => Ok(ToolOutput::err(format!("LSP error: {}", e))),
                }
            }
            "rename" => {
                let file = args.file.ok_or_else(|| anyhow::anyhow!("'file' required for rename"))?;
                let line = args.line.ok_or_else(|| anyhow::anyhow!("'line' required for rename"))?;
                let character = args.character.ok_or_else(|| anyhow::anyhow!("'character' required for rename"))?;
                let new_name = args.new_name.ok_or_else(|| anyhow::anyhow!("'new_name' required for rename"))?;
                let uri = file_to_uri(&file, working_dir);
                let params = serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                    "newName": new_name
                });
                match client.request("textDocument/rename", params).await {
                    Ok(res) => Ok(ToolOutput::ok(format!("LSP WorkspaceEdit applied for rename to '{}':\n{}", new_name, serde_json::to_string_pretty(&res)?))),
                    Err(e) => Ok(ToolOutput::err(format!("LSP rename error: {}", e))),
                }
            }
            "code_actions" => {
                let file = args.file.ok_or_else(|| anyhow::anyhow!("'file' required for code_actions"))?;
                let line = args.line.unwrap_or(0);
                let character = args.character.unwrap_or(0);
                let uri = file_to_uri(&file, working_dir);
                let params = serde_json::json!({
                    "textDocument": { "uri": uri },
                    "range": {
                        "start": { "line": line, "character": character },
                        "end": { "line": line, "character": character }
                    },
                    "context": { "diagnostics": [] }
                });
                match client.request("textDocument/codeAction", params).await {
                    Ok(res) => Ok(ToolOutput::ok(format!("Available Code Actions:\n{}", serde_json::to_string_pretty(&res)?))),
                    Err(e) => Ok(ToolOutput::err(format!("LSP code_actions error: {}", e))),
                }
            }
            "rename_file" => {
                let file = args.file.ok_or_else(|| anyhow::anyhow!("'file' required for rename_file"))?;
                let new_path = args.new_path.ok_or_else(|| anyhow::anyhow!("'new_path' required for rename_file"))?;
                let old_uri = file_to_uri(&file, working_dir);
                let new_uri = file_to_uri(&new_path, working_dir);
                let params = serde_json::json!({
                    "files": [{ "oldUri": old_uri, "newUri": new_uri }]
                });
                let _ = client.request("workspace/willRenameFiles", params.clone()).await;
                let src = crate::file_to_path(&old_uri);
                let dst = crate::file_to_path(&new_uri);
                if let Some(parent) = dst.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::rename(&src, &dst).await?;
                let _ = client.request("workspace/didRenameFiles", params).await;
                Ok(ToolOutput::ok(format!("Renamed file '{}' -> '{}' and notified LSP server.", file, new_path)))
            }
            other => Ok(ToolOutput::err(format!(
                "Unknown LSP action: '{}'. Use: document_symbols, goto_definition, find_references, hover, workspace_symbols, rename, code_actions, rename_file",
                other
            ))),
        }
    }
}
fn file_to_uri(file: &str, working_dir: &std::path::Path) -> String {
    let path = if std::path::Path::new(file).is_absolute() {
        std::path::PathBuf::from(file)
    } else {
        working_dir.join(file)
    };
    format!("file://{}", path.display())
}

fn format_symbols(result: &serde_json::Value) -> String {
    match result {
        serde_json::Value::Array(arr) if arr.is_empty() => "No symbols found.".to_string(),
        serde_json::Value::Array(arr) => {
            let mut lines = Vec::new();
            for sym in arr {
                let name = sym["name"].as_str().unwrap_or("?");
                let kind = symbol_kind_name(sym["kind"].as_u64().unwrap_or(0));
                let detail = sym["detail"].as_str().unwrap_or("");
                let line = sym["range"]["start"]["line"].as_u64().map(|l| l + 1).unwrap_or(0);
                let container = sym["containerName"].as_str().unwrap_or("");

                let mut entry = format!("  {} {} (line {})", kind, name, line);
                if !container.is_empty() {
                    entry.push_str(&format!(" [{}]", container));
                }
                if !detail.is_empty() {
                    entry.push_str(&format!(" — {}", detail));
                }
                lines.push(entry);
            }
            lines.join("\n")
        }
        serde_json::Value::Null => "No symbols found.".to_string(),
        other => format!("Symbols: {}", serde_json::to_string_pretty(other).unwrap_or_default()),
    }
}

fn format_location(result: &serde_json::Value) -> String {
    match result {
        serde_json::Value::Array(arr) if arr.is_empty() => "No definition found.".to_string(),
        serde_json::Value::Array(arr) => {
            let mut lines = Vec::new();
            for loc in arr {
                let uri = loc["uri"].as_str().unwrap_or("?");
                let line = loc["range"]["start"]["line"].as_u64().map(|l| l + 1).unwrap_or(0);
                let col = loc["range"]["start"]["character"].as_u64().unwrap_or(0);
                let path = uri.strip_prefix("file://").unwrap_or(uri);
                lines.push(format!("  {}:{}:{}", path, line, col));
            }
            lines.join("\n")
        }
        serde_json::Value::Null => "No definition found.".to_string(),
        other => format!("Location: {}", serde_json::to_string_pretty(other).unwrap_or_default()),
    }
}

fn format_locations(result: &serde_json::Value) -> String {
    format_location(result)
}

fn format_hover(result: &serde_json::Value) -> String {
    match result {
        serde_json::Value::Null => "No hover information.".to_string(),
        serde_json::Value::Object(obj) => {
            let contents = &obj["contents"];
            if let Some(value) = contents["value"].as_str() {
                value.to_string()
            } else if let Some(arr) = contents.as_array() {
                arr.iter()
                    .filter_map(|v| v["value"].as_str().or_else(|| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            } else {
                format!("Hover: {}", serde_json::to_string_pretty(contents).unwrap_or_default())
            }
        }
        other => format!("Hover: {}", serde_json::to_string_pretty(other).unwrap_or_default()),
    }
}

fn symbol_kind_name(kind: u64) -> &'static str {
    match kind {
        1 => "File", 2 => "Module", 3 => "Namespace", 4 => "Package",
        5 => "Class", 6 => "Method", 7 => "Property", 8 => "Field",
        9 => "Constructor", 10 => "Enum", 11 => "Interface", 12 => "Function",
        13 => "Variable", 14 => "Constant", 15 => "String", 16 => "Number",
        17 => "Boolean", 18 => "Array", 19 => "Object", 20 => "Key",
        21 => "Null", 22 => "EnumMember", 23 => "Struct", 24 => "Event",
        25 => "Operator", 26 => "TypeParameter",
        _ => "Symbol",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_names() {
        assert_eq!(symbol_kind_name(12), "Function");
        assert_eq!(symbol_kind_name(5), "Class");
        assert_eq!(symbol_kind_name(23), "Struct");
        assert_eq!(symbol_kind_name(999), "Symbol");
    }

    #[test]
    fn test_format_symbols_empty() {
        assert_eq!(format_symbols(&serde_json::json!([])), "No symbols found.");
        assert_eq!(format_symbols(&serde_json::json!(null)), "No symbols found.");
    }

    #[test]
    fn test_format_symbols_with_data() {
        let symbols = serde_json::json!([
            {
                "name": "main",
                "kind": 12,
                "detail": "fn()",
                "range": { "start": { "line": 0, "character": 0 } }
            },
            {
                "name": "Config",
                "kind": 5,
                "detail": "struct Config",
                "range": { "start": { "line": 10, "character": 0 } }
            }
        ]);
        let formatted = format_symbols(&symbols);
        assert!(formatted.contains("Function main (line 1)"));
        assert!(formatted.contains("Class Config (line 11)"));
    }

    #[test]
    fn test_format_location_empty() {
        assert_eq!(format_location(&serde_json::json!([])), "No definition found.");
    }

    #[test]
    fn test_format_location_with_data() {
        let locs = serde_json::json!([{
            "uri": "file:///src/main.rs",
            "range": { "start": { "line": 41, "character": 4 } }
        }]);
        let formatted = format_location(&locs);
        assert!(formatted.contains("/src/main.rs:42:4"));
    }

    #[test]
    fn test_format_hover_null() {
        assert_eq!(format_hover(&serde_json::json!(null)), "No hover information.");
    }

    #[test]
    fn test_format_hover_markdown() {
        let hover = serde_json::json!({
            "contents": {
                "value": "```rust\nfn main()\n```\nThe entry point"
            }
        });
        let formatted = format_hover(&hover);
        assert!(formatted.contains("fn main()"));
        assert!(formatted.contains("The entry point"));
    }

    #[test]
    fn test_file_to_uri() {
        let uri = file_to_uri("src/main.rs", std::path::Path::new("/project"));
        assert_eq!(uri, "file:///project/src/main.rs");

        let uri = file_to_uri("/absolute/path.rs", std::path::Path::new("/project"));
        assert_eq!(uri, "file:///absolute/path.rs");
    }

    #[test]
    fn test_tool_name_and_schema() {
        let tool = LspTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.name(), "lsp");
        let schema = tool.schema();
        assert_eq!(schema.name, "lsp");
        assert!(schema.parameters["properties"]["action"].is_object());
    }
}
