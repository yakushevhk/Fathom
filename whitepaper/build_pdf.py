#!/usr/bin/env python3
import os
import subprocess
import re
from pypdf import PdfWriter

CHROME_PATH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
WP_DIR = os.path.dirname(os.path.abspath(__file__))

print("Building Master Full Whitepaper HTML...")
pages_content = []
for i in range(1, 15):
    fname = f"page_{i:02d}.html"
    fpath = os.path.join(WP_DIR, fname)
    if os.path.exists(fpath):
        with open(fpath, "r", encoding="utf-8") as f:
            html = f.read()
            match = re.search(r'<div class="page">([\s\S]*?)</div>\s*</body>', html)
            if match:
                pages_content.append(f'<div class="page">\n{match.group(1)}\n</div>')
            else:
                print(f"Warning: Could not extract page content from {fname}")

full_html = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Fathom Whitepaper — Universal Autonomous AI Workforce Runtime</title>
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
{"\n".join(pages_content)}
</body>
</html>
"""

full_html_path = os.path.join(WP_DIR, "Fathom_Full_Whitepaper.html")
with open(full_html_path, "w", encoding="utf-8") as f:
    f.write(full_html)
print(f"Saved: {full_html_path}")

print("\nRendering Individual Page PDFs via Headless Google Chrome...")
pdf_files = []
for i in range(1, 15):
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
            print(f"Generated [{i:02d}/14]: {pdf_name} ({os.path.getsize(pdf_path):,} bytes)")
            pdf_files.append(pdf_path)
        else:
            print(f"Failed to generate {pdf_name}: {res.stderr}")

print("\nMerging into Master Whitepaper PDF...")
master_pdf_path = os.path.join(WP_DIR, "Fathom_Whitepaper.pdf")
writer = PdfWriter()
for pdf in pdf_files:
    writer.append(pdf)

with open(master_pdf_path, "wb") as f:
    writer.write(f)
writer.close()

print(f"\nSUCCESS: Master Whitepaper PDF generated at {master_pdf_path} ({os.path.getsize(master_pdf_path):,} bytes)")
