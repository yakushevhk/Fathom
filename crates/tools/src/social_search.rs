//! Social media people/company search (lead source #2).
//!
//! Searches for people and companies across social platforms:
//!
//! - **Twitter/X** — Twitter API v2 when `PARALLEL_TWITTER_BEARER_TOKEN` is
//!   set; otherwise falls back to a web search restricted to x.com/twitter.com.
//! - **Telegram** — web search restricted to t.me, enriched by fetching the
//!   public channel preview pages (name, bio, subscriber count).
//! - **LinkedIn** — web search restricted to linkedin.com (LinkedIn blocks
//!   unauthenticated scraping, so results carry no follower counts).
//!
//! All failures degrade to empty results.

use async_trait::async_trait;
use pr_core::{SearchConfig, ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::registry::{Tool, ToolContext};
use crate::search::SearchEngine;

/// Environment variable holding a Twitter/X API v2 bearer token.
pub const TWITTER_BEARER_TOKEN_ENV: &str = "PARALLEL_TWITTER_BEARER_TOKEN";

const REQUEST_TIMEOUT_SECS: u64 = 20;
const USER_AGENT: &str = "Mozilla/5.0 (compatible; ParallelResearch/0.1)";
/// How many t.me preview pages we fetch to enrich Telegram results.
const TELEGRAM_ENRICH_LIMIT: usize = 5;

/// A person or company profile found on a social platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialSearchResult {
    pub platform: String,
    pub profile_url: String,
    pub name: String,
    pub bio: Option<String>,
    pub followers: Option<u32>,
    pub location: Option<String>,
}

/// Social media search engine.
pub struct SocialSearch {
    http: reqwest::Client,
    search_config: SearchConfig,
    /// Explicit Twitter bearer token (used by tests); `None` = read env.
    twitter_token: Option<String>,
}

impl SocialSearch {
    pub fn new(search_config: SearchConfig) -> Self {
        Self {
            http: pr_core::http_client(),
            search_config,
            twitter_token: None,
        }
    }

    /// Construct with an explicit Twitter bearer token (empty = not configured).
    pub fn with_twitter_token(search_config: SearchConfig, token: &str) -> Self {
        Self {
            http: pr_core::http_client(),
            search_config,
            twitter_token: Some(token.to_string()),
        }
    }

    fn twitter_token(&self) -> String {
        self.twitter_token
            .clone()
            .unwrap_or_else(|| std::env::var(TWITTER_BEARER_TOKEN_ENV).unwrap_or_default())
    }

    /// Search all platforms in parallel and merge the results.
    pub async fn search(
        &self,
        query: &str,
        platforms: Option<&[String]>,
        limit: u32,
    ) -> Vec<SocialSearchResult> {
        let cap = limit.clamp(1, 50);
        let want = |name: &str| {
            platforms
                .map(|p| p.iter().any(|x| x.eq_ignore_ascii_case(name)))
                .unwrap_or(true)
        };

        let q = query.to_string();
        let twitter_fut = async {
            if want("twitter") || want("x") {
                self.search_twitter(&q).await
            } else {
                Vec::new()
            }
        };
        let telegram_fut = async {
            if want("telegram") {
                self.search_telegram(&q).await
            } else {
                Vec::new()
            }
        };
        let linkedin_fut = async {
            if want("linkedin") {
                self.search_linkedin(&q).await
            } else {
                Vec::new()
            }
        };

        let (a, b, c) = tokio::join!(twitter_fut, telegram_fut, linkedin_fut);
        let mut all: Vec<SocialSearchResult> = Vec::new();
        for results in [a, b, c] {
            all.extend(results);
        }
        dedupe_profiles(&mut all);
        all.truncate(cap as usize);
        all
    }

    // ─── Twitter/X ───

