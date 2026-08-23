#!/usr/bin/env python3
"""Render 8-min showreel v2: dynamic — cursor, clicks, Ken Burns zoom, live terminal logs."""
import os, math, subprocess, time
from PIL import Image, ImageDraw, ImageFont, ImageFilter

W, H = 1280, 720
FPS = 60
DURATION = 480
TOTAL_FRAMES = DURATION * FPS
OUTPUT = os.path.join(os.path.dirname(__file__), 'public', 'videos', 'fathom_showreel.mp4')

INTRO_DUR = 10
OUTRO_DUR = 12
SCENARIO_DUR = (DURATION - INTRO_DUR - OUTRO_DUR) / 15

WHITE = (255, 255, 255)
BLACK = (8, 8, 16)
DARK = (17, 17, 34)
GRAY = (120, 120, 120)
LIGHT_GRAY = (232, 232, 238)
ACCENT = (120, 80, 240)
ACCENT2 = (60, 130, 240)

MOCKUP_DIR = os.path.join(os.path.dirname(__file__), 'public', 'images', 'mockups')
FONT_DIR = '/usr/share/fonts/truetype/dejavu'

def ease_out_cubic(t): return 1 - (1 - t) ** 3
def ease_in_out(t): return 16 * t ** 4 if t < 0.5 else 1 - (-2 * t + 2) ** 4 / 2
def clamp01(t): return max(0.0, min(1.0, t))

# Cursor texture (pre-drawn, 24x20)
def make_cursor():
    c = Image.new('RGBA', (26, 22), (0, 0, 0, 0))
    d = ImageDraw.Draw(c)
    # Arrow pointing up-left
    d.polygon([(2, 2), (21, 12), (13, 14), (16, 20), (12, 21), (9, 15), (2, 19)], fill=(0, 0, 0, 200))
    d.polygon([(2, 2), (19, 11), (13, 13), (6, 18)], fill=(255, 255, 255, 255))
    return c
CURSOR = make_cursor()

