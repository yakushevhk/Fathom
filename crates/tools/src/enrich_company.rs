//! Company enrichment tool: gathers website, industry, size, revenue,
//! founding year, headquarters, description and detected technologies for a
//! company, combining web search snippets with direct website inspection.

use std::time::Duration;

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};
use crate::search::{SearchEngine, SearchResult};

/// Browser-like User-Agent for fetching company homepages.
const BROWSER_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Fetch timeout for the company website.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

// ─── Result types ───

/// Enriched company profile. Fields are `None` when no signal was found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyEnrichment {
    pub name: String,
    pub website: Option<String>,
    pub industry: Option<String>,
    /// Employee-count bucket: `1-10`, `11-50`, `51-200`, `201-500`,
    /// `501-1,000`, `1,001-5,000`, `5,001-10,000`, `10,000+`.
    pub size: Option<String>,
    /// Raw employee count when one was found.
    pub employees: Option<u64>,
    pub revenue: Option<String>,
    pub founded: Option<u32>,
    pub headquarters: Option<String>,
    pub description: Option<String>,
    /// Technologies detected on the company website.
    pub technologies: Vec<String>,
    /// URLs the enrichment was based on.
    pub sources: Vec<String>,
}

// ─── Enricher ───

pub struct CompanyEnricher;

impl CompanyEnricher {
    /// Enrich a company record.
    ///
    /// 1. Search the web for company facts (employees, founding, HQ, revenue).
    /// 2. Discover or fetch the website and parse its meta description.
    /// 3. Detect technologies from the homepage HTML.
    /// 4. Estimate size from employee counts found in snippets.
    pub async fn enrich(
        &self,
        ctx: &ToolContext,
        company_name: &str,
        website: Option<&str>,
    ) -> CompanyEnrichment {
        let name = company_name.trim().to_string();
        let engine = SearchEngine::new(ctx.search_config.clone());
        let mut sources: Vec<String> = Vec::new();

        // 1. Facts search.
        let facts_query = format!("\"{name}\" company employees founded headquarters revenue");
        let facts = engine.search(&facts_query, 8).await;
        let facts_text: Vec<String> = facts
            .iter()
            .map(|r| format!("{} {}", r.title, r.snippet))
            .collect();
        let facts_blob = facts_text.join("\n");
        sources.extend(facts.iter().map(|r| r.url.clone()).take(8));

        // 2. Website: use the provided one or discover it.
        let mut website = website.map(str::trim).filter(|w| !w.is_empty()).map(|w| {
            if w.starts_with("http://") || w.starts_with("https://") {
                w.to_string()
            } else {
                format!("https://{w}")
            }
        });
        if website.is_none() {
            let site_results = engine.search(&format!("\"{name}\" official website"), 5).await;
            website = pick_official_website(&site_results);
            if let Some(ref w) = website {
                sources.push(w.clone());
            }
        }

        // 3. Fetch the website for description + technology detection.
        let mut description = None;
        let mut technologies = Vec::new();
        if let Some(ref site) = website {
            match ctx
                .http_client
                .get(site)
                .header("User-Agent", BROWSER_UA)
                .header("Accept", "text/html,application/xhtml+xml")
                .timeout(FETCH_TIMEOUT)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let html = resp.text().await.unwrap_or_default();
                    description = extract_meta_description(&html).or_else(|| extract_title(&html));
                    technologies = detect_technologies(&html);
                }
                Ok(resp) => {
                    tracing::warn!("company website {site} returned HTTP {}", resp.status());
                }
                Err(e) => {
                    tracing::warn!("company website {site} fetch failed: {e}");
                }
            }
        }

        // 4. Extract structured facts from snippets (+ description).
        let combined = match &description {
            Some(d) => format!("{facts_blob}\n{d}"),
            None => facts_blob.clone(),
        };
        let employees = extract_employee_count(&combined);
        let size = employees.map(|n| size_bucket(n).to_string());
        let founded = extract_founded(&combined);
        let headquarters = extract_headquarters(&combined);
        let revenue = extract_revenue(&combined);
        let industry = classify_industry(&combined);

        sources.sort();
        sources.dedup();
        sources.truncate(12);

        CompanyEnrichment {
            name,
            website,
            industry,
            size,
            employees,
            revenue,
            founded,
            headquarters,
            description,
            technologies,
            sources,
        }
    }
}

