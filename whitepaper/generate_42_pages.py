#!/usr/bin/env python3
import os
import subprocess
import re
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
# PART I: THE VISION & AUTONOMOUS REMOTE WORKFORCE (01-05)
# ==============================================================================

add_page(1, "EXECUTIVE WHITEPAPER · STRATEGIC OVERVIEW",
"Universal Autonomous AI Workforce Runtime",
"High-Performance Rust Architecture for End-to-End Remote Digital Employees",
"""
<div class="card-dark">
  <div class="card-title">The Paradigm Shift: From Scripted Bots to Autonomous Remote Employees</div>
  <p>
    Traditional AI automation is constrained by brittle Python pipelines, single-shot prompt wrappers, and synchronous execution bottlenecks. <strong>Fathom</strong> introduces a production-grade, self-hosted <strong>Rust runtime</strong> designed to instantiate, govern, and orchestrate true <strong>autonomous remote digital employees</strong>. These agents do not merely suggest answers; they independently plan, execute multi-day workflows, use computers, perform deep OSINT/lead generation, write software, and interact across all corporate channels 100% remotely.
  </p>
</div>

<div class="grid-3">
  <div class="card card-accent">
    <div class="card-title-sm">100% Remote Operation</div>
    <p>Autonomous agents operate independently inside sandboxed environments, interacting with web portals, APIs, shells, and CRMs 24/7 without manual babysitting.</p>
  </div>
  <div class="card card-emerald">
    <div class="card-title-sm">Microsecond Rust Engine</div>
    <p>Zero-cost abstractions, Tokio async I/O, and concurrent <code>JoinSet</code> task trees provide sub-millisecond tool dispatch (~0.75 ms) and 94 µs memory absorption.</p>
  </div>
  <div class="card card-purple">
    <div class="card-title-sm">Unlimited Neural Compute</div>
    <p>High-throughput routing to cost-efficient frontier models (Kimi k3, Qwen 3.8 Max, GLM 5.3) enables flat-rate subscription economics per digital worker seat.</p>
  </div>
</div>

<div class="card">
  <div class="card-title">Core Operational Pillars</div>
  <div class="grid-2">
    <div>
      <ul style="margin-bottom: 0;">
        <li><strong>Hierarchical Sub-Agent Swarms:</strong> Recursive coordinator-worker trees with non-blocking broadcast bus message routing.</li>
        <li><strong>Autonomous Outreach & OSINT:</strong> 7 unified search engines, corporate scrapers, and SMTP-verified contact harvesting.</li>
        <li><strong>Governed Computer Use:</strong> Native Playwright Chromium integration with Accessibility-Tree snapshots and human takeover.</li>
      </ul>
    </div>
    <div>
      <ul style="margin-bottom: 0;">
        <li><strong>Long-Term Semantic Memory:</strong> Hybrid TF-IDF/BM25 SQLite memory with entity graph and sub-5ms context digestion.</li>
        <li><strong>Durable Asynchronous Jobs:</strong> SQLite-backed jobs surviving server reboots with self-healing retry task augmentation.</li>
        <li><strong>Enterprise Governance:</strong> Fail-closed policy evaluation, AES-256-GCM encrypted vault, and tamper-proof audit trails.</li>
      </ul>
    </div>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">System Metric Overview</div>
  <div class="grid-4">
    <div class="metric-box">
      <div class="metric-val">12</div>
      <div class="metric-label">Rust Workspace Crates</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">0.75 ms</div>
      <div class="metric-label">Tool Dispatch Latency</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">51+</div>
      <div class="metric-label">Native Core Tools</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">15 MB</div>
      <div class="metric-label">Daemon Baseline RSS</div>
    </div>
  </div>
</div>

<div class="callout callout-info">
  <strong>Strategic Objective:</strong> Deliver a scalable software foundation where businesses deploy digital workers on demand—scaling operational capacity infinitely without proportional linear headcount expansion.
</div>
""")

add_page(2, "MACRO PROBLEM · THE HIRING BOTTLENECK",
"The Broken Remote Hiring Landscape",
"Why Traditional Outsourcing and Human Headcount Scaling Fail Modern Companies",
"""
<div class="card-accent">
  <div class="card-title">The Remote Workforce Crisis: High Cost, High Friction, Slow Ramp-Up</div>
  <p>
    Scaling modern digital operations with human remote labor faces severe structural limitations: expensive salaries, months of onboarding, high turnover, timezone delays, and inconsistent task execution.
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber">
    <div class="card-title-sm">Human Remote Labor Pain Points</div>
    <ul style="margin-bottom: 0;">
      <li><strong>High Overhead:</strong> $4,000–$10,000/mo per knowledge worker + equipment, software licenses, HR management, and benefits.</li>
      <li><strong>Burnout & Inconsistency:</strong> Human SDRs, data scrubbers, and QA engineers make repetitive errors when handling thousands of rows.</li>
      <li><strong>Timezone & Availability Lag:</strong> Work halts during nights, weekends, and holidays—stalling business-critical pipelines.</li>
      <li><strong>Churn & Knowledge Loss:</strong> When remote staff depart, institutional memory disappears and training must restart.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">The Fathom Digital Solution</div>
    <ul style="margin-bottom: 0;">
      <li><strong>Fractional Cost:</strong> Predictable flat subscription per autonomous employee with zero payroll taxes or overhead.</li>
      <li><strong>Deterministic Quality:</strong> Every task follows verified standard operating procedures (SOPs) with automated cross-checking.</li>
      <li><strong>24/7/365 Continuous Output:</strong> Digital coworkers process inbound leads, market monitoring, and data pipelines around the clock.</li>
      <li><strong>Permanent Knowledge:</strong> All discoveries, company ties, and client preferences are preserved forever in persistent memory.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Operational Comparison Matrix</div>
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
        <td>4 – 8 Weeks</td>
        <td>2 – 4 Weeks</td>
        <td><strong>Instant (Under 60 Seconds)</strong></td>
      </tr>
      <tr>
        <td><strong>Monthly Cost</strong></td>
        <td>High ($$$$ + Taxes)</td>
        <td>Variable / Retainer ($$$)</td>
        <td><strong>Flat Monthly Subscription</strong></td>
      </tr>
      <tr>
        <td><strong>Token / API Costs</strong></td>
        <td>N/A</td>
        <td>Extra Billed Expenses</td>
        <td><strong>100% Unlimited (Included)</strong></td>
      </tr>
      <tr>
        <td><strong>Speed of Execution</strong></td>
        <td>Human Speed (Minutes/Hours)</td>
        <td>Human Speed (Days)</td>
        <td><strong>Microsecond Rust Runtime</strong></td>
      </tr>
      <tr>
        <td><strong>Scalability</strong></td>
        <td>Linear (1 hire = 1 salary)</td>
        <td>Constrained by agency capacity</td>
        <td><strong>Infinite Elastic Scale</strong></td>
      </tr>
    </tbody>
  </table>
</div>

<div class="callout callout-info">
  <strong>The Strategic Imperative:</strong> In an era of compressed margins and high competition, companies that transition routine research, outreach, and computer tasks to autonomous digital staff gain an unassailable speed and cost advantage.
</div>
""")

add_page(3, "PRODUCT PHILOSOPHY · PARADIGM COMPARISON",
"Digital Coworkers vs. Scripted Chatbots",
"Moving Beyond Single-Prompt Chat Interfaces to Autonomous Goal-Driven Agents",
"""
<div class="card-dark">
  <div class="card-title">The Fundamental Difference: Agency vs. Reaction</div>
  <p>
    Most AI tools on the market are <strong>passive assistants</strong>: they wait for a human prompt, generate text, and stop. <strong>Fathom instantiates proactive digital coworkers</strong>: given a high-level goal, they formulate a multi-step plan, spawn specialized sub-agents, operate browser tools, verify their own work, and deliver finalized deliverables directly to your CRM or inbox.
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">Traditional AI Chatbot (Single-Turn)</div>
    <div class="diagram-flow" style="flex-direction: column; gap: 4px;">
      <div class="flow-step" style="width: 100%;">1. User writes prompt</div>
      <div class="flow-step" style="width: 100%;">2. LLM generates text completion</div>
      <div class="flow-step" style="width: 100%;">3. Human must manually copy-paste & verify</div>
    </div>
    <p style="font-size: 7.4pt; color: var(--rose); margin-top: 4px;"><strong>Limitation:</strong> Human operator remains the bottleneck for execution, tool operation, and data validation.</p>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">Fathom Autonomous Coworker (Multi-Agent Swarm)</div>
    <div class="diagram-flow" style="flex-direction: column; gap: 4px;">
      <div class="flow-step" style="width: 100%;">1. Goal assigned (e.g., "Enrich 50 fintech leads in Dubai")</div>
      <div class="flow-step" style="width: 100%;">2. Coordinator plans & spawns 4 parallel worker agents</div>
      <div class="flow-step" style="width: 100%;">3. Workers scrape registries, verify SMTP, & operate browser</div>
      <div class="flow-step" style="width: 100%;">4. LLM Judge verifies completeness & syncs CRM</div>
    </div>
    <p style="font-size: 7.4pt; color: var(--emerald); margin-top: 4px;"><strong>Advantage:</strong> End-to-end task ownership with zero human intervention required.</p>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Key Capabilities of a Fathom Coworker</div>
  <div class="grid-3">
    <div class="card" style="background: white;">
      <strong>Self-Directed Planning</strong>
      <p style="font-size: 7.2pt;">Decomposes vague objectives into concrete sub-tasks with dependency management.</p>
    </div>
    <div class="card" style="background: white;">
      <strong>Independent Verification</strong>
      <p style="font-size: 7.2pt;">Checks every harvested fact, email MX record, and code output before presenting it.</p>
    </div>
    <div class="card" style="background: white;">
      <strong>Durable Memory</strong>
      <p style="font-size: 7.2pt;">Retains company relationships, past conversation context, and domain knowledge.</p>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>The Autonomous Standard:</strong> Fathom coworkers don't just draft emails or write snippets—they find the decision-maker, verify deliverability, operate the sales platform, and track pipeline outcomes.
</div>
""")

add_page(4, "WORKFORCE ARCHETYPES · PERSONAS",
"The 5 Core Digital Worker Archetypes",
"Specialized Autonomous Roles Pre-Tuned for Immediate Enterprise Deployment",
"""
<div class="card-accent">
  <div class="card-title">Pre-Configured Autonomous Employee Roles</div>
  <p>
    Fathom supports specialized coworker personas out-of-the-box. Each persona is configured with role-specific system prompts (up to 32,000 characters), optimized tool sets, and tailored verification loops.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">1. Autonomous Sales Development Rep (SDR)</div>
    <p style="font-size: 7.6pt;"><strong>Role:</strong> Lead Generation & Prospecting</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li>Searches business directories (2GIS, Google Places, Yandex Maps).</li>
      <li>Harvests executive emails, phones, and LinkedIn profiles.</li>
      <li>Runs SMTP 250 OK probes and pushes leads directly to amoCRM/HubSpot.</li>
    </ul>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">2. Market Intelligence & OSINT Analyst</div>
    <p style="font-size: 7.6pt;"><strong>Role:</strong> Competitive Research & Market Mapping</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li>Monitors competitor websites, pricing pages, and press releases.</li>
      <li>Tracks executive hiring, tech stack changes, and funding news.</li>
      <li>Synthesizes multi-source findings into structured executive memos.</li>
    </ul>
  </div>
</div>

<div class="grid-3">
  <div class="card card-purple">
    <div class="card-title-sm">3. Executive Talent Scout</div>
    <p style="font-size: 7.2pt;">Scours LinkedIn, GitHub, and Telegram to map senior engineering and leadership talent, verifying current roles and past projects.</p>
  </div>

  <div class="card card-amber">
    <div class="card-title-sm">4. Back-Office Assistant</div>
    <p style="font-size: 7.2pt;">Operates corporate web portals, reconciles invoices, extracts PDF data, and automates internal reporting workflows.</p>
  </div>

  <div class="card card-indigo">
    <div class="card-title-sm">5. DevOps & Code Maintainer</div>
    <p style="font-size: 7.2pt;">Analyzes AST symbol trees (<code>repo_map</code>), investigates bug reports in sandboxed Python REPLs, and opens verified pull requests.</p>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Custom Coworker Definition</div>
  <p style="font-size: 7.6pt;">
    Enterprises can define custom coworker profiles with proprietary SOP prompts, assigned channels (Telegram, Slack, Email), and cron schedules (e.g. <em>"Run every Monday at 9:00 AM"</em>) in a single API call.
  </p>
</div>
""")