SCENARIOS = [
    dict(name='Sales Outbound SDR', role='Lead Generation Engine', color='#f59e0b',
         mockup='01_sales_outbound_sdr.png',
         clicks=[(0.25, 0.35), (0.55, 0.28), (0.45, 0.62), (0.78, 0.4)],
         logs=['$ fathom run --task "enrich 50 CISOs"',
               '[PLAN]  decompose → 4 sub-tasks',
               '[TOOL]  smtp_verify ciso@bank.co.uk → 250 OK',
               '[TOOL]  enrich_company London Fintech Ltd — 12 rows',
               '[MEMORY]  absorbed 47 verified contacts',
               '[VERIFY]  bounce rate 0.0% — PASS',
               '[FINAL]  pushed 50 leads to HubSpot']),
    dict(name='Market Intelligence', role='Competitive Research', color='#3b82f6',
         mockup='02_market_intelligence.png',
         clicks=[(0.3, 0.3), (0.65, 0.45), (0.4, 0.7), (0.8, 0.25)],
         logs=['$ fathom monitor --track 15 competitors',
               '[TOOL]  scrape_pricing stripe.com → +2.5%',
               '[TOOL]  diff_state vs stored snapshot',
               '[ALERT]  Adyen launched "Adaptive Pricing"',
               '[MEMORY]  stored 23 pricing deltas',
               '[VERIFY]  sources cross-checked — PASS',
               '[FINAL]  briefing: 3 shifts, 2 risks']),
    dict(name='Talent Scout', role='Technical Recruiting', color='#ec4899',
         mockup='03_talent_scout.png',
         clicks=[(0.3, 0.4), (0.7, 0.3), (0.5, 0.65), (0.8, 0.5)],
         logs=['$ fathom source --skill rust --senior',
               '[TOOL]  github_ast tokio-rs → 14 candidates',
               '[TOOL]  linkedin_crosscheck → 9 confirmed',
               '[TOOL]  smtp_verify rust.dev@… → 250 OK',
               '[MEMORY]  absorbed 30 engineer dossiers',
               '[VERIFY]  commit-level icebreakers — PASS',
               '[FINAL]  delivered 30 dossiers']),
    dict(name='Onboarding Agent', role='Client Setup & Triage', color='#10b981',
         mockup='13_customer_success_onboarding.png',
         clicks=[(0.25, 0.35), (0.5, 0.55), (0.75, 0.4), (0.4, 0.7)],
         logs=['$ fathom onboard --client DataStream',
               '[TOOL]  provision_api_key → sk_live_9f3…',
               '[TOOL]  configure_webhook https://api/…',
               '[TOOL]  send_test_payload → 200 OK',
               '[MEMORY]  stored client config (3 secrets)',
               '[VERIFY]  sandbox validated — PASS',
               '[FINAL]  onboarding complete in 12m']),
    dict(name='Finance Ops', role='Invoice Processing', color='#8b5cf6',
         mockup='04_backoffice_invoice.png',
         clicks=[(0.3, 0.3), (0.6, 0.5), (0.45, 0.7), (0.75, 0.35)],
         logs=['$ fathom ingest --invoices 500 --pdf',
               '[TOOL]  parse_invoice INV-4401 → $12,400',
               '[TOOL]  three_way_match PO#8821 → 2/3',
               '[TOOL]  qb_stage_payment → approved',
               '[MEMORY]  absorbed 500 invoice records',
               '[VERIFY]  accuracy 100% — PASS',
               '[FINAL]  412 staged, 3 flagged']),
    dict(name='Software Maintainer', role='Bug Triage & Fix', color='#ef4444',
         mockup='05_devops_engineer.png',
         clicks=[(0.25, 0.35), (0.6, 0.45), (0.4, 0.65), (0.75, 0.3)],
         logs=['$ fathom triage --sentry ZD-4471',
               '[TOOL]  map_repo --files 240',
               '[TOOL]  reproduce in sandbox → ZeroDivisionError',
               '[TOOL]  apply_fix src/engine.py:112',
               '[TOOL]  run_tests → 141/141 passed',
               '[VERIFY]  coverage +2.1% — PASS',
               '[FINAL]  PR #882 merged']),
    dict(name='Compliance Auditor', role='Legal Risk Assessment', color='#06b6d4',
         mockup='12_legal_compliance_auditor.png',
         clicks=[(0.3, 0.35), (0.65, 0.45), (0.5, 0.65), (0.8, 0.3)],
         logs=['$ fathom audit --msa 200 --gdpr',
               '[TOOL]  ingest_contract vendor_a.pdf',
               '[TOOL]  scan_clause data_liability → cap $50k',
               '[TOOL]  parallel_gdpr 5 analysts × 40 docs',
               '[MEMORY]  absorbed 200 MSA profiles',
               '[VERIFY]  risk matrix computed — PASS',
               '[FINAL]  Green 118 / Yellow 62 / Red 20']),
    dict(name='Orchestrator', role='Swarm Coordinator', color='#a78bfa',
         mockup='11_swarm_coordinator.png',
         clicks=[(0.3, 0.3), (0.55, 0.5), (0.7, 0.35), (0.4, 0.65)],
         logs=['$ fathom swarm --task "research market"',
               '[PLAN]  decompose → researcher ×3, analyst, verifier',
               '[SPAWN]  spawned 5 sub-agents',
               '[TOOL]  parallel_dispatch 5 workers',
               '[MEMORY]  merged 3 research streams',
               '[VERIFY]  quality gate — 94/100 PASS',
               '[FINAL]  synthesized deliverable']),
    dict(name='Inbox Manager', role='Email Intelligence', color='#f97316',
         mockup='07_inbox_manager.png',
         clicks=[(0.3, 0.35), (0.6, 0.3), (0.45, 0.6), (0.75, 0.45)],
         logs=['$ fathom inbox --watch 1000/day',
               '[TOOL]  classify 1,204 emails → 6 folders',
               '[TOOL]  priority_triage → 38 urgent',
               '[TOOL]  auto_reply 12 common queries',
               '[MEMORY]  learned sender intent (412 rules)',
               '[VERIFY]  spam precision 99.7% — PASS',
               '[FINAL]  inbox zero by 4pm']),
    dict(name='Lead Audit Report', role='Data Quality Assurance', color='#14b8a6',
         mockup='08_lead_audit_report.png',
         clicks=[(0.3, 0.3), (0.65, 0.45), (0.5, 0.65), (0.8, 0.4)],
         logs=['$ fathom audit --leads 10000',
               '[TOOL]  deduplicate → removed 1,204',
               '[TOOL]  validate_email → 8,421 valid',
               '[TOOL]  enrich_missing → +3,102 fields',
               '[MEMORY]  stored audit snapshot',
               '[VERIFY]  completeness 96.8% — PASS',
               '[FINAL]  report_lead-audit.pdf generated']),
    dict(name='Entity Knowledge Graph', role='Relationship Mapping', color='#6366f1',
         mockup='09_entity_knowledge_graph.png',
         clicks=[(0.3, 0.35), (0.6, 0.4), (0.5, 0.65), (0.8, 0.3)],
         logs=['$ fathom graph --docs 500',
               '[TOOL]  extract_entities → 12,440 nodes',
               '[TOOL]  link_relations contact↔company',
               '[TOOL]  fts5_index → query-ready',
               '[MEMORY]  absorbed knowledge graph',
               '[VERIFY]  node consistency 99.2% — PASS',
               '[FINAL]  graph.fathom exposed']),
    dict(name='Security Vault', role='Credential Governance', color='#dc2626',
         mockup='10_security_credentials_vault.png',
         clicks=[(0.3, 0.4), (0.6, 0.55), (0.45, 0.3), (0.75, 0.6)],
         logs=['$ fathom vault --rotate',
               '[TOOL]  aes_gcm_seal key ring → 32 keys',
               '[TOOL]  policy_check tool:api_call → allowed',
               '[TOOL]  audit_ledger append 1,204 events',
               '[MEMORY]  rotated 12 secrets',
               '[VERIFY]  fail-closed verified — PASS',
               '[FINAL]  vault seal integrity 100%']),
    dict(name='Outreach Dispatcher', role='Campaign Automation', color='#d946ef',
         mockup='14_outreach_campaign_dispatcher.png',
         clicks=[(0.3, 0.3), (0.6, 0.5), (0.45, 0.65), (0.75, 0.35)],
         logs=['$ fathom campaign --run Q3-launch',
               '[TOOL]  personalize 5,200 emails (merge tags)',
               '[TOOL]  send_batch email → 5,200 sent',
               '[TOOL]  schedule_followup day+3',
               '[MEMORY]  tracked reply signals',
               '[VERIFY]  open-rate 41% — PASS',
               '[FINAL]  A/B: variant B wins +18%']),
    dict(name='Agency Fleet Manager', role='Multi-Client Operations', color='#0ea5e9',
         mockup='15_agency_fleet_manager.png',
         clicks=[(0.3, 0.35), (0.65, 0.45), (0.5, 0.65), (0.8, 0.3)],
         logs=['$ fathom fleet --clients 15',
               '[SPAWN]  dedicated fleet per client',
               '[TOOL]  isolate_context client_acme → sandbox',
               '[TOOL]  cross_report 15 dashboards → 15 min',
               '[MEMORY]  summarized weekly SLOs',
               '[VERIFY]  SLA 99.9% — PASS',
               '[FINAL]  resource pool rebalanced']),
    dict(name='Computer Use', role='Desktop Automation', color='#84cc16',
         mockup='06_computer_2fa_takeover.png',
         clicks=[(0.25, 0.4), (0.5, 0.5), (0.7, 0.35), (0.45, 0.7)],
         logs=['$ fathom computer --task "submit PO"',
               '[TOOL]  cdp_navigate supplier-portal.com',
               '[TOOL]  click #login → type credentials',
               '[TOOL]  2FA → human takeover (you approve)',
               '[TOOL]  fill_form PO-8821 → submit',
               '[MEMORY]  stored form strategy',
               '[VERIFY]  confirmation #2834 — PASS',
               '[FINAL]  PO submitted, receipt archived']),
]

