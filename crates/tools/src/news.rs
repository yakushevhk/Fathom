//! News / press search (lead source #4).
//!
//! Finds news articles mentioning people or companies:
//!
//! - **Serper news endpoint** when a Serper key is configured in the search
//!   config (same key `web_search` uses).
//! - **Google News RSS** as the no-key fallback.
//!
//! Each result is enriched with heuristic entity extraction: capitalized
//! word sequences are classified as persons (2-3 words) or companies
//! (corporate suffix or ALL-CAPS ticker-like tokens).

use async_trait::async_trait;
use pr_core::{SearchConfig, ToolOutput, ToolSchema};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

use crate::registry::{Tool, ToolContext};

const REQUEST_TIMEOUT_SECS: u64 = 20;
const USER_AGENT: &str = "Mozilla/5.0 (compatible; ParallelResearch/0.1)";
/// Cap on entity lists per article (keeps payloads small).
const MAX_ENTITIES_PER_RESULT: usize = 8;

/// A news article mentioning people or companies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsResult {
    pub title: String,
    pub url: String,
    pub source: String,
    pub date: Option<String>,
    pub snippet: String,
    pub mentioned_persons: Vec<String>,
    pub mentioned_companies: Vec<String>,
}

/// News search engine.
pub struct NewsSearch {
    http: reqwest::Client,
    search_config: SearchConfig,
}

impl NewsSearch {
    pub fn new(search_config: SearchConfig) -> Self {
        Self {
            http: pr_core::http_client(),
            search_config,
        }
    }

    /// Search news for mentions of `query`. Tries the Serper news endpoint
    /// first (when configured), then falls back to Google News RSS.
    pub async fn search(&self, query: &str, limit: u32) -> Vec<NewsResult> {
        let cap = limit.clamp(1, 50);
        let mut results = Vec::new();

        if let Some(ref serper) = self.search_config.serper {
            results = self.search_serper_news(query, cap, &serper.api_key).await;
        }
        if results.is_empty() {
            results = self.search_google_news_rss(query, cap).await;
        }

        // Enrich with heuristic entity extraction.
        for result in results.iter_mut() {
            let text = format!("{} {}", result.title, result.snippet);
            let (persons, companies) = extract_entities(&text);
            result.mentioned_persons = persons;
            result.mentioned_companies = companies;
        }

        results.truncate(cap as usize);
        results
    }

    // ─── Serper news ───

    async fn search_serper_news(&self, query: &str, limit: u32, api_key: &str) -> Vec<NewsResult> {
        let body = serde_json::json!({
            "q": query,
            "num": limit,
        });
        let response = self
            .http
            .post("https://google.serper.dev/news")
            .header("X-API-KEY", api_key)
            .json(&body)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
                Ok(value) => parse_serper_news_response(&value, limit),
                Err(e) => {
                    tracing::warn!("Serper news response parse error: {e}");
                    vec![]
                }
            },
            Ok(resp) => {
                tracing::warn!("Serper news search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Serper news request error: {e}");
                vec![]
            }
        }
    }

    // ─── Google News RSS (no key) ───

    async fn search_google_news_rss(&self, query: &str, limit: u32) -> Vec<NewsResult> {
        let url = format!(
            "https://news.google.com/rss/search?q={}&hl=en-US&gl=US&ceid=US:en",
            urlencoding::encode(query)
        );
        let response = self
            .http
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => match resp.text().await {
                Ok(xml) => parse_google_news_rss(&xml, limit),
                Err(e) => {
                    tracing::warn!("Google News RSS body read error: {e}");
                    vec![]
                }
            },
            Ok(resp) => {
                tracing::warn!("Google News RSS failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Google News RSS request error: {e}");
                vec![]
            }
        }
    }
}

// ─── Response parsers (pure, unit-testable) ───

/// Parse the Serper `/news` endpoint response.
fn parse_serper_news_response(value: &serde_json::Value, limit: u32) -> Vec<NewsResult> {
    let items = value["news"].as_array().cloned().unwrap_or_default();
    items
        .into_iter()
        .take(limit as usize)
        .filter_map(|item| {
            let title = item["title"].as_str()?.to_string();
            let url = item["link"].as_str().unwrap_or_default().to_string();
            Some(NewsResult {
                title,
                url,
                source: item["source"].as_str().unwrap_or_default().to_string(),
                date: item["date"].as_str().map(|s| s.to_string()),
                snippet: item["snippet"].as_str().unwrap_or_default().to_string(),
                mentioned_persons: Vec::new(),
                mentioned_companies: Vec::new(),
            })
        })
        .collect()
}

