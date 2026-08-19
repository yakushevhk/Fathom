//! Contact database: SQLite-backed storage for contacts collected during
//! research (OSINT / lead generation).
//!
//! Schema:
//! - `contacts`: id, email, phone, name, title, company, created_at,
//!   updated_at, source
//! - `social_profiles`: id, contact_id, platform, url, username
//! - `companies`: id, name, website, industry, size, location, description
//! - `tags`: id, contact_id, tag
//! - `notes`: id, contact_id, note, created_at

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use pr_core::{normalize_email, normalize_phone, Company, Contact, SocialProfile};
use rusqlite::{params, Connection};

/// SQLite-backed contact store.
pub struct ContactDb {
    conn: Mutex<Connection>,
}

impl ContactDb {
    /// Open (or create) the contact database at `path`.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (mainly for tests).
    pub fn in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS contacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
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
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                platform TEXT NOT NULL,
                url TEXT NOT NULL DEFAULT '',
                username TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS companies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                website TEXT,
                industry TEXT,
                size TEXT,
                location TEXT,
                description TEXT
            );

            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                tag TEXT NOT NULL,
                UNIQUE (contact_id, tag)
            );

            CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                note TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
        "#)?;

        // Migrate databases created before these columns existed. Must run
        // BEFORE the indexes below: idx_contacts_phone_norm references the
        // column and fails on legacy databases that lack it.
        crate::add_column_if_missing(&conn, "contacts", "crm_id", "TEXT")?;
        crate::add_column_if_missing(&conn, "contacts", "phone_norm", "TEXT")?;

        conn.execute_batch(r#"
            CREATE INDEX IF NOT EXISTS idx_contacts_email ON contacts(email);
            CREATE INDEX IF NOT EXISTS idx_contacts_phone_norm ON contacts(phone_norm);
            CREATE INDEX IF NOT EXISTS idx_social_contact ON social_profiles(contact_id);
            CREATE INDEX IF NOT EXISTS idx_tags_contact ON tags(contact_id);
            CREATE INDEX IF NOT EXISTS idx_notes_contact ON notes(contact_id);
        "#)?;
        Ok(())
    }

    // ── Write API ───────────────────────────────────────────────────────

    /// Insert a contact with its social profiles, tags and notes.
    /// Returns the new contact id.
    pub fn add_contact(&self, contact: &Contact) -> anyhow::Result<i64> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

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

        tx.execute(
            "INSERT INTO contacts (email, phone, phone_norm, name, title, company, created_at, updated_at, source, crm_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                email,
                phone,
                phone_norm,
                contact.name,
                contact.title,
                contact.company,
                contact.created_at.to_rfc3339(),
                now,
                contact.source,
                contact.crm_id,
            ],
        )?;
        let contact_id = tx.last_insert_rowid();

        for sp in &contact.social_profiles {
            tx.execute(
                "INSERT INTO social_profiles (contact_id, platform, url, username) VALUES (?1, ?2, ?3, ?4)",
                params![contact_id, sp.platform, sp.url, sp.username],
            )?;
        }
        for tag in &contact.tags {
            let tag = tag.trim();
            if !tag.is_empty() {
                tx.execute(
                    "INSERT OR IGNORE INTO tags (contact_id, tag) VALUES (?1, ?2)",
                    params![contact_id, tag],
                )?;
            }
        }
        for note in &contact.notes {
            tx.execute(
                "INSERT INTO notes (contact_id, note, created_at) VALUES (?1, ?2, ?3)",
                params![contact_id, note, now],
            )?;
        }

        tx.commit()?;
        Ok(contact_id)
    }

    /// Atomic find-or-insert (fleet round 2, TOCTOU fix): the dedup check
    /// and the insert run inside ONE locked transaction, so concurrent
    /// agents harvesting the same contact cannot both insert it.
    /// Returns `(contact_id, was_merged)`.
    pub fn save_deduped(&self, contact: &Contact) -> anyhow::Result<(i64, bool)> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

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

        // 1. Look for an existing contact by normalized email, then phone.
        let mut existing: Option<i64> = None;
        if let Some(ref e) = email {
            let found: Option<i64> = tx
                .query_row(
                    "SELECT id FROM contacts WHERE email = ?1 ORDER BY id LIMIT 1",
                    params![e],
                    |row| row.get(0),
                )
                .ok();
            existing = found;
        }
        if existing.is_none() {
            if let Some(ref pn) = phone_norm {
                if !pn.is_empty() {
                    let found: Option<i64> = tx
                        .query_row(
                            "SELECT id FROM contacts WHERE phone_norm = ?1 ORDER BY id LIMIT 1",
                            params![pn],
                            |row| row.get(0),
                        )
                        .ok();
                    existing = found;
                }
            }
        }

        // 2. Insert the new row (becomes either the result or merge fodder).
        tx.execute(
            "INSERT INTO contacts (email, phone, phone_norm, name, title, company, created_at, updated_at, source, crm_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                email,
                phone,
                phone_norm,
                contact.name,
                contact.title,
                contact.company,
                contact.created_at.to_rfc3339(),
                now,
                contact.source,
                contact.crm_id,
            ],
        )?;
        let new_id = tx.last_insert_rowid();

        for sp in &contact.social_profiles {
            tx.execute(
                "INSERT INTO social_profiles (contact_id, platform, url, username) VALUES (?1, ?2, ?3, ?4)",
                params![new_id, sp.platform, sp.url, sp.username],
            )?;
        }
        for tag in &contact.tags {
            let tag = tag.trim();
            if !tag.is_empty() {
                tx.execute(
                    "INSERT OR IGNORE INTO tags (contact_id, tag) VALUES (?1, ?2)",
                    params![new_id, tag],
                )?;
            }
        }
        for note in &contact.notes {
            tx.execute(
                "INSERT INTO notes (contact_id, note, created_at) VALUES (?1, ?2, ?3)",
                params![new_id, note, now],
            )?;
        }

        let Some(old_id) = existing else {
            tx.commit()?;
            return Ok((new_id as i64, false));
        };

        // 3. Merge the fresh row into the existing one (same semantics as
        // merge_contacts, but inside this transaction).
        let (o_email, o_phone, o_name, o_title, o_company, o_crm): (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = tx.query_row(
            "SELECT email, phone, name, title, company, crm_id FROM contacts WHERE id = ?1",
            params![old_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?;
        let m_email = o_email.or_else(|| email.clone());
        let m_phone = o_phone.or_else(|| phone.clone());
        let m_phone_norm = m_phone.as_deref().map(normalize_phone);
        let m_name = o_name.or_else(|| contact.name.clone());
        let m_title = o_title.or_else(|| contact.title.clone());
        let m_company = o_company.or_else(|| contact.company.clone());
        let m_crm = o_crm.or_else(|| contact.crm_id.clone());
        tx.execute(
            "UPDATE contacts SET email=?2, phone=?3, phone_norm=?4, name=?5, title=?6, company=?7, crm_id=?8, updated_at=?9
             WHERE id=?1",
            params![old_id, m_email, m_phone, m_phone_norm, m_name, m_title, m_company, m_crm, now],
        )?;
        tx.execute(
            "INSERT INTO social_profiles (contact_id, platform, url, username)
             SELECT ?1, sp.platform, sp.url, sp.username
             FROM social_profiles sp
             WHERE sp.contact_id = ?2
               AND NOT EXISTS (
                   SELECT 1 FROM social_profiles p2
                   WHERE p2.contact_id = ?1
                     AND p2.platform = sp.platform
                     AND p2.url = sp.url
                     AND p2.username = sp.username)",
            params![old_id, new_id],
        )?;
        tx.execute(
            "DELETE FROM social_profiles WHERE contact_id = ?1",
            params![new_id],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO tags (contact_id, tag)
             SELECT ?1, tag FROM tags WHERE contact_id = ?2",
            params![old_id, new_id],
        )?;
        tx.execute("DELETE FROM tags WHERE contact_id = ?1", params![new_id])?;
        tx.execute(
            "UPDATE notes SET contact_id = ?1 WHERE contact_id = ?2",
            params![old_id, new_id],
        )?;
        tx.execute("DELETE FROM contacts WHERE id = ?1", params![new_id])?;

        tx.commit()?;
        Ok((old_id, true))
    }

    /// Attach a tag to a contact (no-op if already present).
    pub fn add_tag(&self, contact_id: i64, tag: &str) -> anyhow::Result<()> {
        let tag = tag.trim();
        anyhow::ensure!(!tag.is_empty(), "tag must not be empty");
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO tags (contact_id, tag) VALUES (?1, ?2)",
            params![contact_id, tag],
        )?;
        Ok(())
    }

    /// Delete a contact and all its extras (socials/tags/notes cascade).
    pub fn delete_contact(&self, contact_id: i64) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM contacts WHERE id = ?1", params![contact_id])?;
        Ok(())
    }

    /// Record the remote CRM id after a successful push, so repeated
    /// syncs update instead of duplicating.
    pub fn set_crm_id(&self, contact_id: i64, crm_id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE contacts SET crm_id=?2, updated_at=?3 WHERE id=?1",
            params![contact_id, crm_id, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Attach a note to a contact.
    pub fn add_note(&self, contact_id: i64, note: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO notes (contact_id, note, created_at) VALUES (?1, ?2, ?3)",
            params![contact_id, note, now],
        )?;
        Ok(())
    }

    /// Insert or update a company (keyed by name). Returns the company id.
    pub fn upsert_company(&self, company: &Company) -> anyhow::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO companies (name, website, industry, size, location, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(name) DO UPDATE SET
                 website = excluded.website,
                 industry = excluded.industry,
                 size = excluded.size,
                 location = excluded.location,
                 description = excluded.description",
            params![
                company.name,
                company.website,
                company.industry,
                company.size,
                company.location,
                company.description,
            ],
        )?;
        let id = conn.query_row(
            "SELECT id FROM companies WHERE name = ?1",
            params![company.name],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    // ── Read API ────────────────────────────────────────────────────────

    /// Fetch a single contact by id (with social profiles, tags and notes).
    pub fn get_contact(&self, id: i64) -> Option<Contact> {
        self.load_by_ids(&[id]).into_iter().next()
    }

    /// Find a contact by (case-insensitive, trimmed) email address.
    pub fn find_by_email(&self, email: &str) -> Option<Contact> {
        let email = normalize_email(email);
        if email.is_empty() {
            return None;
        }
        let id = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT id FROM contacts WHERE email = ?1 ORDER BY id LIMIT 1",
                params![email],
                |row| row.get::<_, i64>(0),
            )
            .ok()
        };
        id.and_then(|id| self.get_contact(id))
    }

    /// Find a contact by phone number (compared by digits only).
    ///
    /// Uses the `idx_contacts_phone_norm` index on the stored digits-only
    /// `phone_norm` column instead of normalizing every row in Rust. Rows
    /// written before the column existed (`phone_norm IS NULL`) are handled
    /// by a scan fallback that also writes the normalized value back, so
    /// the backfill happens lazily and idempotently.
    pub fn find_by_phone(&self, phone: &str) -> Option<Contact> {
        let needle = normalize_phone(phone);
        if needle.is_empty() {
            return None;
        }
        let id = {
            let conn = self.conn.lock().unwrap();
            // Fast path: indexed equality on the normalized phone.
            // `ORDER BY id LIMIT 1` keeps the historical "lowest id wins"
            // semantics of the old scan.
            let indexed = conn
                .query_row(
                    "SELECT id FROM contacts WHERE phone_norm = ?1 ORDER BY id LIMIT 1",
                    params![needle],
                    |row| row.get::<_, i64>(0),
                )
                .ok();
            match indexed {
                Some(id) => Some(id),
                None => {
                    // Fallback: legacy rows whose phone_norm has not been
                    // backfilled yet. Normalize in Rust and persist the
                    // result so the next lookup hits the index.
                    let mut stmt = conn
                        .prepare(
                            "SELECT id, phone FROM contacts
                             WHERE phone IS NOT NULL AND phone_norm IS NULL
                             ORDER BY id",
                        )
                        .ok()?;
                    let rows = stmt
                        .query_map([], |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                        })
                        .ok()?;
                    let mut found = None;
                    for (row_id, raw) in rows.flatten() {
                        let norm = normalize_phone(&raw);
                        let _ = conn.execute(
                            "UPDATE contacts SET phone_norm = ?2 WHERE id = ?1",
                            params![row_id, norm],
                        );
                        if found.is_none() && norm == needle {
                            found = Some(row_id);
                        }
                    }
                    found
                }
            }
        };
        id.and_then(|id| self.get_contact(id))
    }

    /// Search contacts by substring over name, email, phone, title, company
    /// and tags (case-insensitive). Most recently added first.
    ///
    /// Issues one row query plus three batched extras queries (socials,
    /// tags, notes with `contact_id IN (...)`) regardless of result size.
    pub fn search(&self, query: &str) -> Vec<Contact> {
        let needle = query.trim();
        if needle.is_empty() {
            return self.list_all(usize::MAX, 0);
        }
        let pattern = format!("%{}%", escape_like(needle));
        let conn = self.conn.lock().unwrap();
        let stmt = conn.prepare(
            &format!(
                "SELECT DISTINCT {CONTACT_COLS} FROM contacts c
                 LEFT JOIN tags t ON t.contact_id = c.id
                 WHERE c.name LIKE ?1 ESCAPE '\\'
                    OR c.email LIKE ?1 ESCAPE '\\'
                    OR c.phone LIKE ?1 ESCAPE '\\'
                    OR c.title LIKE ?1 ESCAPE '\\'
                    OR c.company LIKE ?1 ESCAPE '\\'
                    OR t.tag LIKE ?1 ESCAPE '\\'
                 ORDER BY c.id DESC"
            ),
        );
        let mut stmt = match stmt {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("failed to prepare contact search: {e}");
                return Vec::new();
            }
        };
        let rows = stmt.query_map(params![pattern], contact_row_from_stmt);
        let contacts = match rows {
            Ok(rows) => rows.flatten().collect::<Vec<_>>(),
            Err(e) => {
                tracing::error!("failed to search contacts: {e}");
                return Vec::new();
            }
        };
        attach_extras(&conn, contacts)
    }

    /// List all contacts, most recently added first.
    ///
    /// Issues one row query plus three batched extras queries (socials,
    /// tags, notes with `contact_id IN (...)`) regardless of result size.
    pub fn list_all(&self, limit: usize, offset: usize) -> Vec<Contact> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(&format!(
            "SELECT {CONTACT_COLS} FROM contacts c ORDER BY c.id DESC LIMIT ?1 OFFSET ?2"
        )) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("failed to prepare contact listing: {e}");
                return Vec::new();
            }
        };
        let limit = i64::try_from(limit).unwrap_or(-1);
        let offset = i64::try_from(offset).unwrap_or(0);
        let rows = stmt.query_map(params![limit, offset], contact_row_from_stmt);
        let contacts = match rows {
            Ok(rows) => rows.flatten().collect::<Vec<_>>(),
            Err(e) => {
                tracing::error!("failed to list contacts: {e}");
                return Vec::new();
            }
        };
        attach_extras(&conn, contacts)
    }

    /// Total number of stored contacts.
    pub fn count(&self) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM contacts", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// List all companies ordered by name.
    pub fn list_companies(&self) -> Vec<Company> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT id, name, website, industry, size, location, description FROM companies ORDER BY name",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::error!("failed to prepare company listing: {e}");
                return Vec::new();
            }
        };
        let rows = stmt.query_map([], |row| {
            Ok(Company {
                id: row.get(0)?,
                name: row.get(1)?,
                website: row.get(2)?,
                industry: row.get(3)?,
                size: row.get(4)?,
                location: row.get(5)?,
                description: row.get(6)?,
            })
        });
        match rows {
            Ok(rows) => rows.flatten().collect(),
            Err(e) => {
                tracing::error!("failed to list companies: {e}");
                Vec::new()
            }
        }
    }

    // ── Deduplication ───────────────────────────────────────────────────

    /// Find pairs of contacts that share a normalized email or phone number.
    /// Each pair is ordered by id (older contact first).
    pub fn find_duplicates(&self) -> Vec<(Contact, Contact)> {
        let contacts = self.list_all(usize::MAX, 0);

        let mut pairs: Vec<(i64, i64)> = Vec::new();
        let mut by_email: HashMap<String, Vec<i64>> = HashMap::new();
        let mut by_phone: HashMap<String, Vec<i64>> = HashMap::new();
        for contact in &contacts {
            let Some(id) = contact.id else { continue };
            if let Some(email) = contact.normalized_email() {
                by_email.entry(email).or_default().push(id);
            }
            if let Some(phone) = contact.normalized_phone() {
                by_phone.entry(phone).or_default().push(id);
            }
        }

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

        let by_id: HashMap<i64, &Contact> = contacts
            .iter()
            .filter_map(|c| c.id.map(|id| (id, c)))
            .collect();
        pairs
            .into_iter()
            .filter_map(|(a, b)| {
                let primary: Contact = (*by_id.get(&a)?).clone();
                let duplicate: Contact = (*by_id.get(&b)?).clone();
                Some((primary, duplicate))
            })
            .collect()
    }

    /// Merge `duplicate_id` into `primary_id`: blank fields of the primary
    /// are filled from the duplicate, social profiles / tags / notes are
    /// moved over, and the duplicate contact is deleted.
    pub fn merge_contacts(&self, primary_id: i64, duplicate_id: i64) -> anyhow::Result<()> {
        anyhow::ensure!(
            primary_id != duplicate_id,
            "cannot merge a contact with itself"
        );
        let primary = self
            .get_contact(primary_id)
            .ok_or_else(|| anyhow::anyhow!("contact {primary_id} not found"))?;
        let duplicate = self
            .get_contact(duplicate_id)
            .ok_or_else(|| anyhow::anyhow!("contact {duplicate_id} not found"))?;

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();

        // Fill blanks on the primary contact from the duplicate.
        let email = primary.email.clone().or_else(|| duplicate.email.clone());
        let phone = primary.phone.clone().or_else(|| duplicate.phone.clone());
        let phone_norm = phone.as_deref().map(normalize_phone);
        let name = primary.name.clone().or_else(|| duplicate.name.clone());
        let title = primary.title.clone().or_else(|| duplicate.title.clone());
        let company = primary.company.clone().or_else(|| duplicate.company.clone());
        let crm_id = primary.crm_id.clone().or_else(|| duplicate.crm_id.clone());
        tx.execute(
            "UPDATE contacts SET email=?2, phone=?3, phone_norm=?4, name=?5, title=?6, company=?7, crm_id=?8, updated_at=?9
             WHERE id=?1",
            params![primary_id, email, phone, phone_norm, name, title, company, crm_id, now],
        )?;

        // Move social profiles that the primary does not already have.
        tx.execute(
            "INSERT INTO social_profiles (contact_id, platform, url, username)
             SELECT ?1, sp.platform, sp.url, sp.username
             FROM social_profiles sp
             WHERE sp.contact_id = ?2
               AND NOT EXISTS (
                   SELECT 1 FROM social_profiles p2
                   WHERE p2.contact_id = ?1
                     AND p2.platform = sp.platform
                     AND p2.url = sp.url
                     AND p2.username = sp.username)",
            params![primary_id, duplicate_id],
        )?;
        tx.execute(
            "DELETE FROM social_profiles WHERE contact_id = ?1",
            params![duplicate_id],
        )?;

        // Move tags (deduplicated via UNIQUE(contact_id, tag)).
        tx.execute(
            "INSERT OR IGNORE INTO tags (contact_id, tag)
             SELECT ?1, tag FROM tags WHERE contact_id = ?2",
            params![primary_id, duplicate_id],
        )?;
        tx.execute(
            "DELETE FROM tags WHERE contact_id = ?1",
            params![duplicate_id],
        )?;

        // Move notes.
        tx.execute(
            "UPDATE notes SET contact_id = ?1 WHERE contact_id = ?2",
            params![primary_id, duplicate_id],
        )?;

        // Remove the duplicate contact itself (cascades any leftovers).
        tx.execute("DELETE FROM contacts WHERE id = ?1", params![duplicate_id])?;

        tx.commit()?;
        tracing::info!(primary_id, duplicate_id, "merged duplicate contacts");
        Ok(())
    }

    // ── Internal helpers ────────────────────────────────────────────────

    /// Load contacts by id with social profiles, tags and notes.
    ///
    /// Batched: one `id IN (...)` row query plus one extras query per kind
    /// (`social_profiles` / `tags` / `notes`, each `contact_id IN (...)`),
    /// no matter how many ids are requested. Results are returned in the
    /// order of `ids`.
    fn load_by_ids(&self, ids: &[i64]) -> Vec<Contact> {
        if ids.is_empty() {
            return Vec::new();
        }
        let conn = self.conn.lock().unwrap();
        let mut contacts = Vec::with_capacity(ids.len());
        for batch in ids.chunks(ID_BATCH_SIZE) {
            let sql = format!(
                "SELECT {CONTACT_COLS} FROM contacts c WHERE c.id IN ({})",
                sql_placeholders(batch.len()),
            );
            let mut stmt = match conn.prepare(&sql) {
                Ok(stmt) => stmt,
                Err(e) => {
                    tracing::error!("failed to prepare contact fetch: {e}");
                    continue;
                }
            };
            let params: Vec<&dyn rusqlite::ToSql> =
                batch.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(params.as_slice(), contact_row_from_stmt);
            match rows {
                Ok(rows) => contacts.extend(rows.flatten()),
                Err(e) => tracing::error!("failed to fetch contacts: {e}"),
            }
        }
        // `IN (...)` does not preserve order; restore the requested order.
        let position: HashMap<i64, usize> =
            ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        contacts.sort_by_key(|c| {
            c.id.and_then(|id| position.get(&id).copied())
                .unwrap_or(usize::MAX)
        });
        attach_extras(&conn, contacts)
    }
}

