//! Backend-agnostic contact storage.
//!
//! [`ContactStore`] unifies the SQLite ([`crate::ContactDb`]) and optional
//! PostgreSQL ([`crate::PgContactDb`]) contact databases behind one async
//! interface, so tools (`save_contacts`), the CLI (`contacts` subcommand)
//! and CRM sync do not care which backend is configured.

use async_trait::async_trait;
use pr_core::Contact;

/// Unified contact storage backend (SQLite or PostgreSQL).
#[async_trait]
pub trait ContactStore: Send + Sync {
    /// Insert a contact with its social profiles, tags and notes.
    /// Returns the contact id.
    async fn add_contact(&self, contact: &Contact) -> anyhow::Result<i64>;

    /// Fetch a single contact by id.
    async fn get_contact(&self, id: i64) -> anyhow::Result<Option<Contact>>;

    /// Find a contact by normalized email address.
    async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<Contact>>;

    /// Find a contact by normalized phone number.
    async fn find_by_phone(&self, phone: &str) -> anyhow::Result<Option<Contact>>;

    /// Free-text search across emails, phones, names, companies and tags.
    async fn search(&self, query: &str) -> anyhow::Result<Vec<Contact>>;

    /// List contacts, most recently added first.
    async fn list_all(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<Contact>>;

    /// Total number of stored contacts.
    async fn count(&self) -> anyhow::Result<i64>;

    /// Merge `duplicate_id` into `primary_id` (fills blanks, moves extras,
    /// deletes the duplicate).
    async fn merge_contacts(&self, primary_id: i64, duplicate_id: i64) -> anyhow::Result<()>;

    /// Record the remote CRM id after a successful push (dedup on re-sync).
    async fn set_crm_id(&self, contact_id: i64, crm_id: &str) -> anyhow::Result<()>;

    /// Delete a contact (used to roll back temp rows on merge failure).
    async fn delete_contact(&self, contact_id: i64) -> anyhow::Result<()>;

    /// Atomically find-or-insert a contact (dedup by normalized email or
    /// phone). Concurrent callers cannot both insert the same contact:
    /// the whole check+insert runs inside one locked transaction.
    /// Returns `(contact_id, was_merged)`.
    async fn save_deduped(&self, contact: &Contact) -> anyhow::Result<(i64, bool)>;

    /// Human-readable backend name (`"sqlite"` or `"postgres"`).
    fn backend(&self) -> &'static str;
}

/// Open the contact store selected by `[contacts]` config: PostgreSQL when
/// `pg_url` is set (requires the `postgres` feature), otherwise SQLite at
/// `db_path`.
pub async fn open_contact_store(
    cfg: &pr_core::ContactsConfig,
) -> anyhow::Result<std::sync::Arc<dyn ContactStore>> {
    #[cfg(feature = "postgres")]
    if !cfg.pg_url.trim().is_empty() {
        let pg = crate::PgContactDb::connect(&cfg.pg_url).await?;
        return Ok(std::sync::Arc::new(pg));
    }

    #[cfg(not(feature = "postgres"))]
    if !cfg.pg_url.trim().is_empty() {
        tracing::warn!(
            "contacts.pg_url is set but the postgres feature is disabled; falling back to SQLite"
        );
    }

    Ok(std::sync::Arc::new(crate::ContactDb::open(
        std::path::Path::new(&cfg.db_path),
    )?))
}

#[async_trait]
impl ContactStore for crate::ContactDb {
    async fn add_contact(&self, contact: &Contact) -> anyhow::Result<i64> {
        self.add_contact(contact)
    }

    async fn get_contact(&self, id: i64) -> anyhow::Result<Option<Contact>> {
        Ok(self.get_contact(id))
    }

    async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<Contact>> {
        Ok(self.find_by_email(email))
    }

    async fn find_by_phone(&self, phone: &str) -> anyhow::Result<Option<Contact>> {
        Ok(self.find_by_phone(phone))
    }

    async fn search(&self, query: &str) -> anyhow::Result<Vec<Contact>> {
        Ok(self.search(query))
    }

