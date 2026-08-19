//! Crawl and feed tools: multi-page BFS crawling with politeness, and
//! RSS / Atom / sitemap parsing.

use crate::parse::element_text;
use crate::registry::{Tool, ToolContext};
use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use scraper::Html;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashSet, VecDeque};

fn default_depth() -> usize {
    1
}
fn default_max_pages() -> usize {
    10
}
fn default_true() -> bool {
    true
}
fn default_delay() -> u64 {
    500
}
fn default_page_chars() -> usize {
    1500
}
fn default_feed_limit() -> usize {
    50
}

pub struct WebCrawlTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct CrawlParams {
    /// Seed URL to start crawling from (required).
    url: String,
    /// How deep to follow links from the seed (default 1, hard cap 3).
    #[serde(default = "default_depth")]
    max_depth: usize,
    /// Maximum number of pages to fetch (default 10, hard cap 50).
    #[serde(default = "default_max_pages")]
    max_pages: usize,
    /// Stay within the seed's domain (default true).
    #[serde(default = "default_true")]
    same_domain: bool,
    /// Politeness delay between fetches in milliseconds (default 500, cap 5000).
    #[serde(default = "default_delay")]
    delay_ms: u64,
    /// CSS selector scoping text extraction per page (default: whole body).
    #[serde(default)]
    selector: Option<String>,
    /// Max characters of text kept per page (default 1500, cap 8000).
    #[serde(default = "default_page_chars")]
    chars_per_page: usize,
}

/// Canonicalize a URL for deduplication: drop the fragment, lowercase the
/// host, and trim a single trailing slash on non-root paths.
fn normalize_url(raw: &str) -> String {
    let Some(mut u) = url::Url::parse(raw).ok() else {
        return raw.to_string();
    };
    u.set_fragment(None);
    if u.path() != "/" {
        let p = u.path().to_string();
        if let Some(stripped) = p.strip_suffix('/') {
            u.set_path(stripped);
        }
    }
    u.to_string()
}

/// Whether an href is worth following at all.
fn is_followable(href: &str) -> bool {
    let h = href.trim();
    if h.is_empty() || h.starts_with('#') {
        return false;
    }
    let lower = h.to_lowercase();
    !lower.starts_with("mailto:")
        && !lower.starts_with("tel:")
        && !lower.starts_with("javascript:")
        && !lower.starts_with("data:")
}

#[async_trait]
impl Tool for WebCrawlTool {
    fn name(&self) -> &str {
        "web_crawl"
    }

    fn description(&self) -> &str {
        "Crawl a website: fetch the seed page and follow its links breadth-first.

## Capability
- Follows links up to `max_depth` levels from the seed (default 1, cap 3)
- Fetches at most `max_pages` pages (default 10, cap 50)
- Deduplicates URLs, stays on the seed's domain by default, and pauses
  `delay_ms` between fetches for politeness
- Returns per-page title + extracted text (optionally scoped by a CSS
  selector) plus the total number of discovered links

## When to use
- You need content from several linked pages of one site (docs sections,
  paginated listings, category pages) in a single call
- For one page only, use `web_fetch`; for CSS-selector extraction on a page
  you already have, use `parse_html`

## Notes
- Each page's text is truncated to `chars_per_page` characters
- Errors on individual pages do not abort the crawl; they are reported inline"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(CrawlParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: CrawlParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(format!("Invalid parameters: {e}"))),
        };

        let seed = match url::Url::parse(params.url.trim()) {
            Ok(u) if u.scheme() == "http" || u.scheme() == "https" => u,
            _ => return Ok(ToolOutput::err("Parameter 'url' must be an http(s) URL")),
        };
        let seed_host = seed.host_str().unwrap_or_default().to_string();
        let max_depth = params.max_depth.min(3);
        let max_pages = params.max_pages.clamp(1, 50);
        let delay_ms = params.delay_ms.min(5000);
        let chars_per_page = params.chars_per_page.clamp(200, 8000);
        let scope_sel = match &params.selector {
            Some(s) if !s.trim().is_empty() => match scraper::Selector::parse(s) {
                Ok(sel) => Some(sel),
                Err(e) => return Ok(ToolOutput::err(format!("Invalid selector: {e}"))),
            },
            _ => None,
        };

        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        queue.push_back((seed.to_string(), 0));
        let mut pages: Vec<serde_json::Value> = Vec::new();
        let mut discovered = 0usize;