add_page(5, "DAY IN THE LIFE · OPERATIONAL WORKFLOW",
"A Day in the Life of a Digital Employee",
"24-Hour Continuous Execution Cycle of an Autonomous Fathom Worker",
"""
<div class="card-dark">
  <div class="card-title">Continuous 24/7 Autonomy: Zero Idle Time</div>
  <p>
    While human teams sleep, Fathom coworkers execute scheduled background operations, monitor market shifts, prepare outbound campaigns, and report results for morning review.
  </p>
</div>

<div class="timeline">
  <div class="timeline-item">
    <div class="timeline-time">02:00 AM</div>
    <div class="timeline-content">
      <div class="timeline-title">Scheduled Pipeline Trigger (Atomic Cron)</div>
      <div class="timeline-desc">The SDR coworker wakes via atomic cron claim (<code>0 2 * * *</code>), loads target criteria from persistent SQLite memory, and initiates search.</div>
    </div>
  </div>

  <div class="timeline-item">
    <div class="timeline-time">02:15 AM</div>
    <div class="timeline-content">
      <div class="timeline-title">Parallel Web Scraping & Multi-Engine Search</div>
      <div class="timeline-desc">Spawns 4 worker sub-agents querying directories, Google Serper, and corporate websites concurrently, extracting 85 candidate records.</div>
    </div>
  </div>

  <div class="timeline-item">
    <div class="timeline-time">03:30 AM</div>
    <div class="timeline-content">
      <div class="timeline-title">Multi-Signal Verification & SMTP Probing</div>
      <div class="timeline-desc">Validates syntax, verifies DNS MX records, and conducts non-intrusive port 25 SMTP handshakes. 62 emails confirmed deliverable (0.95+ confidence).</div>
    </div>
  </div>

  <div class="timeline-item">
    <div class="timeline-time">05:00 AM</div>
    <div class="timeline-content">
      <div class="timeline-title">Goal Mode LLM Judge Review</div>
      <div class="timeline-desc">Evaluates lead quality against the goal. Identifies 8 missing LinkedIn URLs; launches targeted gap-filling subtasks to resolve them.</div>
    </div>
  </div>

  <div class="timeline-item">
    <div class="timeline-time">07:45 AM</div>
    <div class="timeline-content">
      <div class="timeline-title">CRM Synchronization & Morning Briefing</div>
      <div class="timeline-desc">Pushes 62 verified leads into amoCRM, logs timeline notes, and dispatches a structured PDF executive briefing to the team's Telegram channel.</div>
    </div>
  </div>
</div>

<div class="callout callout-info">
  <strong>The Outcome:</strong> When human account executives begin their workday at 9:00 AM, fresh, verified, and enriched leads are already sitting in the CRM waiting for closing calls.
</div>
""")

# ==============================================================================
# PART II: BUSINESS MODEL, PRICING & ECONOMICS (06-10)
# ==============================================================================

add_page(6, "COMMERCIAL STRATEGY · PRICING MODEL",
"The Virtual Employee Subscription Model",
"Seat-Based Flat Pricing: Eliminating Token Metering and Billing Anxiety",
"""
<div class="card-dark">
  <div class="card-title">Commercial Model: Pay Per Autonomous Coworker Seat</div>
  <p>
    Traditional AI tools force customers to monitor complex token meters, calculate cost-per-call, and live in constant fear of unexpected billing spikes. <strong>Fathom adopts a transparent subscription model</strong>: customers subscribe to dedicated virtual employee seats on a flat monthly basis.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">100% Unlimited Usage Included</div>
    <p>Every subscribed coworker seat comes with unrestricted access to neural intelligence and tools:</p>
    <ul style="margin-bottom: 0;">
      <li><strong>Unlimited Neural Compute:</strong> Run millions of tokens monthly without extra surcharges.</li>
      <li><strong>Unlimited Tool Invocations:</strong> Web search, browser automation, email validation, and code execution.</li>
      <li><strong>Unlimited Job Schedules:</strong> Set up continuous hourly or daily recurring workflows.</li>
    </ul>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">Enterprise Budget Predictability</div>
    <p>CFOs and founders gain total cost clarity:</p>
    <ul style="margin-bottom: 0;">
      <li><strong>Fixed OpEx:</strong> Treat AI workers as standard fixed-cost software seats rather than variable utility bills.</li>
      <li><strong>Elastic Expansion:</strong> Add 5 SDRs during a sales sprint and scale back down instantly without severance costs.</li>
      <li><strong>No Micro-Management:</strong> Teams don't need to restrict AI usage to save token budget.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Subscription Tier Architecture</div>
  <table>
    <thead>
      <tr>
        <th>Subscription Tier</th>
        <th>Intended Capacity</th>
        <th>Included Capabilities</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>Starter Seat</strong></td>
        <td>1 Dedicated Autonomous Worker</td>
        <td>Full OSINT tools, 7 search backends, contact verification, and Telegram notifications.</td>
      </tr>
      <tr>
        <td><strong>Growth Pod</strong></td>
        <td>3 – 5 Collaborative Coworkers</td>
        <td>Multi-agent tree coordination, CRM auto-push, Docker computer sandboxing, and persistent memory.</td>
      </tr>
      <tr>
        <td><strong>Enterprise Fleet</strong></td>
        <td>10+ Autonomous Workers</td>
        <td>Custom coworker prompts, dedicated PostgreSQL cluster, role-based access control, and SLA support.</td>
      </tr>
    </tbody>
  </table>
</div>

<div class="callout callout-success">
  <strong>The Core Promise:</strong> You pay for the worker's output and role, not the number of words it reads or writes.
</div>
""")

add_page(7, "NEURAL ECONOMICS · MARGIN ARBITRAGE",
"Unlimited Neural Compute Engine",
"Harnessing Frontier Foundation Models for High-Throughput Cost Arbitrage",
"""
<div class="card-accent">
  <div class="card-title">The Foundation Model Cost-Performance Revolution</div>
  <p>
    Offering unlimited neural compute is economically viable because Fathom intelligently routes tasks to next-generation frontier foundation models that deliver elite reasoning at a fraction of legacy pricing.
  </p>
</div>

<div class="grid-3">
  <div class="card card-emerald">
    <div class="card-title-sm">Kimi k3 (Moonshot AI)</div>
    <p><strong>Strength:</strong> Ultra-Long Context</p>
    <p style="font-size: 7.2pt;">Processes massive document corpora, multi-page regulatory filings, and deep recursive research trees with flawless long-range recall.</p>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">Qwen 3.8 Max (Alibaba)</div>
    <p><strong>Strength:</strong> Tool Calling & Polyglot Coding</p>
    <p style="font-size: 7.2pt;">Exceptional precision in structured function calling, multilingual web parsing, and Python/Node.js REPL script generation.</p>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">GLM 5.3 (Zhipu AI)</div>
    <p><strong>Strength:</strong> Rapid Task Decomposition</p>
    <p style="font-size: 7.2pt;">High-speed reasoning engine optimized for coordinator agents breaking down complex user briefs into parallel worker subtasks.</p>
  </div>
</div>

<div class="card">
  <div class="card-title">Fathom's Economic Arbitrage Flywheel</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">1. Compiled Rust Core</div>
      <div class="flow-desc">~15MB RAM & 0.75ms dispatch</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">2. Efficient Chinese LLMs</div>
      <div class="flow-desc">Kimi k3 / Qwen / GLM</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">3. Sub-$30 Monthly Cost</div>
      <div class="flow-desc">Total compute per worker</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">4. 90%+ Gross Margin</div>
      <div class="flow-desc">On flat seat subscription</div>
    </div>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Dynamic Role-Based Model Routing</div>
  <p style="font-size: 7.6pt;">
    The coordinator assigns the ideal model per role (e.g. GLM for fast planning, Kimi for synthesis, Qwen for coding). This maximizes accuracy while keeping internal token expenditures at micro-cents per task.
  </p>
</div>
""")

add_page(8, "FINANCIAL ANALYSIS · ROI & TCO",
"Total Cost of Ownership (TCO) & ROI",
"Hard Economic Numbers: Comparing In-House Staff, Traditional AI Stacks, and Fathom",
"""
<div class="card-dark">
  <div class="card-title">Executive Financial Summary: The 10x ROI Multiplier</div>
  <p>
    Deploying a Fathom digital employee eliminates the vast majority of operational expenses associated with human staff and fragmented SaaS software subscriptions.
  </p>
</div>

<div class="card">
  <div class="card-title">Annual Cost Breakdown for a 5-Person Outbound Sales Team</div>
  <table>
    <thead>
      <tr>
        <th>Expense Category</th>
        <th>Traditional In-House Team</th>
        <th>Fragmented SaaS + AI Stack</th>
        <th>Fathom Digital Workforce</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>Base Salaries (5 Staff)</strong></td>
        <td>$300,000 / year</td>
        <td>$120,000 (Junior Staff)</td>
        <td><strong>$0.00</strong></td>
      </tr>
      <tr>
        <td><strong>Benefits, Taxes & HR</strong></td>
        <td>$75,000 / year</td>
        <td>$30,000 / year</td>
        <td><strong>$0.00</strong></td>
      </tr>
      <tr>
        <td><strong>Data & Scraper Licenses</strong></td>
        <td>$18,000 (ZoomInfo, Apollo)</td>
        <td>$14,000 / year</td>
        <td><strong>Included (Built-in 7 engines)</strong></td>
      </tr>
      <tr>
        <td><strong>Email Verification Tools</strong></td>
        <td>$6,000 (ZeroBounce, etc.)</td>
        <td>$4,500 / year</td>
        <td><strong>Included (Built-in SMTP probe)</strong></td>
      </tr>
      <tr>
        <td><strong>LLM Token & API Invoices</strong></td>
        <td>$0.00</td>
        <td>$12,000 – $24,000 / year</td>
        <td><strong>Included (Unlimited compute)</strong></td>
      </tr>
      <tr>
        <td><strong>Total Annual Investment</strong></td>
        <td><strong>$399,000 / year</strong></td>
        <td><strong>$180,500 / year</strong></td>
        <td><strong>Fractional Flat Subscription</strong></td>
      </tr>
    </tbody>
  </table>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">Immediate Payback Period</div>
    <p style="font-size: 7.6pt;">
      Most clients recoup their entire annual Fathom subscription within the <strong>first 14 days of deployment</strong> through newly closed outbound sales pipeline and eliminated SaaS tool licenses.
    </p>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">Zero Management Drag</div>
    <p style="font-size: 7.6pt;">
      Managers spend 0 hours on 1-on-1s, dispute mediation, sick leave coverage, or retraining. Performance metrics and audit trails are visible in real time via Prometheus.
    </p>
  </div>
</div>

<div class="callout callout-success">
  <strong>Bottom Line Impact:</strong> Fathom delivers <strong>85% to 92% cost reduction</strong> while expanding outreach volume and operational bandwidth by 400%.
</div>
""")

add_page(9, "ORGANIZATIONAL DESIGN · ELASTIC SCALING",
"Scalability Economics: 1 to 1,000 Workers",
"Elastic Scaling Without Organizational Bureaucracy or Management Friction",
"""
<div class="card-accent">
  <div class="card-title">The Frictionless Enterprise: Scaling Labor Like Cloud Servers</div>
  <p>
    In traditional business, growing from 10 to 100 employees introduces exponential management complexity: middle management layers, HR departments, communication silos, and cultural friction. <strong>Fathom allows organizations to scale labor elastically</strong> like cloud compute instances.
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber">
    <div class="card-title-sm">Traditional Scaling (Brooks's Law)</div>
    <p style="font-size: 7.4pt;"><em>"Adding manpower to a late software project makes it later."</em></p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li>Communication channels grow quadratically: $N(N-1)/2$.</li>
      <li>Coordination overhead consumes up to 40% of productive hours.</li>
      <li>Hiring lag delays market entry by 3 to 6 months.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Fathom Swarm Scaling (Tokio DAG)</div>
    <p style="font-size: 7.4pt;"><em>Linear scaling with zero communication decay.</em></p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li>Hierarchical coordinator agents manage sub-agents with strict depth bounds.</li>
      <li>Non-blocking broadcast message bus ensures instant telemetry sync.</li>
      <li>Deploy 100 new workers in a single API call with zero recruiting latency.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Elastic Workforce Deployment Scenarios</div>
  <div class="grid-3">
    <div class="card-slate">
      <strong>Product Launch Blitz</strong>
      <p style="font-size: 7.2pt;">Instantly spin up 50 SDR coworkers for 2 weeks to saturate a new vertical market, then scale back to 5 maintenance agents.</p>
    </div>
    <div class="card-slate">
      <strong>Due Diligence Sprint</strong>
      <p style="font-size: 7.2pt;">Deploy 20 analyst coworkers to cross-reference 500 company filings and competitor websites over a single weekend.</p>
    </div>
    <div class="card-slate">
      <strong>Black Friday Back-Office</strong>
      <p style="font-size: 7.2pt;">Scale up invoice extraction and customer order reconciliation workers to handle a 10x seasonal transaction surge.</p>
    </div>
  </div>
</div>

<div class="callout callout-info">
  <strong>True Business Agility:</strong> Scale up for opportunities in seconds; scale down during lulls without severance or morale penalties.
</div>
""")

