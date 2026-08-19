//! Business directory search (lead source #1).
//!
//! Searches businesses across multiple directories: 2GIS, Google Maps
//! (Places API), Yandex Maps (Geosearch API) and Yellow Pages (HTML
//! scraping). API-backed sources require keys via environment variables:
//!
//! - `PARALLEL_2GIS_API_KEY` — 2GIS Catalog API
//! - `PARALLEL_GOOGLE_PLACES_API_KEY` — Google Places API (New)
//! - `PARALLEL_YANDEX_MAPS_API_KEY` — Yandex Maps Geosearch API
//!
//! Yellow Pages needs no key. Sources without a configured key are skipped
//! and every network/API error degrades to an empty result set.

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::registry::{Tool, ToolContext};

/// Environment variables holding directory API keys.
pub const TWO_GIS_API_KEY_ENV: &str = "PARALLEL_2GIS_API_KEY";
pub const GOOGLE_PLACES_API_KEY_ENV: &str = "PARALLEL_GOOGLE_PLACES_API_KEY";
pub const YANDEX_MAPS_API_KEY_ENV: &str = "PARALLEL_YANDEX_MAPS_API_KEY";

const REQUEST_TIMEOUT_SECS: u64 = 20;
const USER_AGENT: &str = "Mozilla/5.0 (compatible; ParallelResearch/0.1)";
/// Maximum number of results fetched from any single directory.
const MAX_PER_SOURCE: u32 = 20;

/// A business found in a directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessResult {
    pub name: String,
    pub category: String,
    pub address: String,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub email: Option<String>,
    pub rating: Option<f32>,
    pub reviews_count: Option<u32>,
    /// Directory the result came from (`2gis`, `google_maps`, `yandex_maps`,
    /// `yellow_pages`).
    pub source: String,
}

/// Business directory search engine.
///
/// Each `search_*` method targets one directory and returns an empty vec
/// when the directory is not configured or the request fails.
pub struct DirectorySearch {
    http: reqwest::Client,
    /// Explicit keys (used by tests); `None` means "read from env at call time".
    two_gis_key: Option<String>,
    google_places_key: Option<String>,
    yandex_maps_key: Option<String>,
}

impl Default for DirectorySearch {
    fn default() -> Self {
        Self::new()
    }
}

impl DirectorySearch {
    pub fn new() -> Self {
        Self {
            http: pr_core::http_client(),
            two_gis_key: None,
            google_places_key: None,
            yandex_maps_key: None,
        }
    }

    /// Construct with explicit API keys (empty string = not configured).
    pub fn with_keys(two_gis: &str, google_places: &str, yandex_maps: &str) -> Self {
        Self {
            http: pr_core::http_client(),
            two_gis_key: Some(two_gis.to_string()),
            google_places_key: Some(google_places.to_string()),
            yandex_maps_key: Some(yandex_maps.to_string()),
        }
    }

    fn two_gis_key(&self) -> String {
        self.two_gis_key
            .clone()
            .unwrap_or_else(|| std::env::var(TWO_GIS_API_KEY_ENV).unwrap_or_default())
    }

    fn google_places_key(&self) -> String {
        self.google_places_key
            .clone()
            .unwrap_or_else(|| std::env::var(GOOGLE_PLACES_API_KEY_ENV).unwrap_or_default())
    }

    fn yandex_maps_key(&self) -> String {
        self.yandex_maps_key
            .clone()
            .unwrap_or_else(|| std::env::var(YANDEX_MAPS_API_KEY_ENV).unwrap_or_default())
    }