        while let Some((page_url, depth)) = queue.pop_front() {
            if pages.len() >= max_pages {
                break;
            }
            let key = normalize_url(&page_url);
            if !visited.insert(key.clone()) {
                continue;
            }
            if !pages.is_empty() && delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            let body = match crate::web::fetch_url_cached(ctx, &key).await {
                Ok((body, _content_type)) => body,
                Err(e) => {
                    pages.push(json!({"url": key, "depth": depth, "error": format!("{} ({})", e.message, e.code)}));
                    continue;
                }
            };
            let doc = Html::parse_document(&body);

            let title_sel = scraper::Selector::parse("title").unwrap();
            let title = doc
                .select(&title_sel)
                .next()
                .map(|t| element_text(&t))
                .unwrap_or_default();

            let scope: scraper::ElementRef = match &scope_sel {
                Some(sel) => doc.select(sel).next(),
                None => {
                    let body_sel = scraper::Selector::parse("body").unwrap();
                    doc.select(&body_sel).next()
                }
            }
            .unwrap_or_else(|| doc.root_element());
            let text: String = element_text(&scope).chars().take(chars_per_page).collect();

            let a_sel = scraper::Selector::parse("a").unwrap();
            let mut links: Vec<String> = Vec::new();
            for a in scope.select(&a_sel) {
                let Some(href) = a.value().attr("href") else { continue };
                if !is_followable(href) {
                    continue;
                }
                let Ok(abs) = seed.join(href) else { continue };
                if abs.scheme() != "http" && abs.scheme() != "https" {
                    continue;
                }
                if params.same_domain && abs.host_str().unwrap_or_default() != seed_host {
                    continue;
                }
                discovered += 1;
                let n = normalize_url(abs.as_ref());
                if depth < max_depth && !visited.contains(&n) {
                    links.push(abs.to_string());
                }
            }

            pages.push(json!({
                "url": key,
                "depth": depth,
                "title": title,
                "text_chars": text.len(),
                "text": text,
                "out_links_queued": links.len(),
            }));
            for link in links {
                queue.push_back((link, depth + 1));
            }
        }

        Ok(ToolOutput::ok(
            serde_json::to_string_pretty(&json!({
                "seed": seed.to_string(),
                "pages_fetched": pages.len(),
                "links_discovered": discovered,
                "pages": pages,
            }))
            .unwrap_or_default(),
        ))
    }
}

