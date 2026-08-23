#!/usr/bin/env python3
import os, subprocess, concurrent.futures, tempfile, shutil
from pypdf import PdfWriter

wp = '/Users/yakushev/Documents/GitHub/Fathom/whitepaper'
chrome = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'

def render_one(i):
    html = f'{wp}/page_{i:02d}.html'
    pdf = f'{wp}/page_{i:02d}.pdf'
    tmp_dir = tempfile.mkdtemp(prefix=f'cw_{i:02d}_')
    cmd = [
        chrome,
        '--headless',
        '--disable-gpu',
        '--no-margins',
        f'--user-data-dir={tmp_dir}',
        f'--print-to-pdf={pdf}',
        f'file://{html}'
    ]
    subprocess.run(cmd, capture_output=True)
    shutil.rmtree(tmp_dir, ignore_errors=True)
    if os.path.exists(pdf) and os.path.getsize(pdf) > 0:
        return (i, pdf, os.path.getsize(pdf))
    return (i, None, 0)

print("Starting super fast parallel render with 8 workers...")
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as ex:
    results = list(ex.map(render_one, range(1, 43)))

results.sort(key=lambda x: x[0])
valid_pdfs = []
for idx, path, size in results:
    if path:
        print(f"[{idx:02d}/42] OK: {size:,} bytes")
        valid_pdfs.append(path)
    else:
        print(f"[{idx:02d}/42] FAILED")

print(f"\nMerging {len(valid_pdfs)} PDFs into Master...")
writer = PdfWriter()
for pdf in valid_pdfs:
    writer.append(pdf)

out = f'{wp}/Fathom_Whitepaper.pdf'
with open(out, 'wb') as f:
    writer.write(f)
writer.close()

print(f"\nSUCCESS! Master PDF: {out} ({os.path.getsize(out):,} bytes, {len(valid_pdfs)} pages)")
