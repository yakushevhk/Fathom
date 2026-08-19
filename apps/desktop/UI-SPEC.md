# UI-спецификация Comet (Zeron) — для воспроизведения на React/Tauri

## 1. Общий Layout (Shell)

```
┌─────────────────────────────────────────────────────────────┐
│ [titlebar overlay — 32px, drag region, window controls]     │
├────────┬────────────────────────────────┬───────────────────┤
│        │                                │                   │
│ SIDEBAR│    CONVERSATION COLUMN         │   RIGHT PANE      │
│ (208-  │    (fills remaining)           │   (360-760px,     │
│ 400px) │                                │   hidden by       │
│        │                                │   default)        │
│        │   [Message Rail — minimap,     │                   │
│        │    768px gate, 12 ticks max]   │   [Changes/Diff]  │
│        │                                │   [Terminal]      │
│        │   [Composer — bottom dock]     │                   │
├────────┴────────────────────────────────┴───────────────────┤
│ [status strip — 24px, engine status, device indicator]      │
└─────────────────────────────────────────────────────────────┘
```

- **Titlebar** — overlay поверх контента (32px), drag region, window controls (traffic lights)
- **Sidebar** — слева, анимированная ширина 208–400px (spring transition)
- **Conversation Column** — стекло (frosted glass), основной контент
- **Right Pane** — 360–760px, скрыта по умолчанию, показывается для diff/changes/terminal
- **Terminal Dock** — выезжает снизу, 160px–55% vh, drag-reorderable tabs
- **Status Strip** — 24px снизу, engine status, device indicator

## 2. Дизайн-токены

### Цвета (oklch-derived, монохромная тема)
```css
/* Тёмная тема (always-dark) */
--bg-surface: #060606;        /* фон sidebar */
--bg-window: #0d0d0d;         /* фон conversation */
--bg-elevated: #141414;       /* hover, cards */
--bg-glass: rgba(10,10,10,0.85); /* frosted glass */
--fg-primary: #e8e8e8;        /* основной текст */
--fg-secondary: #888;         /* второстепенный */
--fg-tertiary: #555;          /* мета-информация */
--border: rgba(255,255,255,0.06); /* hairline borders */
--border-hover: rgba(255,255,255,0.12);
--accent: #888;               /* акцентный (серый) */
--accent-blue: #4a9eff;      /* ссылки, code */
--danger: #e5484d;            /* ошибки, отмена */
--success: #30a46c;           /* успех */
```

### Шрифты
- `Geist` — основной текст (normal, medium, semibold)
- `Geist Mono` — код, моноширинный
- Размеры: 11px (мета, статус), 13px (тело), 14px (заголовки), 24px (hero)

### Скругления
- `--radius-sm: 6px` (кнопки, chips)
- `--radius-md: 10px` (панели, composer)
- `--radius-lg: 16px` (модальные окна, pickers)

## 3. Транскрипт (Conversation)

### Виртуализация
- **Block-granularity rows**: один row = один markdown block / tool group
- Stable ids: `msgId#blockId` (не перерисовывается при стриминге)
- Row height memoization: `(rowId, contentLength, width)` — при стриминге пересчёт только одной строки
- Scroll-anchor absorption: при изменении высоты над viewport'ом — anchor не сдвигается

### Stick-to-bottom (Spring)
- Velocity spring: damping 0.7, stiffness 0.05, mass 1.25
- Feed-forward tracking: при стриминге новый контент плавно догоняет
- Interrupt: при wheel-up/drag — открепляется
- Re-engage: в пределах 70px от низа — автоматически прикрепляется
- Own-send: re-engage + smooth scroll

### Markdown Streaming
- `pulldown-cmark` парсинг в фоне
- Block-level incremental re-parse: только от последнего stable block boundary
- Fade-in veil: новый текст появляется с opacity анимацией (paint-layer, не влияет на layout)
- `prefers-reduced-motion` — отключает анимации

### Tool Chips / Tool Calls
- Свёрнутый вид: иконка + название + статус (spinner/check)
- Развёрнутый: аргументы, результат с preview
- Expandable: клик раскрывает detail

## 4. Композер

### Состояния
- **Compact**: 49px высота, одна строка, placeholder "Research anything..."
- **Expanded**: 124px высота, multi-line textarea
- **Full**: 308px, при длинном тексте или attachments

### Auto-grow
- 76–260px по ширине, Enter/Shift+Enter отправка
- Auto-flip compact↔expanded по измерению ширины текста

### Send/Steer/Stop Morph
- Простой режим: кнопка Send → отправляет запрос
- Во время выполнения: кнопка Steer (mid-run instruction) → morph в Stop
- Stop: красная кнопка, отменяет сессию

### Question Panel
- Заменяет composer когда агент задаёт вопрос
- Paged: 1-9 keys, 220ms auto-advance
- Отображает варианты ответов