def hex_to_rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i+2], 16) for i in (0, 2, 4))

def load_fonts():
    return {
        'title': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 44),
        'role': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 26),
        'log': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSansMono.ttf'), 19),
        'large': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 96),
        'subtitle': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 36),
        'small': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans.ttf'), 20),
        'badge': ImageFont.truetype(os.path.join(FONT_DIR, 'DejaVuSans-Bold.ttf'), 18),
    }

def lerp(a, b, t): return a + (b - a) * t

def draw_cursor_pil(img, x, y):
    """Paste cursor texture."""
    img.paste(CURSOR, (int(x), int(y)), CURSOR)

def draw_click_ripple(img, cx, cy, t, color):
    """Expanding fading ring at (cx,cy); t in [0,1]."""
    r = int(6 + 42 * t)
    alpha = int(200 * (1 - t))
    if alpha <= 0 or r <= 0: return
    ring = Image.new('RGBA', (r*2, r*2), (0, 0, 0, 0))
    rd = ImageDraw.Draw(ring)
    rd.ellipse([0, 0, r*2, r*2], outline=color + (alpha,), width=3)
    img.paste(ring, (int(cx - r), int(cy - r)), ring)

def draw_intro(img, draw, fonts, t):
    phase = t / INTRO_DUR
    for y in range(H):
        bl = y / H
        draw.line([(0, y), (W, y)], fill=(int(250+5*(1-bl)), int(250+5*(1-bl)), 253))
    zoom = 0.9 + 0.1 * min(1, phase * 2)
    title = 'FATHOM'
    tw = fonts['large'].getlength(title)
    ty = 160 - 20 * (1 - ease_out_cubic(min(1, phase * 1.5)))
    draw.text(((W - tw) // 2, ty), title, fill=BLACK, font=fonts['large'])
    sub = 'Autonomous AI Workforce Runtime'
    sw = fonts['subtitle'].getlength(sub)
    sa = min(1, (phase - 0.08) * 6)
    if sa > 0:
        draw.text(((W - sw) // 2, 270), sub, fill=tuple(int(30+(c-30)*sa) for c in GRAY), font=fonts['subtitle'])
    tag = 'Live clicks. Live tools. Live memory.'
    taw = fonts['role'].getlength(tag)
    ta = min(1, (phase - 0.2) * 5)
    if ta > 0:
        draw.text(((W - taw) // 2, 322), tag, fill=tuple(int(100+(c-100)*ta) for c in GRAY), font=fonts['role'])
    if phase > 0.35:
        ba = min(1, (phase - 0.35) * 6)
        cell_w, cols = 220, 5
        gx = (W - cols * cell_w) // 2
        for i, sc in enumerate(SCENARIOS):
            bx = gx + (i % cols) * cell_w
            by = 392 + (i // cols) * 50
            rgb = hex_to_rgb(sc['color'])
            bg = tuple(int(c + (255 - c) * (1 - ba)) for c in rgb)
            draw.rounded_rectangle([bx, by, bx + cell_w - 6, by + 44], radius=6, fill=bg)
            draw.text((bx + 10, by + 12), sc['name'][:18], fill=BLACK, font=fonts['badge'])
    draw.text((W - 200, H - 50), 'fathom.uz', fill=LIGHT_GRAY, font=fonts['small'])
    _bar(draw, 0, phase)

def draw_scenario(img, draw, fonts, t, si):
    sc = SCENARIOS[si]
    color = hex_to_rgb(sc['color'])
    dur = SCENARIO_DUR
    phase = t / dur

    draw.rectangle([(0, 0), (W, H)], fill=WHITE)
    # pulsing top bar
    draw.rectangle([(0, 0), (W, max(2, int(3 + 1.5 * math.sin(t * 2.5))))], fill=color)

    # Title
    ta = ease_out_cubic(clamp01(phase * 12))
    if ta > 0:
        draw.text((48, 14), sc['name'], fill=BLACK, font=fonts['title'])
        ra = clamp01((phase - 0.05) * 10)
        if ra > 0:
            draw.text((48, 62), sc['role'], fill=tuple(int(180+75*ra) for _ in range(3)), font=fonts['role'])

    # ── Screen (mockup w/ Ken Burns zoom + cursor) ──
    mockup = mockups.get(sc['mockup'])
    if mockup:
        mw, mh = mockup.size  # ~960x480
        # Slide-in
        sp = ease_out_cubic(clamp01((phase - 0.06) * 3))
        scy = 108 + int(46 * (1 - sp))
        # Ken Burns: zoom 1.0 → 1.14, pan slight
        kb = clamp01((phase - 0.15) / 0.75)
        zoom = 1.0 + 0.16 * ease_in_out(kb)
        pan_x = int(20 * math.sin(t * 0.12))
        pan_y = int(12 * math.sin(t * 0.09 + 1.0))
        zc = int(mw * zoom), int(mh * zoom)
        cx0 = max(0, min(zc[0] - mw, pan_x))
        cy0 = max(0, min(zc[1] - mh, pan_y))
        try:
            view = mockup.crop((cx0, cy0, cx0 + mw, cy0 + mh))
        except ValueError:
            view = mockup
        sx = (W - mw) // 2
        sy = scy

        # Frame shadow + border
        draw.rounded_rectangle([sx - 3, sy - 3, sx + mw + 8, sy + mh + 8], radius=12, fill=(0, 0, 0, 24))
        draw.rounded_rectangle([sx - 2, sy - 2, sx + mw + 2, sy + mh + 2], radius=10, fill=LIGHT_GRAY, outline=color)
        img.paste(view, (sx, sy), view if view.mode == 'RGBA' else None)

    # ── Cursor + clicks ──
    nclicks = len(sc['clicks'])
    # Cursor visits each click point, dwells, then moves on
    visit_dur = 0.24  # fraction of scenario per visit
    for ci in range(nclicks):
        vs = ci * visit_dur + 0.10
        ve = vs + visit_dur
        if vs <= phase < ve:
            p = (phase - vs) / visit_dur
            pt0 = sc['clicks'][ci]
            pt1 = sc['clicks'][(ci + 1) % nclicks]
            # move 0-0.45, click 0.5-0.75, hold
            if p < 0.45:
                mp = ease_out_cubic(p / 0.45)
                cxp = lerp(pt0[0], pt1[0], mp)
                cyp = lerp(pt0[1], pt1[1], mp)
                cx = sx + int(cxp * mw)
                cy = sy + int(cyp * mh)
                draw_cursor_pil(img, cx, cy)
            elif p < 0.55:
                cx = sx + int(pt1[0] * mw)
                cy = sy + int(pt1[1] * mh)
                if mw and mh:
                    draw_click_ripple(img, cx, cy, (p - 0.5) / 0.05, color)
                draw_cursor_pil(img, cx, cy)
            else:
                cx = sx + int(pt1[0] * mw)
                cy = sy + int(pt1[1] * mh)
                draw_cursor_pil(img, cx, cy)
            break

    # ── Terminal ──
    term_y, term_h = 576, 128
    draw.rounded_rectangle([48, term_y, W - 48, term_y + term_h], radius=10, fill=DARK)
    # header
    draw.rectangle([48, term_y, W - 48, term_y + 26], fill=(30, 30, 52))
    draw.ellipse([60, term_y + 8, 72, term_y + 20], fill='#ef4444')
    draw.ellipse([78, term_y + 8, 90, term_y + 20], fill='#eab308')
    draw.ellipse([96, term_y + 8, 108, term_y + 20], fill='#22c55e')
    draw.text((120, term_y + 5), 'worker@fathom: ~/agent', fill=(150, 150, 180), font=fonts['log'])

    # logs appear one by one with type effect
    nlogs = len(sc['logs'])
    line_h = 24
    max_lines = 4
    for i in range(nlogs):
        entry_t = 0.28 + i * 0.09  # fraction of scenario when line i starts
        if phase < entry_t: break
        typing = (phase - entry_t) / 0.05
        full = sc['logs'][i]
        shown = full[: int(len(full) * clamp01(typing))]
        # scroll window: last max_lines visible
        vis_idx = i
        top_idx = max(0, i - max_lines + 1)
        line_y = term_y + 30 + (i - top_idx) * line_h
        if line_y + line_h > term_y + term_h - 6: break
        x0 = 60
        # color by prefix
        if shown.startswith('$'): col = (210, 210, 230)
        elif '[PLAN]' in shown: col = (167, 139, 250)
        elif '[SPAWN]' in shown: col = (196, 181, 253)
        elif '[TOOL]' in shown: col = (147, 197, 253)
        elif '[MEMORY]' in shown: col = (110, 231, 183)
        elif '[VERIFY]' in shown: col = (251, 191, 36)
        elif '[FINAL]' in shown: col = (74, 222, 128)
        elif '[ALERT]' in shown: col = (249, 115, 22)
        else: col = (200, 200, 220)
        draw.text((x0, line_y), shown, fill=col, font=fonts['log'])

    _bar(draw, si + 1, phase)

def draw_outro(img, draw, fonts, t):
    phase = t / OUTRO_DUR
    for y in range(H):
        bl = y / H
        draw.line([(0, y), (W, y)], fill=(int(245+10*(1-bl)), int(245+10*(1-bl)), 250))
    if phase < 0.5:
        f = min(1, phase * 6)
        text = 'Try the live demo'
        tw = fonts['large'].getlength(text)
        draw.text(((W - tw) // 2, 180), text, fill=BLACK, font=fonts['large'])
        url = 'fathom.uz/demo'
        uw = fonts['title'].getlength(url)
        draw.text(((W - uw) // 2, 284), url, fill=ACCENT, font=fonts['title'])
        tag = 'Your keys. Your model. Your workforce.'
        tgw = fonts['role'].getlength(tag)
        draw.text(((W - tgw) // 2, 348), tag, fill=GRAY, font=fonts['role'])
    if phase > 0.3:
        ba = min(1, (phase - 0.3) * 5)
        bw = 150
        bx = (W - 15 * bw) // 2
        for i, sc in enumerate(SCENARIOS):
            rgb = hex_to_rgb(sc['color'])
            bc = tuple(int(c + (255 - c) * (1 - ba)) for c in rgb)
            draw.rounded_rectangle([bx + i * bw, 430, bx + i * bw + bw - 5, 490], radius=6, fill=bc)
    draw.text((W - 200, H - 50), 'fathom.uz', fill=LIGHT_GRAY, font=fonts['small'])
    _bar(draw, 16, phase)

def _bar(draw, current, phase):
    bar_y, bar_h = H - 18, 4
    bx, bw = 60, W - 120
    fill_w = int(bw * (current + phase) / 17) if current < 16 else bw
    draw.rounded_rectangle([bx, bar_y, bx + bw, bar_y + bar_h], radius=2, fill=LIGHT_GRAY)
    draw.rounded_rectangle([bx, bar_y, bx + fill_w, bar_y + bar_h], radius=2, fill=ACCENT2)

mockups = {}
def main():
    global mockups
    os.makedirs(os.path.dirname(OUTPUT), exist_ok=True)
    fonts = load_fonts()
    for sc in SCENARIOS:
        mp = os.path.join(MOCKUP_DIR, sc['mockup'])
        if os.path.exists(mp):
            im = Image.open(mp).convert('RGBA')
            mw, mh = im.size
            tw = 900
            th = min(470, int(mh * tw / mw))
            mockups[sc['mockup']] = im.resize((tw, th), Image.LANCZOS)
            print(f"  resized {sc['mockup']} {tw}x{th}")
        else:
            print(f"  WARNING {sc['mockup']} missing")

    t0 = time.time()
    cmd = ['ffmpeg', '-y', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-s', f'{W}x{H}',
           '-r', str(FPS), '-i', '-', '-c:v', 'libx264', '-pix_fmt', 'yuv420p',
           '-crf', '20', '-preset', 'medium', '-movflags', '+faststart', OUTPUT]
    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE)

    for fi in range(TOTAL_FRAMES):
        t = fi / FPS
        img = Image.new('RGB', (W, H), WHITE)
        draw = ImageDraw.Draw(img)
        if t < INTRO_DUR:
            draw_intro(img, draw, fonts, t)
        elif t < INTRO_DUR + len(SCENARIOS) * SCENARIO_DUR:
            st = t - INTRO_DUR
            si = min(int(st // SCENARIO_DUR), len(SCENARIOS) - 1)
            draw_scenario(img, draw, fonts, st - si * SCENARIO_DUR, si)
        else:
            draw_outro(img, draw, fonts, t - INTRO_DUR - len(SCENARIOS) * SCENARIO_DUR)
        proc.stdin.write(img.tobytes())
        if fi % 3600 == 0:
            el = time.time() - t0
            print(f"  {fi}/{TOTAL_FRAMES} ({100*fi//TOTAL_FRAMES}%) {el:.0f}s", flush=True)
    proc.stdin.close()
    proc.wait()
    sz = os.path.getsize(OUTPUT)
    print(f"Done! {time.time()-t0:.0f}s, {sz/1024/1024:.1f} MB")

if __name__ == '__main__':
    main()