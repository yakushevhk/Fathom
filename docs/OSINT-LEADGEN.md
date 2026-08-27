# OSINT & Lead Generation

Fathom is an **OSINT engine for contact harvesting**: agents search for companies and people in open sources, extract emails/phones/social networks, verify and enrich data, store it in a database, and sync with CRM.

The engine is designed as a **multi-stage pipeline** rather than a single scraper. Each stage is implemented as a dedicated tool with a narrow, composable contract — search, extract, verify, enrich, store, sync — so the coordinator can weave stages together flexibly depending on the query. A run typically fans out several researcher agents in parallel (one per data source category), each producing structured snippets, then funnels everything through the same verification and storage path. This keeps the system resilient: if one source (say LinkedIn) blocks access, the remaining agents still deliver partial results, and every result carries a confidence score so downstream consumers can decide how much to trust it.

---

## OSINT Capabilities

| Feature | Tools |
|---------|-------|
| Contact extraction | `extract_contacts` |
| Lead search | `find_leads` |
| Business directories | `search_business_directory` (2GIS, Maps) |
| Social networks | `search_social` |
| Corporate websites | `parse_corporate_site` |
| News/mentions | `search_news` |
| Email verification | `verify_email` |
| Phone verification | `verify_phone` |
| Social profile verification | `verify_social_profile` |
| Company enrichment | `enrich_company` |
| Person enrichment | `enrich_person` |
| Contact storage | `ContactDb` (SQLite/PostgreSQL) |
| CRM sync | amoCRM, Bitrix24, HubSpot |

The capability set breaks down into four layers:

- **Collection** — `extract_contacts`, `find_leads`, `search_business_directory`, `search_social`, `parse_corporate_site`, `search_news` gather raw material from the open web: contact strings inside pages, business directory listings, social profiles, corporate team pages, and news mentions respectively.
- **Verification** — `verify_email`, `verify_phone`, `verify_social_profile` check that a harvested contact is real before it enters the database, turning guesses into curated leads.
- **Enrichment** — `enrich_company`, `enrich_person` widen each record with additional attributes (industry, size, technologies, job titles, social links) so sales outreach has context.
- **Persistence & distribution** — `ContactDb` stores deduplicated records, and `save_contacts` optionally pushes them into a configured CRM in the same call.
All OSINT collection, enrichment, and verification tools are **parallel-safe** (read-only): the coordinator can run multiple researchers concurrently, and each individual tool may issue sub-requests to several backends in parallel internally. The only exception is `save_contacts`, which performs sequential writes to the database to guarantee TOCTOU-safe deduplication.
---

## Example Queries

### Finding company executives

```bash
fathom run \
  "Find contacts of executives (CEO, CTO) at small IT companies in Moscow. \
   Extract emails, phones, LinkedIn profiles." \
  --output ./leads/
```

**What agents do:**
1. Search for companies via Unified State Register of Legal Entities (USRLE) aggregators (rusprofile, list-org, sbis, RBC)
2. Parse official websites (`parse_corporate_site`)
3. Extract contacts (`extract_contacts`)
4. Enrich persons (`enrich_person`)
5. Search for LinkedIn profiles (`search_social`)
6. Launch Python for data processing

**Result**: a table of companies with TIN/PSRN, CEO, email, phones, LinkedIn candidates.

The query deliberately names **roles** ("executives", "CEO, CTO") rather than people — the planner interprets this as a role-broad search. This query is a good example of the pipeline's decomposition: it requires a *company-first* pass (find registered companies fitting the criteria), then a *person-first* pass (map roles to names), and finally a *cross-reference* pass (tie each name to contact channels). Agents work these tracks in parallel; well-known company domains make email pattern detection (`suggest_emails`) applicable, so even executives whose addresses are never published can get plausible verified candidates.

### Finding SaaS founders

```bash
fathom run \
  "Find 20 Series A-B SaaS companies in Berlin. \
   Collect emails and LinkedIn profiles of founders and CTOs." \
  --output ./berlin-saas/
```

