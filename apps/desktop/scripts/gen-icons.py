#!/usr/bin/env python3
"""Generate the Fathom Desktop icon set (PNG sizes + icns + ico) without
any image library — pure Python, minimal PNG encoder.

Icon: dark rounded square with a "voice wave" glyph (rings + cross).
"""
import struct
import zlib
import os
import subprocess
import sys

SIZES = [32, 128, 256, 512, 1024]
OUT = os.path.join(os.path.dirname(__file__), "icons")
os.makedirs(OUT, exist_ok=True)


def chunk(tag: bytes, data: bytes) -> bytes:
    c = struct.pack(">I", len(data)) + tag + data
    c += struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    return c


def png(width: int, height: int, pixels: bytes) -> bytes:
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    raw = b"".join(b"\x00" + pixels[y * width * 4:(y + 1) * width * 4]
                   for y in range(height))
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", ihdr)
            + chunk(b"IDAT", zlib.compress(raw, 9))
            + chunk(b"IEND", b""))


def make_icon(size: int) -> bytes:
    """Draw the glyph at arbitrary size into RGBA."""
    buf = bytearray(size * size * 4)

    def px(x: int, y: int, r, g, b, a):
        if 0 <= x < size and 0 <= y < size:
            i = (y * size + x) * 4
            buf[i] = r
            buf[i + 1] = g
            buf[i + 2] = b
            buf[i + 3] = a

    # Rounded-square background
    radius = int(size * 0.22)
    bg = (13, 13, 13)
    for y in range(size):
        for x in range(size):
            # rounded corner test
            dx = min(x, size - x)
            dy = min(y, size - y)
            if dx <= radius and dy <= radius:
                cx = radius
                cy = radius
                if (x % size) < radius or (size - x) <= radius:
                    # inside corner region
                    if (dx - radius) ** 2 + (dy - radius) ** 2 > radius ** 2:
                        continue
            px(x, y, *bg, 255)

    # Glyph: two concentric rings + cross, stroke via distance to circles
    cx = cy = size / 2
    ring_outer = size * 0.38
    ring_inner = size * 0.30
    stroke = max(1, size * 0.035)
    cross_len = size * 0.20
    cross_w = max(1, size * 0.05)

    for y in range(size):
        for x in range(size):
            d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
            if abs(d - ring_outer) <= stroke or abs(d - ring_inner) <= stroke:
                px(x, y, 232, 232, 232, 255)
            # cross bars
            if abs(x - cx) <= cross_w and abs(y - cy) <= cross_len:
                px(x, y, 232, 232, 232, 255)
            if abs(y - cy) <= cross_w and abs(x - cx) <= cross_len:
                px(x, y, 232, 232, 232, 255)

    return png(size, size, bytes(buf))


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--svg":
        # Also write favicon.svg
        svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <rect width="128" height="128" rx="28" fill="#0d0d0d"/>
  <circle cx="64" cy="64" r="36" fill="none" stroke="#e8e8e8" stroke-width="5"/>
  <circle cx="64" cy="64" r="26" fill="none" stroke="#e8e8e8" stroke-width="4"/>
  <path d="M64 44v40M44 64h40" stroke="#e8e8e8" stroke-width="6" stroke-linecap="round"/>
</svg>"""
        with open(os.path.join(OUT, "icon.svg"), "w") as f:
            f.write(svg)
        return

    for s in SIZES:
        with open(os.path.join(OUT, f"{s}x{s}.png"), "wb") as f:
            f.write(make_icon(s))
        print(f"wrote {s}x{s}.png")

    # 128x128@2x = 256
    with open(os.path.join(OUT, "128x128@2x.png"), "wb") as f:
        f.write(make_icon(256))
    print("wrote 128x128@2x.png")

    # icns (macOS): ICON 256 + IC08 256 + IC09 512 + IC10 512@2x
    def icns():
        entries = b""
        data = {32: b"", 256: b"", 512: b""}
        # Simplest valid: single 512 icon, type ic09
        p512 = make_icon(512)
        entries = struct.pack(">4sI", b"ic09", len(p512) + 8) + p512
        return b"icns" + struct.pack(">I", len(entries) + 8) + entries

    with open(os.path.join(OUT, "icon.icns"), "wb") as f:
        f.write(icns())
    print("wrote icon.icns")

    # ico (Windows): embed 32 and 256
    def ico():
        p32 = make_icon(32)
        p256 = make_icon(256)
        header = struct.pack("<HHH", 0, 1, 2)
        entries = b""
        offset = 6 + 16 * 2
        for p, w, h in [(p32, 32, 32), (p256, 256, 256)]:
            entries += struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32,
                                   len(p), offset)
            offset += len(p)
        return header + entries + p32 + p256

    with open(os.path.join(OUT, "icon.ico"), "wb") as f:
        f.write(ico())
    print("wrote icon.ico")


if __name__ == "__main__":
    main()