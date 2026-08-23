#!/usr/bin/env python3
import os, glob
from bs4 import BeautifulSoup

wp = '/Users/yakushev/Documents/GitHub/Fathom/whitepaper'

print(f"{'Page':<6} | {'HTML Size':<10} | {'Cards':<6} | {'Tables':<6} | {'Images':<8} | {'Old Mockups':<12} | {'Text Length':<12}")
print("-" * 75)

sparse_pages = []

for i in range(1, 43):
    html_f = f"{wp}/page_{i:02d}.html"
    if not os.path.exists(html_f):
        print(f"Page {i:02d}: MISSING")
        sparse_pages.append(i)
        continue
    
    with open(html_f, "r") as f:
        content = f.read()
    
    soup = BeautifulSoup(content, "html.parser")
    cards = len(soup.find_all(class_=lambda c: c and "card" in c))
    tables = len(soup.find_all("table"))
    images = len(soup.find_all("img"))
    old_mockups = len(soup.find_all(class_=lambda c: c and ("app-mockup" in c or "mockup-3col" in c or "mockup-body" in c)))
    text = soup.get_text()
    text_len = len(text.strip().split())
    
    status = ""
    if old_mockups > 0:
        status += " [OLD MOCKUP]"
    if text_len < 250 and images == 0:
        status += " [SPARSE <250 words]"
        sparse_pages.append(i)
    elif text_len < 150:
        status += " [VERY SPARSE]"
        sparse_pages.append(i)
        
    print(f"Page {i:02d} | {len(content):<10,} | {cards:<6} | {tables:<6} | {images:<8} | {old_mockups:<12} | {text_len:<12}{status}")

print(f"\nTotal Sparse Pages to Enrich: {len(sparse_pages)}")
print(f"Pages: {sparse_pages}")