    /// Search all directories in parallel and merge the results.
    ///
    /// `sources` filters which directories are queried (`None` = all).
    pub async fn search(
        &self,
        query: &str,
        city: &str,
        sources: Option<&[String]>,
        limit: u32,
    ) -> Vec<BusinessResult> {
        let cap = limit.clamp(1, 50);
        let want = |name: &str| {
            sources
                .map(|s| s.iter().any(|x| x.eq_ignore_ascii_case(name)))
                .unwrap_or(true)
        };

        let q = query.to_string();
        let c = city.to_string();

        let two_gis_fut = async {
            if want("2gis") {
                self.search_2gis(&q, &c).await
            } else {
                Vec::new()
            }
        };
        let google_fut = async {
            if want("google_maps") {
                self.search_google_maps(&q, &c).await
            } else {
                Vec::new()
            }
        };
        let yandex_fut = async {
            if want("yandex_maps") {
                self.search_yandex_maps(&q, &c).await
            } else {
                Vec::new()
            }
        };
        let yellow_fut = async {
            if want("yellow_pages") {
                self.search_yellow_pages(&q, &c).await
            } else {
                Vec::new()
            }
        };

        let (a, b, c2, d) = tokio::join!(two_gis_fut, google_fut, yandex_fut, yellow_fut);

        let mut all: Vec<BusinessResult> = Vec::new();
        for results in [a, b, c2, d] {
            all.extend(results);
        }
        dedupe_businesses(&mut all);
        all.truncate(cap as usize);
        all
    }

    // ─── 2GIS ───

    /// Search the 2GIS Catalog API. Requires `PARALLEL_2GIS_API_KEY`.
    /// Resolves the city to a region id first, then queries items.
    pub async fn search_2gis(&self, query: &str, city: &str) -> Vec<BusinessResult> {
        let key = self.two_gis_key();
        if key.trim().is_empty() {
            return vec![];
        }

        // Resolve region id from the city name when a city is given.
        let region_id = if city.trim().is_empty() {
            None
        } else {
            match self.resolve_2gis_region(city, &key).await {
                Some(id) => Some(id),
                None => return vec![],
            }
        };

        let mut url = format!(
            "https://catalog.api.2gis.com/3.0/items?q={}&page_size={}&fields=items(point,contact_groups,external_content,reviews)&key={}",
            urlencoding::encode(query),
            MAX_PER_SOURCE,
            urlencoding::encode(&key)
        );
        if let Some(region_id) = region_id {
            url.push_str(&format!("&region_id={region_id}"));
        }

        let response = self
            .http
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => match resp.text().await {
                Ok(body) => match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(value) => parse_2gis_response(&value),
                    Err(e) => {
                        tracing::warn!("2GIS response parse error: {e}");
                        vec![]
                    }
                },
                Err(e) => {
                    tracing::warn!("2GIS body read error: {e}");
                    vec![]
                }
            },
            Ok(resp) => {
                tracing::warn!("2GIS search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("2GIS request error: {e}");
                vec![]
            }
        }
    }

    async fn resolve_2gis_region(&self, city: &str, key: &str) -> Option<String> {
        let url = format!(
            "https://catalog.api.2gis.com/3.0/regions?q={}&key={}",
            urlencoding::encode(city),
            urlencoding::encode(key)
        );
        let resp = self
            .http
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            tracing::warn!("2GIS region lookup failed: HTTP {}", resp.status());
            return None;
        }
        let value: serde_json::Value = resp.json().await.ok()?;
        value["result"]["items"][0]["id"].as_str().map(|s| s.to_string())
    }

    // ─── Google Maps (Places API) ───

    /// Search Google Places (New) text search. Requires
    /// `PARALLEL_GOOGLE_PLACES_API_KEY`.
    pub async fn search_google_maps(&self, query: &str, location: &str) -> Vec<BusinessResult> {
        let key = self.google_places_key();
        if key.trim().is_empty() {
            return vec![];
        }

        let text_query = if location.trim().is_empty() {
            query.to_string()
        } else {
            format!("{query} in {location}")
        };

        let body = serde_json::json!({
            "textQuery": text_query,
            "maxResultCount": MAX_PER_SOURCE.min(20),
        });

        let response = self
            .http
            .post("https://places.googleapis.com/v1/places:searchText")
            .header("X-Goog-Api-Key", key)
            .header(
                "X-Goog-FieldMask",
                "places.name,places.formattedAddress,places.nationalPhoneNumber,\
                 places.internationalPhoneNumber,places.websiteUri,places.rating,\
                 places.userRatingCount,places.primaryTypeDisplayName",
            )
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .json(&body)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
                Ok(value) => parse_google_places_response(&value),
                Err(e) => {
                    tracing::warn!("Google Places response parse error: {e}");
                    vec![]
                }
            },
            Ok(resp) => {
                tracing::warn!("Google Places search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Google Places request error: {e}");
                vec![]
            }
        }
    }

    // ─── Yandex Maps ───

    /// Search the Yandex Maps Geosearch API. Requires
    /// `PARALLEL_YANDEX_MAPS_API_KEY`.
    pub async fn search_yandex_maps(&self, query: &str, city: &str) -> Vec<BusinessResult> {
        let key = self.yandex_maps_key();
        if key.trim().is_empty() {
            return vec![];
        }

        let text = if city.trim().is_empty() {
            query.to_string()
        } else {
            format!("{query}, {city}")
        };

        let url = format!(
            "https://search-maps.yandex.ru/v1/?text={}&results={}&type=geom",
            urlencoding::encode(&text),
            MAX_PER_SOURCE
        );

        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("API-Key {key}"))
            .header("Accept-Language", "en_US")
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
                Ok(value) => parse_yandex_maps_response(&value),
                Err(e) => {
                    tracing::warn!("Yandex Maps response parse error: {e}");
                    vec![]
                }
            },
            Ok(resp) => {
                tracing::warn!("Yandex Maps search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Yandex Maps request error: {e}");
                vec![]
            }
        }
    }

    // ─── Yellow Pages (no key) ───

    /// Scrape yellowpages.com search results (US businesses, no API key).
    pub async fn search_yellow_pages(&self, query: &str, city: &str) -> Vec<BusinessResult> {
        let url = format!(
            "https://www.yellowpages.com/search?search_terms={}&geo_location_terms={}",
            urlencoding::encode(query),
            urlencoding::encode(if city.trim().is_empty() { "United States" } else { city })
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
                Ok(html) => parse_yellow_pages_html(&html),
                Err(e) => {
                    tracing::warn!("Yellow Pages body read error: {e}");
                    vec![]
                }
            },
            Ok(resp) => {
                tracing::warn!("Yellow Pages search failed: HTTP {}", resp.status());
                vec![]
            }
            Err(e) => {
                tracing::warn!("Yellow Pages request error: {e}");
                vec![]
            }
        }
    }
}

