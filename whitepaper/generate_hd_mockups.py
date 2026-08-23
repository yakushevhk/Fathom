#!/usr/bin/env python3
import os, subprocess, tempfile, shutil

MOCKUPS_DIR = "/Users/yakushev/Documents/GitHub/Fathom/whitepaper/mockups"
os.makedirs(MOCKUPS_DIR, exist_ok=True)
CHROME_PATH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

COMMON_CSS = """
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600;700&display=swap');

* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: #08080a;
  color: #ececee;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  padding: 30px;
  -webkit-font-smoothing: antialiased;
}

.app-window {
  width: 1200px;
  height: 760px;
  background: #0d0d11;
  border: 1px solid #23232a;
  border-radius: 14px;
  box-shadow: 0 30px 80px rgba(0, 0, 0, 0.75), 0 0 0 1px rgba(255, 255, 255, 0.07);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Chrome Window Bar */
.window-topbar {
  background: #141418;
  padding: 12px 18px;
  border-bottom: 1px solid #202026;
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.traffic-dots { display: flex; gap: 7.5px; align-items: center; }
.tdot { width: 12px; height: 12px; border-radius: 50%; display: inline-block; }
.tdot-red { background: #ff5f56; border: 1px solid #e0443e; }
.tdot-yellow { background: #ffbd2e; border: 1px solid #dea123; }
.tdot-green { background: #27c93f; border: 1px solid #1aab29; }
.window-app-title { font-size: 11.5px; font-weight: 600; color: #8e8e93; font-family: 'JetBrains Mono', monospace; }
.window-live-pill {
  font-size: 10.5px;
  font-weight: 700;
  color: #30d158;
  font-family: 'JetBrains Mono', monospace;
  background: rgba(48, 209, 88, 0.1);
  border: 1px solid rgba(48, 209, 88, 0.25);
  padding: 2px 8px;
  border-radius: 20px;
}

/* 3-Pane Body */
.window-main-body {
  display: grid;
  grid-template-columns: 245px 1fr 315px;
  flex: 1;
  background: #09090c;
  overflow: hidden;
}

/* Left Sidebar */
.sidebar-pane {
  background: #101014;
  border-right: 1px solid #1c1c22;
  padding: 14px 12px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 10px;
}
.sidebar-title-bar { display: flex; justify-content: space-between; align-items: center; font-size: 12.5px; font-weight: 700; color: #ececee; }
.sidebar-plus-btn {
  width: 20px;
  height: 20px;
  background: #1a1a20;
  border: 1px solid #282830;
  border-radius: 5px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: #a1a1aa;
  cursor: pointer;
}
.sidebar-search-box {
  background: #09090c;
  border: 1px solid #23232a;
  border-radius: 7px;
  padding: 7px 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  color: #71717a;
  font-size: 11px;
}
.sidebar-bot-list { display: flex; flex-direction: column; gap: 4px; overflow-y: auto; }
.sidebar-bot-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7.5px 9px;
  border-radius: 9px;
  cursor: pointer;
  transition: all 0.15s;
}
.sidebar-bot-card.is-active {
  background: #1c1c22;
  border-left: 3px solid #6366f1;
}
.bot-avatar-box {
  width: 34px;
  height: 34px;
  border-radius: 9px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  flex-shrink: 0;
  box-shadow: 0 2px 6px rgba(0,0,0,0.3);
}
.bot-card-info { display: flex; flex-direction: column; overflow: hidden; flex: 1; }
.bot-card-top { display: flex; justify-content: space-between; align-items: center; }
.bot-card-name { font-size: 11.5px; font-weight: 700; color: #ececee; }
.bot-card-time { font-size: 9.5px; color: #71717a; font-family: 'JetBrains Mono', monospace; }
.bot-card-preview { font-size: 10px; color: #8e8e93; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 1px; }

.sidebar-user-footer {
  display: flex;
  align-items: center;
  gap: 9px;
  border-top: 1px solid #1c1c22;
  padding-top: 10px;
}
.sidebar-user-avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: linear-gradient(135deg, #3f3f46, #27272a);
  border: 1px solid #52525b;
  color: #ececee;
  font-size: 10.5px;
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
}
.sidebar-user-name { font-size: 11.5px; font-weight: 600; color: #ececee; }

/* Center Chat Pane */
.center-chat-pane {
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  background: #09090c;
  overflow: hidden;
}
.chat-header-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #1a1a20;
  padding-bottom: 10px;
}
.chat-active-profile { display: flex; align-items: center; gap: 9px; }
.chat-active-name { font-size: 13.5px; font-weight: 700; color: #ececee; }
.chat-dropdown-icon { font-size: 10px; color: #71717a; }
.chat-computer-toggle {
  width: 30px;
  height: 30px;
  background: #141418;
  border: 1px solid #23232a;
  border-radius: 7px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  cursor: pointer;
}

.chat-message-thread {
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
  padding: 12px 0;
}
.msg-date-divider {
  text-align: center;
  font-size: 9.5px;
  color: #71717a;
  font-family: 'JetBrains Mono', monospace;
  margin: 4px 0;
}
.msg-user-bubble {
  background: #1c1c22;
  color: #ececee;
  padding: 9px 14px;
  border-radius: 14px 14px 4px 14px;
  align-self: flex-end;
  font-size: 12px;
  max-width: 85%;
  line-height: 1.45;
}
.msg-bot-bubble {
  background: #141418;
  border: 1px solid #23232a;
  color: #d4d4d8;
  padding: 11px 15px;
  border-radius: 14px 14px 14px 4px;
  align-self: flex-start;
  font-size: 12px;
  width: 98%;
  line-height: 1.5;
}
.msg-task-card {
  background: #0d0d11;
  border: 1px solid #26262e;
  border-radius: 9px;
  padding: 10px 14px;
  margin: 8px 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.task-row { display: flex; align-items: center; gap: 9px; font-size: 11.5px; color: #d4d4d8; }
.task-chk { color: #30d158; font-weight: 800; font-size: 13px; }
.task-arr { color: #71717a; }

.chat-composer-box {
  background: #131317;
  border: 1px solid #23232a;
  border-radius: 24px;
  padding: 7px 14px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: 11.5px;
}
.composer-plus { color: #71717a; font-size: 16px; font-weight: 700; cursor: pointer; }
.composer-placeholder { color: #71717a; flex: 1; }
.composer-send-btn {
  background: #27272a;
  color: #ececee;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
}

/* Right Computer Pane */
.right-computer-pane {
  background: #101014;
  border-left: 1px solid #1c1c22;
  padding: 14px 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.comp-pane-head { display: flex; justify-content: space-between; align-items: center; color: #ececee; font-size: 12px; font-weight: 700; }
.comp-viewport-window {
  background: #09090c;
  border: 1px solid #23232a;
  border-radius: 9px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 4px 12px rgba(0,0,0,0.4);
}
.comp-browser-topbar {
  background: #16161c;
  padding: 6px 10px;
  display: flex;
  align-items: center;
  gap: 7px;
  border-bottom: 1px solid #23232a;
}
.comp-window-dots { display: flex; gap: 4.5px; }
.comp-wdot { width: 6.5px; height: 6.5px; border-radius: 50%; background: #52525b; }
.comp-url-field {
  background: #09090c;
  color: #38bdf8;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  padding: 2.5px 8px;
  border-radius: 5px;
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.comp-browser-viewport { padding: 10px; font-size: 10.5px; color: #d4d4d8; min-height: 105px; line-height: 1.45; }

.comp-screen-footer { display: flex; justify-content: space-between; align-items: center; font-size: 10.5px; color: #8e8e93; margin-top: 2px; }
.btn-take-control {
  background: #18181c;
  border: 1px solid #3f3f46;
  color: #ececee;
  border-radius: 8px;
  padding: 4px 10px;
  font-size: 10.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
}
.btn-take-control:hover {
  background: #27272a;
  border-color: #52525b;
}

.comp-routines-section { display: flex; flex-direction: column; gap: 5px; margin-top: 4px; }
.routines-header-title { font-size: 10px; font-weight: 700; color: #71717a; text-transform: uppercase; letter-spacing: 0.05em; }
.routine-card-item {
  background: #141418;
  border: 1px solid #202026;
  border-radius: 7px;
  padding: 5px 9px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 10.5px;
}
.routine-name-text { color: #e4e4e7; font-weight: 600; display: flex; align-items: center; gap: 6px; }
.routine-time-text { color: #38bdf8; font-family: 'JetBrains Mono', monospace; font-size: 9.5px; }
.routine-add-link { color: #71717a; font-size: 10px; cursor: pointer; padding: 2px 4px; }
"""

