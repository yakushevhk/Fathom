/// Status of an LSP server installation check.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    /// Server is available on PATH
    Available,
    /// Server was installed successfully
    Installed,
    /// Server is not available and could not be auto-installed
    NotAvailable(String),
}

/// Check if an LSP server command is available on PATH.
pub fn is_available(command: &str) -> bool {
    which::which(command).is_ok()
}

/// Ensure an LSP server is available. If not on PATH, attempt auto-install.
pub async fn ensure_server(command: &str) -> ServerStatus {
    if is_available(command) {
        return ServerStatus::Available;
    }

    match try_install(command).await {
        Ok(()) => {
            if is_available(command) {
                ServerStatus::Installed
            } else {
                ServerStatus::NotAvailable(format!(
                    "installed but '{}' still not found on PATH",
                    command
                ))
            }
        }
        Err(e) => ServerStatus::NotAvailable(format!(
            "could not auto-install '{}': {}",
            command, e
        )),
    }
}

/// Attempt to auto-install an LSP server using the best available package manager.
async fn try_install(command: &str) -> anyhow::Result<()> {
    match command {
        "rust-analyzer" => {
            if is_available("rustup") {
                return run_cmd("rustup", &["component", "add", "rust-analyzer"]).await;
            }
            Err(anyhow::anyhow!("rustup not found; install rust-analyzer manually"))
        }
        "pyright-langserver" => {
            if is_available("npm") {
                return run_cmd("npm", &["install", "-g", "pyright"]).await;
            }
            if is_available("pip") {
                return run_cmd("pip", &["install", "pyright"]).await;
            }
            Err(anyhow::anyhow!("npm or pip not found"))
        }
        "typescript-language-server" => {
            if is_available("npm") {
                return run_cmd("npm", &["install", "-g", "typescript-language-server", "typescript"]).await;
            }
            Err(anyhow::anyhow!("npm not found"))
        }
        "gopls" => {
            if is_available("go") {
                return run_cmd("go", &["install", "golang.org/x/tools/gopls@latest"]).await;
            }
            Err(anyhow::anyhow!("go not found"))
        }
        "clangd" => {
            if cfg!(target_os = "macos") && is_available("brew") {
                return run_cmd("brew", &["install", "llvm"]).await;
            }
            if is_available("apt-get") {
                return run_cmd("sudo", &["apt-get", "install", "-y", "clangd"]).await;
            }
            Err(anyhow::anyhow!("no suitable package manager found"))
        }
        "solargraph" => {
            if is_available("gem") {
                return run_cmd("gem", &["install", "solargraph"]).await;
            }
            Err(anyhow::anyhow!("gem not found"))
        }
        "zls" => {
            if is_available("brew") {
                return run_cmd("brew", &["install", "zls"]).await;
            }
            Err(anyhow::anyhow!("no suitable package manager found for zls"))
        }
        "lua-language-server" => {
            if is_available("brew") {
                return run_cmd("brew", &["install", "lua-language-server"]).await;
            }
            Err(anyhow::anyhow!("no suitable package manager found"))
        }
        _ => {
            Err(anyhow::anyhow!(
                "no auto-install recipe for '{}'; install it manually",
                command
            ))
        }
    }
}

async fn run_cmd(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "{} {} failed (exit {}): {}",
            program,
            args.join(" "),
            output.status.code().unwrap_or(-1),
            stderr.chars().take(200).collect::<String>()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_sh() {
        // 'sh' should always be available on unix
        assert!(is_available("sh"));
    }

    #[test]
    fn test_is_available_nonexistent() {
        assert!(!is_available("definitely_not_a_real_command_xyz123"));
    }

    #[tokio::test]
    async fn test_ensure_server_sh() {
        let status = ensure_server("sh").await;
        assert_eq!(status, ServerStatus::Available);
    }

    #[tokio::test]
    async fn test_ensure_server_nonexistent() {
        let status = ensure_server("definitely_not_a_real_command_xyz123").await;
        match status {
            ServerStatus::NotAvailable(_) => {}
            _ => panic!("expected NotAvailable"),
        }
    }
}
