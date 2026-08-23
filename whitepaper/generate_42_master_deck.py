#!/usr/bin/env python3
import os, subprocess, concurrent.futures, tempfile, shutil
from pypdf import PdfWriter

WP_DIR = "/Users/yakushev/Documents/GitHub/Fathom/whitepaper"
CHROME_PATH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

# Make sure CSS has deck-mockup-frame classes
EXTRA_CSS = """
/* Embedded HD Mockup Screenshot Component */
.deck-mockup-container {
  width: 100%;
  margin: 5px 0 6px 0;
  display: flex;
  flex-direction: column;
  align-items: center;
}
.deck-mockup-frame {
  width: 100%;
  border-radius: 7px;
  overflow: hidden;
  border: 1px solid #1e293b;
  box-shadow: 0 4px 16px rgba(15, 23, 42, 0.25);
  background: #09090b;
}
.deck-mockup-frame img {
  width: 100%;
  height: auto;
  display: block;
}
.deck-mockup-caption {
  font-size: 7pt;
  color: #64748b;
  font-family: 'JetBrains Mono', monospace;
  margin-top: 3px;
  text-align: center;
  font-weight: 500;
}
"""

with open(os.path.join(WP_DIR, "styles.css"), "r") as f:
    existing_css = f.read()

if ".deck-mockup-frame" not in existing_css:
    with open(os.path.join(WP_DIR, "styles.css"), "a") as f:
        f.write(EXTRA_CSS)
    print("Appended deck mockup CSS rules to styles.css")

pages = []

def add_page(num, category, title, subtitle, content_html):
    pages.append({
        "num": num,
        "category": category,
        "title": title,
        "subtitle": subtitle,
        "html": content_html.strip()
    })

# Helper for embedding HD screenshot
def mockup_img(filename, caption_text):
    return f"""
    <div class="deck-mockup-container">
      <div class="deck-mockup-frame">
        <img src="mockups/{filename}" alt="{caption_text}">
      </div>
      <div class="deck-mockup-caption">{caption_text}</div>
    </div>
    """

# ==============================================================================
# READ BASE 42 PAGES TEMPLATE AND INJECT 15 HD SCREENSHOTS
# ==============================================================================

# Let's import the 42 page definitions from generate_42_dense_with_mockups.py
# and substitute the mockup pages with the real PNG images!

import importlib.util
spec = importlib.util.spec_from_file_location("dense_mod", os.path.join(WP_DIR, "generate_42_dense_with_mockups.py"))
dense_mod = importlib.util.module_from_spec(spec)

# Mock add_page inside dense_mod
dense_mod.pages = []
spec.loader.exec_module(dense_mod)

raw_pages = dense_mod.pages
print(f"Loaded {len(raw_pages)} base pages from template.")

# Map of Page Num -> (Image Filename, Caption)
MOCKUP_MAPPING = {
    4: ("07_inbox_manager.png", "Figure 4.1: Autonomous Chief of Staff — 41-thread inbox sweep, noise archival & held executive drafts"),
    6: ("01_sales_outbound_sdr.png", "Figure 6.1: Autonomous Outbound SDR — Corporate registry discovery, SMTP 250 OK verification & amoCRM pipeline sync"),
    7: ("02_market_intelligence.png", "Figure 7.1: Real-Time Market Intelligence — DOM diff tracker detecting competitor tier pricing shifts"),
    8: ("03_talent_scout.png", "Figure 8.1: Executive Talent Scout — Mining GitHub AST repositories and constructing verified candidate dossiers"),
    9: ("04_backoffice_invoice.png", "Figure 9.1: Back-Office Assistant — 3-way invoice reconciliation across $482,000 transaction volume in QuickBooks"),
    10: ("05_devops_engineer.png", "Figure 10.1: DevOps Maintainer — AST symbol search, zero-division error reproduction, and GitHub PR #142 creation"),
    12: ("11_swarm_coordinator.png", "Figure 12.1: Swarm Coordinator — Tokio JoinSet DAG execution across 4 parallel CPU worker pods with fair-share token budgets"),
    17: ("14_outreach_campaign_dispatcher.png", "Figure 17.1: Cold Outreach Dispatcher — 500-lead campaign staging with 4 Spintax variants and DoH DNS MX/SPF verification"),
    19: ("08_lead_audit_report.png", "Figure 19.1: Value Audit Deliverable — 50 verified DACH FinTech leads table ready for Excel/XLSX export"),
    23: ("06_computer_2fa_takeover.png", "Figure 23.1: Governed Computer Use — Safe pause on 2FA SMS challenge with live operator takeover lease"),
    24: ("13_customer_success_onboarding.png", "Figure 24.1: Customer Success & Webhook REPL — P1 enterprise ticket diagnosed in 12s via isolated sandbox"),
    26: ("10_security_credentials_vault.png", "Figure 26.1: Enterprise Security Vault — AES-256-GCM hardware key derivation with zero LLM prompt exposure"),
    27: ("12_legal_compliance_auditor.png", "Figure 27.1: Legal Compliance Auditor — 200 vendor MSAs evaluated for GDPR clauses and liability cap risk matrix"),
    30: ("09_entity_knowledge_graph.png", "Figure 30.1: Entity Knowledge Graph — 3-hop relationship traversal across 5,420 nodes in 1.62 ms"),
    37: ("15_agency_fleet_manager.png", "Figure 37.1: Agency White-Label Fleet Manager — 12 client worker pods operating in isolation with 92% net profit margin")
}

# Now rebuild pages with injected HD mockups
final_pages = []