    /// Search Twitter/X users. Uses the Twitter API v2 when a bearer token is
    /// configured, otherwise falls back to a site-restricted web search.
    pub async fn search_twitter(&self, query: &str) -> Vec<SocialSearchResult> {
        let token = self.twitter_token();
        if !token.trim().is_empty() {
            let results = self.search_twitter_api(query, &token).await;
            if !results.is_empty() {
                return results;
            }
        }
        // Fallback: web search restricted to X/Twitter profile pages.
        let engine = SearchEngine::new(self.search_config.clone());
        let results = engine
            .search(&format!("site:x.com OR site:twitter.com {query}"), 10)
            .await;
        results
            .into_iter()
            .filter_map(|r| parse_profile_url(&r.url, &["x.com", "twitter.com"]).map(|handle| {
                SocialSearchResult {
                    platform: "twitter".to_string(),
                    profile_url: format!("https://x.com/{handle}"),
                    name: clean_search_title(&r.title, &handle),
                    bio: if r.snippet.trim().is_empty() {
                        None
                    } else {
                        Some(r.snippet)
                    },
                    followers: None,
                    location: None,
                }
            }))
            .collect()
    }

    async fn search_twitter_api(&self, query: &str, token: &str) -> Vec<SocialSearchResult> {
        let url = format!(
            "https://api.twitter.com/2/users/search?query={}&max_results=10&user.fields=description,location,public_metrics",
            urlencoding::encode(query)
        );
        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
                Ok(value) => parse_twitter_api_response(&value),
                Err(e) => {
                    tracing::warn!("Twitter response parse error: {e}");
                    vec![]
                }
            },
            Ok(resp) => {
                tracing::warn!("Twitter search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Twitter request error: {e}");
                vec![]
            }
        }
    }

    // ─── Telegram ───

    /// Search public Telegram channels/users via a site-restricted web search,
    /// then enrich the top candidates from their t.me preview pages.
    pub async fn search_telegram(&self, query: &str) -> Vec<SocialSearchResult> {
        let engine = SearchEngine::new(self.search_config.clone());
        let results = engine
            .search(&format!("site:t.me {query}"), 10)
            .await;

        let mut candidates: Vec<(String, String)> = results
            .into_iter()
            .filter_map(|r| {
                parse_profile_url(&r.url, &["t.me"]).map(|handle| (handle, r.snippet))
            })
            .collect();
        candidates.dedup_by(|a, b| a.0 == b.0);
        candidates.truncate(TELEGRAM_ENRICH_LIMIT);

        let mut out = Vec::new();
        for (handle, snippet) in candidates {
            let enriched = self.fetch_telegram_preview(&handle).await;
            let (name, bio, followers) = enriched.unwrap_or_else(|| {
                (handle.clone(), if snippet.is_empty() { None } else { Some(snippet) }, None)
            });
            out.push(SocialSearchResult {
                platform: "telegram".to_string(),
                profile_url: format!("https://t.me/{handle}"),
                name,
                bio,
                followers,
                location: None,
            });
        }
        out
    }

    /// Fetch the public preview page of a Telegram channel/user and extract
    /// name, description and subscriber count.
    async fn fetch_telegram_preview(&self, handle: &str) -> Option<(String, Option<String>, Option<u32>)> {
        let url = format!("https://t.me/{handle}");
        let resp = self
            .http
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let html = resp.text().await.ok()?;
        let (name, bio, followers) = parse_telegram_preview_html(&html)?;
        Some((name, bio, followers))
    }

    // ─── LinkedIn ───

    /// Search LinkedIn profiles via a site-restricted web search. LinkedIn
    /// blocks unauthenticated scraping, so results are limited to what the
    /// search engine exposes (name, URL, snippet).
    pub async fn search_linkedin(&self, query: &str) -> Vec<SocialSearchResult> {
        let engine = SearchEngine::new(self.search_config.clone());
        let results = engine
            .search(&format!("site:linkedin.com/in {query}"), 10)
            .await;
        results
            .into_iter()
            .filter_map(|r| {
                parse_linkedin_result(&r.url, &r.title, &r.snippet)
            })
            .collect()
    }
}

