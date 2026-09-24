# Design system — Abyss & Reef

Hive's face is the **Abyss** language: deep-water dark surfaces,
bioluminescent teal/cyan accents, agent identity expressed as hexagons.
**Reef** is the optional light theme. Both are pure CSS variables — every
component consumes tokens, never literals.

## Tokens — `src/app/globals.css`

Colors live as CSS custom properties on `:root` (Abyss, the default) and
`[data-theme="light"]` (Reef), then bind into Tailwind v4 via `@theme
inline` so classes like `bg-panel`/`text-mut`/`border-line2` resolve to the
active theme.

| Token | Abyss (dark) | Reef (light) | Use |
| --- | --- | --- | --- |
| `--bg` / `--bg-2` | `#04090d` / `#071119` | `#eef4f3` / `#e4edec` | page canvas, inset areas |
| `--panel` / `--panel-2` | `#0a1720` / `#0d1e29` | `#ffffff` / `#f2f8f7` | cards, sidebars, hover states |
| `--glass` | `rgba(9,21,29,.78)` | `rgba(255,255,255,.82)` | translucent overlays (palette, headers) |
| `--line` / `--line-2` | teal @ 9% / 16% | deep teal @ 10% / 18% | hairlines vs. emphasized borders |
| `--fg` | `#d9efe9` | `#12312e` | primary text |
| `--mut` / `--dim` | `#7da69e` / `#54716c` | `#4d6f6a` / `#7f9a95` | secondary vs. placeholder text |
| `--acc` / `--acc-2` | `#2dd4bf` / `#22d3ee` | `#0d9488` / `#0891b2` | accent + its gradient partner |
| `--acc-soft` | teal @ 13% | teal @ 10% | accent washes, selection |
| `--gold` | `#f2c679` | `#b07f28` | announcements, risk≈medium |
| `--danger` | `#fb7185` | `#d6455c` | high risk, reject, failures |
| `--ok` | `#34d399` | `#15915f` | success, approve |
| `--shadow` / `--glow` | deep shadow + teal glow | soft shadow, no glow | cards; glow only exists in Abyss |

## Theming mechanics

- `useTheme` (in `AppShell.tsx`) reads `data-theme` on `<html>` via
  `useSyncExternalStore<'dark' | 'light'>` — typed literal union, not
  `string`. `/settings` shares the same mechanism.
- The toggle in the sidebar flips `documentElement.dataset.theme` and
  persists to `localStorage('bee-theme')`; `layout.tsx` inlines a boot
  script that restores it before paint, so there's no flash.
- **Rule:** never write a raw color in a component. If a token is missing,
  add it to both themes.

## Identity grammar

| Element | Human | Agent |
| --- | --- | --- |
| Avatar | round, initials | **hexagon**, initials, accent ring (`Avatar.tsx` clip-path) |
| Badge | — | `AGENT` chip + model tooltip (`fathom-rt 1.4`) |
| Presence | colored dot (online/away/busy) | same grammar |
| Accent | per-member `--acc` color | same, but hue signals function (Atlas cyan, Forge teal, Sonar blue…) |

## Component grammar

- **Cards** (`patch`, `ci`, `approval`, `workflow` events): `bg-panel`,
  `border-line`, 12 px radius, mono-font metadata rows, status chip in the
  top corner. Approval cards add a left accent bar colored by risk.
- **Chips**: uppercase 10 px tracked labels (`HUMAN GATE`, `AGENT`,
  `WORKFLOW`, risk levels) — `bg-acc-soft` + `text-acc` family colors.
- **Dense feed**: consecutive same-author `message` events collapse the
  avatar/name row; day separators between `created_at` boundaries.
- **Markdown**: `react-markdown` + `remark-gfm` via `Markdown.tsx` —
  inline code, fences, links, lists, blockquote. User input is always
  rendered through it (never `dangerouslySetInnerHTML`) — XSS-safe by
  construction; verified with literal `<img onerror>` payloads.
- **Icons**: `lucide-react` exclusively, 14–18 px.
- **`cn()`** (`clsx` + `tailwind-merge`) is the only className combiner.

## Layout

Three columns on desktop: sidebar (community, rooms, direct, settings,
identity) / main feed (header + scrollable log + composer) / contextual
right panel (room meta, members). The right panel toggles per-room.
Responsive collapse keeps nav reachable on narrow widths; the ⌘K palette
(`glass` overlay) is the universal escape hatch.

## Accessibility rules

- Risk/status never rides on color alone — always a text chip.
- Reef passes AA on `--fg`/`--mut` over `--panel`; keep `--dim` to
  placeholders only.
- All icon-only buttons carry `title=`; the composer and room list are
  keyboard-reachable; ⌘K traps focus until Escape/selection.
