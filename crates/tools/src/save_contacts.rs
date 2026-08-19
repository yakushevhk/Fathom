//! `save_contacts` tool: persists harvested contacts into the contact
//! database (SQLite or PostgreSQL) and optionally pushes them to the
//! configured CRM. This is the missing tail of the OSINT pipeline:
//! extract_contacts / find_leads → **save_contacts** → CRM / export.

use async_trait::async_trait;
use pr_core::{normalize_phone, Contact, CrmSync, SocialProfile, ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SocialInput {
    /// Platform name: linkedin, twitter/x, instagram, telegram, facebook, github...
    platform: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    username: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ContactInput {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    name: Option<String>,
    /// Job title / position.
    #[serde(default)]
    title: Option<String>,
    /// Company name.
    #[serde(default)]
    company: Option<String>,
    #[serde(default)]
    socials: Vec<SocialInput>,
    /// Tags: accepts a JSON array of strings or a single comma-separated
    /// string (LLMs emit both shapes in the wild).
    #[serde(default, deserialize_with = "one_or_many_strings")]
    #[schemars(with = "Vec<String>")]
    tags: Vec<String>,
    /// Notes: accepts a JSON array of strings or a single string.
    #[serde(default, deserialize_with = "one_or_many_strings")]
    #[schemars(with = "Vec<String>")]
    notes: Vec<String>,
    /// Provenance: URL or tool the contact came from.
    #[serde(default)]
    source: Option<String>,
}

/// Accepts `["a", "b"]`, `"a single note"` (or a comma-separated string for
/// tags) and normalizes into a Vec. Models frequently pass a plain string
/// where the schema declares an array; rejecting those loses real data.
fn one_or_many_strings<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    match Option::<OneOrMany>::deserialize(deserializer)? {
        None => Ok(Vec::new()),
        Some(OneOrMany::One(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Ok(Vec::new());
            }
            // A single string is split on "; " / newlines first, then on
            // commas — covers "note one. note two." style blobs without
            // mangling normal prose.
            if trimmed.contains("; ") || trimmed.contains('\n') {
                Ok(trimmed
                    .split([';', '\n'])
                    .map(|p| p.trim())
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect())
            } else {
                Ok(vec![trimmed.to_string()])
            }
        }
        Some(OneOrMany::Many(items)) => Ok(items
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()),
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SaveContactsParams {
    /// Contacts to persist. Each needs at least one of email/phone/name.
    contacts: Vec<ContactInput>,
    /// Also push saved contacts to the configured CRM (default: true).
    #[serde(default = "default_true")]
    push_crm: bool,
}

fn default_true() -> bool {
    true
}

pub struct SaveContactsTool;

#[async_trait]
impl Tool for SaveContactsTool {
    fn name(&self) -> &str {
        "save_contacts"
    }

    fn description(&self) -> &str {
        "Save harvested contacts (emails, phones, persons) into the persistent contact database, deduplicating against existing records, and optionally push them to the configured CRM (amoCRM/Bitrix24/HubSpot).

## When to Use

- ALWAYS call this after `extract_contacts` or `find_leads` produced contacts that should be kept — otherwise they are lost when the session ends.
- Batch contacts: pass all contacts from one source in a single call.

## Parameters

- `contacts` (required): array of objects with optional fields `email`, `phone`, `name`, `title`, `company`, `socials` (`{platform, url?, username?}`), `tags`, `notes`, `source` (URL or tool the contact came from). Each contact needs at least one of email/phone/name.
- `push_crm` (optional, default true): also create the contact in the configured CRM. Set false to only store locally.

## Behavior

- Deduplication: if a contact with the same normalized email (or phone) exists, the new data is MERGED into the existing record (blank fields filled, socials/tags/notes appended) instead of creating a duplicate.
- Returns a JSON summary: how many contacts were added vs merged, and CRM push results."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(SaveContactsParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: SaveContactsParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(format!("Invalid arguments: {e}"))),
        };

        let db = match &ctx.contact_db {
            Some(db) => db.clone(),
            None => {
                return Ok(ToolOutput::err(
                    "Contact database is not available in this run (check [contacts] config)",
                ))
            }
        };

        if params.contacts.is_empty() {
            return Ok(ToolOutput::err("No contacts provided"));
        }

        let mut added = 0usize;
        let mut merged = 0usize;
        let mut failed = 0usize;
        let mut saved: Vec<(i64, Contact)> = Vec::new();

        for input in &params.contacts {
            let contact = to_contact(input);
            if contact.email.is_none() && contact.phone.is_none() && contact.name.is_none() {
                failed += 1;
                continue;
            }

            match save_with_dedup(&db, &contact).await {
                Ok((id, was_merged)) => {
                    if was_merged {
                        merged += 1;
                    } else {
                        added += 1;
                    }
                    // Re-fetch to get the merged record (with id) for CRM push.
                    let full = db.get_contact(id).await.ok().flatten().unwrap_or_else(|| {
                        let mut c = contact.clone();
                        c.id = Some(id);
                        c
                    });
                    saved.push((id, full));
                }
                Err(e) => {
                    tracing::warn!("save_contacts: failed to save contact: {e}");
                    failed += 1;
                }
            }
        }

        // CRM push (best effort; failures are reported but not fatal).
        let mut crm_pushed = 0usize;
        let mut crm_errors: Vec<String> = Vec::new();
        if params.push_crm {
            if let Some(crm) = &ctx.crm {
                use futures::stream::StreamExt;
                // Contacts pushed earlier already carry a crm_id — skip them.
                let to_push: Vec<(i64, pr_core::Contact)> = saved
                    .iter()
                    .filter(|(_, c)| c.crm_id.is_none())
                    .map(|(id, c)| (*id, c.clone()))
                    .collect();

                // Push concurrently (bounded): a 20-contact batch no longer
                // serializes 20 sequential CRM round-trips.
                let results: Vec<(i64, Result<String, String>)> =
                    futures::stream::iter(to_push)
                        .map(|(id, contact)| {
                            let crm = crm.clone();
                            async move {
                                let label = contact.display_label();
                                let res = push_with_retry(&crm, &contact)
                                    .await
                                    .map_err(|e| format!("{label}: {e}"));
                                (id, res)
                            }
                        })
                        .buffer_unordered(4)
                        .collect()
                        .await;

                let mut crm_pushed_inner = 0usize;
                for (id, res) in results {
                    match res {
                        Ok(remote_id) => {
                            crm_pushed_inner += 1;
                            // Remember the remote id so re-syncs don't duplicate.
                            let _ = db.set_crm_id(id, &remote_id).await;
                        }
                        Err(e) => crm_errors.push(e),
                    }
                }
                crm_pushed = crm_pushed_inner;
            }
        }
        let crm_skipped = if params.push_crm && ctx.crm.is_some() {
            saved.iter().filter(|(_, c)| c.crm_id.is_some()).count()
        } else {
            0
        };

        let summary = serde_json::json!({
            "added": added,
            "merged_with_existing": merged,
            "failed": failed,
            "contact_ids": saved.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            "crm_pushed": crm_pushed,
            "crm_skipped_already_synced": crm_skipped,
            "crm_errors": crm_errors,
        });

        let text = format!(
            "Saved {} contact(s): {} new, {} merged into existing, {} failed.{}",
            added + merged,
            added,
            merged,
            failed,
            if params.push_crm && ctx.crm.is_some() {
                format!(" CRM: {crm_pushed} pushed, {} errors.", crm_errors.len())
            } else {
                String::new()
            }
        );

        Ok(ToolOutput::ok_with_meta(text, summary))
    }
}

/// Convert tool input into a domain [`Contact`], normalizing identifiers.
fn to_contact(input: &ContactInput) -> Contact {
    let email = input
        .email
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| pr_core::normalize_email(e));
    let phone = input
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| normalize_phone(p))
        .filter(|p| !p.is_empty());

    Contact {
        email,
        phone,
        name: input.name.clone().filter(|s| !s.trim().is_empty()),
        title: input.title.clone().filter(|s| !s.trim().is_empty()),
        company: input.company.clone().filter(|s| !s.trim().is_empty()),
        social_profiles: input
            .socials
            .iter()
            .map(|s| {
                SocialProfile::new(
                    s.platform.clone(),
                    s.url.clone().unwrap_or_default(),
                    s.username.clone().unwrap_or_default(),
                )
            })
            .collect(),
        tags: input
            .tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        notes: input
            .notes
            .iter()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect(),
        source: input
            .source
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "agent".to_string()),
        ..Contact::new()
    }
}