// ─── Pure helpers (unit-testable) ───

/// Parse a Twitter API v2 user-search response.
fn parse_twitter_api_response(value: &serde_json::Value) -> Vec<SocialSearchResult> {
    let users = value["data"].as_array().cloned().unwrap_or_default();
    users
        .into_iter()
        .filter_map(|user| {
            let username = user["username"].as_str()?;
            let name = user["name"].as_str().unwrap_or(username).to_string();
            Some(SocialSearchResult {
                platform: "twitter".to_string(),
                profile_url: format!("https://x.com/{username}"),
                name,
                bio: user["description"].as_str().map(|s| s.to_string()),
                followers: user["public_metrics"]["followers_count"].as_u64().map(|v| v as u32),
                location: user["location"].as_str().map(|s| s.to_string()),
            })
        })
        .collect()
}

/// Extract the profile handle from a URL whose host is one of `hosts`.
/// Strips query strings and trailing slashes. Returns `None` for bare
/// domains, reserved paths and deep subpages. Telegram preview URLs
/// (`t.me/s/<handle>/...`) resolve to the underlying handle.
fn parse_profile_url(url: &str, hosts: &[&str]) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    if !hosts.iter().any(|h| host == *h || host.ends_with(&format!(".{h}"))) {
        return None;
    }
    let mut segments = parsed.path_segments()?;
    let mut segment = segments.next()?.to_string();
    // Telegram preview pages: /s/<handle>[/...].
    if segment == "s" {
        segment = segments.next()?.to_string();
    }
    let segment = segment.trim_start_matches('@').trim_end_matches('/').to_string();
    if segment.is_empty() {
        return None;
    }
    // Filter out platform-reserved paths that are not profiles.
    const RESERVED: &[&str] = &[
        "search", "home", "explore", "notifications", "messages", "settings", "i", "hashtag",
        "login", "signup", "share", "add", "company", "school", "showcase", "feed", "jobs",
    ];
    if RESERVED.contains(&segment.to_lowercase().as_str()) {
        return None;
    }
    Some(segment)
}

/// Parse a t.me channel/user preview page: og:title → name, og:description →
/// bio, "N subscribers"/"N members" text → follower count.
fn parse_telegram_preview_html(html: &str) -> Option<(String, Option<String>, Option<u32>)> {
    let document = scraper::Html::parse_document(html);

    let title_sel = scraper::Selector::parse(r#"meta[property="og:title"]"#).ok()?;
    let name = document
        .select(&title_sel)
        .next()?
        .value()
        .attr("content")?
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }

    let desc_sel = scraper::Selector::parse(r#"meta[property="og:description"]"#).ok()?;
    let bio = document
        .select(&desc_sel)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Subscriber count appears in the page body, e.g. "12 345 subscribers".
    let followers = extract_subscriber_count(html);

    Some((name, bio, followers))
}

/// Find "N subscribers" / "N members" / "N подписчиков" in raw HTML text.
fn extract_subscriber_count(html: &str) -> Option<u32> {
    // Strip tags to reduce false positives from attributes.
    let text: String = {
        let mut out = String::with_capacity(html.len());
        let mut in_tag = false;
        for ch in html.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => out.push(ch),
                _ => {}
            }
        }
        out
    };
    let re = regex::Regex::new(r"(?i)([\d][\d\s\u{00A0}\u{202F}]*)\s*(?:subscribers|members|подписчиков|участников)")
        .ok()?;
    re.captures(&text).and_then(|caps| {
        let digits: String = caps[1].chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<u32>().ok()
    })
}

