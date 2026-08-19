use pr_core::SearchConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct SearchEngine {
    config: SearchConfig,
    http: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Maximum snippet length kept per result (avoids huge payloads from raw page text).
const MAX_SNIPPET_CHARS: usize = 1_000;

/// Reciprocal-rank-fusion constant (the standard RRF value).
const RRF_K: f64 = 60.0;

impl SearchEngine {
    pub fn new(config: SearchConfig) -> Self {
        Self {
            config,
            http: pr_core::http_client(),
        }
    }

    /// Run a search using the configured backend.
    ///
    /// Backends: `linkup`, `exa`, `tavily`, `serper`, `brave`, `parallel`,
    /// `hybrid` (try configured backends in priority order, first non-empty
    /// wins), `smart` (run all configured backends in parallel, dedupe and
    /// rank), or anything else → DuckDuckGo fallback.
    pub async fn search(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        match self.config.backend.as_str() {
            "linkup" => self.search_linkup(query, limit).await,
            "exa" => self.search_exa(query, limit).await,
            "tavily" => self.search_tavily(query, limit).await,
            "serper" => self.search_serper(query, limit).await,
            "brave" => self.search_brave(query, limit).await,
            "parallel" => self.search_parallel(query, limit).await,
            "smart" => self.smart_search(query, limit).await,
            "hybrid" => self.search_hybrid(query, limit).await,
            _ => self.search_duckduckgo(query, limit).await,
        }
    }

    /// Hybrid search: try configured backends in priority order and return the
    /// first non-empty result set.
    ///
    /// Order: Linkup → Exa → Tavily → Serper → Brave → Parallel.ai → DuckDuckGo.
    async fn search_hybrid(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        if let Some(ref c) = self.config.linkup {
            let results = self.search_linkup_with_key(query, limit, &c.api_key).await;
            if !results.is_empty() {
                return results;
            }
        }
        if let Some(ref c) = self.config.exa {
            let results = self.search_exa_with_key(query, limit, &c.api_key).await;
            if !results.is_empty() {
                return results;
            }
        }
        if let Some(ref c) = self.config.tavily {
            let results = self.search_tavily_with_key(query, limit, &c.api_key).await;
            if !results.is_empty() {
                return results;
            }
        }
        if let Some(ref c) = self.config.serper {
            let results = self.search_serper_with_key(query, limit, &c.api_key).await;
            if !results.is_empty() {
                return results;
            }
        }
        if let Some(ref c) = self.config.brave {
            let results = self.search_brave_with_key(query, limit, &c.api_key).await;
            if !results.is_empty() {
                return results;
            }
        }
        if let Some(ref c) = self.config.parallel {
            let results = self.search_parallel_with_key(query, limit, &c.api_key).await;
            if !results.is_empty() {
                return results;
            }
        }
        self.search_duckduckgo(query, limit).await
    }

    /// Smart search: run every configured backend in parallel, deduplicate
    /// results by (normalized) URL and rank them with reciprocal rank fusion,
    /// so results returned by multiple backends score higher. Falls back to
    /// DuckDuckGo when no configured backend produced anything.
    pub async fn smart_search(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        // Each future resolves to that backend's results, or an empty vec when
        // the backend is not configured. All run concurrently via `tokio::join!`.
        let linkup_fut = async {
            match self.config.linkup.as_ref() {
                Some(c) => self.search_linkup_with_key(query, limit, &c.api_key).await,
                None => Vec::new(),
            }
        };
        let exa_fut = async {
            match self.config.exa.as_ref() {
                Some(c) => self.search_exa_with_key(query, limit, &c.api_key).await,
                None => Vec::new(),
            }
        };
        let tavily_fut = async {
            match self.config.tavily.as_ref() {
                Some(c) => self.search_tavily_with_key(query, limit, &c.api_key).await,
                None => Vec::new(),
            }
        };
        let serper_fut = async {
            match self.config.serper.as_ref() {
                Some(c) => self.search_serper_with_key(query, limit, &c.api_key).await,
                None => Vec::new(),
            }
        };
        let brave_fut = async {
            match self.config.brave.as_ref() {
                Some(c) => self.search_brave_with_key(query, limit, &c.api_key).await,
                None => Vec::new(),
            }
        };
        let parallel_fut = async {
            match self.config.parallel.as_ref() {
                Some(c) => self.search_parallel_with_key(query, limit, &c.api_key).await,
                None => Vec::new(),
            }
        };

        let (linkup_r, exa_r, tavily_r, serper_r, brave_r, parallel_r) = tokio::join!(
            linkup_fut, exa_fut, tavily_fut, serper_fut, brave_fut, parallel_fut
        );

        let sources: Vec<(&str, Vec<SearchResult>)> = vec![
            ("linkup", linkup_r),
            ("exa", exa_r),
            ("tavily", tavily_r),
            ("serper", serper_r),
            ("brave", brave_r),
            ("parallel", parallel_r),
        ];

        let merged = merge_and_rank(&sources, limit);
        if merged.is_empty() {
            // No API backend returned anything — fall back to DuckDuckGo.
            return self.search_duckduckgo(query, limit).await;
        }
        merged
    }

    // ─── Linkup ───

    async fn search_linkup(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        if let Some(ref linkup) = self.config.linkup {
            self.search_linkup_with_key(query, limit, &linkup.api_key).await
        } else {
            vec![]
        }
    }

    async fn search_linkup_with_key(&self, query: &str, limit: u32, api_key: &str) -> Vec<SearchResult> {
        let url = "https://api.linkup.so/v1/search";

        let body = serde_json::json!({
            "q": query,
            "depth": "standard",
            "outputType": "searchResults",
            "includeImages": false,
        });

        let response = self.http
            .post(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<LinkupResponse>().await {
                    Ok(linkup_resp) => parse_linkup_response(linkup_resp, limit),
                    Err(e) => {
                        tracing::warn!("Linkup response parse error: {e}");
                        vec![]
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!("Linkup search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Linkup request error: {e}");
                vec![]
            }
        }
    }

    // ─── Exa ───

    async fn search_exa(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        if let Some(ref exa) = self.config.exa {
            self.search_exa_with_key(query, limit, &exa.api_key).await
        } else {
            vec![]
        }
    }

    async fn search_exa_with_key(&self, query: &str, limit: u32, api_key: &str) -> Vec<SearchResult> {
        let body = serde_json::json!({
            "query": query,
            "numResults": limit,
            "type": "auto",
        });

        let response = self.http
            .post("https://api.exa.ai/search")
            .header("x-api-key", api_key)
            .json(&body)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(value) => parse_exa_response(&value, limit),
                    Err(e) => {
                        tracing::warn!("Exa response parse error: {e}");
                        vec![]
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!("Exa search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Exa request error: {e}");
                vec![]
            }
        }
    }

    // ─── Tavily ───

    async fn search_tavily(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        if let Some(ref tavily) = self.config.tavily {
            self.search_tavily_with_key(query, limit, &tavily.api_key).await
        } else {
            vec![]
        }
    }

    async fn search_tavily_with_key(&self, query: &str, limit: u32, api_key: &str) -> Vec<SearchResult> {
        let body = serde_json::json!({
            "query": query,
            "max_results": limit,
            "include_answer": true,
        });

        let response = self.http
            .post("https://api.tavily.com/search")
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(value) => parse_tavily_response(&value, limit),
                    Err(e) => {
                        tracing::warn!("Tavily response parse error: {e}");
                        vec![]
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!("Tavily search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Tavily request error: {e}");
                vec![]
            }
        }
    }

    // ─── Serper ───

    async fn search_serper(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        if let Some(ref serper) = self.config.serper {
            self.search_serper_with_key(query, limit, &serper.api_key).await
        } else {
            vec![]
        }
    }

    async fn search_serper_with_key(&self, query: &str, limit: u32, api_key: &str) -> Vec<SearchResult> {
        let body = serde_json::json!({
            "q": query,
            "num": limit,
        });

        let response = self.http
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", api_key)
            .json(&body)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(value) => parse_serper_response(&value, limit),
                    Err(e) => {
                        tracing::warn!("Serper response parse error: {e}");
                        vec![]
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!("Serper search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Serper request error: {e}");
                vec![]
            }
        }
    }

    // ─── Brave ───

    async fn search_brave(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        if let Some(ref brave) = self.config.brave {
            self.search_brave_with_key(query, limit, &brave.api_key).await
        } else {
            vec![]
        }
    }

    async fn search_brave_with_key(&self, query: &str, limit: u32, api_key: &str) -> Vec<SearchResult> {
        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding::encode(query),
            limit
        );

        let response = self.http
            .get(&url)
            .header("X-Subscription-Token", api_key)
            .header("Accept", "application/json")
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(value) => parse_brave_response(&value, limit),
                    Err(e) => {
                        tracing::warn!("Brave response parse error: {e}");
                        vec![]
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!("Brave search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Brave request error: {e}");
                vec![]
            }
        }
    }

    // ─── Parallel.ai ───

    async fn search_parallel(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        if let Some(ref parallel) = self.config.parallel {
            self.search_parallel_with_key(query, limit, &parallel.api_key).await
        } else {
            vec![]
        }
    }

    async fn search_parallel_with_key(&self, query: &str, limit: u32, api_key: &str) -> Vec<SearchResult> {
        // Parallel.ai web search API
        let url = format!(
            "https://api.parallel.ai/v1/web/search?q={}&limit={}",
            urlencoding::encode(query),
            limit
        );

        let response = self.http
            .get(&url)
            .header("x-api-key", api_key)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<ParallelResponse>().await {
                    Ok(parallel_resp) => {
                        parallel_resp.results.unwrap_or_default().into_iter()
                            .take(limit as usize)
                            .map(|r| SearchResult {
                                title: r.title.unwrap_or_default(),
                                url: r.url.unwrap_or_default(),
                                snippet: r.snippet.unwrap_or_default(),
                            })
                            .collect()
                    }
                    Err(e) => {
                        tracing::warn!("Parallel.ai response parse error: {e}");
                        vec![]
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!("Parallel.ai search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Parallel.ai request error: {e}");
                vec![]
            }
        }
    }

    // ─── DuckDuckGo (no key required) ───

    async fn search_duckduckgo(&self, query: &str, limit: u32) -> Vec<SearchResult> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let response = self.http
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (compatible; ParallelResearch/0.1)")
            .send()
            .await;

        match response {
            Ok(resp) => {
                match resp.text().await {
                    Ok(html) => parse_duckduckgo_results(&html, limit),
                    Err(_) => vec![],
                }
            }
            Err(_) => vec![],
        }
    }
}

