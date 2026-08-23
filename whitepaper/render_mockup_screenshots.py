#!/usr/bin/env python3
import os, subprocess, tempfile, shutil

MOCKUPS_DIR = "/Users/yakushev/Documents/GitHub/Fathom/whitepaper/mockups"
os.makedirs(MOCKUPS_DIR, exist_ok=True)
CHROME_PATH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

CSS = """
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: #060608;
  color: #ececee;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  padding: 30px;
}

.window-frame {
  width: 1180px;
  height: 720px;
  background: #0d0d11;
  border: 1px solid #23232a;
  border-radius: 12px;
  box-shadow: 0 25px 60px rgba(0, 0, 0, 0.65), 0 0 0 1px rgba(255,255,255,0.05);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Chrome Topbar */
.window-chrome {
  background: #131317;
  padding: 10px 16px;
  border-bottom: 1px solid #202026;
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.traffic-lights { display: flex; gap: 7px; align-items: center; }
.dot { width: 11px; height: 11px; border-radius: 50%; display: inline-block; }
.dot-red { background: #ff5f56; border: 1px solid #e0443e; }
.dot-yellow { background: #ffbd2e; border: 1px solid #dea123; }
.dot-green { background: #27c93f; border: 1px solid #1aab29; }
.window-title { font-size: 11px; font-weight: 600; color: #8e8e93; font-family: 'JetBrains Mono', monospace; }
.window-status { font-size: 10px; font-weight: 700; color: #30d158; font-family: 'JetBrains Mono', monospace; letter-spacing: 0.05em; }

/* 3-Pane Body */
.window-body {
  display: grid;
  grid-template-columns: 240px 1fr 310px;
  flex: 1;
  background: #09090c;
  overflow: hidden;
}

/* Left Sidebar */
.sidebar {
  background: #101014;
  border-right: 1px solid #1c1c22;
  padding: 12px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 10px;
}
.sidebar-head { display: flex; justify-content: space-between; font-size: 12px; font-weight: 700; color: #ececee; }
.search-input {
  background: #09090c;
  border: 1px solid #23232a;
  border-radius: 6px;
  padding: 6px 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  color: #71717a;
  font-size: 11px;
}
.bot-list { display: flex; flex-direction: column; gap: 4px; overflow-y: auto; }
.bot-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s;
}
.bot-item.is-active {
  background: #1c1c22;
  border-left: 3px solid #6366f1;
}
.bot-avatar {
  width: 32px;
  height: 32px;
  border-radius: 9px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 15px;
  flex-shrink: 0;
}
.bot-details { display: flex; flex-direction: column; overflow: hidden; flex: 1; }
.bot-title-row { display: flex; justify-content: space-between; align-items: center; }
.bot-item-name { font-size: 11px; font-weight: 700; color: #ececee; }
.bot-item-time { font-size: 9px; color: #71717a; font-family: 'JetBrains Mono', monospace; }
.bot-item-preview { font-size: 10px; color: #8e8e93; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 1px; }

.user-profile {
  display: flex;
  align-items: center;
  gap: 8px;
  border-top: 1px solid #1c1c22;
  padding-top: 10px;
}
.user-avatar {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: #27272a;
  color: #ececee;
  font-size: 10px;
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
}
.user-name { font-size: 11px; font-weight: 600; color: #ececee; }

/* Center Chat */
.chat-pane {
  padding: 14px 18px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  background: #09090c;
  overflow: hidden;
}
.chat-topbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #19191f;
  padding-bottom: 8px;
}
.chat-bot-info { display: flex; align-items: center; gap: 8px; }
.chat-bot-name { font-size: 13px; font-weight: 700; color: #ececee; }
.chat-thread { display: flex; flex-direction: column; gap: 8px; overflow-y: auto; padding: 10px 0; }
.bubble-user {
  background: #1c1c22;
  color: #ececee;
  padding: 8px 12px;
  border-radius: 12px 12px 3px 12px;
  align-self: flex-end;
  font-size: 11.5px;
  max-width: 85%;
  line-height: 1.4;
}
.bubble-bot {
  background: #141418;
  border: 1px solid #23232a;
  color: #d4d4d8;
  padding: 9px 13px;
  border-radius: 12px 12px 12px 3px;
  align-self: flex-start;
  font-size: 11.5px;
  width: 98%;
  line-height: 1.45;
}
.task-card {
  background: #0e0e12;
  border: 1px solid #23232a;
  border-radius: 8px;
  padding: 8px 12px;
  margin: 6px 0;
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.task-line { display: flex; align-items: center; gap: 8px; font-size: 11px; color: #d4d4d8; }
.task-check { color: #30d158; font-weight: 800; font-size: 12px; }
.task-arrow { color: #71717a; }

.composer {
  background: #131317;
  border: 1px solid #23232a;
  border-radius: 20px;
  padding: 6px 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: 11px;
}
.comp-plus { color: #71717a; font-size: 14px; font-weight: 700; cursor: pointer; }
.comp-input { color: #71717a; flex: 1; }
.comp-send {
  background: #27272a;
  color: #ececee;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
}

/* Right Computer Pane */
.computer-pane {
  background: #101014;
  border-left: 1px solid #1c1c22;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.comp-head { display: flex; justify-content: space-between; align-items: center; color: #ececee; font-size: 11.5px; font-weight: 700; }
.screen-wrapper {
  background: #09090c;
  border: 1px solid #23232a;
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.browser-bar {
  background: #16161c;
  padding: 5px 8px;
  display: flex;
  align-items: center;
  gap: 6px;
  border-bottom: 1px solid #23232a;
}
.b-dots { display: flex; gap: 4px; }
.b-dot { width: 6px; height: 6px; border-radius: 50%; background: #52525b; }
.url-pill {
  background: #09090c;
  color: #38bdf8;
  font-family: 'JetBrains Mono', monospace;
  font-size: 9.5px;
  padding: 2px 8px;
  border-radius: 4px;
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.browser-body { padding: 8px; font-size: 10px; color: #d4d4d8; min-height: 90px; }
.screen-meta { display: flex; justify-content: space-between; align-items: center; font-size: 10px; color: #8e8e93; margin-top: 2px; }
.btn-takeover {
  background: #18181c;
  border: 1px solid #3f3f46;
  color: #ececee;
  border-radius: 8px;
  padding: 3px 8px;
  font-size: 10px;
  font-weight: 600;
  cursor: pointer;
}
.routines-label { font-size: 10px; font-weight: 700; color: #71717a; text-transform: uppercase; letter-spacing: 0.05em; margin-top: 4px; }
.routine-item {
  background: #141418;
  border: 1px solid #202026;
  border-radius: 6px;
  padding: 4px 8px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 10px;
}
.routine-title { color: #e4e4e7; font-weight: 600; }
.routine-time { color: #38bdf8; font-family: 'JetBrains Mono', monospace; font-size: 9px; }
.new-routine { color: #71717a; font-size: 9.5px; cursor: pointer; padding: 2px; }
"""

