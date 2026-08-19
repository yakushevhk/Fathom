pub mod client;
pub mod detect;
pub mod install;
pub mod tool;

pub use client::LspClient;
pub use detect::{detect_language, detect_project_language, LanguageInfo};
pub use install::{ensure_server, ServerStatus};
pub use tool::LspTool;
