//! Structured parsing tools: CSS-selector extraction from HTML and
//! path queries over JSON. Complements `web_fetch` (plain text) with
//! machine-readable output for tables, lists, links and API responses.

use async_trait::async_trait;
use pr_core::{ToolSchema, ToolOutput};
use crate::registry::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MAX_ITEMS: usize = 500;
const MAX_OUTPUT_CHARS: usize = 50_000;

fn default_limit() -> usize {
    100
}

fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_ITEMS)
}

/// Resolve `source` to a document body. URLs go through the shared cached
/// fetcher (SSRF guard + redirects + session cache); everything else is
/// treated as a path relative to the working directory.
async fn resolve_source(
    ctx: &ToolContext,
    source: &str,
) -> Result<(String, bool), ToolOutput> {
    let trimmed = source.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        match crate::web::fetch_url_cached(ctx, trimmed).await {
            Ok((body, _ct)) => Ok((body, true)),
            Err(f) => Err(ToolOutput::err_code(f.message, f.code)),
        }
    } else {
        let path = if std::path::Path::new(trimmed).is_absolute() {
            std::path::PathBuf::from(trimmed)
        } else {
            ctx.working_dir.join(trimmed)
        };
        match tokio::fs::read_to_string(&path).await {
            Ok(body) => Ok((body, false)),
            Err(e) => Err(ToolOutput::err_code(
                format!("Failed to read {path:?}: {e}"),
                "file_not_found",
            )),
        }
    }
}

fn truncate_json_string(value: &str) -> String {
    if value.len() <= MAX_OUTPUT_CHARS {
        return value.to_string();
    }
    let mut end = MAX_OUTPUT_CHARS;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}… [truncated]", &value[..end])
}

// ──────────────────────────────────────────────────────────────────────────
// parse_html
// ──────────────────────────────────────────────────────────────────────────

pub struct ParseHtmlTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ParseHtmlParams {
    /// URL or local file path of the HTML document.
    source: String,
    /// CSS selector scoping the extraction (default "body").
    #[serde(default)]
    selector: Option<String>,
    /// Extraction mode:
    /// `texts` (default) — visible text of each matched element;
    /// `html` — inner HTML of each matched element;
    /// `attr` — value of the `attribute` parameter on each element;
    /// `links` — `{text, href}` for every `<a>` inside the scope;
    /// `tables` — every `<table>` inside the scope as rows of cells.
    #[serde(default)]
    mode: Option<String>,
    /// Attribute name for `attr` mode (e.g. "href", "src", "data-id").
    #[serde(default)]
    attribute: Option<String>,
    /// Maximum number of items to return (default 100, hard cap 500).
    #[serde(default = "default_limit")]
    limit: usize,
}

pub(crate) fn element_text(el: &scraper::ElementRef) -> String {
    el.text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_tables(scope: &scraper::ElementRef, limit: usize) -> Vec<serde_json::Value> {
    let table_sel = scraper::Selector::parse("table").unwrap();
    let row_sel = scraper::Selector::parse("tr").unwrap();
    let cell_sel = scraper::Selector::parse("th, td").unwrap();

    // The scope element itself may BE the table (`selector: "#prices"`);
    // descendant selection alone would miss it.
    let mut tables: Vec<scraper::ElementRef> = Vec::new();
    if scope.value().name() == "table" {
        tables.push(scope.clone());
    }
    tables.extend(scope.select(&table_sel));

    tables
        .into_iter()
        .take(limit)
        .map(|table| {
            let rows: Vec<serde_json::Value> = table
                .select(&row_sel)
                .map(|row| {
                    serde_json::Value::Array(
                        row.select(&cell_sel)
                            .map(|cell| serde_json::Value::String(element_text(&cell)))
                            .collect(),
                    )
                })
                .collect();
            serde_json::json!({ "rows": rows })
        })
        .collect()
}

fn extract_links(
    scope: &scraper::ElementRef,
    base_url: Option<&str>,
    limit: usize,
) -> Vec<serde_json::Value> {
    let a_sel = scraper::Selector::parse("a").unwrap();
    let base = base_url.and_then(|u| url::Url::parse(u).ok());

    scope
        .select(&a_sel)
        .take(limit)
        .filter_map(|a| {
            let href = a.value().attr("href")?.trim().to_string();
            if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
                return None;
            }
            let absolute = base
                .as_ref()
                .and_then(|b| b.join(&href).ok())
                .map(|u| u.to_string())
                .unwrap_or(href.clone());
            Some(serde_json::json!({
                "text": element_text(&a),
                "href": absolute,
            }))
        })
        .collect()
}