This query exercises **international company screening**. The coordinator seeks firms that have raised Series A or B rounds (usually found via news mentions, Crunchbase-like aggregators accessible through `web_search`, and press releases on corporate sites), scopes them to Berlin, then applies the founder/CTO contact pass. Because Series A–B SaaS companies nearly always maintain a team page, `parse_corporate_site` + `extract_contacts` on those pages yields most of the person names, which are then verified with `verify_email` and cross-referenced with `search_social` for LinkedIn.

### Industry analysis + contacts

```bash
fathom run \
  "Research the fintech startup market in Dubai. \
   For the top 10 companies, find contacts of decision-makers." \
  --output ./dubai-fintech/
```

This is a **market intelligence + lead list** hybrid. The research half (market size, major players, landscape) is handled by generic `web_search`/`web_fetch` agents, while the lead-generation half reuses the company-first pipeline limited to the top 10 identified companies. `enrich_company` adds firmographics (industry, size, location, technologies) alongside the people data, so the final report connects strategic context with actionable contacts.

---

## What Gets Extracted

### Email Addresses
- Regex patterns for standard emails
- Deobfuscation: `name [at] domain [dot] com`, `name (at) domain (dot) com`
- From `mailto:` links, JSON-LD blocks
- **Confidence scoring**: plain (0.95), obfuscated (0.7)

Extraction is **deterministic and layered**: plain-text regex is scanned across the corpus, then a second pass handles obfuscated forms where bots-and-spam protection replaced `@` and `.` with words. A third pass reads `mailto:` anchors and JSON-LD structured data embedded in the page. Results are deduplicated **keyed by normalized email** (case-insensitive, whitespace trimmed), and when the same address appears in both plain and obfuscated form, the higher-confidence plain match wins rather than producing two records. After extraction, corporate email **pattern inference** can generate candidate addresses for other people at the same domain: the verifier detects the structural pattern (e.g. `first.last@`, `firstinitiallast@`) from *other* known addresses, and `suggest_emails` produces permutations for a given name at that domain.

### Phone Numbers
- International formats (+1, +7, +44, +86...)
- Local formats with area codes
- Normalization to **E.164** via libphonenumber
- Country detection, mobile/landline

Phone extraction accepts both international and local notation, then normalizes every hit to **E.164** canonical form using libphonenumber. Normalization includes country detection (from the dialing code and context) and a mobile/landline classification, which is useful for choosing a channel: WhatsApp-eligible mobile numbers behave very differently from office landlines. The extraction pass keeps the raw text alongside the normalized form so later stages can show what was actually found on the page.

### Social Networks
- LinkedIn (`linkedin.com/in/`, `/company/`)
- Twitter/X (`twitter.com/`, `x.com/`, `@username`)
- Instagram (`instagram.com/`)
- Telegram (`t.me/`, `@username`)
- Facebook (`facebook.com/`)

Social profiles are extracted as (platform, handle/username, profile URL) triples. **Keyed deduplication** collapses duplicates per platform: when the same identity appears both as a bare handle (e.g. `@durov`) and as a full URL (e.g. `t.me/durov`), the URL-bearing entry wins because it is directly navigable. `search_social` then runs read-only queries across configured platforms in parallel and merges per-platform results, deduplicating by lowercased profile URL so the same person found via multiple queries appears once.

### Persons
- Name, title, company
- Email, phone, social networks

Person records aggregate everything tied to one identity. `extract_contacts` populates the deterministic fields; a subsequent LLM pass (`extract_entities_with_llm`) adds the soft fields — job title, company affiliation, role — reading only data explicitly present in the source text (never inventing). Person rows are deduplicated by normalized name so team pages that repeat a member's card produce a single entity.

### Companies
- Name, website, industry, size, location
- Employee list

Company records carry firmographics plus an employee list populated from team pages. `enrich_company` deepens this with additional signals detected from the corporate site: contact emails/phones, social links, and technology fingerprints (CMS, frameworks) that are useful for lead scoring. Persons are linked to companies through `enrich_person`, and the knowledge graph (`memory_kb`) records typed person↔company↔location relations so later runs reuse known nodes instead of re-scraping.

---

## Data Sources

### Business Directories
- **2GIS** — CIS companies
- **Google Maps / Places** — globally
- **Yandex Maps** — Russia/CIS
- **Yellow Pages** — international