def generate_mockup_page(filename, title, status, active_bot_name, chat_content, browser_url, browser_body, routines):
    bots = [
        ("Chief of Staff", "linear-gradient(135deg, #0d9488, #14b8a6)", "👑", "venue booked, contracts queued", "Yesterday"),
        ("Sales Outbound", "linear-gradient(135deg, #ea580c, #f97316)", "⚡", "40 accounts researched, 18 queued", "3:10 AM"),
        ("Market Intel", "linear-gradient(135deg, #4f46e5, #6366f1)", "🧠", "competitor pricing diff alert", "12:11 AM"),
        ("Talent Scout", "linear-gradient(135deg, #0284c7, #38bdf8)", "🔍", "5 senior Rust engineers mapped", "Yesterday"),
        ("Back-Office", "linear-gradient(135deg, #e11d48, #fb7185)", "📑", "500 invoices matched, 0 errors", "Monday"),
        ("DevOps Maintainer", "linear-gradient(135deg, #059669, #10b981)", "🛠️", "23/23 tests passing, PR #142", "2:45 PM"),
    ]
    
    bot_list_html = []
    for bname, bg, bsvg, bprev, btime in bots:
        is_act = " is-active" if bname == active_bot_name else ""
        bot_list_html.append(f"""
        <div class="bot-item{is_act}">
          <div class="bot-avatar" style="background: {bg};">{bsvg}</div>
          <div class="bot-details">
            <div class="bot-title-row">
              <span class="bot-item-name">{bname}</span>
              <span class="bot-item-time">{btime}</span>
            </div>
            <div class="bot-item-preview">{bprev}</div>
          </div>
        </div>""")
        
    routines_html = []
    for rtitle, rtime in routines:
        routines_html.append(f"""
        <div class="routine-item">
          <span class="routine-title">◷ {rtitle}</span>
          <span class="routine-time">{rtime}</span>
        </div>""")

    html = f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{title}</title>
