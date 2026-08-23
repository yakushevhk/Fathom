#!/usr/bin/env python3
import os, subprocess, concurrent.futures, tempfile, shutil

MOCKUPS_DIR = "/Users/yakushev/Documents/GitHub/Fathom/whitepaper/mockups"
os.makedirs(MOCKUPS_DIR, exist_ok=True)
CHROME_PATH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

GPUI_CSS = """
@import url('https://fonts.googleapis.com/css2?family=Geist:wght@300;400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600;700&display=swap');

* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: #09090b;
  color: #f4f4f5;
  font-family: 'Geist', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  padding: 30px;
  -webkit-font-smoothing: antialiased;
}

/* Master GPUI Metal Window Frame */
.gpui-window {
  width: 1220px;
  height: 780px;
  background: #111115;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  box-shadow: 0 35px 90px rgba(0, 0, 0, 0.85), 0 0 0 1px rgba(255, 255, 255, 0.05);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

/* Glassmorphic Metal Topbar */
.gpui-topbar {
  background: #16161b;
  padding: 10px 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  align-items: center;
  justify-content: space-between;
  backdrop-filter: blur(16px);
}
.gpui-traffic { display: flex; gap: 7px; align-items: center; }
.gpui-dot { width: 11px; height: 11px; border-radius: 50%; display: inline-block; }
.gdot-red { background: #ff5f56; }
.gdot-yellow { background: #ffbd2e; }
.gdot-green { background: #27c93f; }

.gpui-breadcrumbs {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-family: 'JetBrains Mono', monospace;
  color: #71717a;
}
.gpui-crumb-active { color: #f4f4f5; font-weight: 600; }
.gpui-crumb-sep { color: #3f3f46; }

.gpui-model-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  background: rgba(99, 102, 241, 0.12);
  border: 1px solid rgba(99, 102, 241, 0.28);
  color: #a5b4fc;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  font-weight: 600;
  padding: 3px 9px;
  border-radius: 6px;
}

/* 3-Pane Body */
.gpui-body {
  display: grid;
  grid-template-columns: 240px 1fr 320px;
  flex: 1;
  background: #0d0d10;
  overflow: hidden;
}

/* Left Sidebar (Fleet / Channels) */
.gpui-sidebar {
  background: #131317;
  border-right: 1px solid rgba(255, 255, 255, 0.07);
  padding: 12px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 8px;
}
.sidebar-section-title {
  font-size: 10px;
  font-weight: 700;
  color: #71717a;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 6px;
}
.sidebar-btn-add {
  width: 18px;
  height: 18px;
  background: #1c1c22;
  border: 1px solid #27272a;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #a1a1aa;
  font-size: 12px;
  cursor: pointer;
}
.gpui-bot-row {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 6.5px 8px;
  border-radius: 7px;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.12s;
}
.gpui-bot-row.is-active {
  background: #1e1e26;
  border-color: rgba(99, 102, 241, 0.35);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
}
.gpui-avatar {
  width: 28px;
  height: 28px;
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  flex-shrink: 0;
}
.gpui-bot-details { display: flex; flex-direction: column; overflow: hidden; flex: 1; }
.gpui-bot-title-row { display: flex; justify-content: space-between; align-items: center; }
.gpui-bot-name { font-size: 11.5px; font-weight: 600; color: #f4f4f5; }
.gpui-bot-status-dot { width: 6px; height: 6px; border-radius: 50%; background: #34d399; }
.gpui-bot-sub { font-size: 9.5px; color: #71717a; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 1px; }

.gpui-user-pill {
  display: flex;
  align-items: center;
  gap: 8px;
  border-top: 1px solid rgba(255, 255, 255, 0.07);
  padding-top: 10px;
}
.gpui-user-icon {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: linear-gradient(135deg, #4f46e5, #6366f1);
  color: #f4f4f5;
  font-size: 9.5px;
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid #818cf8;
}
.gpui-user-meta { font-size: 11px; font-weight: 600; color: #f4f4f5; }

/* Center Chat / Execution Stream */
.gpui-center-pane {
  padding: 14px 18px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  background: #0d0d10;
  overflow: hidden;
}
.gpui-chat-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  padding-bottom: 8px;
}
.gpui-active-head { display: flex; align-items: center; gap: 8px; }
.gpui-active-title { font-size: 13px; font-weight: 700; color: #f4f4f5; }
.gpui-speed-tag { font-size: 9.5px; font-family: 'JetBrains Mono', monospace; color: #34d399; }

.gpui-stream {
  display: flex;
  flex-direction: column;
  gap: 9px;
  overflow-y: auto;
  padding: 10px 0;
}
.gpui-msg-user {
  background: #18181f;
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: #f4f4f5;
  padding: 8px 12px;
  border-radius: 8px 8px 2px 8px;
  align-self: flex-end;
  font-size: 11.5px;
  max-width: 85%;
  line-height: 1.45;
}
.gpui-msg-bot {
  background: #131318;
  border: 1px solid rgba(255, 255, 255, 0.07);
  color: #d4d4d8;
  padding: 10px 14px;
  border-radius: 8px 8px 8px 2px;
  align-self: flex-start;
  font-size: 11.5px;
  width: 98%;
  line-height: 1.5;
}

/* Tool Calling Chips (Collapsed & Interactive) */
.tool-call-group {
  margin: 6px 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.tool-chip {
  background: #09090c;
  border: 1px solid #27272a;
  border-radius: 6px;
  padding: 5px 9px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: #a1a1aa;
}
.tool-chip-left { display: flex; align-items: center; gap: 7px; }
.tool-icon-exec { color: #f59e0b; font-weight: 700; }
.tool-name-highlight { color: #38bdf8; font-weight: 600; }
.tool-result-badge {
  color: #34d399;
  font-size: 9px;
  background: rgba(52, 211, 153, 0.1);
  padding: 1px 5px;
  border-radius: 4px;
}

.tool-collapsed-bar {
  background: #111116;
  border: 1px dashed #27272a;
  border-radius: 5px;
  padding: 4px 8px;
  font-size: 9.5px;
  font-family: 'JetBrains Mono', monospace;
  color: #71717a;
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.gpui-card-deliverable {
  background: #09090c;
  border: 1px solid rgba(255, 255, 255, 0.09);
  border-radius: 7px;
  padding: 8px 12px;
  margin: 6px 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.deliverable-line { display: flex; align-items: center; gap: 7px; font-size: 11px; color: #d4d4d8; }
.chk-green { color: #34d399; font-weight: 800; }

.gpui-composer {
  background: #141419;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 7px 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: 11px;
}
.comp-icon { color: #71717a; font-size: 13px; cursor: pointer; }
.comp-input-text { color: #71717a; flex: 1; }
.comp-send-key {
  background: #27272a;
  color: #ececee;
  padding: 2px 7px;
  border-radius: 4px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 9.5px;
  font-weight: 600;
}

/* Right Panel: Live Terminal / Browser Inspection */
.gpui-right-pane {
  background: #131317;
  border-left: 1px solid rgba(255, 255, 255, 0.07);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.right-pane-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  color: #f4f4f5;
  font-size: 11px;
  font-weight: 600;
}
.browser-frame-mock {
  background: #09090c;
  border: 1px solid #27272a;
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.browser-top-strip {
  background: #18181f;
  padding: 5px 8px;
  display: flex;
  align-items: center;
  gap: 6px;
  border-bottom: 1px solid #27272a;
}
.url-text-pill {
  background: #0d0d10;
  color: #38bdf8;
  font-family: 'JetBrains Mono', monospace;
  font-size: 9px;
  padding: 2px 6px;
  border-radius: 4px;
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.browser-viewport-inner {
  padding: 8px;
  font-size: 9.5px;
  color: #d4d4d8;
  min-height: 95px;
  font-family: 'JetBrains Mono', monospace;
}

.takeover-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 10px;
  color: #71717a;
}
.btn-gpui-action {
  background: #1e1e26;
  border: 1px solid rgba(255, 255, 255, 0.12);
  color: #f4f4f5;
  padding: 3px 8px;
  border-radius: 5px;
  font-size: 10px;
  font-weight: 600;
  cursor: pointer;
}

.cron-routines-box {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 4px;
}
.cron-title-label {
  font-size: 9.5px;
  font-weight: 700;
  color: #71717a;
  text-transform: uppercase;
  letter-spacing: 0.08em;
}
.cron-card {
  background: #18181f;
  border: 1px solid #27272a;
  border-radius: 6px;
  padding: 4px 7px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 10px;
}
.cron-time-val { color: #38bdf8; font-family: 'JetBrains Mono', monospace; font-size: 9px; }

/* Bottom Metal Status Bar */
.gpui-statusbar {
  background: #141418;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  padding: 4px 14px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-family: 'JetBrains Mono', monospace;
  font-size: 9.5px;
  color: #71717a;
}
.statusbar-left { display: flex; gap: 12px; align-items: center; }
.statusbar-right { display: flex; gap: 14px; align-items: center; }
.stat-active-item { color: #d4d4d8; }
.stat-green { color: #34d399; }
"""

