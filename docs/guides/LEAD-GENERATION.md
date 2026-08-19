# Lead Generation Guide

A practical step-by-step walkthrough of the lead-generation pipeline in Fathom. Each stage is a composable tool you can call independently or chain together in a single `fathom run` query.

---

## Pipeline Overview

```
Query → 1. Query Formulation → 2. Social Search → 3. Company Enrichment
                                              ↘ 4. Email/Phone Verification
                                                      ↘ 5. Contact DB
                                                           ↘ 6. CRM Push
```

The pipeline is designed around **narrow, composable tools** — each stage produces structured JSON that the next stage consumes. You can run the full pipeline end-to-end with a single `fathom run` command, or drive each tool manually for fine-grained control.

---

## 1. Query Formulation

Good queries are the cheapest optimisation in the entire pipeline. The planner decomposes your natural-language query into parallel subtasks; vague queries produce broad sweeps that waste time on irrelevant results.

### Anatomy of an effective lead-gen query

| Component | Example | Why it matters |
|-----------|---------|----------------|
| **Industry** | `IT companies`, `fintech startups`, `SaaS` | Filters the company-first pass to relevant directories |
| **Location** | `in Berlin`, `Moscow`, `Dubai` | Scopes business directory searches (2GIS, Google Maps, Yandex) |
| **Company size** | `10-50 employees`, `Series A-B` | Avoids one-person shops and enterprise giants |
| **Roles** | `CEO, CTO, Head of Engineering` | Converts a company list into person-level leads |
| **Output format** | `--output ./leads/` | Writes summary, contact table, and exports |

### Examples

```bash
# Basic: broad industry + location
fathom run \
  "Find contacts of executives at IT companies in Moscow. \
   Extract emails, phones, LinkedIn profiles." \
  --output ./leads/

# Targeted: specific size band + named roles
fathom run \
  "Find 20 SaaS companies in Berlin (Series A-B). \
   Collect emails of founders and CTOs." \
  --output ./berlin-saas/

# Industry + geography + decision-maker roles
fathom run \
  "Research the fintech startup market in Dubai. \
   For the top 10 companies, find contacts of decision-makers." \
  --output ./dubai-fintech/

# Precise firmographics (best results)
fathom run \
  "IT companies 10-50 employees in Moscow, OKVED 62.01. \
   Find CEO, CTO, Head of Engineering contacts with emails." \
  --output ./moscow-it/
```

### What the planner does

1. **Decomposes** the query into parallel sub-tasks (company list, social search, website scraping, news/mentions).
2. **Allocates** each sub-task to an independent researcher agent with its own tool set.
3. **Runs agents concurrently** — tool calls peak at 10–13 concurrent executions, saving minutes of wall-clock time.
4. **Funnels** all raw output into the shared pipeline stages (extraction → verification → enrichment → storage).

### Pro tips