<style>{CSS}</style>
</head>
<body>

<div class="window-frame">
  <div class="window-chrome">
    <div class="traffic-lights">
      <span class="dot dot-red"></span>
      <span class="dot dot-yellow"></span>
      <span class="dot dot-green"></span>
      <span class="window-title">fathom-desktop · {title}</span>
    </div>
    <span class="window-status">{status}</span>
  </div>

  <div class="window-body">
    <!-- Sidebar -->
    <div class="sidebar">
      <div>
        <div class="sidebar-head">
          <span>Bots</span>
          <span style="cursor:pointer; color:#71717a;">+</span>
        </div>
        <div class="search-input" style="margin: 8px 0 10px 0;">
          <span>⌕</span>
          <span>Search bots...</span>
        </div>
        <div class="bot-list">
          {''.join(bot_list_html)}
        </div>
      </div>

      <div class="user-profile">
        <div class="user-avatar">AK</div>
        <span class="user-name">Avery Kim</span>
      </div>
    </div>

    <!-- Center Chat -->
    <div class="chat-pane">
      <div class="chat-topbar">
        <div class="chat-bot-info">
          <span class="chat-bot-name">{active_bot_name}</span>
          <span style="font-size:10px; color:#71717a;">▾</span>
        </div>
        <div style="font-size:14px; cursor:pointer;">💻</div>
      </div>

      <div class="chat-thread">
        {chat_content}
      </div>

      <div class="composer">
        <span class="comp-plus">+</span>
        <span class="comp-input">Message {active_bot_name}...</span>
        <span class="comp-send">↑</span>
      </div>
    </div>

    <!-- Right Computer -->
    <div class="computer-pane">
      <div class="comp-head">
        <span>{active_bot_name}’s computer</span>
        <span style="font-size:12px; color:#71717a;">⚙</span>
      </div>

      <div class="screen-wrapper">
        <div class="browser-bar">
          <div class="b-dots"><span class="b-dot"></span><span class="b-dot"></span><span class="b-dot"></span></div>
          <div class="url-pill">{browser_url}</div>
        </div>
        <div class="browser-body">
          {browser_body}
        </div>
      </div>

      <div class="screen-meta">
        <span>{active_bot_name}’s screen</span>
        <button class="btn-takeover">Take control</button>
      </div>

      <div class="routines-label">Routines</div>
      <div style="display:flex; flex-direction:column; gap:4px;">
        {''.join(routines_html)}
        <div class="new-routine">+ New routine</div>
      </div>
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
# DEFINE 10 DETAILED MOCKUPS
# ==============================================================================