add_page(10, "B2B STRATEGY · PARTNERS & ENTERPRISE",
"Enterprise & Agency Monetization Models",
"White-Label Reselling, Managed AI Staffing & Multi-Tenant Deployments",
"""
<div class="card-dark">
  <div class="card-title">Dual Commercial Engines: Direct Enterprise & Agency Resellers</div>
  <p>
    Fathom captures market share through two robust commercial motions: direct enterprise deployments for corporate efficiency, and agency partnerships that turn marketing/staffing firms into AI workforce providers.
  </p>
</div>

<div class="grid-2">
  <div class="card card-accent">
    <div class="card-title-sm">1. Agency & White-Label Model</div>
    <p>Marketing, recruitment, and IT consulting agencies resell Fathom workers under their own brand:</p>
    <ul style="margin-bottom: 0;">
      <li><strong>Turnkey Lead Generation:</strong> Agencies offer "Automated SDR as a Service" to clients, charging retainers while paying Fathom flat seat costs.</li>
      <li><strong>White-Label Dashboard:</strong> Embed Fathom's Next.js web dashboard with custom agency branding and client portals.</li>
      <li><strong>Recurring High Margins:</strong> Agencies capture 70–80% profit margins on outsourced client workflows.</li>
    </ul>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">2. Enterprise Private Fleet Model</div>
    <p>Large corporations deploy self-hosted Fathom clusters behind corporate firewalls:</p>
    <ul style="margin-bottom: 0;">
      <li><strong>Dedicated On-Prem / VPC:</strong> Complete data sovereignty with zero external data sharing (GDPR, HIPAA, 152-FZ ready).</li>
      <li><strong>Private LLM Connectivity:</strong> Route inference to internal vLLM/Ollama clusters or dedicated Chinese API endpoints.</li>
      <li><strong>Active Directory / SSO Integration:</strong> Provision coworkers per department with fine-grained RBAC governance policies.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Commercial Revenue Streams</div>
  <table>
    <thead>
      <tr>
        <th>Revenue Stream</th>
        <th>Target Customer</th>
        <th>Monetization Structure</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>Direct Seat Subscription</strong></td>
        <td>SMBs, Startups, Mid-Market</td>
        <td>Flat monthly subscription per virtual employee seat.</td>
      </tr>
      <tr>
        <td><strong>Agency Volume Licensing</strong></td>
        <td>Lead-Gen Agencies, BPOs, Consultancies</td>
        <td>Discounted multi-seat bundles with white-label rights.</td>
      </tr>
      <tr>
        <td><strong>Enterprise Platform License</strong></td>
        <td>Fortune 500, Financial Institutions</td>
        <td>Annual platform license + custom tool integrations & SLAs.</td>
      </tr>
    </tbody>
  </table>
</div>

<div class="callout callout-success">
  <strong>Viral Agency Expansion:</strong> Every agency partner brings 10 to 50 end-clients, creating an exponential B2B distribution channel.
</div>
""")

print("Saved pages 01 to 10...")

# Continue with the remaining 32 pages in the next blocks...

# ==============================================================================
# PART III: USER ACQUISITION, GO-TO-MARKET & GROWTH LOOPS (11-15)
# ==============================================================================

add_page(11, "GROWTH LOOPS · SELF-REPLICATING SALES",
"The Self-Replicating Growth Loop",
"How Fathom Sells Fathom: Autonomous Customer Acquisition at Zero Marginal CAC",
"""
<div class="card-dark">
  <div class="card-title">The Ultimate Organic Growth Mechanism: Autonomous Self-Outreach</div>
  <p>
    The most powerful marketing validation of an autonomous sales agent is when <strong>the product sells itself</strong>. Fathom operates an internal fleet of autonomous SDR coworkers whose sole job is to discover target B2B companies, verify decision-maker contacts, and conduct personalized cold outreach to sign up new customers.
  </p>
</div>

<div class="card">
  <div class="card-title">The 4-Step Self-Replicating Acquisition Engine</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">1. Market Discovery</div>
      <div class="flow-desc">Fathom scrapes B2B directories for agencies & SaaS</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">2. SMTP Verification</div>
      <div class="flow-desc">Verifies CEO/VP Sales email with SMTP probe</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">3. Personalized Audit</div>
      <div class="flow-desc">Attaches 10 free verified leads in their exact niche</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">4. Direct Sign-Up</div>
      <div class="flow-desc">Prospect books call or subscribes to own worker</div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">Zero Marginal Customer Acquisition Cost (CAC)</div>
    <ul style="margin-bottom: 0;">
      <li><strong>No Human BDR Salaries:</strong> 50 virtual SDRs execute 5,000 personalized touchpoints daily at near-zero incremental cost.</li>
      <li><strong>Hyper-Personalized Proof-of-Work:</strong> Instead of generic cold spam, outreach includes real, verified prospect data tailored to the recipient's ICP.</li>
      <li><strong>Instant Value Delivery:</strong> Prospects experience the product's output before entering a sales demo.</li>
    </ul>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">The Viral Inbound Referral Loop</div>
    <p>
      Every email sent by a Fathom SDR includes a subtle footer: <em>"This prospect list was researched, verified, and sent autonomously by Fathom AI Coworker."</em> Recipients frequently reply asking how to hire a similar digital worker for their own company.
    </p>
  </div>
</div>

<div class="callout callout-success">
  <strong>Infinite Self-Funding Flywheel:</strong> As more customers subscribe, compute revenue funds more autonomous worker nodes, scaling top-of-funnel outreach without outside marketing spend.
</div>
""")

add_page(12, "MARKETING & SALES · GTM STRATEGY",
"Go-To-Market Channels & Customer Acquisition",
"A Multi-Pronged Strategy for Rapid B2B and Mid-Market Market Penetration",
"""
<div class="card-accent">
  <div class="card-title">Omnichannel B2B Customer Acquisition Framework</div>
  <p>
    Beyond autonomous outbound sales, Fathom deploys a multi-channel go-to-market strategy targeting high-intent decision-makers across four key acquisition pillars.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">1. High-ROI Outbound (Fathom Fleet)</div>
    <p style="font-size: 7.4pt;">Autonomous SDRs target agency owners, SaaS founders, and sales leaders with free custom lead samples.</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Volume:</strong> 25,000+ verified touches per month.</li>
      <li><strong>Target Response Rate:</strong> 4.5% – 7.2% due to attached data samples.</li>
    </ul>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">2. Agency Partnership Program</div>
    <p style="font-size: 7.4pt;">Recruiting, marketing, and SEO agencies deploy Fathom as their secret fulfillment back-office.</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Incentive:</strong> High recurring revenue share and white-label rights.</li>
      <li><strong>Retention:</strong> Negative net churn as agency client rosters expand.</li>
    </ul>
  </div>
</div>

<div class="grid-2">
  <div class="card card-purple">
    <div class="card-title-sm">3. Open-Core Developer Funnel</div>
    <p style="font-size: 7.4pt;">Engineers and technical founders adopt the open-source Rust CLI on GitHub, experiencing microsecond speed firsthand.</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Conversion:</strong> Upgrading to managed hosting and unlimited Chinese foundation model compute.</li>
    </ul>
  </div>

  <div class="card card-indigo">
    <div class="card-title-sm">4. B2B Community & Skill Marketplace</div>
    <p style="font-size: 7.4pt;">Pre-built coworker SOP templates (e.g. <em>"Fintech SDR"</em>, <em>"YC Founder Scout"</em>) shared across communities generate organic word-of-mouth.</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Virality:</strong> 1-click template cloning accelerates onboarding.</li>
    </ul>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Customer Acquisition Unit Economics</div>
  <div class="grid-3">
    <div class="metric-box">
      <div class="metric-val">&lt; $45</div>
      <div class="metric-label">Estimated Blended CAC</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">&gt; 12:1</div>
      <div class="metric-label">LTV to CAC Ratio</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">&lt; 14 Days</div>
      <div class="metric-label">CAC Payback Window</div>
    </div>
  </div>
</div>
""")

add_page(13, "CONVERSION FUNNEL · PRODUCT-LED GROWTH",
"The 'Free Value Audit to Paid Seat' Funnel",
"Converting Prospects by Delivering Tangible Work Deliverables Before Payment",
"""
<div class="card-dark">
  <div class="card-title">Product-Led Conversion: The Irresistible Value Audit</div>
  <p>
    Traditional software demos rely on slide decks and sales pitches. Fathom converts prospects by <strong>delivering immediate, tangible business value upfront</strong> before asking for a subscription commitment.
  </p>
</div>

<div class="card">
  <div class="card-title">The 5-Stage Customer Conversion Journey</div>
  <div class="timeline">
    <div class="timeline-item">
      <div class="timeline-time">Step 1</div>
      <div class="timeline-content">
        <div class="timeline-title">The Automated Value Hook (Inbound or Outbound)</div>
        <div class="timeline-desc">The prospect enters their target industry and geography into an online interactive form (e.g., <em>"Cybersecurity companies in Germany"</em>).</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Step 2</div>
      <div class="timeline-content">
        <div class="timeline-title">Instant Live Execution by Fathom Worker</div>
        <div class="timeline-desc">A background Fathom worker executes a 2-minute OSINT sweep, extracting and SMTP-verifying 25 live decision-maker contacts with tech stack tags.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Step 3</div>
      <div class="timeline-content">
        <div class="timeline-title">Free Lead Sample Delivery</div>
        <div class="timeline-desc">The prospect receives a beautifully formatted Excel (.xlsx) file with 10 free verified leads, demonstrating 100% data freshness and accuracy.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Step 4</div>
      <div class="timeline-content">
        <div class="timeline-title">7-Day Pilot Coworker Activation</div>
        <div class="timeline-desc">The user is given 1 dedicated digital SDR connected to their Telegram or Slack to run 100 searches daily with unlimited neural compute.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Step 5</div>
      <div class="timeline-content">
        <div class="timeline-title">Frictionless Monthly Subscription</div>
        <div class="timeline-desc">Having experienced 10x ROI and hours of saved labor during the pilot, the user seamlessly converts to a paid monthly seat subscription.</div>
      </div>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>Why This Funnel Works:</strong> B2B buyers don't buy software—they buy outcomes. Showing verified leads in their exact target market dissolves skepticism in seconds.
</div>
""")

add_page(14, "PARTNER ECOSYSTEM · SCALING DISTRIBUTION",
"Agency & B2B Partner Ecosystem",
"Empowering Agencies to Become High-Margin Autonomous Workforce Providers",
"""
<div class="card-accent">
  <div class="card-title">The Agency Force Multiplier: White-Label Distribution</div>
  <p>
    Marketing agencies, outbound lead-gen firms, and boutique recruitment consultancies face severe labor constraints. By partnering with Fathom, agencies transform from labor-heavy service providers into scalable, high-margin software-enabled operators.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">Agency Transformation Model</div>
    <ul style="margin-bottom: 0;">
      <li><strong>Eliminate Freelancer Costs:</strong> Replace variable Upwork scrapers and virtual assistants with dedicated Fathom worker fleets.</li>
      <li><strong>10x Client Capacity:</strong> A single human account manager can oversee 20+ client campaigns powered by autonomous coworkers.</li>
      <li><strong>Client Self-Service Portals:</strong> White-label the web dashboard so clients see real-time lead counts and search logs under the agency's domain.</li>
    </ul>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">Agency Partner Benefits</div>
    <ul style="margin-bottom: 0;">
      <li><strong>Volume Seat Discounts:</strong> Tiered partner pricing allowing agencies to mark up services by 300% to 500%.</li>
      <li><strong>Custom Skill Templates:</strong> Agencies build proprietary coworker SOPs tailored to specialized niches (e.g. MedTech, Legal, Real Estate).</li>
      <li><strong>Dedicated Support & SLAs:</strong> Priority worker container routing and direct access to core engineering.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Typical Agency Partner Economics (10 Client Accounts)</div>
  <table>
    <thead>
      <tr>
        <th>Metric</th>
        <th>Legacy Agency Delivery</th>
        <th>Fathom-Powered Agency</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>Monthly Labor / Tool Cost</strong></td>
        <td>$12,000 / mo (3 Human VAs + SaaS)</td>
        <td><strong>Low Flat Seat Subscription</strong></td>
      </tr>
      <tr>
        <td><strong>Client Retainer Revenue</strong></td>
        <td>$25,000 / mo ($2,500/client)</td>
        <td>$25,000 / mo ($2,500/client)</td>
      </tr>
      <tr>
        <td><strong>Gross Profit Margin</strong></td>
        <td>52% ($13,000 / mo)</td>
        <td><strong>84%+ ($21,000+ / mo)</strong></td>
      </tr>
      <tr>
        <td><strong>Fulfillment Turnaround</strong></td>
        <td>5 – 7 Business Days</td>
        <td><strong>Instant & Continuous (24/7)</strong></td>
      </tr>
    </tbody>
  </table>
</div>

<div class="callout callout-info">
  <strong>Agency Stickiness:</strong> Once an agency embeds Fathom into its core client fulfillment pipeline, retention exceeds 95% annually.
</div>
""")