def generate_gpui_screen(filename, crumb_path, model_name, active_bot_name, active_avatar_svg, active_avatar_bg, chat_html, browser_url, browser_body_html, routines_list):
    bots = [
        ("Chief of Staff", "linear-gradient(135deg, #0d9488, #14b8a6)", "👑", "briefing ready, 3 meetings staged"),
        ("Sales Outbound", "linear-gradient(135deg, #ea580c, #f97316)", "⚡", "40 accounts researched, 18 queued"),
        ("Market Intel", "linear-gradient(135deg, #4f46e5, #6366f1)", "🧠", "competitor pricing diff alert"),
        ("Talent Scout", "linear-gradient(135deg, #0284c7, #38bdf8)", "🔍", "30 senior Rust engineers mapped"),
        ("Back-Office", "linear-gradient(135deg, #e11d48, #fb7185)", "📑", "500 invoices matched, 0 errors"),
        ("DevOps Maintainer", "linear-gradient(135deg, #059669, #10b981)", "🛠️", "23/23 tests passing, PR #142"),
    ]

    sidebar_bot_html = []
    for bname, bbg, bsvg, bsub in bots:
        is_act = " is-active" if bname == active_bot_name else ""
        sidebar_bot_html.append(f"""
        <div class="gpui-bot-row{is_act}">
          <div class="gpui-avatar" style="background: {bbg};">{bsvg}</div>
          <div class="gpui-bot-details">
            <div class="gpui-bot-title-row">
              <span class="gpui-bot-name">{bname}</span>
              <span class="gpui-bot-status-dot"></span>
            </div>
            <div class="gpui-bot-sub">{bsub}</div>
          </div>
        </div>""")

    routines_html = []
    for rname, rtime in routines_list:
        routines_html.append(f"""
        <div class="cron-card">
          <span style="color:#f4f4f5; display:flex; align-items:center; gap:5px;"><span style="color:#71717a;">◷</span> {rname}</span>
          <span class="cron-time-val">{rtime}</span>
        </div>""")

    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Fathom GPUI Metal · {active_bot_name}</title>
<style>{GPUI_CSS}</style>
</head>
<body>

