#!/usr/bin/env python3
"""Render 8-minute showreel: 60fps, 15 scenarios, smooth animations, English."""
import os, math, subprocess, time
from PIL import Image, ImageDraw, ImageFont

# ── Config ──────────────────────────────────────────────────────────
W, H = 1280, 720
FPS = 60
DURATION = 480  # 8 minutes
TOTAL_FRAMES = DURATION * FPS
OUTPUT = os.path.join(os.path.dirname(__file__), 'public', 'videos', 'fathom_showreel.mp4')

INTRO_DUR = 10
OUTRO_DUR = 12
SCENARIO_DUR = (DURATION - INTRO_DUR - OUTRO_DUR) / 15  # ~30.5s each

WHITE = (255, 255, 255)
BLACK = (10, 10, 20)
GRAY = (120, 120, 120)
LIGHT_GRAY = (230, 230, 235)
ACCENT = (120, 80, 240)
ACCENT2 = (60, 130, 240)

MOCKUP_DIR = os.path.join(os.path.dirname(__file__), 'public', 'images', 'mockups')
FONT_DIR = '/usr/share/fonts/truetype/dejavu'

# ── Easing ──────────────────────────────────────────────────────────
def ease_out_cubic(t):
    return 1 - (1 - t) ** 3

def ease_in_out_quart(t):
    return 16 * t ** 4 if t < 0.5 else 1 - (-2 * t + 2) ** 4 / 2

def ease_out_back(t):
    c1 = 1.70158
    c3 = c1 + 1
    return 1 + c3 * (t - 1) ** 3 + c1 * (t - 1) ** 2

# ── Scenarios ───────────────────────────────────────────────────────
SCENARIOS = [
    dict(name='Sales Outbound SDR', role='Lead Generation Engine', color='#f59e0b',
         mockup='01_sales_outbound_sdr.png',
         lines=['Discover 50+ verified CISO contacts at London fintech firms',
                'SMTP validation — 250 OK in under 3 seconds per address',
                'Enrich, qualify, and push to CRM — zero bounces']),
    dict(name='Market Intelligence', role='Competitive Research', color='#3b82f6',
         mockup='02_market_intelligence.png',
         lines=['Track 15 competitors across fintech and payments',
                'Real-time alerts on pricing shifts, feature launches, exec hires',
                'Diff against stored state — every change logged']),
    dict(name='Talent Scout', role='Technical Recruiting', color='#ec4899',
         mockup='03_talent_scout.png',
         lines=['Source 30 senior Rust / systems engineers',
                'GitHub AST mining — Tokio, Axum, Polars contributors',
                'Cross-check LinkedIn, verify emails, deliver dossiers']),
    dict(name='Onboarding Agent', role='Client Setup & Triage', color='#10b981',
         mockup='13_customer_success_onboarding.png',
         lines=['Provision API keys, configure webhooks, validate payloads',
                'Isolated sandbox REPL for safe debugging',
                'Enterprise client onboarding in under 15 minutes']),
    dict(name='Finance Ops', role='Invoice Processing', color='#8b5cf6',
         mockup='04_backoffice_invoice.png',
         lines=['Ingest 500 PDF vendor invoices — parallel parsing',
                '3-way match: Purchase Order / Receipt / Invoice',
                'Stage approved payments in QuickBooks, flag discrepancies']),
    dict(name='Software Maintainer', role='Bug Triage & Fix', color='#ef4444',
         mockup='05_devops_engineer.png',
         lines=['Triage Sentry zero-division error in Python analytics CLI',
                'Map 240+ file repository, reproduce in sandbox',
                'Write fix, add test, submit PR — all tests pass']),
    dict(name='Compliance Auditor', role='Legal Risk Assessment', color='#06b6d4',
         mockup='12_legal_compliance_auditor.png',
         lines=['Audit 200 vendor MSAs for GDPR compliance',
                'Parallel ingestion across 5 analyst agents',
                'Risk matrix: Green / Yellow / Red with actionable items']),
    dict(name='Orchestrator', role='Swarm Coordinator', color='#a78bfa',
         mockup='11_swarm_coordinator.png',
         lines=['Decompose enterprise research into sub-tasks',
                'Spawn specialist agents — parallel dispatch',
                'Synthesize, verify, deliver — full lifecycle']),
    dict(name='Inbox Manager', role='Email Intelligence', color='#f97316',
         mockup='07_inbox_manager.png',
         lines=['Process 1,000+ emails per day — auto-categorize',
                'Priority triage, auto-reply, CRM sync',
                'Learn from user feedback, improve over time']),
    dict(name='Lead Audit Report', role='Data Quality Assurance', color='#14b8a6',
         mockup='08_lead_audit_report.png',
         lines=['Audit 10,000 leads for completeness and accuracy',
                'Deduplicate, validate emails, enrich missing fields',
                'Generate compliance-ready audit report']),
    dict(name='Entity Knowledge Graph', role='Relationship Mapping', color='#6366f1',
         mockup='09_entity_knowledge_graph.png',
         lines=['Extract entities from 500+ documents',
                'Build knowledge graph — companies, contacts, deals',
                'SQLite FTS5 — cross-reference and query']),
    dict(name='Security Vault', role='Credential Governance', color='#dc2626',
         mockup='10_security_credentials_vault.png',
         lines=['AES-256-GCM vault for secrets and credentials',
                'Policy engine — fail-closed, role-based access',
                'Every tool call audited, every decision logged']),
    dict(name='Outreach Dispatcher', role='Campaign Automation', color='#d946ef',
         mockup='14_outreach_campaign_dispatcher.png',
         lines=['Design multi-channel outreach campaigns',
                'Personalize at scale — email, LinkedIn, phone',
                'A/B test messaging, track open rates, optimize']),
    dict(name='Agency Fleet Manager', role='Multi-Client Operations', color='#0ea5e9',
         mockup='15_agency_fleet_manager.png',
         lines=['Manage 15+ client accounts simultaneously',
                'Dedicated worker fleet per client, isolated contexts',
                'Cross-client reporting, resource pooling, SLA tracking']),
    dict(name='Computer Use', role='Desktop Automation', color='#84cc16',
         mockup='06_computer_2fa_takeover.png',
         lines=['Control browser via CDP — click, type, navigate',
                'Fill multi-step SaaS forms autonomously',
                '2FA human takeover — seamless handoff']),
]

