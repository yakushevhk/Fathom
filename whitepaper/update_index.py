#!/usr/bin/env python3
import os, glob

WP_DIR = "/Users/yakushev/Documents/GitHub/Fathom/whitepaper"

pages_data = [
    (1, "Strategic Overview & Universal Runtime"),
    (2, "The Autonomous Digital Employee Paradigm"),
    (3, "Virtual Office Topology & Autonomous Coordination"),
    (4, "Day in the Life of a Digital Employee Fleet [Mockup: Inbox Zero]"),
    (5, "Core Engine Architecture & Sub-Millisecond Dispatch"),
    (6, "Autonomous SDR & Corporate Registries [Mockup: SDR Outbound]"),
    (7, "Market Intelligence & Competitor Tracking [Mockup: DOM Diff]"),
    (8, "Executive Recruiting & Talent Sourcing [Mockup: GitHub AST]"),
    (9, "Back-Office & Financial Reconciliation [Mockup: QuickBooks]"),
    (10, "DevOps Maintenance & Code Engineering [Mockup: Sentry PR]"),
    (11, "Autonomous Research & Deep Intelligence"),
    (12, "Self-Healing Background Jobs & Scheduled Routines [Mockup: Swarm DAG]"),
    (13, "Automated Outreach: The 7-Layer OSINT Engine"),
    (14, "Multi-Source Identity Fusion & Graph Deduplication"),
    (15, "Automated Enrichment & Cross-Platform Corroboration"),
    (16, "SMTP 250 OK Gauntlet & Deliverability Engine"),
    (17, "Multi-Channel Outbound Dispatch [Mockup: Instantly Bridge]"),
    (18, "Goal-Oriented Autonomous Search & LLM Judge"),
    (19, "Zero-Bounce Deliverability & Anti-Spam Pipeline [Mockup: Audit Table]"),
    (20, "Two-Way CRM Synchronization & Autonomous Deal Staging"),
    (21, "Computer Use: Sandboxed Browser & Desktop Automation"),
    (22, "Accessibility Trees vs Visual Coordinate Scripts"),
    (23, "Governed Execution & 2FA Human Takeover [Mockup: AWS 2FA]"),
    (24, "Live Human Supervision, Telegram Approval & Takeover [Mockup: CS REPL]"),
    (25, "Sub-Millisecond Tokio Dispatch Engine (~0.75 ms)"),
    (26, "Enterprise Security Architecture & AES-256-GCM Vault [Mockup: Vault]"),
    (27, "Regulatory Compliance, Legal Audits & Risk Matrices [Mockup: Legal MSA]"),
    (28, "Hardware Efficiency & High-Concurrency Benchmarks"),
    (29, "Cold Start, Concurrency & Memory Profiles"),
    (30, "Long-Term Memory & Entity Knowledge Graphs [Mockup: Graph]"),
    (31, "Self-Healing Infrastructure & Error Diagnostics"),
    (32, "HTTP Server, Axum Router & SSE Real-Time Protocol"),
    (33, "Multi-Channel Gateway (Telegram, Slack, Discord, Web)"),
    (34, "The Self-Replicating Growth Loop & Inbound Engine"),
    (35, "Customer Acquisition & The Free Value Audit Flywheel"),
    (36, "Pricing Architecture: Flat-Rate Unlimited Seat Economics"),
    (37, "Agency Scaling, Multi-Tenancy & White-Label Deployments [Mockup: Agency Fleet]"),
    (38, "Total Cost of Ownership & 90%+ Gross Margin Unit Economics"),
    (39, "Enterprise Deployment Topologies (Bare-Metal, Cloud, Edge)"),
    (40, "High-Availability, Clustering & State Synchronization"),
    (41, "Enterprise SLA, Disaster Recovery & Continuous Auditing"),
    (42, "Conclusion: The Autonomous Enterprise Era & Roadmap")
]