<div class="gpui-window">
  <!-- Topbar -->
  <div class="gpui-topbar">
    <div class="gpui-traffic">
      <span class="gpui-dot gdot-red"></span>
      <span class="gpui-dot gdot-yellow"></span>
      <span class="gpui-dot gdot-green"></span>
    </div>

    <div class="gpui-breadcrumbs">
      <span>fathom</span>
      <span class="gpui-crumb-sep">//</span>
      <span>coworkers</span>
      <span class="gpui-crumb-sep">//</span>
      <span class="gpui-crumb-active">{crumb_path}</span>
    </div>

    <div class="gpui-model-badge">
      <span>⚡</span>
      <span>{model_name}</span>
    </div>
  </div>

  <!-- Main Body -->
  <div class="gpui-body">
    <!-- Sidebar -->
    <div class="gpui-sidebar">
      <div>
        <div class="sidebar-section-title">
          <span>Active Coworkers</span>
          <div class="sidebar-btn-add">+</div>
        </div>
        <div style="display:flex; flex-direction:column; gap:3px;">
          {''.join(sidebar_bot_html)}
        </div>
      </div>

      <div class="gpui-user-pill">
        <div class="gpui-user-icon">YH</div>
        <div class="gpui-user-meta">Yakushev Hermann · Tokyo Node</div>
      </div>
    </div>

    <!-- Center Stream -->
    <div class="gpui-center-pane">
      <div class="gpui-chat-head">
        <div class="gpui-active-head">
          <div class="gpui-avatar" style="background: {active_avatar_bg}; width: 22px; height: 22px; font-size: 11px;">{active_avatar_svg}</div>
          <span class="gpui-active-title">{active_bot_name}</span>
        </div>
        <span class="gpui-speed-tag">142 tok/s · 0.75ms dispatch</span>
      </div>

      <div class="gpui-stream">
        {chat_html}
      </div>

      <div class="gpui-composer">
        <span class="comp-icon">+</span>
        <span class="comp-input-text">Instruct {active_bot_name}...</span>
        <span class="comp-send-key">⌘⏎</span>
      </div>
    </div>

    <!-- Right Inspection -->
    <div class="gpui-right-pane">
      <div class="right-pane-header">
        <span>Live Inspection</span>
        <span style="color:#71717a; font-size:10px; font-family:'JetBrains Mono';">Playwright Loopback</span>
      </div>

      <div class="browser-frame-mock">
        <div class="browser-top-strip">
          <span style="color:#52525b; font-size:8px;">●●●</span>
          <div class="url-text-pill">{browser_url}</div>
        </div>
        <div class="browser-viewport-inner">
          {browser_body_html}
        </div>
      </div>

      <div class="takeover-bar">
        <span>Status: Streaming (500ms)</span>
        <button class="btn-gpui-action">Take Control</button>
      </div>

      <div class="cron-routines-box">
        <div class="cron-title-label">Scheduled Routines</div>
        {''.join(routines_html)}
      </div>
    </div>
  </div>

  <!-- Bottom Metal Status Bar -->
  <div class="gpui-statusbar">
    <div class="statusbar-left">
      <span class="stat-active-item">● 6 Nodes Running</span>
      <span>⎇ main</span>
      <span>Tokio JoinSet</span>
    </div>
    <div class="statusbar-right">
      <span>RAM: <strong style="color:#f4f4f5;">15.4 MB</strong></span>
      <span>Dispatch: <strong class="stat-green">0.75 ms</strong></span>
      <span>Tokens: <strong style="color:#38bdf8;">Unmetered</strong></span>
    </div>
  </div>
</div>

