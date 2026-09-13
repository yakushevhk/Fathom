---
name: fathom-whitepaper
description: Standards, specifications, and data formatting for the Fathom Technical Whitepaper and Investor Pitch Deck. Use when updating pitch/pitch_deck.html, whitepaper/*.html, benchmarks, or founder credentials.
---

# Technical Whitepaper & Investor Pitch Deck Standards

Guidelines for maintaining investor presentation materials, formal engineering specifications, and accreditation compliance.

## 1. Official Startup Accreditation & Founder Credentials

All investor materials MUST strictly reflect verified government accreditation details:
* **StartupBase Accreditation ID**: `#1924` (Approved Startup, Category: AI in Business & Industry / *Sanoat va tadbirkorlikdagi SI*).
* **Awards**: Winner of PAA AI in Business & Industry Award.
* **Founder & CEO**: **German Yakushev** (`PINFL: 31210968380014`, Lead Systems Architect).
* **Co-Founders**:
  * **Mirtemir Anorboyev** (`PINFL: 51607005470010`, Product Manager).
  * **Elbek Yuldashev** (`PINFL: 51204066180078`, Head of Growth).
* **Rule**: Never use variant transliterations (e.g. avoid "Hermann") to ensure flawless legal due diligence.

## 2. Pitch Deck Design Standards (`pitch/pitch_deck.html`)

* **Aspect Ratio**: Strict **4:3** (`1600px × 1200px`).
* **Visual Theme**: Minimalist, high-contrast, clean white background (`#ffffff` / `#f8fafc`).
* **Typography**: *Plus Jakarta Sans* (headlines, body) + *JetBrains Mono* (metrics, code, data tokens).
* **Narrative Flow**:
  1. Problem (Cloud data leakage, fragile Python wrappers, unmonitored execution).
  2. Solution (Compiled Rust runtime, Fathom Bot chat client, governed execution).
  3. Product walkthrough interspersed with market proof points (IDC $1.3T+ Agentic AI forecast, Uzbekistan Law #547 regulatory moat).
  4. Unit economics & business model ($79/mo Personal, $200–$500/mo Enterprise Node, unlimited tokens on customer hardware).
  5. Traction, 18-month runway allocation (percentage-based), team, and call to action.
* **Funding Numbers**: DO NOT state arbitrary hardcoded dollar valuations or target raises. Emphasize strategic milestones, 18-month runway, and unit economics.

## 3. Technical Whitepaper Specifications (`whitepaper/`)

* **Standalone HTML Deck**: 42 pages rendered in high-density typography.
* **Mathematical Formulations**: Formalize multi-agent fan-out, DAG execution trees, token compaction algorithms, and cryptographic verification hash chains.
* **Benchmark Claims**: Ground all speed claims in measured reality:
  * Sub-millisecond dispatch: `~0.75 ms` overhead.
  * Baseline memory consumption: `~15.4 MB`.
  * In-process SQLite FTS5 memory absorption: `~94 µs`.