def hex_to_rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i+2], 16) for i in (0, 2, 4))

# ── Fonts ───────────────────────────────────────────────────────────
def load_fonts():
    return {
        'title': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 64),
        'title_sm': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 48),
        'role': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 32),
        'caption': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 28),
        'large': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 96),
        'subtitle': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 36),
        'small': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 20),
        'badge': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 18),
    }

# ── Background gradient helper ──────────────────────────────────────
def draw_white_bg(draw):
    draw.rectangle([(0, 0), (W, H)], fill=WHITE)

def draw_subtle_gradient(draw, t):
    """Very subtle animated gradient."""
    for y in range(H):
        blend = y / H
        wave = 0.5 + 0.5 * math.sin(t * 0.3 + y * 0.003)
        r = int(250 + 5 * (1 - blend))
        g = int(250 + 5 * (1 - blend))
        b = int(252 + 3 * (1 - blend) + 2 * wave)
        draw.line([(0, y), (W, y)], fill=(r, g, b))

# ── Frame renderers ─────────────────────────────────────────────────

def draw_intro_frame(img, draw, fonts, t):
    phase = t / INTRO_DUR
    draw_subtle_gradient(draw, t)

    # Title — zoom in
    zoom = 0.9 + 0.1 * min(1, phase * 2)
    title = 'FATHOM'
    title_w = fonts['large'].getlength(title) * zoom
    title_x = (W - title_w) // 2
    title_y = 160 - 20 * (1 - ease_out_cubic(min(1, phase * 1.5)))
    draw.text((title_x, title_y), title, fill=BLACK, font=fonts['large'])

    # Subtitle — fade in
    sub = 'Autonomous AI Workforce Runtime'
    sub_w = fonts['subtitle'].getlength(sub)
    sub_x = (W - sub_w) // 2
    sub_alpha = min(1, (phase - 0.08) * 6)
    if sub_alpha > 0:
        gray = tuple(int(30 + (c - 30) * sub_alpha) for c in GRAY)
        draw.text((sub_x, 270), sub, fill=gray, font=fonts['subtitle'])

    # Tagline
    tag = 'Your keys. Your model. Your workforce.'
    tag_w = fonts['role'].getlength(tag)
    tag_alpha = min(1, (phase - 0.2) * 5)
    if tag_alpha > 0:
        tg = tuple(int(100 + (c - 100) * tag_alpha) for c in GRAY)
        draw.text(((W - tag_w) // 2, 320), tag, fill=tg, font=fonts['role'])

    # Badge grid after phase 0.35
    if phase > 0.35:
        ba = min(1, (phase - 0.35) * 6)
        cols = 5
        cell_w = 220
        grid_w = cols * cell_w
        gx = (W - grid_w) // 2
        gy = 390
        for i, sc in enumerate(SCENARIOS):
            col = i % cols
            row = i // cols
            bx = gx + col * cell_w
            by = gy + row * (44 + 6)
            rgb = hex_to_rgb(sc['color'])
            bg = tuple(int(c + (255 - c) * (1 - ba)) for c in rgb)
            draw.rounded_rectangle([bx, by, bx + cell_w - 6, by + 44], radius=6, fill=bg)
            draw.text((bx + 10, by + 11), sc['name'][:18], fill=BLACK, font=fonts['badge'])

    # Bottom watermark
    draw.text((W - 200, H - 50), 'fathom.uz', fill=LIGHT_GRAY, font=fonts['small'])
    _draw_progress_bar(draw, 0)


def draw_scenario_frame(img, draw, fonts, t, scenario_idx):
    sc = SCENARIOS[scenario_idx]
    color = hex_to_rgb(sc['color'])
    phase = t / SCENARIO_DUR

    draw_white_bg(draw)

    # Colored top bar — animated pulse
    bar_h = 4 + 2 * math.sin(t * 2.5)
    draw.rectangle([(0, 0), (W, int(bar_h))], fill=color)

    # ── Title area ──
    title_alpha = ease_out_cubic(min(1, phase * 12))
    if title_alpha > 0:
        draw.text((60, 24), sc['name'], fill=BLACK, font=fonts['title_sm'])
        role_alpha = min(1, max(0, (phase - 0.06) * 10))
        if role_alpha > 0:
            rc = tuple(int(180 + 75 * role_alpha) for _ in range(3))
            draw.text((60, 76), sc['role'], fill=rc, font=fonts['role'])

    # ── Mockup ──
    mockup = mockups.get(sc['mockup'])
    if mockup:
        mw, mh = mockup.size
        mp = max(0, min(1, (phase - 0.08) * 3))  # 0.08-0.41
        ep = ease_out_cubic(mp)
        mx = (W - mw) // 2
        my = 120 + int(50 * (1 - ep))

        # Shadow
        shadow_offset = 4
        draw.rounded_rectangle(
            [mx - 2, my - 2, mx + mw + 2 + shadow_offset, my + mh + 2 + shadow_offset],
            radius=12, fill=(0, 0, 0, 20))
        # Border
        draw.rounded_rectangle([mx - 2, my - 2, mx + mw + 2, my + mh + 2], radius=12,
                               fill=LIGHT_GRAY, outline=color)
        # Paste
        if mockup.mode == 'RGBA':
            img.paste(mockup, (mx, my), mockup)
        else:
            img.paste(mockup, (mx, my))

    # ── Caption lines ──
    line_start_y = 620
    line_height = 36
    for i, line in enumerate(sc['lines']):
        lshow = (phase - 0.35 - i * 0.07) / 0.12
        if lshow > 0:
            la = min(1, lshow * 4)
            lx = 80
            ly = line_start_y + i * line_height
            # Bullet dot with bounce
            dot_r = 5 * (0.8 + 0.2 * ease_out_back(min(1, la * 2)))
            dc = tuple(int(c * la + 255 * (1 - la)) for c in color)
            draw.ellipse([lx - dot_r, ly + 10 - dot_r, lx + dot_r, ly + 10 + dot_r], fill=dc)
            # Text
            tc = tuple(int(20 + 200 * (1 - la)) for _ in range(3))
            draw.text((lx + 20, ly), line, fill=tc, font=fonts['caption'])

    _draw_progress_bar(draw, scenario_idx + 1)


def draw_outro_frame(img, draw, fonts, t):
    phase = t / OUTRO_DUR
    draw_subtle_gradient(draw, t)

    # Title
    if phase < 0.5:
        f = min(1, phase * 6)
        text = 'Try the live demo'
        tw = fonts['large'].getlength(text)
        draw.text(((W - tw) // 2, 180), text, fill=BLACK, font=fonts['large'])

        url = 'fathom.uz/demo'
        uw = fonts['title_sm'].getlength(url)
        draw.text(((W - uw) // 2, 280), url, fill=ACCENT, font=fonts['title_sm'])

        tag = 'Your keys. Your model. Your workforce.'
        tw2 = fonts['role'].getlength(tag)
        draw.text(((W - tw2) // 2, 350), tag, fill=GRAY, font=fonts['role'])

    # Bot badges
    if phase > 0.3:
        ba = min(1, (phase - 0.3) * 5)
        badge_w = 150
        total_w = 15 * badge_w
        bx = (W - total_w) // 2
        for i, sc in enumerate(SCENARIOS):
            rgb = hex_to_rgb(sc['color'])
            bc = tuple(int(c + (255 - c) * (1 - ba)) for c in rgb)
            draw.rounded_rectangle([bx + i * badge_w, 430, bx + i * badge_w + badge_w - 5, 60 + 430], radius=6, fill=bc)

    # Watermark
    draw.text((W - 200, H - 50), 'fathom.uz', fill=LIGHT_GRAY, font=fonts['small'])
    _draw_progress_bar(draw, 16)


def _draw_progress_bar(draw, current):
    """Filled progress bar at bottom."""
    bar_y = H - 20
    bar_h = 4
    bar_w = W - 120
    bar_x = 60
    filled = current / 17  # 0-16 (intro=0, 15 scenarios=1-15, outro=16)
    # Track
    draw.rounded_rectangle([bar_x, bar_y, bar_x + bar_w, bar_y + bar_h], radius=2, fill=LIGHT_GRAY)
    # Filled
    if filled > 0:
        draw.rounded_rectangle([bar_x, bar_y, bar_x + int(bar_w * filled), bar_y + bar_h], radius=2, fill=ACCENT2)
    # Dots
    for i in range(17):
        dx = bar_x + int(bar_w * (i / 16))
        r = 3 if i == current else 2
        c = ACCENT2 if i <= current else LIGHT_GRAY
        draw.ellipse([dx - r, bar_y - r, dx + r, bar_y + bar_h + r], fill=c)


# ── Main ────────────────────────────────────────────────────────────
def main():
    global mockups
    os.makedirs(os.path.dirname(OUTPUT), exist_ok=True)
    fonts = load_fonts()

    # Pre-resize mockups
    mockups = {}
    for sc in SCENARIOS:
        mp = os.path.join(MOCKUP_DIR, sc['mockup'])
        if os.path.exists(mp):
            im = Image.open(mp).convert('RGBA')
            mw, mh = im.size
            tw = 960
            th = min(480, int(mh * tw / mw))
            mockups[sc['mockup']] = im.resize((tw, th), Image.LANCZOS)
            print(f"  Resized {sc['mockup']} ({tw}x{th})")
        else:
            print(f"  WARNING: {sc['mockup']} not found")

    print(f"Rendering {TOTAL_FRAMES} frames at {FPS}fps ({DURATION}s)...")
    t0 = time.time()

    cmd = [
        'ffmpeg', '-y',
        '-f', 'rawvideo', '-pix_fmt', 'rgb24',
        '-s', f'{W}x{H}', '-r', str(FPS),
        '-i', '-',
        '-c:v', 'libx264', '-pix_fmt', 'yuv420p',
        '-crf', '20', '-preset', 'medium',
        '-movflags', '+faststart',
        OUTPUT,
    ]
    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE)

    for fi in range(TOTAL_FRAMES):
        t = fi / FPS
        img = Image.new('RGB', (W, H), WHITE)
        draw = ImageDraw.Draw(img)

        if t < INTRO_DUR:
            draw_intro_frame(img, draw, fonts, t)
        elif t < INTRO_DUR + len(SCENARIOS) * SCENARIO_DUR:
            st = t - INTRO_DUR
            si = min(int(st // SCENARIO_DUR), len(SCENARIOS) - 1)
            lt = st - si * SCENARIO_DUR
            draw_scenario_frame(img, draw, fonts, lt, si)
        else:
            ot = t - INTRO_DUR - len(SCENARIOS) * SCENARIO_DUR
            draw_outro_frame(img, draw, fonts, ot)

        proc.stdin.write(img.tobytes())

        if fi % 1800 == 0:
            elapsed = time.time() - t0
            pct = 100 * fi // TOTAL_FRAMES
            eta = elapsed / (fi + 1) * (TOTAL_FRAMES - fi - 1) if fi > 0 else 0
            print(f"  {fi}/{TOTAL_FRAMES} ({pct}%)  {elapsed:.0f}s elapsed  ~{eta:.0f}s remaining")

    proc.stdin.close()
    proc.wait()
    total = time.time() - t0
    sz = os.path.getsize(OUTPUT)
    print(f"\nDone! {total:.0f}s, {sz/1024/1024:.1f} MB")
    print(f"File: {OUTPUT}")


if __name__ == '__main__':
    main()