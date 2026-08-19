# OSINT & Lead Generation

Parallel Research is an **OSINT engine for contact harvesting**: agents search for companies and people in open sources, extract emails/phones/social networks, verify and enrich data, store it in a database, and sync with CRM.

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

---

## Example Queries

### Finding company executives

```bash
parallel-research run \
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

### Finding SaaS founders

```bash
parallel-research run \
  "Find 20 Series A-B SaaS companies in Berlin. \
   Collect emails and LinkedIn profiles of founders and CTOs." \
  --output ./berlin-saas/
```

### Industry analysis + contacts

```bash
parallel-research run \
  "Research the fintech startup market in Dubai. \
   For the top 10 companies, find contacts of decision-makers." \
  --output ./dubai-fintech/
```

---

## What Gets Extracted

### Email Addresses
- Regex patterns for standard emails
- Deobfuscation: `name [at] domain [dot] com`, `name (at) domain (dot) com`
- From `mailto:` links, JSON-LD blocks
- **Confidence scoring**: plain (0.95), obfuscated (0.7)

### Phone Numbers
- International formats (+1, +7, +44, +86...)
- Local formats with area codes
- Normalization to **E.164** via libphonenumber
- Country detection, mobile/landline

### Social Networks
- LinkedIn (`linkedin.com/in/`, `/company/`)
- Twitter/X (`twitter.com/`, `x.com/`, `@username`)
- Instagram (`instagram.com/`)
- Telegram (`t.me/`, `@username`)
- Facebook (`facebook.com/`)

### Persons
- Name, title, company
- Email, phone, social networks

### Companies
- Name, website, industry, size, location
- Employee list

---

## Data Sources

### Business Directories
- **2GIS** — CIS companies
- **Google Maps / Places** — globally
- **Yandex Maps** — Russia/CIS
- **Yellow Pages** — international

### USRLE Aggregators (Russia)
- rusprofile.ru
- list-org.com
- sbis.ru
- companies.rbc.ru

### Corporate Websites
- Team pages, About pages
- Contacts, press releases
- Schema.org markup, JSON-LD

### Social Networks
- Twitter/X search
- Telegram channels/users
- LinkedIn (requires anti-bot bypass)

### News
- Serper News API
- Google News RSS

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

### Phone
```
verify_phone("+7 (495) 710-75-80")
→ normalized: "+74957107580"
→ country_code: "RU"
→ is_valid: true
→ is_mobile: false
```

### Social Networks
```
verify_social_profile("https://linkedin.com/in/john-doe")
→ exists: true
→ platform: "linkedin"
→ username: "john-doe"
```

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

### PostgreSQL (for large databases)

```toml
[contacts]
pg_url = "postgres://user:pass@localhost/contacts"
```

Same interface as SQLite.

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

---

## Contact Export

Formats (see [CONFIGURATION.md](CONFIGURATION.md) `[export]`):
- **CSV** — for Excel/Google Sheets
- **vCard** — for address books
- **JSON** — structured data
- **Excel** — .xlsx

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

---

## Limitations & Ethics

### Technical Limitations
- **LinkedIn** returns HTTP 999 (anti-bot) — proxies/cookies needed for stable parsing
- **2GIS/Google Maps** require API keys for reliable access
- Some sites block bots (403) — handled gracefully

### Responsible Use
- Use only **open public data**
- Comply with **GDPR / 152-FZ** when handling personal data
- Do not spam extracted contacts
- Verify data before use (confidence scoring)
- Respect robots.txt and rate limits

### Result Honesty
Agents explicitly mark uncertainty:
- `"requires verification"` — data not confirmed
- `"not found"` — contact not discovered
- `"HTTP 999"` — source blocked access
- `"match unlikely"` — low confidence

---

## Efficiency Tips

1. **Be specific in queries** — "IT companies 10-50 employees in Moscow, OKVED 62.01" is better than "companies in Moscow"
2. **Specify roles** — "CEO, CTO, Head of Engineering" narrows the search
3. **Use `smart` search** — parallel backends yield more results
4. **Set up CRM** — automatic sync saves time
5. **Check confidence** — filter out low-confidence results