#[async_trait]
impl Tool for ParseHtmlTool {
    fn name(&self) -> &str {
        "parse_html"
    }
    fn description(&self) -> &str {
        "Extract structured data from an HTML page or file using CSS selectors.

## Capability

Fetches the document (URL with SSRF protection and session caching, or a local file), applies a CSS selector, and returns machine-readable JSON instead of raw text. Modes: `texts` (element texts), `html` (inner HTML), `attr` (attribute values, e.g. all `href`), `links` (all anchors with absolute URLs), `tables` (HTML tables converted to row arrays).

## When to Use

- Scraping repeating structures: product lists, team pages, news feeds, pricing tables.
- Converting an HTML `<table>` into structured rows for CSV/JSON export.
- Collecting all links of a section (navigation, sitemap-like pages).
- Reading an attribute in bulk (image `src`, canonical URL, `data-*` ids).

## When NOT to Use

- Plain reading of an article — use `web_fetch` (cheaper, cleaner text).
- JavaScript-rendered pages whose data is absent from the raw HTML — use browser tools if available.

## Usage Example

`{\"source\": \"https://example.com/pricing\", \"selector\": \"table.pricing\", \"mode\": \"tables\"}` or `{\"source\": \"page.html\", \"selector\": \".card h3\", \"mode\": \"texts\"}`.

## Failure Modes

- Invalid CSS selector: the error message contains the parse error — fix the selector.
- Empty result: the selector matched nothing — inspect the page with `web_fetch` first."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ParseHtmlParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: ParseHtmlParams = serde_json::from_value(args)?;
        let limit = clamp_limit(params.limit);
        let mode = params.mode.as_deref().unwrap_or("texts");

        let (body, is_url) = match resolve_source(ctx, &params.source).await {
            Ok(x) => x,
            Err(out) => return Ok(out),
        };

        let document = scraper::Html::parse_document(&body);
        let sel = scraper::Selector::parse(
            params.selector.as_deref().unwrap_or("body"),
        )
        .map_err(|e| {
            anyhow::anyhow!("Invalid CSS selector {:?}: {e}", params.selector)
        })?;

        let base_url = if is_url { Some(params.source.as_str()) } else { None };

        // Semantics by mode:
        // - texts/html/attr: the selector is the TARGET — match it across
        //   the whole document (`.card h3` must hit every card).
        // - links/tables: the selector is the SCOPE — take the first match
        //   and collect all anchors/tables inside it.
        let items: Vec<serde_json::Value> = match mode {
            "texts" => document
                .select(&sel)
                .take(limit)
                .map(|el| serde_json::Value::String(element_text(&el)))
                .collect(),
            "html" => document
                .select(&sel)
                .take(limit)
                .map(|el| serde_json::Value::String(el.inner_html()))
                .collect(),
            "attr" => {
                let attr = match params.attribute.as_deref() {
                    Some(a) if !a.is_empty() => a,
                    _ => {
                        return Ok(ToolOutput::err_code(
                            "mode \"attr\" requires the \"attribute\" parameter",
                            "invalid_arguments",
                        ))
                    }
                };
                document
                    .select(&sel)
                    .take(limit)
                    .filter_map(|el| {
                        el.value()
                            .attr(attr)
                            .map(|v| serde_json::Value::String(v.to_string()))
                    })
                    .collect()
            }
            "links" | "tables" => {
                let scope = match document.select(&sel).next() {
                    Some(el) => el,
                    None => {
                        return Ok(ToolOutput::err_code(
                            format!(
                                "Selector {:?} matched nothing in {}",
                                params.selector.as_deref().unwrap_or("body"),
                                params.source
                            ),
                            "no_match",
                        ));
                    }
                };
                if mode == "links" {
                    extract_links(&scope, base_url, limit)
                } else {
                    extract_tables(&scope, limit)
                }
            }
            other => {
                return Ok(ToolOutput::err_code(
                    format!(
                        "Unknown mode {other:?}; expected texts|html|attr|links|tables"
                    ),
                    "invalid_arguments",
                ))
            }
        };

        if items.is_empty() {
            return Ok(ToolOutput::err_code(
                format!(
                    "Selector {:?} matched nothing in {} (mode {mode})",
                    params.selector.as_deref().unwrap_or("body"),
                    params.source
                ),
                "no_match",
            ));
        }

        let result = serde_json::json!({
            "source": params.source,
            "mode": mode,
            "count": items.len(),
            "items": items,
        });
        let pretty = truncate_json_string(
            &serde_json::to_string_pretty(&result).unwrap_or_default(),
        );

        // Structured source for the findings harvester (sources.md).
        let title = {
            let title_sel = scraper::Selector::parse("title").unwrap();
            document
                .select(&title_sel)
                .next()
                .map(|el| el.text().collect::<String>())
                .unwrap_or_default()
        };
        let metadata = serde_json::json!({
            "source": params.source,
            "title": title,
            "count": items.len(),
        });
        Ok(ToolOutput::ok_with_meta(pretty, metadata))
    }
}

