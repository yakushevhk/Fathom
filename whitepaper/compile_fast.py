#!/usr/bin/env python3
import os, glob, subprocess, concurrent.futures, shutil
from pypdf import PdfWriter

WP_DIR = os.path.dirname(os.path.abspath(__file__))
CHROME_PATH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

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

print("Starting true parallel rendering across 10 workers...")
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
print(f"SUCCESS: 42-Page Master Whitepaper PDF Generated!")
print(f"Location: {master_pdf_path}")
print(f"Size: {os.path.getsize(master_pdf_path):,} bytes")
print(f"Total Pages: {len(valid_pdfs)}")
print(f"=======================================================\n")
