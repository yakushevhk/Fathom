//! Social profile verification tool: checks whether a profile URL on a known
//! platform exists, and extracts the username, display name and follower
//! count when publicly visible.
//!
//! GitHub profiles are verified through the public (unauthenticated) API;
//! all other platforms are verified by fetching the profile page and
//! inspecting the HTTP status plus common "not found" indicators.

use std::time::Duration;

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};

/// Browser-like User-Agent: many platforms answer bare HTTP clients with 403.
const BROWSER_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Fetch timeout for one profile page.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

// ─── Result types ───

/// Full result of a social profile verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialVerification {
    pub url: String,
    /// Canonical platform key, e.g. `x`, `instagram`, `linkedin`, `github`.
    pub platform: String,
    /// Whether the profile exists (based on HTTP status and page content).
    pub exists: bool,
    pub username: Option<String>,
    /// Display name, when publicly visible.
    pub name: Option<String>,
    /// Follower count, when publicly visible.
    pub followers: Option<u32>,
    /// HTTP status of the check, when a request was made.
    pub http_status: Option<u16>,
    /// Caveats: blocked by bot protection, rate limited, JS-rendered pages, …
    pub note: Option<String>,
}

// ─── Verifier ───

pub struct SocialVerifier;

impl SocialVerifier {
    /// Verify that a social profile URL exists and extract public metadata.
    pub async fn verify(&self, client: &reqwest::Client, url: &str) -> SocialVerification {
        let normalized = normalize_url(url);
        let Ok(parsed) = url::Url::parse(&normalized) else {
            return SocialVerification {
                url: url.trim().to_string(),
                platform: "unknown".to_string(),
                exists: false,
                username: None,
                name: None,
                followers: None,
                http_status: None,
                note: Some("could not parse URL".to_string()),
            };
        };

        let host = parsed.host_str().unwrap_or_default().to_lowercase();
        let platform = detect_platform(&host);
        let username = extract_username(&platform, &parsed);

        // GitHub has a reliable public API — prefer it.
        if platform == "github" {
            if let Some(ref user) = username {
                return verify_github(client, &normalized, user).await;
            }
        }

        // Generic page-based verification.
        let fetch = client
            .get(parsed.as_str())
            .header("User-Agent", BROWSER_UA)
            .header("Accept", "text/html,application/xhtml+xml")
            .header("Accept-Language", "en-US,en;q=0.9")
            .timeout(FETCH_TIMEOUT)
            .send()
            .await;

        match fetch {
            Err(e) => SocialVerification {
                url: normalized,
                platform: platform.to_string(),
                exists: false,
                username,
                name: None,
                followers: None,
                http_status: None,
                note: Some(format!("request failed: {e}")),
            },
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();

                let (exists, note) = match status {
                    200 => {
                        if looks_like_not_found(&body) {
                            (false, Some("page served a soft-404 (\"not found\" content)".into()))
                        } else {
                            (true, None)
                        }
                    }
                    404 | 410 => (false, None),
                    401 | 403 | 429 => (
                        false,
                        Some(format!(
                            "HTTP {status}: the platform blocks automated checks or requires login; existence could not be confirmed"
                        )),
                    ),
                    _ => (
                        false,
                        Some(format!("unexpected HTTP {status}; existence not confirmed")),
                    ),
                };

                let (name, followers) = if exists {
                    (extract_og_title(&body), extract_followers(&body))
                } else {
                    (None, None)
                };

                SocialVerification {
                    url: normalized,
                    platform: platform.to_string(),
                    exists,
                    username,
                    name,
                    followers,
                    http_status: Some(status),
                    note,
                }
            }
        }
    }
}

// ─── GitHub API verification ───