### Attachments
- Drag-drop/paste изображений
- File mentions: `@filename` auto-complete
- Drafts per chat + attachments persist

## 5. Sidebar (Session List)

### Row
- Высота: ~61px per session
- Layout: [status dot] [title + meta] [timestamp]
- Status: spinner (running), check (done), X (failed)
- Active session: подсветка

### Session States
- Active (running) — spinner + pulse
- Completed — check mark
- Failed — X mark
- Archived — muted, скрыт из основного списка

### Action
- Клик: открывает/переключает сессию в conversation
- Контекстное меню: archive, rename, delete
- Архив: explicit sidebar action, не close tab

### Search/Filter
- Search input сверху
- Фильтр по пространствам (spaces) — dropdown
- "All spaces" included

## 6. Настройки (Settings)

Разделы (7):
1. **Accounts** — управление API ключами, agent accounts
2. **Appearance** — theme switch (dark/light), scale
3. **Archived** — список архивированных сессий
4. **Composer** — поведение композера
5. **Devices** — connected devices, relay status
6. **Harnesses** — какие агенты подключены (Claude Code, Codex, etc.)
7. **Notifications** — звуки, уведомления

Persisted as `ui-settings.json`.

## 7. Motion & Анимации

Каталог (12 категорий):

| Анимация | Duration | Easing | Description |
|----------|----------|--------|-------------|
| `fade-in` | 0.5s | cubic-bezier(0.16,1,0.3,1) | translateY 4→0, новый контент |
| `splash-out` | 0.3s | ease-out | исчезновение |
| `zeron-pulse` | 2.4s | stagger | волна для загрузчиков |
| `gradient-spin` | 750ms | linear | matrix spinner |
| `menu-in` | 200ms | scale+fade | pickers, popovers |
| `dialog-in` | 300ms | scale+fade | модальные окна |
| `sidebar-width` | 260ms | cubic-bezier(0.22,1,0.36,1) | ширина sidebar |
| `resort-glide` | 260ms | cubic-bezier(0.22,1,0.36,1) | FLIP анимация перестановки |
| `hover-fade` | 150ms | ease | кнопки, ссылки |
| `scroll-spring` | — | damping 0.7/stiffness 0.05 | stick-to-bottom |
| `width-transition` | 200ms | ease-out | right pane |
| `height-tween` | 180ms | ease | collapse/expand файлов в diff |

## 8. Дополнительные UI-компоненты

### Message Rail (minimap)
- Слева от conversation (граница с sidebar)
- Показывается при ширине > 768px
- Максимум 12 ticks
- Показывает позицию/структуру сообщений

### Frosted Glass
- 44px blur radius
- Используется для: conversation column, titlebar, composer (в expanded режиме)
- `backdrop-filter: blur(44px)` с тёмным фоном

### Scroll-to-bottom Pill
- Появляется при скролле вверх
- Показывает количество новых сообщений
- Клик: smooth scroll вниз

### Edge Fade
- Градиентные затухания на краях scrollable контейнеров
- Верх/низ/бока — 12px градиент

## 9. Что gpui даёт из коробки vs что надо делать на React

### gpui built-in (надо реализовать на React):
- Virtualized list с `ListState` (React: `react-window` / `virtuoso`)
- Scroll physics + spring (React: `framer-motion` spring)
- `backdrop-filter: blur()` (React: CSS, но GPU-bound)
- Text shaping (React: native browser rendering)
- Native window controls (Tauri: window decorations)

### React-specific advantages:
- CSS animations (GPU-ускоренные, без Rust)
- SVG icons inline
- `react-window` / `virtuoso` для виртуализации
- `framer-motion` для анимаций
- WebSocket/SSE проще, чем gpui RPC

## 10. API эндпоинты, которые UI вызывает

(через Tauri IPC / proxy к `fathom serve`)

```
GET    /api/v1/sessions
POST   /api/v1/sessions         { query }
GET    /api/v1/sessions/:id
DELETE /api/v1/sessions/:id
POST   /api/v1/sessions/:id/steer
POST   /api/v1/sessions/:id/answer
POST   /api/v1/sessions/:id/approve
GET    /api/v1/sessions/:id/results
GET    /api/v1/sessions/:id/events  (SSE)
GET    /api/v1/events               (SSE — global)
GET    /api/v1/agents
GET    /api/v1/agents/:id
POST   /api/v1/jobs
GET    /api/v1/jobs
GET    /api/v1/jobs/:id
DELETE /api/v1/jobs/:id
GET    /api/v1/jobs/:id/log
POST   /api/v1/jobs/:id/rerun
GET    /api/v1/memories
GET    /api/v1/memories/:id
POST   /api/v1/memories/absorb
GET    /api/v1/memories/stats
GET    /health
GET    /metrics
```