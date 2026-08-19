//! Person enrichment tool: searches the web for a person and cross-references
//! the results to fill in title, company, social profiles, contact details,
//! location and a short bio.

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};
use crate::search::{SearchEngine, SearchResult};

/// Maximum bio length returned to the model.
const MAX_BIO_CHARS: usize = 500;

// ─── Result types ───

/// Enriched person profile. Fields are `None` when no corroborated signal
/// was found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonEnrichment {
    pub name: String,
    pub title: Option<String>,
    pub company: Option<String>,
    pub linkedin: Option<String>,
    pub twitter: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub location: Option<String>,
    pub bio: Option<String>,
    /// URLs the enrichment was based on.
    pub sources: Vec<String>,
}

// ─── Enricher ───

pub struct PersonEnricher;

impl PersonEnricher {
    /// Enrich a person record.
    ///
    /// 1. Search the web for the person (general + LinkedIn-targeted queries).
    /// 2. Find LinkedIn / Twitter profile URLs among the results.
    /// 3. Extract contact info (email, phone) from result snippets.
    /// 4. Cross-reference: only results mentioning the person's name are used.
    pub async fn enrich(
        &self,
        ctx: &ToolContext,
        name: &str,
        company: Option<&str>,
    ) -> PersonEnrichment {
        let name = name.trim().to_string();
        let engine = SearchEngine::new(ctx.search_config.clone());
        let company_suffix = company
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(|c| format!(" \"{c}\""))
            .unwrap_or_default();

        // 1. General search + LinkedIn-targeted search.
        let general = engine.search(&format!("\"{name}\"{company_suffix}"), 10).await;
        let linkedin = engine
            .search(&format!("\"{name}\" site:linkedin.com/in{company_suffix}"), 5)
            .await;

        // 2. Cross-reference: keep only results that mention the person.
        let mut results: Vec<SearchResult> = general
            .into_iter()
            .chain(linkedin)
            .filter(|r| mentions_name(&name, r))
            .collect();
        results.sort_by(|a, b| a.url.cmp(&b.url));
        results.dedup_by(|a, b| a.url == b.url);

        let sources: Vec<String> = results.iter().map(|r| r.url.clone()).take(12).collect();

        // 3. Extract profile URLs.
        let linkedin_url = find_profile_url(&results, "linkedin.com/in/");
        let twitter_url = find_twitter_url(&results);

        // 4. Title & company from the LinkedIn result title, with fallbacks.
        let (mut title, mut company_found) = (None, None);
        if let Some(li) = results
            .iter()
            .find(|r| r.url.contains("linkedin.com/in/"))
        {
            if let Some((t, c)) = parse_linkedin_title(&li.title, &name) {
                title = t;
                company_found = c;
            }
        }
        let all_text = results
            .iter()
            .map(|r| format!("{}\n{}", r.title, r.snippet))
            .collect::<Vec<_>>()
            .join("\n");
        if title.is_none() {
            title = extract_title_fallback(&name, &all_text);
        }
        if company_found.is_none() {
            company_found = extract_company_fallback(&name, &all_text);
        }

        // 5. Contact info.
        let email = extract_email(&all_text);
        let phone = extract_phone(&all_text);

        // 6. Location + bio.
        let location = extract_location(&all_text);
        let bio = pick_bio(&name, &results);

        PersonEnrichment {
            name,
            title,
            company: company_found,
            linkedin: linkedin_url,
            twitter: twitter_url,
            email,
            phone,
            location,
            bio,
            sources,
        }
    }
}

// ─── Cross-referencing helpers ───

/// A result is attributed to the person only when the full name appears in
/// its title or snippet (case-insensitive).
pub fn mentions_name(name: &str, result: &SearchResult) -> bool {
    let name = name.to_lowercase();
    result.title.to_lowercase().contains(&name)
        || result.snippet.to_lowercase().contains(&name)
}

// ─── URL extraction ───

/// Find the first profile URL containing `marker` (query string stripped).
pub fn find_profile_url(results: &[SearchResult], marker: &str) -> Option<String> {
    results
        .iter()
        .find(|r| r.url.contains(marker))
        .map(|r| r.url.split('?').next().unwrap_or(&r.url).trim_end_matches('/').to_string())
}

