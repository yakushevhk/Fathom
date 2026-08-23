#!/usr/bin/env python3
"""Render 4-minute showreel video for Fathom /video page.

Frame-by-frame PIL + pipe to ffmpeg.  White background, 8 demo scenarios.
"""

import os, math, subprocess, json
from PIL import Image, ImageDraw, ImageFont

# ── Config ──────────────────────────────────────────────────────────
W, H = 1280, 720
FPS = 15
DURATION = 240  # 4 minutes
TOTAL_FRAMES = DURATION * FPS
OUTPUT = os.path.join(os.path.dirname(__file__), 'public', 'videos', 'fathom_showreel.mp4')

# Timing (in seconds)
INTRO_DUR = 10
OUTRO_DUR = 8
SCENARIO_DUR = (DURATION - INTRO_DUR - OUTRO_DUR) / 8  # ~27.75s each

# Colors
WHITE = (255, 255, 255)
BLACK = (10, 10, 20)
GRAY = (120, 120, 120)
LIGHT_GRAY = (230, 230, 235)
ACCENT = (120, 80, 240)  # purple accent
ACCENT2 = (60, 130, 240)  # blue accent

# Paths
MOCKUP_DIR = os.path.join(os.path.dirname(__file__), 'public', 'images', 'mockups')
FONT_DIR = '/usr/share/fonts/truetype/dejavu'

# ── Scenarios ───────────────────────────────────────────────────────
SCENARIOS = [
    {
        'name': 'Sales Outbound SDR',
        'role': 'SDR · Lead Generation',
        'color': '#f59e0b',  # amber
        'mockup': '01_sales_outbound_sdr.png',
        'lines': [
            'Enrich 50 verified CISO contacts at London fintech firms',
            'SMTP verification · CRM staging · Zero bounces',
            'Parallel email validation with real-time deliverability',
        ],
    },
    {
        'name': 'Market Intelligence',
        'role': 'Researcher · Competitive Monitoring',
        'color': '#3b82f6',
        'mockup': '02_market_intelligence.png',
        'lines': [
            'Monitor 15 competitors across fintech & payments',
            'Pricing changes · Feature launches · Executive hires',
            'Diff against stored state, alert on any shift',
        ],
    },
    {
        'name': 'Talent Scout',
        'role': 'Researcher · Technical Recruiting',
        'color': '#ec4899',
        'mockup': '03_talent_scout.png',
        'lines': [
            'Source 30 senior Rust/systems engineers',
            'GitHub AST mining · LinkedIn cross-check',
            'Commit-level icebreakers · Email verification',
        ],
    },
    {
        'name': 'Onboarding Agent',
        'role': 'Analyst · Client Setup',
        'color': '#10b981',
        'mockup': '13_customer_success_onboarding.png',
        'lines': [
            'API key provisioning · Webhook configuration',
            'Test payload verification · Sandbox validation',
            'Diagnose setup errors in isolated REPL',
        ],
    },
    {
        'name': 'Finance Ops',
        'role': 'Analyst · Invoice Processing',
        'color': '#8b5cf6',
        'mockup': '04_backoffice_invoice.png',
        'lines': [
            'Ingest 500 PDF vendor invoices',
            '3-way match: PO · Receipt · Invoice',
            'Stage approved payments in QuickBooks',
        ],
    },
    {
        'name': 'Software Maintainer',
        'role': 'Developer · Bug Fixing',
        'color': '#ef4444',
        'mockup': '05_devops_engineer.png',
        'lines': [
            'Triage Sentry zero-division error in Python CLI',
            'Map 240+ file repo · Reproduce in sandbox',
            'Write fix + test · Submit PR',
        ],
    },
    {
        'name': 'Compliance Auditor',
        'role': 'Verifier · Legal Review',
        'color': '#06b6d4',
        'mockup': '12_legal_compliance_auditor.png',
        'lines': [
            'Audit 200 vendor MSAs for GDPR compliance',
            'Data liability caps · Non-compete clauses',
            'Parallel ingestion across 5 analysts',
        ],
    },
    {
        'name': 'Orchestrator',
        'role': 'Coordinator · Swarm Manager',
        'color': '#a78bfa',
        'mockup': '11_swarm_coordinator.png',
        'lines': [
            'Decompose enterprise research task',
            'Spawn specialist sub-agents · Parallel dispatch',
            'Synthesize · Verify · Deliver',
        ],
    },
]