async fn verify_github(client: &reqwest::Client, original_url: &str, user: &str) -> SocialVerification {
    let api = format!("https://api.github.com/users/{user}");
    let resp = client
        .get(&api)
        .header("User-Agent", "ParallelResearch/0.1")
        .header("Accept", "application/vnd.github+json")
        .timeout(FETCH_TIMEOUT)
        .send()
        .await;

    match resp {
        Err(e) => SocialVerification {
            url: original_url.to_string(),
            platform: "github".to_string(),
            exists: false,
            username: Some(user.to_string()),
            name: None,
            followers: None,
            http_status: None,
            note: Some(format!("GitHub API request failed: {e}")),
        },
        Ok(r) => {
            let status = r.status().as_u16();
            if status == 200 {
                let value: serde_json::Value = r.json().await.unwrap_or_default();
                SocialVerification {
                    url: original_url.to_string(),
                    platform: "github".to_string(),
                    exists: true,
                    username: Some(user.to_string()),
                    name: value
                        .get("name")
                        .and_then(|v| v.as_str())
                        .or_else(|| value.get("login").and_then(|v| v.as_str()))
                        .map(str::to_string),
                    followers: value.get("followers").and_then(|v| v.as_u64()).map(|v| v as u32),
                    http_status: Some(status),
                    note: None,
                }
            } else if status == 404 {
                SocialVerification {
                    url: original_url.to_string(),
                    platform: "github".to_string(),
                    exists: false,
                    username: Some(user.to_string()),
                    name: None,
                    followers: None,
                    http_status: Some(status),
                    note: None,
                }
            } else {
                SocialVerification {
                    url: original_url.to_string(),
                    platform: "github".to_string(),
                    exists: false,
                    username: Some(user.to_string()),
                    name: None,
                    followers: None,
                    http_status: Some(status),
                    note: Some(format!(
                        "GitHub API returned HTTP {status} (possibly rate limited); existence not confirmed"
                    )),
                }
            }
        }
    }
}

// ─── Helpers ───

/// Ensure the URL has a scheme so `url::Url::parse` succeeds.
fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// Map a host name to a canonical platform key.
pub fn detect_platform(host: &str) -> &'static str {
    let host = host.trim().to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host == "twitter.com" || host == "x.com" || host.ends_with(".twitter.com") || host.ends_with(".x.com") {
        "x"
    } else if host == "instagram.com" || host.ends_with(".instagram.com") {
        "instagram"
    } else if host == "linkedin.com" || host.ends_with(".linkedin.com") {
        "linkedin"
    } else if host == "facebook.com" || host == "fb.com" || host == "fb.me" || host.ends_with(".facebook.com") {
        "facebook"
    } else if host == "github.com" || host.ends_with(".github.com") {
        "github"
    } else if host == "tiktok.com" || host.ends_with(".tiktok.com") {
        "tiktok"
    } else if host == "youtube.com" || host == "youtu.be" || host.ends_with(".youtube.com") {
        "youtube"
    } else if host == "t.me" || host == "telegram.me" {
        "telegram"
    } else if host == "vk.com" || host.ends_with(".vk.com") {
        "vk"
    } else if host == "medium.com" || host.ends_with(".medium.com") {
        "medium"
    } else if host == "threads.net" || host.ends_with(".threads.net") {
        "threads"
    } else if host == "reddit.com" || host.ends_with(".reddit.com") {
        "reddit"
    } else if host == "pinterest.com" || host.ends_with(".pinterest.com") {
        "pinterest"
    } else if host == "twitch.tv" || host.ends_with(".twitch.tv") {
        "twitch"
    } else {
        "other"
    }
}

/// Path segments that are site navigation, not usernames.
const RESERVED_SEGMENTS: &[&str] = &[
    "about", "account", "add", "apps", "blog", "business", "careers", "channel",
    "collections", "company", "customers", "developers", "enterprise", "events",
    "explore", "features", "groups", "h", "hashtag", "hashtags", "help", "home",
    "i", "intents", "join", "legal", "lists", "login", "logout", "marketplace",
    "messages", "news", "notifications", "orgs", "p", "pages", "people", "posts",
    "pricing", "privacy", "profile", "pub", "reel", "reels", "s", "safety",
    "search", "security", "settings", "share", "shop", "signup", "site", "sponsors",
    "statuses", "stories", "t", "team", "terms", "topics", "tos", "trends", "tv",
    "u", "user", "users", "watch", "wiki",
];