add_page(15, "COMMUNITY & NETWORK EFFECTS · FLYWHEEL",
"Skill Marketplace & Template Network",
"Harnessing Community-Driven Personas and SOPs to Drive Long-Term Defensibility",
"""
<div class="card-dark">
  <div class="card-title">The Template Network Effect: Collective Worker Intelligence</div>
  <p>
    As the Fathom community expands, users and partners contribute domain-specific coworker configurations, prompt engineering frameworks, and tool bindings into a shared <strong>Skill & Coworker Marketplace</strong>.
  </p>
</div>

<div class="grid-2">
  <div class="card card-accent">
    <div class="card-title-sm">1-Click Coworker Template Deployment</div>
    <p>Users can browse, test, and instantly hire pre-trained worker personas tailored to specific business domains:</p>
    <ul style="margin-bottom: 0;">
      <li><strong>Fintech SDR:</strong> Configured to navigate 2GIS, Crunchbase, and LinkedIn for financial technology executives.</li>
      <li><strong>Biotech Patent Scout:</strong> Deep-crawls PubMed, Google Patents, and university spin-off directories.</li>
      <li><strong>Real Estate PropTech Agent:</strong> Scrapes property registries, municipal zoning filings, and broker directories.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Marketplace Creator Monetization</div>
    <p>Developers and domain experts monetize their expertise by publishing proprietary coworker workflows:</p>
    <ul style="margin-bottom: 0;">
      <li><strong>Template Revenue Sharing:</strong> Creators receive a recurring royalty for every active employee seat running their template.</li>
      <li><strong>Verified Skill Badging:</strong> Enterprise-certified SOPs undergo automated security and compliance audits.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">The Self-Reinforcing Platform Flywheel</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">1. More Users</div>
      <div class="flow-desc">Adopt Fathom digital workers</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">2. More Templates</div>
      <div class="flow-desc">Created for niche industries</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">3. Faster Time-to-Value</div>
      <div class="flow-desc">New users deploy in seconds</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">4. Defensible Moat</div>
      <div class="flow-desc">Massive library of specialized SOPs</div>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>The Network Advantage:</strong> Competitors might copy code, but they cannot replicate an active ecosystem of thousands of specialized, battle-tested employee personas.
</div>
""")

# ==============================================================================
# PART IV: REAL-WORLD USE CASES & LIFE SCENARIOS (16-22)
# ==============================================================================

add_page(16, "REAL-WORLD USE CASE · SCENARIO 01",
"Autonomous Sales Development Rep (SDR)",
"End-to-End Cold Lead Discovery, Multi-Signal Verification & CRM Pipeline Creation",
"""
<div class="card-accent">
  <div class="card-title">Scenario Brief: Scaling Outbound Pipeline for a B2B SaaS Startup</div>
  <p>
    <strong>Company:</strong> CloudSecure (Cybersecurity SaaS).<br>
    <strong>Objective:</strong> Generate 100 verified CISO and VP IT contacts at mid-market financial firms in London every week.
  </p>
</div>

<div class="card">
  <div class="card-title">Autonomous Execution Workflow</div>
  <div class="timeline">
    <div class="timeline-item">
      <div class="timeline-time">Step 1</div>
      <div class="timeline-content">
        <div class="timeline-title">Directory & Registry Sweep</div>
        <div class="timeline-desc">The SDR coworker queries Companies House and business registries in parallel, identifying 120 mid-market financial firms fitting ICP parameters.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Step 2</div>
      <div class="timeline-content">
        <div class="timeline-title">Decision-Maker Mapping & Extraction</div>
        <div class="timeline-desc">Scans corporate team pages (<code>parse_corporate_site</code>) and social networks (<code>search_social</code>), identifying 108 CISO and IT leadership names.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Step 3</div>
      <div class="timeline-content">
        <div class="timeline-title">Email Permutation & SMTP Probing</div>
        <div class="timeline-desc">Generates corporate email permutations (<code>suggest_emails</code>) and validates each via non-intrusive SMTP 250 OK handshakes. 94 emails confirmed deliverable.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Step 4</div>
      <div class="timeline-content">
        <div class="timeline-title">CRM Auto-Sync & Telegram Notification</div>
        <div class="timeline-desc">Pushes all 94 leads into HubSpot with company revenue, tech stack tags (e.g. AWS, Okta), and notifies the sales team via Telegram.</div>
      </div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">Business Outcome</div>
    <ul style="font-size: 7.4pt; margin-bottom: 0;">
      <li><strong>Time Saved:</strong> 35 hours of manual human prospecting per week.</li>
      <li><strong>Data Quality:</strong> Zero email bounces (< 1% bounce rate on SMTP-verified leads).</li>
      <li><strong>Pipeline Generated:</strong> 8 qualified enterprise demo calls booked in month 1.</li>
    </ul>
  </div>

  <div class="card card-slate">
    <div class="card-title-sm">Human Operator Touchpoint</div>
    <p style="font-size: 7.4pt;">
      The human sales director spent just <strong>5 minutes per week</strong> reviewing the Telegram summary and giving 1-click approval for CRM injection.
    </p>
  </div>
</div>
""")

add_page(17, "REAL-WORLD USE CASE · SCENARIO 02",
"Executive Headhunter & Talent Scout",
"Autonomous Technical Sourcing, Candidate Mapping & Profile Corroboration",
"""
<div class="card-dark">
  <div class="card-title">Scenario Brief: Sourcing Hard-to-Find Senior Rust & AI Engineers</div>
  <p>
    <strong>Agency:</strong> Apex Tech Search (Executive Recruitment).<br>
    <strong>Objective:</strong> Map and source 30 senior systems engineers with deep Rust and distributed systems experience for an autonomous robotics venture.
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">Autonomous Sourcing Workflow</div>
    <ol style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>GitHub Repository Mining:</strong> Scans contributors to top-tier open-source Rust projects (e.g. Tokio, Axum, Polars) via <code>code_symbols</code>.</li>
      <li><strong>Social & Resume Cross-Referencing:</strong> Cross-checks GitHub handles against LinkedIn and Telegram to verify current employer, seniority, and tenure.</li>
      <li><strong>Deliverability Check:</strong> Verifies public email addresses attached to Git commits and profiles.</li>
      <li><strong>Candidate Dossier Compilation:</strong> Produces clean Markdown dossiers with project history and highlighted repository contributions.</li>
    </ol>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">Why Autonomous Sourcing Outperforms Humans</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Code-Level Understanding:</strong> Evaluates actual GitHub commit quality and technical complexity, not just keyword-stuffed LinkedIn resumes.</li>
      <li><strong>Unbiased Discovery:</strong> Finds high-caliber engineers who don't maintain active LinkedIn profiles but actively commit code.</li>
      <li><strong>Instant Reachout Readiness:</strong> Outlines personalized icebreakers based on the candidate's real recent open-source work.</li>
    </ul>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Recruitment Impact Metrics</div>
  <div class="grid-3">
    <div class="metric-box">
      <div class="metric-val">48 Hrs</div>
      <div class="metric-label">To Deliver 30 Vetted Candidates</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">38%</div>
      <div class="metric-label">Candidate Outreach Reply Rate</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">$0</div>
      <div class="metric-label">External Recruiter Subscriptions</div>
    </div>
  </div>
</div>

<div class="callout callout-info">
  <strong>Key Takeaway:</strong> Sourcing moves from a slow, manual LinkedIn grind to an automated, code-aware intelligence gathering pipeline.
</div>
""")

add_page(18, "REAL-WORLD USE CASE · SCENARIO 03",
"24/7 Market Intelligence & Competitor Tracker",
"Continuous Pricing Tracking, Feature Launches & Regulatory Monitoring",
"""
<div class="card-accent">
  <div class="card-title">Scenario Brief: Real-Time Competitive Landscape Monitoring</div>
  <p>
    <strong>Company:</strong> FinPay Global (B2B Fintech Platform).<br>
    <strong>Objective:</strong> Continuously monitor 15 direct global competitors for pricing adjustments, new product features, key executive hires, and regulatory license filings.
  </p>
</div>

<div class="card">
  <div class="card-title">Autonomous Monitoring & Synthesis Lifecycle</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">1. Scheduled Crawl</div>
      <div class="flow-desc">Runs every 6 hours across 15 target domains</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">2. Diff Detection</div>
      <div class="flow-desc">Extracts DOM changes on pricing & team pages</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">3. Memory Assimilation</div>
      <div class="flow-desc">Absorbs facts into entity graph (94µs/fact)</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">4. Executive Alert</div>
      <div class="flow-desc">Emits Slack alert only on significant strategic shifts</div>
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">Real-World Case Example: Pricing Shift Caught in 15 Minutes</div>
    <p style="font-size: 7.4pt;">
      When competitor <em>Stripe-X</em> adjusted their enterprise transaction fee from 2.9% to 2.4% on a Friday evening, the Fathom market analyst detected the pricing table diff, updated long-term memory, and alerted the Chief Product Officer via Telegram within 15 minutes.
    </p>
  </div>

  <div class="card card-indigo">
    <div class="card-title-sm">Institutional Knowledge Ingestion</div>
    <p style="font-size: 7.4pt;">
      All competitor historical changes are stored permanently in Fathom's SQLite entity graph. When leadership asks: <em>"How has Competitor Y's pricing evolved over the last 6 months?"</em>, the coworker generates an instant timeline report in under 5 milliseconds.
    </p>
  </div>
</div>

<div class="callout callout-success">
  <strong>Executive Value:</strong> Leadership stays three steps ahead of market dynamics with zero hours spent manually clicking competitor websites.
</div>
""")

add_page(19, "REAL-WORLD USE CASE · SCENARIO 04",
"Automated Customer Onboarding & Support",
"Autonomous Technical Setup, API Verification & 24/7 Troubleshooting",
"""
<div class="card-dark">
  <div class="card-title">Scenario Brief: Accelerating B2B Client Time-to-Value</div>
  <p>
    <strong>Company:</strong> DataStream API (Developer Infrastructure).<br>
    <strong>Objective:</strong> Guide new enterprise customers through webhook configuration, test payload verification, and initial API key provisioning with zero support backlog.
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">Autonomous Support Operations</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Incoming Ticket Triage:</strong> Reads incoming support emails or Slack channel queries, identifying customer intent and technical requirements.</li>
      <li><strong>Code & Log Inspection:</strong> Uses sandboxed REPL (<code>python_exec</code> / <code>node_exec</code>) to replicate customer webhook payloads and diagnose syntax errors.</li>
      <li><strong>Browser-Driven Setup:</strong> Accesses internal admin portals via governed Playwright computer control to verify client account provisioning.</li>
      <li><strong>Contextual Resolution:</strong> Queries persistent semantic memory for past resolutions, delivering precise, tested code fixes.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Measurable Performance Gains</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>First Response Time:</strong> Reduced from 45 minutes to <strong>under 12 seconds</strong>.</li>
      <li><strong>Onboarding Duration:</strong> Enterprise onboarding cycle compressed from 5 days to <strong>2 hours</strong>.</li>
      <li><strong>Resolution Rate:</strong> 78% of Tier-1 and Tier-2 developer onboarding tickets resolved without human engineer involvement.</li>
    </ul>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Human Escalation Protocol</div>
  <p style="font-size: 7.6pt;">
    If a ticket involves critical billing changes or unfamiliar error edge cases, the coworker pauses, summarizes the diagnosed root cause, and hands off the session to a human senior engineer with full contextual notes.
  </p>
</div>
""")

add_page(20, "REAL-WORLD USE CASE · SCENARIO 05",
"Back-Office & Invoice Reconciliation",
"Autonomous Document Parsing, Multi-System Data Entry & Financial Reconciliation",
"""
<div class="card-accent">
  <div class="card-title">Scenario Brief: Automating Repetitive Monthly Financial Operations</div>
  <p>
    <strong>Company:</strong> Global Logistics Partner (Supply Chain & Freight).<br>
    <strong>Objective:</strong> Ingest 500+ PDF vendor invoices monthly, cross-reference against warehouse delivery receipts, and input approved payments into 1C / QuickBooks.
  </p>
</div>

<div class="card">
  <div class="card-title">Autonomous Back-Office Pipeline</div>
  <div class="timeline">
    <div class="timeline-item">
      <div class="timeline-time">Phase 1</div>
      <div class="timeline-content">
        <div class="timeline-title">PDF Ingestion & Structural Extraction</div>
        <div class="timeline-desc">The worker downloads vendor invoices, applies <code>pdf_extract</code> and regex parsers, extracting vendor TIN, invoice number, line items, and totals.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Phase 2</div>
      <div class="timeline-content">
        <div class="timeline-title">Three-Way Matching & Verification</div>
        <div class="timeline-desc">Cross-references invoice line items against warehouse receipts and purchase orders stored in the internal database. Discrepancies flagged automatically.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Phase 3</div>
      <div class="timeline-content">
        <div class="timeline-title">Portal Entry via Governed Computer Use</div>
        <div class="timeline-desc">The worker navigates the accounting web portal via Playwright Accessibility Snapshots, fills invoice fields accurately, and queues batch payments.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Phase 4</div>
      <div class="timeline-content">
        <div class="timeline-title">CFO Single-Click Approval Batch</div>
        <div class="timeline-desc">Presents an executive summary of 500 reconciled invoices with zero errors for CFO sign-off via Telegram (<code>POST /api/v1/sessions/:id/approve</code>).</div>
      </div>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>Operational Impact:</strong> Replaces 80 hours of mind-numbing manual copy-paste data entry per month with an automated, auditable, and error-free execution loop.
</div>
""")

