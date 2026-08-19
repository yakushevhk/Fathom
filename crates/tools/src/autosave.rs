//! Deterministic contact auto-persistence (fleet report C1).
//!
//! Prompt-only steering ("ALWAYS call save_contacts after extraction") leaks
//! yield whenever the model forgets — especially after compaction prunes the
//! tool result. The runtime therefore calls these helpers right after a
//! successful `extract_contacts` / `find_leads`, so harvested contacts reach
//! the database no matter what the model does next.

use pr_core::Contact;
use pr_core::Verification;
use serde_json::Value;
use std::sync::Arc;

use crate::receipt::{open_default_ledger, ReceiptKind, Verdict};
use crate::save_contacts::save_with_dedup;

/// Outcome of an auto-persist pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoSaveSummary {
    pub saved: usize,
    pub merged: usize,
    pub failed: usize,
}

/// Persist contacts produced by `extract_contacts` (metadata `contacts`).
///
/// Each extracted email/phone/person becomes its own contact row, deduplicated
/// against the existing database via the same merge rules as `save_contacts`.
pub async fn autosave_extracted(
    db: &Arc<dyn pr_persistence::ContactStore>,
    contacts_meta: &Value,
    origin: &str,
) -> AutoSaveSummary {
    let mut contacts: Vec<Contact> = Vec::new();

    if let Some(emails) = contacts_meta.get("emails").and_then(|v| v.as_array()) {
        for e in emails {
            let Some(email) = e.get("email").and_then(|v| v.as_str()) else {
                continue;
            };
            let source = e
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("extract_contacts");
            contacts.push(
                Contact {
                    email: Some(email.to_string()),
                    notes: vec![format!("context: {}", context_snippet(e))],
                    ..Contact::new()
                }
                .with_source(format!("extract_contacts:{source}:{origin}")),
            );
        }
    }

    if let Some(phones) = contacts_meta.get("phones").and_then(|v| v.as_array()) {
        for p in phones {
            // Prefer the normalized E.164 form when present.
            let phone = p
                .get("normalized")
                .and_then(|v| v.as_str())
                .or_else(|| p.get("phone").and_then(|v| v.as_str()));
            let Some(phone) = phone else { continue };
            contacts.push(
                Contact {
                    phone: Some(phone.to_string()),
                    ..Contact::new()
                }
                .with_source(format!("extract_contacts:phone:{origin}")),
            );
        }
    }

    if let Some(persons) = contacts_meta.get("persons").and_then(|v| v.as_array()) {
        for p in persons {
            let name = p
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if name.trim().is_empty() {
                continue;
            }
            let socials = p
                .get("social")
                .and_then(|v| serde_json::from_value::<Vec<pr_core::SocialProfile>>(v.clone()).ok())
                .unwrap_or_default();
            contacts.push(
                Contact {
                    name: Some(name),
                    title: p.get("title").and_then(|v| v.as_str()).map(String::from),
                    company: p.get("company").and_then(|v| v.as_str()).map(String::from),
                    email: p.get("email").and_then(|v| v.as_str()).map(String::from),
                    phone: p.get("phone").and_then(|v| v.as_str()).map(String::from),
                    social_profiles: socials,
                    ..Contact::new()
                }
                .with_source(format!("extract_contacts:person:{origin}")),
            );
        }
    }

    persist_all(db, contacts).await
}