for p in raw_pages:
    num = p["num"]
    cat = p["category"]
    title = p["title"]
    subtitle = p["subtitle"]
    html = p["html"]

    # If this page has a designated HD mockup, replace any inline app-mockup with the clean HD image
    if num in MOCKUP_MAPPING:
        img_name, caption = MOCKUP_MAPPING[num]
        hd_mockup_block = mockup_img(img_name, caption)
        
        # Replace existing inline mockup if present
        if '<div class="app-mockup">' in html:
            # Cut out the inline mockup and replace with image
            parts = html.split('<div class="app-mockup">')
            before = parts[0]
            after = parts[1].split('</div>\n  </div>\n</div>')[-1]
            html = before + hd_mockup_block + after
        else:
            # Insert before the last grid or at bottom
            html = hd_mockup_block + html

    final_pages.append({
        "num": num,
        "category": cat,
        "title": title,
        "subtitle": subtitle,
        "html": html
    })

print(f"Prepared all {len(final_pages)} pages with 15 HD screenshot injections.")

# Output individual HTML page files
for p in final_pages:
    num = p["num"]
    page_html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Page {num:02d} · {p['title']}</title>
<link rel="stylesheet" href="styles.css">
</head>
<body>

<div class="page">
  <!-- Header -->
  <div class="header">
    <div>
      <div class="header-category">{p['category']}</div>
      <div class="header-title">{p['title']}</div>
      <div class="header-subtitle">{p['subtitle']}</div>
    </div>
    <div class="header-brand">
      <div class="brand-name">FATHOM</div>
      <div class="brand-sub">Universal AI Workforce</div>
    </div>
  </div>

  <!-- Content -->
  <div class="content-body">
    {p['html']}
  </div>

  <!-- Footer -->
  <div class="footer">
    <div class="footer-left">FATHOM WHITEPAPER · AUTONOMOUS REMOTE WORKFORCE RUNTIME</div>
    <div class="footer-center">CONFIDENTIAL & PROPRIETARY</div>
    <div class="footer-page-num">{num:02d} / 42</div>
  </div>
</div>

</body>
</html>"""
    
    with open(os.path.join(WP_DIR, f"page_{num:02d}.html"), "w", encoding="utf-8") as f:
        f.write(page_html)

# Also generate full multi-page HTML file
all_pages_html = []
for p in final_pages:
    all_pages_html.append(f"""
<div class="page" id="page-{p['num']}">
  <div class="header">
    <div>
      <div class="header-category">{p['category']}</div>
      <div class="header-title">{p['title']}</div>
      <div class="header-subtitle">{p['subtitle']}</div>
    </div>
    <div class="header-brand">
      <div class="brand-name">FATHOM</div>
      <div class="brand-sub">Universal AI Workforce</div>
    </div>
  </div>

  <div class="content-body">
    {p['html']}
  </div>

  <div class="footer">
    <div class="footer-left">FATHOM WHITEPAPER · AUTONOMOUS REMOTE WORKFORCE RUNTIME</div>
    <div class="footer-center">CONFIDENTIAL & PROPRIETARY</div>
    <div class="footer-page-num">{p['num']:02d} / 42</div>
  </div>
</div>
""")

full_html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Fathom Master Whitepaper (42 Pages)</title>
<link rel="stylesheet" href="styles.css">
</head>
<body>
{''.join(all_pages_html)}
</body>
</html>"""

with open(os.path.join(WP_DIR, "Fathom_Full_Whitepaper.html"), "w", encoding="utf-8") as f:
    f.write(full_html)

print("Generated all 42 page HTML files and Fathom_Full_Whitepaper.html.")

# ==============================================================================
# PARALLEL COMPILATION OF ALL 42 PAGES INTO MASTER PDF
# ==============================================================================

def render_one_page(num):
    html_f = os.path.join(WP_DIR, f"page_{num:02d}.html")
    pdf_f = os.path.join(WP_DIR, f"page_{num:02d}.pdf")
    tmp_d = tempfile.mkdtemp(prefix=f"deck_page_{num:02d}_")
    cmd = [
        CHROME_PATH,
        "--headless",
        "--disable-gpu",
        "--no-margins",
        f"--user-data-dir={tmp_d}",
        f"--print-to-pdf={pdf_f}",
        f"file://{html_f}"
    ]
    subprocess.run(cmd, capture_output=True)
    shutil.rmtree(tmp_d, ignore_errors=True)
    if os.path.exists(pdf_f) and os.path.getsize(pdf_f) > 0:
        return (num, pdf_f, os.path.getsize(pdf_f))
    return (num, None, 0)

print("\nStarting parallel Chrome PDF rendering across 8 workers...")
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as ex:
    results = list(ex.map(render_one_page, range(1, 43)))

results.sort(key=lambda x: x[0])
valid_pdfs = []
for idx, path, size in results:
    if path:
        print(f"  [{idx:02d}/42] OK: {size:,} bytes")
        valid_pdfs.append(path)
    else:
        print(f"  [{idx:02d}/42] FAILED")

print(f"\nMerging {len(valid_pdfs)} PDFs into Master PDF...")
writer = PdfWriter()
for pdf in valid_pdfs:
    writer.append(pdf)

out_pdf = os.path.join(WP_DIR, "Fathom_Whitepaper.pdf")
with open(out_pdf, "wb") as f:
    writer.write(f)
writer.close()

print(f"\n=======================================================")
print(f"SUCCESS: Master Whitepaper PDF Compiled!")
print(f"Path: {out_pdf}")
print(f"Size: {os.path.getsize(out_pdf):,} bytes")
print(f"Total Pages: {len(valid_pdfs)}")
print(f"=======================================================\n")