    async fn list_all(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<Contact>> {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        let offset = usize::try_from(offset).unwrap_or(0);
        Ok(self.list_all(limit, offset))
    }

    async fn count(&self) -> anyhow::Result<i64> {
        Ok(self.count() as i64)
    }

    async fn merge_contacts(&self, primary_id: i64, duplicate_id: i64) -> anyhow::Result<()> {
        self.merge_contacts(primary_id, duplicate_id)
    }

    async fn set_crm_id(&self, contact_id: i64, crm_id: &str) -> anyhow::Result<()> {
        self.set_crm_id(contact_id, crm_id)
    }

    async fn delete_contact(&self, contact_id: i64) -> anyhow::Result<()> {
        self.delete_contact(contact_id)
    }

    async fn save_deduped(&self, contact: &Contact) -> anyhow::Result<(i64, bool)> {
        self.save_deduped(contact)
    }

    fn backend(&self) -> &'static str {
        "sqlite"
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl ContactStore for crate::PgContactDb {
    async fn add_contact(&self, contact: &Contact) -> anyhow::Result<i64> {
        self.add_contact(contact).await
    }

    async fn get_contact(&self, id: i64) -> anyhow::Result<Option<Contact>> {
        self.get_contact(id).await
    }

    async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<Contact>> {
        self.find_by_email(email).await
    }

    async fn find_by_phone(&self, phone: &str) -> anyhow::Result<Option<Contact>> {
        self.find_by_phone(phone).await
    }

    async fn search(&self, query: &str) -> anyhow::Result<Vec<Contact>> {
        self.search(query).await
    }

    async fn list_all(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<Contact>> {
        self.list_all(limit, offset).await
    }

    async fn count(&self) -> anyhow::Result<i64> {
        self.count().await
    }

    async fn merge_contacts(&self, primary_id: i64, duplicate_id: i64) -> anyhow::Result<()> {
        self.merge_contacts(primary_id, duplicate_id).await
    }

    async fn set_crm_id(&self, contact_id: i64, crm_id: &str) -> anyhow::Result<()> {
        self.set_crm_id(contact_id, crm_id).await
    }

    async fn delete_contact(&self, contact_id: i64) -> anyhow::Result<()> {
        self.delete_contact(contact_id).await
    }

    async fn save_deduped(&self, contact: &Contact) -> anyhow::Result<(i64, bool)> {
        self.save_deduped(contact).await
    }

    fn backend(&self) -> &'static str {
        "postgres"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn sample_contact(email: &str, name: &str) -> Contact {
        Contact {
            email: Some(email.to_string()),
            name: if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            },
            ..Contact::new()
        }
        .with_source("audit-test")
    }

    #[tokio::test]
    async fn test_sqlite_store_roundtrip_via_trait() {
        let store: Arc<dyn ContactStore> = Arc::new(crate::ContactDb::in_memory().unwrap());
        assert_eq!(store.backend(), "sqlite");

        let id = store.add_contact(&sample_contact("Alice@Example.com ", "Alice")).await.unwrap();
        assert!(id > 0);
        assert_eq!(store.count().await.unwrap(), 1);

        // find_by_email normalizes (trim + lowercase).
        let found = store.find_by_email("alice@example.com").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name.as_deref(), Some("Alice"));

        let listed = store.list_all(10, 0).await.unwrap();
        assert_eq!(listed.len(), 1);

        let fetched = store.get_contact(id).await.unwrap();
        assert!(fetched.is_some());
    }

    #[tokio::test]
    async fn test_sqlite_store_search_and_merge() {
        let store: Arc<dyn ContactStore> = Arc::new(crate::ContactDb::in_memory().unwrap());

        let a = store
            .add_contact(&sample_contact("dup@acme.com", "Bob"))
            .await
            .unwrap();
        let b = store
            .add_contact(&sample_contact("DUP@acme.com", ""))
            .await
            .unwrap();

        assert!(!store.search("acme").await.unwrap().is_empty());

        store.merge_contacts(a, b).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);
        let merged = store.get_contact(a).await.unwrap().unwrap();
        assert_eq!(merged.name.as_deref(), Some("Bob"));
    }
}