// ─── Response parsers (pure, unit-testable) ───

/// Parse a 2GIS Catalog API 3.0 items response.
fn parse_2gis_response(value: &serde_json::Value) -> Vec<BusinessResult> {
    let items = value["result"]["items"].as_array().cloned().unwrap_or_default();
    items
        .into_iter()
        .filter_map(|item| {
            let name = item["name"].as_str()?.to_string();
            let category = item["purpose_name"]
                .as_str()
                .or_else(|| item["full_name"].as_str())
                .unwrap_or_default()
                .to_string();
            let address = item["address_name"].as_str().unwrap_or_default().to_string();

            let mut phone = None;
            let mut website = None;
            let mut email = None;
            if let Some(groups) = item["contact_groups"].as_array() {
                for group in groups {
                    if let Some(contacts) = group["contacts"].as_array() {
                        for contact in contacts {
                            match contact["type"].as_str().unwrap_or_default() {
                                "phone" if phone.is_none() => {
                                    phone = contact["value"].as_str().map(|s| s.to_string());
                                }
                                "email" if email.is_none() => {
                                    email = contact["value"].as_str().map(|s| s.to_string());
                                }
                                _ => {}
                            }
                        }
                    }
                    if website.is_none() {
                        website = group["org_url"].as_str().map(|s| s.to_string());
                    }
                }
            }
            if website.is_none() {
                website = item["external_content"][0]["url"].as_str().map(|s| s.to_string());
            }

            let rating = item["reviews"]["rating"].as_f64().map(|v| v as f32);
            let reviews_count = item["reviews"]["count"].as_u64().map(|v| v as u32);

            Some(BusinessResult {
                name,
                category,
                address,
                phone,
                website,
                email,
                rating,
                reviews_count,
                source: "2gis".to_string(),
            })
        })
        .collect()
}