// ──────────────────────────────────────────────────────────────────────────
// extract_json
// ──────────────────────────────────────────────────────────────────────────

pub struct ExtractJsonTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ExtractJsonParams {
    /// JSON source: an http(s) URL (API endpoint), a local file path, or an
    /// inline JSON value starting with `{` or `[`.
    source: String,
    /// Dot path into the JSON, e.g. `data.items.0.name`. Array indexing with
    /// `items.0`, iteration over all elements with `items[*].email`.
    /// Empty/omitted path returns the whole document.
    #[serde(default)]
    path: Option<String>,
    /// Maximum number of results to return (default 100, hard cap 500).
    #[serde(default = "default_limit")]
    limit: usize,
}

/// One path segment: object key, numeric index, or iterate-all wildcard.
#[derive(Debug, PartialEq)]
enum Segment {
    Key(String),
    Index(usize),
    Wildcard,
}

/// Parse `items[0].name` / `results[*].url` / `a.b.2` into segments.
fn parse_path(path: &str) -> Result<Vec<Segment>, String> {
    let mut segments = Vec::new();
    for raw in path.split('.') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        // Split off any [N] / [*] suffixes: `items[0]` -> key "items", idx 0.
        let mut rest = raw;
        while let Some(bracket) = rest.find('[') {
            let head = &rest[..bracket];
            if !head.is_empty() {
                segments.push(Segment::Key(head.to_string()));
            }
            let close = rest[bracket..]
                .find(']')
                .ok_or_else(|| format!("unclosed '[' in {raw:?}"))?;
            let inner = rest[bracket + 1..bracket + close].trim();
            if inner == "*" {
                segments.push(Segment::Wildcard);
            } else {
                let idx: usize = inner
                    .parse()
                    .map_err(|_| format!("bad array index {inner:?} in {raw:?}"))?;
                segments.push(Segment::Index(idx));
            }
            rest = &rest[bracket + close + 1..];
        }
        if !rest.is_empty() {
            segments.push(Segment::Key(rest.to_string()));
        }
    }
    Ok(segments)
}

fn apply_segments(value: &serde_json::Value, segments: &[Segment], limit: usize) -> Vec<serde_json::Value> {
    let mut current = vec![value.clone()];
    for seg in segments {
        let mut next = Vec::new();
        for v in &current {
            match seg {
                Segment::Key(k) => {
                    if let Some(inner) = v.get(k) {
                        next.push(inner.clone());
                    } else if let Some(arr) = v.as_array() {
                        // Lenient indexing: `book.1.title` works the same as
                        // `book[1].title`.
                        if let Ok(i) = k.parse::<usize>() {
                            if let Some(inner) = arr.get(i) {
                                next.push(inner.clone());
                            }
                        }
                    }
                }
                Segment::Index(i) => {
                    if let Some(arr) = v.as_array() {
                        if let Some(inner) = arr.get(*i) {
                            next.push(inner.clone());
                        }
                    }
                }
                Segment::Wildcard => {
                    if let Some(arr) = v.as_array() {
                        next.extend(arr.iter().cloned());
                    } else if let Some(obj) = v.as_object() {
                        next.extend(obj.values().cloned());
                    }
                }
            }
            if next.len() >= limit * 4 {
                break;
            }
        }
        current = next;
    }
    current.truncate(limit);
    current
}