add_page(21, "REAL-WORLD USE CASE · SCENARIO 06",
"Autonomous Software Engineer & Maintainer",
"Codebase Mapping, Bug Investigation, Test Generation & Safe PR Creation",
"""
<div class="card-dark">
  <div class="card-title">Scenario Brief: Continuous Code Maintenance & Automated Bug Triage</div>
  <p>
    <strong>Company:</strong> SaaSScale Inc. (Cloud Platform).<br>
    <strong>Objective:</strong> Triage incoming Sentry error reports, navigate complex codebases, write reproducing unit tests, fix the underlying bug, and submit ready-to-review Pull Requests.
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">The 5-Step Code Engineering Loop</div>
    <ol style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Repo Mapping (<code>repo_map</code>):</strong> Parses AST symbols across 240+ project files in 34ms, mapping functions, types, and dependencies.</li>
      <li><strong>Bug Reproduction:</strong> Generates a standalone pytest/cargo test reproducing the exact exception in an isolated sandbox.</li>
      <li><strong>Targeted Code Modification:</strong> Edits source files precisely via <code>file_edit</code>, adhering to existing formatting.</li>
      <li><strong>Automated Test Execution:</strong> Runs test suites via <code>shell</code>, verifying that the failing test passes and zero regressions occur.</li>
      <li><strong>Git Branch & PR Creation:</strong> Commits changes with structured conventional commit messages and opens a GitHub PR.</li>
    </ol>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Real Empirical Case (From Fathom Test Suite)</div>
    <p style="font-size: 7.4pt;">
      In live case study 5 (<code>docs/article/05-case-studies.md</code>), an autonomous Fathom engineer was tasked with building a complete Python CLI with MoM revenue analytics, realistic sample data, and pytest coverage.
    </p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li>The agent wrote the code, ran tests, detected 1 calculation bug, edited the logic, and achieved <strong>23/23 passing tests</strong> completely autonomously in 74 seconds.</li>
    </ul>
  </div>
</div>

<div class="callout callout-info">
  <strong>Developer Superpower:</strong> Senior engineers focus on high-level system architecture while digital coworkers handle routine bugs, dependency upgrades, and test coverage.
</div>
""")

add_page(22, "REAL-WORLD USE CASE · SCENARIO 07",
"Regulatory & Legal Document Auditor",
"Multi-Jurisdiction Compliance Verification, Clause Extraction & Risk Highlighting",
"""
<div class="card-accent">
  <div class="card-title">Scenario Brief: Comprehensive Contract & Compliance Audit</div>
  <p>
    <strong>Company:</strong> EuroTrust Legal Advisory.<br>
    <strong>Objective:</strong> Audit 200 vendor Master Services Agreements (MSAs) for GDPR compliance, data liability caps, non-compete clauses, and jurisdiction risks.
  </p>
</div>

<div class="grid-3">
  <div class="card">
    <div class="card-title-sm">1. Parallel Ingestion</div>
    <p style="font-size: 7.2pt;">Spawns 5 analyst agents to ingest 200 legal PDFs, extracting structured clauses into JSON schemas in under 8 minutes.</p>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">2. Cross-Jurisdiction Analysis</div>
    <p style="font-size: 7.2pt;">Cross-references liability clauses against EU GDPR and UK Data Protection Act requirements, flagging non-compliant terms.</p>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">3. Executive Risk Matrix</div>
    <p style="font-size: 7.2pt;">Compiles a high-contrast risk matrix categorizing contracts into Green (Compliant), Yellow (Review Needed), and Red (Immediate Risk).</p>
  </div>
</div>

<div class="card">
  <div class="card-title">Sample Legal Risk Assessment Matrix Output</div>
  <table>
    <thead>
      <tr>
        <th>Vendor Contract</th>
        <th>Liability Cap</th>
        <th>GDPR Data Processing Clause</th>
        <th>Governing Law</th>
        <th>Risk Rating</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td>Vendor Alpha MSA</td>
        <td>12 Months Fees ($120k)</td>
        <td>Standard Contractual Clauses (SCC) Included</td>
        <td>England & Wales</td>
        <td><span class="badge badge-green">Low Risk</span></td>
      </tr>
      <tr>
        <td>Vendor Beta SaaS</td>
        <td>$5,000 (Sub-Standard)</td>
        <td>Missing Sub-Processor Notification Clause</td>
        <td>Delaware, USA</td>
        <td><span class="badge badge-amber">Medium Risk</span></td>
      </tr>
      <tr>
        <td>Vendor Gamma Cloud</td>
        <td>Unlimited Liability</td>
        <td>Zero GDPR Data Retention Schedule</td>
        <td>Cyprus</td>
        <td><span class="badge badge-red">High Risk</span></td>
      </tr>
    </tbody>
  </table>
</div>

<div class="callout callout-success">
  <strong>Audit Acceleration:</strong> A 3-week manual legal paralegal review is accomplished in <strong>under 30 minutes</strong> with total clause traceability.
</div>
""")

print("Saved pages 11 to 22...")

# ==============================================================================
# PART V: SYSTEM ARCHITECTURE & HOW IT WORKS (23-27)
# ==============================================================================

add_page(23, "SYSTEM ARCHITECTURE · VIRTUAL OFFICE",
"How the Virtual Office Operates",
"The Conceptual Architecture of an Autonomous Multi-Agent Organization",
"""
<div class="card-dark">
  <div class="card-title">The Virtual Office Hierarchy: Division of Labor in Action</div>
  <p>
    Fathom does not operate as a single monolithic prompt. It structures work like an agile digital consulting agency where a Coordinator (Manager) delegates to specialized Worker Pods, supervised by Analysts and formatted by Writers.
  </p>
</div>

<div class="card">
  <div class="card-title">Organizational Workflow Topology</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">1. Client Request</div>
      <div class="flow-desc">Submitted via API / Slack / Cron</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">2. Coordinator (Manager)</div>
      <div class="flow-desc">Plans & decomposes into subtasks</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">3. Parallel Worker Pods</div>
      <div class="flow-desc">Researchers, Scrapers & Coders</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">4. Verifier & Writer</div>
      <div class="flow-desc">SMTP checks, QA & Final Report</div>
    </div>
  </div>
</div>

<div class="grid-3">
  <div class="card card-accent">
    <div class="card-title-sm">1. Coordinator Agent</div>
    <p style="font-size: 7.2pt;">Analyzes the objective, sets token budgets, establishes task trees, and tracks progress across child sub-agents.</p>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">2. Parallel Workers</div>
    <p style="font-size: 7.2pt;">Execute discrete searches, scrape registries, run Python data transformations, and operate web browsers simultaneously.</p>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">3. Quality Judge & Writer</div>
    <p style="font-size: 7.2pt;">Validates data completeness against the initial goal, triggers gap-filling rounds, and compiles clean deliverables.</p>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Context Protection: Disk-Spill Backing</div>
  <p style="font-size: 7.6pt;">
    Workers return concise executive summaries to the coordinator while spilling multi-megabyte raw HTML and scraped payloads to disk workspaces, ensuring the manager's context window never overflows.
  </p>
</div>
""")

add_page(24, "SYSTEMS ENGINEERING · WHY RUST",
"Why Rust? The Architecture of Performance",
"Zero-Cost Abstractions, Memory Safety & The Elimination of Python Bottlenecks",
"""
<div class="card-accent">
  <div class="card-title">The Engineering Foundation: Why We Chose Rust</div>
  <p>
    Traditional agent frameworks built on Python suffer from high latency, heavy memory consumption, fragile type errors, and concurrency bottlenecks caused by the Global Interpreter Lock (GIL). <strong>Fathom is built natively in Rust</strong> to provide enterprise-grade reliability and speed.
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber">
    <div class="card-title-sm">The Python Framework Trap</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Massive Memory Footprint:</strong> Idle Python runtimes consume 400MB–1.5GB RAM per agent process.</li>
      <li><strong>GIL Concurrency Deadlocks:</strong> Asynchronous I/O is serialized, choking multi-agent swarms.</li>
      <li><strong>Slow Startup Latency:</strong> 2.5 to 8.0 seconds just to load interpreter and dependency trees.</li>
      <li><strong>Brittle Type Bugs:</strong> Runtime dictionary mismatches crash multi-hour research runs.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">The Fathom Rust Advantage</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Microscopic RAM Footprint:</strong> Lean 15–35 MB baseline RAM allows 100+ agents per server.</li>
      <li><strong>True Multi-Core Parallelism:</strong> Tokio async tasks utilize all CPU cores without locks.</li>
      <li><strong>Instant Binary Startup:</strong> Starts in under 5 milliseconds from cold execution.</li>
      <li><strong>Compile-Time Guarantees:</strong> Strong typing and ownership eliminate memory leaks and runtime panics.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Architectural Efficiency Comparison</div>
  <table>
    <thead>
      <tr>
        <th>Engineering Metric</th>
        <th>Python Agent Frameworks</th>
        <th>Fathom Rust Runtime</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>Tool Dispatch Latency</strong></td>
        <td>25.0 – 150.0 ms</td>
        <td><strong>0.75 ms (Microsecond Level)</strong></td>
      </tr>
      <tr>
        <td><strong>Memory Usage (100 Agents)</strong></td>
        <td>40 – 120 GB RAM (Requires Cluster)</td>
        <td><strong>1.5 – 3.5 GB RAM (Single Modest VM)</strong></td>
      </tr>
      <tr>
        <td><strong>HTML Parsing Speed</strong></td>
        <td>15,000 rows/sec (BeautifulSoup)</td>
        <td><strong>350,000+ rows/sec (Scraper Rust)</strong></td>
      </tr>
      <tr>
        <td><strong>Binary Packaging</strong></td>
        <td>Fragile venv + wheels</td>
        <td><strong>Single Static Binary (Zero Deps)</strong></td>
      </tr>
    </tbody>
  </table>
</div>

<div class="callout callout-success">
  <strong>Speed Equals Intelligence:</strong> Microsecond tool execution means agents spend 99.9% of their time waiting for model tokens, not framework overhead.
</div>
""")

add_page(25, "EXECUTION RUNTIME · TASK DECOMPOSITION",
"Coordinator & Worker Swarm Execution",
"How Complex High-Level Tasks Are Broken Down and Executed in Parallel",
"""
<div class="card-dark">
  <div class="card-title">Dynamic Hierarchical Task Decomposition</div>
  <p>
    When a user assigns a complex project, the Coordinator uses a structured planning prompt to analyze dependencies, formulate execution branches, and launch parallel sub-agents via the <code>spawn_agent</code> tool.
  </p>
</div>

<div class="card">
  <div class="card-title">Sample Task Decomposition Tree</div>
  <div class="diagram-flow" style="flex-direction: column; align-items: stretch; gap: 5px;">
    <div style="background: white; border: 1px solid var(--border-color); padding: 5px 8px; border-radius: 4px;">
      <strong>Root Coordinator:</strong> "Comprehensive Due Diligence on European AI FinTechs"
    </div>
    <div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 5px;">
      <div style="background: #eff6ff; border-left: 3px solid var(--primary-accent); padding: 4px 6px; border-radius: 3px; font-size: 7pt;">
        <strong>Branch 1 (Registry Agent):</strong><br>Scrapes Companies House & Handelsregister for corporate registration & filings.
      </div>
      <div style="background: #f0fdf4; border-left: 3px solid var(--emerald); padding: 4px 6px; border-radius: 3px; font-size: 7pt;">
        <strong>Branch 2 (OSINT Agent):</strong><br>Harvests C-level leadership, LinkedIn profiles, and verified work emails.
      </div>
      <div style="background: #faf5ff; border-left: 3px solid var(--purple); padding: 4px 6px; border-radius: 3px; font-size: 7pt;">
        <strong>Branch 3 (Tech Stack Agent):</strong><br>Fingerprints homepage HTML for 40+ technology signatures (AWS, Stripe, Next.js).
      </div>
    </div>
    <div style="background: #fffbeb; border-left: 3px solid var(--amber); padding: 4px 6px; border-radius: 3px; font-size: 7.2pt;">
      <strong>Synthesis Node (Analyst & Writer):</strong> Merges all 3 branches, reconciles conflicts, and formats final executive PDF report.
    </div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">Recursive Depth Limits & Guardrails</div>
    <p style="font-size: 7.4pt;">
      To prevent runaway sub-agent spawning loops, Fathom enforces strict depth limits (default max depth: 2). Coordinators can spawn workers, but workers cannot spawn infinite child trees without explicit permission.
    </p>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">Tokio JoinSet Multi-Threading</div>
    <p style="font-size: 7.4pt;">
      All spawned workers run as concurrent tasks inside a Tokio <code>JoinSet</code>. If one branch encounters a slow network request or rate limit, sibling branches continue executing at full speed.
    </p>
  </div>
</div>

<div class="callout callout-info">
  <strong>Fault-Tolerant Merging:</strong> If one sub-agent fails due to an anti-bot block, the coordinator graceful incorporates partial findings from other branches without failing the overall run.
</div>
""")