/// Column list shared by all contact row queries; the `contacts` table is
/// always aliased as `c` so the columns stay unambiguous when joined.
const CONTACT_COLS: &str = "c.id, c.email, c.phone, c.name, c.title, c.company, \
     c.created_at, c.updated_at, c.source, c.crm_id";

/// Maximum number of ids per `IN (...)` clause (stays well below SQLite's
/// minimum bind-parameter limit of 999).
const ID_BATCH_SIZE: usize = 500;

/// Build a `?, ?, ...` placeholder list for `n` bind parameters.
fn sql_placeholders(n: usize) -> String {
    vec!["?"; n].join(", ")
}

/// Attach social profiles, tags and notes to already-loaded contact rows
/// using three `contact_id IN (...)` queries per batch instead of three
/// queries per contact. Within each contact, extras keep insertion order
/// (the queries are ordered by row id). Errors degrade gracefully to
/// contacts without extras, mirroring the previous per-contact behaviour.
fn attach_extras(conn: &Connection, mut contacts: Vec<Contact>) -> Vec<Contact> {
    if contacts.is_empty() {
        return contacts;
    }
    let ids: Vec<i64> = contacts.iter().filter_map(|c| c.id).collect();
    let position: HashMap<i64, usize> = contacts
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.id.map(|id| (id, i)))
        .collect();

    for batch in ids.chunks(ID_BATCH_SIZE) {
        let in_clause = sql_placeholders(batch.len());
        let params: Vec<&dyn rusqlite::ToSql> =
            batch.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

        if let Ok(mut stmt) = conn.prepare(&format!(
            "SELECT contact_id, platform, url, username FROM social_profiles
             WHERE contact_id IN ({in_clause}) ORDER BY id"
        )) {
            let rows = stmt.query_map(params.as_slice(), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    SocialProfile {
                        id: None,
                        platform: row.get(1)?,
                        url: row.get(2)?,
                        username: row.get(3)?,
                    },
                ))
            });
            if let Ok(rows) = rows {
                for (contact_id, profile) in rows.flatten() {
                    if let Some(&i) = position.get(&contact_id) {
                        contacts[i].social_profiles.push(profile);
                    }
                }
            }
        }

        if let Ok(mut stmt) = conn.prepare(&format!(
            "SELECT contact_id, tag FROM tags WHERE contact_id IN ({in_clause}) ORDER BY id"
        )) {
            let rows = stmt.query_map(params.as_slice(), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            });
            if let Ok(rows) = rows {
                for (contact_id, tag) in rows.flatten() {
                    if let Some(&i) = position.get(&contact_id) {
                        contacts[i].tags.push(tag);
                    }
                }
            }
        }

        if let Ok(mut stmt) = conn.prepare(&format!(
            "SELECT contact_id, note FROM notes WHERE contact_id IN ({in_clause}) ORDER BY id"
        )) {
            let rows = stmt.query_map(params.as_slice(), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            });
            if let Ok(rows) = rows {
                for (contact_id, note) in rows.flatten() {
                    if let Some(&i) = position.get(&contact_id) {
                        contacts[i].notes.push(note);
                    }
                }
            }
        }
    }

    contacts
}