/// Parse a Google News RSS feed without an XML dependency: split on `<item>`
/// blocks and extract fields with regexes.
fn parse_google_news_rss(xml: &str, limit: u32) -> Vec<NewsResult> {
    let mut results = Vec::new();

    for chunk in xml.split("<item>").skip(1) {
        if results.len() >= limit as usize {
            break;
        }
        let chunk = match chunk.split("</item>").next() {
            Some(c) => c,
            None => continue,
        };

        let title = match extract_xml_field(chunk, "title") {
            Some(t) => decode_entities(&t),
            None => continue,
        };
        let link = extract_xml_field(chunk, "link").unwrap_or_default();
        let pub_date = extract_xml_field(chunk, "pubDate").unwrap_or_default();
        let description = extract_xml_field(chunk, "description")
            .map(|d| html_to_plain(&decode_entities(&d)))
            .unwrap_or_default();
        // <source url="...">Name</source>
        let source = extract_xml_field(chunk, "source").map(|s| decode_entities(&s));

        // Google News titles are usually "Headline - Publisher". Split the
        // headline off, and prefer the explicit <source> element when present.
        let (clean_title, derived_source) = split_title_source(&title);
        let source_name = match source {
            Some(s) if !s.trim().is_empty() => s,
            _ => derived_source,
        };

        results.push(NewsResult {
            title: clean_title,
            url: decode_entities(&link),
            source: source_name,
            date: normalize_date(&pub_date),
            snippet: description.chars().take(1000).collect(),
            mentioned_persons: Vec::new(),
            mentioned_companies: Vec::new(),
        });
    }

    results
}

/// Extract the raw contents of the first `<tag>...</tag>` occurrence.
/// Handles both `<tag>value</tag>` and CDATA-wrapped values.
fn extract_xml_field(chunk: &str, tag: &str) -> Option<String> {
    let re = Regex::new(&format!(r"(?s)<{tag}[^>]*>(.*?)</{tag}>")).ok()?;
    let inner = re.captures(chunk)?.get(1)?.as_str().to_string();
    let inner = inner
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
        .map(|s| s.to_string())
        .unwrap_or(inner);
    Some(inner.trim().to_string())
}

/// Decode the most common XML/HTML entities.
fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&#x27;", "'")
        .replace("&#xA;", "\n")
}

