//! Optional PostgreSQL contact backend for large-scale deployments.
//!
//! Enabled with the `postgres` feature of `pr-persistence`. The interface
//! mirrors [`crate::contacts::ContactDb`]; async read methods return
//! `Result` wrappers because a networked backend can fail on any call.

use std::collections::HashMap;

use deadpool_postgres::{Config, Pool, Runtime};
use pr_core::{normalize_email, normalize_phone, Company, Contact, SocialProfile, Verification};
use tokio_postgres::NoTls;

/// Column list shared by all contact row queries.
const CONTACT_COLS: &str = "c.id, c.email, c.phone, c.name, c.title, c.company, \
     c.created_at, c.updated_at, c.source, c.crm_id";

/// Maximum number of ids per `IN (...)` clause (stays well below
/// PostgreSQL's 65535 bind-parameter limit).
const ID_BATCH_SIZE: usize = 1000;

/// Build a `$start, $start+1, ...` placeholder list for `count` parameters.
fn in_placeholders(start: usize, count: usize) -> String {
    (start..start + count)
        .map(|i| format!("${i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Bind a slice of ids as query parameters.
fn id_params(batch: &[i64]) -> Vec<&(dyn tokio_postgres::types::ToSql + Sync)> {
    batch
        .iter()
        .map(|id| id as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect()
}

/// PostgreSQL-backed contact store.
#[derive(Clone)]
pub struct PgContactDb {
    pool: Pool,
}

impl PgContactDb {
    /// Connect to PostgreSQL using a connection URL
    /// (`postgres://user:pass@host/db`) and initialise the schema.
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let mut cfg = Config::new();
        cfg.url = Some(url.to_string());
        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
        let db = Self { pool };
        db.init_schema().await?;
        Ok(db)
    }

    /// Build from an existing pool (the schema must already exist or
    /// [`Self::init_schema`] must be called).
    pub fn from_pool(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn init_schema(&self) -> anyhow::Result<()> {
        let client = self.pool.get().await?;
        client
            .batch_execute(
                r#"
            CREATE TABLE IF NOT EXISTS contacts (
                id BIGSERIAL PRIMARY KEY,
                email TEXT,
                phone TEXT,
                phone_norm TEXT,
                name TEXT,
                title TEXT,
                company TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'unknown',
                crm_id TEXT
            );

            CREATE TABLE IF NOT EXISTS social_profiles (
                id BIGSERIAL PRIMARY KEY,
                contact_id BIGINT NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                platform TEXT NOT NULL,
                url TEXT NOT NULL DEFAULT '',
                username TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS companies (
                id BIGSERIAL PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                website TEXT,
                industry TEXT,
                size TEXT,
                location TEXT,
                description TEXT
            );

            CREATE TABLE IF NOT EXISTS tags (
                id BIGSERIAL PRIMARY KEY,
                contact_id BIGINT NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                tag TEXT NOT NULL,
                UNIQUE (contact_id, tag)
            );

            CREATE TABLE IF NOT EXISTS notes (
                id BIGSERIAL PRIMARY KEY,
                contact_id BIGINT NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                note TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_contacts_email ON contacts(email);
            CREATE INDEX IF NOT EXISTS idx_contacts_phone_norm ON contacts(phone_norm);
            CREATE INDEX IF NOT EXISTS idx_social_contact ON social_profiles(contact_id);
            CREATE INDEX IF NOT EXISTS idx_tags_contact ON tags(contact_id);
            CREATE INDEX IF NOT EXISTS idx_notes_contact ON notes(contact_id);
        "#,
            )
            .await?;
        // Migrate databases created before these columns existed.
        client
            .batch_execute(
                "ALTER TABLE contacts ADD COLUMN IF NOT EXISTS crm_id TEXT;
                 ALTER TABLE contacts ADD COLUMN IF NOT EXISTS phone_norm TEXT;",
            )
            .await?;
        Ok(())
    }

    // ── Write API ───────────────────────────────────────────────────────

    /// Insert a contact with its social profiles, tags and notes.
    /// Returns the new contact id.
    pub async fn add_contact(&self, contact: &Contact) -> anyhow::Result<i64> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;

        let email = contact
            .email
            .as_deref()
            .map(normalize_email)
            .filter(|e| !e.is_empty());
        let phone = contact
            .phone
            .as_deref()
            .map(str::trim)
            .map(str::to_string)
            .filter(|p| !p.is_empty());
        // Digits-only form used by the indexed `find_by_phone` lookup.
        // `Some("")` for phones without digits; `NULL` only when there is
        // no phone at all (also marks pre-migration rows for backfill).
        let phone_norm = phone.as_deref().map(normalize_phone);
        let now = chrono::Utc::now().to_rfc3339();

        let row = tx
            .query_one(
                "INSERT INTO contacts (email, phone, phone_norm, name, title, company, created_at, updated_at, source, crm_id)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
                &[
                    &email,
                    &phone,
                    &phone_norm,
                    &contact.name,
                    &contact.title,
                    &contact.company,
                    &contact.created_at.to_rfc3339(),
                    &now,
                    &contact.source,
                    &contact.crm_id,
                ],
            )
            .await?;
        let contact_id: i64 = row.get(0);

        for sp in &contact.social_profiles {
            tx.execute(
                "INSERT INTO social_profiles (contact_id, platform, url, username) VALUES ($1, $2, $3, $4)",
                &[&contact_id, &sp.platform, &sp.url, &sp.username],
            )
            .await?;
        }
        for tag in &contact.tags {
            let tag = tag.trim();
            if !tag.is_empty() {
                tx.execute(
                    "INSERT INTO tags (contact_id, tag) VALUES ($1, $2) ON CONFLICT (contact_id, tag) DO NOTHING",
                    &[&contact_id, &tag],
                )
                .await?;
            }
        }
        for note in &contact.notes {
            tx.execute(
                "INSERT INTO notes (contact_id, note, created_at) VALUES ($1, $2, $3)",
                &[&contact_id, &note, &now],
            )
            .await?;
        }

        tx.commit().await?;
        Ok(contact_id)
    }

    /// Attach a tag to a contact (no-op if already present).
    pub async fn add_tag(&self, contact_id: i64, tag: &str) -> anyhow::Result<()> {
        let tag = tag.trim();
        anyhow::ensure!(!tag.is_empty(), "tag must not be empty");
        let client = self.pool.get().await?;
        let n = client
            .execute(
                "INSERT INTO tags (contact_id, tag) VALUES ($1, $2) ON CONFLICT (contact_id, tag) DO NOTHING",
                &[&contact_id, &tag],
            )
            .await?;
        if n == 0 {
            // Either a duplicate tag or a missing contact; distinguish them.
            let exists: bool = client
                .query_one("SELECT EXISTS(SELECT 1 FROM contacts WHERE id = $1)", &[&contact_id])
                .await?
                .get(0);
            anyhow::ensure!(exists, "contact {contact_id} not found");
        }
        Ok(())
    }

    /// Attach a note to a contact.
    pub async fn add_note(&self, contact_id: i64, note: &str) -> anyhow::Result<()> {
        let client = self.pool.get().await?;
        let now = chrono::Utc::now().to_rfc3339();
        let n = client
            .execute(
                "INSERT INTO notes (contact_id, note, created_at) VALUES ($1, $2, $3)",
                &[&contact_id, &note, &now],
            )
            .await?;
        anyhow::ensure!(n > 0, "contact {contact_id} not found");
        Ok(())
    }

    /// Delete a contact and all its extras (socials/tags/notes cascade).
    pub async fn delete_contact(&self, contact_id: i64) -> anyhow::Result<()> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM contacts WHERE id = $1", &[&contact_id])
            .await?;
        Ok(())
    }

    /// Atomic find-or-insert (fleet round 2, TOCTOU fix). Returns
    /// `(contact_id, was_merged)`.
    pub async fn save_deduped(&self, contact: &Contact) -> anyhow::Result<(i64, bool)> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;

        let email = contact
            .email
            .as_deref()
            .map(normalize_email)
            .filter(|e| !e.is_empty());
        let phone = contact
            .phone
            .as_deref()
            .map(str::trim)
            .map(str::to_string)
            .filter(|p| !p.is_empty());
        let phone_norm = phone.as_deref().map(normalize_phone);
        let now = chrono::Utc::now().to_rfc3339();

        let mut existing: Option<i64> = None;
        if let Some(ref e) = email {
            existing = tx
                .query_opt(
                    "SELECT id FROM contacts WHERE email = $1 ORDER BY id LIMIT 1",
                    &[e],
                )
                .await?
                .map(|row| row.get(0));
        }
        if existing.is_none() {
            if let Some(ref pn) = phone_norm {
                if !pn.is_empty() {
                    existing = tx
                        .query_opt(
                            "SELECT id FROM contacts WHERE phone_norm = $1 ORDER BY id LIMIT 1",
                            &[pn],
                        )
                        .await?
                        .map(|row| row.get(0));
                }
            }
        }

        let row = tx
            .query_one(
                "INSERT INTO contacts (email, phone, phone_norm, name, title, company, created_at, updated_at, source, crm_id)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
                &[
                    &email,
                    &phone,
                    &phone_norm,
                    &contact.name,
                    &contact.title,
                    &contact.company,
                    &contact.created_at.to_rfc3339(),
                    &now,
                    &contact.source,
                    &contact.crm_id,
                ],
            )
            .await?;
        let new_id: i64 = row.get(0);

        for sp in &contact.social_profiles {
            tx.execute(
                "INSERT INTO social_profiles (contact_id, platform, url, username) VALUES ($1, $2, $3, $4)",
                &[&new_id, &sp.platform, &sp.url, &sp.username],
            )
            .await?;
        }
        for tag in &contact.tags {
            let tag = tag.trim();
            if !tag.is_empty() {
                tx.execute(
                    "INSERT INTO tags (contact_id, tag) VALUES ($1, $2) ON CONFLICT (contact_id, tag) DO NOTHING",
                    &[&new_id, &tag],
                )
                .await?;
            }
        }
        for note in &contact.notes {
            tx.execute(
                "INSERT INTO notes (contact_id, note, created_at) VALUES ($1, $2, $3)",
                &[&new_id, &note, &now],
            )
            .await?;
        }

        let Some(old_id) = existing else {
            tx.commit().await?;
            return Ok((new_id, false));
        };

        let old = tx
            .query_one(
                "SELECT email, phone, name, title, company, crm_id FROM contacts WHERE id = $1",
                &[&old_id],
            )
            .await?;
        let o_email: Option<String> = old.get(0);
        let o_phone: Option<String> = old.get(1);
        let o_name: Option<String> = old.get(2);
        let o_title: Option<String> = old.get(3);
        let o_company: Option<String> = old.get(4);
        let o_crm: Option<String> = old.get(5);
        let m_email = o_email.or_else(|| email.clone());
        let m_phone = o_phone.or_else(|| phone.clone());
        let m_phone_norm = m_phone.as_deref().map(normalize_phone);
        let m_name = o_name.or_else(|| contact.name.clone());
        let m_title = o_title.or_else(|| contact.title.clone());
        let m_company = o_company.or_else(|| contact.company.clone());
        let m_crm = o_crm.or_else(|| contact.crm_id.clone());
        tx.execute(
            "UPDATE contacts SET email=$2, phone=$3, phone_norm=$4, name=$5, title=$6, company=$7, crm_id=$8, updated_at=$9
             WHERE id=$1",
            &[&old_id, &m_email, &m_phone, &m_phone_norm, &m_name, &m_title, &m_company, &m_crm, &now],
        )
        .await?;
        tx.execute(
            "INSERT INTO social_profiles (contact_id, platform, url, username)
             SELECT $1, sp.platform, sp.url, sp.username
             FROM social_profiles sp
             WHERE sp.contact_id = $2
               AND NOT EXISTS (
                   SELECT 1 FROM social_profiles p2
                   WHERE p2.contact_id = $1
                     AND p2.platform = sp.platform
                     AND p2.url = sp.url
                     AND p2.username = sp.username)",
            &[&old_id, &new_id],
        )
        .await?;
        tx.execute("DELETE FROM social_profiles WHERE contact_id = $1", &[&new_id])
            .await?;
        tx.execute(
            "INSERT INTO tags (contact_id, tag)
             SELECT $1, tag FROM tags WHERE contact_id = $2
             ON CONFLICT (contact_id, tag) DO NOTHING",
            &[&old_id, &new_id],
        )
        .await?;
        tx.execute("DELETE FROM tags WHERE contact_id = $1", &[&new_id])
            .await?;
        tx.execute(
            "UPDATE notes SET contact_id = $1 WHERE contact_id = $2",
            &[&old_id, &new_id],
        )
        .await?;
        tx.execute("DELETE FROM contacts WHERE id = $1", &[&new_id])
            .await?;

        tx.commit().await?;
        Ok((old_id, true))
    }

    /// Record the remote CRM id after a successful push.
    pub async fn set_crm_id(&self, contact_id: i64, crm_id: &str) -> anyhow::Result<()> {
        let client = self.pool.get().await?;
        let now = chrono::Utc::now().to_rfc3339();
        let n = client
            .execute(
                "UPDATE contacts SET crm_id=$2, updated_at=$3 WHERE id=$1",
                &[&contact_id, &crm_id, &now],
            )
            .await?;
        anyhow::ensure!(n > 0, "contact {contact_id} not found");
        Ok(())
    }

    /// Insert or update a company (keyed by name). Returns the company id.
    pub async fn upsert_company(&self, company: &Company) -> anyhow::Result<i64> {
        let client = self.pool.get().await?;
        let row = client
            .query_one(
                "INSERT INTO companies (name, website, industry, size, location, description)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (name) DO UPDATE SET
                     website = EXCLUDED.website,
                     industry = EXCLUDED.industry,
                     size = EXCLUDED.size,
                     location = EXCLUDED.location,
                     description = EXCLUDED.description
                 RETURNING id",
                &[
                    &company.name,
                    &company.website,
                    &company.industry,
                    &company.size,
                    &company.location,
                    &company.description,
                ],
            )
            .await?;
        Ok(row.get(0))
    }

    // ── Read API ────────────────────────────────────────────────────────

    /// Fetch a single contact by id (with social profiles, tags and notes).
    pub async fn get_contact(&self, id: i64) -> anyhow::Result<Option<Contact>> {
        Ok(self.load_by_ids(&[id]).await?.pop())
    }

    /// Find a contact by (case-insensitive, trimmed) email address.
    pub async fn find_by_email(&self, email: &str) -> anyhow::Result<Option<Contact>> {
        let email = normalize_email(email);
        if email.is_empty() {
            return Ok(None);
        }
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT id FROM contacts WHERE email = $1 ORDER BY id LIMIT 1",
                &[&email],
            )
            .await?;
        match row {
            Some(row) => {
                let id: i64 = row.get(0);
                drop(client);
                self.get_contact(id).await
            }
            None => Ok(None),
        }
    }

    /// Find a contact by phone number (compared by digits only).
    ///
    /// Uses the `idx_contacts_phone_norm` index on the stored digits-only
    /// `phone_norm` column instead of normalizing every row in Rust. Rows
    /// written before the column existed (`phone_norm IS NULL`) are handled
    /// by a scan fallback that also writes the normalized value back, so
    /// the backfill happens lazily and idempotently.
    pub async fn find_by_phone(&self, phone: &str) -> anyhow::Result<Option<Contact>> {
        let needle = normalize_phone(phone);
        if needle.is_empty() {
            return Ok(None);
        }
        let client = self.pool.get().await?;
        // Fast path: indexed equality on the normalized phone.
        // `ORDER BY id LIMIT 1` keeps the historical "lowest id wins"
        // semantics of the old scan.
        let row = client
            .query_opt(
                "SELECT id FROM contacts WHERE phone_norm = $1 ORDER BY id LIMIT 1",
                &[&needle],
            )
            .await?;
        let mut found: Option<i64> = row.map(|r| r.get(0));
        if found.is_none() {
            // Fallback: legacy rows whose phone_norm has not been
            // backfilled yet. Normalize in Rust and persist the result so
            // the next lookup hits the index.
            let rows = client
                .query(
                    "SELECT id, phone FROM contacts
                     WHERE phone IS NOT NULL AND phone_norm IS NULL
                     ORDER BY id",
                    &[],
                )
                .await?;
            for row in rows {
                let id: i64 = row.get(0);
                let raw: String = row.get(1);
                let norm = normalize_phone(&raw);
                let _ = client
                    .execute(
                        "UPDATE contacts SET phone_norm = $2 WHERE id = $1",
                        &[&id, &norm],
                    )
                    .await;
                if found.is_none() && norm == needle {
                    found = Some(id);
                }
            }
        }
        drop(client);
        match found {
            Some(id) => self.get_contact(id).await,
            None => Ok(None),
        }
    }

    /// Search contacts by substring over name, email, phone, title, company
    /// and tags (case-insensitive). Most recently added first.
    ///
    /// Issues one row query plus three batched extras queries (socials,
    /// tags, notes with `contact_id IN (...)`) regardless of result size.
    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<Contact>> {
        let needle = query.trim();
        if needle.is_empty() {
            return self.list_all(i64::MAX, 0).await;
        }
        let pattern = format!("%{}%", escape_like(needle));
        let client = self.pool.get().await?;
        let rows = client
            .query(
                &format!(
                    "SELECT DISTINCT {CONTACT_COLS} FROM contacts c
                     LEFT JOIN tags t ON t.contact_id = c.id
                     WHERE c.name ILIKE $1 ESCAPE '\\'
                        OR c.email ILIKE $1 ESCAPE '\\'
                        OR c.phone ILIKE $1 ESCAPE '\\'
                        OR c.title ILIKE $1 ESCAPE '\\'
                        OR c.company ILIKE $1 ESCAPE '\\'
                        OR t.tag ILIKE $1 ESCAPE '\\'
                     ORDER BY c.id DESC"
                ),
                &[&pattern],
            )
            .await?;
        let contacts = rows
            .iter()
            .map(contact_from_row)
            .collect::<anyhow::Result<Vec<_>>>()?;
        self.attach_extras(&client, contacts).await
    }

    /// List all contacts, most recently added first.
    ///
    /// Issues one row query plus three batched extras queries (socials,
    /// tags, notes with `contact_id IN (...)`) regardless of result size.
    pub async fn list_all(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<Contact>> {
        let client = self.pool.get().await?;
        // LIMIT NULL means "no limit" in PostgreSQL.
        let limit: Option<i64> = if limit < 0 { None } else { Some(limit) };
        let rows = client
            .query(
                &format!(
                    "SELECT {CONTACT_COLS} FROM contacts c ORDER BY c.id DESC LIMIT $1 OFFSET $2"
                ),
                &[&limit, &offset],
            )
            .await?;
        let contacts = rows
            .iter()
            .map(contact_from_row)
            .collect::<anyhow::Result<Vec<_>>>()?;
        self.attach_extras(&client, contacts).await
    }

    /// Total number of stored contacts.
    pub async fn count(&self) -> anyhow::Result<i64> {
        let client = self.pool.get().await?;
        let row = client
            .query_one("SELECT COUNT(*) FROM contacts", &[])
            .await?;
        Ok(row.get::<_, i64>(0))
    }

    /// List all companies ordered by name.
    pub async fn list_companies(&self) -> anyhow::Result<Vec<Company>> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, name, website, industry, size, location, description
                 FROM companies ORDER BY name",
                &[],
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| Company {
                id: row.get(0),
                name: row.get(1),
                website: row.get(2),
                industry: row.get(3),
                size: row.get(4),
                location: row.get(5),
                description: row.get(6),
            })
            .collect())
    }

    // ── Deduplication ───────────────────────────────────────────────────

    /// Find pairs of contacts that share a normalized email or phone number.
    /// Each pair is ordered by id (older contact first).
    pub async fn find_duplicates(&self) -> anyhow::Result<Vec<(Contact, Contact)>> {
        let contacts = self.list_all(i64::MAX, 0).await?;

        let mut by_email: std::collections::HashMap<String, Vec<i64>> =
            std::collections::HashMap::new();
        let mut by_phone: std::collections::HashMap<String, Vec<i64>> =
            std::collections::HashMap::new();
        for contact in &contacts {
            let Some(id) = contact.id else { continue };
            if let Some(email) = contact.normalized_email() {
                by_email.entry(email).or_default().push(id);
            }
            if let Some(phone) = contact.normalized_phone() {
                by_phone.entry(phone).or_default().push(id);
            }
        }

        let mut pairs: Vec<(i64, i64)> = Vec::new();
        for group in by_email.values().chain(by_phone.values()) {
            if group.len() < 2 {
                continue;
            }
            let mut sorted = group.clone();
            sorted.sort_unstable();
            for i in 0..sorted.len() {
                for j in (i + 1)..sorted.len() {
                    pairs.push((sorted[i], sorted[j]));
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();

        let by_id: std::collections::HashMap<i64, &Contact> = contacts
            .iter()
            .filter_map(|c| c.id.map(|id| (id, c)))
            .collect();
        Ok(pairs
            .into_iter()
            .filter_map(|(a, b)| {
                let primary: Contact = (*by_id.get(&a)?).clone();
                let duplicate: Contact = (*by_id.get(&b)?).clone();
                Some((primary, duplicate))
            })
            .collect())
    }

    /// Merge `duplicate_id` into `primary_id`: blank fields of the primary
    /// are filled from the duplicate, social profiles / tags / notes are
    /// moved over, and the duplicate contact is deleted.
    pub async fn merge_contacts(&self, primary_id: i64, duplicate_id: i64) -> anyhow::Result<()> {
        anyhow::ensure!(
            primary_id != duplicate_id,
            "cannot merge a contact with itself"
        );
        let primary = self
            .get_contact(primary_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("contact {primary_id} not found"))?;
        let duplicate = self
            .get_contact(duplicate_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("contact {duplicate_id} not found"))?;

        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let now = chrono::Utc::now().to_rfc3339();

        let email = primary.email.clone().or_else(|| duplicate.email.clone());
        let phone = primary.phone.clone().or_else(|| duplicate.phone.clone());
        let phone_norm = phone.as_deref().map(normalize_phone);
        let name = primary.name.clone().or_else(|| duplicate.name.clone());
        let title = primary.title.clone().or_else(|| duplicate.title.clone());
        let company = primary.company.clone().or_else(|| duplicate.company.clone());
        let crm_id = primary.crm_id.clone().or_else(|| duplicate.crm_id.clone());
        tx.execute(
            "UPDATE contacts SET email=$2, phone=$3, phone_norm=$4, name=$5, title=$6, company=$7, crm_id=$8, updated_at=$9
             WHERE id=$1",
            &[&primary_id, &email, &phone, &phone_norm, &name, &title, &company, &crm_id, &now],
        )
        .await?;

        tx.execute(
            "INSERT INTO social_profiles (contact_id, platform, url, username)
             SELECT $1, sp.platform, sp.url, sp.username
             FROM social_profiles sp
             WHERE sp.contact_id = $2
               AND NOT EXISTS (
                   SELECT 1 FROM social_profiles p2
                   WHERE p2.contact_id = $1
                     AND p2.platform = sp.platform
                     AND p2.url = sp.url
                     AND p2.username = sp.username)",
            &[&primary_id, &duplicate_id],
        )
        .await?;
        tx.execute(
            "DELETE FROM social_profiles WHERE contact_id = $1",
            &[&duplicate_id],
        )
        .await?;

        tx.execute(
            "INSERT INTO tags (contact_id, tag)
             SELECT $1, tag FROM tags WHERE contact_id = $2
             ON CONFLICT (contact_id, tag) DO NOTHING",
            &[&primary_id, &duplicate_id],
        )
        .await?;
        tx.execute("DELETE FROM tags WHERE contact_id = $1", &[&duplicate_id])
            .await?;

        tx.execute(
            "UPDATE notes SET contact_id = $1 WHERE contact_id = $2",
            &[&primary_id, &duplicate_id],
        )
        .await?;

        tx.execute("DELETE FROM contacts WHERE id = $1", &[&duplicate_id])
            .await?;

        tx.commit().await?;
        tracing::info!(primary_id, duplicate_id, "merged duplicate contacts (postgres)");
        Ok(())
    }

    // ── Internal helpers ────────────────────────────────────────────────

    /// Load contacts by id with social profiles, tags and notes.
    ///
    /// Batched: one `id IN (...)` row query plus one extras query per kind
    /// (`social_profiles` / `tags` / `notes`, each `contact_id IN (...)`),
    /// no matter how many ids are requested. Results are returned in the
    /// order of `ids`.
    async fn load_by_ids(&self, ids: &[i64]) -> anyhow::Result<Vec<Contact>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let client = self.pool.get().await?;
        let mut contacts = Vec::with_capacity(ids.len());
        for batch in ids.chunks(ID_BATCH_SIZE) {
            let sql = format!(
                "SELECT {CONTACT_COLS} FROM contacts c WHERE c.id IN ({})",
                in_placeholders(1, batch.len()),
            );
            let rows = client.query(&sql, &id_params(batch)).await?;
            for row in rows {
                contacts.push(contact_from_row(&row)?);
            }
        }
        // `IN (...)` does not preserve order; restore the requested order.
        let position: HashMap<i64, usize> =
            ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        contacts.sort_by_key(|c| {
            c.id.and_then(|id| position.get(&id).copied())
                .unwrap_or(usize::MAX)
        });
        self.attach_extras(&client, contacts).await
    }

    /// Attach social profiles, tags and notes to already-loaded contact
    /// rows using three `contact_id IN (...)` queries per batch instead of
    /// three queries per contact. Within each contact, extras keep
    /// insertion order (the queries are ordered by row id).
    async fn attach_extras(
        &self,
        client: &deadpool_postgres::Client,
        mut contacts: Vec<Contact>,
    ) -> anyhow::Result<Vec<Contact>> {
        if contacts.is_empty() {
            return Ok(contacts);
        }
        let ids: Vec<i64> = contacts.iter().filter_map(|c| c.id).collect();
        let position: HashMap<i64, usize> = contacts
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.id.map(|id| (id, i)))
            .collect();

        for batch in ids.chunks(ID_BATCH_SIZE) {
            let in_clause = in_placeholders(1, batch.len());
            let params = id_params(batch);

            let rows = client
                .query(
                    &format!(
                        "SELECT contact_id, platform, url, username FROM social_profiles
                         WHERE contact_id IN ({in_clause}) ORDER BY id"
                    ),
                    &params,
                )
                .await?;
            for row in rows {
                let contact_id: i64 = row.get(0);
                if let Some(&i) = position.get(&contact_id) {
                    contacts[i].social_profiles.push(SocialProfile {
                        id: None,
                        platform: row.get(1),
                        url: row.get(2),
                        username: row.get(3),
                    });
                }
            }

            let rows = client
                .query(
                    &format!(
                        "SELECT contact_id, tag FROM tags
                         WHERE contact_id IN ({in_clause}) ORDER BY id"
                    ),
                    &params,
                )
                .await?;
            for row in rows {
                let contact_id: i64 = row.get(0);
                if let Some(&i) = position.get(&contact_id) {
                    contacts[i].tags.push(row.get(1));
                }
            }

            let rows = client
                .query(
                    &format!(
                        "SELECT contact_id, note FROM notes
                         WHERE contact_id IN ({in_clause}) ORDER BY id"
                    ),
                    &params,
                )
                .await?;
            for row in rows {
                let contact_id: i64 = row.get(0);
                if let Some(&i) = position.get(&contact_id) {
                    contacts[i].notes.push(row.get(1));
                }
            }
        }

        Ok(contacts)
    }
}

fn contact_from_row(row: &tokio_postgres::Row) -> anyhow::Result<Contact> {
    let created_at: String = row.get(6);
    let updated_at: String = row.get(7);
    Ok(Contact {
        id: row.get(0),
        email: row.get(1),
        phone: row.get(2),
        name: row.get(3),
        title: row.get(4),
        company: row.get(5),
        social_profiles: Vec::new(),
        tags: Vec::new(),
        notes: Vec::new(),
        source: row.get(8),
        crm_id: row.get(9),
        verification: Verification::Unverified,
        created_at: parse_rfc3339(&created_at),
        updated_at: parse_rfc3339(&updated_at),
    })
}

fn parse_rfc3339(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

/// Escape SQL LIKE/ILIKE wildcards so user input matches literally.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration tests run only when `TEST_PG_URL` points at a reachable
    /// PostgreSQL instance, e.g.
    /// `TEST_PG_URL=postgres://postgres:postgres@localhost/postgres cargo test -p pr-persistence --features postgres`.
    fn pg_url_from_env() -> Option<String> {
        std::env::var("TEST_PG_URL").ok().filter(|s| !s.trim().is_empty())
    }

    fn unique_email(label: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!("{label}-{nanos}@example.com")
    }

    /// A unique digits-only phone stem (11 digits, Russian-mobile shape).
    fn unique_phone_formatted() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() % 1_000_000_000)
            .unwrap_or_default();
        format!("+7 (9{:02}) {:03}-{:02}-{:02}",
            nanos / 10_000_000,
            (nanos / 10_000) % 1_000,
            (nanos / 100) % 100,
            nanos % 100)
    }

    fn sample_contact(name: &str, email: &str) -> Contact {
        let mut c = Contact::new().with_source("pg-test");
        c.name = Some(name.to_string());
        c.email = Some(email.to_string());
        c
    }

    #[tokio::test]
    async fn test_pg_contact_lifecycle() {
        let Some(url) = pg_url_from_env() else {
            eprintln!("TEST_PG_URL not set; skipping PostgreSQL integration test");
            return;
        };
        let db = PgContactDb::connect(&url).await.expect("connect to postgres");

        let email = unique_email("jane");
        let mut contact = sample_contact("Jane Doe", &email);
        contact.phone = Some("+1 (555) 010-0100".into());
        contact.title = Some("CTO".into());
        contact
            .social_profiles
            .push(SocialProfile::new("linkedin", "https://linkedin.com/in/jdoe", "jdoe"));
        contact.tags = vec!["lead".into(), "vip".into()];
        contact.notes = vec!["imported".into()];

        let id = db.add_contact(&contact).await.expect("insert");

        let loaded = db.get_contact(id).await.unwrap().expect("exists");
        assert_eq!(loaded.name.as_deref(), Some("Jane Doe"));
        assert_eq!(loaded.tags.len(), 2);
        assert_eq!(loaded.social_profiles.len(), 1);
        assert_eq!(loaded.notes, vec!["imported"]);

        assert!(db.find_by_email(&email.to_uppercase()).await.unwrap().is_some());
        assert!(db.find_by_phone("1-555-010-0100").await.unwrap().is_some());
        assert_eq!(db.search("Jane Doe").await.unwrap().len(), 1);

        // Duplicate + merge.
        let dup_id = db.add_contact(&sample_contact("Jane Dup", &email)).await.unwrap();
        let dupes = db.find_duplicates().await.unwrap();
        assert!(dupes
            .iter()
            .any(|(a, b)| (a.id == Some(id) && b.id == Some(dup_id))
                || (a.id == Some(dup_id) && b.id == Some(id))));

        db.merge_contacts(id, dup_id).await.unwrap();
        assert!(db.get_contact(dup_id).await.unwrap().is_none());
        assert!(db.get_contact(id).await.unwrap().is_some());
    }

    /// `list_all` and `search` must return exactly the same full `Contact`
    /// structures (socials, tags and notes included) as the per-id fetch —
    /// the batched `IN (...)` extras loading must not change shapes or
    /// ordering semantics.
    #[tokio::test]
    async fn test_pg_list_and_search_full_extras_structure() {
        let Some(url) = pg_url_from_env() else {
            eprintln!("TEST_PG_URL not set; skipping PostgreSQL integration test");
            return;
        };
        let db = PgContactDb::connect(&url).await.expect("connect to postgres");
        let suffix = unique_email("extras");

        let mut a = sample_contact("Alice Johnson", &format!("alice-{suffix}"));
        a.social_profiles = vec![
            SocialProfile::new("linkedin", "https://linkedin.com/in/alice", "alice"),
            SocialProfile::new("twitter", "https://twitter.com/alice", "alicej"),
        ];
        a.tags = vec!["lead".into(), "decision-maker".into()];
        a.notes = vec!["Met at conference".into(), "Follow up in Q3".into()];

        let mut b = sample_contact("Bob Stone", &format!("bob-{suffix}"));
        b.social_profiles =
            vec![SocialProfile::new("telegram", "https://t.me/bobstone", "bobstone")];
        b.tags = vec!["vip".into()];
        b.notes = vec!["Referred by Alice".into()];

        let c = sample_contact("Carol Smith", &format!("carol-{suffix}"));

        let id_a = db.add_contact(&a).await.unwrap();
        let id_b = db.add_contact(&b).await.unwrap();
        let id_c = db.add_contact(&c).await.unwrap();

        let listed = db.list_all(i64::MAX, 0).await.unwrap();
        let ours: Vec<Contact> = listed
            .into_iter()
            .filter(|c| matches!(c.id, Some(id) if id == id_a || id == id_b || id == id_c))
            .collect();
        assert_eq!(ours.len(), 3);
        assert_eq!(ours[0].id, Some(id_c)); // newest first
        assert_eq!(ours[1].id, Some(id_b));
        assert_eq!(ours[2].id, Some(id_a));
        for contact in &ours {
            assert_eq!(
                contact,
                &db.get_contact(contact.id.unwrap()).await.unwrap().unwrap(),
                "list_all structure must match get_contact for id {:?}",
                contact.id
            );
        }

        let loaded_a = &ours[2];
        assert_eq!(loaded_a.social_profiles, a.social_profiles);
        assert_eq!(loaded_a.tags, a.tags);
        assert_eq!(loaded_a.notes, a.notes);
        let loaded_b = &ours[1];
        assert_eq!(loaded_b.social_profiles, b.social_profiles);
        assert_eq!(loaded_b.tags, b.tags);
        assert_eq!(loaded_b.notes, b.notes);
        assert!(ours[0].social_profiles.is_empty());

        // Each search hit is structurally identical to the list_all entry.
        for (needle, expected_id) in [
            ("Alice Johnson", id_a),
            ("Bob Stone", id_b),
            ("decision-maker", id_a),
            ("vip", id_b),
        ] {
            let hits = db.search(needle).await.unwrap();
            let hit = hits.iter().find(|c| c.id == Some(expected_id));
            assert!(hit.is_some(), "search({needle:?}) should match");
            let expected = ours.iter().find(|c| c.id == Some(expected_id)).unwrap();
            assert_eq!(hit.unwrap(), expected, "search({needle:?}) structure mismatch");
        }
    }

    /// `find_by_phone` matches on digits only, regardless of formatting,
    /// and lazily backfills legacy rows with `phone_norm IS NULL`.
    #[tokio::test]
    async fn test_pg_find_by_phone_normalized_variants_and_backfill() {
        let Some(url) = pg_url_from_env() else {
            eprintln!("TEST_PG_URL not set; skipping PostgreSQL integration test");
            return;
        };
        let db = PgContactDb::connect(&url).await.expect("connect to postgres");

        let phone = unique_phone_formatted();
        let digits = normalize_phone(&phone);
        let mut c = sample_contact("Ivan Petrov", &unique_email("ivan"));
        c.phone = Some(phone.clone());
        let id = db.add_contact(&c).await.unwrap();

        // Formatting variants resolve to the same contact.
        assert_eq!(
            db.find_by_phone(&digits).await.unwrap().and_then(|c| c.id),
            Some(id)
        );
        assert_eq!(
            db.find_by_phone(&format!("+{digits}")).await.unwrap().and_then(|c| c.id),
            Some(id)
        );
        assert_eq!(
            db.find_by_phone(&phone).await.unwrap().and_then(|c| c.id),
            Some(id),
            "the stored formatted phone itself must resolve"
        );
        assert!(db.find_by_phone("no digits").await.unwrap().is_none());

        // Legacy row: insert without phone_norm (simulates a pre-migration
        // row); the scan fallback must find and backfill it.
        let legacy_phone = unique_phone_formatted();
        let legacy_digits = normalize_phone(&legacy_phone);
        let client = db.pool.get().await.unwrap();
        let row = client
            .query_one(
                "INSERT INTO contacts (phone, created_at, updated_at, source)
                 VALUES ($1, $2, $2, 'legacy') RETURNING id",
                &[&legacy_phone, &chrono::Utc::now().to_rfc3339()],
            )
            .await
            .unwrap();
        let legacy_id: i64 = row.get(0);

        let found = db
            .find_by_phone(&legacy_digits)
            .await
            .unwrap()
            .expect("legacy row found via fallback scan");
        assert_eq!(found.id, Some(legacy_id));

        let norm: Option<String> = client
            .query_one(
                "SELECT phone_norm FROM contacts WHERE id = $1",
                &[&legacy_id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(norm.as_deref(), Some(legacy_digits.as_str()));

        // Second lookup resolves via the index.
        assert_eq!(
            db.find_by_phone(&legacy_digits).await.unwrap().and_then(|c| c.id),
            Some(legacy_id)
        );
    }

    #[test]
    fn test_escape_like() {
        assert_eq!(escape_like("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }
}