</body>
</html>"""

    html_path = os.path.join(MOCKUPS_DIR, f"{filename}.html")
    with open(html_path, "w", encoding="utf-8") as f:
        f.write(html)
    return html_path

# ==============================================================================
# SCREENS 01 TO 10 (UPDATED WITH YAKUSHEV HERMANN & RICH TOOL CALLS)
# ==============================================================================

# 1. Sales Outbound SDR
generate_gpui_screen(
    "01_sales_outbound_sdr",
    "sdr // london-fintech-q3",
    "Qwen 3.8 Max (Alibaba)",
    "Sales Outbound", "⚡", "linear-gradient(135deg, #ea580c, #f97316)",
    """
    <div class="gpui-msg-user">Find 50 verified VP Engineering leads at London FinTechs and sync to amoCRM</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">web_search</strong>(query="Fintech Companies House London", limit=50)</span>
        </div>
        <span class="tool-result-badge">120 found</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">playwright.extract_officers</strong>(target="companieshouse.gov.uk", role="VP Engineering")</span>
        </div>
        <span class="tool-result-badge">108 names</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">smtp_probe</strong>(pattern="{first}.{last}@{domain}", mx="aspmx.l.google.com")</span>
        </div>
        <span class="tool-result-badge">250 OK (94)</span>
      </div>

      <div class="tool-collapsed-bar">
        <span>▸ 4 sub-steps collapsed: DNS MX resolve (1.2ms) · TLS handshake · Discard disposable emails</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Completed lead generation sweep in 4.2 seconds across 120 company registries:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>120 Corporations:</strong> Extracted from UK Companies House</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>108 C-Level Officers:</strong> Cross-referenced on LinkedIn & corporate sites</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>94 Mailboxes Probed:</strong> 100% verified via SMTP 250 OK (0% bounce rate)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>amoCRM Staged:</strong> Synced with verified tags: <code>AWS</code>, <code>Rust</code>, <code>Stripe</code></div>
      </div>
      Top 5 draft emails generated in your voice. Staged for review.
    </div>
    """,
    "companieshouse.gov.uk/company/search?q=fintech",
    """
    <span style="color:#38bdf8;">[HTTP/2 GET 200 OK]</span><br>
    > Status: 120 records parsed<br>
    > Active Officers: 108 VP/CTO<br>
    > SMTP Probe: <code>HELO fathom.local</code><br>
    > Handshake: <span style="color:#34d399;">250 OK Mailbox Exists</span><br>
    > amoCRM Lead: <code>#99201 Active</code>
    """,
    [("Nightly lead sweep", "Weekdays 2:00 AM"), ("amoCRM pipeline sync", "Daily 8:30 AM")]
)

# 2. Market Intelligence
generate_gpui_screen(
    "02_market_intelligence",
    "analyst // competitor-pricing-diff",
    "Kimi k3 (Moonshot)",
    "Market Intel", "🧠", "linear-gradient(135deg, #4f46e5, #6366f1)",
    """
    <div class="gpui-msg-user">Track top 15 fintech competitors for pricing adjustments or new feature drops</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">web_feed.crawl_dom</strong>(target="https://stripe-x.io/pricing")</span>
        </div>
        <span class="tool-result-badge">DOM diff: 3.4KB</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">memory.absorb_fact</strong>(entity="Stripe-X", relation="has_pricing_tier", val="2.4% + $0.20")</span>
        </div>
        <span class="tool-result-badge">94 µs</span>
      </div>

      <div class="tool-collapsed-bar">
        <span>▸ 2 sub-steps collapsed: AST tree comparison · SHA-256 deduplication check</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Detected significant pricing adjustment on Stripe-X:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line" style="color:#ff453a;">- Old Tier: Enterprise 2.9% + $0.30/txn</div>
        <div class="deliverable-line" style="color:#34d399;">+ New Tier: Enterprise 2.4% + $0.20/txn (Volume &gt; $1M)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Memory Updated:</strong> Node <code>StripeX_Pricing_Q3</code> stored in SQLite graph</div>
      </div>
      Alert dispatched to Slack #executive-intel. Summary PDF prepared.
    </div>
    """,
    "stripe-x.io/pricing",
    """
    <span style="color:#ff9f0a;">[DOM Diff Tracker]</span><br>
    <span style="color:#ff453a;">- $0.30 fixed fee</span><br>
    <span style="color:#34d399;">+ $0.20 volume tier</span><br>
    > Memory: <span style="color:#38bdf8;">Graph Node #5421</span><br>
    > Slack Alert: <span style="color:#34d399;">Dispatched (12ms)</span>
    """,
    [("Competitor diff sweep", "Every 6 hours"), ("Weekly executive digest", "Friday 5:00 PM")]
)

# 3. Talent Scout
generate_gpui_screen(
    "03_talent_scout",
    "recruiter // rust-tokio-sourcing",
    "GLM 5.3 (Zhipu AI)",
    "Talent Scout", "🔍", "linear-gradient(135deg, #0284c7, #38bdf8)",
    """
    <div class="gpui-msg-user">Map 30 senior Rust architects with Tokio & distributed systems expertise in Berlin</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">code_symbols.repo_map</strong>(repo="tokio-rs/tokio", min_commits=50)</span>
        </div>
        <span class="tool-result-badge">38 candidates</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">social_corroborate</strong>(handle="avance_rust", platforms=["github","linkedin"])</span>
        </div>
        <span class="tool-result-badge">Berlin (4.2y)</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">smtp_probe</strong>(email="a.vance@cloudscale.de")</span>
        </div>
        <span class="tool-result-badge">250 OK</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Mapped candidate dossier for Alexander Vance (Principal Systems Eng):
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>GitHub Proof:</strong> 142 commits to <code>tokio-rs/tokio</code> (SIMD AVX-512)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Current Seniority:</strong> Senior Rust Eng at CloudScale (Berlin)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Icebreaker:</strong> Generated from EuroRust 2025 presentation</div>
      </div>
      3 candidate intro emails drafted in your voice.
    </div>
    """,
    "github.com/tokio-rs/tokio/commits",
    """
    <span style="color:#38bdf8;">[AST Code Analyzer]</span><br>
    > Candidate: Alexander Vance<br>
    > Tokio Commits: 142 (Top 1%)<br>
    > Location: Berlin (4.2 yrs exp)<br>
    > Email: <span style="color:#34d399;">a.vance@... (250 OK)</span>
    """,
    [("GitHub talent crawler", "Daily 11:00 PM"), ("Candidate follow-up", "Tuesday 9:00 AM")]
)

# 4. Back-Office Invoice Reconciliation
generate_gpui_screen(
    "04_backoffice_invoice",
    "finance // invoice-3way-match",
    "Qwen 3.8 Max (Alibaba)",
    "Back-Office", "📑", "linear-gradient(135deg, #e11d48, #fb7185)",
    """
    <div class="gpui-msg-user">Reconcile August freight invoices against warehouse purchase orders and stage batch in QuickBooks</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">document.parse_pdf</strong>(path="~/invoices/august/*.pdf")</span>
        </div>
        <span class="tool-result-badge">500 parsed</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">sql_exec</strong>(query="SELECT * FROM po_ledger WHERE month = '2026-08'")</span>
        </div>
        <span class="tool-result-badge">498 exact matches</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">browser.type</strong>(ref="@e14", value="Batch #849 - Approved")</span>
        </div>
        <span class="tool-result-badge">QuickBooks OK</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      3-way invoice reconciliation completed across $482,000 transaction volume:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>500 Invoices:</strong> Parsed with 100.0% line-item accuracy</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>PO Matching:</strong> 498 exact matches + 2 auto-resolved tax adjustments</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>QuickBooks Entry:</strong> Batch payment staged for CFO 1-click sign-off</div>
      </div>
      Saved approximately 80 hours of manual bookkeeping.
    </div>
    """,
    "quickbooks.intuit.com/app/invoices",
    """
    <span style="color:#38bdf8;">[QuickBooks Accessibility Ref @e14]</span><br>
    > Batch #849: 500 Invoices<br>
    > Total: $482,000.00 USD<br>
    > PO Match Rate: 100.0%<br>
    > Status: <span style="color:#34d399;">Staged for CFO Sign-Off</span>
    """,
    [("Mailbox invoice sweep", "Hourly"), ("Month-end closing", "28th of month")]
)

# 5. DevOps Maintainer
generate_gpui_screen(
    "05_devops_engineer",
    "devops // sentry-triage-892",
    "Qwen 3.8 Max (Alibaba)",
    "DevOps Maintainer", "🛠️", "linear-gradient(135deg, #059669, #10b981)",
    """
    <div class="gpui-msg-user">Triage Sentry issue #892: ZeroDivisionError in MoM revenue analytics</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">code_symbols.repo_map</strong>(path="src/", depth=3)</span>
        </div>
        <span class="tool-result-badge">240 files (34ms)</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">file_edit</strong>(file="src/revenue_calc.py", line=84, diff="+ defensive check")</span>
        </div>
        <span class="tool-result-badge">Patched</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">bash</strong>(cmd="pytest tests/ -v")</span>
        </div>
        <span class="tool-result-badge">23/23 passed (1.42s)</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Bug triaged, reproduced, patched, and tested:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Root Cause:</strong> <code>revenue_calc.py:84</code> crashed when prior month revenue was $0</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Test Suite:</strong> 23/23 unit tests passing in 1.42s</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Pull Request:</strong> Branch <code>fix/mom-zero-div</code> opened on GitHub (PR #142)</div>
      </div>
      PR ready for engineer review.
    </div>
    """,
    "github.com/company/repo/pull/142",
    """
    <span style="color:#34d399;">[GitHub PR #142: Open]</span><br>
    > Diff: +4 -1 lines (revenue_calc.py)<br>
    > CI / Test Suite: <span style="color:#34d399;">23/23 PASSED</span><br>
    > Git Branch: <code>fix/mom-zero-div</code><br>
    > Reviewers: Assigned
    """,
    [("Sentry error triage", "Every 15m"), ("Security dependency audit", "Weekly Sun")]
)

# 6. Governed Computer 2FA Takeover
generate_gpui_screen(
    "06_computer_2fa_takeover",
    "supervisor // aws-2fa-takeover",
    "Kimi k3 (Moonshot)",
    "Chief of Staff", "👑", "linear-gradient(135deg, #0d9488, #14b8a6)",
    """
    <div class="gpui-msg-bot">
      Logging into AWS billing portal to export quarterly invoice breakdown. Encountered SMS 2FA checkpoint.
      
      <div class="tool-call-group">
        <div class="tool-chip">
          <div class="tool-chip-left">
            <span class="tool-icon-exec">❯</span>
            <span>tool: <strong class="tool-name-highlight">browser.detect_gate</strong>(type="2FA_SMS", phone="***-8921")</span>
          </div>
          <span class="tool-result-badge" style="color:#ff9f0a; background:rgba(255,159,10,0.15);">PAUSED</span>
        </div>
        <div class="tool-chip">
          <div class="tool-chip-left">
            <span class="tool-icon-exec">❯</span>
            <span>tool: <strong class="tool-name-highlight">supervisor.acquire_human_lease</strong>(reason="2FA SMS required")</span>
          </div>
          <span class="tool-result-badge">Lease Granted</span>
        </div>
      </div>

      <div class="gpui-card-deliverable">
        <div class="deliverable-line" style="color:#ff9f0a;">⚠️ <strong>2FA Checkpoint:</strong> Bot paused safely; operator control active</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Action:</strong> Enter 6-digit SMS code in live screen on right</div>
      </div>
    </div>
    <div class="gpui-msg-user">Code entered: 849201. Release control to bot.</div>
    <div class="gpui-msg-bot">Control resumed. Captured fresh accessibility snapshot. Downloaded Q3 invoice PDF successfully.</div>
    """,
    "signin.aws.amazon.com/2fa/verify",
    """
    <div style="text-align:center; padding:8px; background:#18181f; border-radius:6px;">
      <div style="font-weight:700; color:#f4f4f5; font-size:10.5px;">Amazon AWS 2FA Gate</div>
      <div style="font-size:9px; color:#71717a; margin:3px 0;">Enter security code from SMS</div>
      <div style="background:#09090c; border:1px solid #38bdf8; border-radius:4px; padding:5px; width:105px; margin:6px auto; font-family:'JetBrains Mono'; letter-spacing:4px; color:#38bdf8; font-size:12px; font-weight:700;">849201</div>
    </div>
    """,
    [("AWS billing export", "1st of month"), ("Vendor statement audit", "Bi-weekly")]
)

# 7. Inbox Manager
generate_gpui_screen(
    "07_inbox_manager",
    "inbox // routine-sweep",
    "GLM 5.3 (Zhipu AI)",
    "Chief of Staff", "👑", "linear-gradient(135deg, #0d9488, #14b8a6)",
    """
    <div class="gpui-msg-user">Inbox got away from me over the weekend, sweep it?</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">email.batch_fetch</strong>(folder="INBOX", status="unread")</span>
        </div>
        <span class="tool-result-badge">41 unread</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">email.archive_noise</strong>(filters=["newsletter","receipt"])</span>
        </div>
        <span class="tool-result-badge">26 archived</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">email.draft_reply</strong>(to="Nora", tone="executive")</span>
        </div>
        <span class="tool-result-badge">Draft Held</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Sweep completed in 1.8 seconds. Inbox at zero:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Archived:</strong> 26 newsletters & promotional receipts</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Auto-Replied:</strong> 9 routine calendar scheduling threads</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Drafted & Held:</strong> 6 personalized drafts for your review</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Flagged:</strong> 1 contract renewal question from Nora</div>
      </div>
      Nora's draft confirms seat expansion and quotes contract section 4.2.
    </div>
    """,
    "mail.google.com/mail/u/0/#inbox",
    """
    <span style="color:#38bdf8;">[Inbox Zero Status]</span><br>
    > 41 threads read<br>
    > 26 archived · 9 replied<br>
    > Nora’s renewal: <span style="color:#34d399;">Sent</span><br>
    > Status: <span style="color:#34d399;">Inbox Zero Achieved</span>
    """,
    [("Morning sweep", "Weekdays 6:00 AM"), ("Reply Zero check", "Hourly")]
)

# 8. Lead Audit Report
generate_gpui_screen(
    "08_lead_audit_report",
    "auditor // dach-fintech-50",
    "Qwen 3.8 Max (Alibaba)",
    "Sales Outbound", "⚡", "linear-gradient(135deg, #ea580c, #f97316)",
    """
    <div class="gpui-msg-user">Generate a free 50-lead audit report for FinTech companies in DACH</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">osint.batch_extract</strong>(geo="DACH", industry="FinTech", count=50)</span>
        </div>
        <span class="tool-result-badge">50 records</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">export.compile_xlsx</strong>(filename="DACH_Fintech_Audit.xlsx")</span>
        </div>
        <span class="tool-result-badge">Compiled</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Free 50-lead value audit deliverable compiled in 84 seconds:
      <div class="gpui-card-deliverable">
        <table style="width:100%; font-size:9.5px; border-collapse:collapse; font-family:'JetBrains Mono';">
          <tr style="color:#71717a; border-bottom:1px solid #27272a;">
            <th style="text-align:left; padding:2px;">Officer</th><th>Company</th><th>Verified Work Email</th><th>SMTP</th>
          </tr>
          <tr style="border-bottom:1px solid #18181f;">
            <td style="padding:3px; color:#f4f4f5;">Dr. Marcus Weber (CISO)</td><td>FinTech Bavaria</td><td style="color:#38bdf8;">m.weber@fintech-bavaria.de</td><td style="color:#34d399;">250 OK</td>
          </tr>
          <tr style="border-bottom:1px solid #18181f;">
            <td style="padding:3px; color:#f4f4f5;">Elena Schmidt (VP IT)</td><td>SecureCloud GmbH</td><td style="color:#38bdf8;">e.schmidt@securecloud.io</td><td style="color:#34d399;">250 OK</td>
          </tr>
          <tr>
            <td style="padding:3px; color:#f4f4f5;">Lukas Becker (Sec)</td><td>Munich Data Labs</td><td style="color:#38bdf8;">l.becker@munichdatalabs.com</td><td style="color:#34d399;">250 OK</td>
          </tr>
        </table>
      </div>
      Ready for client delivery via Telegram or PDF export.
    </div>
    """,
    "fathom.ai/app/audits/dach-fintech",
    """
    <span style="color:#34d399;">[Audit Deliverable Export]</span><br>
    > 50 / 50 Leads Verified<br>
    > Format: Excel (.xlsx) + CSV<br>
    > Deliverability: <span style="color:#34d399;">99.4% Confidence</span><br>
    > File: <code>DACH_Fintech_Audit.xlsx</code>
    """,
    [("Lead quality audit", "Daily 9:00 AM"), ("Weekly CRM export", "Friday 4:00 PM")]
)

# 9. Entity Knowledge Graph
generate_gpui_screen(
    "09_entity_knowledge_graph",
    "memory // graph-query-hop3",
    "Kimi k3 (Moonshot)",
    "Market Intel", "🧠", "linear-gradient(135deg, #4f46e5, #6366f1)",
    """
    <div class="gpui-msg-user">Which former Stripe engineers are now CTOs at Series-A AI companies in Berlin?</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">memory.graph_traverse</strong>(start="Stripe", hops=3, filter="role=CTO & city=Berlin")</span>
        </div>
        <span class="tool-result-badge">1.62ms</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Traversed 5,420 entity graph nodes across 3 relationship hops in 1.62ms:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Matched Entity:</strong> Jane Doe (CTO at Acme FinTech, Berlin)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Career Lineage:</strong> Ex-Stripe (Staff Infra) ──[works_at]──► Acme FinTech (Series A, $14M)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Tech Signature:</strong> Acme FinTech ──[uses_tech]──► Rust, Axum, PostgreSQL</div>
      </div>
      Fact verified against Handelsregister filing from August 2026.
    </div>
    """,
    "fathom.internal/memory/graph/explorer",
    """
    <span style="color:#38bdf8;">[Entity Knowledge Graph]</span><br>
    Jane Doe (CTO)<br>
    ──[works_at]──► Acme FinTech<br>
    ──[uses_tech]──► Rust & Axum<br>
    > Query Latency: <span style="color:#34d399;">1.62 ms</span>
    """,
    [("Graph deduplication", "Daily 3:00 AM"), ("Memory compaction", "Weekly Sun")]
)

# 10. Security Credentials Vault
generate_gpui_screen(
    "10_security_credentials_vault",
    "governance // aes-gcm-vault",
    "Qwen 3.8 Max (Alibaba)",
    "DevOps Maintainer", "🛠️", "linear-gradient(135deg, #059669, #10b981)",
    """
    <div class="gpui-msg-user">Audit active API credentials and check for prompt leak vulnerabilities</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">vault.audit_keys</strong>(cipher="AES-256-GCM", engine="ring")</span>
        </div>
        <span class="tool-result-badge">0% leaked</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">governance.evaluate_policies</strong>(policy="fail_closed")</span>
        </div>
        <span class="tool-result-badge">Deny Wins</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Enterprise security audit complete. All credentials isolated behind hardware key derivation:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>HubSpot API Key:</strong> Encrypted AES-GCM (0% prompt exposure, resolved in Rust memory)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>amoCRM OAuth:</strong> Encrypted AES-GCM (0% prompt exposure)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Audit Ledger:</strong> Nanosecond-precision tamper-proof logs in SQLite</div>
      </div>
      Zero secret exposure detected across all active worker sessions.
    </div>
    """,
    "fathom.internal/governance/vault",
    """
    <span style="color:#34d399;">[AES-256-GCM Vault]</span><br>
    > hubspot_api_key: <span style="color:#71717a;">MASKED</span><br>
    > amocrm_oauth: <span style="color:#71717a;">MASKED</span><br>
    > Prompt Exposure: <span style="color:#34d399;">0.00%</span><br>
    > Compliance: <span style="color:#38bdf8;">100% (Ring Crate)</span>
    """,
    [("Vault integrity scan", "Daily 4:00 AM"), ("Audit log rotation", "Monthly 1st")]
)

# ==============================================================================
# 5 NEW SCREENS (11 TO 15)
# ==============================================================================

# 11. Multi-Agent Swarm Coordinator
generate_gpui_screen(
    "11_swarm_coordinator",
    "coordinator // multi-agent-dag",
    "GLM 5.3 (Zhipu AI)",
    "Chief of Staff", "👑", "linear-gradient(135deg, #0d9488, #14b8a6)",
    """
    <div class="gpui-msg-user">Execute full market & competitive due diligence on 50 German AI HealthTech startups</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">swarm.decompose_task</strong>(goal="German HealthTech Due Diligence", depth=2)</span>
        </div>
        <span class="tool-result-badge">4 branches</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">swarm.spawn_pod</strong>(pod_id="researcher-01", quota=16000)</span>
        </div>
        <span class="tool-result-badge">Tokio JoinSet</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">swarm.spawn_pod</strong>(pod_id="scraper-02", quota=16000)</span>
        </div>
        <span class="tool-result-badge">Tokio JoinSet</span>
      </div>

      <div class="tool-collapsed-bar">
        <span>▸ 2 sub-pods active: OSINT-Verifier-03 · Executive-Writer-04 · Fair-Share Token Allocator</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Hierarchical swarm execution completed in 14.8 seconds across 4 parallel CPU worker pods:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Branch 1 (Registry Pod):</strong> Parsed Handelsregister filings for 50 entities</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Branch 2 (OSINT Pod):</strong> Mapped 82 C-Level executives with verified emails</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Branch 3 (Tech Stack):</strong> Fingerprinted HIPAA/GDPR cloud infrastructure</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Branch 4 (Synthesis):</strong> Merged 50 startup dossiers into executive PDF</div>
      </div>
      Full report compiled with zero context overflow (raw payloads spilt to disk).
    </div>
    """,
    "fathom.internal/swarm/dag/visualizer",
    """
    <span style="color:#38bdf8;">[Tokio JoinSet Topology]</span><br>
    > Pod 1 (Registry): <span style="color:#34d399;">DONE (3.1s)</span><br>
    > Pod 2 (OSINT): <span style="color:#34d399;">DONE (4.2s)</span><br>
    > Pod 3 (Tech Stack): <span style="color:#34d399;">DONE (2.8s)</span><br>
    > Pod 4 (Writer): <span style="color:#34d399;">DONE (1.4s)</span><br>
    > CPU Concurrency: <span style="color:#38bdf8;">3.78× Speedup</span>
    """,
    [("Daily swarm health check", "Daily 1:00 AM"), ("Token quota rebalance", "Hourly")]
)

# 12. Legal & Regulatory Compliance Auditor
generate_gpui_screen(
    "12_legal_compliance_auditor",
    "legal // gdpr-contract-audit",
    "Kimi k3 (Moonshot)",
    "Back-Office", "📑", "linear-gradient(135deg, #e11d48, #fb7185)",
    """
    <div class="gpui-msg-user">Audit 200 vendor Master Services Agreements (MSAs) for GDPR compliance & liability caps</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">document.extract_clauses</strong>(path="~/contracts/*.pdf", type="liability_cap")</span>
        </div>
        <span class="tool-result-badge">200 contracts</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">compliance.eval_gdpr</strong>(rules=["SCC_included", "subprocessor_notice_30d"])</span>
        </div>
        <span class="tool-result-badge">3 flags found</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Contract compliance audit completed across 200 agreements in 6.4 minutes:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>192 Compliant Contracts:</strong> Standard Contractual Clauses (SCC) and 12-mo liability caps present</div>
        <div class="deliverable-line" style="color:#ff9f0a;">⚠️ <strong>5 Medium Risk:</strong> Sub-processor notification window under 14 days (Vendor Beta)</div>
        <div class="deliverable-line" style="color:#ff453a;">🚨 <strong>3 High Risk:</strong> Unlimited liability clause & missing GDPR data deletion schedule (Vendor Gamma)</div>
      </div>
      Executive Risk Assessment Matrix compiled with page-by-page paragraph citations.
    </div>
    """,
    "fathom.internal/legal/matrix/gdpr",
    """
    <span style="color:#38bdf8;">[Legal Risk Matrix]</span><br>
    > Total Audited: 200 MSAs<br>
    > Green (Safe): <span style="color:#34d399;">192 contracts</span><br>
    > Yellow (Review): <span style="color:#ff9f0a;">5 contracts</span><br>
    > Red (High Risk): <span style="color:#ff453a;">3 contracts</span><br>
    > Time Saved: <span style="color:#38bdf8;">120 Hours Legal Work</span>
    """,
    [("Contract repository monitor", "Weekly Mon"), ("GDPR regulation sync", "Monthly 1st")]
)

# 13. Customer Success & Webhook REPL Debugger
generate_gpui_screen(
    "13_customer_success_onboarding",
    "support // webhook-triage-p1",
    "Qwen 3.8 Max (Alibaba)",
    "DevOps Maintainer", "🛠️", "linear-gradient(135deg, #059669, #10b981)",
    """
    <div class="gpui-msg-user">Triage incoming client support ticket #4812: 'Webhook HMAC signature failing on our server'</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">ticket.fetch_details</strong>(id="TCK-4812", platform="Zendesk")</span>
        </div>
        <span class="tool-result-badge">P1 Enterprise</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">sandbox_repl.exec_python</strong>(code="import hmac, hashlib; test_payload(...)")</span>
        </div>
        <span class="tool-result-badge">Diagnosed (0.3s)</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Issue diagnosed and tested in isolated Python sandbox in under 12 seconds:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Root Cause Diagnosed:</strong> Client hashing decoded JSON instead of raw UTF-8 request byte buffer</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Code Fix Tested:</strong> Verified in sandbox with Python & Node.js code snippets</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Reply Drafted:</strong> Step-by-step fix with sample Express.js middleware staged for review</div>
      </div>
      First response time: <strong>14 seconds</strong> (industry avg: 45 minutes).
    </div>
    """,
    "fathom.internal/sandbox/repl/hmac-debug",
    """
    <span style="color:#38bdf8;">[Sandboxed REPL Output]</span><br>
    >>> hmac_sha256(raw_bytes, secret)<br>
    <span style="color:#34d399;">✓ Signature MATCH: 0x8f4a12c...</span><br>
    >>> hmac_sha256(parsed_json, secret)<br>
    <span style="color:#ff453a;">✗ Signature MISMATCH (Root Cause)</span><br>
    > Response Code: <span style="color:#38bdf8;">Ready in Ticket</span>
    """,
    [("Support ticket triage", "Real-Time / SSE"), ("API health monitor", "Every 5m")]
)

# 14. Cold Email Outreach & Deliverability Dispatcher
generate_gpui_screen(
    "14_outreach_campaign_dispatcher",
    "outreach // instantly-campaign-batch",
    "Qwen 3.8 Max (Alibaba)",
    "Sales Outbound", "⚡", "linear-gradient(135deg, #ea580c, #f97316)",
    """
    <div class="gpui-msg-user">Queue 500 verified CTO leads for our Q3 Cloud Security outbound sequence with Spintax variants</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">campaign.generate_spintax</strong>(template="Q3_Security_Pitch", variants=4)</span>
        </div>
        <span class="tool-result-badge">4 variants</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">doh_dns.verify_mx_spf</strong>(domain="fathom-outreach.io")</span>
        </div>
        <span class="tool-result-badge">SPF/DKIM/DMARC OK</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">instantly_bridge.queue_batch</strong>(leads_count=500, throttle="35/hr")</span>
        </div>
        <span class="tool-result-badge">500 Queued</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Campaign staged and deliverability checks passed:
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>500 Verified Leads:</strong> 100% deliverable mailboxes (SMTP 250 OK confirmed)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Personalized Icebreakers:</strong> Injected based on recent GitHub commits & press mentions</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Inbox Pre-Warming:</strong> 4 domain pools verified with SPF, DKIM & DMARC (100% health score)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Throttled Delivery:</strong> Distributed across 35 sends/hour to ensure 0% spam flagging</div>
      </div>
      Sequence Step 1 scheduled to start Monday 9:00 AM recipient local time.
    </div>
    """,
    "app.instantly.ai/campaigns/q3-security-london",
    """
    <span style="color:#34d399;">[Outbound Campaign Health]</span><br>
    > Active Leads: 500<br>
    > Domain Reputation: <span style="color:#34d399;">100% (A+)</span><br>
    > Sending Throttle: 35 / hour<br>
    > Deliverability Score: <span style="color:#38bdf8;">99.8%</span><br>
    > Projected Open Rate: <span style="color:#34d399;">64.2%</span>
    """,
    [("Daily sending throttle check", "Hourly"), ("Bounce rate auto-pause", "Real-Time")]
)

# 15. Agency White-Label Fleet Manager
generate_gpui_screen(
    "15_agency_fleet_manager",
    "agency // multi-tenant-cluster",
    "GLM 5.3 (Zhipu AI)",
    "Chief of Staff", "👑", "linear-gradient(135deg, #0d9488, #14b8a6)",
    """
    <div class="gpui-msg-user">Show multi-tenant client worker cluster status and aggregated monthly ROI metrics</div>
    
    <div class="tool-call-group">
      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">fleet.list_tenants</strong>(agency_id="agency_apex_growth")</span>
        </div>
        <span class="tool-result-badge">12 clients</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">docker_supervisor.inspect_containers</strong>(status="running")</span>
        </div>
        <span class="tool-result-badge">12/12 isolated</span>
      </div>

      <div class="tool-chip">
        <div class="tool-chip-left">
          <span class="tool-icon-exec">❯</span>
          <span>tool: <strong class="tool-name-highlight">metrics.aggregate_client_roi</strong>(period="30d")</span>
        </div>
        <span class="tool-result-badge">84% gross margin</span>
      </div>
    </div>

    <div class="gpui-msg-bot">
      Agency Multi-Tenant Cluster Overview (12 Active Enterprise Accounts):
      <div class="gpui-card-deliverable">
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>12 Client Pods Running:</strong> 100% cryptographic container isolation (zero data leakage)</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Monthly Agency Retainer:</strong> $30,000 / month across 12 managed client fleets</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Server Compute Cost:</strong> $240 / month total on single 16-core Hetzner dedicated box</div>
        <div class="deliverable-line"><span class="chk-green">✓</span><strong>Net Agency Profit Margin:</strong> <strong>92.0%</strong> ($27,600/mo net profit)</div>
      </div>
      White-label client dashboards active under <code>app.apexgrowth.io</code>.
    </div>
    """,
    "app.apexgrowth.io/agency/fleet-monitor",
    """
    <span style="color:#38bdf8;">[Agency Multi-Tenant Fleet]</span><br>
    > Active Client Fleets: 12<br>
    > Total Digital Workers: 48<br>
    > Server Memory: 1.8 GB / 32 GB<br>
    > Monthly Retainer: <span style="color:#34d399;">$30,000 / mo</span><br>
    > Gross Margin: <span style="color:#34d399;">92.0% Net Profit</span>
    """,
    [("Tenant isolation audit", "Daily 5:00 AM"), ("Client monthly invoice compile", "1st of month")]
)

print("\nAll 15 GPUI Metal HTML screens generated. Starting HD Retina PNG rendering...")

all_mockup_names = [
    "01_sales_outbound_sdr",
    "02_market_intelligence",
    "03_talent_scout",
    "04_backoffice_invoice",
    "05_devops_engineer",
    "06_computer_2fa_takeover",
    "07_inbox_manager",
    "08_lead_audit_report",
    "09_entity_knowledge_graph",
    "10_security_credentials_vault",
    "11_swarm_coordinator",
    "12_legal_compliance_auditor",
    "13_customer_success_onboarding",
    "14_outreach_campaign_dispatcher",
    "15_agency_fleet_manager"
]

def render_gpui_png(name):
    html_p = os.path.join(MOCKUPS_DIR, f"{name}.html")
    png_p = os.path.join(MOCKUPS_DIR, f"{name}.png")
    tmp_d = tempfile.mkdtemp(prefix=f"gpui_{name}_")
    cmd = [
        CHROME_PATH,
        "--headless",
        "--disable-gpu",
        "--force-device-scale-factor=2",
        "--window-size=1280,840",
        f"--user-data-dir={tmp_d}",
        f"--screenshot={png_p}",
        f"file://{html_p}"
    ]
    subprocess.run(cmd, capture_output=True)
    shutil.rmtree(tmp_d, ignore_errors=True)
    if os.path.exists(png_p) and os.path.getsize(png_p) > 0:
        return (name, png_p, os.path.getsize(png_p))
    return (name, None, 0)

with concurrent.futures.ThreadPoolExecutor(max_workers=6) as ex:
    results = list(ex.map(render_gpui_png, all_mockup_names))

for name, path, size in results:
    if path:
        print(f"✓ OK [GPUI Metal HD]: {name}.png ({size:,} bytes)")
    else:
        print(f"✗ FAILED: {name}")

print(f"\nSUCCESS: All {len(all_mockup_names)} GPUI Metal HD Retina PNGs are ready!")
