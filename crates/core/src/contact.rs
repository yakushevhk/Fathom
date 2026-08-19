//! Contact domain types for the lead-generation / OSINT pipeline.
//!
//! These types are shared by the contact database (`pr-persistence`), the
//! CRM synchronizers and the contact exporters.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A social media profile attached to a contact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialProfile {
    /// Database row id (`None` until persisted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Platform name, e.g. `linkedin`, `twitter`, `telegram`.
    pub platform: String,
    /// Full profile URL.
    #[serde(default)]
    pub url: String,
    /// Username / handle on the platform.
    #[serde(default)]
    pub username: String,
}

impl SocialProfile {
    pub fn new(platform: impl Into<String>, url: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            id: None,
            platform: platform.into(),
            url: url.into(),
            username: username.into(),
        }
    }
}

/// A company record (the `companies` table).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Company {
    /// Database row id (`None` until persisted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub industry: Option<String>,
    /// Free-form size descriptor, e.g. `"51-200"`.
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A contact collected during research.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contact {
    /// Database row id (`None` until persisted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub name: Option<String>,
    /// Job title / position.
    pub title: Option<String>,
    /// Company name.
    pub company: Option<String>,
    #[serde(default)]
    pub social_profiles: Vec<SocialProfile>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    /// Where this contact came from (e.g. a search backend or tool name).
    pub source: String,
    /// Remote CRM id once the contact has been pushed (prevents duplicate pushes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crm_id: Option<String>,
    /// Verification status derived from the receipt ledger. Default
    /// `Unverified` — a contact is only tagged `Verified` when a green receipt
    /// exists for the relevant check kind.
    #[serde(default)]
    pub verification: Verification,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Level of verification evidence for a contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    /// No check ever recorded; may still be a perfectly good lead, but it has
    /// not been confirmed.
    #[default]
    Unverified,
    /// Some checks passed but not all (e.g. syntax + domain OK, mailbox not
    /// probed). Better than nothing, weaker than Verified.
    Partial,
    /// The relevant check(s) recorded green receipts (e.g. SMTP accepted).
    Verified,
}

impl Default for Contact {
    fn default() -> Self {
        Self::new()
    }
}

impl Contact {
    /// An empty contact with `source = "unknown"` and timestamps set to now.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: None,
            email: None,
            phone: None,
            name: None,
            title: None,
            company: None,
            social_profiles: Vec::new(),
            tags: Vec::new(),
            notes: Vec::new(),
            source: "unknown".to_string(),
            crm_id: None,
            verification: Verification::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Builder-style source setter.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// A human-readable label: name, else email, else phone, else the id.
    pub fn display_label(&self) -> String {
        if let Some(name) = self.name.as_deref().filter(|s| !s.trim().is_empty()) {
            return name.to_string();
        }
        if let Some(email) = self.email.as_deref().filter(|s| !s.trim().is_empty()) {
            return email.to_string();
        }
        if let Some(phone) = self.phone.as_deref().filter(|s| !s.trim().is_empty()) {
            return phone.to_string();
        }
        match self.id {
            Some(id) => format!("contact #{id}"),
            None => "unnamed contact".to_string(),
        }
    }

    /// Normalized (trimmed, lower-cased) email for comparisons.
    pub fn normalized_email(&self) -> Option<String> {
        self.email
            .as_deref()
            .map(normalize_email)
            .filter(|e| !e.is_empty())
    }

    /// Normalized (digits-only) phone for comparisons.
    pub fn normalized_phone(&self) -> Option<String> {
        self.phone
            .as_deref()
            .map(normalize_phone)
            .filter(|p| !p.is_empty())
    }
}

/// Normalize an email address for deduplication: trim + lower-case.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Normalize a phone number for deduplication: keep ASCII digits only.
pub fn normalize_phone(phone: &str) -> String {
    phone.chars().filter(|c| c.is_ascii_digit()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contact_new_defaults() {
        let contact = Contact::new();
        assert_eq!(contact.source, "unknown");
        assert!(contact.id.is_none());
        assert!(contact.email.is_none());
        assert!(contact.social_profiles.is_empty());
        assert!(contact.tags.is_empty());
        assert!(contact.notes.is_empty());
    }

    #[test]
    fn test_display_label_fallback_chain() {
        let mut c = Contact::new();
        assert_eq!(c.display_label(), "unnamed contact");

        c.id = Some(7);
        assert_eq!(c.display_label(), "contact #7");

        c.phone = Some("+1 555 0100".into());
        assert_eq!(c.display_label(), "+1 555 0100");

        c.email = Some("jane@example.com".into());
        assert_eq!(c.display_label(), "jane@example.com");

        c.name = Some("Jane Doe".into());
        assert_eq!(c.display_label(), "Jane Doe");
    }

    #[test]
    fn test_normalize_email_and_phone() {
        assert_eq!(normalize_email("  Jane.Doe@Example.COM "), "jane.doe@example.com");
        assert_eq!(normalize_phone("+1 (555) 010-0100"), "15550100100");
        assert_eq!(normalize_phone("no digits"), "");

        let mut c = Contact::new();
        assert!(c.normalized_email().is_none());
        c.email = Some(" A@B.c ".into());
        c.phone = Some("+7 999 123-45-67".into());
        assert_eq!(c.normalized_email().as_deref(), Some("a@b.c"));
        assert_eq!(c.normalized_phone().as_deref(), Some("79991234567"));
    }

    #[test]
    fn test_contact_serde_roundtrip() {
        let mut contact = Contact::new().with_source("linkedin");
        contact.email = Some("j@e.com".into());
        contact.social_profiles.push(SocialProfile::new(
            "linkedin",
            "https://linkedin.com/in/jdoe",
            "jdoe",
        ));
        contact.tags.push("lead".into());
        contact.notes.push("Met at conference".into());

        let json = serde_json::to_string(&contact).unwrap();
        let back: Contact = serde_json::from_str(&json).unwrap();
        assert_eq!(back, contact);
    }
}