/// Persist leads produced by `find_leads` (metadata `leads`).
pub async fn autosave_leads(
    db: &Arc<dyn pr_persistence::ContactStore>,
    leads_meta: &Value,
) -> AutoSaveSummary {
    let mut contacts: Vec<Contact> = Vec::new();

    if let Some(leads) = leads_meta.as_array() {
        for lead in leads {
            let person = lead.get("person");
            let company = lead.get("company");
            let name = person
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let company_name = company
                .and_then(|c| c.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if name.trim().is_empty() && company_name.trim().is_empty() {
                continue;
            }

            let mut contact = Contact {
                name: if name.trim().is_empty() { None } else { Some(name) },
                title: person
                    .and_then(|p| p.get("role"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                email: person
                    .and_then(|p| p.get("email"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                phone: person
                    .and_then(|p| p.get("phone"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                company: if company_name.trim().is_empty() {
                    None
                } else {
                    Some(company_name)
                },
                ..Contact::new()
            };
            if let Some(url) = person
                .and_then(|p| p.get("profile_url"))
                .and_then(|v| v.as_str())
            {
                contact.social_profiles.push(pr_core::SocialProfile::new(
                    "linkedin",
                    url,
                    "",
                ));
            }
            if let Some(conf) = lead.get("confidence").and_then(|v| v.as_f64()) {
                contact
                    .notes
                    .push(format!("lead confidence: {conf:.2}"));
            }
            let source = lead
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("find_leads");
            contacts.push(contact.with_source(format!("find_leads:{source}")));
        }
    }

    persist_all(db, contacts).await
}

async fn persist_all(
    db: &Arc<dyn pr_persistence::ContactStore>,
    contacts: Vec<Contact>,
) -> AutoSaveSummary {
    let mut summary = AutoSaveSummary::default();
    // Load the durable verification ledger once so auto-persisted contacts get
    // an honest verification tag (from receipts recorded by verify_* tools),
    // not "unverified" just because this path didn't run a check itself.
    let ledger = open_default_ledger().await.ok();
    for mut contact in contacts {
        if contact.email.is_none() && contact.phone.is_none() && contact.name.is_none() {
            continue;
        }
        if let (Some(ledger), Some(email)) = (&ledger, &contact.email) {
            // Delivering-check first: an accepted mailbox is the strongest
            // signal. Otherwise a syntax+domain green earns at least Partial.
            let smtp = ledger.verdict(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), email).await;
            if smtp == Some(Verdict::Pass) {
                contact.verification = Verification::Verified;
            } else {
                let domain_ok = ledger.is_passing(ReceiptKind::of(ReceiptKind::EMAIL_DOMAIN_MX), email).await
                    || ledger.is_passing(ReceiptKind::of(ReceiptKind::EMAIL_SYNTAX), email).await;
                if domain_ok {
                    contact.verification = Verification::Partial;
                }
            }
        }
        match save_with_dedup(db, &contact).await {
            Ok((_id, merged)) => {
                if merged {
                    summary.merged += 1;
                } else {
                    summary.saved += 1;
                }
            }
            Err(e) => {
                tracing::warn!("autosave: failed to persist contact: {e}");
                summary.failed += 1;
            }
        }
    }
    summary
}

fn context_snippet(value: &Value) -> String {
    value
        .get("context")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Arc<dyn pr_persistence::ContactStore> {
        Arc::new(pr_persistence::ContactDb::in_memory().unwrap())
    }

    #[tokio::test]
    async fn autosave_extracted_persists_emails_phones_persons() {
        let db = test_db();
        let meta = serde_json::json!({
            "emails": [{"email": "ceo@acme.ru", "source": "mailto link", "context": "Contact us: ceo@acme.ru"}],
            "phones": [{"phone": "+7 (495) 000-00-00", "normalized": "+74950000000", "source": "text"}],
            "persons": [{"name": "Ivan Petrov", "title": "CEO", "company": "Acme", "email": null, "social": []}]
        });

        let summary = autosave_extracted(&db, &meta, "https://acme.ru").await;
        assert_eq!(summary.saved, 3);
        assert_eq!(summary.failed, 0);
        assert_eq!(db.count().await.unwrap(), 3);

        assert!(db.find_by_email("ceo@acme.ru").await.unwrap().is_some());
        assert!(db.find_by_phone("+74950000000").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn autosave_extracted_dedups_second_pass() {
        let db = test_db();
        let meta = serde_json::json!({
            "emails": [{"email": "dup@x.io", "source": "text", "context": ""}],
            "phones": [],
            "persons": []
        });

        let first = autosave_extracted(&db, &meta, "a").await;
        assert_eq!(first.saved, 1);

        let second = autosave_extracted(&db, &meta, "b").await;
        assert_eq!(second.merged, 1);
        assert_eq!(second.saved, 0);
        assert_eq!(db.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn autosave_leads_persists_with_company_and_social() {
        let db = test_db();
        let meta = serde_json::json!([{
            "person": {"name": "Ann", "role": "CTO", "email": "ann@corp.io", "phone": null,
                        "profile_url": "https://linkedin.com/in/ann"},
            "company": {"name": "Corp", "website": "https://corp.io"},
            "source": "2gis",
            "confidence": 0.83
        }]);

        let summary = autosave_leads(&db, &meta).await;
        assert_eq!(summary.saved, 1);

        let stored = db.find_by_email("ann@corp.io").await.unwrap().unwrap();
        assert_eq!(stored.name.as_deref(), Some("Ann"));
        assert_eq!(stored.title.as_deref(), Some("CTO"));
        assert_eq!(stored.company.as_deref(), Some("Corp"));
        assert_eq!(stored.social_profiles.len(), 1);
        assert_eq!(stored.social_profiles[0].platform, "linkedin");
    }

    #[tokio::test]
    async fn autosave_leads_skips_empty_entries() {
        let db = test_db();
        let meta = serde_json::json!([
            {"person": {"name": ""}, "company": {"name": ""}, "source": "x", "confidence": 0.1}
        ]);
        let summary = autosave_leads(&db, &meta).await;
        assert_eq!(summary.saved + summary.merged, 0);
        assert_eq!(db.count().await.unwrap(), 0);
    }
}