def create_hd_mockup(filename, window_title, status_badge, active_bot_name, active_bot_avatar, active_bot_gradient, chat_html, browser_url, browser_html, routines_list):
    bots = [
        ("Chief of Staff", "linear-gradient(135deg, #0d9488, #14b8a6)", "👑", "venue booked, contracts queued", "Yesterday"),
        ("Sales Outbound", "linear-gradient(135deg, #ea580c, #f97316)", "⚡", "40 accounts researched, 18 queued", "3:10 AM"),
        ("Market Intel", "linear-gradient(135deg, #4f46e5, #6366f1)", "🧠", "competitor pricing diff alert", "12:11 AM"),
        ("Talent Scout", "linear-gradient(135deg, #0284c7, #38bdf8)", "🔍", "30 senior Rust engineers mapped", "Yesterday"),
        ("Back-Office", "linear-gradient(135deg, #e11d48, #fb7185)", "📑", "500 invoices matched, 0 errors", "Monday"),
        ("DevOps Maintainer", "linear-gradient(135deg, #059669, #10b981)", "🛠️", "23/23 tests passing, PR #142", "2:45 PM"),
    ]

    sidebar_cards = []
    for bname, bbg, bsvg, bprev, btime in bots:
        is_active = " is-active" if bname == active_bot_name else ""
        sidebar_cards.append(f"""
        <div class="sidebar-bot-card{is_active}">
          <div class="bot-avatar-box" style="background: {bbg};">{bsvg}</div>
          <div class="bot-card-info">
            <div class="bot-card-top">
              <span class="bot-card-name">{bname}</span>
              <span class="bot-card-time">{btime}</span>
            </div>
            <div class="bot-card-preview">{bprev}</div>
          </div>
        </div>""")

    routines_cards = []
    for rname, rtime in routines_list:
        routines_cards.append(f"""
        <div class="routine-card-item">
          <span class="routine-name-text"><span style="color:#71717a;">◷</span> {rname}</span>
          <span class="routine-time-text">{rtime}</span>
        </div>""")

    html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{window_title}</title>