/// Insert the contact, merging into an existing record with the same
/// normalized email or phone instead of creating a duplicate. The check and
/// insert are atomic inside the store (fleet round 2 TOCTOU fix).
/// Returns `(contact_id, was_merged)`.
pub(crate) async fn save_with_dedup(
    db: &Arc<dyn pr_persistence::ContactStore>,
    contact: &Contact,
) -> anyhow::Result<(i64, bool)> {
    db.save_deduped(contact).await
}

/// Push a contact to the CRM with one retry (transient network errors).
async fn push_with_retry(crm: &CrmSync, contact: &Contact) -> anyhow::Result<String> {
    match crm.push_contact(contact).await {
        Ok(id) => Ok(id),
        Err(e) => {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            crm.push_contact(contact).await.map_err(|e2| {
                anyhow::anyhow!("{e} (retry: {e2})")
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolContext;
    use pr_core::SearchConfig;
    use std::path::PathBuf;

    fn ctx_with_db(db: Arc<dyn pr_persistence::ContactStore>) -> ToolContext {
        let mut ctx = ToolContext::new(PathBuf::from("/tmp"), SearchConfig::default());
        ctx.contact_db = Some(db);
        ctx
    }

    fn args(contacts_json: &str) -> serde_json::Value {
        serde_json::json!({ "contacts": serde_json::from_str::<serde_json::Value>(contacts_json).unwrap(), "push_crm": false })
    }

    #[tokio::test]
    async fn test_save_contacts_persists_and_reports() {
        let db: Arc<dyn pr_persistence::ContactStore> =
            Arc::new(pr_persistence::ContactDb::in_memory().unwrap());
        let ctx = ctx_with_db(db.clone());
        let tool = SaveContactsTool;

        let out = tool
            .execute(
                args(
                    r#"[{"email":"Ivan@Acme.ru","name":"Ivan","title":"CEO","source":"https://acme.ru/team"}]"#,
                ),
                &ctx,
            )
            .await
            .unwrap();

        assert!(out.success, "output: {}", out.content);
        assert_eq!(db.count().await.unwrap(), 1);
        let stored = db.find_by_email("ivan@acme.ru").await.unwrap().unwrap();
        assert_eq!(stored.name.as_deref(), Some("Ivan"));
        assert_eq!(stored.title.as_deref(), Some("CEO"));

        let meta = out.metadata.unwrap();
        assert_eq!(meta["added"], 1);
        assert_eq!(meta["merged_with_existing"], 0);
    }

    #[tokio::test]
    async fn test_save_contacts_dedup_merges_by_email() {
        let db: Arc<dyn pr_persistence::ContactStore> =
            Arc::new(pr_persistence::ContactDb::in_memory().unwrap());
        let ctx = ctx_with_db(db.clone());
        let tool = SaveContactsTool;

        let out = tool
            .execute(args(r#"[{"email":"dup@x.io","name":"Ann","tags":["lead"]}]"#), &ctx)
            .await
            .unwrap();
        assert!(out.success);

        // Same email, new phone + tag → merged, not duplicated.
        let out2 = tool
            .execute(
                args(r#"[{"email":"DUP@x.io","phone":"+7 916 000-00-00","company":"X","tags":["hot"]}]"#),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out2.success, "output: {}", out2.content);

        assert_eq!(db.count().await.unwrap(), 1, "must merge, not duplicate");
        let meta = out2.metadata.unwrap();
        assert_eq!(meta["merged_with_existing"], 1);
        assert_eq!(meta["added"], 0);

        let stored = db.find_by_email("dup@x.io").await.unwrap().unwrap();
        assert_eq!(stored.name.as_deref(), Some("Ann"), "blank-filled from old");
        assert_eq!(stored.company.as_deref(), Some("X"), "new data applied");
        assert!(stored.phone.is_some());
        assert!(stored.tags.contains(&"lead".to_string()));
        assert!(stored.tags.contains(&"hot".to_string()));
    }

    #[tokio::test]
    async fn test_save_contacts_rejects_empty_and_invalid() {
        let db: Arc<dyn pr_persistence::ContactStore> =
            Arc::new(pr_persistence::ContactDb::in_memory().unwrap());
        let ctx = ctx_with_db(db.clone());
        let tool = SaveContactsTool;

        // Contact without email/phone/name is counted as failed.
        let out = tool
            .execute(args(r#"[{"company":"OnlyCompany"}]"#), &ctx)
            .await
            .unwrap();
        assert!(out.success);
        let meta = out.metadata.unwrap();
        assert_eq!(meta["failed"], 1);
        assert_eq!(db.count().await.unwrap(), 0);

        // Empty array is an error.
        let out = tool.execute(args(r#"[]"#), &ctx).await.unwrap();
        assert!(!out.success);

        // No contact DB attached → error.
        let bare_ctx = ToolContext::new(PathBuf::from("/tmp"), SearchConfig::default());
        let out = tool
            .execute(args(r#"[{"email":"a@b.c"}]"#), &bare_ctx)
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("not available"));
    }

    #[test]
    fn test_to_contact_normalizes() {
        let input: ContactInput = serde_json::from_value(serde_json::json!({
            "email": "  Bob@Example.COM ",
            "phone": " +7 (916) 123-45-67 ",
            "name": "  ",
            "tags": [" a ", "", "b "]
        }))
        .unwrap();
        let c = to_contact(&input);
        assert_eq!(c.email.as_deref(), Some("bob@example.com"));
        assert!(c.name.is_none());
        assert_eq!(c.tags, vec!["a", "b"]);
        assert!(c.phone.is_some());
    }

    #[test]
    fn test_notes_and_tags_accept_plain_strings() {
        // LLMs frequently emit notes/tags as a single string even though
        // the schema declares arrays; both shapes must deserialize.
        let input: ContactInput = serde_json::from_value(serde_json::json!({
            "name": "Andre Zayarni",
            "email": "andre@qdrant.com",
            "notes": "SMTP accepted 2026-08-07; pattern {first}.{last} confirmed",
            "tags": "ceo, founder"
        }))
        .unwrap();
        assert_eq!(
            input.notes,
            vec!["SMTP accepted 2026-08-07", "pattern {first}.{last} confirmed"]
        );
        assert_eq!(input.tags, vec!["ceo, founder"]);

        // Arrays still work, with trimming and empty filtering.
        let input2: ContactInput = serde_json::from_value(serde_json::json!({
            "name": "Ann",
            "notes": ["  note one  ", "", "note two"],
            "tags": ["lead"]
        }))
        .unwrap();
        assert_eq!(input2.notes, vec!["note one", "note two"]);
        assert_eq!(input2.tags, vec!["lead"]);

        // Missing/null/empty all become empty vectors.
        let input3: ContactInput = serde_json::from_value(serde_json::json!({
            "name": "Bob",
            "notes": serde_json::Value::Null,
            "tags": ""
        }))
        .unwrap();
        assert!(input3.notes.is_empty());
        assert!(input3.tags.is_empty());
    }

    #[test]
    fn test_schema_lists_required_contacts() {
        let schema = SaveContactsTool.schema();
        assert_eq!(schema.name, "save_contacts");
        let params = &schema.parameters["properties"];
        assert!(params.get("contacts").is_some());
        assert!(params.get("push_crm").is_some());
    }
}