// ─── Response types ───

// Linkup API response types
#[derive(Debug, Deserialize)]
struct LinkupResponse {
    #[serde(default)]
    results: Vec<LinkupResult>,
}

#[derive(Debug, Deserialize)]
struct LinkupResult {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    result_type: Option<String>,
    title: Option<String>,
    url: Option<String>,
    content: Option<String>,
}

fn parse_linkup_response(resp: LinkupResponse, limit: u32) -> Vec<SearchResult> {
    resp.results.into_iter()
        .take(limit as usize)
        .map(|r| SearchResult {
            title: r.title.unwrap_or_default(),
            url: r.url.unwrap_or_default(),
            snippet: truncate_chars(&r.content.unwrap_or_default(), MAX_SNIPPET_CHARS),
        })
        .collect()
}

// Exa API response types
#[derive(Debug, Deserialize)]
struct ExaResponse {
    #[serde(default)]
    results: Vec<ExaResult>,
}

#[derive(Debug, Deserialize)]
struct ExaResult {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    /// Full page text (Exa returns it when contents are requested).
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    snippet: Option<String>,
    #[serde(default)]
    highlights: Option<Vec<String>>,
}

fn parse_exa_response(value: &serde_json::Value, limit: u32) -> Vec<SearchResult> {
    let parsed: ExaResponse = match serde_json::from_value(value.clone()) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    parsed.results.into_iter()
        .filter(|r| r.url.as_deref().map(|u| !u.trim().is_empty()).unwrap_or(false))
        .take(limit as usize)
        .map(|r| {
            let snippet = r.snippet
                .or_else(|| r.highlights.and_then(|h| h.into_iter().next()))
                .or(r.text)
                .unwrap_or_default();
            SearchResult {
                title: r.title.unwrap_or_default(),
                url: r.url.unwrap_or_default(),
                snippet: truncate_chars(&snippet, MAX_SNIPPET_CHARS),
            }
        })
        .collect()
}

