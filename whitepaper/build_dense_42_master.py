#!/usr/bin/env python3
import os, subprocess, concurrent.futures, tempfile, shutil
from pypdf import PdfWriter, PdfReader

WP_DIR = "/Users/yakushev/Documents/GitHub/Fathom/whitepaper"
CHROME_PATH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

pages = []

def add_page(num, category, title, subtitle, content_html):
    pages.append({
        "num": num,
        "category": category,
        "title": title,
        "subtitle": subtitle,
        "html": content_html.strip()
    })

def mockup_embed(filename, caption_text):
    return f"""
    <div class="deck-mockup-container">
      <div class="deck-mockup-frame">
        <img src="mockups/{filename}" alt="{caption_text}">
      </div>
      <div class="deck-mockup-caption">{caption_text}</div>
    </div>
    """

# ==============================================================================
# 42 ULTRA-DENSE PAGES DEFINITION (01 TO 42)
# ==============================================================================

# ------------------------------------------------------------------------------
# PART I: VISION & AUTONOMOUS REMOTE WORKFORCE (01-05)
# ------------------------------------------------------------------------------

# Page 01
add_page(1, "EXECUTIVE WHITEPAPER · STRATEGIC OVERVIEW",
"Universal Autonomous AI Workforce Runtime",
"High-Performance Rust Architecture for End-to-End Remote Digital Employees",
"""
<div class="card-dark" style="padding: 10px 14px; margin-bottom: 10px;">
  <div class="card-title" style="font-size: 9.5pt; margin-bottom: 4px;">The Paradigm Shift: From Scripted Automation to Autonomous Digital Colleagues</div>
  <p style="font-size: 8.2pt; line-height: 1.45;">
    <strong>Fathom</strong> represents an architectural breakthrough in enterprise digital labor: a production-grade, self-hosted <strong>Rust runtime</strong> designed to instantiate, govern, and coordinate fleets of <strong>autonomous remote digital employees</strong>. Unlike legacy chat wrappers, brittle RPA scripts, or toy Python agent frameworks, Fathom agents operate as fully autonomous knowledge workers. They independently decompose ambiguous high-level business objectives, navigate arbitrary SaaS web portals via semantic accessibility trees, conduct multi-day OSINT investigations, engineer and test production software, and interact across corporate communication channels 100% remotely.
  </p>
</div>

<div class="grid-2" style="margin-bottom: 10px;">
  <div class="card card-accent" style="padding: 9px 12px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 8.5pt;">Autonomous Remote Execution</div>
    <p style="font-size: 7.8pt; line-height: 1.4;">
      Digital coworkers maintain continuous, long-lived operational presence inside isolated sandboxes. They execute scheduled routines, triage incoming enterprise webhooks, manage customer onboarding, and perform complex multi-source research without requiring continuous human prompting or synchronous supervision.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 9px 12px;">
    <div class="card-title-sm" style="color:#059669; font-size: 8.5pt;">Microsecond Tokio Engine</div>
    <p style="font-size: 7.8pt; line-height: 1.4;">
      Built from the ground up in memory-safe Rust with Tokio asynchronous I/O and zero-cost abstractions. Fathom delivers <strong>~0.75 ms tool dispatch latency</strong>, <strong>94 µs memory absorption</strong>, and operates with a featherweight <strong>15.4 MB RAM footprint</strong>—enabling hundreds of concurrent agents per node.
    </p>
  </div>
</div>

<div class="card card-slate" style="padding: 10px 14px; margin-bottom: 10px;">
  <div class="card-title" style="font-size: 8.8pt; margin-bottom: 6px;">Enterprise Architectural Pillars & Strategic Capabilities</div>
  <div class="grid-3">
    <div>
      <strong style="color:#0f172a; font-size:8pt;">1. Governed Computer Use</strong>
      <p style="font-size:7.4pt; color:#475569; margin-top:2px;">
        Operates Playwright-driven browsers using accessibility reference IDs (<code>@e1-@e48</code>) rather than fragile coordinate clicking. Integrates human-in-the-loop takeover leases for 2FA SMS and CAPTCHA gates.
      </p>
    </div>
    <div>
      <strong style="color:#0f172a; font-size:8pt;">2. 7-Layer OSINT Engine</strong>
      <p style="font-size:7.4pt; color:#475569; margin-top:2px;">
        Automates corporate registry harvesting (Companies House, Handelsregister), social corroboration, multi-source contact extraction, and real-time SMTP 250 OK mailbox handshakes with 0% bounce rate.
      </p>
    </div>
    <div>
      <strong style="color:#0f172a; font-size:8pt;">3. Flat-Rate Seat Economics</strong>
      <p style="font-size:7.4pt; color:#475569; margin-top:2px;">
        High-throughput provider routing to frontier open-weight and cost-efficient foundation models (Kimi k3, Qwen 3.8 Max, GLM 5.3) eliminates per-token anxiety, delivering 90%+ gross margins for enterprises and agencies.
      </p>
    </div>
  </div>
</div>

<div class="card card-purple" style="padding: 8px 12px;">
  <div class="card-title-sm" style="color:#7c3aed; font-size:8.2pt;">Document Scope & Executive Structure</div>
  <p style="font-size: 7.4pt; color:#334155;">
    This 42-page technical and economic whitepaper provides an exhaustive exploration of Fathom's Rust runtime internals, digital employee personas, OSINT outbound pipelines, governed computer use, sub-millisecond benchmarks, multi-channel gateways, self-healing background jobs, security governance, and the self-replicating growth flywheel.
  </p>
</div>
""")

