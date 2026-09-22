use pr_core::{PrError, PrResult};
use std::path::PathBuf;

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
        tokio::fs::create_dir_all(&self.vault_path)
            .await
            .map_err(|e| PrError::Tool(e.to_string()))?;
        let mut written_files = 0;

        for triple in triples {
            let sanitized = triple.subject.replace(['/', '\\', ':', '.'], "_");
            let file_path = self.vault_path.join(format!("{}.md", sanitized));

            let entry = format!("- **{}** [[{}]]\n", triple.predicate, triple.object);

            if file_path.exists() {
                let mut content = tokio::fs::read_to_string(&file_path)
                    .await
                    .map_err(|e| PrError::Tool(e.to_string()))?;
                if !content.contains(&triple.object) {
                    content.push_str(&entry);
                    tokio::fs::write(&file_path, &content)
                        .await
                        .map_err(|e| PrError::Tool(e.to_string()))?;
                }
            } else {
                let new_content = format!(
                    "---\ntitle: {}\ntags: [fathom, memory]\n---\n\n# {}\n\n## Relations\n{}",
                    triple.subject, triple.subject, entry
                );
                tokio::fs::write(&file_path, &new_content)
                    .await
                    .map_err(|e| PrError::Tool(e.to_string()))?;
                written_files += 1;
            }
        }

        Ok(written_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triples::RdfTriple;

    fn t(s: &str, p: &str, o: &str) -> RdfTriple {
        RdfTriple {
            subject: s.into(),
            predicate: p.into(),
            object: o.into(),
            confidence: 100,
        }
    }

    #[tokio::test]
    async fn export_creates_markdown_files() {
        let dir = tempfile::tempdir().unwrap();
        let eng = VaultSyncEngine::new(dir.path());
        let n = eng
            .export_to_vault(&[t("alice", "knows", "bob")])
            .await
            .unwrap();
        assert_eq!(n, 1);
        let content = std::fs::read_to_string(dir.path().join("alice.md")).unwrap();
        assert!(content.contains("# alice"));
        assert!(content.contains("**knows** [[bob]]"));
    }

    #[tokio::test]
    async fn export_appends_relations_to_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let eng = VaultSyncEngine::new(dir.path());
        eng.export_to_vault(&[t("alice", "knows", "bob")])
            .await
            .unwrap();
        eng.export_to_vault(&[t("alice", "works_at", "acme")])
            .await
            .unwrap();
        let content = std::fs::read_to_string(dir.path().join("alice.md")).unwrap();
        assert!(content.contains("[[bob]]"));
        assert!(content.contains("[[acme]]"));
    }

    #[tokio::test]
    async fn export_dedupes_same_object() {
        let dir = tempfile::tempdir().unwrap();
        let eng = VaultSyncEngine::new(dir.path());
        eng.export_to_vault(&[t("alice", "knows", "bob")])
            .await
            .unwrap();
        eng.export_to_vault(&[t("alice", "knows", "bob")])
            .await
            .unwrap();
        let content = std::fs::read_to_string(dir.path().join("alice.md")).unwrap();
        assert_eq!(content.matches("[[bob]]").count(), 1);
    }

    #[tokio::test]
    async fn export_sanitizes_unsafe_filename_chars() {
        let dir = tempfile::tempdir().unwrap();
        let eng = VaultSyncEngine::new(dir.path());
        eng.export_to_vault(&[t("a/b\\c:d.e", "p", "o")])
            .await
            .unwrap();
        assert!(dir.path().join("a_b_c_d_e.md").exists());
    }

    #[tokio::test]
    async fn export_empty_list_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let eng = VaultSyncEngine::new(dir.path());
        assert_eq!(eng.export_to_vault(&[]).await.unwrap(), 0);
    }
}