<style>{COMMON_CSS}</style>
</head>
<body>

<div class="app-window">
  <!-- Topbar -->
  <div class="window-topbar">
    <div class="traffic-dots">
      <span class="tdot tdot-red"></span>
      <span class="tdot tdot-yellow"></span>
      <span class="tdot tdot-green"></span>
      <span class="window-app-title">fathom-control-plane · {window_title}</span>
    </div>
    <span class="window-live-pill">{status_badge}</span>
  </div>

  <!-- Body -->
  <div class="window-main-body">
    <!-- Sidebar -->
    <div class="sidebar-pane">
      <div>
        <div class="sidebar-title-bar">
          <span>Bots</span>
          <div class="sidebar-plus-btn">+</div>
        </div>
        <div class="sidebar-search-box" style="margin: 10px 0 12px 0;">
          <span>⌕</span>
          <span>Search bots...</span>
        </div>
        <div class="sidebar-bot-list">
          {''.join(sidebar_cards)}
        </div>
      </div>

      <div class="sidebar-user-footer">
        <div class="sidebar-user-avatar">AK</div>
        <span class="sidebar-user-name">Avery Kim</span>
      </div>
    </div>

    <!-- Center Chat -->
    <div class="center-chat-pane">
      <div class="chat-header-bar">
        <div class="chat-active-profile">
          <div class="bot-avatar-box" style="background: {active_bot_gradient}; width: 26px; height: 26px; font-size: 13px;">{active_bot_avatar}</div>
          <span class="chat-active-name">{active_bot_name}</span>
          <span class="chat-dropdown-icon">▾</span>
        </div>
        <div class="chat-computer-toggle">💻</div>
      </div>

      <div class="chat-message-thread">
        {chat_html}
      </div>

      <div class="chat-composer-box">
        <span class="composer-plus">+</span>
        <span class="composer-placeholder">Message {active_bot_name}...</span>
        <span class="composer-send-btn">↑</span>
      </div>
    </div>

    <!-- Right Computer -->
    <div class="right-computer-pane">
      <div class="comp-pane-head">
        <span>{active_bot_name}’s computer</span>
        <span style="color: #71717a; cursor: pointer;">⚙</span>
      </div>

      <div class="comp-viewport-window">
        <div class="comp-browser-topbar">
          <div class="comp-window-dots"><span class="comp-wdot"></span><span class="comp-wdot"></span><span class="comp-wdot"></span></div>
          <div class="comp-url-field">{browser_url}</div>
        </div>
        <div class="comp-browser-viewport">
          {browser_html}
        </div>
      </div>

      <div class="comp-screen-footer">
        <span>{active_bot_name}’s screen</span>
        <button class="btn-take-control">Take control</button>
      </div>

      <div class="comp-routines-section">
        <div class="routines-header-title">Routines</div>
        {''.join(routines_cards)}
        <div class="routine-add-link">+ New routine</div>
      </div>
    </div>
  </div>