# Page 02
add_page(2, "PART I: VISION & DIGITAL WORKFORCE",
"The Autonomous Digital Employee Paradigm",
"Transitioning from Ephemeral Chat Sessions to Persistent Corporate Coworkers",
"""
<div class="card-slate" style="padding: 10px 14px; margin-bottom: 10px;">
  <div class="card-title" style="font-size: 9.2pt;">The Fundamental Divide: Assistant Bots vs. Autonomous Digital Coworkers</div>
  <p style="font-size: 8pt; line-height: 1.45;">
    Traditional AI tools operate as passive, ephemeral chat assistants: they wait for human commands, generate text within temporary context windows, and forget everything once the browser tab closes. <strong>Fathom digital coworkers</strong> operate under a fundamentally distinct paradigm of persistent corporate identity, autonomous initiative, durable state machines, and scheduled background agency.
  </p>
</div>

<table class="table-deck" style="margin-bottom: 10px;">
  <thead>
    <tr>
      <th style="width: 24%;">Dimension</th>
      <th style="width: 38%;">Traditional AI Assistants (Python / LangChain)</th>
      <th style="width: 38%;">Fathom Digital Coworkers (Rust Tokio Engine)</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td><strong>Lifecycle & State</strong></td>
      <td>Ephemeral, single-turn prompts; memory discarded on session close.</td>
      <td><strong>Persistent SQLite/Graph state</strong>; durable multi-day job resumption.</td>
    </tr>
    <tr>
      <td><strong>Initiative</strong></td>
      <td>100% reactive; cannot act unless a human types a prompt.</td>
      <td><strong>Autonomous background crons</strong>, atomic schedule claiming, webhook triggers.</td>
    </tr>
    <tr>
      <td><strong>Computer Interaction</strong></td>
      <td>Brittle visual screenshot clicking or mock tool wrappers.</td>
      <td><strong>Accessibility Tree (ARIA) DOM manipulation</strong> with live operator takeover.</td>
    </tr>
    <tr>
      <td><strong>Outreach & Verification</strong></td>
      <td>Generic web scraping; unverified email hallucinations.</td>
      <td><strong>7-layer OSINT gauntlet</strong> with live SMTP 250 OK mailbox handshakes.</td>
    </tr>
    <tr>
      <td><strong>Runtime Efficiency</strong></td>
      <td>Python interpreter, 800+ MB RAM per agent, high GC pause.</td>
      <td><strong>Compiled Rust binary</strong>, 15.4 MB RAM footprint, ~0.75 ms tool dispatch.</td>
    </tr>
    <tr>
      <td><strong>Governance & Security</strong></td>
      <td>Raw API keys injected into LLM prompt context (high leak risk).</td>
      <td><strong>AES-256-GCM encrypted vault</strong>; zero LLM prompt visibility.</td>
    </tr>
  </tbody>
</table>

<div class="grid-2">
  <div class="card card-accent" style="padding: 9px 12px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 8.4pt;">Durable Coworker Profiles & Channel Binding</div>
    <p style="font-size: 7.6pt; line-height: 1.4;">
      Each coworker is instantiated with a declared persona, tool allowlist, system prompt, long-term memory graph, and channel bindings (Telegram chat ID, Slack workspace, or corporate email inbox). Coworkers claim scheduled background tasks atomically using SQLite <code>UPDATE ... WHERE claimed_at IS NULL</code>, preventing duplicate execution across clustered nodes.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 9px 12px;">
    <div class="card-title-sm" style="color:#059669; font-size: 8.4pt;">Multi-Turn Reasoning & Self-Correction</div>
    <p style="font-size: 7.6pt; line-height: 1.4;">
      When encountering transient HTTP 429 rate limits, DOM structural shifts, or CAPTCHA challenges, Fathom coworkers do not crash. They inspect prior error stack traces, apply exponential backoff with jitter, rotate proxies, and self-heal autonomously—holding difficult exceptions for human review via asynchronous messaging.
    </p>
  </div>
</div>
""")