/// Find an X/Twitter profile URL, skipping reserved paths (/home, /share, …).
pub fn find_twitter_url(results: &[SearchResult]) -> Option<String> {
    const RESERVED: &[&str] = &[
        "home", "share", "intent", "search", "explore", "i", "hashtag",
        "login", "signup", "tos", "privacy", "about", "settings", "notifications",
    ];
    for result in results {
        let Ok(url) = url::Url::parse(&result.url) else {
            continue;
        };
        let host = url.host_str().unwrap_or_default().to_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host);
        if host != "twitter.com" && host != "x.com" {
            continue;
        }
        let first = url.path_segments().and_then(|mut s| s.next()).unwrap_or("");
        let first = first.trim_start_matches('@');
        if first.is_empty() || RESERVED.contains(&first) {
            continue;
        }
        return Some(format!("https://x.com/{first}"));
    }
    None
}

// ─── LinkedIn title parsing ───

/// Parse `"Jane Doe - CEO & Co-Founder - Acme | LinkedIn"` into
/// `(Some("CEO & Co-Founder"), Some("Acme"))`.
///
/// Handles `- Title at Company`, `- Title, Company` and title-only variants.
/// Returns `None` when the title does not appear to belong to `name`.
pub fn parse_linkedin_title(title: &str, name: &str) -> Option<(Option<String>, Option<String>)> {
    // Strip trailing "| LinkedIn" / "- LinkedIn" decorations.
    let cleaned = title
        .split("| LinkedIn")
        .next()
        .unwrap_or(title)
        .split(" - LinkedIn")
        .next()
        .unwrap_or(title)
        .trim();

    // Split on " - " or " – " separators.
    let parts: Vec<&str> = cleaned
        .split(" - ")
        .flat_map(|s| s.split(" – "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 2 {
        return None;
    }

    // The first segment must be (approximately) the person's name.
    let first = parts[0].to_lowercase();
    let expected = name.to_lowercase();
    if first != expected && !first.starts_with(&expected) && !expected.starts_with(&first) {
        return None;
    }

    let rest = &parts[1..];
    // "Title at Company" inside one segment.
    for part in rest {
        if let Some((t, c)) = part.split_once(" at ") {
            let t = t.trim();
            let c = c.trim();
            if !t.is_empty() && !c.is_empty() {
                return Some((Some(t.to_string()), Some(c.to_string())));
            }
        }
    }
    match rest.len() {
        1 => Some((Some(rest[0].to_string()), None)),
        _ => Some((Some(rest[0].to_string()), Some(rest[1].to_string()))),
    }
}

// ─── Free-text fallbacks ───

/// Fallback title extraction: "… is the CEO of …", "works as Head of X at …".
pub fn extract_title_fallback(name: &str, text: &str) -> Option<String> {
    let escaped = regex::escape(name);
    let pattern = format!(
        r"(?i){escaped}\s+(?:is|works as|serves as)\s+(?:(?:a|an|the)\s+)?([A-Za-z][A-Za-z0-9 ,&'.()-]{{2,80}}?)(?:\s+(?:at|of|for)\s|[,.;]|$)"
    );
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(text)
        .map(|cap| cap[1].trim().trim_end_matches('.').to_string())
        .filter(|t| !t.is_empty())
}

/// Company-name capture: consecutive capitalized words with `of|the|for|&`
/// connectives, so surrounding lowercase prose is not swallowed.
const COMPANY_NAME: &str =
    r"[A-Z][A-Za-z0-9.&'-]*(?:\s+(?:of|the|for|&|[A-Z][A-Za-z0-9.&'-]*))*";

/// Fallback company extraction: "works at Acme", "founder of Acme".
pub fn extract_company_fallback(name: &str, text: &str) -> Option<String> {
    let escaped = regex::escape(name);
    let patterns = [
        format!(r"(?i){escaped}\s+(?:works at|works for|joined)\s+(?-i:({COMPANY_NAME}))"),
        format!(r"(?i){escaped}\s+(?:is|was)\s+(?:a|an|the)?\s*(?:co-)?(?:founder|ceo|cto|coo|cfo|president|owner)\s+of\s+(?-i:({COMPANY_NAME}))"),
    ];
    for pattern in &patterns {
        let Ok(re) = regex::Regex::new(pattern) else {
            continue;
        };
        if let Some(cap) = re.captures(text) {
            let company = cap[1]
                .trim()
                .trim_end_matches(|c: char| matches!(c, ',' | '.' | ';' | ')' | ' '))
                .to_string();
            if !company.is_empty() {
                return Some(company);
            }
        }
    }
    None
}

// ─── Contact extraction ───

/// Email candidates whose TLD is really a file extension are false positives.
const JUNK_EMAIL_TLDS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "js", "css", "woff", "woff2",
];
/// Domains that belong to tooling/markup rather than people.
const JUNK_EMAIL_DOMAINS: &[&str] = &[
    "example.com", "example.org", "sentry.io", "sentry-next.wixpress.com",
    "wixpress.com", "schema.org", "w3.org", "godaddy.com", "google.com",
    "gstatic.com", "cloudflare.com", "domain.com", "email.com", "yourdomain.com",
    "company.com", "site.com",
];

/// Extract the first plausible email address from text.
pub fn extract_email(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").ok()?;
    for m in re.find_iter(text) {
        let email = m.as_str().trim_end_matches('.');
        let (local, domain) = email.rsplit_once('@')?;
        let domain = domain.to_lowercase();
        let tld = domain.rsplit_once('.').map(|(_, t)| t).unwrap_or_default();
        if JUNK_EMAIL_TLDS.contains(&tld) {
            continue;
        }
        if JUNK_EMAIL_DOMAINS.iter().any(|d| domain == *d || domain.ends_with(&format!(".{d}"))) {
            continue;
        }
        if local.is_empty() || local.eq_ignore_ascii_case("name") || local.eq_ignore_ascii_case("yourname") {
            continue;
        }
        return Some(email.to_string());
    }
    None
}

/// Extract the first phone number that validates against libphonenumber.
/// Only numbers with a `+` prefix or 10+ digits are considered, to avoid
/// matching years and other bare numbers.
pub fn extract_phone(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"\+?\d[\d\s().\-]{7,20}\d").ok()?;
    for m in re.find_iter(text) {
        let candidate = m.as_str().trim();
        let digits = candidate.chars().filter(|c| c.is_ascii_digit()).count();
        if !candidate.starts_with('+') && digits < 10 {
            continue;
        }
        if let Ok(number) = phonenumber::parse(None, candidate) {
            if number.is_valid() {
                return Some(number.format().mode(phonenumber::Mode::E164).to_string());
            }
        }
    }
    None
}