</div>

</body>
</html>"""

    html_file = os.path.join(MOCKUPS_DIR, f"{filename}.html")
    with open(html_file, "w", encoding="utf-8") as f:
        f.write(html_content)
    return html_file

# ==============================================================================
# DEFINING 10 COMPLETE HIGH-FIDELITY MOCKUP SCENARIOS
# ==============================================================================

# 1. Sales Outbound SDR
create_hd_mockup(
    "01_sales_outbound_sdr",
    "Sales Outbound SDR · Lead Discovery & SMTP Gauntlet",
    "● 94 VERIFIED LEADS · amoCRM SYNCED",
    "Sales Outbound", "⚡", "linear-gradient(135deg, #ea580c, #f97316)",
    """
    <div class="msg-date-divider">Yesterday 11:30 PM</div>
    <div class="msg-user-bubble">Find 50 verified VP Engineering leads in London fintechs & stage in amoCRM with tech tags</div>
    <div class="msg-bot-bubble">
      On it. Decomposed search across Companies House UK and executive registries. Running SMTP 250 OK handshakes:
      <div class="msg-task-card">
        <div class="task-row"><span class="task-chk">✓</span><strong>Companies House:</strong><span class="task-arr">→</span><span>120 active London FinTech corporations parsed</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Executive Harvesting:</strong><span class="task-arr">→</span><span>108 VP Engineering & CTO names identified</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>SMTP 250 OK Probes:</strong><span class="task-arr">→</span><span>94 direct mailboxes verified deliverable (0% bounce)</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>amoCRM Synced:</strong><span class="task-arr">→</span><span>94 deal cards created with tech tags (AWS, Rust, Stripe)</span></div>
      </div>
      Top 5 draft emails generated in your voice. Ready for review.
    </div>
    <div class="msg-user-bubble">Looks great, approve all 94 for morning outbound</div>
    <div class="msg-bot-bubble">Approved. Staged in amoCRM under 'Q3 Enterprise Outbound'. Scheduled next sweep for 2:00 AM.</div>
    """,
    "companieshouse.gov.uk/company/search?q=fintech+london",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#38bdf8;">[Query: FinTech Banking London]</span><br>
      > Status: 120 matching entities<br>
      > Officers: 108 VP/CTO records found<br>
      > SMTP Probe: <code>HELO fathom.local</code> -> <strong>250 OK</strong><br>
      <span style="color:#30d158;">Sync Status: 100% amoCRM updated</span>
    </div>
    """,
    [("Nightly lead sweep", "Weekdays 2am"), ("amoCRM batch sync", "Daily 8am")]
)