# Page 03
add_page(3, "PART I: VISION & DIGITAL WORKFORCE",
"Virtual Office Topology & Autonomous Coordination",
"Hierarchical Task Decomposition, Specialized Agent Pods & Shared State",
"""
<div class="card-slate" style="padding: 10px 14px; margin-bottom: 10px;">
  <div class="card-title" style="font-size: 9.2pt;">Orchestrating Autonomous Teams: The Virtual Office Architecture</div>
  <p style="font-size: 8pt; line-height: 1.45;">
    Complex enterprise objectives—such as conducting full due diligence on an industry sector or managing a high-volume outbound sales campaign—exceed the context window and capability of any single model. Fathom organizes digital workers into a structured <strong>Virtual Office Topology</strong>: a hierarchical multi-agent network featuring specialized personas, parallel execution pods, and a shared long-term memory graph.
  </p>
</div>

<div class="flow-container" style="margin-bottom: 10px;">
  <div class="flow-step" style="width: 22%;">
    <div class="flow-title">1. Coordinator</div>
    <div class="flow-desc">Decomposes goal into directed acyclic graph (DAG) of subtasks with token budgets.</div>
  </div>
  <div class="flow-arrow">→</div>
  <div class="flow-step" style="width: 24%;">
    <div class="flow-title">2. Specialized Pods</div>
    <div class="flow-desc">Parallel execution across OSINT, Computer Use, Code Maintenance & Finance pods.</div>
  </div>
  <div class="flow-arrow">→</div>
  <div class="flow-step" style="width: 22%;">
    <div class="flow-title">3. Synthesis & Verification</div>
    <div class="flow-desc">LLM Judge validates deliverable completeness against acceptance criteria.</div>
  </div>
  <div class="flow-arrow">→</div>
  <div class="flow-step" style="width: 22%;">
    <div class="flow-title">4. Action & Sync</div>
    <div class="flow-desc">Pushes staged deal cards to CRM, opens GitHub PRs, or sends client emails.</div>
  </div>
</div>

<div class="grid-3" style="margin-bottom: 10px;">
  <div class="card card-accent" style="padding: 8px 10px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 8pt;">Chief of Staff / Manager</div>
    <p style="font-size: 7.2pt; color:#475569;">
      Orchestrates the entire fleet. Performs morning inbox zero sweeps, monitors background cron jobs, allocates token quotas, and prepares daily executive briefings.
    </p>
  </div>
  <div class="card card-amber" style="padding: 8px 10px;">
    <div class="card-title-sm" style="color:#d97706; font-size: 8pt;">Sales Outbound SDR</div>
    <p style="font-size: 7.2pt; color:#475569;">
      Conducts autonomous lead generation, mines corporate registries, validates email deliverability via SMTP probes, and stages qualified deals in amoCRM/HubSpot.
    </p>
  </div>
  <div class="card card-purple" style="padding: 8px 10px;">
    <div class="card-title-sm" style="color:#7c3aed; font-size: 8pt;">DevOps Maintainer</div>
    <p style="font-size: 7.2pt; color:#475569;">
      Triages Sentry error alerts, reproduces bugs in isolated sandboxes, modifies codebases using AST symbol search, and opens clean pull requests with passing tests.
    </p>
  </div>
</div>

<div class="card card-emerald" style="padding: 8px 12px;">
  <div class="card-title-sm" style="color:#059669; font-size: 8.2pt;">Inter-Worker Communication & Shared Knowledge Fabric</div>
  <p style="font-size: 7.5pt; color:#334155;">
    Workers communicate asynchronously via SQLite-backed event buses and share a persistent SQLite entity knowledge graph. When the Market Intel bot detects a competitor price drop, it writes the fact to the graph in 94 µs; the Outbound SDR immediately references the updated intelligence in its afternoon email outreach without human intervention.
  </p>
</div>
""")