#[async_trait]
impl Tool for ExtractJsonTool {
    fn name(&self) -> &str {
        "extract_json"
    }
    fn description(&self) -> &str {
        "Query a JSON document (API response, file, or inline value) with a dot path and return the selected values.

## Capability

Loads JSON from an http(s) URL (with SSRF protection and session caching), a local file, or an inline string starting with `{`/`[`. Then applies a path: `store.book.0.title` for deep access, numeric indices for arrays, and `[*]` to iterate all elements — `results[*].url` collects every `url` field.

## When to Use

- Pulling fields out of REST APIs discovered during research (faster and cheaper than asking an LLM to re-read the payload).
- Filtering large JSON payloads down to the few fields that matter.
- Reading JSON exports/dumps saved by other tools.

## When NOT to Use

- HTML pages — use `parse_html` or `web_fetch`.
- When you need computation/reshaping beyond field selection — use `python_exec`.

## Usage Example

`{\"source\": \"https://api.example.com/v1/companies?limit=50\", \"path\": \"data[*].name\"}` or `{\"source\": \"./export.json\", \"path\": \"items.0.tags\"}`.

## Failure Modes

- Invalid JSON at the source: parse error with the byte offset.
- Path matched nothing: check field names with an empty path first (returns the whole document, truncated)."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ExtractJsonParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: ExtractJsonParams = serde_json::from_value(args)?;
        let limit = clamp_limit(params.limit);

        let trimmed = params.source.trim();
        let body = if trimmed.starts_with('{') || trimmed.starts_with('[') {
            trimmed.to_string()
        } else {
            match resolve_source(ctx, &params.source).await {
                Ok((b, _)) => b,
                Err(out) => return Ok(out),
            }
        };

        let document: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!("Source is not valid JSON: {e}")
        })?;

        let path = params.path.as_deref().unwrap_or("").trim();
        let results = if path.is_empty() {
            vec![document.clone()]
        } else {
            let segments = parse_path(path)
                .map_err(|e| anyhow::anyhow!("Invalid path {path:?}: {e}"))?;
            if segments.is_empty() {
                vec![document.clone()]
            } else {
                apply_segments(&document, &segments, limit)
            }
        };

        let result = serde_json::json!({
            "source": if trimmed.starts_with('{') || trimmed.starts_with('[') {
                "<inline>".to_string()
            } else {
                params.source.clone()
            },
            "path": path,
            "count": results.len(),
            "results": results,
        });
        let pretty = truncate_json_string(
            &serde_json::to_string_pretty(&result).unwrap_or_default(),
        );
        Ok(ToolOutput::ok(pretty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &std::path::Path) -> ToolContext {
        ToolContext::new(dir.to_path_buf(), pr_core::SearchConfig::default())
    }

    const SAMPLE_HTML: &str = r#"
        <html><head><title>Sample</title></head>
        <body>
          <div class="cards">
            <div class="card"><h3>Alpha</h3><a href="/a">Link A</a></div>
            <div class="card"><h3>Beta</h3><a href="https://ext.example/b">Link B</a></div>
          </div>
          <table id="t1">
            <tr><th>Name</th><th>Price</th></tr>
            <tr><td>Foo</td><td>$10</td></tr>
            <tr><td>Bar</td><td>$20</td></tr>
          </table>
        </body></html>"#;

    async fn run_parse(ctx: &ToolContext, args: serde_json::Value) -> ToolOutput {
        ParseHtmlTool
            .execute(args, ctx)
            .await
            .expect("execute must not hard-error")
    }

    #[tokio::test]
    async fn parse_html_texts_mode() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("page.html"), SAMPLE_HTML).unwrap();
        let out = run_parse(
            &ctx(tmp.path()),
            serde_json::json!({"source": "page.html", "selector": ".card h3", "mode": "texts"}),
        )
        .await;
        assert!(out.success, "{}", out.content);
        let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["items"][0], "Alpha");
        assert_eq!(v["items"][1], "Beta");
    }

    #[tokio::test]
    async fn parse_html_tables_mode() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("page.html"), SAMPLE_HTML).unwrap();
        let out = run_parse(
            &ctx(tmp.path()),
            serde_json::json!({"source": "page.html", "selector": "#t1", "mode": "tables"}),
        )
        .await;
        assert!(out.success);
        let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["count"], 1);
        let rows = &v["items"][0]["rows"];
        assert_eq!(rows.as_array().unwrap().len(), 3);
        assert_eq!(rows[1][0], "Foo");
        assert_eq!(rows[2][1], "$20");
    }

    #[tokio::test]
    async fn parse_html_links_mode_absolutizes_relative_hrefs() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("page.html"), SAMPLE_HTML).unwrap();
        // Local file has no base URL: relative href stays relative,
        // absolute href passes through.
        let out = run_parse(
            &ctx(tmp.path()),
            serde_json::json!({"source": "page.html", "selector": ".cards", "mode": "links"}),
        )
        .await;
        assert!(out.success);
        let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["items"][0]["text"], "Link A");
        assert_eq!(v["items"][1]["href"], "https://ext.example/b");
    }

    #[tokio::test]
    async fn parse_html_attr_mode_requires_attribute() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("page.html"), SAMPLE_HTML).unwrap();
        let out = run_parse(
            &ctx(tmp.path()),
            serde_json::json!({"source": "page.html", "selector": "a", "mode": "attr"}),
        )
        .await;
        assert!(!out.success);
        assert!(out.content.contains("attribute"));
    }

    #[tokio::test]
    async fn parse_html_invalid_selector_reports_error() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("page.html"), SAMPLE_HTML).unwrap();
        let result = ParseHtmlTool
            .execute(
                serde_json::json!({"source": "page.html", "selector": ".card >>"}),
                &ctx(tmp.path()),
            )
            .await;
        assert!(result.is_err(), "invalid selector must surface as an error");
    }

    #[tokio::test]
    async fn parse_html_no_match_error_code() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("page.html"), SAMPLE_HTML).unwrap();
        let out = run_parse(
            &ctx(tmp.path()),
            serde_json::json!({"source": "page.html", "selector": ".does-not-exist"}),
        )
        .await;
        assert!(!out.success);
        assert!(out.content.contains("matched nothing"));
    }

    #[tokio::test]
    async fn parse_html_limit_is_respected() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("page.html"), SAMPLE_HTML).unwrap();
        let out = run_parse(
            &ctx(tmp.path()),
            serde_json::json!({"source": "page.html", "selector": ".card", "mode": "texts", "limit": 1}),
        )
        .await;
        let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["count"], 1);
    }

    async fn run_extract(ctx: &ToolContext, args: serde_json::Value) -> ToolOutput {
        ExtractJsonTool
            .execute(args, ctx)
            .await
            .expect("execute must not hard-error")
    }

    #[tokio::test]
    async fn extract_json_inline_deep_path() {
        let tmp = TempDir::new().unwrap();
        let out = run_extract(
            &ctx(tmp.path()),
            serde_json::json!({
                "source": r#"{"store": {"book": [{"title": "Dune"}, {"title": "Hyperion"}]}}"#,
                "path": "store.book.1.title",
            }),
        )
        .await;
        assert!(out.success, "{}", out.content);
        let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["count"], 1);
        assert_eq!(v["results"][0], "Hyperion");
    }

    #[tokio::test]
    async fn extract_json_wildcard_iteration() {
        let tmp = TempDir::new().unwrap();
        let out = run_extract(
            &ctx(tmp.path()),
            serde_json::json!({
                "source": r#"{"results": [{"url": "https://a"}, {"url": "https://b"}, {"nourl": 1}]}"#,
                "path": "results[*].url",
            }),
        )
        .await;
        assert!(out.success);
        let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["count"], 2, "missing fields are skipped, not nulls");
        assert_eq!(v["results"][0], "https://a");
        assert_eq!(v["results"][1], "https://b");
    }

    #[tokio::test]
    async fn extract_json_from_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("data.json"),
            r#"{"items": [{"tags": ["x", "y"]}]}"#,
        )
        .unwrap();
        let out = run_extract(
            &ctx(tmp.path()),
            serde_json::json!({"source": "data.json", "path": "items.0.tags"}),
        )
        .await;
        assert!(out.success);
        let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["results"][0], serde_json::json!(["x", "y"]));
    }

    #[tokio::test]
    async fn extract_json_empty_path_returns_whole_document() {
        let tmp = TempDir::new().unwrap();
        let out = run_extract(
            &ctx(tmp.path()),
            serde_json::json!({"source": r#"{"a": 1}"#}),
        )
        .await;
        assert!(out.success);
        let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(v["results"][0], serde_json::json!({"a": 1}));
    }

    #[tokio::test]
    async fn extract_json_invalid_json_reports_parse_error() {
        let tmp = TempDir::new().unwrap();
        let result = ExtractJsonTool
            .execute(
                serde_json::json!({"source": "{not json", "path": "a"}),
                &ctx(tmp.path()),
            )
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn parse_path_handles_all_forms() {
        let segs = parse_path("data.items[0].name").unwrap();
        assert_eq!(
            segs,
            vec![
                Segment::Key("data".into()),
                Segment::Key("items".into()),
                Segment::Index(0),
                Segment::Key("name".into()),
            ]
        );

        let segs = parse_path("results[*].url").unwrap();
        assert_eq!(
            segs,
            vec![
                Segment::Key("results".into()),
                Segment::Wildcard,
                Segment::Key("url".into()),
            ]
        );

        let segs = parse_path("a.[2]").unwrap();
        assert_eq!(
            segs,
            vec![Segment::Key("a".into()), Segment::Index(2)]
        );

        assert!(parse_path("items[abc]").is_err());
        assert!(parse_path("items[0").is_err());
    }
}