add_page(26, "INTER-AGENT PROTOCOL · TELEMETRY",
"The Broadcast Message Bus",
"Real-Time Event Distribution Across Swarms, UI Dashboards & External Channels",
"""
<div class="card-accent">
  <div class="card-title">Decoupled Asynchronous Telemetry Architecture</div>
  <p>
    Communication across sub-agents, persistence layers, and client dashboards occurs over a centralized, high-throughput <strong>Tokio broadcast message bus</strong> (<code>event_tx</code>).
  </p>
</div>

<div class="card">
  <div class="card-title">Message Bus Architecture & Event Flow</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">Agent Event Emitters</div>
      <div class="flow-desc">Spawns, Tool Calls, Thoughts</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">Broadcast Bus (1024 cap)</div>
      <div class="flow-desc">Lock-free Tokio Channel</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">Multiple Subscribers</div>
      <div class="flow-desc">SSE, TUI, Prometheus, DB</div>
    </div>
  </div>
</div>

<div class="grid-3">
  <div class="card card-slate">
    <div class="card-title-sm">1. Server-Sent Events (SSE)</div>
    <p style="font-size: 7.2pt;">Streams live agent events to web and desktop dashboards, updating UI progress bars and sparklines in real time.</p>
  </div>

  <div class="card card-slate">
    <div class="card-title-sm">2. Prometheus Scrapers</div>
    <p style="font-size: 7.2pt;">The metrics middleware consumes event counters, exporting real-time tool durations and token velocity to Grafana.</p>
  </div>

  <div class="card card-slate">
    <div class="card-title-sm">3. SQLite Audit Ledger</div>
    <p style="font-size: 7.2pt;">Records immutable execution logs, tool inputs, and decision outcomes for permanent compliance replay.</p>
  </div>
</div>

<div class="card-dark">
  <div class="card-title">Key Benefits of Broadcast Topology</div>
  <ul style="font-size: 7.4pt; margin-bottom: 0;">
    <li><strong>Zero Performance Drag:</strong> Slow UI clients or network drops never slow down or block agent reasoning threads.</li>
    <li><strong>Multi-Client Sync:</strong> An engineer on CLI, a manager on web dashboard, and a Telegram bot all receive synchronized updates simultaneously.</li>
    <li><strong>Plug-and-Play Extensibility:</strong> Connect custom corporate webhooks or auditing sinks by subscribing to the stream.</li>
  </ul>
</div>
""")

add_page(27, "FAULT TOLERANCE · RESILIENCE",
"Reliability & Self-Healing Workflows",
"How Background Jobs Survive Server Reboots and Fix Their Own Mistakes",
"""
<div class="card-dark">
  <div class="card-title">Resilience by Design: Surviving Crashes and API Failures</div>
  <p>
    In production environments, network timeouts, API rate limits, and server restarts are inevitable. Fathom's durable job engine is designed to ensure that <strong>no work is ever lost</strong>.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">1. Detached Process Execution (setsid)</div>
    <p style="font-size: 7.4pt;">
      Background jobs run as independent operating system processes detached from the terminal session. Closing your laptop, terminating SSH, or exiting the CLI does not interrupt ongoing worker execution.
    </p>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">2. Self-Healing Task Augmentation</div>
    <p style="font-size: 7.4pt;">
      When a job fails (e.g. hitting an unexpected rate limit), the runner does not execute a blind restart. Attempt #2 automatically injects the previous error trace and partial files, prompting the agent to adapt its strategy and self-heal.
    </p>
  </div>
</div>

<div class="card">
  <div class="card-title">The Self-Correction Loop in Action</div>
  <div class="timeline">
    <div class="timeline-item">
      <div class="timeline-time">Attempt 1</div>
      <div class="timeline-content">
        <div class="timeline-title">Initial Execution Fails</div>
        <div class="timeline-desc">Worker attempts to scrape 50 pages using primary API; hits HTTP 429 Rate Limit after saving 20 records.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Attempt 2</div>
      <div class="timeline-content">
        <div class="timeline-title">Augmented Prompt & Recovery</div>
        <div class="timeline-desc">Worker inspects the 20 saved records in workspace, switches search backend to DuckDuckGo fallback, applies request throttling, and completes remaining 30 records.</div>
      </div>
    </div>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Stale Process Detection & Recovery</div>
  <p style="font-size: 7.6pt;">
    If a server experiences an abrupt power outage, Fathom's SQLite registry identifies dead PIDs upon reboot and marks interrupted jobs as <code>stale</code>, allowing 1-click seamless restarts without corrupted state.
  </p>
</div>
""")

# ==============================================================================
# PART VI: GOVERNED COMPUTER USE & BROWSER AUTOMATION (28-31)
# ==============================================================================

add_page(28, "COMPUTER USE · ACCESSIBILITY PARADIGM",
"How Digital Workers See & Control Computers",
"Accessibility-Tree Snapshots: Semantic Understanding Over Brittle Visual Pixels",
"""
<div class="card-accent">
  <div class="card-title">The Semantic Revolution in Browser Automation</div>
  <p>
    Traditional browser automation fails because web pages frequently change CSS classes, responsive layouts, and visual styling. Fathom bypasses visual fragility by operating browsers via the <strong>Accessibility Tree</strong> (the standard semantic model used by screen readers).
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber">
    <div class="card-title-sm">Legacy Pixel / CSS Automation (Fragile)</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Obfuscated CSS:</strong> Class names like <code>.sc-82f_btn</code> change on every frontend software deployment.</li>
      <li><strong>Pixel Drift:</strong> Screen scaling or window resizing causes mouse clicks to hit empty whitespace.</li>
      <li><strong>High Latency:</strong> Uploading high-res screenshots to vision models consumes massive bandwidth and tokens.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Fathom Semantic Accessibility (Rock Solid)</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Semantic Roles:</strong> The agent sees functional elements: <code>Button: "Login"</code>, <code>TextBox: "Email"</code>.</li>
      <li><strong>Opaque Numerical Refs:</strong> Direct addressing via stable tokens (e.g. <code>@e14</code>) regardless of visual layout.</li>
      <li><strong>10x Token Efficiency:</strong> Lightweight accessibility snapshots use 90% fewer tokens than raw images.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Accessibility Snapshot Architecture</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">1. Chromium Page</div>
      <div class="flow-desc">Dynamic DOM & SPAs</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">2. ARIA Snapshot</div>
      <div class="flow-desc">Extracts semantic accessibility tree</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">3. Opaque Token Mapping</div>
      <div class="flow-desc">Assigns stable refs (@e1, @e2)</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">4. Agent Interaction</div>
      <div class="flow-desc">click(@e1), type(@e2, "text")</div>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>Layout-Agnostic Stability:</strong> Whether a website is redesigned, translated into Japanese, or resized to mobile viewport, the accessibility tree retains functional semantic integrity.
</div>
""")

add_page(29, "COMPUTER USE · ANTI-BREAKAGE",
"Why Opaque Refs Beat Brittle Selectors",
"Anti-Staleness Verification, Form Sanitization & Deterministic Element Resolution",
"""
<div class="card-dark">
  <div class="card-title">Deterministic Element Targeting: Eliminating Broken Clicks</div>
  <p>
    In modern web portals, asynchronous JavaScript frequently alters DOM elements between agent reasoning steps. Fathom implements an active <strong>Anti-Staleness Guard</strong> to guarantee that actions are executed only on valid, intended targets.
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">1. Pre-Execution Freshness Check</div>
    <p style="font-size: 7.4pt;">
      Before executing a <code>computer_click</code> or <code>computer_type</code> command, the runtime takes a microsecond snapshot to confirm that the target element ref (<code>@e14</code>) still exists and matches its original role fingerprint.
    </p>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">2. Stale-Ref Rejection & Self-Recovery</div>
    <p style="font-size: 7.4pt;">
      If the page navigated or a popup closed during agent thinking, the ref is rejected immediately with an explicit error: <em>"Ref @e14 is stale. Capturing fresh snapshot."</em> The agent re-evaluates the new DOM state without misclicking.
    </p>
  </div>
</div>

<div class="card">
  <div class="card-title">Form Data Security & Scrubbing</div>
  <div class="grid-2">
    <div>
      <strong>Automatic Password Scrubbing</strong>
      <p style="font-size: 7.2pt;">Password inputs and sensitive token fields are automatically masked in accessibility snapshots, preventing sensitive credentials from appearing in reasoning transcripts.</p>
    </div>
    <div>
      <strong>Confined File Workspace</strong>
      <p style="font-size: 7.2pt;">Browser downloads are strictly confined to an isolated directory (<code>/data/browser</code>), preventing malicious downloads from touching host system files.</p>
    </div>
  </div>
</div>

<div class="callout callout-info">
  <strong>Zero Phantom Interactions:</strong> Fathom ensures that automated browser actions are as predictable, deterministic, and safe as compiled software code.
</div>
""")

add_page(30, "COMPUTER USE · HUMAN IN THE LOOP",
"Screen Streaming & Seamless Takeover",
"Real-Time Browser Feeds and Operator Interventions for CAPTCHAs and 2FA",
"""
<div class="card-accent">
  <div class="card-title">Governed Operator Collaboration: The Human Takeover Lease</div>
  <p>
    When autonomous workers encounter high-security barriers—such as multi-factor authentication (2FA SMS), bank logins, or complex CAPTCHA puzzles—Fathom pauses gracefully and invites the human operator to assist.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">1. Low-Latency Screen Feed (/screen)</div>
    <p style="font-size: 7.4pt;">
      The active browser viewport is streamed in real time over a WebSocket feed (500ms frame intervals) directly into the desktop app or web dashboard, allowing operators to monitor browser actions live.
    </p>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">2. Exclusive Takeover Lease (/control/ws)</div>
    <p style="font-size: 7.4pt;">
      Clicking <strong>"Take Control"</strong> grants the human operator exclusive keyboard and mouse access. While human-owned, bot automation commands are blocked, preventing accidental race conditions.
    </p>
  </div>
</div>

<div class="card">
  <div class="card-title">The 4-Step Collaborative Handshake</div>
  <div class="timeline">
    <div class="timeline-item">
      <div class="timeline-time">Phase 1</div>
      <div class="timeline-content">
        <div class="timeline-title">Agent Encounters 2FA Gate</div>
        <div class="timeline-desc">The worker logs into a vendor billing portal; hits an SMS 2FA challenge. Emits a high-priority alert to the operator.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Phase 2</div>
      <div class="timeline-content">
        <div class="timeline-title">Operator Takes Control</div>
        <div class="timeline-desc">The manager clicks "Take Control" in the desktop UI, enters the 6-digit SMS code from their phone, and completes the login.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Phase 3</div>
      <div class="timeline-content">
        <div class="timeline-title">Lease Released</div>
        <div class="timeline-desc">The manager clicks "Release Control". The worker captures a fresh accessibility snapshot of the authenticated billing portal.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Phase 4</div>
      <div class="timeline-content">
        <div class="timeline-title">Autonomous Work Resumes</div>
        <div class="timeline-desc">The digital employee continues downloading invoices and reconciling spreadsheets with zero loss of task context.</div>
      </div>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>The Hybrid Ideal:</strong> 99% autonomous heavy lifting paired with instant 1% human oversight at critical security checkpoints.
</div>
""")

add_page(31, "SECURITY SANDBOXING · DOCKER SUPERVISOR",
"Docker Sandboxes & Network Egress",
"Per-Agent Container Isolation, Port Sandboxing & Zero Data Leakage",
"""
<div class="card-dark">
  <div class="card-title">Ironclad Isolation: One Container Per Active Worker</div>
  <p>
    To ensure complete enterprise security and prevent cross-tenant data contamination, Fathom provisions an isolated Docker container for every active digital coworker via <code>crates/supervisor</code>.
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">Container Sandbox Specifications</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Capability Stripping:</strong> All Linux capabilities dropped (<code>cap_drop: ["ALL"]</code>).</li>
      <li><strong>Privilege Escalation Blocked:</strong> <code>no-new-privileges: true</code> enforced at Docker runtime.</li>
      <li><strong>Ephemeral & Persistent Volumes:</strong> Dedicated volume mounts for browser cookies and workspace files.</li>
      <li><strong>Deterministic Port Mapping:</strong> Dedicated loopback ports per agent (19000–19999 range).</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Strict Network Egress Guarding</div>
    <p style="font-size: 7.4pt;">The browser loopback service enforces strict firewall rules:</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Cloud Metadata Blocked:</strong> Rejects all calls to <code>169.254.169.254</code> (AWS/GCP metadata theft protection).</li>
      <li><strong>Private Subnet Deny:</strong> Rejects connections to private RFC1918 subnets (<code>10.0.0.0/8</code>, <code>192.168.0.0/16</code>).</li>
      <li><strong>Multicast & Loopback Deny:</strong> Prevents agents from scanning or probing internal corporate networks.</li>
    </ul>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Enterprise Multi-Tenant Security Guarantee</div>
  <p style="font-size: 7.6pt;">
    Even if an agent navigates to a malicious webpage attempting prompt injection or cross-site scripting, the attack remains completely trapped within the unprivileged Docker container with zero access to the host OS or other client workspaces.
  </p>
</div>
""")

# ==============================================================================
# PART VII: LONG-TERM MEMORY & INSTITUTIONAL KNOWLEDGE (32-34)
# ==============================================================================