/// Extract a location: "based in Berlin", "lives in Austin, TX".
pub fn extract_location(text: &str) -> Option<String> {
    let re = regex::Regex::new(
        r"(?:based in|located in|lives in|living in|resides in)\s+([A-Z][A-Za-z0-9 ,&'-]{2,50})",
    )
    .ok()?;
    re.captures(text)
        .map(|cap| cap[1].trim().trim_end_matches(|c: char| matches!(c, ',' | '.' | ';' | ')')).to_string())
        .filter(|l| !l.is_empty())
}

// ─── Bio ───

/// Pick the most informative snippet as a bio: prefer LinkedIn profile
/// snippets, then the longest snippet mentioning the name.
pub fn pick_bio(name: &str, results: &[SearchResult]) -> Option<String> {
    let name = name.to_lowercase();

    let mut candidates: Vec<&str> = results
        .iter()
        .filter(|r| r.url.contains("linkedin.com/in/"))
        .map(|r| r.snippet.as_str())
        .filter(|s| !s.trim().is_empty())
        .collect();
    if candidates.is_empty() {
        candidates = results
            .iter()
            .map(|r| r.snippet.as_str())
            .filter(|s| !s.trim().is_empty() && s.to_lowercase().contains(&name))
            .collect();
    }

    candidates
        .into_iter()
        .max_by_key(|s| s.len())
        .map(|s| {
            let chars: Vec<char> = s.chars().collect();
            if chars.len() <= MAX_BIO_CHARS {
                s.trim().to_string()
            } else {
                let truncated: String = chars.iter().take(MAX_BIO_CHARS).collect();
                format!("{truncated}...")
            }
        })
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EnrichPersonParams {
    /// Person's full name, e.g. "Jane Doe"
    name: String,
    /// Known employer or company (greatly improves matching for common names)
    #[serde(default)]
    company: Option<String>,
}

#[async_trait]
impl Tool for PersonEnricher {
    fn name(&self) -> &str {
        "enrich_person"
    }
    fn description(&self) -> &str {
        "Enrich a person record: find their title, company, LinkedIn and X/Twitter profiles, public contact details (email, phone), location and a short bio by searching the web.

## Capability

Runs general and LinkedIn-targeted web searches, then cross-references the results: only pages that mention the full name are used. Extracts the job title and company from LinkedIn result titles, discovers profile URLs, pulls email addresses and phone numbers from snippets (phones are validated with libphonenumber), and picks the most informative snippet as the bio.

## When to Use

- Building person profiles during OSINT research or lead generation.
- Completing partial contact records before outreach.
- Finding the right LinkedIn/X profile for a name.

## When NOT to Use

- Common names without a company hint produce ambiguous results — always pass `company` when known.
- Do NOT use for background checks or anything requiring verified official records; data comes from public web snippets.

## Output

Person profile with as many fields as corroborated public signals allow; unknown fields are omitted. The `sources` list shows which URLs the data came from.

## Failure Modes

- Multiple people share names — verify the company/location signals match the intended person.
- Phone numbers found in snippets may belong to the company rather than the person.
- Private individuals with little web presence yield few or no fields."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(EnrichPersonParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: EnrichPersonParams = serde_json::from_value(args)?;
        let result = self.enrich(ctx, &params.name, params.company.as_deref()).await;

        let mut out = format!("Person enrichment: {}\n", result.name);
        if let Some(ref t) = result.title {
            out.push_str(&format!("Title: {t}\n"));
        }
        if let Some(ref c) = result.company {
            out.push_str(&format!("Company: {c}\n"));
        }
        if let Some(ref l) = result.linkedin {
            out.push_str(&format!("LinkedIn: {l}\n"));
        }
        if let Some(ref t) = result.twitter {
            out.push_str(&format!("X/Twitter: {t}\n"));
        }
        if let Some(ref e) = result.email {
            out.push_str(&format!("Email: {e}\n"));
        }
        if let Some(ref p) = result.phone {
            out.push_str(&format!("Phone: {p}\n"));
        }
        if let Some(ref l) = result.location {
            out.push_str(&format!("Location: {l}\n"));
        }
        if let Some(ref b) = result.bio {
            out.push_str(&format!("Bio: {b}\n"));
        }
        if out.lines().count() == 1 {
            out.push_str("No corroborated public information found for this person.\n");
        }

        let meta = serde_json::to_value(&result).unwrap_or_default();
        Ok(ToolOutput::ok_with_meta(out, meta))
    }
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    fn result(title: &str, url: &str, snippet: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: snippet.to_string(),
        }
    }

    // ── Cross-referencing ──

    #[test]
    fn test_mentions_name() {
        let r = result("Jane Doe - CEO", "https://x", "Jane Doe leads Acme");
        assert!(mentions_name("Jane Doe", &r));
        assert!(mentions_name("jane doe", &r));
        let other = result("Someone else", "https://y", "no match");
        assert!(!mentions_name("Jane Doe", &other));
    }

    // ── Profile URLs ──

    #[test]
    fn test_find_profile_url_strips_query() {
        let results = vec![result(
            "Jane",
            "https://www.linkedin.com/in/janedoe?originalSubdomain=de",
            "s",
        )];
        assert_eq!(
            find_profile_url(&results, "linkedin.com/in/"),
            Some("https://www.linkedin.com/in/janedoe".to_string())
        );
        assert_eq!(find_profile_url(&[], "linkedin.com/in/"), None);
    }

    #[test]
    fn test_find_twitter_url() {
        let results = vec![
            result("share", "https://twitter.com/intent/tweet?text=x", "s"),
            result("home", "https://x.com/home", "s"),
            result("profile", "https://twitter.com/janedoe/status/123", "s"),
        ];
        assert_eq!(
            find_twitter_url(&results),
            Some("https://x.com/janedoe".to_string())
        );
        assert_eq!(find_twitter_url(&[]), None);
    }

    // ── LinkedIn title parsing ──

    #[test]
    fn test_parse_linkedin_title_full() {
        let (title, company) =
            parse_linkedin_title("Jane Doe - CEO & Co-Founder - Acme | LinkedIn", "Jane Doe")
                .unwrap();
        assert_eq!(title, Some("CEO & Co-Founder".to_string()));
        assert_eq!(company, Some("Acme".to_string()));
    }

    #[test]
    fn test_parse_linkedin_title_at_variant() {
        let (title, company) =
            parse_linkedin_title("Jane Doe - Head of Engineering at Acme - Berlin | LinkedIn", "Jane Doe")
                .unwrap();
        assert_eq!(title, Some("Head of Engineering".to_string()));
        assert_eq!(company, Some("Acme".to_string()));
    }

    #[test]
    fn test_parse_linkedin_title_only_title() {
        let (title, company) =
            parse_linkedin_title("Jane Doe - Software Engineer | LinkedIn", "Jane Doe").unwrap();
        assert_eq!(title, Some("Software Engineer".to_string()));
        assert_eq!(company, None);
    }

    #[test]
    fn test_parse_linkedin_title_wrong_person() {
        assert!(parse_linkedin_title("John Smith - CEO - Other | LinkedIn", "Jane Doe").is_none());
        assert!(parse_linkedin_title("LinkedIn: sign in", "Jane Doe").is_none());
    }

    // ── Contact extraction ──

    #[test]
    fn test_extract_email() {
        assert_eq!(
            extract_email("Contact me at jane.doe@acme.com anytime"),
            Some("jane.doe@acme.com".to_string())
        );
        // Junk TLD / placeholder domains are skipped.
        assert_eq!(extract_email("logo at image.png and name@example.com"), None);
        assert_eq!(extract_email("no email here"), None);
    }

    #[test]
    fn test_extract_phone_validates_with_libphonenumber() {
        assert_eq!(
            extract_phone("Call Jane at +1 415 555 2671 or visit"),
            Some("+14155552671".to_string())
        );
        assert_eq!(
            extract_phone("Office: +1 415 555"), // too short for NANP → rejected
            None
        );
        // Bare years are not treated as phone numbers.
        assert_eq!(extract_phone("since 2012 and founded 1994"), None);
    }

    // ── Location ──

    #[test]
    fn test_extract_location() {
        assert_eq!(
            extract_location("Jane is based in Austin, TX. She works"),
            Some("Austin, TX".to_string())
        );
        assert_eq!(extract_location("based in Berlin"), Some("Berlin".to_string()));
        assert_eq!(extract_location("no location"), None);
    }

    // ── Fallback extractors ──

    #[test]
    fn test_extract_title_fallback() {
        let text = "Jane Doe is the Chief Technology Officer of Acme Corp.";
        assert_eq!(
            extract_title_fallback("Jane Doe", text),
            Some("Chief Technology Officer".to_string())
        );
        assert_eq!(extract_title_fallback("Jane Doe", "nothing"), None);
    }

    #[test]
    fn test_extract_company_fallback() {
        let text = "Jane Doe works at Acme Corp and leads engineering.";
        assert_eq!(
            extract_company_fallback("Jane Doe", text),
            Some("Acme Corp".to_string())
        );
        let text2 = "Jane Doe is founder of Beta Labs.";
        assert_eq!(
            extract_company_fallback("Jane Doe", text2),
            Some("Beta Labs".to_string())
        );
    }

    // ── Bio ──

    #[test]
    fn test_pick_bio_prefers_linkedin_and_longest() {
        let results = vec![
            result("Jane", "https://other.com/jane", "short"),
            result(
                "Jane",
                "https://www.linkedin.com/in/janedoe",
                "Jane Doe is a CEO with 20 years of experience in fintech.",
            ),
        ];
        let bio = pick_bio("Jane Doe", &results).unwrap();
        assert!(bio.starts_with("Jane Doe is a CEO"));
    }

    #[test]
    fn test_pick_bio_truncates_long_text() {
        let long_snippet = "x".repeat(800);
        let results = vec![result(
            "Jane Doe",
            "https://www.linkedin.com/in/janedoe",
            &long_snippet,
        )];
        let bio = pick_bio("Jane Doe", &results).unwrap();
        assert!(bio.chars().count() <= MAX_BIO_CHARS + 3);
        assert!(bio.ends_with("..."));
    }

    #[test]
    fn test_pick_bio_none_when_empty() {
        assert_eq!(pick_bio("Jane Doe", &[]), None);
    }

    // ── Tool plumbing ──

    #[test]
    fn test_tool_metadata() {
        let tool = PersonEnricher;
        assert_eq!(tool.name(), "enrich_person");
        let schema = tool.schema();
        assert_eq!(schema.name, "enrich_person");
        assert!(schema.parameters.get("properties").is_some());
    }
}