`search_business_directory` queries up to four directories **in parallel** and merges the results. Availability of each directory depends on configured API keys: 2GIS (`PARALLEL_2GIS_API_KEY`), Google Places (`PARALLEL_GOOGLE_PLACES_API_KEY`), Yandex Maps (`PARALLEL_YANDEX_MAPS_API_KEY`); Yellow Pages works keyless for US businesses. Merged listings are deduplicated by (name, address) — the first occurrence is kept but phone/website/email fields are backfilled from duplicates, so two listings of the same business with complementary contact fields collapse into one complete record. The combination of regional and global directories gives broad geo coverage: 2GIS/Yandex for CIS companies, Google Places globally, Yellow Pages internationally.

### USRLE Aggregators (Russia)
- rusprofile.ru
- list-org.com
- sbis.ru
- companies.rbc.ru

These aggregators expose the Unified State Register of Legal Entities — filings that give legal identifiers (**TIN/PSRN**), legal address, registration date, and founder lists. Queries against them are the *company-first* entry point for RU-focused lead generation: they bootstrap a verified list of real registered companies before any contact harvesting happens, guaranteeing that outreach targets are legal entities rather than hobby projects.

### Corporate Websites
- Team pages, About pages
- Contacts, press releases
- Schema.org markup, JSON-LD

`parse_corporate_site` targets the highest-yield pages of a corporate domain. Team/About pages and contact pages carry the majority of per-person contacts; press releases surface executive mentions; Schema.org and JSON-LD markup provide machine-readable name/contact blocks. The site is fetched once and reused across extractors, so a full company pass (contacts + persons + technologies) needs only one download. Site structure is discovered via common URL conventions (`/team`, `/about`, `/contacts`, `/press`) and team-card CSS selectors, in priority order, with graceful handling of sites that return errors.

### Social Networks
- Twitter/X search
- Telegram channels/users
- LinkedIn (requires anti-bot bypass)

Social search is used for the *person-first* track and for cross-referencing. Telegram is particularly reliable for unstructured name→handle mapping (channels and user bios frequently contain job titles), and candidates are cross-validated against other platforms. LinkedIn is the richest professional graph but actively blocks bots (HTTP 999); the platform is still searched, with the understanding that a proxy/cookies may be required for stable parsing, and searches gracefully fall back to general web search when blocked.

### News
- Serper News API
- Google News RSS

News/mentions serve two purposes: **entity discovery** (a round-up article naming the top fintech companies in Dubai is often the fastest route to a company list) and **person intel** (CEO announcements, funding news with exec quotes). `search_news` scans headlines and bodies, and its entity extraction deduplicates repeated mentions of the same person/company across articles. News results feed the pipeline as *contextual leads* — they rarely contain direct contacts but reliably identify who matters where.

---

## Data Verification

### Email
```
verify_email("info@company.com")
→ is_valid_syntax: true
→ domain_exists: true
→ mx_records: ["mx1.company.com"]
→ is_disposable: false
→ is_role_based: true  (info@, admin@, support@)
→ confidence: 0.85
```

Email verification is a **multi-signal check** rather than a single pass:

1. **Syntax** — RFC 5322 practical subset: one `@`, valid local part without consecutive/leading/trailing dots, dotted domain with a non-numeric TLD.
2. **Domain/MX** — DNS-over-HTTPS MX lookups (Google resolver with Cloudflare fallback, no native DNS dependency) to prove the domain accepts mail; if no MX exists, an A-record fallback is checked. Lookups are cached per session so repeated verifications in one run never re-hit the network.
3. **Disposable/role detection** — known throwaway-provider domains and role local-parts (`info@`, `support@`, `admin@`, …) are flagged; role addresses reach a mailbox but usually not a person.
4. **Optional SMTP probe** — the strongest signal: connect to the best MX host on port 25, run the `HELO`/`MAIL FROM`/`RCPT TO` dialogue, and report whether the recipient was accepted (up to `250 OK`). The probe **never sends message content** and bails before `DATA`, so it is safe and non-spamming; it respects strict 10s connect / 5s read timeouts.

