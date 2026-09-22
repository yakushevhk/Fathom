---
name: testing-fathom-website
description: How to run and browser-test the Fathom Astro marketing site static build (EN + RU trees) locally.
---

# Testing the Fathom marketing website build

## Serve the static build

- Rebuild: `cd website && export NVM_DIR="$HOME/.nvm" && . "$NVM_DIR/nvm.sh" && npm run build` — outputs to `website/dist/client/` (EN) with generated `website/dist/client/ru/` tree.
- Serve: `cd website/dist/client && python3 -m http.server 8080`. Base URL `http://localhost:8080/`.
- This is a **static** server — the serverless endpoint `POST /api/lead` returns 501/404; the lead form is expected to show its graceful error message ("Не удалось отправить форму…" on RU). That is correct behavior in this environment.

## Browser tips (Chrome for Testing on this box)

- URL bar autocompletes history aggressively: after typing a short URL like `localhost:8080/`, press **Delete** (removes the autocompleted suffix) before Enter, or you'll land on a previously-visited subpath.
- The globe language switcher (`[data-lang-toggle]` in top nav) opens a dropdown; options navigate via `location.assign()` — `/` ↔ `/ru/`.
- Footer/nav verification: the footer grid is a `[data-reveal]` element — scroll to bottom (End key) to trigger IntersectionObserver.
- **To bypass the `.reveal`/`[data-reveal]` hidden state** (e.g. to inspect hidden content or after a specificity regression): run `document.documentElement.classList.remove('js')` in console — all reveal content becomes visible without reload.
- Real no-JS test: DevTools (Ctrl+Shift+I) → Ctrl+Shift+P → "Disable JavaScript" → F5. `<html>` then lacks `class="js"`.
- Deck pages: `/deck/` (EN) and `/ru/deck/` are a slide viewer — sidebar list + iframe (`#slideFrame`). Single slides are `/deck/page_NN.html`. Note: shared `deck/styles.css` has `@media screen { body{flex-direction:column;align-items:center} }` which pushes the stage iframe below the fold on index pages (pre-existing quirk, both locales).

## Layout / responsive audits

- `body { overflow-x: hidden }` (global.css) propagates to the viewport — horizontal overflow is **clipped, never scrolled**. Never judge by scrollbar presence; measure `document.documentElement.scrollWidth` vs `clientWidth`, and scan element `getBoundingClientRect()` for `right > clientWidth` (exclude elements inside a real scroll container: ancestor with `overflow-x:auto|scroll` that itself fits the viewport, and intentional offscreen a11y like `.skip-link`/`.sr-only`).
- Exact viewport emulation: the box Chrome exposes CDP at `http://localhost:29229` (browser ws from `/json/version`). Node ≥21 drives it with built-in `fetch`+`WebSocket`, no deps: `Target.createTarget` → `Target.attachToTarget{flatten:true}` → `Emulation.setDeviceMetricsOverride{w,h,deviceScaleFactor,mobile}` → `Page.navigate` → `Runtime.evaluate` (audit JS) → `Page.captureScreenshot{captureBeyondViewport:true}` for full-page PNGs. Emulation is **session-scoped** — it clears when your ws session ends, so keep a holder process alive or re-apply per attach.
- On `mobile:true` emulation a page that overflows horizontally expands the layout viewport past device width (`innerWidth` ≈ scrollWidth) — that IS the real-phone zoom-out symptom, so 360 audit numbers may under-report once content gets very wide.
- Chrome can't be resized below ~500px width on this box — real mobile-width windows are impossible; use CDP emulation or a separate `--app=` Chrome (`--user-data-dir=/tmp/...`) for recording.
- `html{scroll-behavior:smooth}` makes `scrollTo(0,0)` animate — screenshots right after a scroll can land mid-scroll; wait ~1s or set scroll-behavior auto first.
- Known fragile markup on docs MDX pages (watch for these patterns): `.code-card` wrappers around `pre` (styled only on some .astro pages — docs pages get no `pre{overflow-x:auto}` → code lines spill); bare text+`<code>` children inside `.callout` (it is `display:flex` → anonymous flex items render as narrow columns); `.mini-table`/`.flags-table`/plain `<table>` not wrapped in `.table-wrap`/`.mini-table-wrap` → no scroll container → page overflow on mobile.

## Devin Secrets Needed

- None for the static site. Telegram/webhook env vars (`LEAD_TELEGRAM_BOT_TOKEN`, `LEAD_TELEGRAM_CHAT_ID`, `LEAD_WEBHOOK_URL`) only matter for the deployed serverless endpoint.
