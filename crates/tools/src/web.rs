use async_trait::async_trait;
use pr_core::{ToolSchema, ToolOutput};
use crate::registry::{Tool, ToolContext};
use crate::search::SearchEngine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Hard cap on how many bytes `web_fetch` will download for a single body
/// (fleet report B15). The response stream is read in chunks and stopped as
/// soon as this limit is reached, so a huge or unbounded transfer can never
/// be pulled fully into memory before the output cap is applied.
const FETCH_MAX_BYTES: usize = 2 * 1024 * 1024;

// ─── Web Search ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WebSearchParams {
    /// Search query
    query: String,
    /// Maximum number of results (default: 10)
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 { 10 }

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }
    fn description(&self) -> &str {
        "Search the web for information using a query string. Returns a ranked list of results, each with a title, URL, and content snippet.

## Capability

Performs a web search across the open internet and returns up to `limit` results (default 10, max 50). Each result includes the page title, a direct URL, and a short text excerpt. Use this as the starting point for any research task — it discovers relevant sources before you fetch full page content.

## When to Use

- Finding information on a topic, event, person, or concept.
- Locating specific pages, documentation, or articles to read in full.
- Answering factual questions that require up-to-date information.
- Gathering multiple perspectives on a topic for cross-referencing.

## When NOT to Use

- Do NOT use `web_search` to read the full content of a known URL — use `web_fetch` instead.
- Do NOT use `web_search` for local file operations — use `file_read` or `grep`.
- If you already have specific URLs from a previous search, skip searching and go straight to `web_fetch`.

## Query Tips

- Use quotes for exact phrase matching: `\"quantum computing applications\"`.
- Include a year for time-sensitive queries: `\"AI regulation 2026\"`.
- Be specific — prefer `\"Rust async runtime tokio performance benchmarks\"` over `\"Rust performance\"`.
- Try 2-3 different query phrasings if initial results are poor.

## Failure Modes

- Empty results: the query may be too specific or use uncommon phrasing. Try broader terms.
- Irrelevant results: add more specific keywords or use quote-enclosed phrases.
- Rate limiting: if you see errors, wait before retrying. Do not spam rapid searches."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(WebSearchParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: WebSearchParams = serde_json::from_value(args)?;
        
        let engine = SearchEngine::new(ctx.search_config.clone());
        let results = engine.search(&params.query, params.limit).await;

        if results.is_empty() {
            return Ok(ToolOutput::ok(format!(
                "No results found for query: '{}'. Try a different search term.",
                params.query
            )));
        }

        let mut output = format!("Search results for '{}':\n\n", params.query);
        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}**\n   URL: {}\n   {}\n\n",
                i + 1, result.title, result.url, result.snippet
            ));
        }

        // Structured sources for the findings harvester (sources.md).
        let metadata = serde_json::json!({
            "query": params.query,
            "count": results.len(),
            "sources": results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "title": r.title,
                        "url": r.url,
                        "excerpt": r.snippet,
                    })
                })
                .collect::<Vec<_>>(),
        });
        Ok(ToolOutput::ok_with_meta(output, metadata))
    }
}

// ─── Web Fetch ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WebFetchParams {
    /// URL to fetch
    url: String,
    /// Extract only text content (default: true)
    #[serde(default = "default_true")]
    extract_text: bool,
}

fn default_true() -> bool { true }

pub struct WebFetchTool;

/// Structured fetch failure: message plus a stable machine code so callers
/// can map it onto `ToolOutput::err_code`.
pub(crate) struct FetchFailure {
    pub message: String,
    pub code: &'static str,
}