Signals are combined into a single **confidence score** (syntax + domain + non-disposable + SMTP acceptance). Crucially, `verify_email` can also validate *suggested* patterns: once the corporate email pattern is detected on a domain, candidate addresses for named people are verified the same way — this is how unpublished executive emails get confirmed. Verification outcomes are recorded as durable receipts in the session ledger, each check kind independently, so a green MX result never masks a red SMTP result.

### Phone
```
verify_phone("+7 (495) 710-75-80")
→ normalized: "+74957107580"
→ country_code: "RU"
→ is_valid: true
→ is_mobile: false
```

Phone verification validates and normalizes to **E.164** via libphonenumber: country code detection, number-type classification (mobile/landline/voicemail/emergency/short-code/carrier). The output's `is_valid` gate prevents garbage from entering the database, while `is_mobile` decides outreach channel suitability. Under the hood this is pure in-process validation (no external API), so it is fast enough to run on every harvested number in a batch.

### Social Networks
```
verify_social_profile("https://linkedin.com/in/john-doe")
→ exists: true
→ platform: "linkedin"
→ username: "john-doe"
```

Social profile verification confirms the profile actually resolves. It normalizes the URL (extracting platform and username), then performs a lightweight existence check (the resolvable/web presence check appropriate to the platform), returning `exists` for profiles that respond. This filters out dead handles and invented links before they are promoted to a lead's contact channels. Like the other verification tools it is read-only and parallel-safe, so batches verify concurrently.

---

## Contact Storage

### ContactDb (SQLite)

```toml
[contacts]
db_path = "./contacts.db"
```

Schema:
- `contacts` — id, email, phone, name, title, company, source, timestamps
- `social_profiles` — contact_id, platform, url, username
- `companies` — name, website, industry, size, location
- `tags` — contact_id, tag
- `notes` — contact_id, note

Functions:
- Add/search/update contacts
- **Deduplication** (by email/phone)
- **Merge** duplicates
- Tags and notes

SQLite is the default store for local runs: zero setup, file-based, ideal for single-machine lead generation. The schema is deliberately normalized: contacts are the hub, with social profiles (a contact can have many handles across platforms), companies, and free-form tags/notes attached as side tables. **Deduplication is keyed by normalized email or phone**: when a new record matches an existing one, the data is **merged** rather than duplicated — blank fields are filled, socials/tags/notes are appended — and the insert is atomic inside the store to avoid race conditions. `save_contacts` reports exactly how many records were added vs merged, so a run's output is verifiable.

### PostgreSQL (for large databases)

```toml
[contacts]
pg_url = "postgres://user:pass@localhost/contacts"
```

Same interface as SQLite.

For shared or high-volume databases, `ContactDb` exposes the **same interface** over PostgreSQL — swap the config line and every tool (extract, save, dedup, search, CRM push) works unchanged. This is the production choice for teams where multiple parallel research sessions write into one shared contact pool, because deduplication and merge happen against the same source of truth regardless of which session saved a record.

---

## CRM Sync

### amoCRM

```toml
[crm]
provider = "amocrm"
domain = "yourcompany"      # yourcompany.amocrm.ru
api_key = "your-api-key"
```

### Bitrix24

```toml
[crm]
provider = "bitrix24"
domain = "yourcompany"      # yourcompany.bitrix24.ru
api_key = "your-api-key"
```

### HubSpot

```toml
[crm]
provider = "hubspot"
api_key = "your-private-app-token"
```

Contacts are automatically pushed to CRM after extraction.

CRM sync turns the harvested list into immediate sales action. When a CRM is configured, `save_contacts` pushes each saved/merged contact to the provider **in the same call** that stores it locally — extraction → database → CRM is one step, not a manual export. Pushes are resilient: each contact gets one retry for transient network errors, and the tool's JSON summary separates `crm_pushed` from push errors so failures are visible rather than silent. `save_contacts` sits in the default **approval tools** list, so a human operator explicitly approves production pushes into the sales system. The sync path is provider-agnostic: amoCRM, Bitrix24, and HubSpot share the same config shape (`provider` + `domain` + `api_key`), differing only in credentials.

---

## Contact Export