fn contact_row_from_stmt(row: &rusqlite::Row<'_>) -> rusqlite::Result<Contact> {
    let created_at: String = row.get(6)?;
    let updated_at: String = row.get(7)?;
    Ok(Contact {
        id: row.get(0)?,
        email: row.get(1)?,
        phone: row.get(2)?,
        name: row.get(3)?,
        title: row.get(4)?,
        company: row.get(5)?,
        social_profiles: Vec::new(),
        tags: Vec::new(),
        notes: Vec::new(),
        source: row.get(8)?,
        crm_id: row.get(9)?,
        verification: pr_core::Verification::Unverified,
        created_at: parse_rfc3339(&created_at),
        updated_at: parse_rfc3339(&updated_at),
    })
}

fn parse_rfc3339(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

/// Escape SQL LIKE wildcards so user input matches literally.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contact(name: &str, email: &str) -> Contact {
        let mut c = Contact::new().with_source("unit-test");
        c.name = Some(name.to_string());
        c.email = Some(email.to_string());
        c
    }

    fn full_contact() -> Contact {
        let mut c = sample_contact("Jane Doe", "jane@example.com");
        c.phone = Some("+1 (555) 010-0100".into());
        c.title = Some("CTO".into());
        c.company = Some("Acme".into());
        c.social_profiles
            .push(SocialProfile::new("linkedin", "https://linkedin.com/in/jdoe", "jdoe"));
        c.tags = vec!["lead".into(), "vip".into()];
        c.notes = vec!["Met at conference".into()];
        c
    }

    #[test]
    fn test_open_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("contacts.db");
        let db = ContactDb::open(&path).unwrap();
        assert!(path.exists());
        assert_eq!(db.count(), 0);
    }

    #[test]
    fn test_open_migrates_legacy_schema_without_phone_norm() {
        // Databases created before the phone_norm/crm_id columns existed
        // must open cleanly: the migration has to run BEFORE the index on
        // phone_norm is created (regression for "no such column: phone_norm").
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("legacy.db");
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE contacts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    email TEXT,
                    phone TEXT,
                    name TEXT,
                    title TEXT,
                    company TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    source TEXT NOT NULL DEFAULT 'unknown'
                 );
                 CREATE TABLE social_profiles (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                    platform TEXT NOT NULL,
                    url TEXT NOT NULL DEFAULT '',
                    username TEXT NOT NULL DEFAULT ''
                 );
                 CREATE TABLE companies (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    website TEXT, industry TEXT, size TEXT, location TEXT, description TEXT
                 );
                 CREATE TABLE tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                    tag TEXT NOT NULL,
                    UNIQUE (contact_id, tag)
                 );
                 CREATE TABLE notes (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
                    note TEXT NOT NULL,
                    created_at TEXT NOT NULL
                 );
                 INSERT INTO contacts (email, name, created_at, updated_at, source)
                 VALUES ('legacy@example.com', 'Legacy Row', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z', 'legacy');",
            )
            .unwrap();
        }

        let db = ContactDb::open(&path).expect("legacy DB must open after migration");
        assert_eq!(db.count(), 1, "legacy row must survive the migration");

        let mut c = sample_contact("New Person", "new@example.com");
        c.phone = Some("+49 30 123456".into());
        db.add_contact(&c).unwrap();
        assert!(db.find_by_phone("+4930123456").is_some());
    }

    #[test]
    fn test_add_and_get_roundtrip() {
        let db = ContactDb::in_memory().unwrap();
        let id = db.add_contact(&full_contact()).unwrap();

        let loaded = db.get_contact(id).expect("contact exists");
        assert_eq!(loaded.id, Some(id));
        assert_eq!(loaded.name.as_deref(), Some("Jane Doe"));
        assert_eq!(loaded.email.as_deref(), Some("jane@example.com"));
        assert_eq!(loaded.phone.as_deref(), Some("+1 (555) 010-0100"));
        assert_eq!(loaded.title.as_deref(), Some("CTO"));
        assert_eq!(loaded.company.as_deref(), Some("Acme"));
        assert_eq!(loaded.source, "unit-test");
        assert_eq!(loaded.social_profiles.len(), 1);
        assert_eq!(loaded.social_profiles[0].platform, "linkedin");
        assert_eq!(loaded.tags, vec!["lead", "vip"]);
        assert_eq!(loaded.notes, vec!["Met at conference"]);
        assert!(db.get_contact(9999).is_none());
    }

    #[test]
    fn test_find_by_email_is_case_insensitive_and_normalized() {
        let db = ContactDb::in_memory().unwrap();
        db.add_contact(&sample_contact("Jane", "Jane.Doe@Example.COM")).unwrap();

        let found = db.find_by_email("jane.doe@example.com").expect("found");
        assert_eq!(found.name.as_deref(), Some("Jane"));
        assert!(db.find_by_email("jane.doe@EXAMPLE.com  ").is_some());
        assert!(db.find_by_email("other@example.com").is_none());
        assert!(db.find_by_email("   ").is_none());
    }

    #[test]
    fn test_find_by_phone_ignores_formatting() {
        let db = ContactDb::in_memory().unwrap();
        let mut c = Contact::new();
        c.phone = Some("+1 (555) 010-0100".into());
        db.add_contact(&c).unwrap();

        // Same digits in different formatting match.
        assert!(db.find_by_phone("+1 555 010 0100").is_some());
        assert!(db.find_by_phone("1-555-010-0100").is_some());
        assert!(db.find_by_phone("15550100100").is_some());
        // Different digit strings do not match (exact digit comparison).
        assert!(db.find_by_phone("555-010-0100").is_none());
        assert!(db.find_by_phone("+44 20 7946 0958").is_none());
    }

    #[test]
    fn test_search_matches_fields_and_tags() {
        let db = ContactDb::in_memory().unwrap();
        let mut a = sample_contact("Alice Johnson", "alice@acme.com");
        a.tags = vec!["decision-maker".into()];
        let mut b = sample_contact("Bob Stone", "bob@globex.com");
        b.company = Some("Globex".into());
        let mut c = sample_contact("Carol Smith", "carol@initech.com");
        c.title = Some("VP Engineering".into());
        db.add_contact(&a).unwrap();
        db.add_contact(&b).unwrap();
        db.add_contact(&c).unwrap();

        assert_eq!(db.search("alice").len(), 1);
        assert_eq!(db.search("globex").len(), 1);
        assert_eq!(db.search("vp engineering").len(), 1);
        assert_eq!(db.search("decision-maker").len(), 1);
        assert_eq!(db.search("@initech").len(), 1);
        assert!(db.search("nobody").is_empty());
        // Empty query lists everything.
        assert_eq!(db.search("  ").len(), 3);
        // SQL LIKE wildcards are treated literally.
        assert!(db.search("%").is_empty());
    }

    #[test]
    fn test_list_all_limit_offset_newest_first() {
        let db = ContactDb::in_memory().unwrap();
        for i in 0..5 {
            db.add_contact(&sample_contact(&format!("C{i}"), &format!("c{i}@x.com")))
                .unwrap();
        }
        let all = db.list_all(100, 0);
        assert_eq!(all.len(), 5);
        assert_eq!(all[0].name.as_deref(), Some("C4")); // newest first

        let page = db.list_all(2, 1);
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].name.as_deref(), Some("C3"));
        assert_eq!(page[1].name.as_deref(), Some("C2"));

        assert_eq!(db.count(), 5);
    }

    #[test]
    fn test_add_tag_deduplicates_and_rejects_missing_contact() {
        let db = ContactDb::in_memory().unwrap();
        let id = db.add_contact(&sample_contact("Jane", "j@x.com")).unwrap();

        db.add_tag(id, "hot").unwrap();
        db.add_tag(id, "hot").unwrap(); // duplicate ignored
        db.add_tag(id, "new").unwrap();
        assert_eq!(db.get_contact(id).unwrap().tags.len(), 2);

        assert!(db.add_tag(id, "   ").is_err()); // empty tag
        assert!(db.add_tag(4242, "x").is_err()); // FK violation
    }

    #[test]
    fn test_add_note() {
        let db = ContactDb::in_memory().unwrap();
        let id = db.add_contact(&sample_contact("Jane", "j@x.com")).unwrap();

        db.add_note(id, "first note").unwrap();
        db.add_note(id, "second note").unwrap();
        let loaded = db.get_contact(id).unwrap();
        assert_eq!(loaded.notes, vec!["first note", "second note"]);

        assert!(db.add_note(4242, "x").is_err()); // FK violation
    }

    #[test]
    fn test_find_duplicates_by_email_and_phone() {
        let db = ContactDb::in_memory().unwrap();
        // Two contacts sharing an email (different case/format).
        db.add_contact(&sample_contact("Jane A", "jane@x.com")).unwrap();
        db.add_contact(&sample_contact("Jane B", "JANE@x.com ")).unwrap();
        // Two contacts sharing a phone number.
        let mut p1 = sample_contact("Phone One", "p1@x.com");
        p1.phone = Some("+1 555 0100".into());
        let mut p2 = sample_contact("Phone Two", "p2@x.com");
        p2.phone = Some("1-555-0100".into());
        db.add_contact(&p1).unwrap();
        db.add_contact(&p2).unwrap();
        // One unique contact.
        db.add_contact(&sample_contact("Unique", "unique@x.com")).unwrap();

        let dupes = db.find_duplicates();
        assert_eq!(dupes.len(), 2);
        let labels: Vec<(String, String)> = dupes
            .iter()
            .map(|(a, b)| (a.name.clone().unwrap(), b.name.clone().unwrap()))
            .collect();
        assert!(labels.contains(&("Jane A".to_string(), "Jane B".to_string())));
        assert!(labels.contains(&("Phone One".to_string(), "Phone Two".to_string())));
    }

    #[test]
    fn test_merge_contacts_moves_everything_and_fills_blanks() {
        let db = ContactDb::in_memory().unwrap();

        let mut primary = sample_contact("Jane", "jane@x.com");
        primary.tags = vec!["lead".into()];
        primary.social_profiles
            .push(SocialProfile::new("linkedin", "https://linkedin.com/in/jane", "jane"));
        primary.notes = vec!["primary note".into()];
        let primary_id = db.add_contact(&primary).unwrap();

        let mut dup = Contact::new().with_source("other");
        dup.email = Some("jane@x.com".into());
        dup.phone = Some("+1 555 0100".into());
        dup.title = Some("CTO".into());
        dup.company = Some("Acme".into());
        dup.social_profiles
            .push(SocialProfile::new("linkedin", "https://linkedin.com/in/jane", "jane")); // duplicate
        dup.social_profiles
            .push(SocialProfile::new("twitter", "https://twitter.com/jane", "jane"));
        dup.tags = vec!["lead".into(), "vip".into()];
        dup.notes = vec!["dup note".into()];
        let dup_id = db.add_contact(&dup).unwrap();

        db.merge_contacts(primary_id, dup_id).unwrap();

        // Duplicate is gone.
        assert!(db.get_contact(dup_id).is_none());
        assert_eq!(db.count(), 1);

        let merged = db.get_contact(primary_id).unwrap();
        assert_eq!(merged.name.as_deref(), Some("Jane")); // kept from primary
        assert_eq!(merged.phone.as_deref(), Some("+1 555 0100")); // filled from dup
        assert_eq!(merged.title.as_deref(), Some("CTO"));
        assert_eq!(merged.company.as_deref(), Some("Acme"));
        assert_eq!(merged.social_profiles.len(), 2); // duplicate profile not duplicated
        assert_eq!(merged.tags.len(), 2); // lead deduplicated
        assert_eq!(merged.notes.len(), 2);
    }

    #[test]
    fn test_merge_contacts_error_cases() {
        let db = ContactDb::in_memory().unwrap();
        let id = db.add_contact(&sample_contact("Jane", "j@x.com")).unwrap();

        assert!(db.merge_contacts(id, id).is_err());
        assert!(db.merge_contacts(id, 999).is_err());
        assert!(db.merge_contacts(999, id).is_err());
    }

    #[test]
    fn test_companies_upsert_and_list() {
        let db = ContactDb::in_memory().unwrap();
        let id1 = db
            .upsert_company(&Company {
                id: None,
                name: "Acme".into(),
                website: Some("https://acme.com".into()),
                industry: Some("Manufacturing".into()),
                size: Some("51-200".into()),
                location: Some("Springfield".into()),
                description: None,
            })
            .unwrap();

        // Upsert with the same name updates in place (same id).
        let id2 = db
            .upsert_company(&Company {
                id: None,
                name: "Acme".into(),
                website: Some("https://www.acme.com".into()),
                industry: None,
                size: None,
                location: None,
                description: Some("Anvil maker".into()),
            })
            .unwrap();
        assert_eq!(id1, id2);

        let companies = db.list_companies();
        assert_eq!(companies.len(), 1);
        assert_eq!(companies[0].website.as_deref(), Some("https://www.acme.com"));
        assert_eq!(companies[0].description.as_deref(), Some("Anvil maker"));
    }

    /// `list_all` and `search` must return exactly the same full `Contact`
    /// structures (socials, tags and notes included) as the per-id fetch —
    /// the batched `IN (...)` extras loading must not change shapes or
    /// ordering semantics.
    #[test]
    fn test_list_all_and_search_return_full_extras_structures() {
        let db = ContactDb::in_memory().unwrap();

        let mut a = sample_contact("Alice Johnson", "alice@acme.com");
        a.phone = Some("+1 (555) 010-0100".into());
        a.title = Some("CTO".into());
        a.company = Some("Acme".into());
        a.social_profiles = vec![
            SocialProfile::new("linkedin", "https://linkedin.com/in/alice", "alice"),
            SocialProfile::new("twitter", "https://twitter.com/alice", "alicej"),
        ];
        a.tags = vec!["lead".into(), "decision-maker".into()];
        a.notes = vec!["Met at conference".into(), "Follow up in Q3".into()];

        let mut b = sample_contact("Bob Stone", "bob@globex.com");
        b.social_profiles =
            vec![SocialProfile::new("telegram", "https://t.me/bobstone", "bobstone")];
        b.tags = vec!["vip".into()];
        b.notes = vec!["Referred by Alice".into()];

        // Contact without any extras at all.
        let c = sample_contact("Carol Smith", "carol@initech.com");

        let id_a = db.add_contact(&a).unwrap();
        let id_b = db.add_contact(&b).unwrap();
        let id_c = db.add_contact(&c).unwrap();

        // list_all: newest first, identical structures to per-id loading.
        let listed = db.list_all(100, 0);
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].id, Some(id_c));
        assert_eq!(listed[1].id, Some(id_b));
        assert_eq!(listed[2].id, Some(id_a));
        for contact in &listed {
            assert_eq!(
                contact,
                &db.get_contact(contact.id.unwrap()).unwrap(),
                "list_all structure must match get_contact for id {:?}",
                contact.id
            );
        }

        // Extras survived the batched IN(...) loading, with insertion order.
        let loaded_a = listed[2].clone();
        assert_eq!(loaded_a.social_profiles, a.social_profiles);
        assert_eq!(loaded_a.tags, a.tags);
        assert_eq!(loaded_a.notes, a.notes);
        assert_eq!(loaded_a.phone.as_deref(), Some("+1 (555) 010-0100"));
        let loaded_b = listed[1].clone();
        assert_eq!(loaded_b.social_profiles, b.social_profiles);
        assert_eq!(loaded_b.tags, b.tags);
        assert_eq!(loaded_b.notes, b.notes);
        assert!(listed[0].social_profiles.is_empty());
        assert!(listed[0].tags.is_empty());
        assert!(listed[0].notes.is_empty());

        // search: each hit is structurally identical to the list_all entry.
        for needle in ["alice", "globex", "@initech", "decision-maker", "vip"] {
            let hits = db.search(needle);
            assert_eq!(hits.len(), 1, "search({needle:?}) should match one contact");
            let expected = listed
                .iter()
                .find(|c| c.id == hits[0].id)
                .expect("search hit belongs to the listed set");
            assert_eq!(&hits[0], expected, "search({needle:?}) structure mismatch");
        }

        // Empty query goes through list_all and returns everything.
        assert_eq!(db.search("   "), listed);
    }

    /// `find_by_phone` matches on digits only, regardless of formatting.
    #[test]
    fn test_find_by_phone_matches_normalized_variants() {
        let db = ContactDb::in_memory().unwrap();
        let mut c = Contact::new().with_source("unit-test");
        c.name = Some("Ivan Petrov".into());
        c.phone = Some("+7 (916) 000-00-00".into());
        let id = db.add_contact(&c).unwrap();

        for variant in [
            "+79160000000",
            "79160000000",
            "+7 916 000 00 00",
            "8-916-000-00-00-nope", // digits differ -> must not match
        ] {
            let found = db.find_by_phone(variant);
            let should_match = variant != "8-916-000-00-00-nope";
            assert_eq!(
                found.as_ref().and_then(|f| f.id),
                should_match.then_some(id),
                "variant {variant:?}"
            );
        }
        assert_eq!(
            db.find_by_phone("+79160000000").unwrap().name.as_deref(),
            Some("Ivan Petrov")
        );
    }

    /// Rows written before the `phone_norm` column existed (NULL) are found
    /// via the scan fallback, which backfills the column lazily and
    /// idempotently so subsequent lookups use the index.
    #[test]
    fn test_find_by_phone_lazy_backfill_of_legacy_rows() {
        let db = ContactDb::in_memory().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Simulate a legacy row: phone present, phone_norm left NULL.
        let legacy_id: i64 = {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO contacts (phone, created_at, updated_at, source)
                 VALUES (?1, ?2, ?2, 'legacy')",
                params!["+7 (916) 000-00-00", now],
            )
            .unwrap();
            conn.last_insert_rowid()
        };
        // A modern row added through the API for comparison.
        let mut modern = Contact::new();
        modern.phone = Some("+1 555 0100".into());
        let modern_id = db.add_contact(&modern).unwrap();

        let is_null = |conn: &Connection, id: i64| -> bool {
            conn.query_row(
                "SELECT phone_norm IS NULL FROM contacts WHERE id = ?1",
                params![id],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
        };
        {
            let conn = db.conn.lock().unwrap();
            assert!(is_null(&conn, legacy_id), "legacy row starts un-backfilled");
            assert!(!is_null(&conn, modern_id), "new rows set phone_norm on insert");
        }

        // The indexed lookup misses; the scan fallback finds and backfills.
        let found = db
            .find_by_phone("+79160000000")
            .expect("legacy row found via fallback scan");
        assert_eq!(found.id, Some(legacy_id));

        {
            let conn = db.conn.lock().unwrap();
            let norm: Option<String> = conn
                .query_row(
                    "SELECT phone_norm FROM contacts WHERE id = ?1",
                    params![legacy_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(norm.as_deref(), Some("79160000000"));
            let remaining: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM contacts WHERE phone IS NOT NULL AND phone_norm IS NULL",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(remaining, 0, "backfill is complete after one lookup");
        }

        // Second lookup resolves via the index and still returns the row.
        assert_eq!(
            db.find_by_phone("7-916-000-00-00").and_then(|c| c.id),
            Some(legacy_id)
        );
    }

    /// `merge_contacts` must maintain `phone_norm` when it fills the phone
    /// from the duplicate.
    #[test]
    fn test_merge_contacts_updates_phone_norm() {
        let db = ContactDb::in_memory().unwrap();
        let primary_id = db.add_contact(&sample_contact("Jane", "jane@x.com")).unwrap();
        let mut dup = Contact::new();
        dup.email = Some("jane@x.com".into());
        dup.phone = Some("+7 (916) 000-00-00".into());
        let dup_id = db.add_contact(&dup).unwrap();

        db.merge_contacts(primary_id, dup_id).unwrap();

        // The merged contact is reachable through the indexed phone lookup.
        assert_eq!(
            db.find_by_phone("+79160000000").and_then(|c| c.id),
            Some(primary_id)
        );
    }
}