# Page 04: MOCKUP 07 (Inbox Manager)
add_page(4, "PART I: VISION & DIGITAL WORKFORCE",
"A Day in the Life of an Autonomous Digital Coworker",
"End-to-End Operational Lifecycle: Morning Sweeps, Routine Execution & Executive Briefings",
f"""
<div class="card-slate" style="padding: 8px 12px; margin-bottom: 6px;">
  <p style="font-size: 7.8pt; line-height: 1.4;">
    To understand how Fathom operates in enterprise production, consider the daily operational routine of an autonomous <strong>Chief of Staff</strong> coworker. Operating 24/7 on a Tokyo cloud node, it performs proactive morning sweeps, triages executive inboxes, drafts context-aware replies in the founder's tone, and flags critical contract renewals.
  </p>
</div>

{mockup_embed("07_inbox_manager.png", "Figure 4.1: Autonomous Chief of Staff — 41-thread inbox sweep with noise archival, automated calendar scheduling and held executive drafts")}

<div class="grid-3" style="margin-top: 6px;">
  <div class="card card-accent" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 7.8pt;">06:00 AM · Morning Sweep</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Fetches 41 unread emails via IMAP/Gmail API. Automatically archives 26 marketing receipts and newsletters. Auto-replies to 9 routine calendar requests.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#059669; font-size: 7.8pt;">06:05 AM · Executive Drafting</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Identifies 6 high-value correspondence threads. Drafts personalized replies matching the executive's voice and parks them in the review queue.
    </p>
  </div>
  <div class="card card-purple" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#7c3aed; font-size: 7.8pt;">06:10 AM · Telegram Notification</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Sends a concise 3-bullet summary to the founder's Telegram: "Inbox at zero. Nora's contract question drafted for approval. 5 items staged."
    </p>
  </div>
</div>
""")