# 2. Market Intelligence
create_hd_mockup(
    "02_market_intelligence",
    "Market Intelligence · Real-Time Competitor DOM Diff",
    "🚨 COMPETITOR PRICING SHIFT DETECTED",
    "Market Intel", "🧠", "linear-gradient(135deg, #4f46e5, #6366f1)",
    """
    <div class="msg-date-divider">Today 12:11 AM</div>
    <div class="msg-user-bubble">Track our top 15 fintech competitors for pricing changes, feature drops, or key executive hires</div>
    <div class="msg-bot-bubble">
      Continuous monitoring active across 15 domains. Detected significant DOM diff on Stripe-X:
      <div class="msg-task-card">
        <div class="task-row"><span class="task-chk">✓</span><strong>Target Domain:</strong> <code>stripe-x.io/pricing</code> (Friday 18:42 UTC)</div>
        <div class="task-row" style="color:#ff453a;">- Old Tier: Enterprise 2.9% + $0.30/transaction</div>
        <div class="task-row" style="color:#30d158;">+ New Tier: Enterprise 2.4% + $0.20/transaction (for volume &gt; $1M)</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Memory Ingestion:</strong> Node <code>StripeX_Pricing_2026_Q3</code> stored in SQLite graph (94µs)</div>
      </div>
      Instant alert dispatched to Slack #executive-intel. Summary PDF prepared for Monday briefing.
    </div>
    """,
    "stripe-x.io/pricing",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#ff9f0a;">[DOM Diff Tracker: /pricing]</span><br>
      <span style="color:#ff453a;">- $0.30 fixed fee</span><br>
      <span style="color:#30d158;">+ $0.20 volume tier</span><br>
      > Ingested to graph in 94µs<br>
      <span style="color:#38bdf8;">Status: Alert dispatched</span>
    </div>
    """,
    [("Competitor diff sweep", "Every 6h"), ("Executive briefing", "Friday 5pm")]
)

# 3. Talent Scout
create_hd_mockup(
    "03_talent_scout",
    "Talent Scout · GitHub AST Mining & Technical Sourcing",
    "● 30 RUST ARCHITECTS MAPPED",
    "Talent Scout", "🔍", "linear-gradient(135deg, #0284c7, #38bdf8)",
    """
    <div class="msg-date-divider">Yesterday 4:15 PM</div>
    <div class="msg-user-bubble">Map 30 senior Rust architects with Tokio & distributed systems expertise in Berlin</div>
    <div class="msg-bot-bubble">
      Mined top open-source repositories (tokio-rs/tokio, axum, polars). Evaluated commit ASTs:
      <div class="msg-task-card">
        <div class="task-row"><span class="task-chk">✓</span><strong>Candidate Found:</strong> Alexander Vance (Principal Systems Eng, Berlin)</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>GitHub Proof:</strong> 142 commits to tokio-rs/tokio (SIMD AVX-512 optimizations)</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Verified Contact:</strong> Work email deliverable via SMTP probe</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Personalized Icebreaker:</strong> Referencing EuroRust 2025 presentation</div>
      </div>
      Dossier added to Talent CRM. 3 candidate intro emails drafted in your voice, held for your read.
    </div>
    """,
    "github.com/tokio-rs/tokio/commits",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#38bdf8;">[AST Code Analyzer]</span><br>
      > Candidate: Alexander Vance<br>
      > Tokio commits: 142 (Top 1%)<br>
      > Location: Berlin (4.2 yrs exp)<br>
      <span style="color:#30d158;">Status: Dossier staged</span>
    </div>
    """,
    [("GitHub talent crawler", "Daily 11pm"), ("Candidate follow-up", "Tuesday 9am")]
)

# 4. Back-Office Invoice Reconciliation
create_hd_mockup(
    "04_backoffice_invoice",
    "Back-Office Assistant · 3-Way Invoice Reconciliation",
    "● 500 INVOICES MATCHED · 0 DISCREPANCIES",
    "Back-Office", "📑", "linear-gradient(135deg, #e11d48, #fb7185)",
    """
    <div class="msg-date-divider">Monday 8:00 AM</div>
    <div class="msg-user-bubble">Reconcile August freight invoices against warehouse purchase orders and stage batch payments in QuickBooks</div>
    <div class="msg-bot-bubble">
      Ingested 500 PDF invoices from vendor portal. 3-way match completed:
      <div class="msg-task-card">
        <div class="task-row"><span class="task-chk">✓</span><strong>Parsed:</strong><span class="task-arr">→</span><span>500 PDF invoices ($482,000 total volume)</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>PO Cross-Check:</strong><span class="task-arr">→</span><span>498 exact PO matches in ERP system</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Resolved:</strong><span class="task-arr">→</span><span>2 minor tax discrepancies corrected autonomously</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>QuickBooks Entry:</strong><span class="task-arr">→</span><span>Batch payment staged for CFO 1-click approval</span></div>
      </div>
      Saved approximately 80 hours of manual bookkeeping. Zero discrepancies outstanding.
    </div>
    """,
    "quickbooks.intuit.com/app/invoices",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#38bdf8;">[QuickBooks Accessibility Ref @e14]</span><br>
      > Batch #849: 500 Invoices<br>
      > Total Volume: $482,000.00 USD<br>
      > PO Match Rate: 100.0%<br>
      <span style="color:#30d158;">Status: Ready for CFO approval</span>
    </div>
    """,
    [("Invoice mailbox sweep", "Hourly"), ("Month-end reconciliation", "28th of month")]
)

# 5. DevOps Maintainer
create_hd_mockup(
    "05_devops_engineer",
    "DevOps Maintainer · AST Bug Investigation & Pytest Suite",
    "● 23/23 TESTS PASSING · PR #142 OPENED",
    "DevOps Maintainer", "🛠️", "linear-gradient(135deg, #059669, #10b981)",
    """
    <div class="msg-date-divider">Today 2:45 PM</div>
    <div class="msg-user-bubble">Triage Sentry issue #892: ZeroDivisionError in MoM revenue analytics</div>
    <div class="msg-bot-bubble">
      Investigating. Mapped repository AST symbols in 34ms:
      <div class="msg-task-card">
        <div class="task-row"><span class="task-chk">✓</span><strong>Root Cause:</strong> <code>revenue_calc.py:84</code> crashes when prior month revenue is $0</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Reproduction:</strong> Created standalone test <code>test_mom_zero_prior_month</code></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Patched:</strong> Added defensive check returning <code>0.0%</code> on zero prior baseline</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Pytest Suite:</strong> 23/23 tests passed in 1.42s</div>
      </div>
      Branch <code>fix/mom-zero-div</code> pushed. Pull Request #142 opened and ready for review.
    </div>
    """,
    "github.com/company/repo/pull/142",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#30d158;">[GitHub PR #142: Open]</span><br>
      > 1 file changed, +4 -1 lines<br>
      > CI Pipeline: Green (23 passed)<br>
      > Reviewers: Assigned<br>
      <span style="color:#38bdf8;">Status: Awaiting merge</span>
    </div>
    """,
    [("Sentry error triage", "Every 15m"), ("Dependency security scan", "Weekly Sun")]
)

