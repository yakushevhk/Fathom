//! Universal lead finder (lead source #5).
//!
//! High-level lead generation pipeline that combines the other lead sources:
//!
//! 1. Search business directories for companies matching industry + location.
//! 2. Parse each company's website for team members and contacts.
//! 3. Search social media for people matching the requested role titles.
//! 4. Attach emails/phones to people where plausible, deduplicate and rank
//!    by confidence.
//!
//! A lead with an empty `person.name` is a company-level contact lead (the
//! company has public contact details but no identifiable person matched).

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::corporate::{CorporateData, CorporateParser};
use crate::directories::{BusinessResult, DirectorySearch};
use crate::registry::{Tool, ToolContext};
use crate::social_search::{SocialSearch, SocialSearchResult};

/// How many company websites the pipeline parses (bounds runtime).
const MAX_SITES_TO_PARSE: usize = 6;
/// How many role titles drive social searches.
const MAX_ROLE_QUERIES: usize = 3;

/// Input for lead generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LeadQuery {
    pub industry: Option<String>,
    pub location: Option<String>,
    pub company_size: Option<String>,
    /// Roles to look for, e.g. ["CEO", "CTO", "Marketing Director"].
    /// Empty means "any person found on a team page".
    pub role_titles: Vec<String>,
    pub limit: u32,
}

/// A person identified during lead generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersonInfo {
    pub name: String,
    pub role: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub profile_url: Option<String>,
}

/// A company identified during lead generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompanyInfo {
    pub name: String,
    pub website: Option<String>,
    pub industry: Option<String>,
    pub location: Option<String>,
    pub size: Option<String>,
    pub description: Option<String>,
}

/// A single lead: person + company + provenance + confidence in [0, 1].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lead {
    pub person: PersonInfo,
    pub company: CompanyInfo,
    pub source: String,
    pub confidence: f32,
}

/// Universal lead generation pipeline.
pub struct LeadFinder {
    directories: DirectorySearch,
    social: SocialSearch,
    parser: CorporateParser,
}

impl LeadFinder {
    pub fn new(search_config: pr_core::SearchConfig) -> Self {
        Self {
            directories: DirectorySearch::new(),
            social: SocialSearch::new(search_config),
            parser: CorporateParser::new(),
        }
    }