# Page 05
add_page(5, "PART I: VISION & DIGITAL WORKFORCE",
"Core Engine Architecture & Sub-Millisecond Dispatch",
"Microsecond Tokio Async I/O, Zero-Cost Tool Routing & Memory Footprint",
"""
<div class="card-slate" style="padding: 10px 14px; margin-bottom: 10px;">
  <div class="card-title" style="font-size: 9.2pt;">The Rust Advantage: Eliminating the Python Agent Bottleneck</div>
  <p style="font-size: 8pt; line-height: 1.45;">
    Most existing agent systems are built on Python (LangChain, AutoGPT, CrewAI), inheriting severe operational flaws: 800+ MB RAM consumption per agent, Global Interpreter Lock (GIL) contention, unpredictable garbage collection pauses, and multi-second tool dispatch latency. <strong>Fathom is written in 100% native Rust</strong>, achieving enterprise-grade performance and resource efficiency.
  </p>
</div>

<div class="grid-3" style="margin-bottom: 10px;">
  <div class="stat-box">
    <div class="stat-val">~0.75 ms</div>
    <div class="stat-label">Tool Dispatch Latency (Rust vs 240ms Python)</div>
  </div>
  <div class="stat-box" style="border-left-color:#059669;">
    <div class="stat-val" style="color:#059669;">15.4 MB</div>
    <div class="stat-label">Resident Set Size (RAM per Idle Coworker)</div>
  </div>
  <div class="stat-box" style="border-left-color:#7c3aed;">
    <div class="stat-val" style="color:#7c3aed;">94 µs</div>
    <div class="stat-label">Memory Absorption Time into SQLite Graph</div>
  </div>
</div>

<div class="card-dark" style="padding: 10px 14px; margin-bottom: 10px;">
  <div class="card-title" style="font-size: 8.8pt; color:#38bdf8; margin-bottom: 4px;">Tokio Asynchronous Event Loop & Atomic Crate Architecture</div>
  <p style="font-size: 7.6pt; color:#d4d4d8; line-height: 1.45;">
    Fathom's internal workspace is structured into highly cohesive, loosely coupled modular crates:
  </p>
  <div class="grid-2" style="margin-top: 6px;">
    <div style="font-size: 7.3pt; color:#a1a1aa; font-family:'JetBrains Mono',monospace;">
      • <strong>crates/core:</strong> Domain primitives & durable state machines<br>
      • <strong>crates/llm:</strong> Streaming provider router with Hermes compaction<br>
      • <strong>crates/agent:</strong> Autonomous multi-turn reasoning loop
    </div>
    <div style="font-size: 7.3pt; color:#a1a1aa; font-family:'JetBrains Mono',monospace;">
      • <strong>crates/server:</strong> Axum HTTP/SSE server & channel bridges<br>
      • <strong>crates/supervisor:</strong> Playwright & Docker sandboxing engine<br>
      • <strong>crates/governance:</strong> AES-256-GCM vault & fail-closed policies
    </div>
  </div>
</div>

<div class="card card-emerald" style="padding: 8px 12px;">
  <div class="card-title-sm" style="color:#059669; font-size: 8.2pt;">Zero-Cost Abstractions & Extreme Concurrency</div>
  <p style="font-size: 7.5pt; color:#334155;">
    By utilizing non-blocking Tokio asynchronous I/O and compiled Rust abstractions, a single 16-core dedicated server can host over <strong>500 simultaneous active digital employees</strong> handling web crawling, CRM updates, and code compilation concurrently with zero CPU thrashing.
  </p>
</div>
""")

# Page 06: MOCKUP 01 (Sales Outbound SDR)
add_page(6, "PART II: PERSONAS & SPECIALIZED COWORKERS",
"The Autonomous Sales Development Representative (SDR)",
"Corporate Registry Mining, Decision-Maker Harvesting & Multi-Channel Pipeline Staging",
f"""
<div class="card-slate" style="padding: 8px 12px; margin-bottom: 6px;">
  <p style="font-size: 7.8pt; line-height: 1.4;">
    The <strong>Autonomous SDR</strong> coworker independently executes full-cycle outbound prospecting: mining official government registries (Companies House UK, Handelsregister DE), extracting executive decision-makers, validating direct email deliverability via live SMTP 250 OK handshakes, and staging deal cards in amoCRM.
  </p>
</div>

{mockup_embed("01_sales_outbound_sdr.png", "Figure 6.1: Autonomous Outbound SDR — Corporate registry discovery, SMTP 250 OK verification & amoCRM pipeline sync")}

<div class="grid-3" style="margin-top: 6px;">
  <div class="card card-accent" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 7.8pt;">1. Registry Harvesting</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Parses Companies House and official gazettes to find incorporated entities meeting exact industry, revenue, and geographical criteria.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#059669; font-size: 7.8pt;">2. SMTP 250 OK Probes</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Executes RFC 5321 compliant SMTP mail exchanger handshakes without sending test emails, ensuring 0% bounce rate before CRM staging.
    </p>
  </div>
  <div class="card card-purple" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#7c3aed; font-size: 7.8pt;">3. 2-Way CRM Sync</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Stages qualified contacts in amoCRM with verified tech tags (AWS, Rust, Stripe) and auto-drafts icebreakers tailored to recent press.
    </p>
  </div>
</div>
""")

