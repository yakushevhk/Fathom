//! Corporate website parser (lead source #3).
//!
//! Fetches a company's website and extracts structured company data:
//! name, description, contacts (emails/phones), social profiles and the
//! team page with team members. Company metadata is pulled from schema.org
//! JSON-LD, OpenGraph/meta tags and visible page text.

use async_trait::async_trait;
use futures::FutureExt;
use pr_core::{ToolOutput, ToolSchema};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

use crate::registry::{Tool, ToolContext};

const REQUEST_TIMEOUT_SECS: u64 = 20;
const USER_AGENT: &str = "Mozilla/5.0 (compatible; ParallelResearch/0.1)";
/// Maximum bytes of HTML kept in memory per fetched page.
const MAX_PAGE_BYTES: usize = 2_000_000;

/// Contacts extracted from a website.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedContacts {
    pub emails: Vec<String>,
    pub phones: Vec<String>,
}

/// A social profile link found on a website.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialProfile {
    pub platform: String,
    pub url: String,
}

/// A person listed on the team page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub name: String,
    pub role: Option<String>,
}

/// Structured data extracted from a corporate website.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorporateData {
    pub company_name: String,
    pub description: Option<String>,
    pub industry: Option<String>,
    pub size: Option<String>,
    pub headquarters: Option<String>,
    pub website: String,
    pub contacts: ExtractedContacts,
    pub team_page_url: Option<String>,
    pub team: Vec<TeamMember>,
    pub social_profiles: Vec<SocialProfile>,
}

/// Corporate website analysis engine.
pub struct CorporateParser {
    http: reqwest::Client,
}

impl Default for CorporateParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CorporateParser {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Parse a corporate website: fetch the homepage, extract company info
    /// and contacts, locate and parse the team page.
    pub async fn parse_website(&self, url: &str) -> CorporateData {
        let base = normalize_base_url(url);
        let mut data = CorporateData {
            company_name: String::new(),
            description: None,
            industry: None,
            size: None,
            headquarters: None,
            website: base.clone(),
            contacts: ExtractedContacts::default(),
            team_page_url: None,
            team: Vec::new(),
            social_profiles: Vec::new(),
        };

        let Some(home_html) = self.fetch_page(&base).await else {
            tracing::warn!("Corporate parser: failed to fetch {base}");
            return data;
        };

        // Parse the homepage once and pull everything the later steps need
        // out of it up front. `scraper::Html` is not `Send`, so the document
        // must be dropped before the next network await.
        let (home_text, contact_url, team_link_url, about_url) = {
            let home_doc = scraper::Html::parse_document(&home_html);
            let home_jsonld = jsonld_blocks(&home_doc);
            let home_text = html_visible_text(&home_doc);

            data.company_name = extract_company_name(&home_doc, &home_jsonld, &base);
            data.description = extract_meta_description(&home_doc, &home_jsonld);
            data.industry = extract_industry(&home_jsonld, &home_text);
            data.social_profiles = extract_social_links(&home_doc);

            let contact_url = find_linked_page(&home_doc, &base, CONTACT_PAGE_HINTS);
            let team_link_url = find_linked_page(&home_doc, &base, TEAM_PAGE_HINTS);
            let about_url = find_linked_page(&home_doc, &base, ABOUT_PAGE_HINTS);
            (home_text, contact_url, team_link_url, about_url)
        };

        data.size = extract_company_size(&home_text);
        data.headquarters = extract_headquarters(&home_text);
        data.contacts.emails = extract_emails(&home_html);
        data.contacts.phones = extract_phones(&home_text);

        // Enrich contacts from the contact/about pages when they exist.
        if let Some(contact_url) = contact_url {
            if let Some(contact_html) = self.fetch_page(&contact_url).await {
                let contact_text = {
                    let contact_doc = scraper::Html::parse_document(&contact_html);
                    html_visible_text(&contact_doc)
                };
                merge_contacts(&mut data.contacts, &contact_html, &contact_text);
            }
        }

        // Find and parse the team page.
        data.team_page_url = self.find_team_page(&base, team_link_url).await;
        if let Some(ref team_url) = data.team_page_url {
            if let Some(team_html) = self.fetch_page(team_url).await {
                data.team = {
                    let team_doc = scraper::Html::parse_document(&team_html);
                    parse_team_members(&team_doc)
                };
                if data.team.is_empty() {
                    // Some sites list the team on the about page instead.
                    if let Some(about_url) = about_url {
                        if let Some(about_html) = self.fetch_page(&about_url).await {
                            data.team = {
                                let about_doc = scraper::Html::parse_document(&about_html);
                                parse_team_members(&about_doc)
                            };
                        }
                    }
                }
            }
        }

        data
    }

