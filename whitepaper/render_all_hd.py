#!/usr/bin/env python3
import os, subprocess, concurrent.futures, tempfile, shutil

MOCKUPS_DIR = "/Users/yakushev/Documents/GitHub/Fathom/whitepaper/mockups"
CHROME_PATH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

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

def render_png(name):
    html_p = os.path.join(MOCKUPS_DIR, f"{name}.html")
    png_p = os.path.join(MOCKUPS_DIR, f"{name}.png")
    tmp_d = tempfile.mkdtemp(prefix=f"hd_rend_{name}_")
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
    if os.path.exists(png_p) and os.path.getsize(png_p) > 0:
        return (name, png_p, os.path.getsize(png_p))
    return (name, None, 0)

print("Rendering all 10 HD Retina screenshots in parallel...")
with concurrent.futures.ThreadPoolExecutor(max_workers=6) as ex:
    results = list(ex.map(render_png, mockups))

for name, path, size in results:
    if path:
        print(f"✓ OK [HD Retina]: {name}.png ({size:,} bytes)")
    else:
        print(f"✗ FAILED: {name}")

print("\nSUCCESS: All 10 HD PNGs are ready!")
