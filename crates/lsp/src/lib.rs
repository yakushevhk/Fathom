pub mod client;
pub mod detect;
pub mod install;
pub mod tool;

pub use client::LspClient;
pub use detect::{detect_language, detect_project_language, LanguageInfo};
pub use install::{ensure_server, ServerStatus};
pub use tool::LspTool;

pub fn file_to_path(uri: &str) -> std::path::PathBuf {
    if let Some(stripped) = uri.strip_prefix("file://") {
        std::path::PathBuf::from(stripped)
    } else {
        std::path::PathBuf::from(uri)
    }
}