Formats (see [CONFIGURATION.md](CONFIGURATION.md) `[export]`):
- **CSV** — for Excel/Google Sheets
- **vCard** — for address books
- **JSON** — structured data
- **Excel** — .xlsx

Export covers the common downstream consumers: CSV for spreadsheet work (pivot tables, dedup by eye, mail-merge), vCard for importing straight into Outlook/Contacts address books, JSON for programmatic handoff to other systems, and native Excel .xlsx for polished client deliverables. Because the database already holds deduplicated, verified data, exports inherit that quality — no post-export cleaning required.

---

## Full OSINT Pipeline

```
Query: "Find contacts of CEOs at IT companies in Moscow"
    │
    ▼
1. Plan: coordinator decomposes into subtasks
    │
    ├─► Agent 1: Company list (USRLE aggregators)
    │     └─► search_business_directory, web_search, web_fetch
    │
    ├─► Agent 2: LinkedIn profiles of CEO/CTO
    │     └─► search_social, enrich_person
    │
    ├─► Agent 3: Contacts from official websites
    │     └─► parse_corporate_site, extract_contacts
    │
    └─► Agent 4: Government/commercial databases
          └─► web_search, web_fetch, python_exec
    │
    ▼
2. Extraction: extract_contacts (email, phone, social)
    │
    ▼
3. Verification: verify_email, verify_phone, verify_social_profile
    │
    ▼
4. Enrichment: enrich_company, enrich_person
    │
    ▼
5. Storage: ContactDb (SQLite/PG) + deduplication
    │
    ▼
6. Sync: CRM (amoCRM/Bitrix24/HubSpot)
    │
    ▼
7. Report: summary.md + contact table + export (CSV/vCard)
```

The pipeline's execution model: the **coordinator decomposes** the natural-language query into per-data-source subtasks (shown as agents 1–4), each of which is an independent researcher allocated its own tool set. All researcher agents run **in parallel** — read-only OSINT tools are concurrency-safe, so a company-list agent, a social agent, and a website agent never block each other. This is where OSINT work gets most of its speed: in real runs, tool calls peak at ~10–13 concurrent executions, and wall-clock savings of minutes per run come from that parallelism.

Each agent's raw output funnels into the **shared, ordered middle stages**:

1. **Planning** — the coordinator maps the goal to subtasks; planned subtasks are persisted so progress is observable and resumable.
2. **Extraction** — `extract_contacts` turns page text/HTML into structured, deduplicated contact records with confidence scores.
3. **Verification** — every email/phone/social hit is checked by its verifier before it counts as a lead; unverifiable items are flagged, not deleted.
4. **Enrichment** — company/person enrichment adds firmographics and social links so leads are contextualized.
5. **Storage** — `ContactDb` writes records with dedup/merge, so the same person found by three agents appears once.
6. **Sync** — optionally pushes to the CRM (with operator approval).
7. **Report** — `summary.md`, contact tables, and exports are written to the `--output` directory.

### Goal Mode for lead generation

Beyond the basic fan-out, the coordinator runs **Goal Mode** — a quality loop specifically valuable for lead-gen queries, where partial results are the norm. After the initial agents finish, an **LLM judge** reviews everything collected against the *original goal* and decides whether the goal is met. If concrete gaps remain (e.g. "no LinkedIn profiles found for 12 of 20 targets", "CTO contacts missing for 5 companies"), the judge proposes up to 3 gap-filling subtasks and a **replan round** launches to close exactly those gaps — not a blind re-run. Up to `replan_rounds` such rounds run (config in `[agent]`, default 1; `0` disables Goal Mode entirely), each capped by the remaining agent/session token budget, and the loop stops early as soon as the judge declares the goal satisfied. Subtask rows are kept in sync with agent outcomes throughout, so progress is visible even across replan rounds. For lead generation this means a run that initially finds only company-level contacts ("we have the switchboard, we don't have the CEO") explicitly retries with targeted follow-up queries until the specific contact gap is filled or the judge is confident nothing more is reachable. `replan_rounds` can also be overridden per request profile, giving operators a per-run knob between "single pass" (off) and "iterate until satisfied".

### Detection heuristics

