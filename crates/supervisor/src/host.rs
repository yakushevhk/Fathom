use std::path::PathBuf;
use std::process::Stdio;
use pr_core::{PrError, PrResult};

/// Sub-millisecond host-level sandbox runner using Linux bubblewrap (`bwrap`)
/// or macOS `sandbox-exec` with strict filesystem and network isolation.
pub struct HostSandbox {
    pub workspace_root: PathBuf,
    pub allow_network: bool,
    pub read_only_root: bool,
}

impl HostSandbox {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            allow_network: false,
            read_only_root: true,
        }
    }

    pub fn with_network(mut self, allow: bool) -> Self {
        self.allow_network = allow;
        self
    }

    /// Wrap a command into a sandboxed invocation.
    pub fn wrap_command(&self, program: &str, args: &[String]) -> (String, Vec<String>) {
        #[cfg(target_os = "linux")]
        {
            let mut bwrap_args = vec![
                "--ro-bind".to_string(), "/usr".to_string(), "/usr".to_string(),
                "--ro-bind".to_string(), "/bin".to_string(), "/bin".to_string(),
                "--ro-bind".to_string(), "/lib".to_string(), "/lib".to_string(),
                "--ro-bind".to_string(), "/lib64".to_string(), "/lib64".to_string(),
                "--ro-bind".to_string(), "/etc".to_string(), "/etc".to_string(),
                "--dev".to_string(), "/dev".to_string(),
                "--proc".to_string(), "/proc".to_string(),
                "--tmpfs".to_string(), "/tmp".to_string(),
                "--bind".to_string(), self.workspace_root.display().to_string(), self.workspace_root.display().to_string(),
                "--chdir".to_string(), self.workspace_root.display().to_string(),
                "--die-with-parent".to_string(),
                "--unshare-pid".to_string(),
                "--unshare-ipc".to_string(),
                "--unshare-uts".to_string(),
            ];

            if !self.allow_network {
                bwrap_args.push("--unshare-net".to_string());
            }

            bwrap_args.push(program.to_string());
            bwrap_args.extend(args.iter().cloned());

            ("bwrap".to_string(), bwrap_args)
        }

        #[cfg(target_os = "macos")]
        {
            let profile = format!(
                "(version 1)
(deny default)
(allow process-exec (literal \"/bin/sh\") (literal \"/bin/bash\") (literal \"/usr/bin/git\") (literal \"/usr/bin/cargo\") (literal \"{}\"))
(allow file-read* (subpath \"/usr\") (subpath \"/bin\") (subpath \"/Library\") (subpath \"/System\") (subpath \"{}\"))
(allow file-write* (subpath \"/tmp\") (subpath \"{}\"))
{}
",
                program,
                self.workspace_root.display(),
                self.workspace_root.display(),
                if self.allow_network { "(allow network*)" } else { "(deny network*)" }
            );

            let mut sb_args = vec![
                "-p".to_string(),
                profile,
                program.to_string(),
            ];
            sb_args.extend(args.iter().cloned());

            ("sandbox-exec".to_string(), sb_args)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            (program.to_string(), args.to_vec())
        }
    }

    /// Execute a sandboxed command asynchronously with bounded timeout.
    pub async fn run_sandboxed(
        &self,
        program: &str,
        args: &[String],
        timeout_secs: u64,
    ) -> PrResult<(i32, String, String)> {
        let (exec_prog, exec_args) = self.wrap_command(program, args);

        let mut cmd = tokio::process::Command::new(&exec_prog);
        cmd.args(&exec_args)
            .current_dir(&self.workspace_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let future = cmd.output();
        let output = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), future)
            .await
            .map_err(|_| PrError::Tool(format!("Sandboxed process '{}' timed out after {}s", program, timeout_secs)))?
            .map_err(|e| PrError::Tool(format!("Failed to execute sandboxed command '{}': {}", exec_prog, e)))?;

        let code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok((code, stdout, stderr))
    }
}
