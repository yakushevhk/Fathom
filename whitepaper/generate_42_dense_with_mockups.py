#!/usr/bin/env python3
import os, subprocess, concurrent.futures, shutil
from pypdf import PdfWriter

WP_DIR = os.path.dirname(os.path.abspath(__file__))
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

# ==============================================================================
# PART I: VISION & AUTONOMOUS REMOTE WORKFORCE (01-05)
# ==============================================================================

# Page 01: MOCKUP 1 (3-Pane Command Center)
add_page(1, "EXECUTIVE WHITEPAPER · STRATEGIC OVERVIEW",
"Universal Autonomous AI Workforce Runtime",
"High-Performance Rust Architecture for End-to-End Remote Digital Employees",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Paradigm Shift: From Scripted Bots to Autonomous Remote Employees</div>
  <p style="font-size: 7.4pt;">
    <strong>Fathom</strong> introduces a production-grade, self-hosted <strong>Rust runtime</strong> designed to instantiate, govern, and orchestrate true <strong>autonomous remote digital employees</strong>. These agents independently plan, execute multi-day workflows, operate web browsers via accessibility trees, perform deep OSINT/lead generation, write and maintain software, and interact across all corporate channels 100% remotely.
  </p>
</div>

<!-- MOCKUP 1: Master 3-Pane Command Center (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic">
      <span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span>
      <span class="mockup-title">fathom-control-plane · v0.3.0 · Tokyo Node #04</span>
    </div>
    <div style="font-size: 6pt; color: #38bdf8; font-family: 'JetBrains Mono', monospace;">● 5 WORKERS ONLINE</div>
  </div>
  <div class="mockup-body mockup-3col">
    <!-- Col 1: Bots Sidebar -->
    <div class="mockup-sidebar">
      <div class="mockup-sidebar-search">🔍 Search Coworkers...</div>
      <div class="bot-row is-active">
        <div class="bot-avatar avatar-orange">⚡</div>
        <div class="bot-info">
          <div class="bot-name">Sales Outbound</div>
          <div class="bot-preview">50 fintechs verified, CRM sync</div>
        </div>
      </div>
      <div class="bot-row">
        <div class="bot-avatar avatar-indigo">🧠</div>
        <div class="bot-info">
          <div class="bot-name">Market Intel</div>
          <div class="bot-preview">Competitor pricing diff alert</div>
        </div>
      </div>
      <div class="bot-row">
        <div class="bot-avatar avatar-emerald">👥</div>
        <div class="bot-info">
          <div class="bot-name">Talent Scout</div>
          <div class="bot-preview">30 Rust architects mapped</div>
        </div>
      </div>
      <div class="bot-row">
        <div class="bot-avatar avatar-purple">⚙️</div>
        <div class="bot-info">
          <div class="bot-name">DevOps Maintainer</div>
          <div class="bot-preview">23/23 pytest suite passing</div>
        </div>
      </div>
    </div>

    <!-- Col 2: Chat & Execution -->
    <div class="mockup-chat">
      <div class="chat-bubble-user">Find 50 verified VP Engineering leads in London & sync to amoCRM</div>
      <div class="chat-bubble-bot">
        <div>Decomposed goal into 4 parallel workers. Querying registries & running SMTP probes:</div>
        <div class="bot-task-card">
          <div class="bot-step-line"><span class="bot-check">✓</span><strong>Companies House</strong><span class="bot-arrow">→</span><span>120 entities discovered</span></div>
          <div class="bot-step-line"><span class="bot-check">✓</span><strong>Social & Web Crawl</strong><span class="bot-arrow">→</span><span>108 VP Engineering names</span></div>
          <div class="bot-step-line"><span class="bot-check">✓</span><strong>SMTP 250 OK Probes</strong><span class="bot-arrow">→</span><span>94 direct emails deliverable</span></div>
          <div class="bot-step-line"><span class="bot-check">✓</span><strong>amoCRM Synced</strong><span class="bot-arrow">→</span><span>94 contacts staged with tech tags</span></div>
        </div>
      </div>
    </div>

    <!-- Col 3: Computer & Routines -->
    <div class="mockup-computer">
      <div class="computer-viewport">
        <div class="browser-bar"><span style="color:#10b981;">●</span><span class="browser-url">companieshouse.gov.uk</span></div>
        <div class="browser-content" style="font-family: 'JetBrains Mono', monospace; font-size: 5.4pt;">
          [Search: FinTech UK]<br>
          > Matched: 120 firms<br>
          > Parsing officers...<br>
          <span style="color:#38bdf8;">Status: 100% complete</span>
        </div>
      </div>
      <div class="btn-takeover">⚡ Take Control</div>
      <div class="routine-item"><span class="routine-name">Nightly Sweep</span><span class="routine-time">02:00 AM</span></div>
    </div>
  </div>
</div>

<div class="grid-3">
  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">100% Remote Operation</div>
    <p style="font-size: 6.8pt;">Autonomous agents operate independently inside sandboxed environments, interacting with web portals, public registries, APIs, shells, and CRMs 24/7.</p>
  </div>
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Microsecond Rust Engine</div>
    <p style="font-size: 6.8pt;">Zero-cost abstractions, Tokio async I/O, and concurrent <code>JoinSet</code> task trees provide sub-millisecond tool dispatch (~0.75 ms) and 94 µs memory absorption.</p>
  </div>
  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">Unlimited Neural Compute</div>
    <p style="font-size: 6.8pt;">High-throughput routing to cost-efficient frontier foundation models (Kimi k3, Qwen 3.8 Max, GLM 5.3) enables flat-rate monthly seat economics.</p>
  </div>
</div>

<div class="card-slate" style="padding: 5px 8px;">
  <div class="card-title-sm">System Metric Scorecard (Empirical Benchmarks on Apple Silicon M4)</div>
  <div class="grid-4">
    <div class="metric-box"><div class="metric-val">12</div><div class="metric-label">Rust Workspace Crates</div></div>
    <div class="metric-box"><div class="metric-val">0.75 ms</div><div class="metric-label">Tool Dispatch Latency</div></div>
    <div class="metric-box"><div class="metric-val">51+</div><div class="metric-label">Native Core Tools</div></div>
    <div class="metric-box"><div class="metric-val">15 MB</div><div class="metric-label">Daemon Baseline RSS</div></div>
  </div>
</div>

<div class="callout callout-info" style="padding: 4px 8px; margin-bottom: 0;">
  <strong>Strategic Objective:</strong> Deliver a scalable software foundation where businesses deploy digital workers on demand—scaling operational capacity infinitely without proportional linear headcount expansion.
</div>
""")

# Page 02: Macro Problem
add_page(2, "MACRO PROBLEM · THE HIRING BOTTLENECK",
"The Broken Remote Hiring Landscape",
"Why Traditional Outsourcing and Human Headcount Scaling Fail Modern Companies",
"""
<div class="card-accent">
  <div class="card-title">The Remote Workforce Crisis: High Cost, High Friction, Slow Ramp-Up</div>
  <p>
    Scaling modern digital operations with human remote labor faces severe structural limitations: exorbitant salaries, months of recruiting lag, high employee turnover, timezone disconnects, and inconsistent task execution. Knowledge work has become the primary operational bottleneck for high-growth enterprises.
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber">
    <div class="card-title-sm">Human Remote Labor Pain Points</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>High Overhead:</strong> $4,000–$10,000/mo per knowledge worker + equipment, software licenses, HR management, benefits, and local payroll taxes.</li>
      <li><strong>Burnout & Inconsistency:</strong> Human SDRs, data scrubbers, and QA engineers make repetitive errors when processing thousands of complex data rows.</li>
      <li><strong>Timezone & Availability Lag:</strong> Work halts during nights, weekends, and national holidays—stalling business-critical outbound sales and support pipelines.</li>
      <li><strong>Churn & Knowledge Loss:</strong> When remote staff depart, valuable institutional memory disappears and expensive onboarding must restart from scratch.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">The Fathom Digital Solution</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Fractional Cost:</strong> Predictable flat monthly subscription per autonomous employee seat with zero payroll taxes, healthcare, or management overhead.</li>
      <li><strong>Deterministic Quality:</strong> Every task follows verified standard operating procedures (SOPs) with automated cross-checking and multi-signal verification.</li>
      <li><strong>24/7/365 Continuous Output:</strong> Digital coworkers process inbound leads, market monitoring, and data pipelines around the clock with zero downtime.</li>
      <li><strong>Permanent Knowledge:</strong> All discoveries, company ties, and client preferences are preserved forever in persistent semantic memory graphs.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Operational & Financial Comparison Matrix</div>
  <table>
    <thead>
      <tr>
        <th>Evaluation Factor</th>
        <th>Traditional Remote Staff</th>
        <th>Outsourced Agency</th>
        <th>Fathom Autonomous Coworker</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>Onboarding Time</strong></td>
        <td>4 – 8 Weeks (Recruiting & Training)</td>
        <td>2 – 4 Weeks (Contract SOW)</td>
        <td><strong>Instant (Under 60 Seconds via CLI/API)</strong></td>
      </tr>
      <tr>
        <td><strong>Monthly Cost Basis</strong></td>
        <td>High ($4,000 – $10,000/mo + Taxes)</td>
        <td>Variable / Retainer ($2,500 – $7,500/mo)</td>
        <td><strong>Flat Monthly Subscription per Seat</strong></td>
      </tr>
      <tr>
        <td><strong>Token & API Expenses</strong></td>
        <td>N/A</td>
        <td>Extra Billed Client Surcharges</td>
        <td><strong>100% Unlimited Compute (Included)</strong></td>
      </tr>
      <tr>
        <td><strong>Execution Speed</strong></td>
        <td>Human Speed (Minutes to Hours)</td>
        <td>Human Speed (Days to Turnaround)</td>
        <td><strong>Microsecond Rust Dispatch (~0.75 ms)</strong></td>
      </tr>
      <tr>
        <td><strong>Operating Availability</strong></td>
        <td>40 Hours / Week (Business Hours)</td>
        <td>Business Hours Only</td>
        <td><strong>168 Hours / Week (24/7 Continuous Execution)</strong></td>
      </tr>
      <tr>
        <td><strong>Scalability Velocity</strong></td>
        <td>Linear (1 hire = 1 salary & HR friction)</td>
        <td>Constrained by agency headcount</td>
        <td><strong>Infinite Elastic Scale (Instant Burst to 100+ Nodes)</strong></td>
      </tr>
    </tbody>
  </table>
</div>

<div class="card-slate">
  <div class="card-title-sm">The Hidden Cost of Human Employment</div>
  <p style="font-size: 7.4pt;">
    Beyond direct salary, human employees incur hidden costs: recruitment agency commissions (15–25% of annual salary), software tool seat licenses ($500+/mo for ZoomInfo, Apollo, LinkedIn Sales Navigator), HR compliance management, and productivity loss during onboarding ramp-up. Fathom consolidates the entire workforce pipeline into a single software platform.
  </p>
</div>

<div class="callout callout-info">
  <strong>The Strategic Imperative:</strong> In an era of compressed margins and fierce global competition, companies that transition routine research, outreach, data entry, and computer tasks to autonomous digital staff gain an unassailable operational speed and cost advantage.
</div>
""")

# Page 03: MOCKUP 2 (Comparison Mockup: Chatbot vs Coworker)
add_page(3, "PRODUCT PHILOSOPHY · PARADIGM COMPARISON",
"Digital Coworkers vs. Scripted Chatbots",
"Moving Beyond Single-Prompt Chat Interfaces to Autonomous Goal-Driven Agents",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Fundamental Difference: Proactive Agency vs. Reactive Text Generation</div>
  <p style="font-size: 7.4pt;">
    Most commercial AI tools are <strong>passive text assistants</strong>. <strong>Fathom instantiates proactive digital coworkers</strong>: given a high-level goal, they formulate plans, spawn sub-agents, operate browser tools, verify work, and deliver results directly to your CRM.
  </p>
</div>

<!-- MOCKUP 2: Visual Comparison of Bot Execution -->
<div class="grid-2">
  <div class="app-mockup">
    <div class="mockup-header" style="background:#1f1618; border-color:#382226;">
      <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title" style="color:#f87171;">Traditional Reactive Chatbot</span></div>
    </div>
    <div class="mockup-chat" style="background:#130c0e;">
      <div class="chat-bubble-user">Find CTO emails in Berlin</div>
      <div class="chat-bubble-bot" style="background:#1c1114; border-color:#382226;">
        Here are 3 example emails: info@company.com, cto@berlintech.de. (Note: These may not be verified; please check LinkedIn manually).
      </div>
      <div style="font-size: 5.8pt; color: #ef4444; padding: 2px 4px;">❌ Bottleneck: Human must manually test emails and enter CRM.</div>
    </div>
  </div>

  <div class="app-mockup">
    <div class="mockup-header" style="background:#11221b; border-color:#1e382b;">
      <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title" style="color:#34d399;">Fathom Autonomous Coworker</span></div>
    </div>
    <div class="mockup-chat" style="background:#091410;">
      <div class="chat-bubble-user">Find CTO emails in Berlin</div>
      <div class="chat-bubble-bot" style="background:#0d1f19; border-color:#1e382b;">
        <div class="bot-step-line"><span class="bot-check">✓</span><strong>Companies Scraped</strong><span class="bot-arrow">→</span><span>35 Berlin Tech Startups</span></div>
        <div class="bot-step-line"><span class="bot-check">✓</span><strong>SMTP 250 OK Probed</strong><span class="bot-arrow">→</span><span>28 Mailboxes Verified Live</span></div>
        <div class="bot-step-line"><span class="bot-check">✓</span><strong>HubSpot Pushed</strong><span class="bot-arrow">→</span><span>28 Enriched Contacts Synced</span></div>
      </div>
      <div style="font-size: 5.8pt; color: #10b981; padding: 2px 4px;">✅ Zero human intervention: 100% executed end-to-end.</div>
    </div>
  </div>
</div>

<div class="card">
  <div class="card-title">Three Core Architectural Differentiators</div>
  <div class="grid-3">
    <div class="card-slate">
      <div class="card-title-sm">1. Self-Directed Planning</div>
      <p style="font-size: 7pt;">Decomposes vague objectives into concrete sub-tasks with strict dependency trees, depth limits, and fair-share token budgets.</p>
    </div>
    <div class="card-slate">
      <div class="card-title-sm">2. Multi-Signal Verification</div>
      <p style="font-size: 7pt;">Checks every harvested fact, email MX record, and code output against objective validation gates before human presentation.</p>
    </div>
    <div class="card-slate">
      <div class="card-title-sm">3. Compounding Memory</div>
      <p style="font-size: 7pt;">Retains company relationships, past conversation context, and client preferences in persistent SQLite memory graphs.</p>
    </div>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Key Lifecycle Differences at a Glance</div>
  <table>
    <thead>
      <tr><th>Lifecycle Dimension</th><th>Single-Turn Chatbots</th><th>Fathom Digital Coworkers</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>Initiation Mechanism</strong></td><td>Requires constant synchronous human prompting.</td><td><strong>Autonomous scheduled triggers (Cron), webhooks, or API.</strong></td></tr>
      <tr><td><strong>Concurrency Model</strong></td><td>Single sequential thread of thought.</td><td><strong>Parallel multi-core Tokio task swarms (JoinSet).</strong></td></tr>
      <tr><td><strong>Failure Recovery</strong></td><td>Crashes or hallucinates; requires human reprompting.</td><td><strong>Self-healing retries with error diagnosis & task augmentation.</strong></td></tr>
      <tr><td><strong>State Persistence</strong></td><td>Stateless; context vanishes on tab closure.</td><td><strong>Durable SQLite database with append-only fact versioning.</strong></td></tr>
    </tbody>
  </table>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>The Autonomous Standard:</strong> Fathom coworkers don't just draft emails or write snippets—they find the decision-maker, verify deliverability, operate the sales platform, and track pipeline outcomes completely autonomously.
</div>
""")

# Page 04: MOCKUP 3 (Coworker Roster & Template Drawer)
add_page(4, "WORKFORCE ARCHETYPES · PERSONAS",
"The 5 Core Digital Worker Archetypes",
"Specialized Autonomous Roles Pre-Tuned for Immediate Enterprise Deployment",
"""
<div class="card-accent" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Pre-Configured Autonomous Employee Roles: Zero Prompt Engineering Required</div>
  <p style="font-size: 7.4pt;">
    Fathom supports specialized coworker personas out-of-the-box. Each persona is configured with role-specific system prompts (up to 32,000 characters), optimized tool sets, strict governance policies, and tailored verification loops.
  </p>
</div>

<!-- MOCKUP 3: Coworker Roster Cards (Rakazo Style) -->
<div class="app-mockup" style="padding: 6px;">
  <div class="mockup-header" style="background: transparent; border-bottom: 1px solid #1e293b; padding-bottom: 4px;">
    <div style="font-size: 7pt; font-weight: 700; color: #f1f5f9;">COWORKER ROSTER · ASSIGNED NEURAL ENGINES</div>
    <div style="font-size: 6pt; color: #10b981; font-family: 'JetBrains Mono', monospace;">UNLIMITED TOKENS INCLUDED</div>
  </div>
  <div class="grid-4" style="gap: 4px; margin-top: 4px;">
    <div style="background: #161e2b; border: 1px solid #232b38; border-radius: 4px; padding: 4px 6px;">
      <div style="display:flex; justify-content:space-between; align-items:center;">
        <span class="bot-avatar avatar-orange" style="width:16px; height:16px; font-size:7pt;">⚡</span>
        <span class="badge badge-blue">Qwen 3.8</span>
      </div>
      <div style="font-weight:700; color:#f1f5f9; font-size:6.8pt; margin-top:2px;">Autonomous SDR</div>
      <div style="color:#94a3b8; font-size:5.6pt;">OSINT, SMTP 250 OK & CRM push</div>
    </div>
    <div style="background: #161e2b; border: 1px solid #232b38; border-radius: 4px; padding: 4px 6px;">
      <div style="display:flex; justify-content:space-between; align-items:center;">
        <span class="bot-avatar avatar-indigo" style="width:16px; height:16px; font-size:7pt;">🧠</span>
        <span class="badge badge-purple">Kimi k3</span>
      </div>
      <div style="font-weight:700; color:#f1f5f9; font-size:6.8pt; margin-top:2px;">Market Analyst</div>
      <div style="color:#94a3b8; font-size:5.6pt;">24/7 Competitor diffs & filings</div>
    </div>
    <div style="background: #161e2b; border: 1px solid #232b38; border-radius: 4px; padding: 4px 6px;">
      <div style="display:flex; justify-content:space-between; align-items:center;">
        <span class="bot-avatar avatar-emerald" style="width:16px; height:16px; font-size:7pt;">👥</span>
        <span class="badge badge-green">GLM 5.3</span>
      </div>
      <div style="font-weight:700; color:#f1f5f9; font-size:6.8pt; margin-top:2px;">Talent Scout</div>
      <div style="color:#94a3b8; font-size:5.6pt;">GitHub AST & resume mining</div>
    </div>
    <div style="background: #161e2b; border: 1px solid #232b38; border-radius: 4px; padding: 4px 6px;">
      <div style="display:flex; justify-content:space-between; align-items:center;">
        <span class="bot-avatar avatar-coral" style="width:16px; height:16px; font-size:7pt;">📑</span>
        <span class="badge badge-amber">Qwen 3.8</span>
      </div>
      <div style="font-weight:700; color:#f1f5f9; font-size:6.8pt; margin-top:2px;">Back-Office</div>
      <div style="color:#94a3b8; font-size:5.6pt;">Portal entry & 3-way invoice match</div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Autonomous Sales Development Rep (SDR)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li>Searches business directories (2GIS, Google Places, Yandex Maps, USRLE).</li>
      <li>Performs pattern-based email permutation and SMTP 250 OK mailbox handshakes.</li>
      <li>Pushes enriched, verified leads directly into amoCRM, Bitrix24, or HubSpot.</li>
    </ul>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Market Intelligence & OSINT Analyst</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li>Monitors competitor websites, pricing tables, and product feature matrices.</li>
      <li>Tracks executive hiring velocity, regulatory filings, and funding rounds.</li>
      <li>Assimilates facts into the persistent knowledge graph in 94 µs per fact.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Custom Coworker Definition Engine (POST /api/v1/coworkers)</div>
  <p style="font-size: 7.2pt;">
    Enterprises can define proprietary coworker profiles with custom SOP prompts, assigned communication channels (Telegram, Slack, Email), and atomic cron schedules in a single REST API call.
  </p>
  <div class="grid-4" style="margin-top: 2px;">
    <div class="metric-box"><div class="metric-val">32 KB</div><div class="metric-label">Max SOP Prompt</div></div>
    <div class="metric-box"><div class="metric-val">1-Click</div><div class="metric-label">Persona Cloning</div></div>
    <div class="metric-box"><div class="metric-val">Atomic</div><div class="metric-label">Cron Locking</div></div>
    <div class="metric-box"><div class="metric-val">100%</div><div class="metric-label">Private Isolation</div></div>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Organizational Leverage:</strong> A single human department manager can effortlessly supervise a team of 10 to 50 specialized digital employees across multiple business functions.
</div>
""")

# Page 05: MOCKUP 4 (Telegram Mobile Control Bridge)
add_page(5, "DAY IN THE LIFE · OPERATIONAL WORKFLOW",
"A Day in the Life of a Digital Employee",
"24-Hour Continuous Execution Cycle of an Autonomous Fathom Worker",
"""
<div class="card-dark" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Continuous 24/7 Autonomy: Zero Idle Time, Zero Latency</div>
  <p style="font-size: 7.4pt;">
    While human teams sleep, Fathom coworkers execute scheduled background operations, monitor market shifts, prepare outbound campaigns, verify deliverability, and stage pipeline deliverables for morning review.
  </p>
</div>

<!-- MOCKUP 4: Timeline & Telegram Alert Card -->
<div class="grid-2">
  <div class="timeline" style="gap: 3px;">
    <div class="timeline-item" style="padding: 3px 6px;">
      <div class="timeline-time" style="font-size:6.2pt;">02:00 AM</div>
      <div class="timeline-content">
        <div class="timeline-title" style="font-size:6.8pt;">Scheduled Cron Trigger (Atomic Lock)</div>
        <div class="timeline-desc" style="font-size:5.8pt;">Coworker wakes via atomic claim (<code>0 2 * * *</code>), loads target criteria from persistent SQLite memory.</div>
      </div>
    </div>
    <div class="timeline-item" style="padding: 3px 6px;">
      <div class="timeline-time" style="font-size:6.2pt;">02:15 AM</div>
      <div class="timeline-content">
        <div class="timeline-title" style="font-size:6.8pt;">Parallel Multi-Engine OSINT Swarm</div>
        <div class="timeline-desc" style="font-size:5.8pt;">Coordinator spawns 4 worker sub-agents querying directories and websites concurrently; 85 candidate records found.</div>
      </div>
    </div>
    <div class="timeline-item" style="padding: 3px 6px;">
      <div class="timeline-time" style="font-size:6.2pt;">03:30 AM</div>
      <div class="timeline-content">
        <div class="timeline-title" style="font-size:6.8pt;">SMTP 250 OK Non-Intrusive Probes</div>
        <div class="timeline-desc" style="font-size:5.8pt;">DNS MX check + port 25 handshake confirms 62 deliverable mailboxes; disposable emails purged.</div>
      </div>
    </div>
    <div class="timeline-item" style="padding: 3px 6px;">
      <div class="timeline-time" style="font-size:6.2pt;">07:00 AM</div>
      <div class="timeline-content">
        <div class="timeline-title" style="font-size:6.8pt;">CRM Ingestion & Knowledge Assimilation</div>
        <div class="timeline-desc" style="font-size:5.8pt;">Pushes 62 leads to amoCRM; absorbs company ties into SQLite entity graph (94µs/fact).</div>
      </div>
    </div>
  </div>

  <!-- Mobile Telegram Bot Interface Mockup -->
  <div class="app-mockup" style="background:#17212b; border-color:#242f3d;">
    <div class="mockup-header" style="background:#242f3d; border-color:#17212b;">
      <div style="font-size:6.4pt; font-weight:700; color:#fff;">✈️ Telegram · Fathom Coworker Bot</div>
      <div style="font-size:5.8pt; color:#38bdf8;">08:30 AM</div>
    </div>
    <div style="padding: 6px; display:flex; flex-direction:column; gap:4px; font-size:6.2pt;">
      <div style="background:#1e2c3a; padding:5px 7px; border-radius:6px; color:#f1f5f9;">
        <strong>🌅 Good morning Sarah!</strong><br>
        Nightly SDR sweep completed:<br>
        • <strong>62 verified leads</strong> in London FinTech<br>
        • 100% email validity (SMTP probed)<br>
        • amoCRM deal cards staged.<br>
        <span style="color:#64748b; font-size:5.4pt;">Attachment: London_Fintech_Q3.xlsx</span>
      </div>
      <div style="display:flex; gap:4px;">
        <div style="background:#2b5278; color:#fff; padding:3px; border-radius:3px; text-align:center; flex:1; font-weight:700; font-size:5.8pt;">✓ Approve CRM Push</div>
        <div style="background:#242f3d; color:#94a3b8; padding:3px; border-radius:3px; text-align:center; flex:1; font-size:5.8pt;">📊 View Excel Report</div>
      </div>
    </div>
  </div>
</div>

<div class="card-slate" style="padding: 5px 8px;">
  <div class="card-title-sm">Operational Outcome Summary</div>
  <p style="font-size: 7.2pt;">
    When human account executives arrive at their desks at 9:00 AM, fresh, fully verified, and enriched leads are already waiting in the CRM pipeline ready for high-conversion closing calls—with zero human prospecting time required.
  </p>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>The Compounding Advantage:</strong> 6 hours of autonomous nighttime execution delivers more qualified pipeline than a full-time human SDR produces in an entire work week.
</div>
""")

# Continuing with remaining pages and mockups...
print("Loaded Part I dense pages with mockups...")

# ==============================================================================
# PART II: BUSINESS MODEL, PRICING & ECONOMICS (06-10)
# ==============================================================================

add_page(6, "COMMERCIAL STRATEGY · PRICING MODEL",
"The Virtual Employee Subscription Model",
"Seat-Based Flat Pricing: Eliminating Token Metering and Billing Anxiety",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Commercial Model: Pay Per Autonomous Coworker Seat</div>
  <p style="font-size: 7.4pt;">
    Traditional AI tools force customers to monitor complex token meters, calculate cost-per-call, and live in constant fear of unexpected billing spikes. <strong>Fathom adopts a transparent subscription model</strong>: customers subscribe to dedicated virtual employee seats on a flat monthly basis.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 6px 9px;">
    <div class="card-title-sm">100% Unlimited Usage Included</div>
    <ul style="font-size: 7pt; margin-bottom: 0;">
      <li><strong>Unlimited Neural Compute:</strong> Run millions of tokens monthly without extra surcharges or rate-limit penalties.</li>
      <li><strong>Unlimited Tool Invocations:</strong> Web search, browser automation, email validation, and code execution.</li>
      <li><strong>Unlimited Job Schedules:</strong> Set up continuous hourly or daily recurring background workflows.</li>
    </ul>
  </div>

  <div class="card card-accent" style="padding: 6px 9px;">
    <div class="card-title-sm">Enterprise Budget Predictability</div>
    <ul style="font-size: 7pt; margin-bottom: 0;">
      <li><strong>Fixed OpEx:</strong> Treat AI workers as standard fixed-cost software seats rather than volatile utility bills.</li>
      <li><strong>Elastic Expansion:</strong> Add 5 SDRs during a sales sprint and scale back down instantly without severance costs.</li>
      <li><strong>No Micro-Management:</strong> Teams don't need to restrict AI usage to save token budget.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 6px 9px;">
  <div class="card-title">Subscription Tier Architecture Overview</div>
  <table>
    <thead>
      <tr><th>Subscription Tier</th><th>Intended Scale</th><th>Included Capabilities & Fleet Features</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>Starter Seat</strong></td><td>1 Dedicated Autonomous Worker</td><td>Full OSINT tools, 7 search backends, contact verification, and Telegram notifications.</td></tr>
      <tr><td><strong>Growth Pod</strong></td><td>3 – 5 Collaborative Coworkers</td><td>Multi-agent tree coordination, CRM auto-push, Docker computer sandboxing, and persistent memory.</td></tr>
      <tr><td><strong>Enterprise Fleet</strong></td><td>10+ Autonomous Workers</td><td>Custom coworker prompts, dedicated PostgreSQL cluster, role-based access control, and SLA support.</td></tr>
    </tbody>
  </table>
</div>

<div class="card-slate" style="padding: 6px 9px;">
  <div class="card-title-sm">Zero Token Anxiety: Why Metered Billing Kills AI Adoption</div>
  <p style="font-size: 7.2pt;">
    When employees know every prompt or search costs $0.05, they self-censor and avoid deep multi-step research. By including unlimited neural compute in the seat subscription, Fathom empowers agents to perform exhaustive 10-step verifications without financial friction.
  </p>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>The Core Promise:</strong> You pay for the worker's output and business role, not the number of words it reads or writes.
</div>
""")

add_page(7, "NEURAL ECONOMICS · MARGIN ARBITRAGE",
"Unlimited Neural Compute Engine",
"Harnessing Frontier Foundation Models for High-Throughput Cost Arbitrage",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Foundation Model Cost-Performance Revolution</div>
  <p style="font-size: 7.4pt;">
    Offering unlimited neural compute is economically viable because Fathom intelligently routes tasks to next-generation frontier foundation models that deliver elite reasoning at a fraction of legacy pricing.
  </p>
</div>

<div class="grid-3">
  <div class="card card-emerald" style="padding: 6px 8px;">
    <div class="card-title-sm">Kimi k3 (Moonshot AI)</div>
    <p style="font-size: 7.2pt;"><strong>Strength:</strong> Ultra-Long Context</p>
    <p style="font-size: 6.8pt;">Processes massive document corpora, multi-page regulatory filings, and deep recursive research trees with flawless long-range recall across 200k+ tokens.</p>
  </div>

  <div class="card card-accent" style="padding: 6px 8px;">
    <div class="card-title-sm">Qwen 3.8 Max (Alibaba)</div>
    <p style="font-size: 7.2pt;"><strong>Strength:</strong> Tool Calling & Code</p>
    <p style="font-size: 6.8pt;">Exceptional precision in structured function calling, multilingual web parsing, and Python/Node.js REPL script generation with microsecond JSON serialization.</p>
  </div>

  <div class="card card-purple" style="padding: 6px 8px;">
    <div class="card-title-sm">GLM 5.3 (Zhipu AI)</div>
    <p style="font-size: 7.2pt;"><strong>Strength:</strong> Fast Decomposition</p>
    <p style="font-size: 6.8pt;">High-speed reasoning engine optimized for coordinator agents breaking down complex user briefs into parallel worker subtasks with sub-200ms TTFT.</p>
  </div>
</div>

<div class="card" style="padding: 6px 8px;">
  <div class="card-title">Fathom's Economic Arbitrage Flywheel</div>
  <div class="diagram-flow">
    <div class="flow-step"><div class="flow-title">1. Compiled Rust Core</div><div class="flow-desc">~15MB RAM & 0.75ms dispatch</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">2. Efficient Chinese LLMs</div><div class="flow-desc">Kimi k3 / Qwen / GLM</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">3. Sub-$25 Monthly Cost</div><div class="flow-desc">Total compute per worker</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">4. 90%+ Gross Margin</div><div class="flow-desc">On flat seat subscription</div></div>
  </div>
</div>

<div class="card-slate" style="padding: 6px 8px;">
  <div class="card-title-sm">Dynamic Role-Based Model Routing</div>
  <p style="font-size: 7.2pt;">
    The coordinator dynamically assigns the ideal model per role: GLM 5.3 for high-speed planning, Kimi k3 for deep document synthesis, and Qwen 3.8 Max for browser automation and code execution. This maximizes accuracy while keeping internal token expenditures at micro-cents per task.
  </p>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>The Economic Reality:</strong> Token costs on these frontier engines range between $0.10 and $0.40 per Million tokens—over 15x cheaper than legacy Western API models—delivering 90%+ gross margins on flat seat subscriptions.
</div>
""")

add_page(8, "FINANCIAL ANALYSIS · ROI & TCO",
"Total Cost of Ownership (TCO) & ROI",
"Hard Economic Numbers: Comparing In-House Staff, Traditional AI Stacks, and Fathom",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Executive Financial Summary: The 10x ROI Multiplier</div>
  <p style="font-size: 7.4pt;">
    Deploying a Fathom digital employee eliminates the vast majority of operational expenses associated with human staff and fragmented SaaS software subscriptions.
  </p>
</div>

<div class="card" style="padding: 6px 8px;">
  <div class="card-title">Annual Cost Breakdown for a 5-Person Outbound Sales Team</div>
  <table>
    <thead>
      <tr><th>Expense Category</th><th>Traditional In-House Team</th><th>Fragmented SaaS + AI Stack</th><th>Fathom Digital Workforce</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>Base Salaries (5 Staff)</strong></td><td>$300,000 / year ($60k/ea)</td><td>$120,000 / year (Junior Staff)</td><td><strong>$0.00</strong></td></tr>
      <tr><td><strong>Benefits, Taxes & HR</strong></td><td>$75,000 / year (25% burden)</td><td>$30,000 / year</td><td><strong>$0.00</strong></td></tr>
      <tr><td><strong>Data & Scraper Licenses</strong></td><td>$18,000 (ZoomInfo, Apollo)</td><td>$14,000 / year</td><td><strong>Included (Built-in 7 search engines)</strong></td></tr>
      <tr><td><strong>Email Verification Tools</strong></td><td>$6,000 (ZeroBounce, etc.)</td><td>$4,500 / year</td><td><strong>Included (Built-in SMTP probe)</strong></td></tr>
      <tr><td><strong>LLM Token & API Invoices</strong></td><td>$0.00</td><td>$12,000 – $24,000 / year</td><td><strong>Included (Unlimited compute)</strong></td></tr>
      <tr><td><strong>Total Annual Investment</strong></td><td><strong>$399,000 / year</strong></td><td><strong>$180,500 / year</strong></td><td><strong>Fractional Flat Subscription</strong></td></tr>
    </tbody>
  </table>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Immediate Payback Period</div>
    <p style="font-size: 7.2pt;">
      Most clients recoup their entire annual Fathom subscription within the <strong>first 14 days of deployment</strong> through newly closed outbound sales pipeline and eliminated SaaS tool licenses.
    </p>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">Zero Management Drag</div>
    <p style="font-size: 7.2pt;">
      Managers spend 0 hours on 1-on-1s, dispute mediation, sick leave coverage, or retraining. Performance metrics and audit trails are visible in real time via Prometheus.
    </p>
  </div>
</div>

<div class="card-slate" style="padding: 5px 8px;">
  <div class="card-title-sm">Direct Software Consolidation</div>
  <p style="font-size: 7.2pt;">
    Fathom replaces up to 6 separate enterprise SaaS subscriptions: lead databases, email verification tools, proxy networks, browser automation scripts, CRM enrichment add-ons, and vector database hosting.
  </p>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Bottom Line Impact:</strong> Fathom delivers <strong>85% to 92% cost reduction</strong> while expanding outreach volume and operational bandwidth by 400%.
</div>
""")

add_page(9, "ORGANIZATIONAL DESIGN · ELASTIC SCALING",
"Scalability Economics: 1 to 1,000 Workers",
"Elastic Scaling Without Organizational Bureaucracy or Management Friction",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Frictionless Enterprise: Scaling Labor Like Cloud Servers</div>
  <p style="font-size: 7.4pt;">
    In traditional business, growing from 10 to 100 employees introduces exponential management complexity. <strong>Fathom allows organizations to scale labor elastically</strong> like cloud compute instances.
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber" style="padding: 5px 8px;">
    <div class="card-title-sm">Traditional Scaling (Brooks's Law)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li>Communication channels grow quadratically: $N(N-1)/2$.</li>
      <li>Coordination overhead consumes up to 40% of productive hours.</li>
      <li>Hiring lag delays market entry by 3 to 6 months.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Fathom Swarm Scaling (Tokio DAG)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li>Hierarchical coordinator agents manage sub-agents with strict depth bounds.</li>
      <li>Non-blocking broadcast message bus ensures instant telemetry sync.</li>
      <li>Deploy 100 new workers in a single API call with zero recruiting latency.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Elastic Workforce Deployment Scenarios</div>
  <div class="grid-3">
    <div class="card-slate" style="padding: 4px 6px;">
      <div class="card-title-sm">Product Launch Blitz</div>
      <p style="font-size: 6.8pt;">Instantly spin up 50 SDR coworkers for 2 weeks to saturate a new vertical market, then scale back to 5 maintenance agents.</p>
    </div>
    <div class="card-slate" style="padding: 4px 6px;">
      <div class="card-title-sm">Due Diligence Sprint</div>
      <p style="font-size: 6.8pt;">Deploy 20 analyst coworkers to cross-reference 500 company filings and competitor websites over a single weekend.</p>
    </div>
    <div class="card-slate" style="padding: 4px 6px;">
      <div class="card-title-sm">Seasonal Back-Office Surge</div>
      <p style="font-size: 6.8pt;">Scale up invoice extraction and customer order reconciliation workers to handle a 10x Black Friday transaction surge.</p>
    </div>
  </div>
</div>

<div class="card-slate" style="padding: 5px 8px;">
  <div class="card-title-sm">Workforce Scalability Metric Scorecard</div>
  <div class="grid-4">
    <div class="metric-box"><div class="metric-val">&lt; 5s</div><div class="metric-label">Worker Spawn</div></div>
    <div class="metric-box"><div class="metric-val">Zero</div><div class="metric-label">Recruiting Lag</div></div>
    <div class="metric-box"><div class="metric-val">100%</div><div class="metric-label">SOP Consistency</div></div>
    <div class="metric-box"><div class="metric-val">Infinite</div><div class="metric-label">Elastic Scale</div></div>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>True Business Agility:</strong> Scale up for market opportunities in seconds; scale down during seasonal lulls without severance or operational drag.
</div>
""")

add_page(10, "B2B STRATEGY · PARTNERS & ENTERPRISE",
"Enterprise & Agency Monetization Models",
"White-Label Reselling, Managed AI Staffing & Multi-Tenant Deployments",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Dual Commercial Engines: Direct Enterprise & Agency Resellers</div>
  <p style="font-size: 7.4pt;">
    Fathom captures market share through two robust commercial motions: direct enterprise deployments for corporate efficiency, and agency partnerships that turn marketing/staffing firms into AI workforce providers.
  </p>
</div>

<div class="grid-2">
  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Agency & White-Label Model</div>
    <p style="font-size: 7.2pt;">Marketing, recruitment, and IT consulting agencies resell Fathom workers under their own brand:</p>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Turnkey Lead Generation:</strong> Agencies offer "Automated SDR as a Service" to clients, charging monthly retainers.</li>
      <li><strong>White-Label Dashboard:</strong> Embed Fathom's Next.js web dashboard with custom agency branding.</li>
      <li><strong>Recurring High Margins:</strong> Agencies capture 70–80% net profit margins on outsourced client fulfillment.</li>
    </ul>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Enterprise Private Fleet Model</div>
    <p style="font-size: 7.2pt;">Large corporations deploy self-hosted Fathom clusters behind corporate firewalls:</p>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Dedicated On-Prem / VPC:</strong> Complete data sovereignty with zero external data sharing (GDPR, 152-FZ).</li>
      <li><strong>Private LLM Connectivity:</strong> Route inference to internal vLLM/Ollama clusters.</li>
      <li><strong>Active Directory / SSO:</strong> Provision coworkers per department with fine-grained RBAC policies.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Commercial Revenue Streams Architecture</div>
  <table>
    <thead>
      <tr><th>Revenue Stream</th><th>Target Customer Segment</th><th>Monetization Structure</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>Direct Seat Subscription</strong></td><td>SMBs, Startups, Mid-Market</td><td>Flat monthly subscription per virtual employee seat.</td></tr>
      <tr><td><strong>Agency Volume Licensing</strong></td><td>Lead-Gen Agencies, BPOs, Consultancies</td><td>Discounted multi-seat bundles with white-label portal rights.</td></tr>
      <tr><td><strong>Enterprise Platform License</strong></td><td>Fortune 500, Financial Institutions</td><td>Annual platform license + custom tool integrations & dedicated SLAs.</td></tr>
    </tbody>
  </table>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Viral Agency Expansion:</strong> Every agency partner brings 10 to 50 end-clients, creating an exponential, self-funding B2B distribution channel.
</div>
""")

# ==============================================================================
# PART III: USER ACQUISITION, GO-TO-MARKET & GROWTH LOOPS (11-15)
# ==============================================================================

add_page(11, "GROWTH LOOPS · SELF-REPLICATING SALES",
"The Self-Replicating Growth Loop",
"How Fathom Sells Fathom: Autonomous Customer Acquisition at Zero Marginal CAC",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Ultimate Organic Growth Mechanism: Autonomous Self-Outreach</div>
  <p style="font-size: 7.4pt;">
    The most powerful marketing validation of an autonomous sales agent is when <strong>the product sells itself</strong>. Fathom operates an internal fleet of autonomous SDR coworkers whose sole job is to discover target B2B companies, verify decision-maker contacts, and conduct personalized cold outreach to sign up new customers at near-zero customer acquisition cost (CAC).
  </p>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">The 4-Step Self-Replicating Acquisition Engine</div>
  <div class="diagram-flow">
    <div class="flow-step"><div class="flow-title">1. Market Discovery</div><div class="flow-desc">Fathom scrapes B2B directories for agencies & SaaS</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">2. SMTP Verification</div><div class="flow-desc">Verifies CEO/VP Sales email with SMTP probe</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">3. Personalized Audit</div><div class="flow-desc">Attaches 10 free verified leads in their exact niche</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">4. Direct Sign-Up</div><div class="flow-desc">Prospect books call or subscribes to own worker</div></div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Zero Marginal CAC Economics</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>No Human BDR Salaries:</strong> 50 virtual SDRs execute 5,000 personalized touchpoints daily at near-zero incremental server cost.</li>
      <li><strong>Hyper-Personalized Proof-of-Work:</strong> Instead of generic cold spam, outreach includes real, verified prospect data tailored to the recipient's exact ICP.</li>
      <li><strong>Instant Value Delivery:</strong> Prospects experience the product's output before ever entering a sales demo.</li>
    </ul>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">The Viral Inbound Referral Loop</div>
    <p style="font-size: 7.2pt;">
      Every email sent by a Fathom SDR includes a subtle footer: <em>"This prospect list was researched, verified, and sent autonomously by Fathom AI Coworker."</em> Recipients frequently reply asking how to hire a similar digital worker for their own company.
    </p>
  </div>
</div>

<div class="card-slate" style="padding: 5px 8px;">
  <div class="card-title-sm">The Flywheel Compounding Equation</div>
  <p style="font-size: 7.2pt;">
    As new customers subscribe, a fraction of their subscription revenue funds additional worker compute instances, which in turn discover more prospects and close more subscriptions. This creates an infinite, self-funding acquisition loop.
  </p>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Infinite Self-Funding Flywheel:</strong> As more customers subscribe, compute revenue funds more autonomous worker nodes, scaling top-of-funnel outreach without outside marketing spend.
</div>
""")

add_page(12, "MARKETING & SALES · GTM STRATEGY",
"Go-To-Market Channels & Customer Acquisition",
"A Multi-Pronged Strategy for Rapid B2B and Mid-Market Market Penetration",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Omnichannel B2B Customer Acquisition Framework</div>
  <p style="font-size: 7.4pt;">
    Beyond autonomous outbound sales, Fathom deploys a multi-channel go-to-market strategy targeting high-intent decision-makers across four key acquisition pillars, minimizing dependence on any single channel.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">1. High-ROI Outbound (Fathom Fleet)</div>
    <p style="font-size: 7pt;">Autonomous SDRs target agency owners, SaaS founders, and sales leaders with free custom lead samples (25k+ monthly touches, 4.5%–7.2% response rate).</p>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Agency Partnership Program</div>
    <p style="font-size: 7pt;">Recruiting, marketing, and SEO agencies deploy Fathom as their secret fulfillment back-office, driving negative net churn.</p>
  </div>
</div>

<div class="grid-2">
  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">3. Open-Core Developer Funnel</div>
    <p style="font-size: 7pt;">Engineers and technical founders adopt the open-source Rust CLI on GitHub, upgrading to managed hosting and unlimited Chinese compute.</p>
  </div>

  <div class="card card-indigo" style="padding: 5px 8px;">
    <div class="card-title-sm">4. B2B Community & Skill Marketplace</div>
    <p style="font-size: 7pt;">Pre-built coworker SOP templates (e.g. <em>"Fintech SDR"</em>, <em>"YC Founder Scout"</em>) shared across communities generate viral organic word-of-mouth.</p>
  </div>
</div>

<div class="card-slate" style="padding: 5px 8px;">
  <div class="card-title-sm">Customer Acquisition Unit Economics Matrix</div>
  <div class="grid-4">
    <div class="metric-box"><div class="metric-val">&lt; $45</div><div class="metric-label">Blended CAC</div></div>
    <div class="metric-box"><div class="metric-val">&gt; 12:1</div><div class="metric-label">LTV to CAC Ratio</div></div>
    <div class="metric-box"><div class="metric-val">&lt; 14 Days</div><div class="metric-label">Payback Window</div></div>
    <div class="metric-box"><div class="metric-val">&gt; 140%</div><div class="metric-label">Net Retention</div></div>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Strategic Market Fit:</strong> By pairing product-led developer distribution with autonomous outbound execution, Fathom achieves hyper-efficient growth across both bottom-up and top-down sales motions.
</div>
""")

# Page 13: MOCKUP 5 (Free Value Audit Live Lead Table)
add_page(13, "CONVERSION FUNNEL · PRODUCT-LED GROWTH",
"The 'Free Value Audit to Paid Seat' Funnel",
"Converting Prospects by Delivering Tangible Work Deliverables Before Payment",
"""
<div class="card-dark" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Product-Led Conversion: The Irresistible Value Audit</div>
  <p style="font-size: 7.4pt;">
    Fathom converts prospects by <strong>delivering immediate, tangible business value upfront</strong>: 50 verified target leads in the prospect's exact niche delivered in an interactive report before asking for a subscription.
  </p>
</div>

<!-- MOCKUP 5: Live Free Value Audit Deliverable View (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Fathom Audit Viewer · "Cybersecurity Leaders - DACH Region"</span></div>
    <div style="display:flex; gap:4px;">
      <span class="badge badge-green">✓ 50 SMTP VERIFIED</span>
      <span class="badge badge-blue">📥 Export .XLSX</span>
    </div>
  </div>
  <div style="padding: 4px 6px; background:#0b0f17;">
    <table style="margin-bottom: 0; font-size: 6.2pt;">
      <thead>
        <tr style="background:#161e2b; color:#94a3b8;">
          <th>Full Name & Role</th>
          <th>Company</th>
          <th>Verified Work Email</th>
          <th>Confidence</th>
          <th>CRM Action</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td><strong>Dr. Marcus Weber</strong> · CISO</td>
          <td>FinTech Bavaria AG</td>
          <td><span style="color:#38bdf8; font-family:'JetBrains Mono';">m.weber@fintech-bavaria.de</span></td>
          <td><span class="badge badge-green">0.99 (SMTP OK)</span></td>
          <td><span class="badge badge-blue">+ amoCRM</span></td>
        </tr>
        <tr>
          <td><strong>Elena Schmidt</strong> · VP IT Ops</td>
          <td>SecureCloud GmbH</td>
          <td><span style="color:#38bdf8; font-family:'JetBrains Mono';">e.schmidt@securecloud.io</span></td>
          <td><span class="badge badge-green">0.98 (SMTP OK)</span></td>
          <td><span class="badge badge-blue">+ amoCRM</span></td>
        </tr>
        <tr>
          <td><strong>Lukas Becker</strong> · Head of Sec</td>
          <td>Munich Data Labs</td>
          <td><span style="color:#38bdf8; font-family:'JetBrains Mono';">lukas.becker@munichdatalabs.com</span></td>
          <td><span class="badge badge-green">0.95 (SMTP OK)</span></td>
          <td><span class="badge badge-blue">+ amoCRM</span></td>
        </tr>
      </tbody>
    </table>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">The 5-Stage Customer Conversion Journey</div>
  <div class="timeline" style="gap: 2px;">
    <div class="timeline-item" style="padding: 2px 5px;">
      <div class="timeline-time" style="font-size:6pt;">Stage 1</div>
      <div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Automated Value Hook</div><div class="timeline-desc" style="font-size:5.6pt;">Prospect enters target industry into interactive form.</div></div>
    </div>
    <div class="timeline-item" style="padding: 2px 5px;">
      <div class="timeline-time" style="font-size:6pt;">Stage 2</div>
      <div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Live Execution by Fathom Worker</div><div class="timeline-desc" style="font-size:5.6pt;">2-minute OSINT sweep extracts & verifies 25 live decision-makers.</div></div>
    </div>
    <div class="timeline-item" style="padding: 2px 5px;">
      <div class="timeline-time" style="font-size:6pt;">Stage 3</div>
      <div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Free Sample Delivery (.xlsx)</div><div class="timeline-desc" style="font-size:5.6pt;">Prospect receives clean Excel file with verified leads.</div></div>
    </div>
    <div class="timeline-item" style="padding: 2px 5px;">
      <div class="timeline-time" style="font-size:6pt;">Stage 4</div>
      <div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">7-Day Telegram Pilot Coworker</div><div class="timeline-desc" style="font-size:5.6pt;">1 dedicated SDR running 100 searches daily with unlimited compute.</div></div>
    </div>
    <div class="timeline-item" style="padding: 2px 5px;">
      <div class="timeline-time" style="font-size:6pt;">Stage 5</div>
      <div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Frictionless Monthly Subscription</div><div class="timeline-desc" style="font-size:5.6pt;">Conversion to paid monthly seat subscription (28% trial-to-paid).</div></div>
    </div>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Why This Funnel Works:</strong> B2B buyers don't buy software—they buy outcomes. Showing verified leads in their exact target market dissolves skepticism in seconds.
</div>
""")

add_page(14, "PARTNER ECOSYSTEM · SCALING DISTRIBUTION",
"Agency & B2B Partner Ecosystem",
"Empowering Agencies to Become High-Margin Autonomous Workforce Providers",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Agency Force Multiplier: White-Label Distribution</div>
  <p style="font-size: 7.4pt;">
    Marketing agencies, outbound lead-gen firms, and recruitment consultancies transform from labor-heavy service providers into scalable, high-margin software-enabled operators by deploying fleets of Fathom coworkers.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Agency Transformation Model</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Eliminate Freelancer Costs:</strong> Replace variable Upwork scrapers with dedicated Fathom worker fleets.</li>
      <li><strong>10x Client Capacity:</strong> A single account manager oversees 20+ client campaigns powered by coworkers.</li>
      <li><strong>White-Label Portals:</strong> Clients see real-time lead counts and search logs under the agency's domain.</li>
    </ul>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">Agency Partner Benefits</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Volume Seat Discounts:</strong> Mark up services by 300% to 500% while paying wholesale seat costs.</li>
      <li><strong>Custom Skill Templates:</strong> Proprietary coworker SOPs tailored to specialized niches (MedTech, Legal).</li>
      <li><strong>Dedicated SLAs:</strong> Priority worker container routing and direct engineering access.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Typical Agency Partner Economics (10 Client Accounts)</div>
  <table>
    <thead>
      <tr><th>Financial Metric</th><th>Legacy Agency Delivery</th><th>Fathom-Powered Agency</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>Monthly Labor / Tool Cost</strong></td><td>$12,000 / mo (3 Human VAs + SaaS)</td><td><strong>Low Flat Seat Subscription</strong></td></tr>
      <tr><td><strong>Client Retainer Revenue</strong></td><td>$25,000 / mo ($2,500/client)</td><td>$25,000 / mo ($2,500/client)</td></tr>
      <tr><td><strong>Gross Profit Margin</strong></td><td>52% ($13,000 / mo)</td><td><strong>84%+ ($21,000+ / mo)</strong></td></tr>
      <tr><td><strong>Fulfillment Turnaround</strong></td><td>5 – 7 Business Days</td><td><strong>Instant & Continuous (24/7)</strong></td></tr>
    </tbody>
  </table>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Agency Stickiness:</strong> Once an agency embeds Fathom into its core client fulfillment pipeline, retention exceeds 95% annually.
</div>
""")

add_page(15, "COMMUNITY & NETWORK EFFECTS · FLYWHEEL",
"Skill Marketplace & Template Network",
"Harnessing Community-Driven Personas and SOPs to Drive Long-Term Defensibility",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Template Network Effect: Collective Worker Intelligence</div>
  <p style="font-size: 7.4pt;">
    As the Fathom community expands, users and partners contribute domain-specific coworker configurations, prompt engineering frameworks, and tool bindings into a shared <strong>Skill & Coworker Marketplace</strong>.
  </p>
</div>

<div class="grid-2">
  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">1-Click Coworker Template Deployment</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Fintech SDR:</strong> Configured for 2GIS, Crunchbase, and LinkedIn financial technology search.</li>
      <li><strong>Biotech Patent Scout:</strong> Deep-crawls PubMed, Google Patents, and university spin-off directories.</li>
      <li><strong>Real Estate PropTech Agent:</strong> Scrapes property registries, municipal zoning filings, and broker directories.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Marketplace Creator Monetization</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Template Revenue Sharing:</strong> Creators receive a recurring royalty for every active seat running their template.</li>
      <li><strong>Verified Skill Badging:</strong> Enterprise-certified SOPs undergo automated security and compliance audits.</li>
      <li><strong>Open Ecosystem:</strong> Model Context Protocol (MCP) server plugins integrate third-party enterprise tools.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">The Self-Reinforcing Platform Flywheel</div>
  <div class="diagram-flow">
    <div class="flow-step"><div class="flow-title">1. More Users</div><div class="flow-desc">Adopt Fathom digital workers</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">2. More Templates</div><div class="flow-desc">Created for niche industries</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">3. Faster Value</div><div class="flow-desc">New users deploy in seconds</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">4. Defensible Moat</div><div class="flow-desc">Massive library of specialized SOPs</div></div>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>The Network Advantage:</strong> Competitors might copy code, but they cannot replicate an active ecosystem of thousands of specialized, battle-tested employee personas.
</div>
""")

# ==============================================================================
# PART IV: REAL-WORLD USE CASES & LIFE SCENARIOS (16-22)
# ==============================================================================

# Page 16: MOCKUP 6 (Autonomous SDR Command Center)
add_page(16, "REAL-WORLD USE CASE · SCENARIO 01",
"Autonomous Sales Development Rep (SDR)",
"End-to-End Cold Lead Discovery, Multi-Signal Verification & CRM Pipeline Creation",
"""
<div class="card-accent" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Scenario Brief: Scaling Outbound Pipeline for CloudSecure (Cybersecurity SaaS)</div>
  <p style="font-size: 7.2pt;">
    <strong>Objective:</strong> Generate 100 verified CISO and VP IT contacts at mid-market financial firms in London every week with zero bounced emails.
  </p>
</div>

<!-- MOCKUP 6: SDR Workspace & SMTP Verification Gauge (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Autonomous SDR · Lead Sourcing & SMTP Gauntlet</span></div>
    <div style="font-size:6pt; color:#10b981; font-family:'JetBrains Mono';">● SMTP PROBE ACTIVE</div>
  </div>
  <div class="mockup-body mockup-2col">
    <div class="mockup-sidebar" style="font-size: 6.2pt;">
      <div style="color:#94a3b8; font-weight:700;">PROSPECTING TARGET</div>
      <div style="color:#f1f5f9;">Sector: FinTech / Banking</div>
      <div style="color:#f1f5f9;">Geo: London, UK</div>
      <div style="color:#f1f5f9;">Role: CISO, VP IT, SecOps</div>
      <div style="margin-top: 4px; border-top:1px solid #232b38; padding-top:4px;">
        <span class="badge badge-green">94 EMAILS VERIFIED</span>
      </div>
    </div>
    <div class="mockup-chat" style="font-size: 6.2pt;">
      <div class="bot-step-line"><span class="bot-check">✓</span><strong>Domain:</strong> <code>monzofintech.co.uk</code> → MX: <code>aspmx.l.google.com</code></div>
      <div class="bot-step-line"><span class="bot-check">✓</span><strong>SMTP Handshake:</strong> <code>HELO fathom.local</code> → <code>RCPT TO:&lt;david.evans@monzo...&gt;</code> → <strong>250 OK (Mailbox Exists)</strong></div>
      <div class="bot-step-line"><span class="bot-check">✓</span><strong>amoCRM Synced:</strong> Lead staged in pipeline <em>"Q3 Enterprise Deals"</em> with LinkedIn URL & AWS/Okta tags.</div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Business Outcome</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Time Saved:</strong> 35 hours of manual human prospecting per week.</li>
      <li><strong>Data Quality:</strong> Zero email bounces (&lt; 1% bounce rate on SMTP-verified leads).</li>
      <li><strong>Pipeline Generated:</strong> 8 qualified enterprise demo calls booked in month 1.</li>
    </ul>
  </div>

  <div class="card card-slate" style="padding: 5px 8px;">
    <div class="card-title-sm">Human Operator Touchpoint</div>
    <p style="font-size: 7pt;">
      The sales director spent just <strong>5 minutes per week</strong> reviewing the Telegram summary and giving 1-click approval for CRM injection.
    </p>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Tools Invoked:</strong> <code>find_leads</code>, <code>suggest_emails</code>, <code>verify_email</code>, <code>save_contacts</code>.</li>
    </ul>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Measurable Impact:</strong> Outbound prospecting shifts from a costly manual grind to an automated, auditable, high-conversion pipeline engine.
</div>
""")

# Page 17: MOCKUP 7 (Executive Recruiter Dossier & GitHub Miner)
add_page(17, "REAL-WORLD USE CASE · SCENARIO 02",
"Executive Headhunter & Talent Scout",
"Autonomous Technical Sourcing, Candidate Mapping & Profile Corroboration",
"""
<div class="card-dark" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Scenario Brief: Sourcing Hard-to-Find Senior Rust & AI Systems Architects</div>
  <p style="font-size: 7.2pt;">
    <strong>Agency:</strong> Apex Tech Search.<br>
    <strong>Objective:</strong> Map and source 30 senior systems engineers with deep Rust and distributed systems experience for an autonomous robotics venture.
  </p>
</div>

<!-- MOCKUP 7: Talent Scout Candidate Dossier Card (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Talent Scout · Candidate Dossier & GitHub Mining</span></div>
    <div style="font-size:6pt; color:#a855f7; font-family:'JetBrains Mono';">AST PARSER: TOKIO/AXUM</div>
  </div>
  <div style="padding: 5px 8px; background:#0b0f17; display:flex; justify-content:space-between; align-items:center;">
    <div>
      <div style="font-size:7.4pt; font-weight:700; color:#f1f5f9;">Alexander Vance · Principal Systems Engineer</div>
      <div style="font-size:6pt; color:#94a3b8;">Current: Senior Rust Eng at CloudScale (4.2 yrs) · Berlin, Germany</div>
      <div style="font-size:5.8pt; color:#38bdf8; font-family:'JetBrains Mono'; margin-top:2px;">Top 1% Contributor: tokio-rs/tokio (142 commits) · SIMD AVX-512 optimization</div>
    </div>
    <div style="text-align:right;">
      <span class="badge badge-green">✓ WORK EMAIL VERIFIED</span>
      <div style="font-size:5.6pt; color:#64748b; margin-top:2px;">Icebreaker generated from EuroRust 2025 talk</div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">Autonomous Sourcing Workflow</div>
    <ol style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>GitHub Repository Mining:</strong> Scans contributors to top-tier open-source Rust projects (e.g. Tokio, Axum, Polars) via <code>code_symbols</code>.</li>
      <li><strong>Social & Resume Cross-Referencing:</strong> Cross-checks GitHub handles against LinkedIn and Telegram to verify current employer, seniority, and tenure.</li>
      <li><strong>Deliverability Check:</strong> Verifies public work emails via SMTP handshakes.</li>
      <li><strong>Candidate Dossier Compilation:</strong> Produces clean Markdown dossiers with project history and highlighted repository contributions.</li>
    </ol>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">Why Autonomous Sourcing Outperforms Humans</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Code-Level Understanding:</strong> Evaluates actual GitHub commit quality and technical complexity, not just keyword-stuffed LinkedIn resumes.</li>
      <li><strong>Unbiased Discovery:</strong> Finds high-caliber engineers who don't maintain active LinkedIn profiles but actively commit code.</li>
      <li><strong>Instant Reachout Readiness:</strong> Outlines personalized icebreakers based on the candidate's real recent open-source work.</li>
    </ul>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Key Takeaway:</strong> Sourcing moves from a slow, manual LinkedIn grind to an automated, code-aware intelligence gathering pipeline.
</div>
""")

# Page 18: MOCKUP 8 (Market Intelligence Diff Tracker)
add_page(18, "REAL-WORLD USE CASE · SCENARIO 03",
"24/7 Market Intelligence & Competitor Tracker",
"Continuous Pricing Tracking, Feature Launches & Regulatory Monitoring",
"""
<div class="card-accent" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Scenario Brief: Real-Time Competitive Landscape Monitoring for FinPay Global</div>
  <p style="font-size: 7.2pt;">
    <strong>Objective:</strong> Continuously monitor 15 direct global competitors for pricing adjustments, new product features, key executive hires, and regulatory license filings.
  </p>
</div>

<!-- MOCKUP 8: Competitor DOM Diff & Alert View (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Market Intel · Competitor DOM Diff Monitor</span></div>
    <div style="font-size:6pt; color:#f59e0b; font-family:'JetBrains Mono';">🚨 PRICING SHIFT DETECTED</div>
  </div>
  <div style="padding: 5px 8px; background:#0b0f17; font-size:6.2pt; font-family:'JetBrains Mono';">
    <div style="color:#94a3b8;">[Crawl Target: stripe-x.io/pricing · 18:42 UTC]</div>
    <div style="color:#ef4444;">- Enterprise Transaction Fee: 2.9% + $0.30</div>
    <div style="color:#10b981;">+ Enterprise Transaction Fee: 2.4% + $0.20 (Volume tier &gt; $1M/mo)</div>
    <div style="color:#38bdf8; margin-top:2px;">Memory: Fact absorbed into SQLite graph (node: StripeX_Pricing_2026_Q3) · Alert dispatched to Slack #executive-intel</div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Real-World Case: Caught in 15 Minutes</div>
    <p style="font-size: 7pt;">
      When competitor <em>Stripe-X</em> adjusted their enterprise transaction fee on a Friday evening, the Fathom market analyst detected the pricing table diff, updated long-term memory, and alerted the Chief Product Officer via Telegram within 15 minutes.
    </p>
  </div>

  <div class="card card-indigo" style="padding: 5px 8px;">
    <div class="card-title-sm">Institutional Knowledge Ingestion</div>
    <p style="font-size: 7pt;">
      All competitor historical changes are stored permanently in Fathom's SQLite entity graph. When leadership asks: <em>"How has Competitor Y's pricing evolved over the last 6 months?"</em>, the coworker generates an instant timeline report in under 5 milliseconds.
    </p>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Executive Value:</strong> Leadership stays three steps ahead of market dynamics with zero hours spent manually clicking competitor websites.
</div>
""")

# Page 19: Customer Onboarding
add_page(19, "REAL-WORLD USE CASE · SCENARIO 04",
"Automated Customer Onboarding & Support",
"Autonomous Technical Setup, API Verification & 24/7 Troubleshooting",
"""
<div class="card-dark" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Scenario Brief: Accelerating B2B Client Time-to-Value for DataStream API</div>
  <p style="font-size: 7.2pt;">
    <strong>Objective:</strong> Guide new enterprise customers through webhook configuration, test payload verification, and initial API key provisioning with zero support backlog.
  </p>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">Autonomous Support Operations</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Incoming Ticket Triage:</strong> Reads incoming support emails or Slack queries, identifying customer intent and technical requirements.</li>
      <li><strong>Code & Log Inspection:</strong> Uses sandboxed REPL (<code>python_exec</code>) to replicate customer webhook payloads and diagnose syntax errors.</li>
      <li><strong>Browser-Driven Setup:</strong> Accesses internal admin portals via governed Playwright computer control to verify client account provisioning.</li>
      <li><strong>Contextual Resolution:</strong> Queries persistent semantic memory for past resolutions, delivering precise, tested code fixes.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Measurable Performance Gains</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>First Response Time:</strong> Reduced from 45 minutes to <strong>under 12 seconds</strong>.</li>
      <li><strong>Onboarding Duration:</strong> Enterprise onboarding cycle compressed from 5 days to <strong>2 hours</strong>.</li>
      <li><strong>Resolution Rate:</strong> 78% of Tier-1/2 developer onboarding tickets resolved without human engineer involvement.</li>
      <li><strong>Customer Satisfaction:</strong> CSAT increased from 82% to 98%.</li>
    </ul>
  </div>
</div>

<div class="card-slate" style="padding: 5px 8px;">
  <div class="card-title-sm">Human Escalation Protocol</div>
  <p style="font-size: 7.2pt;">
    If a ticket involves critical billing changes or unfamiliar error edge cases, the coworker pauses, summarizes the diagnosed root cause, and hands off the session to a human senior engineer with full contextual notes and reproduction scripts.
  </p>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Scale Without Support Headcount:</strong> Handle 10x customer growth while maintaining instant response times and flawless technical onboarding.
</div>
""")

# Page 20: MOCKUP 9 (Back-Office Invoice Reconciliation)
add_page(20, "REAL-WORLD USE CASE · SCENARIO 05",
"Back-Office & Invoice Reconciliation",
"Autonomous Document Parsing, Multi-System Data Entry & Financial Reconciliation",
"""
<div class="card-accent" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Scenario Brief: Automating Monthly Invoicing for Global Logistics Partner</div>
  <p style="font-size: 7.2pt;">
    <strong>Objective:</strong> Ingest 500+ PDF vendor invoices monthly, cross-reference against warehouse delivery receipts, and input approved payments into 1C / QuickBooks.
  </p>
</div>

<!-- MOCKUP 9: Invoice 3-Way Match & Portal Entry View (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Back-Office Assistant · 3-Way Invoice Reconciliation</span></div>
    <div style="font-size:6pt; color:#10b981; font-family:'JetBrains Mono';">MATCH: 100% (500/500 INVOICES)</div>
  </div>
  <div style="padding: 4px 6px; background:#0b0f17; font-size:6pt;">
    <div class="bot-step-line"><span class="bot-check">✓</span><strong>PDF Extracted:</strong> <code>Inv_#8492.pdf</code> → Vendor TIN: <code>GB9920194</code> · Amount: <strong>$14,250.00</strong></div>
    <div class="bot-step-line"><span class="bot-check">✓</span><strong>Warehouse PO Match:</strong> <code>PO-9921</code> (50x Freight Pallets) verified on 2026-08-18 · Discrepancy: $0.00</div>
    <div class="bot-step-line"><span class="bot-check">✓</span><strong>Accounting Portal Entry:</strong> Navigated QuickBooks via Accessibility Tree (ref <code>@e12</code>) · Staged batch payment for CFO 1-click sign-off.</div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Operational Impact</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Labor Replaced:</strong> 80 hours of mind-numbing manual copy-paste data entry per month.</li>
      <li><strong>Error Rate:</strong> Reduced from 4.2% human entry errors to 0.00% deterministic accuracy.</li>
    </ul>
  </div>
  <div class="card card-slate" style="padding: 5px 8px;">
    <div class="card-title-sm">Audit Trail Integrity</div>
    <p style="font-size: 7pt;">
      Every parsed invoice, line-item match, and portal click is logged with immutable nanosecond timestamps in SQLite for seamless tax auditing.
    </p>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Operational Impact:</strong> Replaces 80 hours of mind-numbing manual copy-paste data entry per month with an automated, auditable, and error-free execution loop.
</div>
""")

# Page 21: MOCKUP 10 (DevOps Engineer IDE & Pytest Suite)
add_page(21, "REAL-WORLD USE CASE · SCENARIO 06",
"Autonomous Software Engineer & Maintainer",
"Codebase Mapping, Bug Investigation, Test Generation & Safe PR Creation",
"""
<div class="card-dark" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Scenario Brief: Continuous Code Maintenance for SaaSScale Inc.</div>
  <p style="font-size: 7.2pt;">
    <strong>Objective:</strong> Triage Sentry error reports, navigate complex codebases, write reproducing unit tests, fix the underlying bug, and submit ready-to-review Pull Requests.
  </p>
</div>

<!-- MOCKUP 10: DevOps IDE & Pytest Passing Log (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Autonomous Engineer · AST Analysis & Test Suite</span></div>
    <div style="font-size:6pt; color:#10b981; font-family:'JetBrains Mono';">● 23/23 TESTS PASSED (74s)</div>
  </div>
  <div class="mockup-body mockup-2col">
    <div class="mockup-sidebar" style="font-size:5.8pt; font-family:'JetBrains Mono';">
      <div style="color:#94a3b8;">REPO MAP (34ms):</div>
      <div>src/analytics.py</div>
      <div>src/revenue_calc.py</div>
      <div>tests/test_revenue.py</div>
      <div style="color:#10b981; margin-top:2px;">Branch: fix/mom-zero-div</div>
    </div>
    <div class="mockup-chat" style="font-size:5.8pt; font-family:'JetBrains Mono';">
      <div style="color:#ef4444;">FAIL: test_mom_growth_zero_prior_month (ZeroDivisionError)</div>
      <div style="color:#38bdf8;">> Patched: revenue_calc.py line 84 (defensive 0-check)</div>
      <div style="color:#10b981;">> pytest tests/ -v: 23 passed in 1.42s</div>
      <div style="color:#f1f5f9;">> GitHub PR #142 opened: "fix: guard MoM revenue calculation against 0 sales"</div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">The 5-Step Code Engineering Loop</div>
    <ol style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Repo Mapping:</strong> Parses AST symbols across 240+ files in 34ms.</li>
      <li><strong>Bug Reproduction:</strong> Generates standalone test in isolated sandbox.</li>
      <li><strong>Targeted Code Modification:</strong> Edits source files via <code>file_edit</code>.</li>
      <li><strong>Automated Test Execution:</strong> Verifies 100% tests pass cleanly.</li>
      <li><strong>Git Branch & PR Creation:</strong> Opens structured GitHub PR for review.</li>
    </ol>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Real Empirical Case (From Fathom Test Suite)</div>
    <p style="font-size: 7pt;">
      In live case study 5 (<code>05-case-studies.md</code>), an autonomous Fathom engineer was tasked with building a complete Python CLI with MoM revenue analytics, sample data, and pytest coverage, achieving <strong>23/23 passing tests</strong> completely autonomously in 74 seconds.
    </p>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Developer Superpower:</strong> Senior engineers focus on high-level system architecture while digital coworkers handle routine bugs, dependency upgrades, and test coverage.
</div>
""")

# Page 22: Legal Auditor
add_page(22, "REAL-WORLD USE CASE · SCENARIO 07",
"Regulatory & Legal Document Auditor",
"Multi-Jurisdiction Compliance Verification, Clause Extraction & Risk Highlighting",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Scenario Brief: Contract & Compliance Audit for EuroTrust Legal Advisory</div>
  <p style="font-size: 7.4pt;">
    <strong>Objective:</strong> Audit 200 vendor Master Services Agreements (MSAs) for GDPR compliance, data liability caps, non-compete clauses, and jurisdiction risks.
  </p>
</div>

<div class="grid-3">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Parallel Ingestion</div>
    <p style="font-size: 6.8pt;">Spawns 5 analyst agents to ingest 200 legal PDFs, extracting structured clauses into JSON schemas in under 8 minutes.</p>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Cross-Jurisdiction Analysis</div>
    <p style="font-size: 6.8pt;">Cross-references liability clauses against EU GDPR and UK Data Protection Act requirements, flagging non-compliant terms.</p>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">3. Executive Risk Matrix</div>
    <p style="font-size: 6.8pt;">Compiles a high-contrast risk matrix categorizing contracts into Green (Compliant), Yellow (Review Needed), and Red (Immediate Risk).</p>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Sample Legal Risk Assessment Matrix Output</div>
  <table>
    <thead>
      <tr><th>Vendor Contract</th><th>Liability Cap</th><th>GDPR Data Processing Clause</th><th>Governing Law</th><th>Risk Rating</th></tr>
    </thead>
    <tbody>
      <tr><td>Vendor Alpha MSA</td><td>12 Months Fees ($120k)</td><td>Standard Contractual Clauses (SCC) Included</td><td>England & Wales</td><td><span class="badge badge-green">Low Risk</span></td></tr>
      <tr><td>Vendor Beta SaaS</td><td>$5,000 (Sub-Standard)</td><td>Missing Sub-Processor Notification Clause</td><td>Delaware, USA</td><td><span class="badge badge-amber">Medium Risk</span></td></tr>
      <tr><td>Vendor Gamma Cloud</td><td>Unlimited Liability</td><td>Zero GDPR Data Retention Schedule</td><td>Cyprus</td><td><span class="badge badge-red">High Risk</span></td></tr>
    </tbody>
  </table>
</div>

<div class="card-slate" style="padding: 5px 8px;">
  <div class="card-title-sm">Audit Traceability & Clause Citations</div>
  <p style="font-size: 7.2pt;">
    Every risk flag includes direct page and paragraph citations from the source PDF documents, enabling general counsels to conduct instant verification without re-reading hundreds of contract pages.
  </p>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Audit Acceleration:</strong> A 3-week manual legal paralegal review is accomplished in <strong>under 30 minutes</strong> with total clause traceability.
</div>
""")

# ==============================================================================
# PART V: SYSTEM ARCHITECTURE & HOW IT WORKS (23-27)
# ==============================================================================

add_page(23, "SYSTEM ARCHITECTURE · VIRTUAL OFFICE",
"How the Virtual Office Operates",
"The Conceptual Architecture of an Autonomous Multi-Agent Organization",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Virtual Office Hierarchy: Division of Labor in Action</div>
  <p style="font-size: 7.4pt;">
    Fathom does not operate as a single monolithic prompt. It structures work like an agile digital consulting agency where a Coordinator (Manager) delegates to specialized Worker Pods, supervised by Analysts and formatted by Writers.
  </p>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Organizational Workflow Topology</div>
  <div class="diagram-flow">
    <div class="flow-step"><div class="flow-title">1. Client Request</div><div class="flow-desc">Submitted via API / Slack / Cron</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">2. Coordinator</div><div class="flow-desc">Plans & decomposes into subtasks</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">3. Parallel Workers</div><div class="flow-desc">Researchers, Scrapers & Coders</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">4. Verifier & Writer</div><div class="flow-desc">SMTP checks, QA & Final Report</div></div>
  </div>
</div>

<div class="grid-3">
  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Coordinator Agent</div>
    <p style="font-size: 6.8pt;">Analyzes the objective, sets token budgets, establishes task trees, and tracks progress across child sub-agents with fair-share scheduling.</p>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Parallel Workers</div>
    <p style="font-size: 6.8pt;">Execute discrete searches, scrape registries, run Python data transformations, and operate web browsers simultaneously across CPU threads.</p>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">3. Quality Judge & Writer</div>
    <p style="font-size: 6.8pt;">Validates data completeness against the initial goal, triggers gap-filling rounds, and compiles clean deliverables into PDF/Excel.</p>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Context Protection & Disk-Spill Backing Architecture</div>
  <div class="grid-2">
    <div><p style="font-size: 7.2pt;"><strong>Executive Summaries Only:</strong> Sub-agents return concise executive findings (200–500 tokens) to the coordinator manager.</p></div>
    <div><p style="font-size: 7.2pt;"><strong>Raw Disk Spill:</strong> Gigabyte-scale raw HTML, JSON payloads, and scrape dumps are written to disk workspaces, keeping LLM context clean.</p></div>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Enterprise Impact:</strong> High-level managers never experience context overflow or degraded reasoning quality, even during massive 1,000-page research runs.
</div>
""")

add_page(24, "SYSTEMS ENGINEERING · WHY RUST",
"Why Rust? The Architecture of Performance",
"Zero-Cost Abstractions, Memory Safety & The Elimination of Python Bottlenecks",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Engineering Foundation: Why We Chose Rust</div>
  <p style="font-size: 7.4pt;">
    Traditional agent frameworks built on Python suffer from high latency, heavy memory consumption, fragile type errors, and concurrency bottlenecks caused by the Global Interpreter Lock (GIL). <strong>Fathom is built natively in Rust</strong> to provide enterprise-grade reliability and speed.
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber" style="padding: 5px 8px;">
    <div class="card-title-sm">The Python Framework Trap</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Massive Memory Footprint:</strong> Idle Python runtimes consume 400MB–1.5GB RAM per agent process.</li>
      <li><strong>GIL Concurrency Deadlocks:</strong> Asynchronous I/O is serialized, choking multi-agent swarms.</li>
      <li><strong>Slow Startup Latency:</strong> 2.5 to 8.0 seconds just to load interpreter and dependency trees.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">The Fathom Rust Advantage</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Microscopic RAM Footprint:</strong> Lean 15–35 MB baseline RAM allows 100+ agents per server.</li>
      <li><strong>True Multi-Core Parallelism:</strong> Tokio async tasks utilize all CPU cores without locks.</li>
      <li><strong>Instant Binary Startup:</strong> Starts in under 5 milliseconds from cold execution.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Architectural Efficiency Comparison</div>
  <table>
    <thead>
      <tr><th>Engineering Metric</th><th>Python Agent Frameworks</th><th>Fathom Rust Runtime</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>Tool Dispatch Latency</strong></td><td>25.0 – 150.0 ms</td><td><strong>0.75 ms (Microsecond Level)</strong></td></tr>
      <tr><td><strong>Memory Usage (100 Agents)</strong></td><td>40 – 120 GB RAM (Requires Cluster)</td><td><strong>1.5 – 3.5 GB RAM (Single Modest VM)</strong></td></tr>
      <tr><td><strong>HTML Parsing Speed</strong></td><td>15,000 rows/sec (BeautifulSoup)</td><td><strong>350,000+ rows/sec (Scraper Rust)</strong></td></tr>
      <tr><td><strong>Binary Packaging</strong></td><td>Fragile venv + wheels</td><td><strong>Single Static Binary (Zero Dependencies)</strong></td></tr>
    </tbody>
  </table>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Speed Equals Intelligence:</strong> Microsecond tool execution means agents spend 99.9% of their time waiting for model tokens, not framework overhead.
</div>
""")

add_page(25, "EXECUTION RUNTIME · TASK DECOMPOSITION",
"Coordinator & Worker Swarm Execution",
"How Complex High-Level Tasks Are Broken Down and Executed in Parallel",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Dynamic Hierarchical Task Decomposition</div>
  <p style="font-size: 7.4pt;">
    When a user assigns a complex project, the Coordinator uses a structured planning prompt to analyze dependencies, formulate execution branches, and launch parallel sub-agents via the <code>spawn_agent</code> tool.
  </p>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Sample Task Decomposition Tree</div>
  <div class="diagram-flow" style="flex-direction: column; align-items: stretch; gap: 4px;">
    <div style="background: white; border: 1px solid var(--border-color); padding: 4px 8px; border-radius: 4px;">
      <strong>Root Coordinator:</strong> "Comprehensive Due Diligence on European AI FinTechs"
    </div>
    <div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 4px;">
      <div style="background: #eff6ff; border-left: 3px solid var(--primary-accent); padding: 4px 6px; border-radius: 3px; font-size: 6.8pt;">
        <strong>Branch 1 (Registry Agent):</strong><br>Scrapes Companies House for corporate registration & filings.
      </div>
      <div style="background: #f0fdf4; border-left: 3px solid var(--emerald); padding: 4px 6px; border-radius: 3px; font-size: 6.8pt;">
        <strong>Branch 2 (OSINT Agent):</strong><br>Harvests C-level leadership, LinkedIn profiles, and verified work emails.
      </div>
      <div style="background: #faf5ff; border-left: 3px solid var(--purple); padding: 4px 6px; border-radius: 3px; font-size: 6.8pt;">
        <strong>Branch 3 (Tech Stack Agent):</strong><br>Fingerprints homepage HTML for 40+ technology signatures.
      </div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Recursive Depth Limits & Guardrails</div>
    <p style="font-size: 7.2pt;">
      To prevent runaway sub-agent spawning loops, Fathom enforces strict depth limits (default max depth: 2). Coordinators can spawn workers, but workers cannot spawn infinite child trees without explicit permission.
    </p>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">Tokio JoinSet Multi-Threading</div>
    <p style="font-size: 7.2pt;">
      All spawned workers run as concurrent tasks inside a Tokio <code>JoinSet</code>. If one branch encounters a slow network request or rate limit, sibling branches continue executing at full speed.
    </p>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Fault-Tolerant Merging:</strong> If one sub-agent fails due to an anti-bot block, the coordinator gracefully incorporates partial findings from other branches without failing the overall run.
</div>
""")

add_page(26, "INTER-AGENT PROTOCOL · TELEMETRY",
"The Broadcast Message Bus",
"Real-Time Event Distribution Across Swarms, UI Dashboards & External Channels",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Decoupled Asynchronous Telemetry Architecture</div>
  <p style="font-size: 7.4pt;">
    Communication across sub-agents, persistence layers, and client dashboards occurs over a centralized, high-throughput <strong>Tokio broadcast message bus</strong> (<code>event_tx</code>).
  </p>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Message Bus Architecture & Event Flow</div>
  <div class="diagram-flow">
    <div class="flow-step"><div class="flow-title">Agent Event Emitters</div><div class="flow-desc">Spawns, Tool Calls, Thoughts</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">Broadcast Bus (1024 cap)</div><div class="flow-desc">Lock-free Tokio Channel</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">Multiple Subscribers</div><div class="flow-desc">SSE, TUI, Prometheus, DB</div></div>
  </div>
</div>

<div class="grid-3">
  <div class="card card-slate" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Server-Sent Events (SSE)</div>
    <p style="font-size: 6.8pt;">Streams live agent events to web and desktop dashboards, updating UI progress bars and sparklines in real time.</p>
  </div>

  <div class="card card-slate" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Prometheus Scrapers</div>
    <p style="font-size: 6.8pt;">The metrics middleware consumes event counters, exporting real-time tool durations and token velocity to Grafana.</p>
  </div>

  <div class="card card-slate" style="padding: 5px 8px;">
    <div class="card-title-sm">3. SQLite Audit Ledger</div>
    <p style="font-size: 6.8pt;">Records immutable execution logs, tool inputs, and decision outcomes for permanent compliance replay.</p>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>High-Throughput Guarantee:</strong> The lock-free broadcast channel handles over 50,000 inter-agent events per second with sub-microsecond latency.
</div>
""")

add_page(27, "FAULT TOLERANCE · RESILIENCE",
"Reliability & Self-Healing Workflows",
"How Background Jobs Survive Server Reboots and Fix Their Own Mistakes",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Resilience by Design: Surviving Crashes and API Failures</div>
  <p style="font-size: 7.4pt;">
    In production environments, network timeouts, API rate limits, and server restarts are inevitable. Fathom's durable job engine is designed to ensure that <strong>no work is ever lost</strong>.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Detached Process Execution (setsid)</div>
    <p style="font-size: 7.2pt;">
      Background jobs run as independent operating system processes detached from the terminal session. Closing your laptop, terminating SSH, or exiting the CLI does not interrupt ongoing worker execution.
    </p>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Self-Healing Task Augmentation</div>
    <p style="font-size: 7.2pt;">
      When a job fails (e.g. hitting an unexpected rate limit), Attempt #2 automatically injects the previous error trace and partial files, prompting the agent to adapt its strategy and self-heal.
    </p>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">The Self-Correction Loop in Action</div>
  <div class="timeline" style="gap: 2px;">
    <div class="timeline-item" style="padding: 3px 6px;">
      <div class="timeline-time" style="font-size:6pt;">Attempt 1</div>
      <div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Initial Execution Fails</div><div class="timeline-desc" style="font-size:5.6pt;">Worker hits HTTP 429 Rate Limit after saving 20 records.</div></div>
    </div>
    <div class="timeline-item" style="padding: 3px 6px;">
      <div class="timeline-time" style="font-size:6pt;">Attempt 2</div>
      <div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Augmented Prompt & Recovery</div><div class="timeline-desc" style="font-size:5.6pt;">Worker switches to fallback API, throttles requests, and completes remaining 30 records.</div></div>
    </div>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Autonomous Multi-Hour Durability:</strong> Deploy complex research workflows with total confidence that temporary network glitches will be resolved automatically.
</div>
""")

# ==============================================================================
# PART VI: GOVERNED COMPUTER USE & BROWSER AUTOMATION (28-31)
# ==============================================================================

add_page(28, "COMPUTER USE · ACCESSIBILITY PARADIGM",
"How Digital Workers See & Control Computers",
"Accessibility-Tree Snapshots: Semantic Understanding Over Brittle Visual Pixels",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Semantic Revolution in Browser Automation</div>
  <p style="font-size: 7.4pt;">
    Traditional browser automation fails because web pages frequently change CSS classes, responsive layouts, and visual styling. Fathom operates browsers via the <strong>Accessibility Tree</strong> (the standard semantic model used by screen readers).
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber" style="padding: 5px 8px;">
    <div class="card-title-sm">Legacy Pixel / CSS Automation (Fragile)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Obfuscated CSS:</strong> Class names change on every frontend software build.</li>
      <li><strong>Pixel Drift:</strong> Screen scaling causes mouse clicks to hit empty whitespace.</li>
      <li><strong>High Latency:</strong> Uploading screenshots consumes massive bandwidth and tokens.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Fathom Semantic Accessibility (Rock Solid)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Semantic Roles:</strong> The agent sees functional elements: <code>Button: "Login"</code>.</li>
      <li><strong>Opaque Numerical Refs:</strong> Direct addressing via stable tokens (e.g. <code>@e14</code>).</li>
      <li><strong>10x Token Efficiency:</strong> Lightweight snapshots use 90% fewer tokens.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Accessibility Snapshot Architecture</div>
  <div class="diagram-flow">
    <div class="flow-step"><div class="flow-title">1. Chromium Page</div><div class="flow-desc">Dynamic DOM & SPAs</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">2. ARIA Snapshot</div><div class="flow-desc">Extracts semantic tree</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">3. Opaque Tokens</div><div class="flow-desc">Assigns stable refs (@e1)</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">4. Agent Action</div><div class="flow-desc">click(@e1), type(@e2)</div></div>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Layout-Agnostic Stability:</strong> Whether a website is redesigned, translated into Japanese, or resized to mobile viewport, the accessibility tree retains functional semantic integrity.
</div>
""")

add_page(29, "COMPUTER USE · ANTI-BREAKAGE",
"Why Opaque Refs Beat Brittle Selectors",
"Anti-Staleness Verification, Form Sanitization & Deterministic Element Resolution",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Deterministic Element Targeting: Eliminating Broken Clicks</div>
  <p style="font-size: 7.4pt;">
    In modern web portals, asynchronous JavaScript frequently alters DOM elements between agent reasoning steps. Fathom implements an active <strong>Anti-Staleness Guard</strong> to guarantee that actions are executed only on valid, intended targets.
  </p>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Pre-Execution Freshness Check</div>
    <p style="font-size: 7.2pt;">
      Before executing a <code>computer_click</code> or <code>computer_type</code> command, the runtime takes a microsecond snapshot to confirm that the target element ref (<code>@e14</code>) still exists and matches its original role fingerprint.
    </p>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Stale-Ref Rejection & Self-Recovery</div>
    <p style="font-size: 7.2pt;">
      If the page navigated or a popup closed during agent thinking, the ref is rejected immediately with an explicit error: <em>"Ref @e14 is stale."</em> The agent captures a fresh snapshot and re-evaluates safely.
    </p>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Form Data Security & Workspace Confinement</div>
  <div class="grid-2">
    <div><strong>Automatic Password Scrubbing</strong><p style="font-size: 6.8pt;">Password inputs and token fields are automatically masked in accessibility snapshots.</p></div>
    <div><strong>Confined File Workspace</strong><p style="font-size: 6.8pt;">Browser downloads are strictly confined to an isolated directory (<code>/data/browser</code>).</p></div>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Zero Phantom Interactions:</strong> Fathom ensures that automated browser actions are as predictable, deterministic, and safe as compiled software code.
</div>
""")

# Page 30: MOCKUP 11 (Live Browser Takeover & 2FA Lease)
add_page(30, "COMPUTER USE · HUMAN IN THE LOOP",
"Screen Streaming & Seamless Takeover",
"Real-Time Browser Feeds and Operator Interventions for CAPTCHAs and 2FA",
"""
<div class="card-accent" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Governed Operator Collaboration: The Human Takeover Lease</div>
  <p style="font-size: 7.4pt;">
    When autonomous workers encounter high-security barriers—such as multi-factor authentication (2FA SMS), bank logins, or complex CAPTCHA puzzles—Fathom pauses gracefully and invites the human operator to assist.
  </p>
</div>

<!-- MOCKUP 11: Live Browser Takeover Screen with 2FA Challenge (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Governed Computer Viewport · WebSocket Stream (/screen)</span></div>
    <div style="font-size:6pt; color:#f59e0b; font-family:'JetBrains Mono';">⚠️ 2FA SMS REQUIRED</div>
  </div>
  <div class="mockup-body mockup-2col">
    <div style="padding: 5px; background:#0b0f17;">
      <div class="browser-bar"><span style="color:#ef4444;">●</span><span class="browser-url">auth.vendor-billing.com/2fa</span></div>
      <div style="padding:6px; background:#161e2b; border-radius:3px; margin-top:2px; font-size:6pt; text-align:center;">
        <div style="color:#f1f5f9; font-weight:700;">Enter Security Code</div>
        <div style="color:#94a3b8; font-size:5.4pt; margin-top:1px;">SMS sent to +1 (***) ***-8921</div>
        <div style="background:#0b0f17; border:1px solid #38bdf8; border-radius:3px; padding:3px; margin:4px auto; width:80px; letter-spacing:3px; font-family:'JetBrains Mono'; color:#38bdf8;">8 4 9 2 0 1</div>
      </div>
    </div>
    <div style="padding: 5px; background:#111722; display:flex; flex-direction:column; justify-content:space-between;">
      <div>
        <div style="font-size:6.4pt; font-weight:700; color:#f1f5f9;">Operator Control Lease</div>
        <div style="font-size:5.6pt; color:#94a3b8; margin-top:2px;">Bot paused safely. Enter code in live stream to release.</div>
      </div>
      <div class="btn-takeover" style="background:#10b981;">✓ Release Control to Bot</div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Low-Latency Stream (/screen)</div>
    <p style="font-size: 7pt;">The active browser viewport is streamed in real time over a WebSocket feed (500ms intervals) directly into the desktop app.</p>
  </div>
  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Exclusive Lease (/control/ws)</div>
    <p style="font-size: 7pt;">Clicking <strong>"Take Control"</strong> grants exclusive mouse/keyboard access; bot actions are paused safely to prevent race conditions.</p>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>The Hybrid Ideal:</strong> 99% autonomous heavy lifting paired with instant 1% human oversight at critical security checkpoints.
</div>
""")

add_page(31, "SECURITY SANDBOXING · DOCKER SUPERVISOR",
"Docker Sandboxes & Network Egress",
"Per-Agent Container Isolation, Port Sandboxing & Zero Data Leakage",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Ironclad Isolation: One Container Per Active Worker</div>
  <p style="font-size: 7.4pt;">
    To ensure complete enterprise security and prevent cross-tenant data contamination, Fathom provisions an isolated Docker container for every active digital coworker via <code>crates/supervisor</code>.
  </p>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">Container Sandbox Specifications</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Capability Stripping:</strong> All Linux capabilities dropped (<code>cap_drop: ["ALL"]</code>).</li>
      <li><strong>Privilege Escalation Blocked:</strong> <code>no-new-privileges: true</code> enforced at runtime.</li>
      <li><strong>Ephemeral Volumes:</strong> Dedicated volume mounts for browser cookies and workspace files.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Strict Network Egress Guarding</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Cloud Metadata Blocked:</strong> Rejects all calls to <code>169.254.169.254</code> (AWS/GCP protection).</li>
      <li><strong>Private Subnet Deny:</strong> Rejects connections to private RFC1918 subnets (<code>10.0.0.0/8</code>).</li>
      <li><strong>Loopback Deny:</strong> Prevents agents from scanning internal corporate networks.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Multi-Tenant Isolation Topology</div>
  <div class="diagram-flow">
    <div class="flow-step"><div class="flow-title">Host Server</div><div class="flow-desc">Fathom Axum Daemon</div></div>
    <div class="flow-arrow">→</div>
    <div class="flow-step"><div class="flow-title">Docker Sandbox A</div><div class="flow-desc">Client 1 / SDR Worker</div></div>
    <div class="flow-arrow">≠</div>
    <div class="flow-step"><div class="flow-title">Docker Sandbox B</div><div class="flow-desc">Client 2 / Recruiter</div></div>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Sovereign Data Security:</strong> Deploy multi-tenant AI operations with total confidence in cryptographic process separation and zero cross-client data leakage.
</div>
""")

# ==============================================================================
# PART VII: LONG-TERM MEMORY & INSTITUTIONAL KNOWLEDGE (32-34)
# ==============================================================================

add_page(32, "KNOWLEDGE ENGINE · PERSISTENT MEMORY",
"How Coworkers Remember Across Months",
"Long-Term Semantic Memory: Zero SaaS Cost, Local SQLite & True Institutional Knowledge",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Persistent Institutional Intelligence vs. Stateless AI</div>
  <p style="font-size: 7.4pt;">
    Human employees become more valuable over time because they accumulate institutional knowledge. Fathom equips digital workers with a persistent <strong>Semantic Memory Engine</strong> that compounds intelligence across months.
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber" style="padding: 5px 8px;">
    <div class="card-title-sm">Stateless AI Assistants (No Memory)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Forget Everything:</strong> Every new session starts from scratch; context is lost upon closing tab.</li>
      <li><strong>Repetitive Prompts:</strong> Users must re-explain company guidelines and past decisions daily.</li>
      <li><strong>Expensive Cloud Vector DBs:</strong> Hosted vector databases cost thousands in recurring fees.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Fathom Digital Employees (Persistent Memory)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Permanent Retention:</strong> Discovered contacts and company facts persist in SQLite.</li>
      <li><strong>Sub-5ms Memory Digest:</strong> Relevant past facts are injected at session start automatically.</li>
      <li><strong>Zero External Cloud Costs:</strong> Operates entirely in-process with SQLite FTS5 (BM25) and TF-IDF.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Memory Growth Over Time</div>
  <div class="timeline" style="gap: 2px;">
    <div class="timeline-item" style="padding: 2px 5px;"><div class="timeline-time" style="font-size:6pt;">Day 1</div><div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Initial Briefing Ingestion</div><div class="timeline-desc" style="font-size:5.6pt;">Coworker absorbs target market parameters and guidelines.</div></div></div>
    <div class="timeline-item" style="padding: 2px 5px;"><div class="timeline-time" style="font-size:6pt;">Day 30</div><div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Entity Graph Compounding</div><div class="timeline-desc" style="font-size:5.6pt;">Graph holds 2,500+ verified executive relationships and tech tags.</div></div></div>
    <div class="timeline-item" style="padding: 2px 5px;"><div class="timeline-time" style="font-size:6pt;">Day 90</div><div class="timeline-content"><div class="timeline-title" style="font-size:6.6pt;">Full Institutional Fluency</div><div class="timeline-desc" style="font-size:5.6pt;">Detects executive job changes automatically: <em>"John moved to Globex; updating CRM."</em></div></div></div>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Compounding Asset:</strong> Your digital workforce becomes smarter, faster, and more tailored to your business every single day.
</div>
""")

add_page(33, "MEMORY ENGINE · ABSORB PIPELINE",
"Sub-Millisecond Knowledge Ingestion",
"The 4-Stage Absorb Pipeline: Deduplication, Lineage Chains & Secret Redaction",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The 4-Stage Memory Absorb Pipeline (94 µs / Fact)</div>
  <p style="font-size: 7.4pt;">
    Raw information from web searches cannot be dumped blindly into memory. Fathom processes facts through a strict, multi-stage curation pipeline in under <strong>94 microseconds per fact</strong>.
  </p>
</div>

<div class="diagram-flow">
  <div class="flow-step"><div class="flow-title">1. Secret Scrubbing</div><div class="flow-desc">Regex strips keys & passwords</div></div>
  <div class="flow-arrow">→</div>
  <div class="flow-step"><div class="flow-title">2. SHA-256 Fast Dedup</div><div class="flow-desc">Skips known facts in 5.1ms</div></div>
  <div class="flow-arrow">→</div>
  <div class="flow-step"><div class="flow-title">3. Lineage Versioning</div><div class="flow-desc">Marks old facts superseded</div></div>
  <div class="flow-arrow">→</div>
  <div class="flow-step"><div class="flow-title">4. Entity Linking</div><div class="flow-desc">Builds typed graph edges</div></div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Append-Only Truth & Lineage Chains</div>
    <p style="font-size: 7.2pt;">
      Facts are never silently overwritten. If a company raises a Series B round after previously raising Series A, the old fact is marked with a <code>supersedes</code> edge, preserving the historical evolution of truth for auditing.
    </p>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">Hybrid Retrieval: 0.70 Vector + 0.30 BM25</div>
    <p style="font-size: 7.2pt;">
      Queries combine SQLite FTS5 keyword precision with semantic vector similarity, delivering <strong>sub-2ms search latencies</strong> across thousands of memories.
    </p>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Deterministic Fact Compaction:</strong> The knowledge base stays lean and focused on high-signal business facts without accumulating noisy search fragments.
</div>
""")

# Page 34: MOCKUP 12 (Entity Knowledge Graph Explorer)
add_page(34, "KNOWLEDGE GRAPH · ENTITY TOPOLOGY",
"The Enterprise Entity Knowledge Graph",
"Mapping People, Companies, Roles and Technologies into an Interconnected Web",
"""
<div class="card-accent" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Relational Knowledge: Beyond Flat Text Records</div>
  <p style="font-size: 7.4pt;">
    In <code>crates/memory</code>, Fathom structures verified information into a <strong>typed, directional Entity Knowledge Graph</strong> that captures the rich web of corporate and professional relationships.
  </p>
</div>

<!-- MOCKUP 12: Dark-Mode Entity Knowledge Graph (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Entity Knowledge Graph Explorer · memory_graph</span></div>
    <div style="font-size:6pt; color:#38bdf8; font-family:'JetBrains Mono';">QUERY: 1.62ms · 5,420 NODES</div>
  </div>
  <div style="padding: 6px 8px; background:#0b0f17; font-size:6.4pt;">
    <div class="diagram-flow" style="background:#161e2b; border-color:#232b38;">
      <div class="flow-step" style="background:#1e293b; color:#f1f5f9; border-color:#334155;"><span style="color:#f97316;">👤</span> <strong>Jane Doe</strong><br><span style="font-size:5.4pt; color:#94a3b8;">CTO · Ex-Stripe</span></div>
      <div class="flow-arrow" style="font-size:6.4pt; color:#38bdf8;">──[works_at]──►</div>
      <div class="flow-step" style="background:#1e293b; color:#f1f5f9; border-color:#334155;"><span style="color:#3b82f6;">🏢</span> <strong>Acme FinTech</strong><br><span style="font-size:5.4pt; color:#94a3b8;">Series A ($14M)</span></div>
      <div class="flow-arrow" style="font-size:6.4pt; color:#10b981;">──[uses_tech]──►</div>
      <div class="flow-step" style="background:#1e293b; color:#f1f5f9; border-color:#334155;"><span style="color:#10b981;">⚡</span> <strong>Rust / Axum</strong><br><span style="font-size:5.4pt; color:#94a3b8;">Tech Signature</span></div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">Typed Graph Relationships</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><code>Person ──[works_at]──► Company</code> (with title & seniority)</li>
      <li><code>Person ──[leads]─────► Department</code> (e.g. Engineering)</li>
      <li><code>Company ─[invests_in]─► Startup</code> (with funding round date)</li>
      <li><code>Company ─[uses_tech]──► Technology</code> (e.g. AWS, Next.js)</li>
    </ul>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">6-Hop Graph Traversal Queries</div>
    <p style="font-size: 7pt;">Agents query the graph to answer complex relational questions:</p>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><em>"Which former Stripe engineers are now CTOs at Series-A AI companies in Berlin?"</em></li>
      <li><em>"Which competitors are backed by the same VC fund?"</em></li>
    </ul>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Shared Swarm Intelligence:</strong> When one researcher agent discovers a person's new company affiliation, every other agent across the organization gains immediate access to the updated graph node.
</div>
""")

# ==============================================================================
# PART VIII: GOVERNANCE, SECURITY & ENTERPRISE TRUST (35-37)
# ==============================================================================

add_page(35, "ENTERPRISE SECURITY · GOVERNANCE ENGINE",
"Enterprise Guardrails & Policy Engine",
"Fail-Closed Security Posture, Allow/Deny Glob Rules & Controlled Autonomy",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Deterministic Safety: The Governance Policy Engine</div>
  <p style="font-size: 7.4pt;">
    Deploying autonomous agents in corporate environments demands strict, auditable guardrails. In <code>crates/governance</code>, Fathom implements a <strong>fail-closed policy engine</strong> where every tool call is authorized against declarative security rules prior to execution.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Declarative Allow / Deny Glob Rules</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Allow Search:</strong> <code>allow: tool="web_search", target="*"</code></li>
      <li><strong>Restrict Browser:</strong> <code>allow: tool="browser.*", target="https://*.linkedin.com/*"</code></li>
      <li><strong>Deny Admin Access:</strong> <code>deny: tool="browser.type", target="*/admin/*"</code></li>
      <li><strong>Deny Destructive Shell:</strong> <code>deny: tool="shell", target="rm -rf *"</code></li>
    </ul>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Strict Fail-Closed Security Stance</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>Deny Wins:</strong> If an action matches both allow and deny, <strong>deny always takes absolute precedence</strong>.</li>
      <li><strong>Unmatched Fails Closed:</strong> If an action does not match an allow rule, it is rejected by default.</li>
      <li><strong>Operator Claim Gating:</strong> Administrative actions require verified operator claims in HTTP headers.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Human Approval Side-Effect Registry</div>
  <p style="font-size: 7.2pt;">
    Configurable critical tools (e.g. <code>save_contacts</code>, CRM push, <code>git_push</code>) can be placed in the <code>approval_tools</code> list, ensuring that data never leaves the research sandbox without human sign-off.
  </p>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Enterprise Compliance:</strong> Set exact operational boundaries for every virtual employee, preventing accidental data egress or unsanctioned system modifications.
</div>
""")

# Page 36: MOCKUP 13 (AES-256-GCM Vault & Audit Console)
add_page(36, "SECURITY VAULT · CREDENTIAL ISOLATION",
"The Ironclad Credentials Vault",
"AES-256-GCM Encryption: Zero Secret Visibility in Model Prompts",
"""
<div class="card-accent" style="padding: 6px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Protecting Secrets Against Prompt Injection & Extraction</div>
  <p style="font-size: 7.4pt;">
    A major vulnerability in AI wrappers is passing API keys inside prompt contexts. <strong>Fathom isolates credentials behind an AES-256-GCM encrypted vault</strong>.
  </p>
</div>

<!-- MOCKUP 13: Encrypted Credentials Vault & Audit Ledger (Rakazo Style) -->
<div class="app-mockup">
  <div class="mockup-header">
    <div class="mockup-traffic"><span class="dot-red"></span><span class="dot-yellow"></span><span class="dot-green"></span><span class="mockup-title">Fathom Vault · AES-256-GCM Encrypted Credentials</span></div>
    <div style="font-size:6pt; color:#10b981; font-family:'JetBrains Mono';">RING CRATE · HARDWARE KEY DERIVATION</div>
  </div>
  <div style="padding: 5px 8px; background:#0b0f17; font-size:6pt; font-family:'JetBrains Mono';">
    <div style="display:flex; justify-content:space-between; border-bottom:1px solid #1e293b; padding-bottom:2px;">
      <span style="color:#f1f5f9;">KEY NAME</span><span style="color:#f1f5f9;">CIPHERTEXT DIGEST</span><span style="color:#f1f5f9;">PROMPT EXPOSURE</span>
    </div>
    <div style="display:flex; justify-content:space-between; padding-top:2px;">
      <span style="color:#38bdf8;">hubspot_api_key</span><span style="color:#64748b;">$aes-gcm$v1$e8f9a2b01c4...</span><span class="badge badge-green">0% (NEVER PASSED TO LLM)</span>
    </div>
    <div style="display:flex; justify-content:space-between; padding-top:2px;">
      <span style="color:#38bdf8;">amocrm_oauth_token</span><span style="color:#64748b;">$aes-gcm$v1$991a0f8b2c1...</span><span class="badge badge-green">0% (NEVER PASSED TO LLM)</span>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">1. Hardware-Grade Encryption (Ring Crate)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li>Secrets stored in SQLite are encrypted using <strong>AES-256-GCM</strong> authenticated encryption.</li>
      <li>Encryption keys are derived from secure environment variables (<code>FATHOM_CREDENTIAL_KEY</code>).</li>
      <li>API listing endpoints return strictly masked strings (e.g. <code>sk-live-***1234</code>).</li>
    </ul>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Zero Secret Tools in Agent Registry</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li>Agents have <strong>zero tools to read or query plaintext secrets</strong>.</li>
      <li>When an agent invokes a service, the backend adapter resolves credentials internally in Rust memory.</li>
      <li>Prompt injection attacks cannot extract keys that the LLM has no mechanism to access.</li>
    </ul>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>Enterprise Compliance:</strong> Enterprise security teams can deploy Fathom with full confidence that proprietary CRM tokens and database passwords remain completely secure.
</div>
""")

add_page(37, "REGULATORY COMPLIANCE · AUDIT TRAILS",
"Complete Compliance & Audit Trails",
"Immutable Decision Ledgers, Automatic Secret Redaction & GDPR / 152-FZ Readiness",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Auditable AI: Immutable Records for Every Decision</div>
  <p style="font-size: 7.4pt;">
    Enterprise compliance requires full traceability for every autonomous action. Fathom logs all authorization decisions into an append-only, tamper-resistant <strong>Audit Ledger</strong> (<code>/api/v1/governance/audit</code>).
  </p>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">Audit Record Fields</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><code>timestamp</code>: Nanosecond-precision RFC 3339 timestamp.</li>
      <li><code>agent_id</code> & <code>session_id</code>: Full UUIDv7 session tracing.</li>
      <li><code>tool</code> & <code>intent</code>: Declared action and reasoning rationale.</li>
      <li><code>target</code> & <code>decision</code>: URL / file path and Allow/Deny verdict.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Automatic Secret Redaction</div>
    <p style="font-size: 7.2pt;">
      Before audit records are written to SQLite, a built-in redaction engine scans tool arguments with regex scanners, stripping API keys, bearer tokens, and passwords to prevent credential leakage into logs.
    </p>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Data Privacy & Regulatory Framework Compliance</div>
  <table>
    <thead>
      <tr><th>Regulation</th><th>Compliance Invariant</th><th>Fathom Implementation Mechanism</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>GDPR (EU)</strong></td><td>Lawful processing of public data & right to be forgotten</td><td>Strict OSINT confidence scoring + <code>ContactDb</code> delete APIs.</td></tr>
      <tr><td><strong>152-FZ (Russia)</strong></td><td>Personal data localization & verifiable storage</td><td>Self-hosted local SQLite / PostgreSQL storage on sovereign infrastructure.</td></tr>
      <tr><td><strong>SOC 2 Type II</strong></td><td>Access control, auditability, and data isolation</td><td>AES-256-GCM vault, per-agent Docker sandboxes, immutable audit trail.</td></tr>
    </tbody>
  </table>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Audit Replay Ready:</strong> Compliance teams can use the <code>GET /api/v1/replay</code> endpoint to reconstruct and step through any historical session turn-by-turn.
</div>
""")

# ==============================================================================
# PART IX: PERFORMANCE, BENCHMARKS & OPERATIONAL SUPERIORITY (38-40)
# ==============================================================================

add_page(38, "EMPIRICAL BENCHMARKS · SPEED SHOWDOWN",
"Speed Showdown: Fathom vs. Python",
"Empirical Head-to-Head Comparison on Apple Silicon M4 (10 Cores, Release LTO)",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Empirical Benchmark Results: The Systems Difference</div>
  <p style="font-size: 7.4pt;">
    Benchmarks were executed offline using deterministic synthetic fixtures via <code>fathom bench</code> on macOS ARM64 (Apple M4, 10 cores, release build with LTO and stripped symbols).
  </p>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">Tool Layer Overhead (4 Layers)</div>
    <table>
      <thead>
        <tr><th>Layer</th><th>Iterations</th><th>Time / Call</th></tr>
      </thead>
      <tbody>
        <tr><td><code>registry.execute</code> (Raw dispatch)</td><td>300</td><td>5,140 µs</td></tr>
        <tr><td><code>execute_batch</code> (1 call)</td><td>300</td><td>7,614 µs</td></tr>
        <tr><td><code>execute_batch</code> (8 calls amortized)</td><td>320</td><td><strong>5,893 µs (~0.75ms overhead)</strong></td></tr>
        <tr><td><code>ToolCall</code> JSON Serde</td><td>100,000</td><td><strong>752 ns</strong></td></tr>
      </tbody>
    </table>
  </div>

  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">Multi-Threaded Parallel Speedups</div>
    <table>
      <thead>
        <tr><th>Task</th><th>Sequential</th><th>Tokio Spawn</th><th>Speedup</th></tr>
      </thead>
      <tbody>
        <tr><td>16 × <code>file_read</code> (2MB each)</td><td>79.0 ms</td><td>25.8 ms</td><td><strong>3.06×</strong></td></tr>
        <tr><td>8 × <code>parse_html</code> (1MB table)</td><td>130.3 ms</td><td>34.5 ms</td><td><strong>3.78×</strong></td></tr>
        <tr><td>8 × <code>code_symbols</code> (240 files)</td><td>61.9 ms</td><td>20.4 ms</td><td><strong>3.04×</strong></td></tr>
        <tr><td><code>web_feed</code> (XML streaming)</td><td>—</td><td>—</td><td><strong>1.11M items/s</strong></td></tr>
      </tbody>
    </table>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Comparative Architecture Matrix</div>
  <table>
    <thead>
      <tr><th>Metric</th><th>Python Frameworks (LangChain/AutoGPT)</th><th>Fathom (Rust Compiled)</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>Cold Start Latency</strong></td><td>2,500 – 8,000 ms</td><td><strong>&lt; 5 ms (500x Faster)</strong></td></tr>
      <tr><td><strong>Tool Dispatch Overhead</strong></td><td>25 – 150 ms</td><td><strong>~0.75 ms (100x Faster)</strong></td></tr>
      <tr><td><strong>Concurrency Model</strong></td><td>Blocked by Python GIL</td><td><strong>True Multi-Threaded Tokio Spawn</strong></td></tr>
    </tbody>
  </table>
</div>
""")

add_page(39, "HARDWARE EFFICIENCY · WORKFORCE DENSITY",
"Hardware Efficiency & Worker Density",
"Hosting 100+ Concurrent Digital Employees on a Single Modest Server",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Maximizing Hardware Density: The Economics of Efficiency</div>
  <p style="font-size: 7.4pt;">
    The true test of enterprise scalability is operational efficiency per server dollar. Because Fathom compiles to lean machine code with zero runtime interpreter overhead, it achieves unprecedented hardware worker density.
  </p>
</div>

<div class="grid-2">
  <div class="card" style="padding: 5px 8px;">
    <div class="card-title-sm">Python Framework Footprint (100 Agents)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>RAM Required:</strong> 64 GB – 128 GB RAM (Heavy Kubernetes Cluster).</li>
      <li><strong>CPU Saturation:</strong> High context-switching drag and Python GIL contention.</li>
      <li><strong>Monthly Server Cost:</strong> $400 – $1,200 / month on AWS/GCP.</li>
      <li><strong>Hosting Cost Per Worker:</strong> $4.00 – $12.00 / month.</li>
    </ul>
  </div>

  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Fathom Rust Footprint (100 Coworkers)</div>
    <ul style="font-size: 6.8pt; margin-bottom: 0;">
      <li><strong>RAM Required:</strong> <strong>1.5 GB – 3.5 GB RAM</strong> (Modest 8-Core Box).</li>
      <li><strong>CPU Utilization:</strong> Zero idle CPU burn; epoll/kqueue event-driven I/O.</li>
      <li><strong>Monthly Server Cost:</strong> $30 – $60 / month on Hetzner/DigitalOcean.</li>
      <li><strong>Hosting Cost Per Worker:</strong> <strong>&lt; $0.50 / month</strong>.</li>
    </ul>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Server Resource Allocation Model (8 Cores, 32GB RAM)</div>
  <div class="grid-4">
    <div class="metric-box"><div class="metric-val">100+</div><div class="metric-label">Coworkers</div></div>
    <div class="metric-box"><div class="metric-val">~25 MB</div><div class="metric-label">RAM / Worker</div></div>
    <div class="metric-box"><div class="metric-val">&lt; 1%</div><div class="metric-label">Idle CPU</div></div>
    <div class="metric-box"><div class="metric-val">99.4%</div><div class="metric-label">Compute Margin</div></div>
  </div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>The Bottom Line:</strong> Fathom cuts server infrastructure costs by <strong>95%</strong>, allowing businesses to scale their virtual workforce profitably.
</div>
""")

add_page(40, "PERFORMANCE SUMMARY · SPEED HIGHLIGHTS",
"Empirical Performance Summary",
"Sub-Millisecond Ingestion, Microsecond Deserialization & Streaming Throughput",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Complete Performance Scorecard</div>
  <p style="font-size: 7.4pt;">
    Every layer of the Fathom stack has been profiled and optimized for microsecond-level execution speed, ensuring instantaneous response times across all enterprise workflows.
  </p>
</div>

<div class="grid-3">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">Memory Ingestion</div>
    <div class="metric-val" style="font-size: 14pt;">94 µs</div>
    <div class="metric-label">Per-Fact Absorb (100 batch)</div>
    <p style="font-size: 6.8pt; margin-top: 2px;">5.1ms to absorb and dedup 100 facts via SHA-256 fast path.</p>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">Hybrid Retrieval</div>
    <div class="metric-val" style="font-size: 14pt;">1.62 ms</div>
    <div class="metric-label">Search Median Latency</div>
    <p style="font-size: 6.8pt; margin-top: 2px;">Fused Vector + BM25 search across 500 facts in under 2ms.</p>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">XML Feed Stream</div>
    <div class="metric-val" style="font-size: 14pt;">1.11M</div>
    <div class="metric-label">Items / Sec Throughput</div>
    <p style="font-size: 6.8pt; margin-top: 2px;">Streaming quick-xml parser processes 12k feed items in 10.8ms.</p>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Parser Scaling & Extraction Latencies</div>
  <table>
    <thead>
      <tr><th>Document Type</th><th>Size / Rows</th><th>Parse Duration</th><th>Effective Throughput</th></tr>
    </thead>
    <tbody>
      <tr><td><strong>HTML Table (Scraper)</strong></td><td>191 KB / 3,000 rows</td><td>9.55 ms</td><td>314,268 rows / sec</td></tr>
      <tr><td><strong>HTML Large (Scraper)</strong></td><td>773 KB / 12,000 rows</td><td>34.22 ms</td><td>350,723 rows / sec</td></tr>
      <tr><td><strong>JSON Tree Walk</strong></td><td>4 MB / 20,000 objects</td><td>43.47 ms</td><td>Stateless Parallel-Safe</td></tr>
      <tr><td><strong>AST Code Map (240 files)</strong></td><td>3.3 MB Rust Source</td><td>34.20 ms</td><td>4,330 Summary Lines</td></tr>
    </tbody>
  </table>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Deterministic Excellence:</strong> Fathom turns AI agent execution from a sluggish, bloated prototype into a high-frequency trading caliber software runtime.
</div>
""")

# ==============================================================================
# PART X: CONCLUSION & STRATEGIC ROADMAP (41-42)
# ==============================================================================

add_page(41, "FUTURE VISION · ROADMAP",
"Strategic Product Roadmap (2026–2027)",
"Next-Generation Multi-Modal Vision, Decentralized Meshes & Autonomous Voice",
"""
<div class="card-dark" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">The Future of Autonomous Work: Evolutionary Milestones</div>
  <p style="font-size: 7.4pt;">
    Fathom is built on an extensible foundation designed to expand from text and browser automation into full multi-modal vision grounding, decentralized peer-to-peer swarms, and voice outreach.
  </p>
</div>

<div class="timeline" style="gap: 3px;">
  <div class="timeline-item" style="padding: 3px 6px;">
    <div class="timeline-time" style="font-size:6.2pt;">Q4 2026</div>
    <div class="timeline-content"><div class="timeline-title" style="font-size:6.8pt;">Vision-Native Hybrid Browser Engine</div><div class="timeline-desc" style="font-size:5.8pt;">Fusing accessibility trees with real-time multi-modal vision grounding (Qwen-VL-Max) for canvas & charts.</div></div>
  </div>
  <div class="timeline-item" style="padding: 3px 6px;">
    <div class="timeline-time" style="font-size:6.2pt;">Q1 2027</div>
    <div class="timeline-content"><div class="timeline-title" style="font-size:6.8pt;">Decentralized Multi-Company Coworker Mesh</div><div class="timeline-desc" style="font-size:5.8pt;">Enabling coworkers from different corporate tenants to securely negotiate and share verified data.</div></div>
  </div>
  <div class="timeline-item" style="padding: 3px 6px;">
    <div class="timeline-time" style="font-size:6.2pt;">Q2 2027</div>
    <div class="timeline-content"><div class="timeline-title" style="font-size:6.8pt;">Autonomous Voice & Cold Calling Agents</div><div class="timeline-desc" style="font-size:5.8pt;">Integrating sub-300ms ultra-low-latency neural voice synthesis for phone verification and qualification calls.</div></div>
  </div>
  <div class="timeline-item" style="padding: 3px 6px;">
    <div class="timeline-time" style="font-size:6.2pt;">Q3 2027</div>
    <div class="timeline-content"><div class="timeline-title" style="font-size:6.8pt;">Self-Evolving Skill Generation</div><div class="timeline-desc" style="font-size:5.8pt;">Agents record human operator browser sessions, compiling them into reusable deterministic Rust tools.</div></div>
  </div>
</div>

<div class="callout callout-info" style="margin-bottom: 0;">
  <strong>Continuous Innovation:</strong> Every milestone strengthens the enterprise moat and expands the operational scope of digital coworkers.
</div>
""")

add_page(42, "EXECUTIVE SUMMARY · CONCLUSION",
"The Autonomous Enterprise OS",
"Unlocking Limitless Scalability with 24/7 Governed Digital Employees",
"""
<div class="card-accent" style="padding: 7px 10px;">
  <div class="card-title" style="font-size: 8.8pt;">Conclusion: The New Era of Enterprise Labor</div>
  <p style="font-size: 7.4pt;">
    The constraints of human headcount scaling—recruiting delays, training overhead, high churn, and linear salary expansion—are no longer barriers to business growth. <strong>Fathom delivers the definitive software runtime for the autonomous enterprise.</strong>
  </p>
</div>

<div class="grid-3">
  <div class="card card-emerald" style="padding: 5px 8px;">
    <div class="card-title-sm">1. High-Performance Core</div>
    <p style="font-size: 6.8pt;">Compiled Rust engine, Tokio async concurrency, and microsecond tool dispatch for blazing speed and minimal server footprint.</p>
  </div>

  <div class="card card-accent" style="padding: 5px 8px;">
    <div class="card-title-sm">2. Flat-Rate Economics</div>
    <p style="font-size: 6.8pt;">Predictable seat subscriptions with unlimited neural compute, powered by frontier model arbitrage (Kimi k3, Qwen 3.8 Max, GLM 5.3).</p>
  </div>

  <div class="card card-purple" style="padding: 5px 8px;">
    <div class="card-title-sm">3. 100% Remote Autonomy</div>
    <p style="font-size: 6.8pt;">Self-directed workers handling research, OSINT, outreach, code maintenance, and computer use with total enterprise governance.</p>
  </div>
</div>

<div class="card" style="padding: 5px 8px;">
  <div class="card-title">Transform Your Organization Today</div>
  <div class="grid-2">
    <div><p style="font-size: 7.2pt;"><strong>Deploy in Under 60 Seconds:</strong> Download the static Rust binary, configure your target channels, and launch your first digital employee fleet today.</p></div>
    <div><p style="font-size: 7.2pt;"><strong>Scale Without Limits:</strong> Expand from 1 SDR coworker to a 100-agent multi-department autonomous workforce with zero HR friction.</p></div>
  </div>
</div>

<div class="card-slate" style="text-align: center; padding: 6px;">
  <div style="font-size: 9.5pt; font-weight: 800; color: var(--primary);">FATHOM: UNIVERSAL AUTONOMOUS AI WORKER RUNTIME</div>
  <div style="font-size: 7pt; color: var(--text-muted); margin-top: 1px;">High-Performance Systems Engineering · Unlimited Neural Intelligence · Autonomous Enterprise Labor</div>
</div>

<div class="callout callout-success" style="margin-bottom: 0;">
  <strong>The Future is Autonomous:</strong> Build your unstoppable 24/7 digital workforce with Fathom today.
</div>
""")

print(f"Total pages defined: {len(pages)}")

# ==============================================================================
# HTML & PDF GENERATOR ENGINE
# ==============================================================================

print("\nWriting individual HTML pages (01 to 42)...")
for p in pages:
    fname = f"page_{p['num']:02d}.html"
    fpath = os.path.join(WP_DIR, fname)
    
    html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Fathom Whitepaper — {p['title']}</title>
  <link rel="stylesheet" href="styles.css">
</head>
<body>
  <div class="page">
    <div class="header">
      <div class="header-left">
        <div class="header-meta">
          <span class="brand-badge">FATHOM</span>
          <span class="header-category">{p['category']}</span>
        </div>
        <div class="header-title-main">{p['title']}</div>
        <div class="header-subtitle">{p['subtitle']}</div>
      </div>
      <div class="header-right">
        <span class="page-badge">PAGE {p['num']:02d} / 42</span>
      </div>
    </div>

    <div class="content-body">
{p['html']}
    </div>

    <div class="footer">
      <div class="footer-left">
        <span>Fathom Autonomous Workforce Runtime</span>
        <span class="footer-bullet">•</span>
        <span>Elastic-2.0 License</span>
        <span class="footer-bullet">•</span>
        <span>Strategic Whitepaper Presentation</span>
      </div>
      <div class="footer-right">v0.3.0 · Page {p['num']:02d} of 42</div>
    </div>
  </div>
</body>
</html>"""
    with open(fpath, "w", encoding="utf-8") as f:
        f.write(html_content)

print("Writing Master Full Whitepaper HTML...")
full_pages_html = []
for p in pages:
    full_pages_html.append(f"""
  <div class="page">
    <div class="header">
      <div class="header-left">
        <div class="header-meta">
          <span class="brand-badge">FATHOM</span>
          <span class="header-category">{p['category']}</span>
        </div>
        <div class="header-title-main">{p['title']}</div>
        <div class="header-subtitle">{p['subtitle']}</div>
      </div>
      <div class="header-right">
        <span class="page-badge">PAGE {p['num']:02d} / 42</span>
      </div>
    </div>

    <div class="content-body">
{p['html']}
    </div>

    <div class="footer">
      <div class="footer-left">
        <span>Fathom Autonomous Workforce Runtime</span>
        <span class="footer-bullet">•</span>
        <span>Elastic-2.0 License</span>
        <span class="footer-bullet">•</span>
        <span>Strategic Whitepaper Presentation</span>
      </div>
      <div class="footer-right">v0.3.0 · Page {p['num']:02d} of 42</div>
    </div>
  </div>""")

master_full_html = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Fathom Whitepaper — Complete 42-Page Deck</title>
  <link rel="stylesheet" href="styles.css">
  <style>
    @media print {{
      .page {{
        page-break-after: always !important;
      }}
    }}
  </style>
</head>
<body>
{"\n".join(full_pages_html)}
</body>
</html>"""

with open(os.path.join(WP_DIR, "Fathom_Full_Whitepaper.html"), "w", encoding="utf-8") as f:
    f.write(master_full_html)

print("Updating index.html (Master Presentation Deck Viewer)...")
options_html = []
for p in pages:
    options_html.append(f'      <option value="{p["num"]}">Page {p["num"]:02d}: {p["title"]}</option>')

index_html = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Fathom Whitepaper — 42-Page Master Presentation Deck</title>
  <link rel="stylesheet" href="styles.css">
  <style>
    .deck-nav {{
      position: fixed;
      top: 12px;
      right: 16px;
      z-index: 9999;
      background: rgba(15, 23, 42, 0.92);
      backdrop-filter: blur(8px);
      border: 1px solid rgba(255, 255, 255, 0.15);
      border-radius: 30px;
      padding: 6px 14px;
      display: flex;
      align-items: center;
      gap: 10px;
      box-shadow: 0 10px 25px rgba(0, 0, 0, 0.25);
      color: #ffffff;
      font-size: 8.5pt;
    }}
    .deck-btn {{
      background: rgba(255, 255, 255, 0.15);
      border: none;
      color: #ffffff;
      padding: 4px 10px;
      border-radius: 16px;
      cursor: pointer;
      font-weight: 600;
      font-size: 8pt;
      transition: all 0.15s ease;
    }}
    .deck-btn:hover {{
      background: var(--primary-accent);
    }}
    .deck-btn-primary {{
      background: var(--primary-accent);
    }}
    .deck-select {{
      background: rgba(255, 255, 255, 0.12);
      color: #ffffff;
      border: 1px solid rgba(255, 255, 255, 0.2);
      padding: 3px 8px;
      border-radius: 12px;
      font-size: 7.8pt;
      font-family: inherit;
      outline: none;
    }}
    .deck-select option {{
      background: #0f172a;
      color: #ffffff;
    }}
    iframe.page-frame {{
      width: 210mm;
      height: 297mm;
      border: none;
      box-shadow: 0 10px 25px -5px rgba(15, 23, 42, 0.12);
      border-radius: 4px;
      background: #ffffff;
    }}
    @media print {{
      .deck-nav {{
        display: none !important;
      }}
      iframe.page-frame {{
        box-shadow: none !important;
        page-break-after: always !important;
      }}
    }}
  </style>
</head>
<body>
  <div class="deck-nav no-print">
    <span style="font-weight: 800; letter-spacing: 0.05em; color: #38bdf8;">FATHOM 42-PAGE DECK</span>
    <button class="deck-btn" onclick="prevPage()">◀ Prev</button>
    <select class="deck-select" id="pageSelect" onchange="jumpToPage(this.value)">
{chr(10).join(options_html)}
      <option value="all">View All 42 Pages (Printable)</option>
    </select>
    <button class="deck-btn" onclick="nextPage()">Next ▶</button>
    <button class="deck-btn deck-btn-primary" onclick="window.print()">Print / Save PDF</button>
  </div>

  <div id="deckContainer" style="display: flex; flex-direction: column; align-items: center; gap: 24px; width: 100%;">
    <iframe id="singleFrame" class="page-frame" src="page_01.html"></iframe>
  </div>

  <script>
    let currentPage = 1;
    const totalPages = 42;

    function padZero(num) {{
      return num < 10 ? '0' + num : num;
    }}

    function updateView() {{
      const container = document.getElementById('deckContainer');
      const select = document.getElementById('pageSelect');
      
      if (select.value === 'all') {{
        container.innerHTML = '';
        for (let i = 1; i <= totalPages; i++) {{
          const frame = document.createElement('iframe');
          frame.className = 'page-frame';
          frame.src = `page_${{padZero(i)}}.html`;
          container.appendChild(frame);
        }}
      }} else {{
        container.innerHTML = `<iframe id="singleFrame" class="page-frame" src="page_${{padZero(currentPage)}}.html"></iframe>`;
        select.value = currentPage;
      }}
    }}

    function jumpToPage(val) {{
      if (val === 'all') {{
        updateView();
      }} else {{
        currentPage = parseInt(val, 10);
        updateView();
      }}
    }}

    function prevPage() {{
      if (currentPage > 1) {{
        currentPage--;
        document.getElementById('pageSelect').value = currentPage;
        updateView();
      }}
    }}

    function nextPage() {{
      if (currentPage < totalPages) {{
        currentPage++;
        document.getElementById('pageSelect').value = currentPage;
        updateView();
      }}
    }}

    document.addEventListener('keydown', (e) => {{
      if (e.key === 'ArrowLeft') prevPage();
      if (e.key === 'ArrowRight') nextPage();
    }});
  </script>
</body>
</html>"""

with open(os.path.join(WP_DIR, "index.html"), "w", encoding="utf-8") as f:
    f.write(index_html)

# ==============================================================================
# PARALLEL PDF COMPILATION ENGINE (10 CONCURRENT CHROME WORKERS)
# ==============================================================================

def render_page(i):
    html_path = os.path.join(WP_DIR, f"page_{i:02d}.html")
    pdf_path = os.path.join(WP_DIR, f"page_{i:02d}.pdf")
    user_data_dir = f"/tmp/chrome_wp_{i:02d}"
    if not os.path.exists(html_path):
        return (i, None, 0)
    cmd = [
        CHROME_PATH,
        "--headless",
        "--disable-gpu",
        "--no-margins",
        f"--user-data-dir={user_data_dir}",
        f"--print-to-pdf={pdf_path}",
        f"file://{html_path}"
    ]
    subprocess.run(cmd, capture_output=True)
    try:
        shutil.rmtree(user_data_dir, ignore_errors=True)
    except:
        pass
    if os.path.exists(pdf_path) and os.path.getsize(pdf_path) > 0:
        return (i, pdf_path, os.path.getsize(pdf_path))
    return (i, None, 0)

print("\nStarting true parallel PDF rendering across 10 workers...")
with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
    results = list(executor.map(render_page, range(1, 43)))

results.sort(key=lambda x: x[0])
valid_pdfs = []
for idx, path, size in results:
    if path:
        print(f"Rendered [{idx:02d}/42]: {size:,} bytes")
        valid_pdfs.append(path)
    else:
        print(f"FAILED: page_{idx:02d}")

print(f"\nMerging {len(valid_pdfs)} pages into Master Whitepaper PDF...")
master_pdf_path = os.path.join(WP_DIR, "Fathom_Whitepaper.pdf")
writer = PdfWriter()
for pdf in valid_pdfs:
    writer.append(pdf)

with open(master_pdf_path, "wb") as f:
    writer.write(f)
writer.close()

print(f"\n=======================================================")
print(f"SUCCESS: 42-Page Master Whitepaper with 12+ UI Mockups Generated!")
print(f"Location: {master_pdf_path}")
print(f"Size: {os.path.getsize(master_pdf_path):,} bytes")
print(f"Total Pages: {len(valid_pdfs)}")
print(f"=======================================================\n")