/// Extract the profile handle from a parsed URL.
///
/// `linkedin.com/in/<handle>`, `tiktok.com/@<handle>`, `youtube.com/@<handle>`,
/// `reddit.com/user/<name>`, and generally the first non-reserved path segment.
pub fn extract_username(platform: &str, url: &url::Url) -> Option<String> {
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() {
        return None;
    }

    // Platform-specific shapes.
    match platform {
        "linkedin" => {
            let pos = segments.iter().position(|s| matches!(*s, "in" | "company" | "school"))?;
            let handle = segments.get(pos + 1)?;
            return clean_handle(handle);
        }
        "reddit" => {
            let pos = segments.iter().position(|s| matches!(*s, "user" | "u"))?;
            return clean_handle(segments.get(pos + 1)?);
        }
        _ => {}
    }

    // General case: first non-reserved segment (skip @handle markers).
    for seg in &segments {
        let cleaned = seg.trim_start_matches('@');
        if RESERVED_SEGMENTS.contains(&cleaned.to_lowercase().as_str()) {
            continue;
        }
        if let Some(handle) = clean_handle(cleaned) {
            return Some(handle);
        }
    }
    None
}

/// Lowercase-trim a handle, drop query remnants and validate basic charset.
fn clean_handle(raw: &str) -> Option<String> {
    let handle = raw.trim().trim_start_matches('@').trim_end_matches('/').to_string();
    if handle.is_empty() || handle.len() > 64 {
        return None;
    }
    if handle.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')) {
        Some(handle)
    } else {
        None
    }
}

/// Content substrings that indicate a soft-404 profile page (checked
/// case-insensitively).
const NOT_FOUND_INDICATORS: &[&str] = &[
    "page not found",
    "404 not found",
    "user not found",
    "account not found",
    "profile not found",
    "content not found",
    "no such user",
    "this page isn't available",
    "page isn't available",
    "this account doesn't exist",
    "account doesn't exist",
    "this profile is not available",
    "profile is unavailable",
    "doesn't exist",
    "does not exist",
    "the page you were looking for doesn't exist",
    "страница не найдена",
    "профиль не найден",
    "пользователь не найден",
    "аккаунт не существует",
];

/// Heuristically detect soft-404 pages by scanning a slice of the body.
pub fn looks_like_not_found(html: &str) -> bool {
    // Scan only a window: indicators appear in <title>/headings near the top.
    let window: String = html.chars().take(20_000).collect::<String>().to_lowercase();
    NOT_FOUND_INDICATORS.iter().any(|indicator| window.contains(indicator))
}