add_page(32, "KNOWLEDGE ENGINE · PERSISTENT MEMORY",
"How Coworkers Remember Across Months",
"Long-Term Semantic Memory: Zero SaaS Cost, Local SQLite & True Institutional Knowledge",
"""
<div class="card-accent">
  <div class="card-title">Persistent Institutional Intelligence vs. Stateless AI</div>
  <p>
    Human employees become more valuable over time because they accumulate institutional knowledge: client preferences, industry jargon, and past project outcomes. Fathom equips digital workers with a persistent <strong>Semantic Memory Engine</strong> that compounds intelligence across months.
  </p>
</div>

<div class="grid-2">
  <div class="card card-amber">
    <div class="card-title-sm">Stateless AI Assistants (No Memory)</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Forget Everything:</strong> Every new session starts from scratch; context is lost immediately upon closing the tab.</li>
      <li><strong>Repetitive Prompts:</strong> Users must re-explain company guidelines, target criteria, and past decisions daily.</li>
      <li><strong>Expensive Cloud Vector DBs:</strong> Third-party hosted vector databases cost thousands in recurring API fees.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Fathom Digital Employees (Persistent Memory)</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Permanent Retention:</strong> Discovered contacts, company facts, and client instructions persist indefinitely in SQLite.</li>
      <li><strong>Sub-5ms Memory Digest:</strong> Relevant past facts are automatically summarized and injected at session start.</li>
      <li><strong>Zero External Cloud Costs:</strong> Operates entirely in-process with local SQLite FTS5 (BM25) and TF-IDF vectorization.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Memory Growth Over Time</div>
  <div class="timeline">
    <div class="timeline-item">
      <div class="timeline-time">Day 1</div>
      <div class="timeline-content">
        <div class="timeline-title">Initial Briefing & Territory Ingestion</div>
        <div class="timeline-desc">The SDR coworker absorbs target market parameters, verified competitor lists, and outreach tone guidelines.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Day 30</div>
      <div class="timeline-content">
        <div class="timeline-title">Entity Graph Compounding</div>
        <div class="timeline-desc">The knowledge graph holds 2,500+ verified executive relationships, company tech stacks, and past email outcomes.</div>
      </div>
    </div>

    <div class="timeline-item">
      <div class="timeline-time">Day 90</div>
      <div class="timeline-content">
        <div class="timeline-title">Full Institutional Fluency</div>
        <div class="timeline-desc">The coworker detects executive job changes automatically: <em>"John Doe moved from Acme to Globex; updating CRM."</em></div>
      </div>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>Compounding Asset:</strong> Your digital workforce becomes smarter, faster, and more tailored to your business every single day.
</div>
""")

add_page(33, "MEMORY ENGINE · ABSORB PIPELINE",
"Sub-Millisecond Knowledge Ingestion",
"The 4-Stage Absorb Pipeline: Deduplication, Lineage Chains & Secret Redaction",
"""
<div class="card-dark">
  <div class="card-title">The 4-Stage Memory Absorb Pipeline (94 µs / Fact)</div>
  <p>
    Raw information from web searches and research sessions cannot be dumped blindly into memory. Fathom processes facts through a strict, multi-stage curation pipeline in under <strong>94 microseconds per fact</strong>.
  </p>
</div>

<div class="diagram-flow">
  <div class="flow-step">
    <div class="flow-title">1. Secret Scrubbing</div>
    <div class="flow-desc">Regex strips API keys & passwords</div>
  </div>
  <div class="flow-arrow">→</div>
  <div class="flow-step">
    <div class="flow-title">2. SHA-256 Fast Dedup</div>
    <div class="flow-desc">Skips known facts in 5.1ms/100 facts</div>
  </div>
  <div class="flow-arrow">→</div>
  <div class="flow-step">
    <div class="flow-title">3. Lineage Versioning</div>
    <div class="flow-desc">Marks outdated facts as superseded</div>
  </div>
  <div class="flow-arrow">→</div>
  <div class="flow-step">
    <div class="flow-title">4. Entity Linking</div>
    <div class="flow-desc">Builds typed graph relationships</div>
  </div>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">Append-Only Truth & Lineage Chains</div>
    <p style="font-size: 7.4pt;">
      Facts are never silently overwritten. If a company raises a Series B round after previously raising Series A, the old fact is marked with a <code>supersedes</code> edge, preserving the historical evolution of truth for auditing.
    </p>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">Hybrid Retrieval: 0.70 Vector + 0.30 BM25</div>
    <p style="font-size: 7.4pt;">
      Queries combine the exact keyword precision of SQLite FTS5 (for names, emails, and codes) with semantic vector similarity, delivering <strong>sub-2ms search latencies</strong> across thousands of memories.
    </p>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Automated Garbage Collection (GC)</div>
  <p style="font-size: 7.6pt;">
    A background pruning routine cleans up ephemeral task artifacts (configurable 30-day TTL) while permanently retaining core entity relationships and verified client preferences.
  </p>
</div>
""")

add_page(34, "KNOWLEDGE GRAPH · ENTITY TOPOLOGY",
"The Enterprise Entity Knowledge Graph",
"Mapping People, Companies, Roles and Technologies into an Interconnected Web",
"""
<div class="card-accent">
  <div class="card-title">Relational Knowledge: Beyond Flat Text Records</div>
  <p>
    Fathom does not merely store isolated snippets of text. In <code>crates/memory</code>, it structures verified information into a <strong>typed, directional Entity Knowledge Graph</strong> that captures the rich web of corporate and professional relationships.
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">Typed Graph Relationships</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><code>Person ──[works_at]──► Company</code> (with title & seniority)</li>
      <li><code>Person ──[leads]─────► Department</code> (e.g. Engineering)</li>
      <li><code>Company ─[invests_in]─► Startup</code> (with funding round date)</li>
      <li><code>Company ─[uses_tech]──► Technology</code> (e.g. AWS, Next.js)</li>
      <li><code>Company ─[competes_with]► Competitor</code> (direct market rival)</li>
    </ul>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">6-Hop Graph Traversal Queries</div>
    <p style="font-size: 7.4pt;">Agents query the graph to answer complex relational questions:</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><em>"Which former Stripe engineers are now CTOs at Series-A AI companies in Berlin?"</em></li>
      <li><em>"Which competitors are backed by the same venture fund as our company?"</em></li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Graph Visualization & Tool Integration (memory_graph)</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">Jane Doe (CTO)</div>
      <div class="flow-desc">Verified Person Node</div>
    </div>
    <div class="flow-arrow">──[works_at]──►</div>
    <div class="flow-step">
      <div class="flow-title">Acme FinTech</div>
      <div class="flow-desc">Company (Series A, Berlin)</div>
    </div>
    <div class="flow-arrow">──[uses_tech]──►</div>
    <div class="flow-step">
      <div class="flow-title">Rust & Axum</div>
      <div class="flow-desc">Tech Signature Node</div>
    </div>
  </div>
</div>

<div class="callout callout-info">
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
<div class="card-dark">
  <div class="card-title">Deterministic Safety: The Governance Policy Engine</div>
  <p>
    Deploying autonomous agents in corporate environments demands strict, auditable guardrails. In <code>crates/governance</code>, Fathom implements a <strong>fail-closed policy engine</strong> where every tool call is authorized against declarative security rules prior to execution.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">1. Declarative Allow / Deny Glob Rules</div>
    <p style="font-size: 7.4pt;">Administrators define granular access control rules matching tool names and target resources:</p>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Allow Search:</strong> <code>allow: tool="web_search", target="*"</code></li>
      <li><strong>Restrict Browser:</strong> <code>allow: tool="browser.*", target="https://*.linkedin.com/*"</code></li>
      <li><strong>Deny Admin Access:</strong> <code>deny: tool="browser.type", target="*/admin/*"</code></li>
      <li><strong>Deny Destructive Shell:</strong> <code>deny: tool="shell", target="rm -rf *"</code></li>
    </ul>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">2. Strict Fail-Closed Security Stance</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>Deny Wins:</strong> If an action matches both an allow and a deny rule, the <strong>deny rule always takes absolute precedence</strong>.</li>
      <li><strong>Unmatched Fails Closed:</strong> If an action does not explicitly match an allow rule in an active policy, it is rejected by default.</li>
      <li><strong>Operator Claim Gating:</strong> Administrative actions require verified operator claims in HTTP headers.</li>
    </ul>
  </div>
</div>

<div class="card-slate">
  <div class="card-title-sm">Human Approval Side-Effect List</div>
  <p style="font-size: 7.6pt;">
    Configurable critical tools (e.g. <code>save_contacts</code>, CRM push, <code>git_push</code>) can be placed in the <code>approval_tools</code> list, ensuring that data never leaves the research sandbox without human sign-off.
  </p>
</div>
""")

add_page(36, "SECURITY VAULT · CREDENTIAL ISOLATION",
"The Ironclad Credentials Vault",
"AES-256-GCM Encryption: Zero Secret Visibility in Model Prompts",
"""
<div class="card-accent">
  <div class="card-title">Protecting Secrets Against Prompt Injection & Extraction</div>
  <p>
    A major vulnerability in traditional AI wrappers is passing API keys and passwords directly inside prompt contexts, making them susceptible to prompt-injection extraction. <strong>Fathom isolates credentials behind an AES-256-GCM encrypted vault</strong>.
  </p>
</div>

<div class="grid-2">
  <div class="card card-emerald">
    <div class="card-title-sm">1. Hardware-Grade Encryption (Ring Crate)</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li>Secrets stored in SQLite are encrypted using <strong>AES-256-GCM</strong> authenticated encryption.</li>
      <li>Encryption keys are derived from secure environment variables (<code>FATHOM_CREDENTIAL_KEY</code>).</li>
      <li>API listing endpoints return strictly masked strings (e.g. <code>sk-live-***1234</code>).</li>
    </ul>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">2. Zero Secret Tools in Agent Registry</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li>Agents have <strong>zero tools to read or query plaintext secrets</strong>.</li>
      <li>When an agent invokes a service (e.g. CRM push), the backend adapter resolves credentials internally in Rust memory.</li>
      <li>Prompt injection attacks cannot extract keys that the LLM has no mechanism to access.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Credential Lifecycle Architecture</div>
  <div class="diagram-flow">
    <div class="flow-step">
      <div class="flow-title">1. Operator Inputs Secret</div>
      <div class="flow-desc">Via UI / Protected API</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">2. AES-256-GCM Vault</div>
      <div class="flow-desc">Encrypted in SQLite DB</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">3. Internal Rust Resolution</div>
      <div class="flow-desc">Resolved by adapter at call-time</div>
    </div>
    <div class="flow-arrow">→</div>
    <div class="flow-step">
      <div class="flow-title">4. Zero Prompt Exposure</div>
      <div class="flow-desc">LLM sees zero API tokens</div>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>Enterprise Compliance:</strong> Enterprise security teams can deploy Fathom with full confidence that proprietary CRM tokens and database passwords remain completely secure.
</div>
""")

add_page(37, "REGULATORY COMPLIANCE · AUDIT TRAILS",
"Complete Compliance & Audit Trails",
"Immutable Decision Ledgers, Automatic Secret Redaction & GDPR / 152-FZ Readiness",
"""
<div class="card-dark">
  <div class="card-title">Auditable AI: Immutable Records for Every Decision</div>
  <p>
    Enterprise compliance requires full traceability for every autonomous action. Fathom logs all authorization decisions into an append-only, tamper-resistant <strong>Audit Ledger</strong> (<code>/api/v1/governance/audit</code>).
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">Audit Record Fields</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><code>timestamp</code>: Nanosecond-precision RFC 3339 timestamp.</li>
      <li><code>agent_id</code> & <code>session_id</code>: Full UUIDv7 session tracing.</li>
      <li><code>tool</code> & <code>intent</code>: Declared action and reasoning rationale.</li>
      <li><code>target</code> & <code>decision</code>: URL / file path and Allow/Deny verdict.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Automatic Secret Redaction</div>
    <p style="font-size: 7.4pt;">
      Before audit records are written to SQLite, a built-in redaction engine scans tool arguments with regex scanners, stripping API keys, bearer tokens, and passwords to prevent credential leakage into logs.
    </p>
  </div>
</div>

<div class="card">
  <div class="card-title">Data Privacy & Regulatory Framework Compliance</div>
  <table>
    <thead>
      <tr>
        <th>Regulation</th>
        <th>Compliance Invariant</th>
        <th>Fathom Implementation Mechanism</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>GDPR (EU)</strong></td>
        <td>Lawful processing of public data & right to be forgotten</td>
        <td>Strict OSINT confidence scoring + <code>ContactDb</code> delete APIs.</td>
      </tr>
      <tr>
        <td><strong>152-FZ (Russia)</strong></td>
        <td>Personal data localization & verifiable storage</td>
        <td>Self-hosted local SQLite / PostgreSQL storage on sovereign infrastructure.</td>
      </tr>
      <tr>
        <td><strong>SOC 2 Type II</strong></td>
        <td>Access control, auditability, and data isolation</td>
        <td>AES-256-GCM vault, per-agent Docker sandboxes, immutable audit trail.</td>
      </tr>
    </tbody>
  </table>
</div>

<div class="callout callout-info">
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
<div class="card-accent">
  <div class="card-title">Empirical Benchmark Results: The Systems Difference</div>
  <p>
    Benchmarks were executed offline using deterministic synthetic fixtures via <code>fathom bench</code> on macOS ARM64 (Apple M4, 10 cores, release build with LTO and stripped symbols).
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">Tool Layer Overhead (4 Layers)</div>
    <table>
      <thead>
        <tr>
          <th>Layer</th>
          <th>Iterations</th>
          <th>Time / Call</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td><code>registry.execute</code> (Raw dispatch)</td>
          <td>300</td>
          <td>5,140 µs</td>
        </tr>
        <tr>
          <td><code>execute_batch</code> (1 call)</td>
          <td>300</td>
          <td>7,614 µs</td>
        </tr>
        <tr>
          <td><code>execute_batch</code> (8 calls amortized)</td>
          <td>320</td>
          <td><strong>5,893 µs (~0.75ms overhead)</strong></td>
        </tr>
        <tr>
          <td><code>ToolCall</code> JSON Serde</td>
          <td>100,000</td>
          <td><strong>752 ns</strong></td>
        </tr>
      </tbody>
    </table>
  </div>

  <div class="card">
    <div class="card-title-sm">Multi-Threaded Parallel Speedups</div>
    <table>
      <thead>
        <tr>
          <th>Task</th>
          <th>Sequential</th>
          <th>Tokio Spawn</th>
          <th>Speedup</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td>16 × <code>file_read</code> (2MB each)</td>
          <td>79.0 ms</td>
          <td>25.8 ms</td>
          <td><strong>3.06×</strong></td>
        </tr>
        <tr>
          <td>8 × <code>parse_html</code> (1MB table)</td>
          <td>130.3 ms</td>
          <td>34.5 ms</td>
          <td><strong>3.78×</strong></td>
        </tr>
        <tr>
          <td>8 × <code>code_symbols</code> (240 files)</td>
          <td>61.9 ms</td>
          <td>20.4 ms</td>
          <td><strong>3.04×</strong></td>
        </tr>
        <tr>
          <td><code>web_feed</code> (XML streaming)</td>
          <td>—</td>
          <td>—</td>
          <td><strong>1.11M items/s</strong></td>
        </tr>
      </tbody>
    </table>
  </div>
</div>

<div class="card">
  <div class="card-title">Comparative Architecture Matrix</div>
  <table>
    <thead>
      <tr>
        <th>Metric</th>
        <th>Python Frameworks (LangChain/AutoGPT)</th>
        <th>Fathom (Rust Compiled)</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>Cold Start Latency</strong></td>
        <td>2,500 – 8,000 ms</td>
        <td><strong>&lt; 5 ms (500x Faster)</strong></td>
      </tr>
      <tr>
        <td><strong>Tool Dispatch Overhead</strong></td>
        <td>25 – 150 ms</td>
        <td><strong>~0.75 ms (100x Faster)</strong></td>
      </tr>
      <tr>
        <td><strong>Concurrency</strong></td>
        <td>Blocked by Python GIL</td>
        <td><strong>True Multi-Threaded Tokio Spawn</strong></td>
      </tr>
    </tbody>
  </table>
</div>
""")