/// Parse a Google Places API (New) `places:searchText` response.
fn parse_google_places_response(value: &serde_json::Value) -> Vec<BusinessResult> {
    let places = value["places"].as_array().cloned().unwrap_or_default();
    places
        .into_iter()
        .filter_map(|place| {
            let name = place["name"].as_str()?.to_string();
            let address = place["formattedAddress"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let category = place["primaryTypeDisplayName"]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let phone = place["nationalPhoneNumber"]
                .as_str()
                .or_else(|| place["internationalPhoneNumber"].as_str())
                .map(|s| s.to_string());
            let website = place["websiteUri"].as_str().map(|s| s.to_string());
            let rating = place["rating"].as_f64().map(|v| v as f32);
            let reviews_count = place["userRatingCount"].as_u64().map(|v| v as u32);

            Some(BusinessResult {
                name,
                category,
                address,
                phone,
                website,
                email: None,
                rating,
                reviews_count,
                source: "google_maps".to_string(),
            })
        })
        .collect()
}

/// Parse a Yandex Maps Geosearch (GeoJSON FeatureCollection) response.
fn parse_yandex_maps_response(value: &serde_json::Value) -> Vec<BusinessResult> {
    let features = value["features"].as_array().cloned().unwrap_or_default();
    features
        .into_iter()
        .filter_map(|feature| {
            let props = &feature["properties"];
            if props["type"].as_str() == Some("house") {
                // Plain addresses are not businesses.
                return None;
            }
            let name = props["name"].as_str()?.to_string();
            let category = props["description"].as_str().unwrap_or_default().to_string();
            let address = props["address"].as_str().unwrap_or_default().to_string();

            let meta = &props["CompanyMetaData"];
            let mut phone = None;
            if let Some(phones) = meta["Phones"].as_array() {
                phone = phones
                    .iter()
                    .find_map(|p| p["formatted"].as_str().map(|s| s.to_string()));
            }
            let website = meta["url"].as_str().map(|s| s.to_string());
            let rating = value_f32(&meta["Reviews"]["rating"]);
            let reviews_count = value_u32(&meta["Reviews"]["count"]);

            Some(BusinessResult {
                name,
                category,
                address,
                phone,
                website,
                email: None,
                rating,
                reviews_count,
                source: "yandex_maps".to_string(),
            })
        })
        .collect()
}

/// Parse a JSON value that may be a number or a numeric string into f32.
fn value_f32(value: &serde_json::Value) -> Option<f32> {
    value
        .as_f64()
        .map(|v| v as f32)
        .or_else(|| value.as_str().and_then(|s| s.parse::<f32>().ok()))
}

/// Parse a JSON value that may be a number or a numeric string into u32.
fn value_u32(value: &serde_json::Value) -> Option<u32> {
    value
        .as_u64()
        .map(|v| v as u32)
        .or_else(|| value.as_str().and_then(|s| s.parse::<u32>().ok()))
}

/// Parse yellowpages.com organic search results HTML.
fn parse_yellow_pages_html(html: &str) -> Vec<BusinessResult> {
    let document = scraper::Html::parse_document(html);

    let Ok(item_sel) = scraper::Selector::parse("div.organic") else {
        return vec![];
    };
    let Ok(name_sel) = scraper::Selector::parse("a.business-name, .info h2.n a, h2.n a") else {
        return vec![];
    };
    let Ok(addr_sel) = scraper::Selector::parse(".adr") else {
        return vec![];
    };
    let Ok(phone_sel) = scraper::Selector::parse(".phones") else {
        return vec![];
    };
    let Ok(cat_sel) = scraper::Selector::parse(".links a, .categories a, .info a") else {
        return vec![];
    };

    document
        .select(&item_sel)
        .take(MAX_PER_SOURCE as usize)
        .filter_map(|el| {
            let name_el = el.select(&name_sel).next()?;
            let name = name_el.text().collect::<String>().trim().to_string();
            if name.is_empty() {
                return None;
            }
            let website = name_el.value().attr("href").map(|h| {
                if h.starts_with("http") {
                    h.to_string()
                } else {
                    format!("https://www.yellowpages.com{h}")
                }
            });
            let address = el
                .select(&addr_sel)
                .next()
                .map(|a| a.text().collect::<String>().trim().replace('\n', ", "))
                .unwrap_or_default();
            let phone = el
                .select(&phone_sel)
                .next()
                .map(|p| p.text().collect::<String>().trim().to_string());
            let category = el
                .select(&cat_sel)
                .next()
                .map(|c| c.text().collect::<String>().trim().to_string());

            Some(BusinessResult {
                name,
                category: category.unwrap_or_default(),
                address,
                phone,
                website,
                email: None,
                rating: None,
                reviews_count: None,
                source: "yellow_pages".to_string(),
            })
        })
        .collect()
}

/// Deduplicate businesses by (lowercased name, lowercased address). Keeps the
/// first occurrence but backfills missing phone/website/email from duplicates.
fn dedupe_businesses(results: &mut Vec<BusinessResult>) {
    let mut seen: std::collections::HashMap<(String, String), usize> =
        std::collections::HashMap::new();
    let mut unique: Vec<BusinessResult> = Vec::new();

    for result in results.drain(..) {
        let key = (result.name.to_lowercase(), result.address.to_lowercase());
        if let Some(&idx) = seen.get(&key) {
            let existing = &mut unique[idx];
            if existing.phone.is_none() {
                existing.phone = result.phone.clone();
            }
            if existing.website.is_none() {
                existing.website = result.website.clone();
            }
            if existing.email.is_none() {
                existing.email = result.email.clone();
            }
            if existing.rating.is_none() {
                existing.rating = result.rating;
            }
            if existing.reviews_count.is_none() {
                existing.reviews_count = result.reviews_count;
            }
        } else {
            seen.insert(key, unique.len());
            unique.push(result);
        }
    }
    *results = unique;
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DirectorySearchParams {
    /// What to search for, e.g. "coffee shop", "dental clinic", "IT company".
    query: String,
    /// City or location to search in (recommended).
    #[serde(default)]
    city: Option<String>,
    /// Which directories to query: `2gis`, `google_maps`, `yandex_maps`,
    /// `yellow_pages`. Defaults to all configured directories.
    #[serde(default)]
    sources: Option<Vec<String>>,
    /// Maximum number of results (default 10, max 50).
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    10
}

pub struct DirectorySearchTool;

#[async_trait]
impl Tool for DirectorySearchTool {
    fn name(&self) -> &str {
        "search_business_directory"
    }
    fn description(&self) -> &str {
        "Search business directories (2GIS, Google Maps, Yandex Maps, Yellow Pages) for companies matching a query in a city. Returns company names, categories, addresses, phones, websites, emails, ratings and review counts.

## Capability

Queries up to four business directories in parallel and merges the results. Directory coverage depends on configured API keys: 2GIS (PARALLEL_2GIS_API_KEY), Google Maps (PARALLEL_GOOGLE_PLACES_API_KEY), Yandex Maps (PARALLEL_YANDEX_MAPS_API_KEY). Yellow Pages (US) works without a key.

## When to Use

- Finding companies by category/industry in a specific city (lead generation).
- Getting contact details (phone, website, email) for local businesses.
- Building a list of businesses for outreach or research.

## When NOT to Use

- Global/non-local company research without a city — use `web_search`.
- People search — use `search_social`.
- Deep company profiling — use `parse_corporate_site` on the website found here.

## Failure Modes

- Empty results: no directory is configured for the region, or the query is too narrow. Try a broader category or different city spelling.
- Missing phones/websites: some directories do not expose them; fetch the company website with `web_fetch` instead."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(DirectorySearchParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: DirectorySearchParams = serde_json::from_value(args)?;
        if params.query.trim().is_empty() {
            return Ok(ToolOutput::err("Parameter `query` must not be empty."));
        }

        let searcher = DirectorySearch::new();
        let city = params.city.clone().unwrap_or_default();
        let results = searcher
            .search(
                params.query.trim(),
                city.trim(),
                params.sources.as_deref(),
                params.limit,
            )
            .await;

        if results.is_empty() {
            return Ok(ToolOutput::ok(format!(
                "No businesses found for '{}' in '{}'. Directory APIs may not be configured; try `web_search` instead.",
                params.query,
                if city.is_empty() { "(any location)" } else { city.as_str() }
            )));
        }

        let mut output = format!(
            "Found {} businesses for '{}'{}:\n\n",
            results.len(),
            params.query,
            if city.is_empty() {
                String::new()
            } else {
                format!(" in {city}")
            }
        );
        for (i, b) in results.iter().enumerate() {
            output.push_str(&format!("{}. **{}**\n", i + 1, b.name));
            if !b.category.is_empty() {
                output.push_str(&format!("   Category: {}\n", b.category));
            }
            if !b.address.is_empty() {
                output.push_str(&format!("   Address: {}\n", b.address));
            }
            if let Some(ref phone) = b.phone {
                output.push_str(&format!("   Phone: {phone}\n"));
            }
            if let Some(ref website) = b.website {
                output.push_str(&format!("   Website: {website}\n"));
            }
            if let Some(ref email) = b.email {
                output.push_str(&format!("   Email: {email}\n"));
            }
            if let Some(rating) = b.rating {
                output.push_str(&format!(
                    "   Rating: {:.1}{}\n",
                    rating,
                    b.reviews_count
                        .map(|c| format!(" ({c} reviews)"))
                        .unwrap_or_default()
                ));
            }
            output.push_str(&format!("   Source: {}\n\n", b.source));
        }

        let metadata = serde_json::json!({
            "results": results,
            "count": results.len(),
        });
        let _ = ctx; // ctx reserved for future per-context configuration
        Ok(ToolOutput::ok_with_meta(output, metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── Construction / key gating ───

    #[test]
    fn test_default_construction() {
        let ds = DirectorySearch::new();
        // No panic; defaults read env lazily.
        let _ = ds.two_gis_key();
        let _ = ds.google_places_key();
        let _ = ds.yandex_maps_key();
    }

    #[tokio::test]
    async fn test_unconfigured_sources_return_empty() {
        // Explicit empty keys → every API-backed source short-circuits.
        let ds = DirectorySearch::with_keys("", "", "");
        assert!(ds.search_2gis("coffee", "Moscow").await.is_empty());
        assert!(ds.search_google_maps("coffee", "Berlin").await.is_empty());
        assert!(ds.search_yandex_maps("coffee", "Moscow").await.is_empty());
    }

    // ─── 2GIS parsing ───

    #[test]
    fn test_parse_2gis_response() {
        let value = json!({
            "result": {
                "items": [
                    {
                        "id": "1",
                        "name": "Coffee House",
                        "purpose_name": "Coffee shop",
                        "address_name": "Moscow, Tverskaya st., 1",
                        "contact_groups": [{
                            "contacts": [
                                {"type": "phone", "value": "+7 495 123-45-67"},
                                {"type": "email", "value": "info@coffeehouse.ru"}
                            ],
                            "org_url": "https://coffeehouse.ru"
                        }],
                        "reviews": {"rating": 4.5, "count": 120}
                    },
                    {"id": "2", "name": "No Contacts Cafe"}
                ]
            }
        });
        let results = parse_2gis_response(&value);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "Coffee House");
        assert_eq!(results[0].category, "Coffee shop");
        assert_eq!(results[0].phone.as_deref(), Some("+7 495 123-45-67"));
        assert_eq!(results[0].email.as_deref(), Some("info@coffeehouse.ru"));
        assert_eq!(results[0].website.as_deref(), Some("https://coffeehouse.ru"));
        assert_eq!(results[0].rating, Some(4.5));
        assert_eq!(results[0].reviews_count, Some(120));
        assert_eq!(results[0].source, "2gis");
        assert!(results[1].phone.is_none());
    }

    #[test]
    fn test_parse_2gis_response_invalid() {
        assert!(parse_2gis_response(&json!({})).is_empty());
        assert!(parse_2gis_response(&json!({"result": {"items": "oops"}})).is_empty());
    }

    // ─── Google Places parsing ───

    #[test]
    fn test_parse_google_places_response() {
        let value = json!({
            "places": [
                {
                    "name": "Cafe Berlin",
                    "formattedAddress": "Unter den Linden 1, Berlin",
                    "primaryTypeDisplayName": {"text": "Cafe"},
                    "nationalPhoneNumber": "030 123456",
                    "websiteUri": "https://cafe-berlin.de",
                    "rating": 4.7,
                    "userRatingCount": 89
                },
                {"name": "Nameless rating only", "rating": 3.0}
            ]
        });
        let results = parse_google_places_response(&value);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "Cafe Berlin");
        assert_eq!(results[0].category, "Cafe");
        assert_eq!(results[0].phone.as_deref(), Some("030 123456"));
        assert_eq!(results[0].website.as_deref(), Some("https://cafe-berlin.de"));
        assert_eq!(results[0].rating, Some(4.7));
        assert_eq!(results[0].reviews_count, Some(89));
        assert_eq!(results[0].source, "google_maps");
        assert!(results[1].phone.is_none());
    }

    #[test]
    fn test_parse_google_places_response_invalid() {
        assert!(parse_google_places_response(&json!({})).is_empty());
        assert!(parse_google_places_response(&json!({"places": 42})).is_empty());
    }

    // ─── Yandex Maps parsing ───

    #[test]
    fn test_parse_yandex_maps_response() {
        let value = json!({
            "features": [
                {
                    "properties": {
                        "type": "business",
                        "name": "Barbershop Borodach",
                        "description": "Barbershop",
                        "address": "Saint Petersburg, Nevsky pr., 10",
                        "CompanyMetaData": {
                            "Phones": [{"formatted": "+7 812 000-00-00"}],
                            "url": "https://borodach.spb.ru",
                            "Reviews": {"rating": "4.9", "count": "33"}
                        }
                    }
                },
                {
                    "properties": {
                        "type": "house",
                        "name": "Building 10"
                    }
                }
            ]
        });
        let results = parse_yandex_maps_response(&value);
        // Houses are filtered out.
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Barbershop Borodach");
        assert_eq!(results[0].category, "Barbershop");
        assert_eq!(results[0].phone.as_deref(), Some("+7 812 000-00-00"));
        assert_eq!(results[0].website.as_deref(), Some("https://borodach.spb.ru"));
        assert_eq!(results[0].rating, Some(4.9));
        assert_eq!(results[0].reviews_count, Some(33));
        assert_eq!(results[0].source, "yandex_maps");
    }

    #[test]
    fn test_parse_yandex_maps_response_invalid() {
        assert!(parse_yandex_maps_response(&json!({})).is_empty());
        assert!(parse_yandex_maps_response(&json!({"features": "nope"})).is_empty());
    }

    // ─── Yellow Pages parsing ───

    #[test]
    fn test_parse_yellow_pages_html() {
        let html = r#"
        <html><body>
          <div class="organic">
            <div class="info">
              <h2 class="n"><a class="business-name" href="https://plumber.example.com">Ace Plumbing</a></h2>
              <div class="info-primary"><a href="/spb/plumbing">Plumbers</a></div>
              <p class="adr">123 Main St<span>, Springfield</span></p>
              <div class="phones">555-1234</div>
            </div>
          </div>
          <div class="organic">
            <div class="info"><h2 class="n"><a class="business-name" href="/biz/2">Empty Name Co</a></h2></div>
          </div>
        </body></html>"#;
        let results = parse_yellow_pages_html(html);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "Ace Plumbing");
        assert_eq!(results[0].website.as_deref(), Some("https://plumber.example.com"));
        assert_eq!(results[0].phone.as_deref(), Some("555-1234"));
        assert_eq!(results[0].source, "yellow_pages");
    }

    #[test]
    fn test_parse_yellow_pages_html_empty() {
        assert!(parse_yellow_pages_html("<html><body></body></html>").is_empty());
    }

    // ─── Deduplication ───

    fn business(name: &str, address: &str, source: &str) -> BusinessResult {
        BusinessResult {
            name: name.to_string(),
            category: String::new(),
            address: address.to_string(),
            phone: None,
            website: None,
            email: None,
            rating: None,
            reviews_count: None,
            source: source.to_string(),
        }
    }

    #[test]
    fn test_dedupe_businesses_merges_duplicates() {
        let mut results = vec![
            BusinessResult {
                phone: Some("111".to_string()),
                ..business("Coffee House", "Tverskaya 1", "2gis")
            },
            BusinessResult {
                website: Some("https://coffeehouse.ru".to_string()),
                rating: Some(4.5),
                ..business("coffee house", "Tverskaya 1", "google_maps")
            },
            business("Other Place", "Elsewhere 2", "yandex_maps"),
        ];
        dedupe_businesses(&mut results);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "Coffee House");
        assert_eq!(results[0].phone.as_deref(), Some("111"));
        assert_eq!(results[0].website.as_deref(), Some("https://coffeehouse.ru"));
        assert_eq!(results[0].rating, Some(4.5));
        assert_eq!(results[1].name, "Other Place");
    }

    #[test]
    fn test_dedupe_businesses_empty() {
        let mut results: Vec<BusinessResult> = vec![];
        dedupe_businesses(&mut results);
        assert!(results.is_empty());
    }
}