// ─── Website discovery ───

/// Hosts that talk *about* companies rather than *being* the company.
const NON_OFFICIAL_HOSTS: &[&str] = &[
    "wikipedia.org", "linkedin.com", "crunchbase.com", "glassdoor.com",
    "facebook.com", "twitter.com", "x.com", "instagram.com", "youtube.com",
    "github.com", "zoominfo.com", "dnb.com", "yelp.com", "indeed.com",
    "bloomberg.com", "reuters.com", "forbes.com", "fortune.com", "owler.com",
    "craft.co", "tracxn.com", "pitchbook.com", "apple.com", "google.com",
    "wikidata.org", "fandom.com", "yellowpages.com", "mapquest.com",
];

/// Pick the first search result that plausibly is the company's own website.
pub fn pick_official_website(results: &[SearchResult]) -> Option<String> {
    for result in results {
        let Ok(url) = url::Url::parse(&result.url) else {
            continue;
        };
        let host = url.host_str().unwrap_or_default().to_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host);
        let dominated = NON_OFFICIAL_HOSTS
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}")));
        if !dominated {
            return Some(result.url.clone());
        }
    }
    None
}

// ─── Fact extraction (pure, testable) ───

/// Extract an employee count: "10,000+ employees", "employs 250 people", "1.2k employees".
pub fn extract_employee_count(text: &str) -> Option<u64> {
    let patterns = [
        r"(?i)(\d[\d,.]*)\s*(k|m)?\s*\+?\s*(?:employees|team members|staff members|staff|people worldwide|people)",
        r"(?i)employs\s+(?:approximately|about|over|around|nearly)?\s*(\d[\d,.]*)\s*(k|m)?",
    ];
    for pattern in patterns {
        let Ok(re) = regex::Regex::new(pattern) else {
            continue;
        };
        if let Some(cap) = re.captures(text) {
            let number = cap.get(1)?.as_str().replace(',', "");
            let base: f64 = number.parse().ok()?;
            let multiplier = match cap.get(2).map(|m| m.as_str().to_lowercase()) {
                Some(ref s) if s == "k" => 1_000.0,
                Some(ref s) if s == "m" => 1_000_000.0,
                _ => 1.0,
            };
            let total = base * multiplier;
            if total >= 1.0 {
                return Some(total.round() as u64);
            }
        }
    }
    None
}

/// Map an employee count to a conventional size bucket.
pub fn size_bucket(employees: u64) -> &'static str {
    match employees {
        0..=10 => "1-10",
        11..=50 => "11-50",
        51..=200 => "51-200",
        201..=500 => "201-500",
        501..=1_000 => "501-1,000",
        1_001..=5_000 => "1,001-5,000",
        5_001..=10_000 => "5,001-10,000",
        _ => "10,000+",
    }
}

/// Extract a founding year: "founded in 1994", "Founded: 2010".
pub fn extract_founded(text: &str) -> Option<u32> {
    let re = regex::Regex::new(r"(?i)founded\s*(?:in|:)?\s*(\d{4})").ok()?;
    let current_year = chrono::Utc::now().format("%Y").to_string().parse::<u32>().unwrap_or(2100);
    re.captures(text)
        .and_then(|cap| cap[1].parse::<u32>().ok())
        .filter(|year| (1800..=current_year).contains(year))
}

/// Extract headquarters: "headquartered in Dublin, Ireland" / "based in Berlin".
pub fn extract_headquarters(text: &str) -> Option<String> {
    let patterns = [
        r"(?i)headquartered in\s+([A-Za-z][^.!?\n;]{2,80})",
        r"based in\s+([A-Z][^.!?\n;]{2,80})",
    ];
    for pattern in patterns {
        let Ok(re) = regex::Regex::new(pattern) else {
            continue;
        };
        if let Some(cap) = re.captures(text) {
            let place = cap[1]
                .trim()
                .trim_end_matches(|c: char| matches!(c, ',' | '.' | ';' | ')' | ' '))
                .trim();
            if !place.is_empty() {
                return Some(place.to_string());
            }
        }
    }
    None
}