    /// Locate the team page. `team_link` is the result of scanning homepage
    /// links for team-like paths; when no link exists, common team page paths
    /// are probed concurrently (first success wins).
    pub async fn find_team_page(&self, base_url: &str, team_link: Option<String>) -> Option<String> {
        if let Some(url) = team_link {
            return Some(url);
        }
        let base = normalize_base_url(base_url);
        let mut remaining: Vec<_> = TEAM_PATHS
            .iter()
            .map(|path| {
                let candidate = join_url(&base, path);
                async move {
                    let exists = self.page_exists(&candidate).await;
                    (candidate, exists)
                }
                .boxed()
            })
            .collect();
        while !remaining.is_empty() {
            let ((candidate, exists), _, rest) = futures::future::select_all(remaining).await;
            if exists {
                return Some(candidate);
            }
            remaining = rest;
        }
        None
    }

    async fn fetch_page(&self, url: &str) -> Option<String> {
        // SSRF guard (fleet round 2): never fetch internal addresses.
        if crate::guard::ensure_safe_url(url).await.is_err() {
            return None;
        }
        let resp = self
            .http
            .get(url)
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();
        if !content_type.is_empty() && !content_type.contains("html") && !content_type.contains("text") {
            return None;
        }
        let bytes = resp.bytes().await.ok()?;
        if bytes.len() > MAX_PAGE_BYTES {
            return None;
        }
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Lightweight existence check used when probing team page paths.
    async fn page_exists(&self, url: &str) -> bool {
        // SSRF guard (fleet round 2).
        if crate::guard::ensure_safe_url(url).await.is_err() {
            return false;
        }
        // Use GET (not HEAD): many servers reject HEAD or serve different routes.
        match self
            .http
            .get(url)
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}

// ─── Page hints ───

/// Path substrings that indicate a team page link.
const TEAM_PAGE_HINTS: &[&str] = &[
    "/team",
    "our-team",
    "ourteam",
    "/people",
    "/staff",
    "/leadership",
    "/management",
    "/about-us/team",
    "/company/team",
    "команда",
];

/// Path substrings that indicate an about page link.
const ABOUT_PAGE_HINTS: &[&str] = &[
    "/about",
    "about-us",
    "aboutus",
    "/company",
    "/who-we-are",
    "о-нас",
    "о-компании",
];

/// Path substrings that indicate a contacts page link.
const CONTACT_PAGE_HINTS: &[&str] = &[
    "/contact",
    "contact-us",
    "contactus",
    "/contacts",
    "/kontakty",
    "/get-in-touch",
    "контакты",
];

/// Common team page paths probed when no link is found on the homepage.
const TEAM_PATHS: &[&str] = &[
    "/team",
    "/about/team",
    "/our-team",
    "/people",
    "/staff",
    "/about-us/team",
    "/company/team",
    "/leadership",
];

// ─── Cached selectors (parsed once, reused across all pages/cards) ───

macro_rules! cached_selector {
    ($fn_name:ident, $css:expr) => {
        fn $fn_name() -> &'static scraper::Selector {
            static SEL: OnceLock<scraper::Selector> = OnceLock::new();
            SEL.get_or_init(|| {
                scraper::Selector::parse($css).expect("static selector strings must compile")
            })
        }
    };
}

cached_selector!(body_selector, "body");
cached_selector!(og_site_name_selector, r#"meta[property="og:site_name"]"#);
cached_selector!(h1_selector, "h1");
cached_selector!(title_selector, "title");
cached_selector!(meta_description_selector, r#"meta[name="description"]"#);
cached_selector!(og_description_selector, r#"meta[property="og:description"]"#);
cached_selector!(jsonld_selector, r#"script[type="application/ld+json"]"#);
cached_selector!(anchor_selector, "a[href]");
cached_selector!(
    team_card_selector,
    "[class*='team-member'], [class*='team_member'], [class*='member'], [class*='person'], [class*='staff'], [id*='team-member']"
);

/// Per-card selectors for member names — parsed once, reused for every card.
fn member_name_selectors() -> &'static [scraper::Selector] {
    static SELS: OnceLock<Vec<scraper::Selector>> = OnceLock::new();
    SELS.get_or_init(|| {
        ["h1", "h2", "h3", "h4", "h5", "strong", "b", ".name", "[class*='name']"]
            .into_iter()
            .filter_map(|s| scraper::Selector::parse(s).ok())
            .collect()
    })
}

/// Per-card selectors for member roles — parsed once, reused for every card.
fn member_role_selectors() -> &'static [scraper::Selector] {
    static SELS: OnceLock<Vec<scraper::Selector>> = OnceLock::new();
    SELS.get_or_init(|| {
        [
            "[class*='role']",
            "[class*='title']",
            "[class*='position']",
            ".role",
            ".title",
            ".position",
        ]
        .into_iter()
        .filter_map(|s| scraper::Selector::parse(s).ok())
        .collect()
    })
}

// ─── Pure extraction helpers (unit-testable) ───

/// Normalize a URL to a site base: ensure scheme, strip path.
fn normalize_base_url(url: &str) -> String {
    let with_scheme = if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };
    match url::Url::parse(&with_scheme) {
        Ok(parsed) => {
            let host = parsed.host_str().unwrap_or_default().to_string();
            if host.is_empty() {
                return with_scheme;
            }
            let scheme = parsed.scheme();
            // Keep the root of the site.
            format!("{scheme}://{host}")
        }
        Err(_) => with_scheme,
    }
}

/// Join a relative path onto a base URL.
fn join_url(base: &str, path: &str) -> String {
    match url::Url::parse(base) {
        Ok(parsed) => parsed.join(path).map(|u| u.to_string()).unwrap_or_else(|_| format!("{base}{path}")),
        Err(_) => format!("{base}{path}"),
    }
}

/// Extract visible text from a parsed document (scripts/styles skipped).
fn html_visible_text(document: &scraper::Html) -> String {
    let Some(body) = document.select(body_selector()).next() else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    collect_visible_text_nodes(&body, &mut parts);
    parts.join(" ")
}

/// Collect text nodes under `el`, skipping `script`/`style`/`noscript`
/// subtrees (their contents are not visible text).
fn collect_visible_text_nodes(el: &scraper::ElementRef, parts: &mut Vec<String>) {
    for child in el.children() {
        if let Some(text) = child.value().as_text() {
            parts.push(text.to_string());
        } else if let Some(child_el) = scraper::ElementRef::wrap(child) {
            if matches!(child_el.value().name(), "script" | "style" | "noscript") {
                continue;
            }
            collect_visible_text_nodes(&child_el, parts);
        }
    }
}

/// Company name precedence: JSON-LD `Organization.name` → og:site_name →
/// first `<h1>` → `<title>` stripped of suffixes → host name.
fn extract_company_name(document: &scraper::Html, jsonld: &[serde_json::Value], base_url: &str) -> String {
    // JSON-LD Organization.
    if let Some(name) = jsonld_field(jsonld, "Organization", "name") {
        if !name.is_empty() {
            return name;
        }
    }

    if let Some(name) = document
        .select(og_site_name_selector())
        .next()
        .and_then(|el| el.value().attr("content"))
    {
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }

    if let Some(el) = document.select(h1_selector()).next() {
        let text: String = el.text().collect();
        let text = text.trim();
        // Only trust short h1s — long ones are usually slogans.
        if !text.is_empty() && text.chars().count() <= 60 {
            return text.to_string();
        }
    }

    if let Some(el) = document.select(title_selector()).next() {
        let raw: String = el.text().collect();
        let name = clean_title_as_name(&raw);
        if !name.is_empty() {
            return name;
        }
    }

    // Last resort: host name.
    url::Url::parse(base_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
        .unwrap_or_default()
}

/// Clean a `<title>` into a company name: cut separators, keep the first part.
fn clean_title_as_name(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    for sep in [" | ", " — ", " - ", " :: ", " » ", " – "] {
        if let Some(idx) = trimmed.find(sep) {
            let head = trimmed[..idx].trim();
            if !head.is_empty() {
                return head.to_string();
            }
        }
    }
    trimmed.chars().take(100).collect()
}

/// Description precedence: JSON-LD → meta description → og:description.
fn extract_meta_description(document: &scraper::Html, jsonld: &[serde_json::Value]) -> Option<String> {
    if let Some(desc) = jsonld_field(jsonld, "Organization", "description") {
        if !desc.is_empty() {
            return Some(desc);
        }
    }
    for sel in [meta_description_selector(), og_description_selector()] {
        if let Some(content) = document.select(sel).next().and_then(|el| el.value().attr("content")) {
            let content = content.trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
        }
    }
    None
}

/// Parse the JSON-LD blocks of a document once, in document order.
///
/// Matches the historical scan semantics: collection stops at the first
/// block that fails to parse (blocks after it were never consulted).
fn jsonld_blocks(document: &scraper::Html) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for el in document.select(jsonld_selector()) {
        let raw: String = el.text().collect();
        match serde_json::from_str::<serde_json::Value>(raw.trim()) {
            Ok(value) => out.push(value),
            Err(_) => break,
        }
    }
    out
}

/// Extract a string field from the first JSON-LD block of the given @type.
fn jsonld_field(jsonld: &[serde_json::Value], schema_type: &str, field: &str) -> Option<String> {
    jsonld
        .iter()
        .find_map(|value| find_jsonld_value(value, schema_type, field))
}

/// Walk a JSON-LD value (object, array, or graph) looking for a node whose
/// `@type` matches, then return its string field.
fn find_jsonld_value(value: &serde_json::Value, schema_type: &str, field: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            let type_matches = map.get("@type").map(|t| match t {
                serde_json::Value::String(s) => s.eq_ignore_ascii_case(schema_type),
                serde_json::Value::Array(items) => items
                    .iter()
                    .any(|i| i.as_str().map(|s| s.eq_ignore_ascii_case(schema_type)).unwrap_or(false)),
                _ => false,
            });
            if type_matches == Some(true) {
                if let Some(v) = map.get(field) {
                    if let Some(s) = v.as_str() {
                        if !s.is_empty() {
                            return Some(s.to_string());
                        }
                    }
                    // numberOfEmployees may be a QuantitativeValue.
                    if field == "numberOfEmployees" {
                        if let Some(n) = v.get("value").and_then(|x| x.as_u64()) {
                            return Some(n.to_string());
                        }
                    }
                }
            }
            for (_, child) in map {
                if let Some(found) = find_jsonld_value(child, schema_type, field) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(found) = find_jsonld_value(item, schema_type, field) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Industry detection: JSON-LD keywords, then common industry keywords in text.
fn extract_industry(jsonld: &[serde_json::Value], visible_text: &str) -> Option<String> {
    for field in ["industry", "keywords"] {
        if let Some(v) = jsonld_field(jsonld, "Organization", field) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.chars().take(120).collect());
            }
        }
    }
    let lower = visible_text.to_lowercase();
    const KEYWORDS: &[(&str, &str)] = &[
        ("software development", "Software Development"),
        ("software", "Software"),
        ("fintech", "Fintech"),
        ("financial services", "Financial Services"),
        ("e-commerce", "E-commerce"),
        ("ecommerce", "E-commerce"),
        ("construction", "Construction"),
        ("logistics", "Logistics"),
        ("healthcare", "Healthcare"),
        ("marketing agency", "Marketing"),
        ("manufacturing", "Manufacturing"),
        ("consulting", "Consulting"),
        ("real estate", "Real Estate"),
        ("education", "Education"),
    ];
    KEYWORDS
        .iter()
        .find(|(kw, _)| lower.contains(kw))
        .map(|(_, label)| label.to_string())
}

/// Detect employee-count mentions like "250+ employees" / "1,000 employees".
fn extract_company_size(text: &str) -> Option<String> {
    let re = Regex::new(r"(?i)(\d[\d\s,\.]*)\s*\+?\s*(?:employees|staff members|people|team members|сотрудников)")
        .ok()?;
    re.captures(text).map(|caps| caps[0].trim().to_string())
}

/// Detect headquarters mentions like "headquartered in London".
fn extract_headquarters(text: &str) -> Option<String> {
    // Capture a run of capitalized words (city name), stopping at commas etc.
    let re = Regex::new(r"(?i)headquartered\s+in\s+([A-Z][A-Za-z\-]+(?:\s+[A-Z][A-Za-z\-]+)*)").ok()?;
    re.captures(text).map(|caps| caps[1].trim().to_string())
}

/// Extract unique, plausible email addresses from raw HTML.
fn extract_emails(text: &str) -> Vec<String> {
    let re = match Regex::new(r"(?i)[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}") {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in re.find_iter(text) {
        let email = m.as_str().to_lowercase();
        // Skip junk: image/file names and tracking pixels.
        if email.ends_with(".png")
            || email.ends_with(".jpg")
            || email.ends_with(".jpeg")
            || email.ends_with(".gif")
            || email.ends_with(".svg")
            || email.ends_with(".webp")
            || email.contains("example.")
            || email.contains("sentry")
            || email.contains("wixpress")
            || email.contains("@2x")
        {
            continue;
        }
        if seen.insert(email.clone()) {
            out.push(email);
        }
        if out.len() >= 20 {
            break;
        }
    }
    out
}

/// Extract phone numbers from visible text.
fn extract_phones(text: &str) -> Vec<String> {
    let re = match Regex::new(
        r"(?:\+\d{1,3}[\s\-.]?)?(?:\(\d{2,5}\)[\s\-.]?)?\d{1,4}(?:[\s\-.]?\d{1,4}){2,5}",
    ) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in re.find_iter(text) {
        let raw = m.as_str().trim();
        let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        // Require at least 9 digits to avoid matching dates, versions, etc.
        // (unless the number has a leading +, which signals an explicit phone).
        if digits.len() < 9 && !raw.starts_with('+') {
            continue;
        }
        if digits.len() < 5 || digits.len() > 15 {
            continue;
        }
        let normalized = if raw.starts_with('+') {
            format!("+{}", digits)
        } else {
            digits.clone()
        };
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
        if out.len() >= 10 {
            break;
        }
    }
    out
}

/// Known social domains → platform names.
fn social_platform_for(host: &str) -> Option<&'static str> {
    let host = host.to_lowercase();
    let host = host.trim_start_matches("www.");
    if host == "twitter.com" || host == "x.com" {
        Some("twitter")
    } else if host == "linkedin.com" || host.ends_with(".linkedin.com") {
        Some("linkedin")
    } else if host == "facebook.com" || host.ends_with(".facebook.com") {
        Some("facebook")
    } else if host == "instagram.com" {
        Some("instagram")
    } else if host == "github.com" {
        Some("github")
    } else if host == "youtube.com" || host.ends_with(".youtube.com") {
        Some("youtube")
    } else if host == "t.me" || host == "telegram.me" {
        Some("telegram")
    } else if host == "vk.com" {
        Some("vk")
    } else if host == "ok.ru" {
        Some("ok")
    } else {
        None
    }
}

/// Extract social profile links from all anchors in the parsed document.
fn extract_social_links(document: &scraper::Html) -> Vec<SocialProfile> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for el in document.select(anchor_selector()) {
        let Some(href) = el.value().attr("href") else { continue };
        let Ok(parsed) = url::Url::parse(href) else { continue };
        let scheme = parsed.scheme().to_string();
        if scheme != "http" && scheme != "https" {
            continue;
        }
        let Some(host) = parsed.host_str() else { continue };
        let Some(platform) = social_platform_for(host) else { continue };
        // Skip share/empty links.
        let path = parsed.path();
        if path.is_empty() || path == "/" || path.starts_with("/share") || path.starts_with("/intent") {
            continue;
        }
        let normalized = format!("{scheme}://{host}{path}");
        if seen.insert(normalized.clone()) {
            out.push(SocialProfile {
                platform: platform.to_string(),
                url: normalized,
            });
        }
        if out.len() >= 20 {
            break;
        }
    }
    out
}