# 6. Governed Computer 2FA Takeover
create_hd_mockup(
    "06_computer_2fa_takeover",
    "Governed Computer Use · 2FA SMS Human Takeover Lease",
    "⚠️ 2FA SMS CHALLENGE · BOT PAUSED",
    "Chief of Staff", "👑", "linear-gradient(135deg, #0d9488, #14b8a6)",
    """
    <div class="msg-date-divider">Yesterday 11:15 AM</div>
    <div class="msg-bot-bubble">
      Logging into AWS billing portal to export quarterly invoice breakdown. Encountered SMS 2FA checkpoint.
      <div class="msg-task-card">
        <div class="task-row" style="color:#ff9f0a;">⚠️ <strong>2FA SMS Gate:</strong> Security code sent to registered mobile ending in 8921</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Safety State:</strong> Autonomous bot commands paused; operator lease active</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Action Needed:</strong> Enter 6-digit code in live browser feed on right</div>
      </div>
    </div>
    <div class="msg-user-bubble">Code entered: 849201. Release control to bot.</div>
    <div class="msg-bot-bubble">Control resumed. Capturing fresh accessibility snapshot. Downloaded Q3 invoice PDF successfully.</div>
    """,
    "signin.aws.amazon.com/2fa/verify",
    """
    <div style="text-align:center; padding:12px; background:#16161e; border-radius:7px;">
      <div style="font-weight:700; color:#ececee; font-size:11.5px;">Amazon AWS 2FA Verification</div>
      <div style="font-size:10px; color:#8e8e93; margin:4px 0;">Enter security code from SMS</div>
      <div style="background:#09090c; border:1px solid #38bdf8; border-radius:5px; padding:7px; width:120px; margin:8px auto; font-family:'JetBrains Mono'; letter-spacing:4px; color:#38bdf8; font-size:13px; font-weight:700;">849201</div>
    </div>
    """,
    [("AWS billing export", "1st of month"), ("Vendor statement audit", "Bi-weekly")]
)