# 1. Sales Outbound SDR
generate_mockup_page(
    "mockup_01_sales_outbound",
    "Sales Outbound SDR Workspace",
    "● 40 ACCOUNTS RESEARCHED · 18 QUEUED",
    "Sales Outbound",
    """
    <div class="bubble-user">Find 50 verified VP Engineering leads in London fintechs & sync to amoCRM</div>
    <div class="bubble-bot">
      On it. Decomposed search across Companies House UK and executive registries. Running SMTP probes:
      <div class="task-card">
        <div class="task-line"><span class="task-check">✓</span><strong>Discovered</strong><span class="task-arrow">→</span><span>120 London FinTech companies on Companies House</span></div>
        <div class="task-line"><span class="task-check">✓</span><strong>Harvested</strong><span class="task-arrow">→</span><span>108 VP Engineering & CTO names via search backends</span></div>
        <div class="task-line"><span class="task-check">✓</span><strong>SMTP 250 OK Probes</strong><span class="task-arrow">→</span><span>94 direct mailboxes deliverable (0% bounce rate)</span></div>
        <div class="task-line"><span class="task-check">✓</span><strong>amoCRM Synced</strong><span class="task-arrow">→</span><span>94 contacts staged with tech tags (AWS, Rust, Stripe)</span></div>
      </div>
      Top 5 draft emails generated in your tone. Staged for review.
    </div>
    <div class="bubble-user">Approve all 94 for amoCRM push</div>
    <div class="bubble-bot">Pushed. 94 deal cards active in amoCRM under 'Q3 Outbound'. Scheduled next sweep for 2:00 AM.</div>
    """,
    "companieshouse.gov.uk/company/search",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:9px;">
      <span style="color:#38bdf8;">[Query: FinTech Banking London]</span><br>
      > Status: 120 companies found<br>
      > Officers: 108 VP/CTO records parsed<br>
      > SMTP Probe: <code>HELO fathom.local</code> -> 250 OK<br>
      <span style="color:#30d158;">Sync Status: 100% amoCRM updated</span>
    </div>
    """,
    [("Nightly lead sweep", "Weekdays 2am"), ("amoCRM pipeline push", "Daily 8am")]
)

# 2. Market Intel
generate_mockup_page(
    "mockup_02_market_intel",
    "Market Intelligence & Competitor Tracker",
    "🚨 COMPETITOR PRICING SHIFT DETECTED",
    "Market Intel",
    """
    <div class="bubble-user">Track our top 15 fintech competitors for pricing adjustments or feature drops</div>
    <div class="bubble-bot">
      Monitoring 15 domains continuously. Detected significant DOM diff on Stripe-X:
      <div class="task-card">
        <div class="task-line"><span class="task-check">✓</span><strong>Target Domain:</strong> <code>stripe-x.io/pricing</code> (Friday 18:42 UTC)</div>
        <div class="task-line" style="color:#ff453a;">- Old Tier: Enterprise 2.9% + $0.30/txn</div>
        <div class="task-line" style="color:#30d158;">+ New Tier: Enterprise 2.4% + $0.20/txn (for volume &gt; $1M)</div>
        <div class="task-line"><span class="task-check">✓</span><strong>Memory Updated:</strong> Node <code>StripeX_Pricing_2026_Q3</code> stored in SQLite graph</div>
      </div>
      Alert sent to Slack #executive-intel. Summary PDF prepared.
    </div>
    """,
    "stripe-x.io/pricing",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:9px;">
      <span style="color:#ff9f0a;">[DOM Diff Tracker: /pricing]</span><br>
      <span style="color:#ff453a;">- $0.30 fixed fee</span><br>
      <span style="color:#30d158;">+ $0.20 volume tier</span><br>
      > Ingested to graph in 94µs<br>
      <span style="color:#38bdf8;">Status: Alert dispatched</span>
    </div>
    """,
    [("Competitor diff sweep", "Every 6h"), ("Weekly executive report", "Friday 5pm")]
)