/// Fetch a URL with the shared session cache, SSRF guard, manual
/// redirect-following with per-hop re-validation, and a capped body read.
/// Shared by `web_fetch` and the structured parsing tools (`parse_html`,
/// `extract_json`) so every network read enforces the same safety policy.
pub(crate) async fn fetch_url_cached(
    ctx: &ToolContext,
    url: &str,
) -> Result<(String, String), FetchFailure> {
    // Session cache (fleet B15/B16): repeated fetches of the same URL
    // within the TTL are served from memory instead of the network.
    if let Some(cached) = ctx.fetch_cache.get(url) {
        return Ok((cached.0.clone(), cached.1.clone()));
    }

    // SSRF guard (fleet round 2): validate the URL and every
    // redirect hop against internal ranges; redirects are
    // followed manually with per-hop re-validation.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut current = url.to_string();
    let mut hops = 0usize;
    let response = loop {
        let validated = crate::guard::ensure_safe_url(&current)
            .await
            .map_err(|e| FetchFailure {
                message: format!("Refusing to fetch {current}: {e}"),
                code: "blocked",
            })?;
        let resp = client
            .get(validated.clone())
            .header("User-Agent", "Mozilla/5.0 (compatible; ParallelResearch/0.1)")
            .send()
            .await
            .map_err(|e| FetchFailure {
                message: format!("Failed to fetch {url}: {e}"),
                code: "network",
            })?;
        if resp.status().is_redirection() {
            hops += 1;
            if hops > crate::guard::MAX_REDIRECTS {
                return Err(FetchFailure {
                    message: format!("Too many redirects fetching {url}"),
                    code: "too_many_redirects",
                });
            }
            let loc = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| FetchFailure {
                    message: format!("Redirect without Location header from {current}"),
                    code: "http_error",
                })?
                .to_string();
            current = crate::guard::resolve_redirect(&validated, &loc)
                .map_err(|e| FetchFailure {
                    message: e,
                    code: "blocked",
                })?
                .to_string();
            continue;
        }
        break resp;
    };

    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Fleet B15: bound the download itself — read at most
    // FETCH_MAX_BYTES in chunks and stop, instead of buffering
    // the entire response body via `.text()` before truncation.
    let body = read_body_capped(response, FETCH_MAX_BYTES)
        .await
        .map_err(|e| FetchFailure {
            message: format!("Failed to read body of {url}: {e}"),
            code: "network",
        })?;

    if !status.is_success() {
        let code = match status.as_u16() {
            401 | 403 => "blocked",
            404 | 410 => "not_found",
            429 => "rate_limited",
            408 | 504 => "timeout",
            _ => "http_error",
        };
        return Err(FetchFailure {
            message: format!("Failed to fetch {url}: HTTP {status}"),
            code,
        });
    }

    // Only successful responses are cached — an error response
    // should stay retriable and never get pinned in the cache.
    ctx.fetch_cache.insert(url, body.clone(), content_type.clone());

    Ok((body, content_type))
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }
    fn description(&self) -> &str {
        "Fetch a web page by URL and return its text content. Converts HTML to readable plain text.

## Capability

Downloads a web page from the given URL, strips HTML markup, navigation, scripts, and styles, and returns clean readable text. Content is truncated at 50,000 characters for very long pages. The response includes the source URL and page title at the top.

## When to Use

- Reading the full content of a page discovered via `web_fetch` or `web_search`.
- Fetching documentation, articles, blog posts, or API references.
- Extracting specific data points, quotes, or facts from a known URL.
- Verifying information by reading primary sources directly.

## When NOT to Use

- Do NOT use `web_fetch` to discover new pages — use `web_search` first.
- Do NOT use `web_fetch` for local files — use `file_read` instead.
- Do NOT use `web_fetch` for binary files (PDFs, images, videos) — it only handles HTML/text.

## Usage Example

Call with a single `url` parameter. Optionally set `extract_text: false` to get raw HTML instead of cleaned text (rarely needed).

## Failure Modes

- `HTTP 403`: the site blocks automated access. Try a different source or search for a cached version.
- `HTTP 404`: the page does not exist. The URL may be outdated — try searching for the topic.
- `HTTP 429`: rate limited. Wait before retrying.
- Timeout: the site is slow or unreachable. Try again later or find an alternative source.
- Very long content is truncated at 50,000 characters. If you need more, note this in your findings."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(WebFetchParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: WebFetchParams = serde_json::from_value(args)?;

        let (body, content_type) = match fetch_url_cached(ctx, &params.url).await {
            Ok(x) => x,
            Err(f) => return Ok(ToolOutput::err_code(f.message, f.code)),
        };

        let (text, page_title) = if params.extract_text && content_type.contains("html") {
            html_to_text_with_title(&body, &params.url)
        } else {
            (body, String::new())
        };

        // Truncate very long content (char-boundary safe)
        let max_chars = 50_000;
        let text = if text.len() > max_chars {
            let mut end = max_chars;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...\n\n[Content truncated at {} characters]", &text[..end], max_chars)
        } else {
            text
        };

        // Frame the page as untrusted data and flag known injection patterns
        // (fleet report D1) before the content enters the LLM context.
        let (wrapped, hits) = crate::injection::scan_and_wrap(&text);
        let mut meta = serde_json::json!({
            "url": params.url,
            "title": page_title,
        });
        if !hits.is_empty() {
            meta["injection_hits"] = serde_json::json!(hits);
        }
        let mut out = ToolOutput::ok(wrapped);
        out.metadata = Some(meta);
        Ok(out)
    }
}