# 7. Inbox Manager
create_hd_mockup(
    "07_inbox_manager",
    "Inbox Manager · Autonomous Email Sweeper",
    "● INBOX AT ZERO · 5 DRAFTS PARKED",
    "Chief of Staff", "👑", "linear-gradient(135deg, #0d9488, #14b8a6)",
    """
    <div class="msg-date-divider">Yesterday 11:11 PM</div>
    <div class="msg-user-bubble">Inbox got away from me over the weekend, sweep it?</div>
    <div class="msg-bot-bubble">On it. 41 unread since Friday. Archiving the noise, drafting the rest.</div>
    <div class="msg-date-divider">12:11 AM</div>
    <div class="msg-bot-bubble">
      <div class="msg-task-card">
        <div class="task-row"><span class="task-chk">✓</span><strong>Archived</strong><span class="task-arr">→</span><span>26 newsletters + receipts</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Replied</strong><span class="task-arr">→</span><span>9 routine threads · scheduling and intros</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Drafted</strong><span class="task-arr">→</span><span>6 that sound like you · held for your read</span></div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Flagged</strong><span class="task-arr">→</span><span>1 from Nora · contract question</span></div>
      </div>
      Nora is asking whether the renewal covers the new seats. Her draft answers yes and quotes the contract line. It is at the top of the held pile.
    </div>
    <div class="msg-user-bubble">Send Nora's, I'll take the rest tomorrow</div>
    <div class="msg-bot-bubble">Sent. Inbox is at zero, with 5 drafts parked for tomorrow.</div>
    """,
    "mail.google.com/mail/u/0/#inbox",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#38bdf8;">[Inbox Zero Status]</span><br>
      > 41 threads read<br>
      > 26 archived · 9 replied<br>
      > Nora’s renewal: Sent<br>
      <span style="color:#30d158;">5 drafts parked for tomorrow</span>
    </div>
    """,
    [("Morning sweep", "Weekdays 6am"), ("Reply Zero", "Hourly")]
)

# 8. Free Value Audit Lead Table
create_hd_mockup(
    "08_lead_audit_report",
    "Lead Gen Auditor · 50-Lead Value Audit Deliverable",
    "✓ 50 SMTP VERIFIED LEADS · READY FOR EXPORT",
    "Sales Outbound", "⚡", "linear-gradient(135deg, #ea580c, #f97316)",
    """
    <div class="msg-date-divider">Today 9:00 AM</div>
    <div class="msg-user-bubble">Generate a free 50-lead audit report for FinTech companies in DACH</div>
    <div class="msg-bot-bubble">
      Audit completed in 84 seconds. 50 verified decision-maker records ready:
      <div class="msg-task-card">
        <table style="width:100%; font-size:10px; border-collapse:collapse; margin-top:2px;">
          <tr style="color:#8e8e93; border-bottom:1px solid #23232a; text-align:left;">
            <th style="padding:3px;">Name & Role</th><th style="padding:3px;">Company</th><th style="padding:3px;">Work Email</th><th style="padding:3px;">SMTP Status</th>
          </tr>
          <tr style="border-bottom:1px solid #1c1c22;">
            <td style="padding:4px; font-weight:700;">Dr. Marcus Weber (CISO)</td><td>FinTech Bavaria AG</td><td style="color:#38bdf8; font-family:'JetBrains Mono';">m.weber@fintech-bavaria.de</td><td><span style="color:#30d158; font-weight:700;">250 OK (0.99)</span></td>
          </tr>
          <tr style="border-bottom:1px solid #1c1c22;">
            <td style="padding:4px; font-weight:700;">Elena Schmidt (VP IT)</td><td>SecureCloud GmbH</td><td style="color:#38bdf8; font-family:'JetBrains Mono';">e.schmidt@securecloud.io</td><td><span style="color:#30d158; font-weight:700;">250 OK (0.98)</span></td>
          </tr>
          <tr>
            <td style="padding:4px; font-weight:700;">Lukas Becker (Head Sec)</td><td>Munich Data Labs</td><td style="color:#38bdf8; font-family:'JetBrains Mono';">l.becker@munichdatalabs.com</td><td><span style="color:#30d158; font-weight:700;">250 OK (0.95)</span></td>
          </tr>
        </table>
      </div>
      Excel file <code>DACH_Fintech_Audit.xlsx</code> compiled and ready for client delivery.
    </div>
    """,
    "fathom.ai/app/audits/dach-fintech-q3",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#30d158;">[Audit Deliverable Export]</span><br>
      > 50 / 50 Leads Verified<br>
      > Format: Excel (.xlsx) + CSV<br>
      > amoCRM / HubSpot ready<br>
      <span style="color:#38bdf8;">Download: DACH_Audit.xlsx</span>
    </div>
    """,
    [("Lead quality check", "Daily 9am"), ("Weekly CRM export", "Friday 4pm")]
)