# Page 07: MOCKUP 02 (Market Intelligence)
add_page(7, "PART II: PERSONAS & SPECIALIZED COWORKERS",
"Market Intelligence & Competitive Tracking Coworker",
"Continuous DOM Diffing, Pricing Shift Detection & Knowledge Graph Ingestion",
f"""
<div class="card-slate" style="padding: 8px 12px; margin-bottom: 6px;">
  <p style="font-size: 7.8pt; line-height: 1.4;">
    The <strong>Market Intelligence Analyst</strong> coworker continuously monitors competitor digital footprints across 15+ domains. It crawls pricing tiers, captures structural DOM diffs, ingests corporate shifts into the persistent SQLite knowledge graph in 94 µs, and broadcasts real-time alerts to executive Slack channels.
  </p>
</div>

{mockup_embed("02_market_intelligence.png", "Figure 7.1: Real-Time Market Intelligence — DOM diff tracker detecting competitor tier pricing shifts")}

<div class="grid-3" style="margin-top: 6px;">
  <div class="card card-accent" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 7.8pt;">DOM Diff Engine</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Calculates SHA-256 tree deltas across competitor landing pages to identify subtle pricing drops and unannounced feature rollouts.
    </p>
  </div>
  <div class="card card-amber" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#d97706; font-size: 7.8pt;">94 µs Graph Absorption</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Ingests competitive insights into SQLite as structured entity nodes (<code>StripeX_Pricing_Q3</code>) for immediate multi-agent cross-referencing.
    </p>
  </div>
  <div class="card card-purple" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#7c3aed; font-size: 7.8pt;">Slack & PDF Briefings</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Dispatches immediate alerts to executive teams and compiles automated weekly competitive digest PDFs every Friday at 5:00 PM.
    </p>
  </div>
</div>
""")

# Page 08: MOCKUP 03 (Talent Scout)
add_page(8, "PART II: PERSONAS & SPECIALIZED COWORKERS",
"Executive Recruiting & Technical Talent Sourcing",
"Mining Open-Source Repositories, Commit AST Analysis & Technical Dossiers",
f"""
<div class="card-slate" style="padding: 8px 12px; margin-bottom: 6px;">
  <p style="font-size: 7.8pt; line-height: 1.4;">
    The <strong>Talent Scout</strong> coworker sources elite engineering candidates by inspecting open-source codebase commits. It mines repository commit histories (e.g. Tokio, Polars), evaluates abstract syntax tree (AST) complexity, corroborates social profiles, and constructs verified technical dossiers.
  </p>
</div>

{mockup_embed("03_talent_scout.png", "Figure 8.1: Executive Talent Scout — Mining GitHub AST repositories and constructing verified candidate dossiers")}

<div class="grid-3" style="margin-top: 6px;">
  <div class="card card-accent" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 7.8pt;">AST Commit Mining</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Analyzes real code contributions (e.g. SIMD AVX-512 in Tokio) to measure genuine technical depth rather than resume keywords.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#059669; font-size: 7.8pt;">Identity Corroboration</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Connects GitHub usernames, personal blogs, and LinkedIn profiles to locate deliverable direct corporate emails via SMTP verification.
    </p>
  </div>
  <div class="card card-purple" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#7c3aed; font-size: 7.8pt;">Hyper-Personalized Reach</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Drafts icebreakers referencing specific conference talks (EuroRust 2025) and specific merged PRs, achieving 60%+ response rates.
    </p>
  </div>
</div>
""")

# Page 09: MOCKUP 04 (Back-Office Invoice Reconciliation)
add_page(9, "PART II: PERSONAS & SPECIALIZED COWORKERS",
"Back-Office Operations & Financial Reconciliation",
"3-Way Invoice Matching, Automated ERP Entry & Discrepancy Resolution",
f"""
<div class="card-slate" style="padding: 8px 12px; margin-bottom: 6px;">
  <p style="font-size: 7.8pt; line-height: 1.4;">
    The <strong>Back-Office Assistant</strong> coworker ingests batch PDF invoices from vendor portals, performs automated 3-way matching against warehouse purchase orders and receiving logs, resolves line-item tax discrepancies, and stages batch payment entries in QuickBooks for CFO one-click approval.
  </p>
</div>

{mockup_embed("04_backoffice_invoice.png", "Figure 9.1: Back-Office Assistant — 3-way invoice reconciliation across $482,000 transaction volume in QuickBooks")}

<div class="grid-3" style="margin-top: 6px;">
  <div class="card card-accent" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 7.8pt;">Batch Document Ingestion</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Parses hundreds of PDF invoices ($482k volume) with 100% optical and tabular accuracy, extracting PO numbers, line items, and tax rates.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#059669; font-size: 7.8pt;">Automated 3-Way Match</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Cross-checks invoices against ERP purchase orders and receiving logs, autonomously resolving minor rounding discrepancies.
    </p>
  </div>
  <div class="card card-purple" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#7c3aed; font-size: 7.8pt;">QuickBooks Accessibility Sync</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Interacts with QuickBooks web portal using Accessibility Trees (<code>@e14</code>), saving 80+ hours of manual bookkeeping per month.
    </p>
  </div>
</div>
""")