- **Be specific** — `"IT companies 10-50 employees in Moscow, OKVED 62.01"` outperforms `"companies in Moscow"`.
- **Name roles** — `"CEO, CTO, Head of Engineering"` tells the planner to do a person-first pass, not just a company list.
- **Use `smart` search** — the default backend mode runs all configured search engines in parallel and fuses results with reciprocal-rank fusion.
- **Set up CRM** — `save_contacts` pushes to the CRM automatically when configured (see [CRM Setup](#6-crm-push)).
- **Check confidence scores** — every record has a `confidence` field in `[0, 1]`; filter by it for high-ROI outreach.

---

## 2. Social Search

The `search_social` tool searches for people and companies across Twitter/X, Telegram, and LinkedIn. It is the **person-first** entry point: given a name, role, or company name, it finds social profiles.

### CLI usage

```bash
# Search all platforms for a person
fathom run \
  "Search social media for Jane Doe, CTO at Acme Corp" \
  --output ./social/

# Target a specific platform
fathom run \
  "Search LinkedIn for CTOs at fintech companies in London" \
  --output ./linkedin/

# Search by role + company
fathom run \
  "Search Telegram and Twitter for marketing directors at SaaS companies" \
  --output ./social/
```

### Tool reference

**Tool name:** `search_social`

| Parameter | Type | Description |
|-----------|------|-------------|
| `query` | `string` (required) | Person name, company name, or keywords like `"CTO Acme Corp"` |
| `platforms` | `string[]` | `twitter` (or `x`), `telegram`, `linkedin`. Defaults to all. |
| `limit` | `integer` | Max results (default 10, max 50) |

### Example output

```json
{
  "platform": "linkedin",
  "profile_url": "https://linkedin.com/in/jane-doe",
  "display_name": "Jane Doe",
  "handle": "jane-doe"
}
```

### What gets searched

- **Twitter/X** — Twitter API v2 when `PARALLEL_TWITTER_BEARER_TOKEN` is set; otherwise falls back to web search restricted to `x.com`/`twitter.com`.
- **Telegram** — web search restricted to `t.me`, enriched by fetching the public channel preview page (name, bio, subscriber count).
- **LinkedIn** — web search restricted to `linkedin.com` (LinkedIn blocks unauthenticated scraping, so results carry no follower counts).

All failures degrade gracefully to empty results — one blocked platform never blocks the pipeline.

### Telegram enrichment

When Telegram results are found, the tool fetches up to 5 `t.me` preview pages to extract:

- Channel/user name (from `og:title`)
- Bio (from `og:description`)
- Subscriber count (from `"N subscribers"` / `"N members"` text)

### Using social search as a lead source

The `find_leads` tool (see [end-to-end pipelines](#end-to-end-pipelines)) internally calls `search_social` with role titles from the query. Social results are cross-referenced against company names found in the directory pass: if a profile bio mentions one of the discovered companies, it is attached as a corroborated lead.

---

## 3. Company Enrichment

The `enrich_company` tool gathers firmographics — website, industry, size, revenue, founding year, headquarters, description, and detected technologies — for a company.

### CLI usage

```bash
# Enrich a company by name (website discovered automatically)
fathom run \
  "Enrich company: Acme Corp" \
  --output ./enrich/

# Enrich with a known website
fathom run \
  "Enrich company: Acme Corp (acme.com)" \
  --output ./enrich/
```

### Tool reference

**Tool name:** `enrich_company`

| Parameter | Type | Description |
|-----------|------|-------------|
| `company` | `string` (required) | Company name to enrich |
| `website` | `string` (optional) | Known website; discovered via search when omitted |

### Example output

```json
{
  "website": "https://acme.com",
  "industry": "Software Development",
  "company_size": "51-200",
  "employees": 120,
  "founded": 2015,
  "headquarters": "San Francisco, CA",
  "revenue": "$10 million",
  "description": "Acme Corp builds developer tools for teams.",
  "technologies": ["Next.js", "React", "Stripe", "HubSpot"]
}
```

### How it works

1. **Website discovery** — searches the web for the company's official website, filtering out aggregator/crunchbase/yellowpages URLs.
2. **Homepage fetch** — downloads the homepage with a browser-like User-Agent, parses meta tags, title, and HTML.
3. **Fact extraction** — scans search snippets and homepage text for:
   - **Employee count** — patterns like `"10,000+ employees"`, `"employs 250 people"`, `"1.2k employees"` → size bucket (`11-50`, `51-200`, `201-1000`, `1001-5000`, `5000+`).
   - **Founding year** — `"founded in 1994"`, `"Founded: 2010"`.
   - **Headquarters** — `"headquartered in Dublin, Ireland"`, `"based in Berlin"`.
   - **Revenue** — `"$5.2 billion in revenue"`, `"revenue of $340 million"`.
4. **Technology detection** — scans homepage HTML against 40+ signature markers (WordPress, Shopify, Next.js, React, Vue, Angular, HubSpot, Salesforce, Stripe, Shopify, Magento, Cloudflare, AWS, Google Analytics, Intercom, Zendesk, etc.).
5. **Industry classification** — keyword rules in priority order: `"software"|"saas"|"cloud"` → `"Software & SaaS"`, `"fintech"|"payment"|"banking"` → `"Fintech"`, etc.

### When to use

- Before outreach to understand a target company's tech stack and size.
- After `find_leads` to add context to company records.
- As a standalone research step for competitor analysis.

---

## 4. Email / Phone / Social Verification

Raw harvested contacts are guesses. Verification turns them into actionable leads.

### 4a. Email Verification

**Tool name:** `verify_email`

| Parameter | Type | Description |
|-----------|------|-------------|
| `email` | `string` (required) | Email address to verify |
| `smtp_check` | `boolean` (optional) | Run SMTP probe (default: `false`) |

```bash
# Basic verification
fathom run \
  "Verify email: info@acme.com" \
  --output ./verify/

# With SMTP probe (checks if mailbox actually accepts mail)
fathom run \
  "Verify email with SMTP check: jane.doe@acme.com" \
  --output ./verify/
```

#### Verification stages

| Stage | What it checks | Cost |
|-------|---------------|------|
| **Syntax** | RFC 5322 subset: one `@`, valid local part, dotted domain, non-numeric TLD | Free, instant |
| **Domain/MX** | DNS-over-HTTPS MX lookup (Google → Cloudflare fallback); A-record fallback if no MX exists | Fast, cached per session |
| **Disposable** | Known throwaway domains (mailinator.com, yopmail.com, 10minutemail.com, …) | Free, instant |
| **Role-based** | Local parts like `info@`, `support@`, `admin@`, `sales@`, `noreply@` | Free, instant |
| **SMTP probe** | Connects to best MX host on port 25, runs `HELO`/`MAIL FROM`/`RCPT TO` (never sends data) | Slow, 10s timeout |

#### Example output

```json
{
  "email": "jane.doe@acme.com",
  "is_valid_syntax": true,
  "domain_exists": true,
  "mx_records": ["mx1.acme.com", "mx2.acme.com"],
  "is_disposable": false,
  "is_role_based": false,
  "smtp_check": {
    "accepted": true,
    "mx_host": "mx1.acme.com",
    "response": "250 OK"
  },
  "confidence": 0.95
}
```

#### Confidence scoring

```
confidence = syntax(0.25) + domain_exists(0.25) + !disposable(0.25) + smtp_accepted(0.25)
```

- **0.95+** — syntax valid, domain exists, not disposable, SMTP accepted → highest quality.
- **0.75** — syntax valid, domain exists, not disposable, SMTP not run → good for bulk.
- **0.50** — syntax valid, domain exists but disposable or role-based → use with caution.
- **< 0.25** — syntax failure or domain doesn't exist → discard.

#### Email pattern suggestion

When you have a person's name and a company domain, `suggest_emails` generates candidate addresses using common corporate patterns:

```bash
fathom run \
  "Suggest email for Jane Doe at acme.com" \
  --output ./suggest/
```

Patterns tried (in order):
1. `first.last@domain` — `jane.doe@acme.com`
2. `firstinitial.last@domain` — `j.doe@acme.com`
3. `first@domain` — `jane@acme.com`
4. `firstlast@domain` — `janedoe@acme.com`
5. `first_initial_last@domain` — `jdoe@acme.com`
6. `first.lastinitial@domain` — `jane.d@acme.com`

When colleagues' emails are already known on the same domain, the tool **detects the structural pattern** (e.g., `first.last@` vs `firstinitialast@`) and generates only matching candidates.

### 4b. Phone Verification

**Tool name:** `verify_phone`

| Parameter | Type | Description |
|-----------|------|-------------|
| `phone` | `string` (required) | Phone number in any format |
| `default_country` | `string` (optional) | ISO 3166-1 alpha-2 for local format numbers |

```bash
# With international prefix
fathom run \
  "Verify phone: +7 (495) 710-75-80" \
  --output ./verify/

# Local format with default country
fathom run \
  "Verify phone: (415) 555-2671 with default country US" \
  --output ./verify/
```

#### Example output

```json
{
  "input": "+7 (495) 710-75-80",
  "normalized": "+74957107580",
  "country_code": "RU",
  "is_valid": true,
  "is_mobile": false,
  "line_type": "Fixed-line"
}
```

Phone verification uses Google's libphonenumber metadata in-process — no external API call, so it is fast enough to run on every harvested number.

### 4c. Social Profile Verification

**Tool name:** `verify_social_profile`

| Parameter | Type | Description |
|-----------|------|-------------|
| `url` | `string` (required) | Social profile URL |

```bash
fathom run \
  "Verify social profile: https://linkedin.com/in/jane-doe" \
  --output ./verify/

fathom run \
  "Verify social profile: https://github.com/torvalds" \
  --output ./verify/
```

#### Supported platforms

X/Twitter, Instagram, LinkedIn, Facebook, GitHub, TikTok, YouTube, Telegram, VK, Medium, Threads, Reddit, Pinterest, Twitch.

#### Example output

```json
{
  "url": "https://github.com/torvalds",
  "platform": "github",
  "username": "torvalds",
  "display_name": "Linus Torvalds",
  "followers": 245000,
  "exists": true
}
```

**Note:** LinkedIn, Instagram, and X may return HTTP 403 (bot protection). In that case `exists` is `false` with an explanatory note — treat as "unconfirmed", not "deleted".

---

## 5. Contact Database

The `save_contacts` tool persists harvested contacts into the contact database (SQLite or PostgreSQL), deduplicates against existing records, and optionally pushes to the CRM.

### Tool reference

**Tool name:** `save_contacts`

| Parameter | Type | Description |
|-----------|------|-------------|
| `contacts` | `object[]` (required) | Array of contacts — each needs at least one of `email`/`phone`/`name` |
| `push_crm` | `boolean` (optional) | Also push to CRM (default: `true`) |

### Contact input fields

| Field | Type | Description |
|-------|------|-------------|
| `email` | `string` | Email address |
| `phone` | `string` | Phone number |
| `name` | `string` | Full name |
| `title` | `string` | Job title |
| `company` | `string` | Company name |
| `socials` | `object[]` | `{platform, url?, username?}` |
| `tags` | `string[]` | Free-form tags |
| `notes` | `string` | Free-text notes |
| `source` | `string` | URL or tool the contact came from |

### Deduplication

Deduplication is **keyed by normalized email or phone**:

- **Email normalization** — `trim().toLowercase()`
- **Phone normalization** — keep only ASCII digits

When a new record matches an existing one, the data is **merged** rather than duplicated:
- Blank fields are filled from the new record.
- Social profiles, tags, and notes are *appended* (no duplicates).
- The merge is atomic inside the store (TOCTOU-safe).

### CLI usage

```bash
# save_contacts is called automatically by the pipeline — you don't invoke it directly.
# It runs at the end of every fathom run that produces contacts.
```

### Configuration

**SQLite** (default, zero setup):

```toml
[contacts]
db_path = "./contacts.db"
```

**PostgreSQL** (production / shared):

```toml
[contacts]
pg_url = "postgres://user:pass@localhost/contacts"
```

### Schema

```
contacts
  ├── id, email, phone, name, title, company, source, timestamps
  ├── social_profiles  → contact_id, platform, url, username
  ├── companies        → name, website, industry, size, location
  ├── tags             → contact_id, tag
  └── notes            → contact_id, note
```

### Export formats

After storage, contacts can be exported from the database:

| Format | File extension | Use case |
|--------|---------------|----------|
| **CSV** | `.csv` | Excel / Google Sheets / mail merge |
| **vCard** | `.vcf` | Outlook / Contacts address books |
| **JSON** | `.json` | Programmatic handoff |
| **Excel** | `.xlsx` | Polished client deliverables |

Configure in `[export]` section of `config.toml`.

---

## 6. CRM Push

Contacts are automatically pushed to the configured CRM when `save_contacts` runs with `push_crm: true` (the default).

### Supported CRMs

| Provider | Configuration |
|----------|---------------|
| **amoCRM** | `provider = "amocrm"`, `domain`, `api_key` |
| **Bitrix24** | `provider = "bitrix24"`, `domain`, `api_key` |
| **HubSpot** | `provider = "hubspot"`, `api_key` |

### Configuration

```toml
[crm]
provider = "amocrm"
domain = "yourcompany"    # yourcompany.amocrm.ru
api_key = "your-api-key"

# or

[crm]
provider = "bitrix24"
domain = "yourcompany"    # yourcompany.bitrix24.ru
api_key = "your-api-key"

# or

[crm]
provider = "hubspot"
api_key = "your-private-app-token"
```

### Safety

`save_contacts` sits in the **default approval tools** list — a human operator must explicitly approve CRM pushes in production. This is the gate where data leaves the research environment and enters the sales system.

### Resilience

Each contact push gets **one retry** for transient network errors. The tool's JSON summary separates `crm_pushed` from push errors so failures are visible rather than silent.

---

## End-to-End Pipelines

### Full pipeline (single command)

```bash
fathom run \
  "Find contacts of CEOs of IT companies in Moscow. \
   Extract emails and phone numbers." \
  --output ./leads/
```

This single command triggers the entire pipeline:

```
1. Plan: coordinator decomposes into 4 parallel agents
   ├─ Agent 1: Company list (USRLE aggregators, directories)
   ├─ Agent 2: LinkedIn profiles of CEOs
   ├─ Agent 3: Contacts from official websites
   └─ Agent 4: Government/commercial databases
       │
       ▼
2. Extract → extract_contacts (email, phone, social)
       │
       ▼
3. Verify → verify_email, verify_phone, verify_social
       │
       ▼
4. Enrich → enrich_company, enrich_person
       │
       ▼
5. Store → ContactDb (SQLite/PG) + dedup
       │
       ▼
6. Sync → CRM (amoCRM/Bitrix24/HubSpot)
       │
       ▼
7. Report → summary.md + contact table + export (CSV/vCard)
```

### Manual step-by-step

If you prefer to drive each tool manually (e.g., in an MCP client), the tools compose as follows:

```json
// Step 1: Find leads
{
  "tool": "find_leads",
  "args": {
    "industry": "IT",
    "location": "Moscow",
    "company_size": "10-50 employees",
    "role_titles": ["CEO", "CTO"],
    "limit": 10
  }
}

// Step 2: Enrich companies found
{
  "tool": "enrich_company",
  "args": {
    "company": "Acme Corp"
  }
}

// Step 3: Enrich people found
{
  "tool": "enrich_person",
  "args": {
    "name": "Jane Doe",
    "company": "Acme Corp"
  }
}

// Step 4: Search social profiles
{
  "tool": "search_social",
  "args": {
    "query": "Jane Doe Acme Corp",
    "platforms": ["linkedin", "twitter"],
    "limit": 5
  }
}

// Step 5: Verify emails
{
  "tool": "verify_email",
  "args": {
    "email": "jane.doe@acme.com",
    "smtp_check": true
  }
}

// Step 6: Verify phones
{
  "tool": "verify_phone",
  "args": {
    "phone": "+7 (495) 710-75-80",
    "default_country": "RU"
  }
}

// Step 7: Verify social profiles
{
  "tool": "verify_social_profile",
  "args": {
    "url": "https://linkedin.com/in/jane-doe"
  }
}

// Step 8: Save to database (and push to CRM)
{
  "tool": "save_contacts",
  "args": {
    "contacts": [
      {
        "email": "jane.doe@acme.com",
        "phone": "+74957107580",
        "name": "Jane Doe",
        "title": "CEO",
        "company": "Acme Corp",
        "socials": [
          {
            "platform": "linkedin",
            "url": "https://linkedin.com/in/jane-doe",
            "username": "jane-doe"
          }
        ],
        "tags": ["CEO", "IT", "Moscow"],
        "source": "find_leads"
      }
    ],
    "push_crm": true
  }
}
```

### Watch mode (repeated runs)

```bash
# Re-run every 6 hours, alert on new contacts
fathom run \
  "Find new IT companies in Moscow with CEO contacts" \
  --output ./leads/ \
  --repeat 21600
```

The watch loop maintains a set of known contact identity keys between runs, so only genuinely new contacts trigger alerts.

---

## Goal Mode

Beyond the basic fan-out, the coordinator runs **Goal Mode** — a quality loop specifically valuable for lead-gen queries. After the initial agents finish, an LLM judge reviews everything collected against the *original goal* and decides whether it is met.

If concrete gaps remain (e.g., "no LinkedIn profiles found for 12 of 20 targets", "CTO contacts missing for 5 companies"), the judge proposes up to 3 gap-filling subtasks and a **replan round** launches to close exactly those gaps — not a blind re-run.

Configure replan rounds:

```toml
[agent]
replan_rounds = 1   # default; 0 disables Goal Mode
```

---

## Best Practices

### Query design

1. **Stack filters** — industry + location + size + role together.
2. **Use OKVED codes** for Russian companies — the USRLE aggregators search by them.
3. **Name role titles explicitly** — the planner uses them to set `role_titles` in `find_leads`.
4. **Set `--output`** — results are written to disk; without it they are only displayed in the terminal.

### Confidence-based filtering

Every record carries a `confidence` score. Use it to prioritize:

| Confidence | Meaning | Action |
|-----------|---------|--------|
| 0.90–1.00 | Verified, non-role, SMTP-accepted | Highest-ROI outreach |
| 0.75–0.89 | Syntax valid, domain exists, not disposable | Good for bulk sequences |
| 0.50–0.74 | Valid but role-based or disposable | Use with caution |
| < 0.50 | Unverified or syntax failure | Discard or re-verify |

### Performance

- **Parallelise by default** — the planner runs agents concurrently; you don't need to manage this.
- **SMTP probes are slow** — only enable `smtp_check` for short lists of high-priority leads.
- **MX lookups are cached** per session — repeated verifications of the same domain hit cache, not the network.
- **Set `limit`** — `find_leads` defaults to 10; increase to 50–100 for broad sweeps, decrease for quick checks.

### Data quality

- **Enrich before verifying** — `enrich_person` often finds contact details that the initial extraction missed.
- **Verify in batches** — verification tools are read-only and parallel-safe.
- **Corroborate across platforms** — an email found on both a company site and a LinkedIn profile is more reliable than one found in a single snippet.
- **Use `enrich_entities: true`** in `extract_contacts` to get LLM-assisted person/company extraction from team pages.

### Responsible use

- Use only **open public data**.
- Comply with **GDPR** (EU) and **152-FZ** (Russia) when handling personal data.
- **Do not spam** extracted contacts.
- Respect `robots.txt` and rate limits.
- `save_contacts` requires operator approval for CRM pushes — use this gate intentionally.

---

## Tool Reference Summary

| Tool | Purpose | Key inputs |
|------|---------|------------|
| `find_leads` | Full lead-generation pipeline | `industry`, `location`, `company_size`, `role_titles`, `limit` |
| `search_social` | Social media people/company search | `query`, `platforms`, `limit` |
| `enrich_company` | Company firmographics | `company`, `website` (optional) |
| `enrich_person` | Person profile enrichment | `name`, `company` (optional) |
| `extract_contacts` | Email/phone/social extraction from text/HTML/URL | `text`, `html`, `url`, `enrich_entities` |
| `verify_email` | Email deliverability check | `email`, `smtp_check` |
| `verify_phone` | Phone number validation + normalization | `phone`, `default_country` |
| `verify_social_profile` | Social profile existence check | `url` |
| `suggest_emails` | Email pattern generation | name + domain (inferred) |
| `save_contacts` | Persist to DB + CRM push | `contacts`, `push_crm` |