# 3. Talent Scout
generate_mockup_page(
    "mockup_03_talent_scout",
    "Executive Talent Scout & GitHub Miner",
    "● 30 RUST ARCHITECTS MAPPED",
    "Talent Scout",
    """
    <div class="bubble-user">Map 30 senior Rust architects with Tokio & distributed systems expertise in Berlin</div>
    <div class="bubble-bot">
      Mined top open-source repositories (tokio, axum, polars). Evaluated commit ASTs:
      <div class="task-card">
        <div class="task-line"><span class="task-check">✓</span><strong>Candidate Found:</strong> Alexander Vance (Principal Systems Eng, Berlin)</div>
        <div class="task-line"><span class="task-check">✓</span><strong>GitHub Proof:</strong> 142 commits to tokio-rs/tokio (SIMD AVX-512 optimizations)</div>
        <div class="task-line"><span class="task-check">✓</span><strong>Verified Contact:</strong> Work email deliverable via SMTP probe</div>
        <div class="task-line"><span class="task-check">✓</span><strong>Personalized Icebreaker:</strong> Referencing EuroRust 2025 presentation</div>
      </div>
      Dossier added to Talent CRM. 3 candidate intro emails drafted in your voice.
    </div>
    """,
    "github.com/tokio-rs/tokio/commits",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:9px;">
      <span style="color:#38bdf8;">[AST Code Analyzer]</span><br>
      > Candidate: Alexander Vance<br>
      > Tokio commits: 142 (Top 1%)<br>
      > LinkedIn: Berlin, 4.2 yrs exp<br>
      <span style="color:#30d158;">Status: Dossier staged</span>
    </div>
    """,
    [("GitHub talent crawler", "Daily 11pm"), ("Candidate follow-up check", "Tuesday 9am")]
)

# 4. Back-Office Invoice Reconciliation
generate_mockup_page(
    "mockup_04_backoffice_invoice",
    "Back-Office & Invoice Reconciliation",
    "● 500 INVOICES MATCHED · 0 DISCREPANCIES",
    "Back-Office",
    """
    <div class="bubble-user">Reconcile August freight invoices against warehouse purchase orders</div>
    <div class="bubble-bot">
      Ingested 500 PDF invoices from vendor portal. 3-way match completed:
      <div class="task-card">
        <div class="task-line"><span class="task-check">✓</span><strong>Parsed</strong><span class="task-arrow">→</span><span>500 PDF invoices ($482,000 total volume)</span></div>
        <div class="task-line"><span class="task-check">✓</span><strong>PO Cross-Check</strong><span class="task-arrow">→</span><span>498 exact PO matches in ERP system</span></div>
        <div class="task-line"><span class="task-check">✓</span><strong>Resolved</strong><span class="task-arrow">→</span><span>2 minor tax discrepancies corrected autonomously</span></div>
        <div class="task-line"><span class="task-check">✓</span><strong>QuickBooks Entry</strong><span class="task-arrow">→</span><span>Batch payment staged for CFO 1-click approval</span></div>
      </div>
      Saved approximately 80 hours of manual bookkeeping.
    </div>
    """,
    "quickbooks.intuit.com/app/invoices",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:9px;">
      <span style="color:#38bdf8;">[QuickBooks Accessibility Ref @e14]</span><br>
      > Batch #849: 500 Invoices<br>
      > Total: $482,000.00 USD<br>
      > PO Match Rate: 100.0%<br>
      <span style="color:#30d158;">Status: Ready for CFO approval</span>
    </div>
    """,
    [("Invoice mailbox sweep", "Hourly"), ("Month-end reconciliation", "28th of month")]
)