pub struct WebFeedTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct FeedParams {
    /// URL or local file path of an RSS / Atom / sitemap XML document.
    source: String,
    /// Maximum number of items to return (default 50, cap 200).
    #[serde(default = "default_feed_limit")]
    limit: usize,
    /// Include item summaries/descriptions (default true).
    #[serde(default = "default_true")]
    include_summaries: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct FeedItem {
    title: String,
    link: String,
    date: String,
    summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FeedKind {
    Rss,
    Atom,
    Sitemap,
}

/// Parse RSS 2.0, Atom, or sitemap/sitemapindex XML into a flat item list.
/// Tolerant: unknown elements are ignored, namespaces are stripped via
/// local-name matching.
fn parse_feed_xml(xml: &str) -> anyhow::Result<(FeedKind, Vec<FeedItem>)> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut kind: Option<FeedKind> = None;
    let mut items: Vec<FeedItem> = Vec::new();
    let mut current: Option<FeedItem> = None;
    let mut buf: Vec<u8> = Vec::new();
    let mut in_item = false;
    let mut in_text = false;
    let mut text_target: Option<fn(&mut FeedItem, String)> = None;

    let local = |name: quick_xml::name::QName<'_>| -> String {
        name.as_ref()
            .rsplit(|&b| b == b':')
            .next()
            .map(|s| String::from_utf8_lossy(s).to_lowercase())
            .unwrap_or_default()
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local(e.name());
                match name.as_str() {
                    "rss" => kind = Some(FeedKind::Rss),
                    "feed" => kind = Some(FeedKind::Atom),
                    "urlset" | "sitemapindex" => kind = Some(FeedKind::Sitemap),
                    "item" | "entry" | "url" | "sitemap" => {
                        in_item = true;
                        current = Some(FeedItem {
                            title: String::new(),
                            link: String::new(),
                            date: String::new(),
                            summary: String::new(),
                        });
                    }
                    "link" if in_item => {
                        let href = e
                            .attributes()
                            .flatten()
                            .find(|a| {
                                a.key.as_ref().rsplit(|&b| b == b':').next()
                                    == Some("href".as_bytes())
                            })
                            .map(|a| String::from_utf8_lossy(&a.value).trim().to_string());
                        if let Some(h) = href {
                            if let Some(cur) = current.as_mut() {
                                if cur.link.is_empty() {
                                    cur.link = h;
                                }
                            }
                        } else {
                            in_text = true;
                            text_target = Some(|it, s| {
                                if it.link.is_empty() {
                                    it.link = s;
                                }
                            });
                        }
                    }
                    "title" if in_item => {
                        in_text = true;
                        text_target = Some(|it, s| {
                            if it.title.is_empty() {
                                it.title = s;
                            }
                        });
                    }
                    "pubdate" | "published" | "updated" | "lastmod" | "date" if in_item => {
                        in_text = true;
                        text_target = Some(|it, s| {
                            if it.date.is_empty() {
                                it.date = s;
                            }
                        });
                    }
                    "description" | "summary" | "content" if in_item => {
                        in_text = true;
                        text_target = Some(|it, s| {
                            if it.summary.is_empty() {
                                it.summary = s;
                            }
                        });
                    }
                    "loc" if in_item => {
                        in_text = true;
                        text_target = Some(|it, s| {
                            if it.link.is_empty() {
                                it.link = s;
                            }
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if in_text {
                    if let Some(target) = text_target {
                        let clean: String = e
                            .unescape()
                            .unwrap_or_default()
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ");
                        if let Some(cur) = current.as_mut() {
                            target(cur, clean);
                        }
                    }
                }
            }
            Ok(Event::CData(e)) => {
                if in_text {
                    if let Some(target) = text_target {
                        let raw = String::from_utf8_lossy(e.as_ref()).to_string();
                        let clean: String =
                            raw.split_whitespace().collect::<Vec<_>>().join(" ");
                        if let Some(cur) = current.as_mut() {
                            target(cur, clean);
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = local(e.name());
                match name.as_str() {
                    "item" | "entry" | "url" | "sitemap" => {
                        if let Some(cur) = current.take() {
                            items.push(cur);
                        }
                        in_item = false;
                    }
                    _ => {
                        in_text = false;
                        text_target = None;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    let kind = kind.ok_or_else(|| anyhow::anyhow!("not an RSS/Atom/sitemap document"))?;
    Ok((kind, items))
}

#[async_trait]
impl Tool for WebFeedTool {
    fn name(&self) -> &str {
        "web_feed"
    }

    fn description(&self) -> &str {
        "Parse an RSS 2.0, Atom, or sitemap.xml feed into a clean item list.

## Capability
- Accepts a feed URL (http/https) or a local XML file path
- Detects the format: RSS 2.0 (<item>), Atom (<entry>), sitemap/sitemapindex
- Returns per item: title, link, date, and (when available) summary/description
- Namespaces are handled via local-name matching (media:, dc:, etc. work)

## When to use
- Enumerating recent posts/articles of a site via its RSS/Atom feed instead
  of crawling
- Getting every URL listed in a sitemap.xml for targeted crawling
- Feeds are cheaper and more reliable than scraping front pages"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(FeedParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: FeedParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(format!("Invalid parameters: {e}"))),
        };
        if params.source.trim().is_empty() {
            return Ok(ToolOutput::err("Parameter 'source' is required"));
        }
        let limit = params.limit.clamp(1, 200);

        let xml = if params.source.starts_with("http://") || params.source.starts_with("https://")
        {
            match crate::web::fetch_url_cached(ctx, params.source.trim()).await {
                Ok((body, _ct)) => body,
                Err(e) => return Ok(ToolOutput::err(format!("fetch failed: {} ({})", e.message, e.code))),
            }
        } else {
            match tokio::fs::read_to_string(&params.source).await {
                Ok(b) => b,
                Err(e) => {
                    return Ok(ToolOutput::err(format!(
                        "Cannot read '{}': {e}",
                        params.source
                    )))
                }
            }
        };

        let (kind, items) = match parse_feed_xml(&xml) {
            Ok(x) => x,
            Err(e) => return Ok(ToolOutput::err(e.to_string())),
        };

        let kind_name = match kind {
            FeedKind::Rss => "rss",
            FeedKind::Atom => "atom",
            FeedKind::Sitemap => "sitemap",
        };
        let items_json: Vec<serde_json::Value> = items
            .into_iter()
            .take(limit)
            .map(|it| {
                let mut m = serde_json::Map::new();
                m.insert("title".into(), json!(it.title));
                m.insert("link".into(), json!(it.link));
                if !it.date.is_empty() {
                    m.insert("date".into(), json!(it.date));
                }
                if params.include_summaries && !it.summary.is_empty() {
                    let s: String = it.summary.chars().take(500).collect();
                    m.insert("summary".into(), json!(s));
                }
                serde_json::Value::Object(m)
            })
            .collect();

        Ok(ToolOutput::ok(
            serde_json::to_string_pretty(&json!({
                "kind": kind_name,
                "source": params.source,
                "count": items_json.len(),
                "items": items_json,
            }))
            .unwrap_or_default(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_fragment_and_trailing_slash() {
        assert_eq!(
            normalize_url("https://Example.com/page#section"),
            "https://example.com/page"
        );
        assert_eq!(
            normalize_url("https://example.com/docs/"),
            "https://example.com/docs"
        );
        assert_eq!(normalize_url("https://example.com/"), "https://example.com/");
    }

    #[test]
    fn followable_filters_junk_hrefs() {
        assert!(is_followable("/page"));
        assert!(is_followable("https://a.b/c"));
        assert!(!is_followable(""));
        assert!(!is_followable("#anchor"));
        assert!(!is_followable("mailto:x@y.z"));
        assert!(!is_followable("javascript:void(0)"));
        assert!(!is_followable("tel:+123"));
    }

    #[test]
    fn parse_rss_fixture() {
        let xml = r#"<?xml version="1.0"?>
        <rss version="2.0">
          <channel>
            <title>Site news</title>
            <item>
              <title>First post</title>
              <link>https://site.test/a</link>
              <pubDate>Mon, 01 Jan 2024 10:00:00 GMT</pubDate>
              <description>Hello   world</description>
            </item>
            <item>
              <title>Second post</title>
              <link>https://site.test/b</link>
            </item>
          </channel>
        </rss>"#;
        let (kind, items) = parse_feed_xml(xml).unwrap();
        assert_eq!(kind, FeedKind::Rss);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "First post");
        assert_eq!(items[0].link, "https://site.test/a");
        assert_eq!(items[0].summary, "Hello world");
        assert!(items[0].date.contains("2024"));
        assert_eq!(items[1].title, "Second post");
    }

    #[test]
    fn parse_atom_fixture_with_namespaced_link() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
          <title>Atom feed</title>
          <entry>
            <title>Entry one</title>
            <link rel="alternate" href="https://site.test/e1"/>
            <updated>2024-02-02T00:00:00Z</updated>
            <summary>Short summary</summary>
          </entry>
        </feed>"#;
        let (kind, items) = parse_feed_xml(xml).unwrap();
        assert_eq!(kind, FeedKind::Atom);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Entry one");
        assert_eq!(items[0].link, "https://site.test/e1");
        assert_eq!(items[0].date, "2024-02-02T00:00:00Z");
    }

    #[test]
    fn parse_sitemap_fixture() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <url><loc>https://site.test/p1</loc><lastmod>2024-03-01</lastmod></url>
          <url><loc>https://site.test/p2</loc></url>
        </urlset>"#;
        let (kind, items) = parse_feed_xml(xml).unwrap();
        assert_eq!(kind, FeedKind::Sitemap);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].link, "https://site.test/p1");
        assert_eq!(items[0].date, "2024-03-01");
    }

    #[test]
    fn parse_rejects_non_feed_xml() {
        let xml = r#"<html><body>hi</body></html>"#;
        assert!(parse_feed_xml(xml).is_err());
    }
}