/// Read a response body in chunks, stopping once `max_bytes` have been
/// collected (fleet report B15). This bounds memory use and transfer time
/// regardless of how large the remote resource actually is.
///
/// Note: unlike `Response::text()`, which honors the response's declared
/// charset, the collected bytes are decoded as UTF-8 (lossy) — the content
/// is treated as untrusted input downstream anyway.
async fn read_body_capped(response: reqwest::Response, max_bytes: usize) -> anyhow::Result<String> {
    let mut response = response;
    let mut buf: Vec<u8> = Vec::new();
    while buf.len() < max_bytes {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let take = chunk.len().min(max_bytes - buf.len());
                buf.extend_from_slice(&chunk[..take]);
            }
            Ok(None) => break, // EOF
            Err(e) => return Err(e.into()),
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
fn html_to_text(html: &str, url: &str) -> String {
    html_to_text_with_title(html, url).0
}

/// Same as [`html_to_text`] but also returns the extracted page title so
/// callers (findings harvester) can record it as structured metadata.
fn html_to_text_with_title(html: &str, url: &str) -> (String, String) {
    let document = scraper::Html::parse_document(html);

    let title_sel = scraper::Selector::parse("title").unwrap();
    let title = document.select(&title_sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default();

    let body_sel = scraper::Selector::parse("body").unwrap();
    let text = document.select(&body_sel)
        .next()
        .map(|body| {
            let mut text = String::new();
            extract_text_recursive(&body, &mut text);
            text
        })
        .unwrap_or_else(|| html.to_string());

    let text = text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    (format!("Source: {url}\nTitle: {title}\n\n{text}"), title)
}

fn extract_text_recursive(element: &scraper::ElementRef, output: &mut String) {
    let tag = element.value().name();
    if matches!(tag, "script" | "style" | "nav" | "footer" | "header" | "noscript") {
        return;
    }

    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            let t = text.trim();
            if !t.is_empty() {
                output.push_str(t);
                output.push(' ');
            }
        } else if let Some(child_el) = scraper::ElementRef::wrap(child) {
            extract_text_recursive(&child_el, output);
        }
    }

    if matches!(tag, "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "br" | "tr") {
        output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── default helpers ───

    #[test]
    fn test_default_limit_is_10() {
        assert_eq!(default_limit(), 10);
    }

    #[test]
    fn test_default_true_is_true() {
        assert_eq!(default_true(), true);
    }

    // ─── tool schema names ───

    #[test]
    fn test_web_fetch_tool_schema_name() {
        let tool = WebFetchTool;
        let schema = tool.schema();
        assert_eq!(schema.name, "web_fetch");
    }

    #[test]
    fn test_web_search_tool_schema_name() {
        let tool = WebSearchTool;
        let schema = tool.schema();
        assert_eq!(schema.name, "web_search");
    }

    #[test]
    fn test_web_fetch_tool_trait_name() {
        let tool = WebFetchTool;
        assert_eq!(tool.name(), "web_fetch");
    }

    #[test]
    fn test_web_search_tool_trait_name() {
        let tool = WebSearchTool;
        assert_eq!(tool.name(), "web_search");
    }

    #[test]
    fn test_html_to_text_with_title_returns_both() {
        let html = r#"<html><head><title>My Title</title></head><body><p>Body text</p></body></html>"#;
        let (text, title) = html_to_text_with_title(html, "https://example.com");
        assert_eq!(title, "My Title");
        assert!(text.contains("Body text"));
        assert!(text.contains("Title: My Title"));
    }

    #[test]
    fn test_html_to_text_with_title_empty_when_missing() {
        let html = r#"<html><body><p>No title here</p></body></html>"#;
        let (_, title) = html_to_text_with_title(html, "https://example.com");
        assert!(title.is_empty());
    }

    // ─── html_to_text ───

    #[test]
    fn test_html_to_text_basic() {
        let html = r#"<html><head><title>Test Page</title></head><body><p>Hello world</p></body></html>"#;
        let result = html_to_text(html, "https://example.com");
        assert!(result.contains("Test Page"), "should contain the page title");
        assert!(result.contains("Hello world"), "should contain body text");
        assert!(
            result.contains("Source: https://example.com"),
            "should contain the source URL"
        );
    }

    #[test]
    fn test_html_to_text_strips_script_tags() {
        let html = r#"<html><body><script>alert('xss')</script><p>Safe text</p></body></html>"#;
        let result = html_to_text(html, "https://example.com");
        assert!(!result.contains("alert"), "script content must be stripped");
        assert!(result.contains("Safe text"), "visible text must remain");
    }

    #[test]
    fn test_html_to_text_strips_style_tags() {
        let html = r#"<html><body><style>body { color: red; }</style><p>Visible</p></body></html>"#;
        let result = html_to_text(html, "https://example.com");
        assert!(!result.contains("color: red"), "style content must be stripped");
        assert!(result.contains("Visible"), "visible text must remain");
    }

    #[test]
    fn test_html_to_text_strips_nav_footer_header_noscript() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Page</title></head>
<body>
  <nav>Navigation link</nav>
  <header>Header text</header>
  <p>Main content</p>
  <footer>Footer text</footer>
  <noscript>Please enable JavaScript</noscript>
</body>
</html>"#;
        let result = html_to_text(html, "https://example.com");
        assert!(result.contains("Main content"), "main content must survive");
        assert!(
            !result.contains("Navigation link"),
            "nav content must be stripped"
        );
        assert!(
            !result.contains("Header text"),
            "header content must be stripped"
        );
        assert!(
            !result.contains("Footer text"),
            "footer content must be stripped"
        );
        assert!(
            !result.contains("enable JavaScript"),
            "noscript content must be stripped"
        );
    }

    #[test]
    fn test_html_to_text_nested_elements() {
        let html = r#"<html>
<head><title>Nested</title></head>
<body>
  <div>
    <h1>Title</h1>
    <div>
      <p>Paragraph <strong>bold</strong> and <em>italic</em></p>
      <ul>
        <li>Item one</li>
        <li>Item two</li>
      </ul>
    </div>
  </div>
</body>
</html>"#;
        let result = html_to_text(html, "https://example.com");
        assert!(result.contains("Title"), "h1 text must appear");
        assert!(result.contains("Paragraph"), "paragraph text must appear");
        assert!(result.contains("bold"), "strong text must appear");
        assert!(result.contains("italic"), "em text must appear");
        assert!(result.contains("Item one"), "first list item must appear");
        assert!(result.contains("Item two"), "second list item must appear");
    }

    #[test]
    fn test_html_to_text_empty_html() {
        let html = "";
        let result = html_to_text(html, "https://example.com");
        // Even with empty input, the function should produce the framing header
        assert!(
            result.contains("Source: https://example.com"),
            "source URL must always appear"
        );
        assert!(
            result.contains("Title:"),
            "Title: label must always appear"
        );
    }

    #[test]
    fn test_html_to_text_multiple_paragraphs() {
        let html = r#"<html>
<head><title>Multi</title></head>
<body>
  <p>First paragraph.</p>
  <p>Second paragraph.</p>
  <p>Third paragraph.</p>
</body>
</html>"#;
        let result = html_to_text(html, "https://example.com");
        assert!(result.contains("First paragraph."));
        assert!(result.contains("Second paragraph."));
        assert!(result.contains("Third paragraph."));
    }

    #[test]
    fn test_html_to_text_title_extraction() {
        let html = r#"<html><head><title>My Special Title</title></head><body><p>Body</p></body></html>"#;
        let result = html_to_text(html, "https://test.org");
        assert!(
            result.contains("Title: My Special Title"),
            "title should appear in Title: line"
        );
    }

    #[test]
    fn test_html_to_text_no_body_tag_uses_scraper_default() {
        // scraper::Html::parse_document always synthesizes a <body>, so even
        // without an explicit body tag the body branch is taken (not the raw
        // fallback). The text inside the <p> should still be extracted.
        let html = "<p>No body wrapper</p>";
        let result = html_to_text(html, "https://example.com");
        assert!(result.contains("Source: https://example.com"));
        assert!(result.contains("No body wrapper"), "text inside <p> must be extracted");
    }

    // ─── extract_text_recursive ───

    #[test]
    fn test_extract_text_recursive_simple_paragraph() {
        let html = "<p>Hello world</p>";
        let document = scraper::Html::parse_document(html);
        let sel = scraper::Selector::parse("p").unwrap();
        let element = document.select(&sel).next().unwrap();
        let mut output = String::new();
        extract_text_recursive(&element, &mut output);
        assert!(output.contains("Hello world"));
    }

    #[test]
    fn test_extract_text_recursive_skips_script() {
        let html = "<div><script>evil();</script><p>good</p></div>";
        let document = scraper::Html::parse_document(html);
        let sel = scraper::Selector::parse("div").unwrap();
        let element = document.select(&sel).next().unwrap();
        let mut output = String::new();
        extract_text_recursive(&element, &mut output);
        assert!(!output.contains("evil"), "script text must be skipped");
        assert!(output.contains("good"), "non-script text must remain");
    }

    #[test]
    fn test_extract_text_recursive_skips_style() {
        let html = "<div><style>.x{color:red}</style><p>text</p></div>";
        let document = scraper::Html::parse_document(html);
        let sel = scraper::Selector::parse("div").unwrap();
        let element = document.select(&sel).next().unwrap();
        let mut output = String::new();
        extract_text_recursive(&element, &mut output);
        assert!(!output.contains("color"), "style text must be skipped");
        assert!(output.contains("text"));
    }

    #[test]
    fn test_extract_text_recursive_adds_newlines_for_block_elements() {
        let html = "<div><p>First</p><p>Second</p></div>";
        let document = scraper::Html::parse_document(html);
        let sel = scraper::Selector::parse("div").unwrap();
        let element = document.select(&sel).next().unwrap();
        let mut output = String::new();
        extract_text_recursive(&element, &mut output);
        // Each <p> appends a '\n', so there should be newlines in the output
        assert!(output.contains('\n'), "block elements should produce newlines");
        assert!(output.contains("First"));
        assert!(output.contains("Second"));
    }

    #[test]
    fn test_extract_text_recursive_nested_span() {
        let html = "<p><span>inner text</span></p>";
        let document = scraper::Html::parse_document(html);
        let sel = scraper::Selector::parse("p").unwrap();
        let element = document.select(&sel).next().unwrap();
        let mut output = String::new();
        extract_text_recursive(&element, &mut output);
        assert!(output.contains("inner text"), "span text must be extracted");
    }

    #[test]
    fn test_extract_text_recursive_empty_element() {
        let html = "<p></p>";
        let document = scraper::Html::parse_document(html);
        let sel = scraper::Selector::parse("p").unwrap();
        let element = document.select(&sel).next().unwrap();
        let mut output = String::new();
        extract_text_recursive(&element, &mut output);
        // Empty element should just produce a trailing newline from the <p> tag
        let trimmed = output.trim();
        assert!(trimmed.is_empty(), "empty element should produce no visible text");
    }

    // ─── serde round-trips for param structs ───

    #[test]
    fn test_web_fetch_params_defaults() {
        let json = r#"{"url": "https://example.com"}"#;
        let params: WebFetchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.url, "https://example.com");
        assert!(params.extract_text, "extract_text should default to true");
    }

    #[test]
    fn test_web_search_params_defaults() {
        let json = r#"{"query": "rust async"}"#;
        let params: WebSearchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.query, "rust async");
        assert_eq!(params.limit, 10, "limit should default to 10");
    }

    #[test]
    fn test_web_fetch_params_explicit_values() {
        let json = r#"{"url": "https://example.com", "extract_text": false}"#;
        let params: WebFetchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.url, "https://example.com");
        assert!(!params.extract_text);
    }

    #[test]
    fn test_web_search_params_explicit_limit() {
        let json = r#"{"query": "test", "limit": 25}"#;
        let params: WebSearchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.limit, 25);
    }
}
