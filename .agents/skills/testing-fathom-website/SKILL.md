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

## Devin Secrets Needed

- None for the static site. Telegram/webhook env vars (`LEAD_TELEGRAM_BOT_TOKEN`, `LEAD_TELEGRAM_CHAT_ID`, `LEAD_WEBHOOK_URL`) only matter for the deployed serverless endpoint.