# Page 10: MOCKUP 05 (DevOps Maintainer)
add_page(10, "PART II: PERSONAS & SPECIALIZED COWORKERS",
"DevOps Maintenance & Autonomous Code Engineering",
"Sentry Issue Triage, Repository AST Symbol Mapping & Verified PR Creation",
f"""
<div class="card-slate" style="padding: 8px 12px; margin-bottom: 6px;">
  <p style="font-size: 7.8pt; line-height: 1.4;">
    The <strong>DevOps Maintainer</strong> coworker autonomously triages production Sentry errors. It maps the repository symbol graph in 34 ms, reproduces the issue in a standalone test case, applies defensive code modifications, runs the complete test suite (23/23 passing), and submits a clean GitHub Pull Request.
  </p>
</div>

{mockup_embed("05_devops_engineer.png", "Figure 10.1: DevOps Maintainer — AST symbol search, zero-division error reproduction, and GitHub PR #142 creation")}

<div class="grid-3" style="margin-top: 6px;">
  <div class="card card-accent" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 7.8pt;">AST Symbol Indexing</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Indexes entire 240-file codebases in 34 ms, pinpointing exact function call sites and variable definitions without slow regex scans.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#059669; font-size: 7.8pt;">Automated Test Verification</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Executes pytest/cargo test suites in isolated sandboxes, ensuring zero regressions before committing code modifications.
    </p>
  </div>
  <div class="card card-purple" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#7c3aed; font-size: 7.8pt;">GitHub PR Automation</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Creates dedicated git branches (<code>fix/mom-zero-div</code>), pushes commits, and opens detailed Pull Requests ready for human sign-off.
    </p>
  </div>
</div>
""")

