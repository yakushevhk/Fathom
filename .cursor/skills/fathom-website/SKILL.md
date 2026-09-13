---
name: fathom-website
description: Architecture, routing, bilingual i18n generation, and content conventions for the Fathom marketing website in website/. Use when modifying website pages (.astro), documentation (.mdx), translations, or styles.
---

# Fathom Marketing Website & Documentation

The website (`website/`) is built with Astro v5/v7 (static output + Vercel serverless functions) with custom post-build bilingual i18n generation.

## 1. Directory Layout (`website/`)

```
website/
├── src/
│   ├── pages/                   # Canonical English routes (.astro)
│   │   ├── bot.astro            # Fathom Bot showcase (using /images/bot/*)
│   │   ├── pricing.astro        # Pricing tiers ($79/mo Personal, $200-$500/mo Enterprise Node)
│   │   ├── architecture.astro   # 13 crates, 63 tools, Rust runtime
│   │   └── docs/                # Docs index and section entry points
│   ├── content/docs/            # MDX documentation source files
│   │   ├── bot.mdx              # Fathom Bot guide
│   │   ├── tools.mdx            # Tools catalog (63 built-in tools)
│   │   ├── quickstart.mdx       # Quickstart setup
│   │   └── architecture.mdx     # Architectural deep-dive
│   ├── i18n/
│   │   ├── translations.js      # Base translation dictionary (EN/RU)
│   │   ├── unified.js           # Deep-merged index of base + groups/*.js
│   │   ├── switcher.js          # Client-side language switcher & resolver
│   │   └── groups/              # Modular translation groups
│   │       ├── bot.js           # botPage.* keys
│   │       ├── meta.js          # Canonical page title/desc metadata
│   │       ├── postdocs.js      # Documentation UI translations
│   │       └── solutions.js     # Enterprise solutions copy
│   └── layouts/Layout.astro     # Main shell, SEO metadata, nav & footer
├── public/
│   └── images/
│       └── bot/                 # 7 canonical Fathom Bot screenshots
└── scripts/
    └── i18n-generate.mjs        # Post-build generator: emits dist/ru/* from dist/*
```

## 2. i18n & Translation Workflow

* **Bilingual Requirement**: Every customer-facing string MUST have both English (`en`) and Russian (`ru`) translations.
* **Build-Time Generation**:
  * The Astro build emits static HTML in `dist/` (English).
  * `scripts/i18n-generate.mjs` runs immediately after `astro build`. It walks the English HTML, replaces strings matching `data-i18n`, `data-i18n-html`, and meta tags, and generates `dist/ru/<path>/index.html`.
* **Runtime / Dev Switcher**:
  * `src/i18n/switcher.js` imports `unified.js` to resolve all group keys client-side during development.
* **Page Titles & Metadata**:
  * Add canonical localized titles and descriptions to `src/i18n/groups/meta.js`.

## 3. Screenshots & Visual Assets

* All Fathom Bot screenshots reside in `website/public/images/bot/`:
  1. `fathom-agent-thinking-stream-tool-call.png` (Live execution & reasoning)
  2. `fathom-bot-identity-settings.png` (Persona & instruction setup)
  3. `fathom-automations-calendar-schedule-modal.png` (24/7 calendar routines)
  4. `fathom-agent-setup-thought-run-steps.png` (Deterministic tool pipeline)
  5. `fathom-uzbekistan-vc-funds-research.png` (Deep OSINT & web intelligence)
  6. `fathom-tokyo-weather-report-card.png` (Interactive action cards)
  7. `fathom-system-resources-monitor-report.png` (Hardware telemetry & diagnostics)
* NEVER re-introduce deleted placeholder assets (`hero.png`, `approval-card.png`, etc.).

## 4. Build & Verification Commands

```bash
cd website
npm run build     # Runs `astro build && node scripts/i18n-generate.mjs`
```