/// Find a linked page whose href or link text matches one of the hints.
/// Returns an absolute URL.
fn find_linked_page(document: &scraper::Html, base_url: &str, hints: &[&str]) -> Option<String> {
    let mut fallback: Option<String> = None;
    for el in document.select(anchor_selector()) {
        let href = el.value().attr("href").unwrap_or_default();
        if href.starts_with("mailto:") || href.starts_with("tel:") || href.starts_with('#') {
            continue;
        }
        let text: String = el.text().collect();
        let text_lower = text.to_lowercase();
        let href_lower = href.to_lowercase();

        let href_match = hints.iter().any(|h| href_lower.contains(h));
        let text_match = hints.iter().any(|h| text_lower.trim() == h.trim_start_matches('/'));

        if href_match || text_match {
            let absolute = resolve_url(base_url, href);
            if href_match {
                return Some(absolute);
            } else if fallback.is_none() {
                fallback = Some(absolute);
            }
        }
    }
    fallback
}

/// Resolve a possibly-relative href against the page base URL.
fn resolve_url(base_url: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if let Ok(base) = url::Url::parse(base_url) {
        if let Ok(joined) = base.join(href) {
            return joined.to_string();
        }
    }
    format!("{base_url}{}", if href.starts_with('/') { href.to_string() } else { format!("/{href}") })
}