# ── Fonts ───────────────────────────────────────────────────────────
def load_fonts():
    return {
        'title': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 64),
        'title_sm': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 48),
        'role': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 32),
        'caption': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 28),
        'caption_bold': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 28),
        'large': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 96),
        'subtitle': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 36),
        'small': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 20),
        'badge': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 18),
    }

def hex_to_rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i+2], 16) for i in (0, 2, 4))

# ── Frame Renderers ─────────────────────────────────────────────────

def draw_intro_frame(img, draw, fonts, t, mockups):
    """t in [0, INTRO_DUR) seconds."""
    # Background with subtle gradient
    for y in range(H):
        blend = y / H
        r = int(250 + 5 * (1 - blend))
        g = int(250 + 5 * (1 - blend))
        b = int(252 + 3 * (1 - blend))
        draw.line([(0, y), (W, y)], fill=(r, g, b))

    # Phase
    phase = t / INTRO_DUR  # 0→1

    # Title
    title = 'FATHOM'
    title_w = fonts['large'].getlength(title)
    title_x = (W - title_w) // 2
    title_y = 180

    # Slide in from bottom
    offset = max(0, 60 * (1 - phase * 3))  # first 0.33 of intro
    draw.text((title_x, title_y - offset), title, fill=BLACK, font=fonts['large'])

    # Subtitle with fade-in
    subtitle = 'Autonomous AI Workforce Runtime'
    sub_w = fonts['subtitle'].getlength(subtitle)
    sub_x = (W - sub_w) // 2
    sub_y = 280
    alpha = min(1, phase * 4)  # fade in over first 0.25
    sub_color = tuple(int(30 + (c - 30) * alpha) for c in GRAY)
    draw.text((sub_x, sub_y - offset), subtitle, fill=sub_color, font=fonts['subtitle'])

    # Badge grid — 8 bots in 2 rows
    if phase > 0.3:
        badge_alpha = min(1, (phase - 0.3) * 5)
        cols = 4
        cell_w = 280
        cell_h = 50
        grid_w = cols * cell_w
        grid_x = (W - grid_w) // 2
        grid_y = 380

        for i, sc in enumerate(SCENARIOS):
            col = i % cols
            row = i // cols
            bx = grid_x + col * cell_w
            by = grid_y + row * (cell_h + 10)

            # Badge bg
            color = hex_to_rgb(sc['color'])
            bg_color = tuple(int(c + (255 - c) * (1 - badge_alpha)) for c in color)
            draw.rounded_rectangle([bx, by, bx + cell_w - 10, by + cell_h], radius=8, fill=bg_color + (int(255 * badge_alpha),) if hasattr(ImageDraw, 'rounded_rectangle') else bg_color)

            # Badge text
            draw.text((bx + 12, by + 12), sc['name'], fill=BLACK, font=fonts['badge'])

    # "Fathom" watermark bottom
    draw.text((W - 200, H - 50), 'fathom.uz', fill=LIGHT_GRAY, font=fonts['small'])

    # Bottom progress
    _draw_progress(draw, fonts, 0, phase)