    /// Run the full pipeline and return ranked, deduplicated leads.
    pub async fn find_leads(&self, query: LeadQuery) -> Vec<Lead> {
        let limit = query.limit.clamp(1, 100) as usize;
        let mut leads: Vec<Lead> = Vec::new();

        // 1. Find companies via business directories.
        let companies = self.find_companies(&query).await;

        // 2. Parse corporate websites for team members and contacts.
        let sites: Vec<&BusinessResult> = companies
            .iter()
            .filter(|c| c.website.as_deref().map(|w| !w.is_empty()).unwrap_or(false))
            .take(MAX_SITES_TO_PARSE)
            .collect();

        // Social search only needs the directory results, so start it now and
        // let it run concurrently with the (slower) website parsing below.
        let social_fut = self.social_role_search(&query, &companies);

        let parsed: Vec<(&BusinessResult, CorporateData)> = if sites.is_empty() {
            Vec::new()
        } else {
            let futures: Vec<_> = sites
                .iter()
                .map(|company| async move {
                    let url = company.website.clone().unwrap_or_default();
                    let data = self.parser.parse_website(&url).await;
                    (company, data)
                })
                .collect();
            let results: Vec<(&&BusinessResult, CorporateData)> = futures::future::join_all(futures).await;
            results.into_iter().map(|(c, d)| (*c, d)).collect()
        };

        for (company, data) in &parsed {
            let company_info = build_company_info(company, data, &query);
            leads.extend(leads_from_corporate_data(&company_info, data, &query.role_titles));
        }

        // 3. Company-level contact leads for companies with public contacts
        //    but no team members found.
        for company in &companies {
            let already_covered = leads
                .iter()
                .any(|l| !l.person.name.is_empty() && l.company.name.eq_ignore_ascii_case(&company.name));
            if already_covered {
                continue;
            }
            let has_contacts = company.phone.is_some() || company.email.is_some() || company.website.is_some();
            if has_contacts {
                leads.push(Lead {
                    person: PersonInfo {
                        name: String::new(),
                        email: company.email.clone(),
                        phone: company.phone.clone(),
                        ..Default::default()
                    },
                    company: CompanyInfo {
                        name: company.name.clone(),
                        website: company.website.clone(),
                        industry: if company.category.is_empty() {
                            None
                        } else {
                            Some(company.category.clone())
                        },
                        location: if company.address.is_empty() {
                            query.location.clone()
                        } else {
                            Some(company.address.clone())
                        },
                        size: query.company_size.clone(),
                        description: None,
                    },
                    source: format!("directory:{}", company.source),
                    confidence: score_lead(company.email.is_some(), company.phone.is_some(), false, false),
                });
            }
        }

        // 4. Social search for people matching the requested roles (started
        //    concurrently with the website parsing in step 2).
        let social_leads = social_fut.await;
        leads.extend(social_leads);

        // 5. Deduplicate and rank.
        dedupe_leads(&mut leads);
        leads.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.person.name.cmp(&b.person.name))
        });
        leads.truncate(limit);
        leads
    }

    /// Step 1: directory search for companies.
    async fn find_companies(&self, query: &LeadQuery) -> Vec<BusinessResult> {
        let what = query
            .industry
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "companies".to_string());
        let city = query.location.clone().unwrap_or_default();
        let want = (query.limit.clamp(1, 100) * 2).max(10);
        self.directories.search(&what, &city, None, want).await
    }

    /// Step 3: social search per role title, matched against found companies.
    /// The role queries run concurrently (`join_all` preserves their order).
    async fn social_role_search(
        &self,
        query: &LeadQuery,
        companies: &[BusinessResult],
    ) -> Vec<Lead> {
        if query.role_titles.is_empty() {
            return vec![];
        }
        let searches = query.role_titles.iter().take(MAX_ROLE_QUERIES).map(|role| {
            let mut q = role.to_string();
            if let Some(ref industry) = query.industry {
                q.push(' ');
                q.push_str(industry);
            }
            if let Some(ref location) = query.location {
                q.push(' ');
                q.push_str(location);
            }
            async move {
                let platforms = ["twitter".to_string(), "linkedin".to_string()];
                let results = self.social.search(&q, Some(&platforms), 8).await;
                (role.clone(), results)
            }
        });
        let results = futures::future::join_all(searches).await;

        let mut leads = Vec::new();
        for (role, results) in results {
            for result in results {
                leads.push(lead_from_social(&result, &role, companies));
            }
        }
        leads
    }
}

// ─── Pure helpers (unit-testable) ───

/// Combine a directory business with parsed website data.
fn build_company_info(business: &BusinessResult, data: &CorporateData, query: &LeadQuery) -> CompanyInfo {
    CompanyInfo {
        name: if data.company_name.is_empty() {
            business.name.clone()
        } else {
            data.company_name.clone()
        },
        website: data
            .website
            .clone()
            .into_option()
            .or_else(|| business.website.clone()),
        industry: data.industry.clone().or_else(|| {
            if business.category.is_empty() {
                query.industry.clone()
            } else {
                Some(business.category.clone())
            }
        }),
        location: if business.address.is_empty() {
            query.location.clone()
        } else {
            Some(business.address.clone())
        },
        size: data.size.clone().or_else(|| query.company_size.clone()),
        description: data.description.clone(),
    }
}

trait IntoOption {
    fn into_option(self) -> Option<String>;
}

impl IntoOption for String {
    fn into_option(self) -> Option<String> {
        if self.trim().is_empty() {
            None
        } else {
            Some(self)
        }
    }
}

/// Create person leads from a parsed team page.
fn leads_from_corporate_data(company: &CompanyInfo, data: &CorporateData, role_titles: &[String]) -> Vec<Lead> {
    let mut leads = Vec::new();
    for member in &data.team {
        let matched = role_matches(member.role.as_deref(), role_titles);
        // Only assign a company email to a person when it plausibly belongs
        // to that person (name appears in the local part).
        let email = data
            .contacts
            .emails
            .iter()
            .find(|e| email_matches_name(e, &member.name))
            .cloned();
        let has_email = email.is_some();

        leads.push(Lead {
            person: PersonInfo {
                name: member.name.clone(),
                role: member.role.clone(),
                email,
                phone: None,
                profile_url: None,
            },
            company: company.clone(),
            source: "corporate_site".to_string(),
            confidence: score_lead(has_email, false, matched, company.website.is_some()),
        });
    }
    leads
}