/// Strip HTML tags from an RSS description and decode entities.
fn html_to_plain(html: &str) -> String {
    let decoded = decode_entities(html);
    let re = match Regex::new(r"(?s)<[^>]+>") {
        Ok(r) => r,
        Err(_) => return decoded,
    };
    let no_tags = re.replace_all(&decoded, " ").into_owned();
    no_tags.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split "Headline - Publisher" into (headline, publisher). Only splits on
/// the last " - " so headlines containing dashes survive.
fn split_title_source(title: &str) -> (String, String) {
    if let Some(idx) = title.rfind(" - ") {
        let source = title[idx + 3..].trim();
        // Publisher names are short; otherwise the dash is part of the headline.
        if !source.is_empty() && source.chars().count() <= 40 {
            return (title[..idx].trim().to_string(), source.to_string());
        }
    }
    (title.to_string(), String::new())
}

/// Normalize an RFC-822 date ("Tue, 05 Aug 2026 10:00:00 GMT") to YYYY-MM-DD.
/// Returns the original string when it cannot be parsed.
fn normalize_date(date: &str) -> Option<String> {
    let trimmed = date.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Try full RFC2822 first (works when the weekday matches the date).
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(trimmed) {
        return Some(dt.format("%Y-%m-%d").to_string());
    }
    // Some feeds emit a weekday that does not match the date, which chrono
    // rejects as `Impossible`. Strip an optional leading "Day, " and retry
    // without the weekday.
    let no_weekday = if let Some(comma_pos) = trimmed.find(',') {
        let prefix = trimmed[..comma_pos].trim();
        if prefix.len() == 3 {
            trimmed[comma_pos + 1..].trim()
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    for zone in [" GMT", " UTC", " UT", " Z"] {
        let candidate = no_weekday.strip_suffix(zone).unwrap_or(no_weekday);
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(candidate, "%d %b %Y %H:%M:%S") {
            return Some(dt.format("%Y-%m-%d").to_string());
        }
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(no_weekday, "%d %b %Y %H:%M:%S") {
        return Some(dt.format("%Y-%m-%d").to_string());
    }
    Some(trimmed.to_string())
}

// ─── Heuristic entity extraction ───

/// Corporate suffixes that mark a capitalized sequence as a company.
const COMPANY_SUFFIXES: &[&str] = &[
    "Inc", "Inc.", "LLC", "Ltd", "Ltd.", "GmbH", "Corp", "Corp.", "Corporation",
    "Group", "Company", "Co", "Co.", "AG", "SAS", "SA", "Holdings", "Labs",
    "Technologies", "Technology", "Systems", "Solutions", "Partners", "Capital",
    "Bank", "Airways", "Airlines", "Motors", "Studios", "OOO", "OAO", "PAO",
    "ZAO", "АО", "ПАО", "ОАО", "ЗАО",
];

/// Sentence-initial / function words that disqualify a person candidate.
const ENTITY_STOPWORDS: &[&str] = &[
    "The", "This", "That", "These", "Those", "A", "An", "In", "On", "At", "It",
    "He", "She", "They", "We", "You", "Our", "Your", "His", "Her", "Its", "But",
    "And", "Or", "If", "When", "While", "After", "Before", "For", "From", "With",
    "About", "As", "By", "To", "Of", "New", "Last", "First", "Next", "Also",
    "More", "Most", "Some", "Many", "All", "No", "Not", "Now", "Today",
];

/// Extract person and company candidates from text.
///
/// Heuristic over maximal runs of consecutive capitalized words:
/// - company suffixes split off a company ("Acme Corp CEO Tim Cook" →
///   company "Acme Corp", then person "Tim Cook");
/// - ALL-CAPS tokens of 3+ letters (IBM, RBC) count as companies;
/// - remaining 2-3 word runs without leading stopwords are person candidates.
fn extract_entities(text: &str) -> (Vec<String>, Vec<String>) {
    let mut persons: Vec<String> = Vec::new();
    let mut companies: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let push_person = |name: &str, persons: &mut Vec<String>, seen: &mut HashSet<String>| {
        let key = name.to_lowercase();
        if persons.len() < MAX_ENTITIES_PER_RESULT && seen.insert(key) {
            persons.push(name.to_string());
        }
    };
    let push_company = |name: &str, companies: &mut Vec<String>, seen: &mut HashSet<String>| {
        let key = name.to_lowercase();
        if companies.len() < MAX_ENTITIES_PER_RESULT && seen.insert(key) {
            companies.push(name.to_string());
        }
    };

    for run in capitalized_runs(text) {
        let mut words: Vec<String> = run;

        // Split off companies at corporate suffixes, rescan the remainder.
        loop {
            let suffix_pos = words.iter().position(|w| {
                COMPANY_SUFFIXES.contains(&w.trim_matches(|c: char| !c.is_alphanumeric()))
            });
            let Some(pos) = suffix_pos else { break };
            let company = words[..=pos].join(" ");
            push_company(&company, &mut companies, &mut seen);
            words = words.split_off(pos + 1);
        }
        if words.is_empty() {
            continue;
        }

        // Drop leading ALL-CAPS title tokens (CEO, VP, CTO ...).
        while words.len() > 1 && is_all_caps(&words[0]) {
            words.remove(0);
        }

        // Drop trailing short ALL-CAPS title tokens too (e.g. "Maria Gonzalez VP").
        while words.len() > 1
            && is_all_caps(&words[words.len() - 1])
            && words[words.len() - 1].chars().count() <= 4
        {
            words.pop();
        }

        if words.len() == 1 {
            // Single ALL-CAPS token (3+ letters) → organization-like.
            if is_all_caps(&words[0]) && words[0].chars().count() >= 3 {
                push_company(&words[0], &mut companies, &mut seen);
            }
            continue;
        }

        if (2..=3).contains(&words.len()) && !ENTITY_STOPWORDS.contains(&words[0].as_str()) {
            push_person(&words.join(" "), &mut persons, &mut seen);
        }
    }

    (persons, companies)
}

/// Split text into maximal runs of consecutive capitalized words, trimming
/// surrounding punctuation.
fn capitalized_runs(text: &str) -> Vec<Vec<String>> {
    let mut runs: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();

    for token in text.split_whitespace() {
        let clean: String = token.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
        let is_cap = clean
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        if is_cap {
            current.push(clean);
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
}

/// True when the word has letters and all of them are uppercase.
fn is_all_caps(word: &str) -> bool {
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    !letters.is_empty() && letters.iter().all(|c| c.is_uppercase())
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct NewsSearchParams {
    /// Who/what to find mentions of: a person's name, a company name, or a topic.
    query: String,
    /// Maximum number of articles (default 10, max 50).
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    10
}

pub struct NewsSearchTool;

#[async_trait]
impl Tool for NewsSearchTool {
    fn name(&self) -> &str {
        "search_news"
    }
    fn description(&self) -> &str {
        "Search news and press coverage for articles mentioning a person, company, or topic. Returns title, URL, source, date, snippet and heuristically extracted mentioned persons/companies.

## Capability

Uses the Serper news endpoint when a Serper key is configured; otherwise falls back to Google News RSS (no key required). Each article includes lists of likely mentioned persons and companies extracted from the title and snippet (heuristic, may contain false positives).

## When to Use

- Checking recent press coverage of a company or executive (lead research).
- Finding context and events before outreach.
- Tracking announcements: funding, hires, product launches.

## When NOT to Use

- General web research — use `web_search`.
- Finding social profiles — use `search_social`.

## Failure Modes

- Empty results: the entity is not covered in news; try broader keywords.
- Mentioned-persons/companies lists are heuristic — verify before relying on them.
- Google News RSS links are redirect URLs; open them to reach the original article."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(NewsSearchParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: NewsSearchParams = serde_json::from_value(args)?;
        if params.query.trim().is_empty() {
            return Ok(ToolOutput::err("Parameter `query` must not be empty."));
        }

        let searcher = NewsSearch::new(ctx.search_config.clone());
        let results = searcher.search(params.query.trim(), params.limit).await;

        if results.is_empty() {
            return Ok(ToolOutput::ok(format!(
                "No news found for '{}'. Try broader keywords or `web_search`.",
                params.query
            )));
        }

        let mut output = format!("Found {} news articles for '{}':\n\n", results.len(), params.query);
        for (i, r) in results.iter().enumerate() {
            output.push_str(&format!("{}. **{}**\n", i + 1, r.title));
            output.push_str(&format!("   URL: {}\n", r.url));
            let mut meta = Vec::new();
            if !r.source.is_empty() {
                meta.push(format!("Source: {}", r.source));
            }
            if let Some(ref d) = r.date {
                meta.push(format!("Date: {d}"));
            }
            if !meta.is_empty() {
                output.push_str(&format!("   {}\n", meta.join(" | ")));
            }
            if !r.snippet.is_empty() {
                let snippet: String = r.snippet.chars().take(400).collect();
                output.push_str(&format!("   Snippet: {snippet}\n"));
            }
            if !r.mentioned_persons.is_empty() {
                output.push_str(&format!("   Persons: {}\n", r.mentioned_persons.join(", ")));
            }
            if !r.mentioned_companies.is_empty() {
                output.push_str(&format!("   Companies: {}\n", r.mentioned_companies.join(", ")));
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

    // ─── Serper news parsing ───

    #[test]
    fn test_parse_serper_news_response() {
        let value = json!({
            "news": [
                {
                    "title": "Acme Corp raises $10M",
                    "link": "https://news.example.com/acme",
                    "snippet": "Acme Corp announced a Series A.",
                    "date": "2 hours ago",
                    "source": "TechDaily"
                },
                {"link": "https://no-title.example.com"}
            ]
        });
        let results = parse_serper_news_response(&value, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Acme Corp raises $10M");
        assert_eq!(results[0].source, "TechDaily");
        assert_eq!(results[0].date.as_deref(), Some("2 hours ago"));
    }

    #[test]
    fn test_parse_serper_news_response_invalid() {
        assert!(parse_serper_news_response(&json!({}), 10).is_empty());
        assert!(parse_serper_news_response(&json!({"news": "oops"}), 10).is_empty());
    }

    // ─── Google News RSS parsing ───

    const RSS_FIXTURE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel>
<title>Query - Google News</title>
<item>
  <title>Acme Corp Announces New CEO - TechDaily</title>
  <link>https://news.google.com/rss/articles/CBMiTkFV?oc=5</link>
  <guid isPermaLink="false">abc123</guid>
  <pubDate>Tue, 05 Aug 2026 10:00:00 GMT</pubDate>
  <description>&lt;a href="https://news.google.com/rss/articles/CBMiTkFV?oc=5" target="_blank"&gt;Acme Corp Announces New CEO&lt;/a&gt;&amp;nbsp;&amp;nbsp;&lt;font color="#6f6f6f"&gt;TechDaily&lt;/font&gt;</description>
  <source url="https://techdaily.example.com">TechDaily</source>
</item>
<item>
  <title>Second story - Wire Service</title>
  <link>https://news.google.com/rss/articles/CBMiTkFY?oc=5</link>
  <pubDate>Mon, 04 Aug 2026 08:30:00 GMT</pubDate>
  <description>Plain text description</description>
  <source url="https://wire.example.com">Wire Service</source>
</item>
</channel></rss>"##;

    #[test]
    fn test_parse_google_news_rss() {
        let results = parse_google_news_rss(RSS_FIXTURE, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Acme Corp Announces New CEO");
        assert_eq!(results[0].source, "TechDaily");
        assert_eq!(results[0].date.as_deref(), Some("2026-08-05"));
        assert!(results[0].url.starts_with("https://news.google.com"));
        assert_eq!(results[1].source, "Wire Service");
        assert_eq!(results[1].snippet, "Plain text description");
    }

    #[test]
    fn test_parse_google_news_rss_limit_and_empty() {
        assert_eq!(parse_google_news_rss(RSS_FIXTURE, 1).len(), 1);
        assert!(parse_google_news_rss("<rss></rss>", 10).is_empty());
    }

    // ─── Field helpers ───

    #[test]
    fn test_extract_xml_field() {
        assert_eq!(
            extract_xml_field("<title>Hello &amp; World</title>", "title").as_deref(),
            Some("Hello &amp; World")
        );
        assert_eq!(extract_xml_field("<x>no match</x>", "title"), None);
        assert_eq!(
            extract_xml_field("<description><![CDATA[inner <b>html</b>]]></description>", "description").as_deref(),
            Some("inner <b>html</b>")
        );
    }

    #[test]
    fn test_decode_entities() {
        assert_eq!(decode_entities("Tom &amp; Jerry &#39;s &quot;show&quot;"), "Tom & Jerry 's \"show\"");
    }

    #[test]
    fn test_html_to_plain() {
        assert_eq!(
            html_to_plain("<a href=\"x\">Title</a>&nbsp;<font>Source</font>"),
            "Title Source"
        );
    }

    #[test]
    fn test_split_title_source() {
        assert_eq!(
            split_title_source("Big news today - Reuters"),
            ("Big news today".to_string(), "Reuters".to_string())
        );
        // Dash inside headline with a long tail is kept whole.
        let (t, s) = split_title_source("Headline with dash - and a very long trailing part that exceeds forty chars");
        assert_eq!(t, "Headline with dash - and a very long trailing part that exceeds forty chars");
        assert_eq!(s, "");
        let (t, s) = split_title_source("No source here");
        assert_eq!(t, "No source here");
        assert_eq!(s, "");
    }

    #[test]
    fn test_normalize_date() {
        assert_eq!(normalize_date("Tue, 05 Aug 2026 10:00:00 GMT").as_deref(), Some("2026-08-05"));
        assert_eq!(normalize_date("2 hours ago").as_deref(), Some("2 hours ago"));
        assert_eq!(normalize_date(""), None);
    }

    // ─── Entity extraction ───

    #[test]
    fn test_extract_entities_persons_and_companies() {
        let text = "Acme Corp CEO Tim Cook met with IBM executives. Maria Gonzalez, \
                      VP at Globex Corporation, also attended the Berlin summit.";
        let (persons, companies) = extract_entities(text);
        assert!(persons.iter().any(|p| p == "Tim Cook"), "persons: {persons:?}");
        assert!(persons.iter().any(|p| p == "Maria Gonzalez"), "persons: {persons:?}");
        assert!(companies.iter().any(|c| c.contains("Acme")), "companies: {companies:?}");
        assert!(companies.iter().any(|c| c == "IBM"), "companies: {companies:?}");
        assert!(companies.iter().any(|c| c.contains("Globex")), "companies: {companies:?}");
    }

    #[test]
    fn test_extract_entities_skips_stopwords() {
        let (persons, _) = extract_entities("The Future Is Now And Then Some More");
        assert!(!persons.iter().any(|p| p.starts_with("The ")));
        assert!(!persons.iter().any(|p| p.starts_with("And ")));
    }

    #[test]
    fn test_extract_entities_dedupes() {
        let (persons, _) = extract_entities("John Smith met John Smith and john smith");
        assert_eq!(persons.iter().filter(|p| p.as_str() == "John Smith").count(), 1);
    }

    #[test]
    fn test_extract_entities_empty() {
        let (persons, companies) = extract_entities("no capitalized words here at all");
        assert!(persons.is_empty());
        assert!(companies.is_empty());
    }
}