def draw_scenario_frame(img, draw, fonts, t, scenario_idx, mockups):
    """Render one scenario frame at time t (0 → SCENARIO_DUR)."""
    sc = SCENARIOS[scenario_idx]
    color = hex_to_rgb(sc['color'])

    # White background
    draw.rectangle([(0, 0), (W, H)], fill=WHITE)

    # Subtle header bar
    draw.rectangle([(0, 0), (W, 90)], fill=color + (50,) if hasattr(ImageDraw, 'rectangle') else color)

    # Phase within scenario
    phase = t / SCENARIO_DUR  # 0→1

    # ── Title area ──
    title_alpha = min(1, phase * 8)  # fade in over 0.125 of scenario
    title_y = 30
    title_color = tuple(int(max(0, 255 - (1 - title_alpha) * 200)) for _ in range(3))
    draw.text((60, title_y), sc['name'], fill=title_color, font=fonts['title_sm'])

    # Role badge
    role_alpha = min(1, max(0, (phase - 0.1) * 10))
    if role_alpha > 0:
        role_color = tuple(int(200 + 55 * role_alpha) for _ in range(3))
        draw.text((60, title_y + 55), sc['role'], fill=role_color, font=fonts['role'])

    # ── Mockup screenshot ──
    mockup_phase = min(1, max(0, (phase - 0.1) * 3))  # slide in 0.1-0.43
    mockup = mockups.get(sc['mockup'])
    if mockup:
        mw, mh = mockup.size
        mockup_resized = mockup

        # Position
        mx = (W - mw) // 2
        my = 120 + int(30 * (1 - mockup_phase))  # slide down from above

        # Shadow
        shadow_offset = 4
        draw.rounded_rectangle([mx - 2, my - 2, mx + mw + 2 + shadow_offset, my + mh + 2 + shadow_offset],
                               radius=12, fill=(0, 0, 0, 20) if hasattr(ImageDraw, 'rounded_rectangle') else LIGHT_GRAY)

        # Border
        draw.rounded_rectangle([mx - 2, my - 2, mx + mw + 2, my + mh + 2], radius=12,
                               fill=LIGHT_GRAY, outline=color)

        # Paste mockup
        if mockup.mode == 'RGBA':
            img.paste(mockup_resized, (mx, my), mockup_resized)
        else:
            img.paste(mockup_resized, (mx, my))

    # ── Caption lines ──
    line_start_y = 620
    line_height = 36
    for i, line in enumerate(sc['lines']):
        line_show = (phase - 0.35 - i * 0.08) / 0.15  # each line appears 0.15s after previous
        if line_show > 0:
            line_alpha = min(1, line_show * 3)
            line_x = 80
            line_y = line_start_y + i * line_height
            # Bullet dot
            dot_color = tuple(int(a * line_alpha + 255 * (1 - line_alpha)) for a in color)
            draw.ellipse([line_x - 5, line_y + 8, line_x + 5, line_y + 18], fill=dot_color)
            # Text
            text_color = tuple(int(20 + 200 * (1 - line_alpha)) for _ in range(3))  # fade in from white
            draw.text((line_x + 20, line_y), line, fill=text_color, font=fonts['caption'])

    # ── Bottom progress ──
    _draw_progress(draw, fonts, scenario_idx + 1, phase)