/// Create a lead from a social profile, attaching it to a known company when
/// the profile bio/name mentions one.
fn lead_from_social(result: &SocialSearchResult, role: &str, companies: &[BusinessResult]) -> Lead {
    let haystack = format!(
        "{} {}",
        result.name,
        result.bio.as_deref().unwrap_or_default()
    )
    .to_lowercase();

    let company = companies.iter().find(|c| {
        let name = c.name.to_lowercase();
        !name.is_empty() && haystack.contains(&name)
    });

    let corroborated = company.is_some();
    Lead {
        person: PersonInfo {
            name: result.name.clone(),
            role: Some(role.to_string()),
            email: None,
            phone: None,
            profile_url: Some(result.profile_url.clone()),
        },
        company: match company {
            Some(c) => CompanyInfo {
                name: c.name.clone(),
                website: c.website.clone(),
                industry: if c.category.is_empty() { None } else { Some(c.category.clone()) },
                location: if c.address.is_empty() { None } else { Some(c.address.clone()) },
                size: None,
                description: None,
            },
            None => CompanyInfo::default(),
        },
        source: format!("social:{}", result.platform),
        confidence: score_lead(false, false, true, corroborated),
    }
}

/// Whether a role string matches any requested title: the title must appear
/// as a whole-word sequence inside the role (case-insensitive), e.g. title
/// "CEO" matches role "Chief Executive Officer (CEO)".
fn role_matches(role: Option<&str>, role_titles: &[String]) -> bool {
    let Some(role) = role else {
        return false;
    };
    let role_words = split_words(role);
    if role_words.is_empty() {
        return false;
    }
    role_titles.iter().any(|t| {
        let title_words = split_words(t);
        if title_words.is_empty() {
            return false;
        }
        role_words.windows(title_words.len()).any(|w| w == title_words.as_slice())
    })
}

/// Lowercase alphanumeric words of a string.
fn split_words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Plausibility check that an email belongs to a person: any of the person's
/// name parts (2+ chars) appears in the email local part.
fn email_matches_name(email: &str, name: &str) -> bool {
    let local = email.split('@').next().unwrap_or_default().to_lowercase();
    if local.is_empty() {
        return false;
    }
    let parts: Vec<String> = name
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.chars().count() >= 2)
        .collect();
    if parts.is_empty() {
        return false;
    }
    let hits = parts.iter().filter(|p| local.contains(p.as_str())).count();
    // Require the first-name or last-name hit; both is even better.
    parts.first().map(|p| local.contains(p.as_str())).unwrap_or(false)
        || parts.last().map(|p| local.contains(p.as_str())).unwrap_or(false)
        || hits >= 2
}

/// Confidence scoring: base for an identified contact, bonuses for evidence.
fn score_lead(has_email: bool, has_phone: bool, role_matched: bool, corroborated: bool) -> f32 {
    let mut score = 0.15f32;
    if has_email {
        score += 0.35;
    }
    if has_phone {
        score += 0.20;
    }
    if role_matched {
        score += 0.20;
    }
    if corroborated {
        score += 0.10;
    }
    score.min(1.0)
}