The pipeline leans on a set of **deterministic heuristics** that combine precision with graceful degradation:

- **Email**: layered regex (plain → obfuscated → `mailto:`/JSON-LD), case-insensitive keyed dedup with highest-confidence winner, role/disposable local-part flags, corporate pattern detection across a domain, and permutation-based `suggest_emails` for unpublished addresses.
- **Phone**: raw text + E.164 normalized form kept together, country detection, mobile/landline classification.
- **Social**: (platform, handle, URL) triples with URL-bearing entries preferred over bare handles; lowercased-URL dedup across search results.
- **Businesses**: directory results merged in parallel and deduplicated by (name, address) with field backfill from duplicates.
- **Search**: `smart` mode runs all backends in parallel, dedupes by normalized URL (lowercase, fragment dropped, http→https, trailing slash trimmed), and ranks with **reciprocal rank fusion** — a result found by several backends scores higher — falling back to DuckDuckGo when every configured backend comes back empty.
- **Leads**: `find_leads` merges the directory + corporate-site + social + email-attribution passes into one ranked list, deduplicated by (person name, company name) keeping the higher-confidence entry, and sorted by confidence so the strongest leads surface first.
- **Entity extraction**: every OSINT run absorbs successful extractions into agent memory (as `contact`/`lead` facts), and the knowledge graph dedupes entities by name+type automatically.
- **Honesty**: agents mark unconfirmed data as `"requires verification"` instead of presenting it as fact — confidence scoring is the thread that ties every stage together.

---

## Limitations & Ethics

### Technical Limitations
- **LinkedIn** returns HTTP 999 (anti-bot) — proxies/cookies needed for stable parsing
- **2GIS/Google Maps** require API keys for reliable access
- Some sites block bots (403) — handled gracefully

These are accepted constraints rather than bugs: each blocked source degrades to a fallback (`search_social` falls back to general web search, blocked fetches are reported and skipped) and everything else continues. Directory coverage is conditional on configured API keys; when a directory is missing its key it is simply not queried. Rate limits are respected, and verification probes are designed to be non-intrusive (no `DATA`, no content delivery).

### Responsible Use
- Use only **open public data**
- Comply with **GDPR / 152-FZ** when handling personal data
- Do not spam extracted contacts
- Verify data before use (confidence scoring)
- Respect robots.txt and rate limits

The engine is a legitimate lead-gen and research tool: it collects what is already public, and it is the operator's responsibility to use the data lawfully. GDPR (EU) and 152-FZ (Russia) govern how personal data may be processed, and the confidence scores exist precisely so you can decide which records meet your quality bar before outreach. `save_contacts` requiring operator approval for CRM pushes is an additional enforcement point — human gates the moment where data leaves the research environment and enters a sales system.

### Result Honesty
Agents explicitly mark uncertainty:
- `"requires verification"` — data not confirmed
- `"not found"` — contact not discovered
- `"HTTP 999"` — source blocked access
- `"match unlikely"` — low confidence

Honesty is a first-class output property: the report distinguishes *known* from *suspected* from *missing*, so downstream users never mistake an unverified scrape for a confirmed lead. Low-confidence results remain in the export but carry their flags, letting users filter by confidence.

---

## Efficiency Tips

1. **Be specific in queries** — "IT companies 10-50 employees in Moscow, OKVED 62.01" is better than "companies in Moscow"
2. **Specify roles** — "CEO, CTO, Head of Engineering" narrows the search
3. **Use `smart` search** — parallel backends yield more results
4. **Set up CRM** — automatic sync saves time
5. **Check confidence** — filter out low-confidence results

Specificity is the cheapest optimization: precise firmographics (employee band, OKVED activity code, city) give the planner an actionable filter instead of a broad sweep, and named roles convert a company list into person-level leads. `smart` search is the recommended backend mode because parallel queries across providers plus reciprocal-rank fusion both widen coverage and rank the best results first. A configured CRM removes the export/import friction entirely — extraction pushes straight into the sales pipeline (after approval). And because every record carries a confidence score, the final list is trivially filterable to, say, "verified, non-role, SMTP-accepted emails only" for the highest-ROI outbound follow-up.