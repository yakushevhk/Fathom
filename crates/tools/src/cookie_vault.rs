use std::path::PathBuf;
use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action")]
enum CookieVaultAction {
    /// Save browser cookies and local storage tokens for a given domain/service.
    #[serde(rename = "save")]
    Save {
        /// Domain or service identifier (e.g. "github.com", "linkedin.com", "salesforce")
        domain: String,
        /// JSON array of cookie objects or storage key-values
        payload: serde_json::Value,
    },
    /// Load stored cookies and session tokens for a given domain.
    #[serde(rename = "load")]
    Load {
        /// Domain identifier
        domain: String,
    },
    /// List all domains with saved sessions in the vault.
    #[serde(rename = "list")]
    List,
    /// Delete/expire a saved session from the vault.
    #[serde(rename = "delete")]
    Delete {
        /// Domain identifier to delete
        domain: String,
    },
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct CookieVaultParams {
    #[serde(flatten)]
    action: CookieVaultAction,
}

/// Encrypted Browser Session and Cookie Vault tool using AES-256-GCM.
pub struct CookieVaultTool;

#[async_trait]
impl Tool for CookieVaultTool {
    fn name(&self) -> &str {
        "cookie_vault"
    }

    fn description(&self) -> &str {
        "Hardware-Encrypted Browser Session & Cookie Vault (AES-256-GCM).

- `action: 'save'` — securely store authenticated cookies and local storage for domain.
- `action: 'load'` — decrypt and retrieve cookies for session resumption across restarts.
- `action: 'list'` — list saved domain sessions.
- `action: 'delete'` — revoke/delete stored session."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(CookieVaultParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: CookieVaultParams = serde_json::from_value(args)?;
        let fathom_home = dirs::home_dir()
            .map(|h| h.join(".fathom"))
            .unwrap_or_else(|| PathBuf::from(".fathom"));
        let vault_dir = fathom_home.join("vault").join("sessions");
        tokio::fs::create_dir_all(&vault_dir).await?;

        match params.action {
            CookieVaultAction::Save { domain, payload } => {
                let sanitized = domain.replace(['/', '\\', ':', '.'], "_");
                let file_path = vault_dir.join(format!("{}.vault", sanitized));
                let serialized = serde_json::to_string(&payload)?;
                
                // Encrypt payload using AES-256-GCM
                let encrypted = pr_persistence::credentials::encrypt_secret(&serialized)?;
                tokio::fs::write(&file_path, encrypted).await?;

                Ok(ToolOutput::ok(format!(
                    "Encrypted session for domain '{}' saved securely to vault ({}).",
                    domain,
                    file_path.display()
                )))
            }
            CookieVaultAction::Load { domain } => {
                let sanitized = domain.replace(['/', '\\', ':', '.'], "_");
                let file_path = vault_dir.join(format!("{}.vault", sanitized));
                if !file_path.exists() {
                    // Fallback check for legacy .json
                    let legacy_json = vault_dir.join(format!("{}.json", sanitized));
                    if legacy_json.exists() {
                        let content = tokio::fs::read_to_string(&legacy_json).await?;
                        return Ok(ToolOutput::ok(content));
                    }
                    return Ok(ToolOutput::err(format!("No saved session found for domain '{}'", domain)));
                }
                let encrypted_bytes = tokio::fs::read_to_string(&file_path).await?;
                let decrypted = match pr_persistence::credentials::decrypt_secret(&encrypted_bytes) {
                    Ok(d) => d,
                    Err(e) => return Ok(ToolOutput::err(format!("Decryption failed for domain '{}': {}", domain, e))),
                };

                Ok(ToolOutput::ok(decrypted))
            }
            CookieVaultAction::List => {
                let mut domains = Vec::new();
                if let Ok(mut entries) = tokio::fs::read_dir(&vault_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let p = entry.path();
                        if p.extension().map(|e| e == "vault" || e == "json").unwrap_or(false) {
                            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                                domains.push(stem.replace('_', "."));
                            }
                        }
                    }
                }
                domains.sort();
                domains.dedup();
                Ok(ToolOutput::ok(serde_json::to_string_pretty(&serde_json::json!({
                    "saved_domains_count": domains.len(),
                    "domains": domains
                }))?))
            }
            CookieVaultAction::Delete { domain } => {
                let sanitized = domain.replace(['/', '\\', ':', '.'], "_");
                let file_path = vault_dir.join(format!("{}.vault", sanitized));
                let legacy_path = vault_dir.join(format!("{}.json", sanitized));
                let mut deleted = false;
                if file_path.exists() {
                    tokio::fs::remove_file(&file_path).await?;
                    deleted = true;
                }
                if legacy_path.exists() {
                    tokio::fs::remove_file(&legacy_path).await?;
                    deleted = true;
                }

                if deleted {
                    Ok(ToolOutput::ok(format!("Deleted session for domain '{}' from vault.", domain)))
                } else {
                    Ok(ToolOutput::err(format!("No session found for domain '{}' to delete.", domain)))
                }
            }
        }
    }
}