def draw_outro_frame(img, draw, fonts, t, mockups):
    """t in [0, OUTRO_DUR) seconds."""
    phase = t / OUTRO_DUR

    # Background with subtle gradient
    for y in range(H):
        blend = y / H
        r = int(245 + 10 * (1 - blend))
        g = int(245 + 10 * (1 - blend))
        b = int(250 + 5 * (1 - blend))
        draw.line([(0, y), (W, y)], fill=(r, g, b))

    # Fade out first half, fade in second half
    fade = min(1, phase * 2) if phase < 0.5 else min(1, (1 - phase) * 2)

    # "Try the live demo"
    text = 'Try the live demo'
    tw = fonts['large'].getlength(text)
    draw.text(((W - tw) // 2, 200), text, fill=BLACK, font=fonts['large'])

    # URL
    url = 'fathom.uz/demo'
    uw = fonts['title_sm'].getlength(url)
    draw.text(((W - uw) // 2, 300), url, fill=ACCENT, font=fonts['title_sm'])

    # Tagline
    tag = 'Your keys. Your model. Your workforce.'
    tagw = fonts['role'].getlength(tag)
    draw.text(((W - tagw) // 2, 380), tag, fill=GRAY, font=fonts['role'])

    # Bot badges row
    if phase > 0.3:
        row_alpha = min(1, (phase - 0.3) * 5)
        badge_w = 140
        total_w = 8 * badge_w
        bx = (W - total_w) // 2
        for i, sc in enumerate(SCENARIOS):
            color = hex_to_rgb(sc['color'])
            bc = tuple(int(c + (255 - c) * (1 - row_alpha)) for c in color)
            draw.rounded_rectangle([bx + i * badge_w, 450, bx + i * badge_w + badge_w - 5, 480], radius=6, fill=bc)

    # Watermark
    draw.text((W - 200, H - 50), 'fathom.uz', fill=LIGHT_GRAY, font=fonts['small'])

    # Bottom progress
    _draw_progress(draw, fonts, 9, phase)


def _draw_progress(draw, fonts, current, phase):
    """Dots at bottom showing progress through scenarios."""
    n = len(SCENARIOS) + 2  # intro + 8 scenarios + outro
    dot_r = 5
    gap = 30
    total_w = n * gap
    dx = (W - total_w) // 2
    dy = H - 35

    for i in range(n):
        x = dx + i * gap + gap // 2
        is_active = (i == current) or (i == current - 1 and phase < 0.5)
        if is_active:
            draw.ellipse([x - dot_r, dy - dot_r, x + dot_r, dy + dot_r], fill=ACCENT2)
        else:
            draw.ellipse([x - dot_r, dy - dot_r, x + dot_r, dy + dot_r], fill=LIGHT_GRAY)


# ── Main ────────────────────────────────────────────────────────────
def main():
    os.makedirs(os.path.dirname(OUTPUT), exist_ok=True)
    fonts = load_fonts()

    # Load mockups
    mockups = {}
    for sc in SCENARIOS:
        mpath = os.path.join(MOCKUP_DIR, sc['mockup'])
        if os.path.exists(mpath):
            im = Image.open(mpath).convert('RGBA')
            mw0, mh0 = im.size
            tw = 960
            th = min(480, int(mh0 * tw / mw0))
            mockups[sc['mockup']] = im.resize((tw, th), Image.LANCZOS)
            print(f"  Resized {sc['mockup']} ({tw}x{th})")
        else:
            print(f"  WARNING: {sc['mockup']} not found")

    print(f"Rendering {TOTAL_FRAMES} frames...")

    # Pipe to ffmpeg
    cmd = [
        'ffmpeg', '-y',
        '-f', 'rawvideo',
        '-pix_fmt', 'rgb24',
        '-s', f'{W}x{H}',
        '-r', str(FPS),
        '-i', '-',
        '-c:v', 'libx264',
        '-pix_fmt', 'yuv420p',
        '-crf', '20',
        '-preset', 'medium',
        '-movflags', '+faststart',
        OUTPUT,
    ]
    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE)

    for frame_idx in range(TOTAL_FRAMES):
        t = frame_idx / FPS  # current time in seconds

        # Create frame
        img = Image.new('RGB', (W, H), WHITE)
        draw = ImageDraw.Draw(img)

        if t < INTRO_DUR:
            draw_intro_frame(img, draw, fonts, t, mockups)
        elif t < INTRO_DUR + len(SCENARIOS) * SCENARIO_DUR:
            scenario_t = t - INTRO_DUR
            scenario_idx = int(scenario_t // SCENARIO_DUR)
            local_t = scenario_t - scenario_idx * SCENARIO_DUR
            draw_scenario_frame(img, draw, fonts, local_t, scenario_idx, mockups)
        else:
            outro_t = t - INTRO_DUR - len(SCENARIOS) * SCENARIO_DUR
            draw_outro_frame(img, draw, fonts, outro_t, mockups)

        # Write raw frame to ffmpeg
        proc.stdin.write(img.tobytes())

        if frame_idx % 300 == 0:
            print(f"  {frame_idx}/{TOTAL_FRAMES} ({100 * frame_idx // TOTAL_FRAMES}%)")

    proc.stdin.close()
    proc.wait()
    print(f"\nDone! Output: {OUTPUT}")
    size = os.path.getsize(OUTPUT)
    print(f"Size: {size / 1024 / 1024:.1f} MB")


if __name__ == '__main__':
    main()