// Tavily API response types
#[derive(Debug, Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    answer: Option<String>,
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    content: Option<String>,
}

fn parse_tavily_response(value: &serde_json::Value, limit: u32) -> Vec<SearchResult> {
    let parsed: TavilyResponse = match serde_json::from_value(value.clone()) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    let mut results: Vec<SearchResult> = parsed.results.into_iter()
        .filter(|r| r.url.as_deref().map(|u| !u.trim().is_empty()).unwrap_or(false))
        .take(limit as usize)
        .map(|r| SearchResult {
            title: r.title.unwrap_or_default(),
            url: r.url.unwrap_or_default(),
            snippet: truncate_chars(&r.content.unwrap_or_default(), MAX_SNIPPET_CHARS),
        })
        .collect();

    // Surface the synthesized answer first when Tavily produced one.
    if let Some(answer) = parsed.answer {
        let answer = answer.trim().to_string();
        if !answer.is_empty() {
            let url = results.first().map(|r| r.url.clone()).unwrap_or_default();
            results.insert(0, SearchResult {
                title: "Tavily answer".to_string(),
                url,
                snippet: truncate_chars(&answer, MAX_SNIPPET_CHARS),
            });
            results.truncate(limit as usize);
        }
    }
    results
}

// Serper (Google SERP) response types
#[derive(Debug, Deserialize)]
struct SerperResponse {
    #[serde(default)]
    organic: Vec<SerperResult>,
    #[serde(default, rename = "answerBox")]
    answer_box: Option<SerperAnswerBox>,
}