/// Extract the `og:title` (or `<title>`) of a page as the display name.
pub fn extract_og_title(html: &str) -> Option<String> {
    let document = scraper::Html::parse_document(html);

    let og = scraper::Selector::parse(r#"meta[property="og:title"]"#).ok()?;
    let title = document
        .select(&og)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if title.is_some() {
        return title;
    }

    let title_sel = scraper::Selector::parse("title").ok()?;
    document
        .select(&title_sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Best-effort follower count extraction from page text or embedded JSON.
pub fn extract_followers(html: &str) -> Option<u32> {
    // Embedded JSON first: "followers_count": 1234, follower_count=567.
    let json_re = regex::Regex::new(r#"(?i)"?followers?_?(?:count)"?\s*[:=]\s*"?(\d[\d,]*)"#).ok()?;
    if let Some(cap) = json_re.captures(html) {
        if let Some(n) = parse_count(&cap[1], "") {
            return Some(n);
        }
    }

    // Visible text: "1,234 followers", "1.2K followers", "3.4M followers".
    let text_re = regex::Regex::new(r"(?i)(\d[\d.,]*)\s*(k|m|thousand|million)?\s*followers").ok()?;
    let window: String = html.chars().take(200_000).collect();
    if let Some(cap) = text_re.captures(&window) {
        let suffix = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        return parse_count(&cap[1], suffix);
    }
    None
}

/// Parse a possibly-suffixed count: `1,234` / `1.2K` / `3.4 million`.
fn parse_count(number: &str, suffix: &str) -> Option<u32> {
    let cleaned = number.replace(',', "");
    let value: f64 = cleaned.parse().ok()?;
    let multiplier: f64 = match suffix.to_lowercase().as_str() {
        "k" | "thousand" => 1_000.0,
        "m" | "million" => 1_000_000.0,
        "" => 1.0,
        _ => 1.0,
    };
    let total = value * multiplier;
    if total >= 0.0 && total <= u32::MAX as f64 {
        Some(total.round() as u32)
    } else {
        None
    }
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct VerifySocialParams {
    /// Social profile URL to verify (e.g. https://x.com/jack, https://github.com/torvalds)
    url: String,
}

#[async_trait]
impl Tool for SocialVerifier {
    fn name(&self) -> &str {
        "verify_social_profile"
    }
    fn description(&self) -> &str {
        "Check whether a social media profile URL exists and extract public metadata (username, display name, follower count).

## Capability

Detects the platform from the URL (X/Twitter, Instagram, LinkedIn, Facebook, GitHub, TikTok, YouTube, Telegram, VK, Medium, Threads, Reddit, Pinterest, Twitch) and verifies existence. GitHub profiles are checked via the public GitHub API (reliable). Other platforms are checked by fetching the profile page: HTTP 404/410 means the profile does not exist; HTTP 200 without \"not found\" indicators means it exists; 403/429 means the platform blocks automated checks.

## When to Use

- Confirming social handles found during OSINT research still exist.
- Getting a person's or brand's public display name and follower count.
- Normalizing profile URLs and extracting the bare username.

## When NOT to Use

- Do NOT use to read a profile's posts or timeline — use `web_fetch` on the URL for that.
- Private or login-walled profiles cannot be inspected; existence may still be confirmable.

## Failure Modes

- Platforms with aggressive bot protection (LinkedIn, Instagram, X) may answer 403; the result then reports `exists=false` with an explanatory note — treat as \"unconfirmed\", not \"deleted\".
- Follower counts are best-effort: JS-rendered pages may not expose them."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(VerifySocialParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: VerifySocialParams = serde_json::from_value(args)?;
        let result = self.verify(&ctx.http_client, &params.url).await;

        let mut out = format!("Social profile verification: {}\n", result.url);
        out.push_str(&format!("Platform: {}\n", result.platform));
        out.push_str(&format!(
            "Exists: {}\n",
            if result.exists { "yes" } else { "NO" }
        ));
        if let Some(ref u) = result.username {
            out.push_str(&format!("Username: {u}\n"));
        }
        if let Some(ref n) = result.name {
            out.push_str(&format!("Name: {n}\n"));
        }
        if let Some(f) = result.followers {
            out.push_str(&format!("Followers: {f}\n"));
        }
        if let Some(s) = result.http_status {
            out.push_str(&format!("HTTP status: {s}\n"));
        }
        if let Some(ref note) = result.note {
            out.push_str(&format!("Note: {note}\n"));
        }

        let meta = serde_json::to_value(&result).unwrap_or_default();
        Ok(ToolOutput::ok_with_meta(out, meta))
    }
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(url: &str) -> url::Url {
        url::Url::parse(url).unwrap()
    }

    // ── Platform detection ──

    #[test]
    fn test_detect_platforms() {
        assert_eq!(detect_platform("x.com"), "x");
        assert_eq!(detect_platform("www.twitter.com"), "x");
        assert_eq!(detect_platform("www.instagram.com"), "instagram");
        assert_eq!(detect_platform("linkedin.com"), "linkedin");
        assert_eq!(detect_platform("www.linkedin.com"), "linkedin");
        assert_eq!(detect_platform("facebook.com"), "facebook");
        assert_eq!(detect_platform("github.com"), "github");
        assert_eq!(detect_platform("tiktok.com"), "tiktok");
        assert_eq!(detect_platform("youtube.com"), "youtube");
        assert_eq!(detect_platform("t.me"), "telegram");
        assert_eq!(detect_platform("vk.com"), "vk");
        assert_eq!(detect_platform("medium.com"), "medium");
        assert_eq!(detect_platform("threads.net"), "threads");
        assert_eq!(detect_platform("reddit.com"), "reddit");
        assert_eq!(detect_platform("example.com"), "other");
        assert_eq!(detect_platform("WWW.X.COM"), "x");
    }

    // ── Username extraction ──

    #[test]
    fn test_extract_username_general() {
        assert_eq!(
            extract_username("x", &parse("https://x.com/jack")),
            Some("jack".to_string())
        );
        assert_eq!(
            extract_username("instagram", &parse("https://www.instagram.com/therock/?hl=en")),
            Some("therock".to_string())
        );
        assert_eq!(
            extract_username("tiktok", &parse("https://www.tiktok.com/@scout2015")),
            Some("scout2015".to_string())
        );
        assert_eq!(
            extract_username("telegram", &parse("https://t.me/durov")),
            Some("durov".to_string())
        );
    }

    #[test]
    fn test_extract_username_platform_specific() {
        assert_eq!(
            extract_username("linkedin", &parse("https://www.linkedin.com/in/john-doe/")),
            Some("john-doe".to_string())
        );
        assert_eq!(
            extract_username(
                "linkedin",
                &parse("https://www.linkedin.com/company/acme-corp?originalSubdomain=de")
            ),
            Some("acme-corp".to_string())
        );
        assert_eq!(
            extract_username("reddit", &parse("https://www.reddit.com/user/spez/")),
            Some("spez".to_string())
        );
        // Site root → no username.
        assert_eq!(extract_username("github", &parse("https://github.com/")), None);
        // Reserved navigation segment.
        assert_eq!(extract_username("github", &parse("https://github.com/about")), None);
    }

    // ── Not-found detection ──

    #[test]
    fn test_looks_like_not_found() {
        assert!(looks_like_not_found("<html><title>Page not found</title></html>"));
        assert!(looks_like_not_found("<p>This account doesn't exist.</p>"));
        assert!(looks_like_not_found("<div>Пользователь не найден</div>"));
        assert!(!looks_like_not_found(
            "<html><title>John Doe (@john) / X</title><body>profile</body></html>"
        ));
        assert!(!looks_like_not_found(""));
    }

    // ── og:title extraction ──

    #[test]
    fn test_extract_og_title() {
        let html = r#"<html><head>
            <meta property="og:title" content="John Doe - CEO at Acme">
            <title>Fallback title</title>
            </head><body></body></html>"#;
        assert_eq!(extract_og_title(html), Some("John Doe - CEO at Acme".to_string()));

        let no_og = "<html><head><title>Only title</title></head></html>";
        assert_eq!(extract_og_title(no_og), Some("Only title".to_string()));

        assert_eq!(extract_og_title("<html></html>"), None);
    }

    // ── Followers extraction ──

    #[test]
    fn test_extract_followers_text() {
        assert_eq!(extract_followers("<span>1,234 followers</span>"), Some(1234));
        assert_eq!(extract_followers("<span>1.2K followers</span>"), Some(1200));
        assert_eq!(extract_followers("<span>3.4M followers</span>"), Some(3_400_000));
        assert_eq!(extract_followers("<span>42 Followers</span>"), Some(42));
        assert_eq!(extract_followers("no counts here"), None);
    }

    #[test]
    fn test_extract_followers_json() {
        assert_eq!(
            extract_followers(r#"{"followers_count": 567}"#),
            Some(567)
        );
        assert_eq!(extract_followers(r#"followers_count=89"#), Some(89));
    }

    #[test]
    fn test_parse_count_edge_cases() {
        assert_eq!(parse_count("0", ""), Some(0));
        assert_eq!(parse_count("abc", ""), None);
        assert_eq!(parse_count("2", "million"), Some(2_000_000));
    }

    // ── URL normalization ──

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("github.com/torvalds"), "https://github.com/torvalds");
        assert_eq!(
            normalize_url("https://github.com/torvalds"),
            "https://github.com/torvalds"
        );
        assert_eq!(
            normalize_url("http://x.com/jack"),
            "http://x.com/jack"
        );
    }

    // ── Tool plumbing ──

    #[test]
    fn test_tool_metadata() {
        let tool = SocialVerifier;
        assert_eq!(tool.name(), "verify_social_profile");
        let schema = tool.schema();
        assert_eq!(schema.name, "verify_social_profile");
        assert!(schema.parameters.get("properties").is_some());
    }
}