/// Parse team members from a parsed team/about page document.
///
/// Heuristic: elements whose class/id contains team/member/person/staff are
/// treated as member cards; within each card the name is the first heading or
/// the longest bold text, and the role is an element whose class contains
/// role/title/position or the line right after the name.
fn parse_team_members(document: &scraper::Html) -> Vec<TeamMember> {
    let mut members = Vec::new();
    let mut seen = HashSet::new();

    for card in document.select(team_card_selector()) {
        let name = find_member_name(&card);
        let Some(name) = name else { continue };
        let name = normalize_whitespace(&name);
        if !looks_like_person_name(&name) || !seen.insert(name.clone()) {
            continue;
        }
        let role = find_member_role(&card).map(|r| normalize_whitespace(&r));
        members.push(TeamMember { name, role });
        if members.len() >= 50 {
            break;
        }
    }

    // Fallback: JSON-LD Person nodes.
    if members.is_empty() {
        for el in document.select(jsonld_selector()) {
            let raw: String = el.text().collect();
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw.trim()) {
                collect_jsonld_persons(&value, &mut members, &mut seen);
            }
        }
    }

    members
}

fn collect_jsonld_persons(value: &serde_json::Value, members: &mut Vec<TeamMember>, seen: &mut HashSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            let is_person = map.get("@type").map(|t| match t {
                serde_json::Value::String(s) => s.eq_ignore_ascii_case("Person"),
                serde_json::Value::Array(items) => items
                    .iter()
                    .any(|i| i.as_str().map(|s| s.eq_ignore_ascii_case("Person")).unwrap_or(false)),
                _ => false,
            });
            if is_person == Some(true) {
                if let Some(name) = map.get("name").and_then(|n| n.as_str()) {
                    let name = normalize_whitespace(name);
                    if looks_like_person_name(&name) && seen.insert(name.clone()) {
                        let role = map.get("jobTitle").and_then(|r| r.as_str()).map(|s| s.to_string());
                        members.push(TeamMember { name, role });
                    }
                }
            }
            for (_, child) in map {
                collect_jsonld_persons(child, members, seen);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_jsonld_persons(item, members, seen);
            }
        }
        _ => {}
    }
}