options_html = []
for num, title in pages_data:
    options_html.append(f'<option value="{num}">Page {num:02d}: {title}</option>')

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
      background: rgba(15, 23, 42, 0.94);
      backdrop-filter: blur(12px);
      border: 1px solid rgba(255, 255, 255, 0.18);
      border-radius: 30px;
      padding: 6px 14px;
      display: flex;
      align-items: center;
      gap: 10px;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
      color: #ffffff;
      font-size: 8.5pt;
    }}
    .deck-btn {{
      background: rgba(255, 255, 255, 0.15);
      border: none;
      color: #ffffff;
      padding: 5px 12px;
      border-radius: 16px;
      cursor: pointer;
      font-weight: 600;
      font-size: 8pt;
      transition: all 0.15s ease;
    }}
    .deck-btn:hover {{ background: #2563eb; }}
    .deck-btn-primary {{ background: #2563eb; }}
    .deck-select {{
      background: rgba(255, 255, 255, 0.12);
      color: #ffffff;
      border: 1px solid rgba(255, 255, 255, 0.2);
      padding: 4px 10px;
      border-radius: 14px;
      font-size: 8pt;
      outline: none;
      max-width: 320px;
    }}
    .deck-select option {{ background: #0f172a; color: #ffffff; }}
    iframe.page-frame {{
      width: 210mm;
      height: 297mm;
      border: none;
      box-shadow: 0 12px 35px rgba(15, 23, 42, 0.18);
      border-radius: 4px;
      display: block;
      margin: 20px auto;
      background: #ffffff;
    }}
  </style>
</head>
<body style="background: #e2e8f0; margin: 0; padding: 40px 0 80px 0;">

  <div class="deck-nav">
    <button class="deck-btn" onclick="prevPage()">◀ Prev</button>
    <select class="deck-select" id="pageSelect" onchange="jumpToPage(this.value)">
      {''.join(options_html)}
    </select>
    <button class="deck-btn" onclick="nextPage()">Next ▶</button>
    <span style="color: rgba(255,255,255,0.4);">|</span>
    <button class="deck-btn" onclick="toggleViewMode(this)" id="viewModeBtn">Show All 42 Pages</button>
    <a href="Fathom_Whitepaper.pdf" target="_blank" class="deck-btn deck-btn-primary" style="text-decoration:none;">📄 Download PDF (14.6 MB)</a>
    <a href="mockups/index.html" target="_blank" class="deck-btn" style="text-decoration:none; background:#4f46e5;">🖼 15 HD Mockups</a>
  </div>

  <div id="deckContainer">
    <iframe class="page-frame" id="singleFrame" src="page_01.html"></iframe>
  </div>

  <script>
    let cur = 1;
    const total = 42;
    let isAllView = false;

    function jumpToPage(num) {{
      cur = parseInt(num);
      document.getElementById('pageSelect').value = cur;
      if (!isAllView) {{
        document.getElementById('singleFrame').src = 'page_' + String(cur).padStart(2, '0') + '.html';
        window.scrollTo(0, 0);
      }} else {{
        const el = document.getElementById('frame-' + cur);
        if (el) el.scrollIntoView({{ behavior: 'smooth' }});
      }}
    }}

    function prevPage() {{
      if (cur > 1) jumpToPage(cur - 1);
    }}

    function nextPage() {{
      if (cur < total) jumpToPage(cur + 1);
    }}

    function toggleViewMode(btn) {{
      const container = document.getElementById('deckContainer');
      isAllView = !isAllView;
      if (isAllView) {{
        btn.innerText = "Single Slide Mode";
        let html = '';
        for (let i = 1; i <= total; i++) {{
          html += '<iframe class="page-frame" id="frame-' + i + '" src="page_' + String(i).padStart(2, '0') + '.html"></iframe>';
        }}
        container.innerHTML = html;
      }} else {{
        btn.innerText = "Show All 42 Pages";
        container.innerHTML = '<iframe class="page-frame" id="singleFrame" src="page_' + String(cur).padStart(2, '0') + '.html"></iframe>';
      }}
    }}

    document.addEventListener('keydown', (e) => {{
      if (e.key === 'ArrowRight' || e.key === 'PageDown') nextPage();
      if (e.key === 'ArrowLeft' || e.key === 'PageUp') prevPage();
    }});
  </script>

</body>
</html>"""

with open(os.path.join(WP_DIR, "index.html"), "w", encoding="utf-8") as f:
    f.write(index_html)

print("Updated whitepaper/index.html successfully.")