/// Build a LinkedIn result from a search hit. Titles typically look like
/// "John Doe - CEO at Acme Corp | LinkedIn".
fn parse_linkedin_result(url: &str, title: &str, snippet: &str) -> Option<SocialSearchResult> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    if host != "linkedin.com" && !host.ends_with(".linkedin.com") {
        return None;
    }
    // Keep only personal profile URLs (/in/...) and company pages (/company/...).
    let path = parsed.path();
    let is_person = path.starts_with("/in/");
    let is_company = path.starts_with("/company/");
    if !is_person && !is_company {
        return None;
    }

    let name = title
        .split('|')
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            parsed
                .path_segments()
                .and_then(|mut s| s.nth(1))
                .unwrap_or("Unknown")
                .to_string()
        });

    Some(SocialSearchResult {
        platform: "linkedin".to_string(),
        profile_url: url.to_string(),
        name,
        bio: if snippet.trim().is_empty() {
            None
        } else {
            Some(snippet.to_string())
        },
        followers: None,
        location: None,
    })
}

/// Derive a display name from a search engine title ("Name (@handle) / X" →
/// "Name"), falling back to the handle.
fn clean_search_title(title: &str, handle: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return handle.to_string();
    }
    // Cut off the earliest common suffix.
    const SEPARATORS: &[&str] = &[
        " / X", " / Twitter", " (@", " on X", " on Twitter", " | LinkedIn", " - LinkedIn",
    ];
    let earliest = SEPARATORS
        .iter()
        .filter_map(|sep| trimmed.find(sep))
        .min();
    if let Some(idx) = earliest {
        let head = trimmed[..idx].trim();
        if !head.is_empty() {
            return head.to_string();
        }
    }
    trimmed.to_string()
}