#[derive(Debug, Deserialize)]
struct SerperResult {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    snippet: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerperAnswerBox {
    #[serde(default)]
    answer: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    snippet: Option<String>,
}

fn parse_serper_response(value: &serde_json::Value, limit: u32) -> Vec<SearchResult> {
    let parsed: SerperResponse = match serde_json::from_value(value.clone()) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    let mut results: Vec<SearchResult> = parsed.organic.into_iter()
        .filter(|r| r.link.as_deref().map(|u| !u.trim().is_empty()).unwrap_or(false))
        .take(limit as usize)
        .map(|r| SearchResult {
            title: r.title.unwrap_or_default(),
            url: r.link.unwrap_or_default(),
            snippet: truncate_chars(&r.snippet.unwrap_or_default(), MAX_SNIPPET_CHARS),
        })
        .collect();

    // Google's answer box (when present) is the most direct answer — put it first.
    if let Some(box_) = parsed.answer_box {
        let answer = box_.answer.or(box_.snippet).unwrap_or_default();
        if !answer.trim().is_empty() {
            results.insert(0, SearchResult {
                title: box_.title.unwrap_or_else(|| "Direct answer".to_string()),
                url: box_.link.unwrap_or_default(),
                snippet: truncate_chars(&answer, MAX_SNIPPET_CHARS),
            });
            results.truncate(limit as usize);
        }
    }
    results
}

// Brave Search API response types
#[derive(Debug, Deserialize)]
struct BraveResponse {
    #[serde(default)]
    web: Option<BraveWeb>,
}

#[derive(Debug, Deserialize)]
struct BraveWeb {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

fn parse_brave_response(value: &serde_json::Value, limit: u32) -> Vec<SearchResult> {
    let parsed: BraveResponse = match serde_json::from_value(value.clone()) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    parsed.web
        .map(|w| w.results)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.url.as_deref().map(|u| !u.trim().is_empty()).unwrap_or(false))
        .take(limit as usize)
        .map(|r| SearchResult {
            title: r.title.unwrap_or_default(),
            url: r.url.unwrap_or_default(),
            snippet: truncate_chars(&r.description.unwrap_or_default(), MAX_SNIPPET_CHARS),
        })
        .collect()
}

// Parallel.ai API response types
#[derive(Debug, Deserialize)]
struct ParallelResponse {
    #[serde(default)]
    results: Option<Vec<ParallelResult>>,
}

#[derive(Debug, Deserialize)]
struct ParallelResult {
    title: Option<String>,
    url: Option<String>,
    snippet: Option<String>,
}

fn parse_duckduckgo_results(html: &str, limit: u32) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let document = scraper::Html::parse_document(html);

    let result_selector = scraper::Selector::parse(".result").unwrap();
    let title_selector = scraper::Selector::parse(".result__a").unwrap();
    let snippet_selector = scraper::Selector::parse(".result__snippet").unwrap();