fn find_member_name(card: &scraper::ElementRef) -> Option<String> {
    for sel in member_name_selectors() {
        if let Some(el) = card.select(sel).next() {
            let text: String = el.text().collect();
            let text = text.trim().to_string();
            if !text.is_empty() && text.chars().count() <= 60 {
                return Some(text);
            }
        }
    }
    None
}

fn find_member_role(card: &scraper::ElementRef) -> Option<String> {
    for sel in member_role_selectors() {
        if let Some(el) = card.select(sel).next() {
            let text: String = el.text().collect();
            let text = text.trim().to_string();
            if !text.is_empty() && text.chars().count() <= 100 {
                return Some(text);
            }
        }
    }
    None
}

/// Loose person-name check: 2-4 capitalized words, letters/spaces/hyphens,
/// and no common UI/navigation words.
fn looks_like_person_name(name: &str) -> bool {
    const STOPWORDS: &[&str] = &[
        "read", "more", "about", "our", "view", "see", "profile", "contact",
        "join", "team", "meet", "the", "and", "all", "staff", "people",
        "home", "learn", "details", "download", "subscribe",
    ];
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() < 2 || words.len() > 4 {
        return false;
    }
    if words.iter().any(|w| {
        let cleaned = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        STOPWORDS.contains(&cleaned.as_str())
    }) {
        return false;
    }
    words.iter().all(|w| {
        let clean = w.trim_matches(|c: char| !c.is_alphanumeric());
        !clean.is_empty()
            && clean.chars().next().map(|c| c.is_uppercase() || c.is_ascii_digit()).unwrap_or(false)
    }) && words[0].chars().all(|c| c.is_alphabetic() || c == '-' || c == '.')
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn merge_contacts(contacts: &mut ExtractedContacts, html: &str, visible_text: &str) {
    for email in extract_emails(html) {
        if !contacts.emails.contains(&email) {
            contacts.emails.push(email);
        }
    }
    for phone in extract_phones(visible_text) {
        if !contacts.phones.contains(&phone) {
            contacts.phones.push(phone);
        }
    }
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct CorporateParseParams {
    /// Company website URL (homepage or any page of the site).
    url: String,
}

pub struct CorporateParseTool;

#[async_trait]
impl Tool for CorporateParseTool {
    fn name(&self) -> &str {
        "parse_corporate_site"
    }
    fn description(&self) -> &str {
        "Analyze a corporate website and extract structured company data: name, description, industry, size, headquarters, contact emails/phones, social profiles and team members.

## Capability

Fetches the homepage, reads schema.org JSON-LD / OpenGraph / meta tags and visible text, discovers and parses the contact page and the team page (linked or probed at common paths like /team, /people, /staff), and returns a consolidated company profile.

## When to Use

- Profiling a company before outreach (lead generation).
- Extracting contact emails/phones and social profiles from a company website.
- Finding decision-makers via the team page.

## When NOT to Use

- Generic web content reading — use `web_fetch`.
- Finding companies you don't know yet — use `search_business_directory` first.

## Failure Modes

- Missing fields: many sites lack structured markup; fields are best-effort.
- Team members not found: team pages rendered client-side (JavaScript) are not visible to this tool — try `browser_navigate` if available.
- Sites blocking bots return an empty profile."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(CorporateParseParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: CorporateParseParams = serde_json::from_value(args)?;
        if params.url.trim().is_empty() {
            return Ok(ToolOutput::err("Parameter `url` must not be empty."));
        }

        let parser = CorporateParser::new();
        let data = parser.parse_website(params.url.trim()).await;

        if data.company_name.is_empty() && data.contacts.emails.is_empty() && data.contacts.phones.is_empty() {
            return Ok(ToolOutput::ok(format!(
                "Could not extract company data from {} (site unreachable or blocks automated access).",
                params.url
            )));
        }

        let mut output = String::new();
        output.push_str(&format!("Company: {}\n", if data.company_name.is_empty() { "(unknown)" } else { &data.company_name }));
        output.push_str(&format!("Website: {}\n", data.website));
        if let Some(ref d) = data.description {
            output.push_str(&format!("Description: {}\n", d.chars().take(500).collect::<String>()));
        }
        if let Some(ref i) = data.industry {
            output.push_str(&format!("Industry: {i}\n"));
        }
        if let Some(ref s) = data.size {
            output.push_str(&format!("Size: {s}\n"));
        }
        if let Some(ref h) = data.headquarters {
            output.push_str(&format!("Headquarters: {h}\n"));
        }
        if !data.contacts.emails.is_empty() {
            output.push_str(&format!("Emails: {}\n", data.contacts.emails.join(", ")));
        }
        if !data.contacts.phones.is_empty() {
            output.push_str(&format!("Phones: {}\n", data.contacts.phones.join(", ")));
        }
        if !data.social_profiles.is_empty() {
            output.push_str("Social profiles:\n");
            for sp in &data.social_profiles {
                output.push_str(&format!("  - {}: {}\n", sp.platform, sp.url));
            }
        }
        if let Some(ref team_url) = data.team_page_url {
            output.push_str(&format!("Team page: {team_url}\n"));
        }
        if !data.team.is_empty() {
            output.push_str("Team:\n");
            for member in data.team.iter().take(30) {
                output.push_str(&format!(
                    "  - {}{}\n",
                    member.name,
                    member.role.as_ref().map(|r| format!(" — {r}")).unwrap_or_default()
                ));
            }
        }

        let metadata = serde_json::json!({ "company": data });
        Ok(ToolOutput::ok_with_meta(output, metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper: parse HTML into a document + pre-parsed JSON-LD blocks.
    fn parsed(html: &str) -> (scraper::Html, Vec<serde_json::Value>) {
        let doc = scraper::Html::parse_document(html);
        let jsonld = jsonld_blocks(&doc);
        (doc, jsonld)
    }

    // ─── URL helpers ───

    #[test]
    fn test_normalize_base_url() {
        assert_eq!(normalize_base_url("example.com"), "https://example.com");
        assert_eq!(normalize_base_url("https://example.com/about/team"), "https://example.com");
        assert_eq!(normalize_base_url("http://example.com/x"), "http://example.com");
    }

    #[test]
    fn test_join_url() {
        assert_eq!(join_url("https://example.com", "/team"), "https://example.com/team");
        assert_eq!(join_url("https://example.com/", "/team"), "https://example.com/team");
    }

    #[test]
    fn test_resolve_url() {
        assert_eq!(resolve_url("https://a.com", "/team"), "https://a.com/team");
        assert_eq!(resolve_url("https://a.com", "https://b.com/x"), "https://b.com/x");
        assert_eq!(resolve_url("https://a.com/dir/page", "team"), "https://a.com/dir/team");
    }

    // ─── Company name ───

    #[test]
    fn test_extract_company_name_from_jsonld() {
        let html = r#"<html><head>
            <script type="application/ld+json">{"@type": "Organization", "name": "Acme Corp"}</script>
            <title>Something else | Site</title>
        </head><body></body></html>"#;
        let (doc, jsonld) = parsed(html);
        assert_eq!(extract_company_name(&doc, &jsonld, "https://acme.com"), "Acme Corp");
    }

    #[test]
    fn test_extract_company_name_from_og_and_title() {
        let html = r#"<html><head>
            <meta property="og:site_name" content="Beta Labs">
            <title>Beta Labs — Home</title>
        </head><body></body></html>"#;
        let (doc, jsonld) = parsed(html);
        assert_eq!(extract_company_name(&doc, &jsonld, "https://beta.dev"), "Beta Labs");

        let html2 = r#"<html><head><title>Gamma GmbH | Official Site</title></head><body></body></html>"#;
        let (doc2, jsonld2) = parsed(html2);
        assert_eq!(extract_company_name(&doc2, &jsonld2, "https://gamma.de"), "Gamma GmbH");
    }

    #[test]
    fn test_extract_company_name_falls_back_to_host() {
        let (doc, jsonld) = parsed("<html></html>");
        assert_eq!(extract_company_name(&doc, &jsonld, "https://www.delta.io"), "delta.io");
    }

    #[test]
    fn test_clean_title_as_name() {
        assert_eq!(clean_title_as_name("Acme Corp | Best Widgets"), "Acme Corp");
        assert_eq!(clean_title_as_name("Acme Corp — Widgets"), "Acme Corp");
        assert_eq!(clean_title_as_name("Just A Name"), "Just A Name");
        assert_eq!(clean_title_as_name("  "), "");
    }

    // ─── Description / industry / size / HQ ───

    #[test]
    fn test_extract_meta_description() {
        let html = r#"<html><head><meta name="description" content="We build widgets."></head></html>"#;
        let (doc, jsonld) = parsed(html);
        assert_eq!(
            extract_meta_description(&doc, &jsonld).as_deref(),
            Some("We build widgets.")
        );
        let (empty_doc, empty_jsonld) = parsed("<html></html>");
        assert!(extract_meta_description(&empty_doc, &empty_jsonld).is_none());
    }

    #[test]
    fn test_extract_industry_from_keywords() {
        assert_eq!(
            extract_industry(&[], "We are a software development company."),
            Some("Software Development".to_string())
        );
        assert_eq!(extract_industry(&[], "nothing relevant"), None);
    }

    #[test]
    fn test_extract_company_size() {
        assert!(extract_company_size("We have 250+ employees worldwide.")
            .unwrap()
            .contains("250"));
        assert!(extract_company_size("500 сотрудников в штате").is_some());
        assert!(extract_company_size("no size info").is_none());
    }

    #[test]
    fn test_extract_headquarters() {
        assert_eq!(
            extract_headquarters("Acme is headquartered in London, UK and has offices...").as_deref(),
            Some("London")
        );
        assert!(extract_headquarters("no hq info").is_none());
    }

    // ─── Contacts ───

    #[test]
    fn test_extract_emails() {
        let text = "Contact us: info@acme.com or sales@acme.co.uk. Ignore logo.png and a@example.com and x@x.com@2x.png";
        let emails = extract_emails(text);
        assert!(emails.contains(&"info@acme.com".to_string()));
        assert!(emails.contains(&"sales@acme.co.uk".to_string()));
        assert!(!emails.contains(&"a@example.com".to_string()));
    }

    #[test]
    fn test_extract_emails_deduplicates() {
        let emails = extract_emails("a@b.com A@B.com a@b.com");
        assert_eq!(emails.len(), 1);
    }

    #[test]
    fn test_extract_phones() {
        let phones = extract_phones("Call +7 (495) 123-45-67 or 8 800 555 35 35 now");
        assert!(phones.iter().any(|p| p == "+74951234567"));
        assert!(phones.iter().any(|p| p == "88005553535"));
    }

    #[test]
    fn test_extract_phones_ignores_short_numbers() {
        let phones = extract_phones("Founded in 2012, version 1.2.3, code 1234");
        assert!(phones.is_empty());
    }

    // ─── Social links ───

    #[test]
    fn test_extract_social_links() {
        let html = r#"<html><body>
            <a href="https://twitter.com/acme">Twitter</a>
            <a href="https://www.linkedin.com/company/acme">LinkedIn</a>
            <a href="https://t.me/acme_news">Telegram</a>
            <a href="https://example.com/about">About</a>
            <a href="https://twitter.com/share?url=x">Share</a>
        </body></html>"#;
        let (doc, _) = parsed(html);
        let links = extract_social_links(&doc);
        let platforms: Vec<&str> = links.iter().map(|l| l.platform.as_str()).collect();
        assert!(platforms.contains(&"twitter"));
        assert!(platforms.contains(&"linkedin"));
        assert!(platforms.contains(&"telegram"));
        assert_eq!(links.len(), 3, "share links and non-social links are skipped");
    }

    #[test]
    fn test_social_platform_for() {
        assert_eq!(social_platform_for("www.twitter.com"), Some("twitter"));
        assert_eq!(social_platform_for("x.com"), Some("twitter"));
        assert_eq!(social_platform_for("ru.linkedin.com"), Some("linkedin"));
        assert_eq!(social_platform_for("example.com"), None);
    }

    // ─── Linked page discovery ───

    #[test]
    fn test_find_linked_page_team() {
        let html = r#"<html><body>
            <a href="/about">About</a>
            <a href="/our-team">Our Team</a>
        </body></html>"#;
        let (doc, _) = parsed(html);
        let found = find_linked_page(&doc, "https://a.com", TEAM_PAGE_HINTS).unwrap();
        assert_eq!(found, "https://a.com/our-team");
    }

    #[test]
    fn test_find_linked_page_none() {
        let html = r#"<html><body><a href="/products">Products</a></body></html>"#;
        let (doc, _) = parsed(html);
        assert!(find_linked_page(&doc, "https://a.com", TEAM_PAGE_HINTS).is_none());
    }

    // ─── Team parsing ───

    #[test]
    fn test_parse_team_members_from_cards() {
        let html = r#"<html><body>
          <div class="team-member">
            <h3>Jane Doe</h3>
            <p class="role">CEO</p>
          </div>
          <div class="team-member">
            <h3>John Smith</h3>
            <span class="title">CTO</span>
          </div>
        </body></html>"#;
        let (doc, _) = parsed(html);
        let members = parse_team_members(&doc);
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].name, "Jane Doe");
        assert_eq!(members[0].role.as_deref(), Some("CEO"));
        assert_eq!(members[1].name, "John Smith");
        assert_eq!(members[1].role.as_deref(), Some("CTO"));
    }

    #[test]
    fn test_parse_team_members_from_jsonld() {
        let html = r#"<html><head>
          <script type="application/ld+json">
            {"@graph": [
              {"@type": "Person", "name": "Alice Cooper", "jobTitle": "Marketing Director"},
              {"@type": "Organization", "name": "Acme"}
            ]}
          </script>
        </head><body></body></html>"#;
        let (doc, _) = parsed(html);
        let members = parse_team_members(&doc);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].name, "Alice Cooper");
        assert_eq!(members[0].role.as_deref(), Some("Marketing Director"));
    }

    #[test]
    fn test_parse_team_members_ignores_non_names() {
        let html = r#"<html><body>
          <div class="team-member"><h3>Read More About Us</h3><p class="role">CTO</p></div>
          <div class="team-member"><h3>Single</h3></div>
        </body></html>"#;
        let (doc, _) = parsed(html);
        let members = parse_team_members(&doc);
        assert!(members.is_empty(), "headings that are not person names are skipped");
    }

    // ─── Name heuristics ───

    #[test]
    fn test_looks_like_person_name() {
        assert!(looks_like_person_name("Jane Doe"));
        assert!(looks_like_person_name("Jean-Claude Van Damme"));
        assert!(!looks_like_person_name("Jane"));
        assert!(!looks_like_person_name("read more about us"));
        assert!(!looks_like_person_name("Our Great Company International Group"));
    }

    #[test]
    fn test_html_visible_text_strips_scripts() {
        let html = r#"<html><body><script>var x = 1;</script><p>Hello world</p></body></html>"#;
        let (doc, _) = parsed(html);
        let text = html_visible_text(&doc);
        assert!(text.contains("Hello world"));
        assert!(!text.contains("var x"));
    }
}