/// Extract revenue: "$5.2 billion in revenue", "revenue of $340 million".
pub fn extract_revenue(text: &str) -> Option<String> {
    let patterns = [
        r"(?i)revenue[^.!?\n]{0,40}?\$\s?([\d.,]+)\s*(billion|million|trillion|b|m|t)\b",
        r"(?i)\$\s?([\d.,]+)\s*(billion|million|trillion|b|m|t)\b[^.!?\n]{0,30}revenue",
    ];
    for pattern in patterns {
        let Ok(re) = regex::Regex::new(pattern) else {
            continue;
        };
        if let Some(cap) = re.captures(text) {
            let amount = cap[1].trim_end_matches('.');
            let unit = match cap[2].to_lowercase().as_str() {
                "b" | "billion" => "billion",
                "m" | "million" => "million",
                "t" | "trillion" => "trillion",
                _ => continue,
            };
            return Some(format!("${amount} {unit}"));
        }
    }
    None
}

// ─── Technology detection ───

/// Signature table: (case-insensitive marker in HTML, technology name).
const TECH_SIGNATURES: &[(&str, &str)] = &[
    ("wp-content", "WordPress"),
    ("wp-includes", "WordPress"),
    ("woocommerce", "WooCommerce"),
    ("cdn.shopify.com", "Shopify"),
    ("shopify.theme", "Shopify"),
    ("squarespace", "Squarespace"),
    ("wixstatic.com", "Wix"),
    ("webflow", "Webflow"),
    ("__next_data__", "Next.js"),
    ("_next/static", "Next.js"),
    ("___gatsby", "Gatsby"),
    ("__nuxt__", "Nuxt.js"),
    ("react-dom", "React"),
    ("react.production.min.js", "React"),
    ("vue.global", "Vue.js"),
    ("__vue__", "Vue.js"),
    ("vue.min.js", "Vue.js"),
    ("ng-version", "Angular"),
    ("svelte", "Svelte"),
    ("drupal", "Drupal"),
    ("joomla", "Joomla"),
    ("magento", "Magento"),
    ("bigcommerce", "BigCommerce"),
    ("prestashop", "PrestaShop"),
    ("opencart", "OpenCart"),
    ("elementor", "Elementor"),
    ("googletagmanager.com", "Google Tag Manager"),
    ("google-analytics.com", "Google Analytics"),
    ("gtag(", "Google Analytics"),
    ("js.stripe.com", "Stripe"),
    ("paypal", "PayPal"),
    ("hubspot", "HubSpot"),
    ("hs-scripts.com", "HubSpot"),
    ("intercom", "Intercom"),
    ("zendesk", "Zendesk"),
    ("cdn.segment.com", "Segment"),
    ("mixpanel", "Mixpanel"),
    ("amplitude", "Amplitude"),
    ("hotjar", "Hotjar"),
    ("clarity.ms", "Microsoft Clarity"),
    ("facebook.net", "Facebook Pixel"),
    ("fbq(", "Facebook Pixel"),
    ("klaviyo", "Klaviyo"),
    ("mailchimp", "Mailchimp"),
    ("browser.sentry-cdn.com", "Sentry"),
    ("jquery", "jQuery"),
    ("bootstrap", "Bootstrap"),
    ("tailwindcss", "Tailwind CSS"),
    ("font-awesome", "Font Awesome"),
    ("fontawesome", "Font Awesome"),
    ("_vercel", "Vercel"),
    ("netlify", "Netlify"),
    ("s3.amazonaws.com", "Amazon S3"),
    ("cloudfront", "Amazon CloudFront"),
    ("matomo", "Matomo"),
    ("piwik", "Matomo"),
    ("mc.yandex.ru", "Yandex Metrika"),
    ("salesforce", "Salesforce"),
    ("marketo", "Marketo"),
    ("laravel", "Laravel"),
    ("django", "Django"),
];