    for element in document.select(&result_selector).take(limit as usize) {
        let title = element.select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default();

        let url = element.select(&title_selector)
            .next()
            .and_then(|el| el.value().attr("href"))
            .map(|href| {
                if href.starts_with("//duckduckgo.com/l/?uddg=") {
                    let encoded = href.trim_start_matches("//duckduckgo.com/l/?uddg=");
                    urlencoding::decode(encoded).map(|s| s.into_owned()).unwrap_or_else(|_| href.to_string())
                } else if href.starts_with("http") {
                    href.to_string()
                } else {
                    format!("https:{href}")
                }
            })
            .unwrap_or_default();

        let snippet = element.select(&snippet_selector)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default();

        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult { title, url, snippet });
        }
    }

    results
}

// ─── Smart-search merging helpers ───

/// Normalize a URL for deduplication: lowercase it, drop the fragment,
/// treat http as https and strip trailing slashes.
fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let no_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let mut lowered = no_fragment.to_lowercase();
    if let Some(rest) = lowered.strip_prefix("http://") {
        lowered = format!("https://{rest}");
    }
    lowered.trim_end_matches('/').to_string()
}

/// Truncate a string to at most `max` chars (char-boundary safe).
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max).collect();
    format!("{truncated}...")
}

/// Deduplicate results from multiple backends by normalized URL and rank them
/// with reciprocal rank fusion: each occurrence at 1-based rank `r` in a
/// backend's list contributes `1 / (RRF_K + r)` to the URL's score, so URLs
/// returned by several backends (or ranked highly) end up on top.
fn merge_and_rank(sources: &[(&str, Vec<SearchResult>)], limit: u32) -> Vec<SearchResult> {
    struct Merged {
        result: SearchResult,
        score: f64,
        occurrences: usize,
    }

    let mut merged: HashMap<String, Merged> = HashMap::new();

    for (_name, results) in sources {
        for (idx, result) in results.iter().enumerate() {
            let key = normalize_url(&result.url);
            if key.is_empty() {
                // Results without a URL cannot be deduplicated or fetched later.
                continue;
            }
            let score = 1.0 / (RRF_K + (idx + 1) as f64);
            let entry = merged.entry(key).or_insert_with(|| Merged {
                result: result.clone(),
                score: 0.0,
                occurrences: 0,
            });
            entry.score += score;
            entry.occurrences += 1;
            // Backfill missing title/snippet from later sources.
            if entry.result.title.is_empty() {
                entry.result.title = result.title.clone();
            }
            if entry.result.snippet.is_empty() {
                entry.result.snippet = result.snippet.clone();
            }
        }
    }

    let mut entries: Vec<Merged> = merged.into_values().collect();
    entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.occurrences.cmp(&a.occurrences))
            // Deterministic tie-break so output is stable.
            .then_with(|| a.result.url.cmp(&b.result.url))
    });

    entries
        .into_iter()
        .take(limit as usize)
        .map(|e| e.result)
        .collect()
}

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

    // ─── Engine construction / dispatch ───

    #[test]
    fn test_search_engine_creation() {
        let config = SearchConfig::default();
        let _engine = SearchEngine::new(config);
    }

    #[tokio::test]
    async fn test_unconfigured_backend_returns_empty_without_network() {
        // No keys configured → each named backend short-circuits to empty.
        let engine = SearchEngine::new(SearchConfig::default());
        assert!(engine.search_linkup("q", 5).await.is_empty());
        assert!(engine.search_exa("q", 5).await.is_empty());
        assert!(engine.search_tavily("q", 5).await.is_empty());
        assert!(engine.search_serper("q", 5).await.is_empty());
        assert!(engine.search_brave("q", 5).await.is_empty());
        assert!(engine.search_parallel("q", 5).await.is_empty());
    }

    // ─── DuckDuckGo HTML parsing ───

    #[test]
    fn test_parse_duckduckgo_empty() {
        let results = parse_duckduckgo_results("<html></html>", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_duckduckgo_results() {
        let html = r##"
        <html><body>
          <div class="result">
            <a class="result__a" href="https://example.com/first">First title</a>
            <a class="result__snippet" href="#">First snippet</a>
          </div>
          <div class="result">
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org">Redirected</a>
            <a class="result__snippet" href="#">Via redirect</a>
          </div>
        </body></html>"##;
        let results = parse_duckduckgo_results(html, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "First title");
        assert_eq!(results[0].url, "https://example.com/first");
        assert_eq!(results[0].snippet, "First snippet");
        // DDG redirect links must be decoded to the real URL.
        assert_eq!(results[1].url, "https://rust-lang.org");
    }

    #[test]
    fn test_parse_duckduckgo_respects_limit() {
        let html = r#"
        <html><body>
          <div class="result"><a class="result__a" href="https://a.com">A</a></div>
          <div class="result"><a class="result__a" href="https://b.com">B</a></div>
          <div class="result"><a class="result__a" href="https://c.com">C</a></div>
        </body></html>"#;
        let results = parse_duckduckgo_results(html, 2);
        assert_eq!(results.len(), 2);
    }

    // ─── Exa response parsing ───

    #[test]
    fn test_parse_exa_response() {
        let value = serde_json::json!({
            "results": [
                {"title": "T1", "url": "https://a.com", "text": "Full page text", "score": 0.98},
                {"title": "T2", "url": "https://b.com", "snippet": "Short snippet"},
                {"title": "T3", "url": "https://c.com", "highlights": ["highlight one", "highlight two"]}
            ]
        });
        let results = parse_exa_response(&value, 10);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].url, "https://a.com");
        assert_eq!(results[0].snippet, "Full page text");
        assert_eq!(results[1].snippet, "Short snippet");
        assert_eq!(results[2].snippet, "highlight one");
    }

    #[test]
    fn test_parse_exa_response_limit_and_invalid() {
        let value = serde_json::json!({
            "results": [
                {"title": "T1", "url": "https://a.com"},
                {"title": "T2", "url": "https://b.com"}
            ]
        });
        assert_eq!(parse_exa_response(&value, 1).len(), 1);
        // Wrong shape → empty, no panic.
        assert!(parse_exa_response(&serde_json::json!({"results": "oops"}), 10).is_empty());
        assert!(parse_exa_response(&serde_json::json!({}), 10).is_empty());
    }

    #[test]
    fn test_parse_exa_skips_results_without_url() {
        let value = serde_json::json!({
            "results": [
                {"title": "No URL"},
                {"title": "Has URL", "url": "https://ok.com"}
            ]
        });
        let results = parse_exa_response(&value, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://ok.com");
    }

    // ─── Tavily response parsing ───

    #[test]
    fn test_parse_tavily_response_with_answer() {
        let value = serde_json::json!({
            "answer": "The synthesized answer.",
            "results": [
                {"title": "T1", "url": "https://a.com", "content": "Content A", "score": 0.9},
                {"title": "T2", "url": "https://b.com", "content": "Content B"}
            ]
        });
        let results = parse_tavily_response(&value, 10);
        assert_eq!(results.len(), 3);
        // Answer is prepended and borrows the top result's URL.
        assert_eq!(results[0].title, "Tavily answer");
        assert_eq!(results[0].snippet, "The synthesized answer.");
        assert_eq!(results[0].url, "https://a.com");
        assert_eq!(results[1].url, "https://a.com");
        assert_eq!(results[1].snippet, "Content A");
    }

    #[test]
    fn test_parse_tavily_response_without_answer() {
        let value = serde_json::json!({
            "results": [{"title": "T1", "url": "https://a.com", "content": "c"}]
        });
        let results = parse_tavily_response(&value, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "T1");
        // Malformed → empty.
        assert!(parse_tavily_response(&serde_json::json!({"results": 42}), 10).is_empty());
    }

    // ─── Serper response parsing ───

    #[test]
    fn test_parse_serper_response() {
        let value = serde_json::json!({
            "organic": [
                {"title": "O1", "link": "https://a.com", "snippet": "S1", "position": 1},
                {"title": "O2", "link": "https://b.com", "snippet": "S2", "position": 2}
            ]
        });
        let results = parse_serper_response(&value, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "O1");
        assert_eq!(results[0].url, "https://a.com");
        assert_eq!(results[1].snippet, "S2");
    }

    #[test]
    fn test_parse_serper_response_with_answer_box() {
        let value = serde_json::json!({
            "answerBox": {"answer": "42", "title": "The answer", "link": "https://ref.com"},
            "organic": [{"title": "O1", "link": "https://a.com", "snippet": "S1"}]
        });
        let results = parse_serper_response(&value, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "The answer");
        assert_eq!(results[0].snippet, "42");
        assert_eq!(results[0].url, "https://ref.com");
        assert_eq!(results[1].title, "O1");
        // Malformed → empty.
        assert!(parse_serper_response(&serde_json::json!({"organic": {}}), 10).is_empty());
    }

    // ─── Brave response parsing ───

    #[test]
    fn test_parse_brave_response() {
        let value = serde_json::json!({
            "web": {
                "results": [
                    {"title": "B1", "url": "https://a.com", "description": "D1", "age": "2 days ago"},
                    {"title": "B2", "url": "https://b.com", "description": "D2"}
                ]
            }
        });
        let results = parse_brave_response(&value, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "B1");
        assert_eq!(results[0].snippet, "D1");
        assert_eq!(results[1].url, "https://b.com");
    }

    #[test]
    fn test_parse_brave_response_empty_and_invalid() {
        assert!(parse_brave_response(&serde_json::json!({}), 10).is_empty());
        assert!(parse_brave_response(&serde_json::json!({"web": {"results": "nope"}}), 10).is_empty());
        assert!(parse_brave_response(&serde_json::json!({"web": {"results": []}}), 10).is_empty());
    }

    // ─── URL normalization ───

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("https://Example.com/Path/"), "https://example.com/path");
        assert_eq!(normalize_url("http://example.com/x"), "https://example.com/x");
        assert_eq!(normalize_url("https://example.com/a#section"), "https://example.com/a");
        assert_eq!(normalize_url("  https://example.com  "), "https://example.com");
        assert_eq!(normalize_url(""), "");
        assert_eq!(normalize_url("   "), "");
    }

    // ─── Smart-search merging ───

    #[test]
    fn test_merge_and_rank_dedupes_by_url() {
        let source_a = vec![
            result("A", "https://example.com/a", "snippet a"),
            result("B", "https://example.com/b", ""),
        ];
        let source_b = vec![
            result("B duplicate", "https://example.com/b/", "snippet b"),
            result("C", "https://example.com/c", "snippet c"),
        ];

        let merged = merge_and_rank(&[("one", source_a), ("two", source_b)], 10);
        assert_eq!(merged.len(), 3, "duplicate URLs must collapse");

        // B appears in both sources, so RRF ranks it above single-source results.
        assert!(merged[0].url.starts_with("https://example.com/b"));
        assert_eq!(merged[0].title, "B", "first-seen title is kept");
        assert_eq!(merged[0].snippet, "snippet b", "empty snippet is backfilled");
    }

    #[test]
    fn test_merge_and_rank_order_by_rank() {
        // Same set of URLs but different ranks: top-ranked items should win.
        let source_a = vec![
            result("A", "https://a.com", "sa"),
            result("B", "https://b.com", "sb"),
        ];
        let source_b = vec![
            result("A2", "https://a.com", "sa2"),
            result("C", "https://c.com", "sc"),
        ];

        let merged = merge_and_rank(&[("one", source_a), ("two", source_b)], 10);
        // a.com: rank 1 + rank 1 → highest score.
        assert_eq!(merged[0].url, "https://a.com");
        // b.com (rank 2, one source) vs c.com (rank 2, one source): tie broken by URL.
        assert_eq!(merged[1].url, "https://b.com");
        assert_eq!(merged[2].url, "https://c.com");
    }

    #[test]
    fn test_merge_and_rank_respects_limit_and_skips_empty_urls() {
        let source_a = vec![
            result("No URL", "", "x"),
            result("A", "https://a.com", "sa"),
            result("B", "https://b.com", "sb"),
        ];
        let merged = merge_and_rank(&[("one", source_a)], 1);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].url, "https://a.com", "empty URLs are dropped");
    }

    #[test]
    fn test_merge_and_rank_empty_input() {
        let merged = merge_and_rank(&[], 10);
        assert!(merged.is_empty());
        let merged = merge_and_rank(&[("one", vec![]), ("two", vec![])], 10);
        assert!(merged.is_empty());
    }

    // ─── Snippet truncation ───

    #[test]
    fn test_truncate_chars() {
        assert_eq!(truncate_chars("short", 10), "short");
        assert_eq!(truncate_chars("0123456789", 10), "0123456789");
        assert_eq!(truncate_chars("0123456789x", 10), "0123456789...");
        // Char-boundary safe with multibyte characters.
        let s = "приветпривет"; // 12 Cyrillic chars
        let t = truncate_chars(s, 6);
        assert_eq!(t, "привет...");
    }
}