/// Deduplicate profiles by (platform, lowercased profile URL).
fn dedupe_profiles(results: &mut Vec<SocialSearchResult>) {
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    results.retain(|r| {
        seen.insert((r.platform.clone(), r.profile_url.to_lowercase()))
    });
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SocialSearchParams {
    /// Who/what to search for: a person's name, a company name, or keywords
    /// like "CTO Acme Corp".
    query: String,
    /// Platforms to search: `twitter` (or `x`), `telegram`, `linkedin`.
    /// Defaults to all platforms.
    #[serde(default)]
    platforms: Option<Vec<String>>,
    /// Maximum number of results (default 10, max 50).
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    10
}

pub struct SocialSearchTool;

#[async_trait]
impl Tool for SocialSearchTool {
    fn name(&self) -> &str {
        "search_social"
    }
    fn description(&self) -> &str {
        "Search social platforms (Twitter/X, Telegram, LinkedIn) for people and companies. Returns profile URLs, names, bios, follower counts and locations where available.

## Capability

Runs platform-specific searches in parallel. Twitter uses the X API v2 when PARALLEL_TWITTER_BEARER_TOKEN is set, otherwise a site-restricted web search. Telegram results are enriched from public t.me preview pages (name, bio, subscriber count). LinkedIn uses a site-restricted web search (LinkedIn blocks unauthenticated scraping).

## When to Use

- Finding a person's social profiles (lead generation, background research).
- Discovering company accounts and public channels.
- Getting follower counts and bios to gauge influence/relevance.

## When NOT to Use

- Company contact details — use `search_business_directory` or `parse_corporate_site`.
- News about a person/company — use `search_news`.

## Failure Modes

- Few results without API keys: web-search fallbacks depend on indexing quality; try different name spellings or add keywords (role, company, city).
- LinkedIn results have no follower counts or full bios due to platform restrictions."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(SocialSearchParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: SocialSearchParams = serde_json::from_value(args)?;
        if params.query.trim().is_empty() {
            return Ok(ToolOutput::err("Parameter `query` must not be empty."));
        }

        let searcher = SocialSearch::new(ctx.search_config.clone());
        let results = searcher
            .search(params.query.trim(), params.platforms.as_deref(), params.limit)
            .await;

        if results.is_empty() {
            return Ok(ToolOutput::ok(format!(
                "No social profiles found for '{}'. Try different phrasing or fewer keywords.",
                params.query
            )));
        }

        let mut output = format!(
            "Found {} social profiles for '{}':\n\n",
            results.len(),
            params.query
        );
        for (i, r) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}** ({})\n   URL: {}\n",
                i + 1,
                r.name,
                r.platform,
                r.profile_url
            ));
            if let Some(ref bio) = r.bio {
                let bio: String = bio.chars().take(300).collect();
                output.push_str(&format!("   Bio: {bio}\n"));
            }
            if let Some(followers) = r.followers {
                output.push_str(&format!("   Followers: {followers}\n"));
            }
            if let Some(ref location) = r.location {
                output.push_str(&format!("   Location: {location}\n"));
            }
            output.push('\n');
        }

        let metadata = serde_json::json!({
            "results": results,
            "count": results.len(),
        });
        Ok(ToolOutput::ok_with_meta(output, metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── Twitter API parsing ───

    #[test]
    fn test_parse_twitter_api_response() {
        let value = json!({
            "data": [
                {
                    "id": "1",
                    "name": "Jane Doe",
                    "username": "janedoe",
                    "description": "CEO @ Acme",
                    "location": "Berlin",
                    "public_metrics": {"followers_count": 1234}
                },
                {"id": "2", "username": "no_name"}
            ]
        });
        let results = parse_twitter_api_response(&value);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "Jane Doe");
        assert_eq!(results[0].profile_url, "https://x.com/janedoe");
        assert_eq!(results[0].bio.as_deref(), Some("CEO @ Acme"));
        assert_eq!(results[0].followers, Some(1234));
        assert_eq!(results[0].location.as_deref(), Some("Berlin"));
        assert_eq!(results[1].name, "no_name");
        assert!(results[1].followers.is_none());
    }

    #[test]
    fn test_parse_twitter_api_response_invalid() {
        assert!(parse_twitter_api_response(&json!({})).is_empty());
        assert!(parse_twitter_api_response(&json!({"data": "nope"})).is_empty());
    }

    #[tokio::test]
    async fn test_twitter_without_token_uses_fallback_path() {
        // Empty token → no API call is made (no panic, no network to api.twitter.com).
        let search = SocialSearch::with_twitter_token(SearchConfig::default(), "");
        assert_eq!(search.twitter_token(), "");
    }

    // ─── Profile URL parsing ───

    #[test]
    fn test_parse_profile_url() {
        assert_eq!(
            parse_profile_url("https://x.com/janedoe", &["x.com", "twitter.com"]),
            Some("janedoe".to_string())
        );
        assert_eq!(
            parse_profile_url("https://twitter.com/janedoe?ref=1", &["x.com", "twitter.com"]),
            Some("janedoe".to_string())
        );
        assert_eq!(
            parse_profile_url("https://x.com/@handle/", &["x.com"]),
            Some("handle".to_string())
        );
        // Reserved paths and non-profile hosts are rejected.
        assert_eq!(parse_profile_url("https://x.com/search?q=a", &["x.com"]), None);
        assert_eq!(parse_profile_url("https://x.com/", &["x.com"]), None);
        assert_eq!(parse_profile_url("https://example.com/janedoe", &["x.com"]), None);
        assert_eq!(parse_profile_url("not a url", &["x.com"]), None);
    }

    #[test]
    fn test_parse_profile_url_telegram() {
        assert_eq!(
            parse_profile_url("https://t.me/durov", &["t.me"]),
            Some("durov".to_string())
        );
        // /s/ is the public preview path; the handle is the next segment.
        assert_eq!(
            parse_profile_url("https://t.me/s/durov/123", &["t.me"]),
            Some("durov".to_string())
        );
        assert_eq!(parse_profile_url("https://t.me/share/url?url=x", &["t.me"]), None);
    }

    // ─── Telegram preview parsing ───

    #[test]
    fn test_parse_telegram_preview_html() {
        let html = r#"
        <html><head>
          <meta property="og:title" content="Tech News Daily">
          <meta property="og:description" content="Daily tech news and analysis.">
        </head><body>
          <div class="tgme_page_extra">42 500 subscribers</div>
        </body></html>"#;
        let (name, bio, followers) = parse_telegram_preview_html(html).unwrap();
        assert_eq!(name, "Tech News Daily");
        assert_eq!(bio.as_deref(), Some("Daily tech news and analysis."));
        assert_eq!(followers, Some(42500));
    }

    #[test]
    fn test_parse_telegram_preview_html_minimal() {
        let html = r#"<html><head><meta property="og:title" content="Solo"></head><body></body></html>"#;
        let (name, bio, followers) = parse_telegram_preview_html(html).unwrap();
        assert_eq!(name, "Solo");
        assert!(bio.is_none());
        assert!(followers.is_none());
    }

    #[test]
    fn test_parse_telegram_preview_html_missing_title() {
        assert!(parse_telegram_preview_html("<html></html>").is_none());
    }

    #[test]
    fn test_extract_subscriber_count_variants() {
        assert_eq!(extract_subscriber_count("<div>1 234 subscribers</div>"), Some(1234));
        assert_eq!(extract_subscriber_count("55 members joined"), Some(55));
        assert_eq!(extract_subscriber_count("10 подписчиков"), Some(10));
        assert_eq!(extract_subscriber_count("no count here"), None);
    }

    // ─── LinkedIn parsing ───

    #[test]
    fn test_parse_linkedin_result() {
        let r = parse_linkedin_result(
            "https://www.linkedin.com/in/john-doe-123",
            "John Doe - CEO at Acme Corp | LinkedIn",
            "Acme Corp CEO with 20 years experience.",
        )
        .unwrap();
        assert_eq!(r.name, "John Doe - CEO at Acme Corp");
        assert_eq!(r.platform, "linkedin");
        assert_eq!(r.bio.as_deref(), Some("Acme Corp CEO with 20 years experience."));
        assert!(r.followers.is_none());
    }

    #[test]
    fn test_parse_linkedin_result_rejects_non_profiles() {
        assert!(parse_linkedin_result("https://www.linkedin.com/pulse/article", "T", "").is_none());
        assert!(parse_linkedin_result("https://example.com/in/someone", "T", "").is_none());
        // Company pages are kept.
        assert!(parse_linkedin_result("https://www.linkedin.com/company/acme", "Acme | LinkedIn", "").is_some());
    }

    // ─── Title cleaning ───

    #[test]
    fn test_clean_search_title() {
        assert_eq!(clean_search_title("Jane Doe (@janedoe) / X", "janedoe"), "Jane Doe");
        assert_eq!(clean_search_title("Jane Doe / X", "janedoe"), "Jane Doe");
        assert_eq!(clean_search_title("", "janedoe"), "janedoe");
        assert_eq!(clean_search_title("Plain title", "handle"), "Plain title");
    }

    // ─── Deduplication ───

    #[test]
    fn test_dedupe_profiles() {
        let mut results = vec![
            SocialSearchResult {
                platform: "twitter".to_string(),
                profile_url: "https://x.com/janedoe".to_string(),
                name: "Jane".to_string(),
                bio: None,
                followers: None,
                location: None,
            },
            SocialSearchResult {
                platform: "twitter".to_string(),
                profile_url: "https://x.com/JaneDoe".to_string(),
                name: "Jane duplicate".to_string(),
                bio: None,
                followers: None,
                location: None,
            },
            SocialSearchResult {
                platform: "telegram".to_string(),
                profile_url: "https://t.me/janedoe".to_string(),
                name: "Jane TG".to_string(),
                bio: None,
                followers: None,
                location: None,
            },
        ];
        dedupe_profiles(&mut results);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "Jane");
        assert_eq!(results[1].platform, "telegram");
    }
}