# 9. Entity Knowledge Graph
create_hd_mockup(
    "09_entity_knowledge_graph",
    "Knowledge Engine · Entity Memory Graph Explorer",
    "● 5,420 NODES · 1.62ms QUERY TIME",
    "Market Intel", "🧠", "linear-gradient(135deg, #4f46e5, #6366f1)",
    """
    <div class="msg-date-divider">Today 3:00 PM</div>
    <div class="msg-user-bubble">Which former Stripe engineers are now CTOs at Series-A AI companies in Berlin?</div>
    <div class="msg-bot-bubble">
      Traversed 5,420 graph nodes across 3 relationship hops in 1.62ms:
      <div class="msg-task-card">
        <div class="task-row"><span class="task-chk">✓</span><strong>Matched Entity:</strong> Jane Doe (CTO at Acme FinTech, Berlin)</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Lineage:</strong> Ex-Stripe (Staff Infra, 2021–2024) ──[works_at]──► Acme FinTech (Series A, $14M)</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Technology Stack:</strong> Acme FinTech ──[uses_tech]──► Rust, Axum, PostgreSQL</div>
      </div>
      Memory fact verified against Handelsregister filing from August 2026.
    </div>
    """,
    "fathom.internal/memory/graph/explorer",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#38bdf8;">[Entity Knowledge Graph]</span><br>
      Jane Doe (CTO)<br>
      ──[works_at]──► Acme FinTech<br>
      ──[uses_tech]──► Rust & Axum<br>
      <span style="color:#30d158;">Query Latency: 1.62 ms</span>
    </div>
    """,
    [("Graph deduplication", "Daily 3am"), ("Memory compaction", "Weekly Sun")]
)

# 10. Security Governance Vault
create_hd_mockup(
    "10_security_credentials_vault",
    "Enterprise Security · AES-256-GCM Credentials Vault",
    "🔒 RING CRATE · ZERO PROMPT EXPOSURE",
    "DevOps Maintainer", "🛠️", "linear-gradient(135deg, #059669, #10b981)",
    """
    <div class="msg-date-divider">Today 4:00 PM</div>
    <div class="msg-user-bubble">Audit active API credentials and check for prompt leak vulnerabilities</div>
    <div class="msg-bot-bubble">
      Security audit complete. All secrets isolated behind AES-256-GCM encrypted vault:
      <div class="msg-task-card">
        <div class="task-row"><span class="task-chk">✓</span><strong>HubSpot API Key:</strong> Encrypted AES-GCM (0% prompt exposure, resolved in Rust memory)</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>amoCRM OAuth:</strong> Encrypted AES-GCM (0% prompt exposure)</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Policy Engine:</strong> Fail-Closed (Deny wins on destructive shell commands)</div>
        <div class="task-row"><span class="task-chk">✓</span><strong>Audit Ledger:</strong> Nanosecond-precision tamper-proof logs in SQLite</div>
      </div>
      Zero secret leakage detected across all active worker sessions.
    </div>
    """,
    "fathom.internal/governance/vault",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:10px;">
      <span style="color:#30d158;">[AES-256-GCM Vault]</span><br>
      > hubspot_api_key: MASKED<br>
      > amocrm_oauth: MASKED<br>
      > Prompt Exposure: 0.00%<br>
      <span style="color:#38bdf8;">Status: 100% Compliant</span>
    </div>
    """,
    [("Vault integrity scan", "Daily 4am"), ("Audit log rotation", "Monthly 1st")]
)

print("\n10 HTML mockup templates generated. Starting HD Retina PNG rendering...")

mockups = [
    "01_sales_outbound_sdr",
    "02_market_intelligence",
    "03_talent_scout",
    "04_backoffice_invoice",
    "05_devops_engineer",
    "06_computer_2fa_takeover",
    "07_inbox_manager",
    "08_lead_audit_report",
    "09_entity_knowledge_graph",
    "10_security_credentials_vault"
]

for name in mockups:
    html_p = os.path.join(MOCKUPS_DIR, f"{name}.html")
    png_p = os.path.join(MOCKUPS_DIR, f"{name}.png")
    tmp_d = tempfile.mkdtemp(prefix=f"hd_{name}_")
    cmd = [
        CHROME_PATH,
        "--headless",
        "--disable-gpu",
        "--force-device-scale-factor=2",
        "--window-size=1280,820",
        f"--user-data-dir={tmp_d}",
        f"--screenshot={png_p}",
        f"file://{html_p}"
    ]
    subprocess.run(cmd, capture_output=True)
    shutil.rmtree(tmp_d, ignore_errors=True)
    if os.path.exists(png_p):
        print(f"OK [HD Retina]: {name}.png ({os.path.getsize(png_p):,} bytes)")

print("\nALL 10 HD RETINA PNG SCREENSHOTS GENERATED SUCCESSFULLY!")