/// Detect technologies from homepage HTML via signature matching.
pub fn detect_technologies(html: &str) -> Vec<String> {
    let lower = html.to_lowercase();
    let mut found: Vec<String> = TECH_SIGNATURES
        .iter()
        .filter(|(marker, _)| lower.contains(marker))
        .map(|(_, name)| name.to_string())
        .collect();
    found.sort();
    found.dedup();
    found
}

// ─── Industry classification ───

/// Keyword rules in priority order: first match wins.
const INDUSTRY_RULES: &[(&[&str], &str)] = &[
    (&["fintech", "financial technology"], "FinTech"),
    (&["cryptocurrency", "blockchain", "web3", "crypto"], "Crypto & Blockchain"),
    (&["cybersecurity", "information security"], "Cybersecurity"),
    (&["biotech", "pharmaceutical", "healthcare", "medical", "clinical"], "Healthcare & Life Sciences"),
    (&["insurance"], "Insurance"),
    (&["e-commerce", "ecommerce", "online store", "online retail"], "E-Commerce & Retail"),
    (&["artificial intelligence", "machine learning"], "Artificial Intelligence"),
    (&["saas", "software platform", "software company", "cloud software"], "Software & Technology"),
    (&["video game", "gaming"], "Gaming"),
    (&["edtech", "e-learning", "education"], "Education"),
    (&["real estate"], "Real Estate"),
    (&["logistics", "freight", "supply chain", "shipping"], "Logistics & Supply Chain"),
    (&["manufacturing", "industrial"], "Manufacturing"),
    (&["telecommunications", "telecom"], "Telecommunications"),
    (&["renewable energy", "energy"], "Energy"),
    (&["automotive"], "Automotive"),
    (&["publishing", "media company", "news"], "Media & Publishing"),
    (&["hospitality", "travel"], "Travel & Hospitality"),
];

/// Classify the industry from combined text signals (search snippets, meta
/// description). Returns the first matching category, if any.
pub fn classify_industry(text: &str) -> Option<String> {
    let lower = format!(" {} ", text.to_lowercase());
    for (keywords, industry) in INDUSTRY_RULES {
        if keywords.iter().any(|kw| lower.contains(kw)) {
            return Some(industry.to_string());
        }
    }
    None
}

// ─── HTML meta extraction ───

/// Extract `meta[name="description"]` or `meta[property="og:description"]`.
pub fn extract_meta_description(html: &str) -> Option<String> {
    let document = scraper::Html::parse_document(html);
    for selector in [
        r#"meta[name="description"]"#,
        r#"meta[property="og:description"]"#,
    ] {
        let Ok(sel) = scraper::Selector::parse(selector) else {
            continue;
        };
        if let Some(content) = document
            .select(&sel)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            return Some(content);
        }
    }
    None
}

