use std::path::PathBuf;
use pr_core::{PrError, PrResult};

/// Synchronizes Fathom long-term memory facts and entity graphs with a local Obsidian / Markdown vault.
pub struct VaultSyncEngine {
    pub vault_path: PathBuf,
}

impl VaultSyncEngine {
    pub fn new(vault_path: impl Into<PathBuf>) -> Self {
        Self {
            vault_path: vault_path.into(),
        }
    }

    /// Export entity graph and facts into linked Markdown files with [[Wikilinks]].
    pub async fn export_to_vault(&self, triples: &[crate::triples::RdfTriple]) -> PrResult<usize> {
        tokio::fs::create_dir_all(&self.vault_path).await.map_err(|e| PrError::Tool(e.to_string()))?;
        let mut written_files = 0;

        for triple in triples {
            let sanitized = triple.subject.replace(['/', '\\', ':', '.'], "_");
            let file_path = self.vault_path.join(format!("{}.md", sanitized));

            let entry = format!(
                "- **{}** [[{}]]\n",
                triple.predicate,
                triple.object
            );

            if file_path.exists() {
                let mut content = tokio::fs::read_to_string(&file_path).await.map_err(|e| PrError::Tool(e.to_string()))?;
                if !content.contains(&triple.object) {
                    content.push_str(&entry);
                    tokio::fs::write(&file_path, &content).await.map_err(|e| PrError::Tool(e.to_string()))?;
                }
            } else {
                let new_content = format!(
                    "---\ntitle: {}\ntags: [fathom, memory]\n---\n\n# {}\n\n## Relations\n{}",
                    triple.subject,
                    triple.subject,
                    entry
                );
                tokio::fs::write(&file_path, &new_content).await.map_err(|e| PrError::Tool(e.to_string()))?;
                written_files += 1;
            }
        }

        Ok(written_files)
    }
}