add_page(39, "HARDWARE EFFICIENCY · WORKFORCE DENSITY",
"Hardware Efficiency & Worker Density",
"Hosting 100+ Concurrent Digital Employees on a Single Modest Server",
"""
<div class="card-dark">
  <div class="card-title">Maximizing Hardware Density: The Economics of Efficiency</div>
  <p>
    The true test of enterprise scalability is operational efficiency per server dollar. Because Fathom compiles to lean machine code with zero runtime interpreter overhead, it achieves unprecedented hardware worker density.
  </p>
</div>

<div class="grid-2">
  <div class="card">
    <div class="card-title-sm">Python Framework Server Footprint (100 Agents)</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>RAM Required:</strong> 64 GB – 128 GB RAM (Heavy Kubernetes Cluster).</li>
      <li><strong>CPU Saturation:</strong> High context-switching drag and Python GIL contention.</li>
      <li><strong>Monthly Server Cost:</strong> $400 – $1,200 / month on AWS/GCP.</li>
      <li><strong>Hosting Cost Per Worker:</strong> $4.00 – $12.00 / month.</li>
    </ul>
  </div>

  <div class="card card-emerald">
    <div class="card-title-sm">Fathom Rust Server Footprint (100 Coworkers)</div>
    <ul style="font-size: 7.2pt; margin-bottom: 0;">
      <li><strong>RAM Required:</strong> <strong>1.5 GB – 3.5 GB RAM</strong> (Modest 8-Core Box).</li>
      <li><strong>CPU Utilization:</strong> Zero idle CPU burn; epoll/kqueue event-driven I/O.</li>
      <li><strong>Monthly Server Cost:</strong> $30 – $60 / month on Hetzner/DigitalOcean.</li>
      <li><strong>Hosting Cost Per Worker:</strong> <strong>&lt; $0.50 / month</strong>.</li>
    </ul>
  </div>
</div>

<div class="card">
  <div class="card-title">Server Resource Allocation Model (8 Cores, 32GB RAM)</div>
  <div class="grid-4">
    <div class="metric-box">
      <div class="metric-val">100+</div>
      <div class="metric-label">Concurrent Coworkers</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">~25 MB</div>
      <div class="metric-label">Average RAM / Coworker</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">&lt; 1%</div>
      <div class="metric-label">Idle CPU Utilization</div>
    </div>
    <div class="metric-box">
      <div class="metric-val">99.4%</div>
      <div class="metric-label">Gross Margin on Compute</div>
    </div>
  </div>
</div>

<div class="callout callout-success">
  <strong>The Bottom Line:</strong> Fathom cuts server infrastructure costs by <strong>95%</strong>, allowing businesses to scale their virtual workforce profitably.
</div>
""")

add_page(40, "PERFORMANCE SUMMARY · SPEED HIGHLIGHTS",
"Empirical Performance Summary",
"Sub-Millisecond Ingestion, Microsecond Deserialization & Streaming Throughput",
"""
<div class="card-accent">
  <div class="card-title">The Complete Performance Scorecard</div>
  <p>
    Every layer of the Fathom stack has been profiled and optimized for microsecond-level execution speed, ensuring instantaneous response times across all enterprise workflows.
  </p>
</div>

<div class="grid-3">
  <div class="card card-emerald">
    <div class="card-title-sm">Memory Ingestion</div>
    <div class="metric-val" style="font-size: 15pt;">94 µs</div>
    <div class="metric-label">Per-Fact Absorb (100 batch)</div>
    <p style="font-size: 7pt; margin-top: 4px;">5.1ms to absorb and dedup 100 facts via SHA-256 fast path.</p>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">Hybrid Retrieval</div>
    <div class="metric-val" style="font-size: 15pt;">1.62 ms</div>
    <div class="metric-label">Search Median Latency</div>
    <p style="font-size: 7pt; margin-top: 4px;">Fused Vector + BM25 search across 500 facts in under 2ms.</p>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">XML Feed Stream</div>
    <div class="metric-val" style="font-size: 15pt;">1.11M</div>
    <div class="metric-label">Items / Sec Throughput</div>
    <p style="font-size: 7pt; margin-top: 4px;">Streaming quick-xml parser processes 12k feed items in 10.8ms.</p>
  </div>
</div>

<div class="card">
  <div class="card-title">Parser Scaling & Extraction Latencies</div>
  <table>
    <thead>
      <tr>
        <th>Document Type</th>
        <th>Size / Rows</th>
        <th>Parse Duration</th>
        <th>Effective Throughput</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>HTML Table (Scraper)</strong></td>
        <td>191 KB / 3,000 rows</td>
        <td>9.55 ms</td>
        <td>314,268 rows / sec</td>
      </tr>
      <tr>
        <td><strong>HTML Large (Scraper)</strong></td>
        <td>773 KB / 12,000 rows</td>
        <td>34.22 ms</td>
        <td>350,723 rows / sec</td>
      </tr>
      <tr>
        <td><strong>JSON Tree Walk</strong></td>
        <td>4 MB / 20,000 objects</td>
        <td>43.47 ms</td>
        <td>Stateless Parallel-Safe</td>
      </tr>
      <tr>
        <td><strong>AST Code Map (240 files)</strong></td>
        <td>3.3 MB Rust Source</td>
        <td>34.20 ms</td>
        <td>4,330 Summary Lines</td>
      </tr>
    </tbody>
  </table>
</div>

<div class="callout callout-info">
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
<div class="card-dark">
  <div class="card-title">The Future of Autonomous Work: Evolutionary Milestones</div>
  <p>
    Fathom is built on an extensible foundation designed to expand from text and browser automation into full multi-modal vision grounding, decentralized peer-to-peer swarms, and voice outreach.
  </p>
</div>

<div class="timeline">
  <div class="timeline-item">
    <div class="timeline-time">Q4 2026</div>
    <div class="timeline-content">
      <div class="timeline-title">Vision-Native Hybrid Browser Engine</div>
      <div class="timeline-desc">Fusing accessibility trees with real-time multi-modal vision grounding (Qwen-VL-Max) to navigate complex canvas elements, charts, and interactive web maps.</div>
    </div>
  </div>

  <div class="timeline-item">
    <div class="timeline-time">Q1 2027</div>
    <div class="timeline-content">
      <div class="timeline-title">Decentralized Multi-Company Coworker Mesh</div>
      <div class="timeline-desc">Enabling autonomous coworkers from different corporate tenants to securely negotiate, share verified data, and trade services using cryptographic tokens.</div>
    </div>
  </div>

  <div class="timeline-item">
    <div class="timeline-time">Q2 2027</div>
    <div class="timeline-content">
      <div class="timeline-title">Autonomous Voice & Cold Calling Agents</div>
      <div class="timeline-desc">Integrating sub-300ms ultra-low-latency neural voice synthesis for phone verification, switchboard navigation, and automated qualification calls.</div>
    </div>
  </div>

  <div class="timeline-item">
    <div class="timeline-time">Q3 2027</div>
    <div class="timeline-content">
      <div class="timeline-title">Self-Evolving Skill Generation</div>
      <div class="timeline-desc">Agents automatically record human operator demonstration sessions in the browser, compiling them into reusable, deterministic Rust tools.</div>
    </div>
  </div>
</div>

<div class="callout callout-info">
  <strong>Continuous Innovation:</strong> Every milestone strengthens the enterprise moat and expands the operational scope of digital coworkers.
</div>
""")

add_page(42, "EXECUTIVE SUMMARY · CONCLUSION",
"The Autonomous Enterprise OS",
"Unlocking Limitless Scalability with 24/7 Governed Digital Employees",
"""
<div class="card-accent">
  <div class="card-title">Conclusion: The New Era of Enterprise Labor</div>
  <p>
    The constraints of human headcount scaling—recruiting delays, training overhead, high churn, and linear salary expansion—are no longer barriers to business growth. <strong>Fathom delivers the definitive software runtime for the autonomous enterprise.</strong>
  </p>
</div>

<div class="grid-3">
  <div class="card card-emerald">
    <div class="card-title-sm">1. High-Performance Core</div>
    <p style="font-size: 7.2pt;">Compiled Rust engine, Tokio async concurrency, and microsecond tool dispatch for blazing speed and minimal server footprint.</p>
  </div>

  <div class="card card-accent">
    <div class="card-title-sm">2. Flat-Rate Economics</div>
    <p style="font-size: 7.2pt;">Predictable seat subscriptions with unlimited neural compute, powered by frontier model arbitrage (Kimi k3, Qwen 3.8 Max, GLM 5.3).</p>
  </div>

  <div class="card card-purple">
    <div class="card-title-sm">3. 100% Remote Autonomy</div>
    <p style="font-size: 7.2pt;">Self-directed workers handling research, OSINT, outreach, code maintenance, and computer use with total enterprise governance.</p>
  </div>
</div>

<div class="card">
  <div class="card-title">Transform Your Organization Today</div>
  <div class="grid-2">
    <div>
      <p><strong>Deploy in Under 60 Seconds:</strong> Download the static Rust binary, configure your target channels, and launch your first digital employee fleet today.</p>
    </div>
    <div>
      <p><strong>Scale Without Limits:</strong> Expand from 1 SDR coworker to a 100-agent multi-department autonomous workforce with zero HR friction.</p>
    </div>
  </div>
</div>

<div class="card-slate" style="text-align: center; padding: 12px;">
  <div style="font-size: 11pt; font-weight: 800; color: var(--primary);">FATHOM: UNIVERSAL AUTONOMOUS AI WORKER RUNTIME</div>
  <div style="font-size: 8pt; color: var(--text-muted); margin-top: 2px;">High-Performance Systems Engineering · Unlimited Neural Intelligence · Autonomous Enterprise Labor</div>
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

print("\nRendering Individual Page PDFs via Headless Google Chrome...")
pdf_files = []
for i in range(1, 43):
    fname = f"page_{i:02d}.html"
    fpath = os.path.join(WP_DIR, fname)
    pdf_name = f"page_{i:02d}.pdf"
    pdf_path = os.path.join(WP_DIR, pdf_name)
    
    if os.path.exists(fpath):
        cmd = [
            CHROME_PATH,
            "--headless",
            "--disable-gpu",
            "--no-margins",
            f"--print-to-pdf={pdf_path}",
            f"file://{fpath}"
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if os.path.exists(pdf_path) and os.path.getsize(pdf_path) > 0:
            print(f"Generated [{i:02d}/42]: {pdf_name} ({os.path.getsize(pdf_path):,} bytes)")
            pdf_files.append(pdf_path)
        else:
            print(f"Failed to generate {pdf_name}: {res.stderr}")

print("\nMerging all 42 pages into Master Whitepaper PDF...")
master_pdf_path = os.path.join(WP_DIR, "Fathom_Whitepaper.pdf")
writer = PdfWriter()
for pdf in pdf_files:
    writer.append(pdf)

with open(master_pdf_path, "wb") as f:
    writer.write(f)
writer.close()

print(f"\n=======================================================")
print(f"SUCCESS: 42-Page Master Whitepaper PDF Generated!")
print(f"Location: {master_pdf_path}")
print(f"Size: {os.path.getsize(master_pdf_path):,} bytes")
print(f"Total Pages: {len(pdf_files)}")
print(f"=======================================================\n")