/// Extract the `<title>` text.
pub fn extract_title(html: &str) -> Option<String> {
    let document = scraper::Html::parse_document(html);
    let sel = scraper::Selector::parse("title").ok()?;
    document
        .select(&sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct EnrichCompanyParams {
    /// Company name to enrich
    company: String,
    /// Known company website (optional; discovered via search when omitted)
    #[serde(default)]
    website: Option<String>,
}

#[async_trait]
impl Tool for CompanyEnricher {
    fn name(&self) -> &str {
        "enrich_company"
    }
    fn description(&self) -> &str {
        "Enrich a company record: discover its website and gather industry, company size, revenue, founding year, headquarters, description and technologies used on the website.

## Capability

Combines web search snippets with direct inspection of the company homepage. From search results it extracts employee counts (converted to size buckets like 11-50 or 51-200), founding year, headquarters and revenue. From the website it reads the meta description and detects technologies (WordPress, Shopify, Next.js, React, HubSpot, Stripe, …) via HTML signatures.

## When to Use

- Building company profiles during lead generation or OSINT research.
- Completing partial CRM records (missing website, size, industry).
- Prioritizing leads by company size or technology stack.

## When NOT to Use

- Do NOT use for financial due diligence — revenue figures come from public snippets and may be outdated.
- Do NOT use for individuals — use `enrich_person`.

## Output

Company profile with as many fields as public signals allow; unknown fields are omitted. The `sources` list shows which URLs the data came from.

## Failure Modes

- Obscure companies may have little public data — fields come back null.
- Website behind bot protection → technology detection unavailable.
- Employee counts from snippets can be stale; prefer recent sources when they conflict."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(EnrichCompanyParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: EnrichCompanyParams = serde_json::from_value(args)?;
        let result = self.enrich(ctx, &params.company, params.website.as_deref()).await;

        let mut out = format!("Company enrichment: {}\n", result.name);
        if let Some(ref w) = result.website {
            out.push_str(&format!("Website: {w}\n"));
        }
        if let Some(ref i) = result.industry {
            out.push_str(&format!("Industry: {i}\n"));
        }
        if let Some(ref s) = result.size {
            out.push_str(&format!("Size: {s} employees\n"));
        }
        if let Some(ref r) = result.revenue {
            out.push_str(&format!("Revenue: {r}\n"));
        }
        if let Some(f) = result.founded {
            out.push_str(&format!("Founded: {f}\n"));
        }
        if let Some(ref h) = result.headquarters {
            out.push_str(&format!("Headquarters: {h}\n"));
        }
        if let Some(ref d) = result.description {
            out.push_str(&format!("Description: {d}\n"));
        }
        if !result.technologies.is_empty() {
            out.push_str(&format!("Technologies: {}\n", result.technologies.join(", ")));
        }
        if out.lines().count() == 1 {
            out.push_str("No public information found for this company.\n");
        }

        let meta = serde_json::to_value(&result).unwrap_or_default();
        Ok(ToolOutput::ok_with_meta(out, meta))
    }
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    fn result(url: &str) -> SearchResult {
        SearchResult {
            title: "t".to_string(),
            url: url.to_string(),
            snippet: "s".to_string(),
        }
    }

    // ── Website discovery ──

    #[test]
    fn test_pick_official_website_skips_third_parties() {
        let results = vec![
            result("https://en.wikipedia.org/wiki/Acme"),
            result("https://www.linkedin.com/company/acme"),
            result("https://www.acme.com/"),
            result("https://other.com"),
        ];
        assert_eq!(
            pick_official_website(&results),
            Some("https://www.acme.com/".to_string())
        );
    }

    #[test]
    fn test_pick_official_website_none_when_only_third_parties() {
        let results = vec![
            result("https://www.crunchbase.com/organization/acme"),
            result("https://www.glassdoor.com/Overview/Acme"),
        ];
        assert_eq!(pick_official_website(&results), None);
        assert_eq!(pick_official_website(&[]), None);
    }

    // ── Employee count / size ──

    #[test]
    fn test_extract_employee_count_variants() {
        assert_eq!(extract_employee_count("Acme has 10,000+ employees worldwide"), Some(10_000));
        assert_eq!(extract_employee_count("the company employs 250 people"), Some(250));
        assert_eq!(extract_employee_count("1.2k employees"), Some(1_200));
        assert_eq!(extract_employee_count("with 3,500 staff members"), Some(3_500));
        assert_eq!(extract_employee_count("no headcount info here"), None);
    }

    #[test]
    fn test_size_bucket() {
        assert_eq!(size_bucket(5), "1-10");
        assert_eq!(size_bucket(11), "11-50");
        assert_eq!(size_bucket(150), "51-200");
        assert_eq!(size_bucket(300), "201-500");
        assert_eq!(size_bucket(800), "501-1,000");
        assert_eq!(size_bucket(3_000), "1,001-5,000");
        assert_eq!(size_bucket(9_000), "5,001-10,000");
        assert_eq!(size_bucket(50_000), "10,000+");
    }

    // ── Founding year ──

    #[test]
    fn test_extract_founded() {
        assert_eq!(extract_founded("Acme was founded in 1994 and grew"), Some(1994));
        assert_eq!(extract_founded("Founded: 2010"), Some(2010));
        assert_eq!(extract_founded("founded 1887"), Some(1887));
        assert_eq!(extract_founded("no founding info"), None);
        assert_eq!(extract_founded("founded in 1492"), None); // before 1800 → rejected
        assert_eq!(extract_founded("founded in 2399"), None); // future → rejected
    }

    // ── Headquarters ──

    #[test]
    fn test_extract_headquarters() {
        assert_eq!(
            extract_headquarters("The company is headquartered in Dublin, Ireland."),
            Some("Dublin, Ireland".to_string())
        );
        assert_eq!(
            extract_headquarters("Acme, based in Berlin. It was founded"),
            Some("Berlin".to_string())
        );
        assert_eq!(extract_headquarters("no location info"), None);
        // "based in" requires an uppercase capture (a proper noun).
        assert_eq!(extract_headquarters("based in the building"), None);
    }

    // ── Revenue ──

    #[test]
    fn test_extract_revenue() {
        assert_eq!(
            extract_revenue("reported $5.2 billion in revenue last year"),
            Some("$5.2 billion".to_string())
        );
        assert_eq!(
            extract_revenue("annual revenue of $340 million"),
            Some("$340 million".to_string())
        );
        assert_eq!(
            extract_revenue("$1.4B revenue for FY24"),
            Some("$1.4 billion".to_string())
        );
        assert_eq!(extract_revenue("no money figures"), None);
    }

    // ── Technology detection ──

    #[test]
    fn test_detect_technologies() {
        let html = r#"<html><head>
            <script src="https://cdn.shopify.com/shopifycloud.js"></script>
            <script>window.dataLayer = []; gtag('js', new Date());</script>
            <link rel="stylesheet" href="/assets/tailwindcss.min.css">
            </head><body></body></html>"#;
        let tech = detect_technologies(html);
        assert!(tech.contains(&"Shopify".to_string()));
        assert!(tech.contains(&"Google Analytics".to_string()));
        assert!(tech.contains(&"Tailwind CSS".to_string()));
        assert!(!tech.contains(&"WordPress".to_string()));
    }

    #[test]
    fn test_detect_technologies_dedupes_and_sorts() {
        let html = "<html>wp-content and more wp-content</html>";
        assert_eq!(detect_technologies(html), vec!["WordPress".to_string()]);
        assert!(detect_technologies("<html></html>").is_empty());
    }

    // ── Industry classification ──

    #[test]
    fn test_classify_industry() {
        assert_eq!(classify_industry("a leading fintech startup"), Some("FinTech".to_string()));
        assert_eq!(classify_industry("Cybersecurity solutions provider"), Some("Cybersecurity".to_string()));
        assert_eq!(classify_industry("SaaS platform for HR teams"), Some("Software & Technology".to_string()));
        assert_eq!(classify_industry("unrelated text about bananas"), None);
    }

    #[test]
    fn test_classify_industry_priority() {
        // "fintech" beats the generic "software" rule.
        assert_eq!(
            classify_industry("fintech software company"),
            Some("FinTech".to_string())
        );
    }

    // ── Meta extraction ──

    #[test]
    fn test_extract_meta_description_and_title() {
        let html = r#"<html><head>
            <meta name="description" content="Acme builds widgets.">
            <title>Acme — Widgets for everyone</title>
            </head></html>"#;
        assert_eq!(
            extract_meta_description(html),
            Some("Acme builds widgets.".to_string())
        );
        assert_eq!(
            extract_title(html),
            Some("Acme — Widgets for everyone".to_string())
        );
        assert_eq!(extract_meta_description("<html></html>"), None);
    }

    // ── Tool plumbing ──

    #[test]
    fn test_tool_metadata() {
        let tool = CompanyEnricher;
        assert_eq!(tool.name(), "enrich_company");
        let schema = tool.schema();
        assert_eq!(schema.name, "enrich_company");
        assert!(schema.parameters.get("properties").is_some());
    }
}