# Page 11
add_page(11, "PART II: PERSONAS & SPECIALIZED COWORKERS",
"Autonomous Research & Deep Intelligence Specialists",
"Multi-Step Academic, Regulatory & Competitive Synthesis Without Context Overflow",
"""
<div class="card-slate" style="padding: 10px 14px; margin-bottom: 10px;">
  <div class="card-title" style="font-size: 9.2pt;">Solving the Long-Horizon Deep Research Challenge</div>
  <p style="font-size: 8pt; line-height: 1.45;">
    Conducting deep technical, academic, or corporate intelligence research typically causes standard AI agents to suffer from <strong>context window saturation</strong>, hallucinations, and loss of focus over multi-hour investigations. Fathom resolves this through modular sub-agent delegation, disk-backed document spillage, and iterative recursive summarization.
  </p>
</div>

<div class="grid-2" style="margin-bottom: 10px;">
  <div class="card card-accent" style="padding: 9px 12px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 8.4pt;">Hermes Context Compaction</div>
    <p style="font-size: 7.6pt; line-height: 1.4;">
      Fathom monitors rolling token usage within each sub-agent conversation. When context exceeds 75% capacity, the runtime activates <strong>Hermes compaction</strong>: compressing earlier conversational steps into structured factual digests while maintaining active tool call signatures and environment variables intact.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 9px 12px;">
    <div class="card-title-sm" style="color:#059669; font-size: 8.4pt;">Disk-Backed Raw Payload Storage</div>
    <p style="font-size: 7.6pt; line-height: 1.4;">
      Large raw PDF filings, HTML dumps, and JSON payloads are never injected directly into LLM prompts. Instead, they are persisted to local SSD storage (<code>scratch/</code>) and referenced via lightweight handles, allowing workers to execute targeted grep/jq queries with zero token overhead.
    </p>
  </div>
</div>

<table class="table-deck" style="margin-bottom: 10px;">
  <thead>
    <tr>
      <th>Research Workflow Phase</th>
      <th>Engine Tool / Subsystem</th>
      <th>Key Performance Indicator (KPI)</th>
      <th>Outcome Deliverable</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td><strong>1. Discovery & Crawl</strong></td>
      <td><code>web_search</code> + <code>playwright</code></td>
      <td>100+ pages traversed in &lt; 8 sec</td>
      <td>Raw HTML/PDF documents staged to disk</td>
    </tr>
    <tr>
      <td><strong>2. Fact Extraction</strong></td>
      <td><code>memory_absorb</code> + regex filters</td>
      <td>94 µs per entity fact absorption</td>
      <td>Structured knowledge graph nodes created</td>
    </tr>
    <tr>
      <td><strong>3. Cross-Verification</strong></td>
      <td><code>llm_judge</code> + citation matrix</td>
      <td>100% claim-to-source traceability</td>
      <td>Zero hallucinated statistics or quotes</td>
    </tr>
    <tr>
      <td><strong>4. Executive Synthesis</strong></td>
      <td>Document compiler + PDF generator</td>
      <td>Full 15-page dossier in &lt; 30 sec</td>
      <td>Publication-ready whitepapers and briefs</td>
    </tr>
  </tbody>
</table>

<div class="card card-purple" style="padding: 8px 12px;">
  <div class="card-title-sm" style="color:#7c3aed; font-size: 8.2pt;">Academic & Patent Intelligence Mining</div>
  <p style="font-size: 7.4pt; color:#334155;">
    Fathom research workers connect directly to arXiv, Google Patents, and regulatory registries to analyze patent claims, citation graphs, and emerging technology breakthroughs autonomously—compiling comprehensive executive competitive assessments.
  </p>
</div>
""")

# Page 12: MOCKUP 11 (Swarm Coordinator)
add_page(12, "PART II: PERSONAS & SPECIALIZED COWORKERS",
"Self-Healing Background Jobs & Scheduled Routines",
"Atomic Schedule Claiming, Error Inspection & Hierarchical Swarm Orchestration",
f"""
<div class="card-slate" style="padding: 8px 12px; margin-bottom: 6px;">
  <p style="font-size: 7.8pt; line-height: 1.4;">
    Fathom coworkers execute long-running background jobs via atomic SQLite schedule claiming. If a worker encounters an error, the runtime automatically retries with previous failure inspection. The <strong>Swarm Coordinator</strong> orchestrates complex goals across parallel CPU worker pods in a Tokio <code>JoinSet</code>.
  </p>
</div>

{mockup_embed("11_swarm_coordinator.png", "Figure 12.1: Swarm Coordinator — Tokio JoinSet DAG execution across 4 parallel CPU worker pods with fair-share token budgets")}

<div class="grid-3" style="margin-top: 6px;">
  <div class="card card-accent" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#2563eb; font-size: 7.8pt;">Atomic Cron Claiming</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Workers claim schedules atomically via SQLite, guaranteeing exactly-once execution across clustered nodes without external Redis locks.
    </p>
  </div>
  <div class="card card-emerald" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#059669; font-size: 7.8pt;">Self-Healing Diagnostics</div>
    <p style="font-size: 7.1pt; color:#475569;">
      Failed background jobs pass previous error stack traces to the next retry prompt, allowing the agent to diagnose and fix the root cause.
    </p>
  </div>
  <div class="card card-purple" style="padding: 7px 9px;">
    <div class="card-title-sm" style="color:#7c3aed; font-size: 7.8pt;">Token Budget Allocation</div>
    <p style="font-size: 7.1pt; color:#475569;">
      The Swarm Coordinator allocates strict token budgets across worker pods, ensuring predictable inference throughput and zero runaway costs.
    </p>
  </div>
</div>
""")

print("Prepared Pages 01 through 12.")

# We will append the rest of the pages (13 to 42) in the script.
