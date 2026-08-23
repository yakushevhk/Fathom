# High-Fidelity Rakazo UI Component Library for Fathom Whitepaper

def make_rakazo_demo_ui(
    bot_name="Sales Outbound",
    bot_avatar_color="orange",
    bot_avatar_svg="⚡",
    bot_preview="done. 40 accounts researched, 18 drafts queued",
    active_bot_index=1,
    chat_thread_html="",
    browser_url="companieshouse.gov.uk",
    browser_content_html="",
    routines=[("Nightly lead sweep", "Weekdays 2am"), ("CRM batch sync", "Daily 8am")],
    user_name="Alex Rivera",
    user_initials="AR"
):
    bots = [
        ("Chief of Staff", "teal", "👑", "briefing ready, 3 meetings scheduled", "Yesterday"),
        ("Sales Outbound", "orange", "⚡", "40 accounts researched, 18 queued", "3:10 AM"),
        ("Market Intel", "indigo", "🧠", "competitor pricing diff alert logged", "12:11 AM"),
        ("Talent Scout", "blue", "🔍", "5 senior Rust engineers mapped", "Yesterday"),
        ("Back-Office", "coral", "📑", "500 invoices matched, 0 discrepancies", "Monday"),
        ("DevOps Maintainer", "emerald", "🛠️", "23/23 tests passing, PR #142 opened", "2:45 PM"),
    ]
    
    bot_rows_html = []
    for idx, (bname, bcolor, bsvg, bprev, btime) in enumerate(bots):
        is_act = " is-active" if bname == bot_name else ""
        color_map = {
            "teal": "background: linear-gradient(135deg, #0d9488, #14b8a6);",
            "orange": "background: linear-gradient(135deg, #ea580c, #f97316);",
            "indigo": "background: linear-gradient(135deg, #4f46e5, #6366f1);",
            "blue": "background: linear-gradient(135deg, #0284c7, #38bdf8);",
            "coral": "background: linear-gradient(135deg, #e11d48, #fb7185);",
            "emerald": "background: linear-gradient(135deg, #059669, #10b981);",
        }
        bg = color_map.get(bcolor, color_map["orange"])
        bot_rows_html.append(f"""
        <div class="rk-bot-row{is_act}">
          <div class="rk-avatar" style="{bg}">{bsvg}</div>
          <div class="rk-bot-meta">
            <div class="rk-bot-top">
              <span class="rk-bot-title">{bname}</span>
              <span class="rk-bot-time">{btime}</span>
            </div>
            <div class="rk-bot-sub">{bprev}</div>
          </div>
        </div>""")

    routines_html = []
    for rname, rwhen in routines:
        routines_html.append(f"""
        <div class="rk-routine">
          <span class="rk-routine-icon">◷</span>
          <span class="rk-routine-title">{rname}</span>
          <span class="rk-routine-time">{rwhen}</span>
        </div>""")

    return f"""
<div class="rk-frame">
  <!-- Top Window Bar -->
  <div class="rk-chrome">
    <div class="rk-traffic">
      <span class="rk-dot rk-dot-red"></span>
      <span class="rk-dot rk-dot-yellow"></span>
      <span class="rk-dot rk-dot-green"></span>
    </div>
    <span class="rk-window-title">Fathom Autonomous Workspace · {bot_name}</span>
    <div class="rk-chrome-badge">● LIVE AGENT ACTIVE</div>
  </div>

  <div class="rk-body">
    <!-- Left Sidebar: Bots List -->
    <div class="rk-sidebar">
      <div class="rk-sidebar-head">
        <span>Bots</span>
        <span class="rk-plus-btn">+</span>
      </div>
      <div class="rk-search-box">
        <span>⌕</span>
        <input type="text" placeholder="Search bots..." readonly value="Search">
      </div>
      <div class="rk-bot-list">
        {''.join(bot_rows_html)}
      </div>
      <div class="rk-user-footer">
        <div class="rk-user-badge">{user_initials}</div>
        <span class="rk-user-name">{user_name}</span>
      </div>
    </div>

    <!-- Center Pane: Chat Thread -->
    <div class="rk-chat-pane">
      <div class="rk-topbar">
        <div class="rk-active-bot">
          <div class="rk-avatar rk-avatar-sm" style="background: linear-gradient(135deg, #ea580c, #f97316);">{bot_avatar_svg}</div>
          <span class="rk-active-title">{bot_name}</span>
          <span class="rk-chevron">▾</span>
        </div>
        <div class="rk-top-right">
          <span class="rk-panel-icon">💻</span>
        </div>
      </div>

      <div class="rk-thread">
        {chat_thread_html}
      </div>

      <div class="rk-composer">
        <span class="rk-comp-plus">+</span>
        <div class="rk-comp-input">Message {bot_name}...</div>
        <div class="rk-comp-send">↑</div>
      </div>
    </div>

    <!-- Right Pane: Live Computer & Routines -->
    <div class="rk-computer-pane">
      <div class="rk-comp-head">
        <span>{bot_name}’s computer</span>
        <span class="rk-gear">⚙</span>
      </div>

      <div class="rk-screen-wrapper">
        <div class="rk-browser-window">
          <div class="rk-browser-bar">
            <span class="rk-b-dot"></span><span class="rk-b-dot"></span><span class="rk-b-dot"></span>
            <div class="rk-url-pill">{browser_url}</div>
          </div>
          <div class="rk-browser-body">
            {browser_content_html}
          </div>
        </div>
      </div>

      <div class="rk-screen-meta">
        <span>Screen feed (/screen)</span>
        <button class="rk-btn-takeover">Take control</button>
      </div>

      <div class="rk-routines-label">Routines</div>
      <div class="rk-routines-list">
        {''.join(routines_html)}
        <div class="rk-new-routine">+ New routine</div>
      </div>
    </div>
  </div>
</div>
"""
print("Loaded Rakazo component builder")