# 5. DevOps Maintainer
generate_mockup_page(
    "mockup_05_devops_engineer",
    "DevOps Maintainer & Code Engineer",
    "● 23/23 TESTS PASSING · PR #142 OPENED",
    "DevOps Maintainer",
    """
    <div class="bubble-user">Triage Sentry issue #892: ZeroDivisionError in MoM revenue analytics</div>
    <div class="bubble-bot">
      Investigating. Mapped repository AST symbols in 34ms:
      <div class="task-card">
        <div class="task-line"><span class="task-check">✓</span><strong>Root Cause:</strong> <code>revenue_calc.py:84</code> crashes when prior month revenue is $0</div>
        <div class="task-line"><span class="task-check">✓</span><strong>Reproduction:</strong> Created standalone test <code>test_mom_zero_prior_month</code></div>
        <div class="task-line"><span class="task-check">✓</span><strong>Patched:</strong> Added defensive check returning <code>0.0%</code> on zero prior baseline</div>
        <div class="task-line"><span class="task-check">✓</span><strong>Pytest Suite:</strong> 23/23 tests passed in 1.42s</div>
      </div>
      Branch <code>fix/mom-zero-div</code> pushed. Pull Request #142 ready for review.
    </div>
    """,
    "github.com/company/repo/pull/142",
    """
    <div style="font-family:'JetBrains Mono',monospace; font-size:9px;">
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
generate_mockup_page(
    "mockup_06_computer_2fa_takeover",
    "Governed Computer Use · 2FA Takeover",
    "⚠️ 2FA SMS CHALLENGE · BOT PAUSED",
    "Chief of Staff",
    """
    <div class="bubble-bot">
      Logging into AWS billing portal to export invoice breakdown. Encountered SMS 2FA checkpoint.
      <div class="task-card">
        <div class="task-line" style="color:#ff9f0a;">⚠️ <strong>2FA SMS Gate:</strong> Code sent to registered mobile phone ending in 8921</div>
        <div class="task-line"><span class="task-check">✓</span><strong>Safety State:</strong> Autonomous bot commands paused; operator lease active</div>
        <div class="task-line"><span class="task-check">✓</span><strong>Action Needed:</strong> Enter 6-digit code in live browser feed on right</div>
      </div>
    </div>
    <div class="bubble-user">Code entered: 849201. Release control.</div>
    <div class="bubble-bot">Control resumed. Capturing fresh accessibility snapshot. Downloaded invoice PDF successfully.</div>
    """,
    "signin.aws.amazon.com/2fa/verify",
    """
    <div style="text-align:center; padding:10px; background:#16161e; border-radius:6px;">
      <div style="font-weight:700; color:#ececee; font-size:11px;">Amazon AWS 2FA Verification</div>
      <div style="font-size:9.5px; color:#8e8e93; margin:4px 0;">Enter security code from SMS</div>
      <div style="background:#09090c; border:1px solid #38bdf8; border-radius:4px; padding:6px; width:110px; margin:6px auto; font-family:'JetBrains Mono'; letter-spacing:4px; color:#38bdf8; font-size:12px;">849201</div>
    </div>
    """,
    [("AWS billing export", "1st of month"), ("Vendor statement audit", "Bi-weekly")]
)

print("Generated HTML mockup files. Starting Chrome PNG screenshot rendering...")

def render_png(fname):
    html_file = os.path.join(MOCKUPS_DIR, f"{fname}.html")
    png_file = os.path.join(MOCKUPS_DIR, f"{fname}.png")
    tmp_dir = tempfile.mkdtemp(prefix=f"ch_scr_{fname}_")
    cmd = [
        CHROME_PATH,
        "--headless",
        "--disable-gpu",
        "--window-size=1280,800",
        f"--user-data-dir={tmp_dir}",
        f"--screenshot={png_file}",
        f"file://{html_file}"
    ]
    subprocess.run(cmd, capture_output=True)
    shutil.rmtree(tmp_dir, ignore_errors=True)
    if os.path.exists(png_file):
        print(f"Rendered: {fname}.png ({os.path.getsize(png_file):,} bytes)")
        return png_file
    return None

mockup_names = [
    "mockup_01_sales_outbound",
    "mockup_02_market_intel",
    "mockup_03_talent_scout",
    "mockup_04_backoffice_invoice",
    "mockup_05_devops_engineer",
    "mockup_06_computer_2fa_takeover"
]

for name in mockup_names:
    render_png(name)

print("\nALL PNG SCREENSHOTS GENERATED IN whitepaper/mockups/!")