/// Deduplicate leads by (person name, company name), keeping the higher
/// confidence entry; social profiles are merged into existing persons.
fn dedupe_leads(leads: &mut Vec<Lead>) {
    let mut unique: Vec<Lead> = Vec::new();

    for lead in leads.drain(..) {
        let name_key = lead.person.name.to_lowercase();
        let company_key = lead.company.name.to_lowercase();

        if let Some(existing) = unique.iter_mut().find(|l| {
            l.person.name.to_lowercase() == name_key && l.company.name.to_lowercase() == company_key
        }) {
            // Merge: keep the best confidence and fill gaps.
            if lead.confidence > existing.confidence {
                existing.confidence = lead.confidence;
            }
            if existing.person.email.is_none() {
                existing.person.email = lead.person.email.clone();
            }
            if existing.person.phone.is_none() {
                existing.person.phone = lead.person.phone.clone();
            }
            if existing.person.role.is_none() {
                existing.person.role = lead.person.role.clone();
            }
            if existing.person.profile_url.is_none() {
                existing.person.profile_url = lead.person.profile_url.clone();
            }
            if existing.company.website.is_none() {
                existing.company.website = lead.company.website.clone();
            }
            if !existing.source.contains(&lead.source) {
                existing.source.push(',');
                existing.source.push_str(&lead.source);
            }
        } else {
            unique.push(lead);
        }
    }

    *leads = unique;
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct FindLeadsParams {
    /// Industry or business category, e.g. "dental clinic", "software company".
    #[serde(default)]
    industry: Option<String>,
    /// City/region to search in.
    #[serde(default)]
    location: Option<String>,
    /// Preferred company size, e.g. "10-50 employees" (used for filtering labels).
    #[serde(default)]
    company_size: Option<String>,
    /// Role titles to look for, e.g. ["CEO", "CTO", "Marketing Director"].
    #[serde(default)]
    role_titles: Vec<String>,
    /// Maximum number of leads to return (default 10, max 100).
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    10
}

pub struct LeadFinderTool;

#[async_trait]
impl Tool for LeadFinderTool {
    fn name(&self) -> &str {
        "find_leads"
    }
    fn description(&self) -> &str {
        "Generate leads: find companies in an industry/location via business directories, extract team members and contacts from their websites, and search social media for people with the requested role titles. Returns ranked leads with confidence scores.

## Capability

Pipeline: (1) directory search for companies → (2) corporate site parsing for team/contacts → (3) social search for role matches → (4) email attribution, deduplication and confidence ranking. A lead with no person name is a company-level contact (public phone/email found, no identifiable person).

## When to Use

- Building outreach lists: industry + location + target roles.
- Finding decision-makers (CEO, CTO, Marketing Director, ...) at target companies.

## When NOT to Use

- Researching a single known company — use `parse_corporate_site` directly.
- Finding companies only (no people) — use `search_business_directory`.
- News monitoring — use `search_news`.

## Output

Each lead includes the person (name, role, email, phone, profile URL), the company (name, website, industry, location, size), the source and a confidence score in [0, 1].

## Failure Modes

- Few/no leads: directories may lack API keys for the region — the pipeline still returns whatever each source provides. Try broader industry terms or a bigger city.
- Company-level leads only: team pages missing or JavaScript-rendered.
- Confidence reflects evidence availability, not outreach quality."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(FindLeadsParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: FindLeadsParams = serde_json::from_value(args)?;

        let query = LeadQuery {
            industry: params.industry.clone(),
            location: params.location.clone(),
            company_size: params.company_size.clone(),
            role_titles: params.role_titles.clone(),
            limit: params.limit,
        };

        if query.industry.is_none() && query.location.is_none() && query.role_titles.is_empty() {
            return Ok(ToolOutput::err(
                "Provide at least one of: `industry`, `location`, or `role_titles`.",
            ));
        }

        let finder = LeadFinder::new(ctx.search_config.clone());
        let leads = finder.find_leads(query).await;

        if leads.is_empty() {
            return Ok(ToolOutput::ok(
                "No leads found. Directory APIs may not be configured for this region; try `search_business_directory` or `web_search` manually."
                    .to_string(),
            ));
        }

        let mut output = format!("Found {} leads:\n\n", leads.len());
        for (i, lead) in leads.iter().enumerate() {
            if lead.person.name.is_empty() {
                output.push_str(&format!("{}. [Company contact] **{}**\n", i + 1, lead.company.name));
            } else {
                output.push_str(&format!("{}. **{}**", i + 1, lead.person.name));
                if let Some(ref role) = lead.person.role {
                    output.push_str(&format!(" — {role}"));
                }
                output.push_str(&format!(" @ {}\n", if lead.company.name.is_empty() { "(unknown company)" } else { &lead.company.name }));
            }
            if let Some(ref email) = lead.person.email {
                output.push_str(&format!("   Email: {email}\n"));
            }
            if let Some(ref phone) = lead.person.phone {
                output.push_str(&format!("   Phone: {phone}\n"));
            }
            if let Some(ref url) = lead.person.profile_url {
                output.push_str(&format!("   Profile: {url}\n"));
            }
            if let Some(ref website) = lead.company.website {
                output.push_str(&format!("   Company website: {website}\n"));
            }
            output.push_str(&format!(
                "   Source: {} | Confidence: {:.2}\n\n",
                lead.source, lead.confidence
            ));
        }

        let metadata = serde_json::json!({
            "leads": leads,
            "count": leads.len(),
        });
        Ok(ToolOutput::ok_with_meta(output, metadata))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lead(name: &str, company: &str, confidence: f32, source: &str) -> Lead {
        Lead {
            person: PersonInfo {
                name: name.to_string(),
                ..Default::default()
            },
            company: CompanyInfo {
                name: company.to_string(),
                ..Default::default()
            },
            source: source.to_string(),
            confidence,
        }
    }

    fn business(name: &str, category: &str) -> BusinessResult {
        BusinessResult {
            name: name.to_string(),
            category: category.to_string(),
            address: String::new(),
            phone: None,
            website: None,
            email: None,
            rating: None,
            reviews_count: None,
            source: "2gis".to_string(),
        }
    }

    // ─── Confidence scoring ───

    #[test]
    fn test_score_lead() {
        assert_eq!(score_lead(false, false, false, false), 0.15);
        assert!(score_lead(true, false, false, false) > score_lead(false, false, false, false));
        assert!(score_lead(true, true, true, true) <= 1.0);
        assert!(score_lead(true, true, true, true) > score_lead(true, false, false, false));
    }

    // ─── Role matching ───

    #[test]
    fn test_role_matches() {
        let titles = vec!["CEO".to_string(), "Marketing Director".to_string()];
        assert!(role_matches(Some("Chief Executive Officer (CEO)"), &titles));
        assert!(role_matches(Some("marketing director"), &titles));
        assert!(role_matches(Some("CEO"), &titles));
        assert!(!role_matches(Some("CTO"), &titles));
        assert!(!role_matches(None, &titles));
        assert!(!role_matches(Some("CEO"), &[]));
    }

    // ─── Email attribution ───

    #[test]
    fn test_email_matches_name() {
        assert!(email_matches_name("john.doe@acme.com", "John Doe"));
        assert!(email_matches_name("jdoe@acme.com", "John Doe")); // last name hits
        assert!(email_matches_name("info@acme.com", "John Doe") == false);
        assert!(email_matches_name("ceo@acme.com", "John Doe") == false);
    }

    #[test]
    fn test_email_matches_name_edge_cases() {
        assert!(!email_matches_name("@acme.com", "John Doe"));
        assert!(!email_matches_name("john@acme.com", ""));
    }

    // ─── Deduplication ───

    #[test]
    fn test_dedupe_leads_merges_same_person_company() {
        let mut leads = vec![
            Lead {
                person: PersonInfo {
                    name: "Jane Doe".to_string(),
                    role: Some("CEO".to_string()),
                    email: Some("jane@acme.com".to_string()),
                    ..Default::default()
                },
                company: CompanyInfo {
                    name: "Acme".to_string(),
                    ..Default::default()
                },
                source: "corporate_site".to_string(),
                confidence: 0.6,
            },
            Lead {
                person: PersonInfo {
                    name: "jane doe".to_string(),
                    profile_url: Some("https://x.com/janedoe".to_string()),
                    ..Default::default()
                },
                company: CompanyInfo {
                    name: "ACME".to_string(),
                    website: Some("https://acme.com".to_string()),
                    ..Default::default()
                },
                source: "social:twitter".to_string(),
                confidence: 0.4,
            },
            lead("Other Person", "Acme", 0.3, "social:linkedin"),
        ];
        dedupe_leads(&mut leads);
        assert_eq!(leads.len(), 2);
        let jane = &leads[0];
        assert_eq!(jane.person.email.as_deref(), Some("jane@acme.com"));
        assert_eq!(jane.person.profile_url.as_deref(), Some("https://x.com/janedoe"));
        assert_eq!(jane.company.website.as_deref(), Some("https://acme.com"));
        assert_eq!(jane.confidence, 0.6, "keeps the higher confidence");
        assert!(jane.source.contains("corporate_site"));
        assert!(jane.source.contains("social:twitter"));
    }

    #[test]
    fn test_dedupe_leads_empty() {
        let mut leads: Vec<Lead> = vec![];
        dedupe_leads(&mut leads);
        assert!(leads.is_empty());
    }

    // ─── Company info building ───

    #[test]
    fn test_build_company_info_prefers_parsed_data() {
        let biz = BusinessResult {
            website: Some("https://acme.com".to_string()),
            ..business("Acme LLC", "Software")
        };
        let data = CorporateData {
            company_name: "Acme Corporation".to_string(),
            description: Some("We build things.".to_string()),
            industry: Some("Software Development".to_string()),
            size: Some("250+ employees".to_string()),
            headquarters: None,
            website: "https://acme.com".to_string(),
            contacts: Default::default(),
            team_page_url: None,
            team: vec![],
            social_profiles: vec![],
        };
        let query = LeadQuery::default();
        let info = build_company_info(&biz, &data, &query);
        assert_eq!(info.name, "Acme Corporation");
        assert_eq!(info.industry.as_deref(), Some("Software Development"));
        assert_eq!(info.size.as_deref(), Some("250+ employees"));
        assert_eq!(info.website.as_deref(), Some("https://acme.com"));
    }

    #[test]
    fn test_build_company_info_falls_back_to_directory() {
        let biz = business("Acme LLC", "Software");
        let data = CorporateData {
            company_name: String::new(),
            description: None,
            industry: None,
            size: None,
            headquarters: None,
            website: String::new(),
            contacts: Default::default(),
            team_page_url: None,
            team: vec![],
            social_profiles: vec![],
        };
        let query = LeadQuery {
            location: Some("Berlin".to_string()),
            ..Default::default()
        };
        let info = build_company_info(&biz, &data, &query);
        assert_eq!(info.name, "Acme LLC");
        assert_eq!(info.industry.as_deref(), Some("Software"));
        assert_eq!(info.location.as_deref(), Some("Berlin"));
        assert!(info.website.is_none());
    }

    // ─── Team → leads ───

    #[test]
    fn test_leads_from_corporate_data_assigns_matching_emails() {
        let company = CompanyInfo {
            name: "Acme".to_string(),
            website: Some("https://acme.com".to_string()),
            ..Default::default()
        };
        let data = CorporateData {
            company_name: "Acme".to_string(),
            description: None,
            industry: None,
            size: None,
            headquarters: None,
            website: "https://acme.com".to_string(),
            contacts: crate::corporate::ExtractedContacts {
                emails: vec!["jane.doe@acme.com".to_string(), "info@acme.com".to_string()],
                phones: vec![],
            },
            team_page_url: None,
            team: vec![
                crate::corporate::TeamMember {
                    name: "Jane Doe".to_string(),
                    role: Some("CEO".to_string()),
                },
                crate::corporate::TeamMember {
                    name: "Bob Stone".to_string(),
                    role: Some("CTO".to_string()),
                },
            ],
            social_profiles: vec![],
        };
        let titles = vec!["CEO".to_string()];
        let leads = leads_from_corporate_data(&company, &data, &titles);
        assert_eq!(leads.len(), 2);
        let jane = leads.iter().find(|l| l.person.name == "Jane Doe").unwrap();
        assert_eq!(jane.person.email.as_deref(), Some("jane.doe@acme.com"));
        assert!(jane.confidence > leads.iter().find(|l| l.person.name == "Bob Stone").unwrap().confidence);
        let bob = leads.iter().find(|l| l.person.name == "Bob Stone").unwrap();
        assert!(bob.person.email.is_none(), "generic emails are not assigned to people");
    }

    // ─── Social → leads ───

    #[test]
    fn test_lead_from_social_matches_company_in_bio() {
        let companies = vec![
            BusinessResult {
                website: Some("https://acme.com".to_string()),
                ..business("Acme Corp", "Software")
            },
        ];
        let profile = SocialSearchResult {
            platform: "linkedin".to_string(),
            profile_url: "https://linkedin.com/in/janedoe".to_string(),
            name: "Jane Doe".to_string(),
            bio: Some("CEO at ACME Corp, Berlin".to_string()),
            followers: None,
            location: None,
        };
        let lead = lead_from_social(&profile, "CEO", &companies);
        assert_eq!(lead.company.name, "Acme Corp");
        assert!(lead.confidence >= score_lead(false, false, true, true) - f32::EPSILON);
    }

    #[test]
    fn test_lead_from_social_unknown_company() {
        let profile = SocialSearchResult {
            platform: "twitter".to_string(),
            profile_url: "https://x.com/janedoe".to_string(),
            name: "Jane Doe".to_string(),
            bio: None,
            followers: None,
            location: None,
        };
        let lead = lead_from_social(&profile, "CEO", &[]);
        assert!(lead.company.name.is_empty());
        assert!(lead.confidence < 0.5);
    }

    // ─── Query validation ───

    #[test]
    fn test_lead_query_defaults() {
        let q = LeadQuery::default();
        assert!(q.industry.is_none());
        assert!(q.role_titles.is_empty());
        assert_eq!(q.limit, 0);
    }
}
