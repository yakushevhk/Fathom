// Localized metadata (titles & descriptions) for canonical routes.
// Keys map to URL paths; nested routes use a literal slash key
// (e.g. 'docs/recipes' -> /docs/recipes).
export default {
  meta: {
    agents: {
      title: { en: 'Worker Agents — Fathom', ru: 'Агенты-воркеры — Fathom' },
      desc: {
        en: 'Specialized autonomous workers: browser, coder, coordinator, outreach, research, memory, verification, and ops agents in one governed runtime.',
        ru: 'Специализированные автономные воркеры: браузер, код, координация, аутрич, ресёрч, память, верификация и операции в одном управляемом рантайме.',
      },
    },
    'agents/browser': {
      title: { en: 'Browser Specialist — Fathom AI Workers', ru: 'Браузерный специалист — ИИ-воркеры Fathom' },
      desc: {
        en: 'Governed computer use: ARIA accessibility tree navigation, live WebSocket screen streaming, and 2FA human takeover.',
        ru: 'Управляемое использование компьютера: навигация по дереву доступности ARIA, стриминг экрана по WebSocket и перехват управления человеком при 2FA.',
      },
    },
    'agents/cleaning': {
      title: { en: 'Verification Worker — Fathom', ru: 'Воркер верификации — Fathom' },
      desc: {
        en: 'Checks email, phone, and profile data before it reaches a governed action — five gates, honest signals, reviewable evidence.',
        ru: 'Проверяет email, телефоны и профили до управляемого действия — пять уровней проверки, честные сигналы, проверяемые доказательства.',
      },
    },
    'agents/coder': {
      title: { en: 'Coder Specialist — Fathom AI Workers', ru: 'Воркер-программист — ИИ-воркеры Fathom' },
      desc: {
        en: 'Autonomous software engineer: AST symbol extraction, sandboxed execution, precision code edits, and verified Git commits.',
        ru: 'Автономный инженер-программист: извлечение AST-символов, исполнение в песочнице, точечные правки кода и верифицированные git-коммиты.',
      },
    },
    'agents/coordinator': {
      title: { en: 'Coordinator Specialist — Fathom AI Workers', ru: 'Координатор — ИИ-воркеры Fathom' },
      desc: {
        en: 'The central orchestrator: decomposes goals, spawns parallel Tokio worker swarms, reflects on quality, and synthesizes deliverables.',
        ru: 'Центральный оркестратор: декомпозиция целей, параллельные рои воркеров на Tokio, рефлексия по качеству и сборка финальных результатов.',
      },
    },
    'agents/extracting': {
      title: { en: 'Extraction Worker — Fathom', ru: 'Воркер извлечения данных — Fathom' },
      desc: {
        en: 'Turns pages, documents, and browser output into structured data with confidence scores and provenance.',
        ru: 'Превращает страницы, документы и вывод браузера в структурированные данные с оценками уверенности и происхождением.',
      },
    },
    'agents/memoring': {
      title: { en: 'Memory Worker — Fathom', ru: 'Воркер памяти — Fathom' },
      desc: {
        en: 'Persistent memory for the whole worker fleet: append-only knowledge base, hybrid search, entity graph, and pre-task digests.',
        ru: 'Постоянная память для всего флота воркеров: append-only база знаний, гибридный поиск, граф сущностей и дайджесты перед задачами.',
      },
    },
    'agents/operator': {
      title: { en: 'Operator Specialist — Fathom AI Workers', ru: 'Оператор — ИИ-воркеры Fathom' },
      desc: {
        en: 'Autonomous systems operator: durable background jobs, cron scheduling, daemon management, and multi-channel alert dispatch.',
        ru: 'Автономный оператор систем: устойчивые фоновые задачи, cron-расписания, управление демонами и отправка алертов по каналам.',
      },
    },
    'agents/outreach': {
      title: { en: 'Outreach & Operations Worker — Fathom', ru: 'Воркер аутрича и операций — Fathom' },
      desc: {
        en: 'Turns verified context and memory into governed action: personalized drafts, approved updates, exports, scheduled runs, and notifications.',
        ru: 'Превращает проверенный контекст и память в управляемые действия: персонализированные черновики, подтверждённые обновления, экспорты, запуски по расписанию и уведомления.',
      },
    },
    'agents/researcher': {
      title: { en: 'Research & Analysis Worker — Fathom', ru: 'Воркер ресёрча и анализа — Fathom' },
      desc: {
        en: 'A reusable worker for source-aware analysis, code inspection, and durable handoffs across autonomous workflows.',
        ru: 'Универсальный воркер для анализа с учётом источников, инспекции кода и надёжной передачи результатов в автономных процессах.',
      },
    },
    'agents/reviewer': {
      title: { en: 'Reviewer Specialist — Fathom AI Workers', ru: 'Ревьюер — ИИ-воркеры Fathom' },
      desc: {
        en: 'Enterprise security and governance reviewer: approval gates, hardware secret scanning, injection defense, and verification receipts.',
        ru: 'Корпоративный ревьюер безопасности и управления: шлюзы подтверждения, сканирование секретов, защита от инъекций и квитанции верификации.',
      },
    },
    'agents/searching': {
      title: { en: 'Search Worker — Fathom', ru: 'Поисковый воркер — Fathom' },
      desc: {
        en: 'Fused multi-backend discovery: hybrid and smart modes, RRF ranking, social and directory surfaces, and guarded fetching.',
        ru: 'Объединённый поиск по нескольким бэкендам: гибридный и умный режимы, ранжирование RRF, соцсети и каталоги, защищённая загрузка.',
      },
    },
    'agents/structuring': {
      title: { en: 'Structuring Worker — Fathom', ru: 'Воркер структурирования — Fathom' },
      desc: {
        en: 'Turns extracted fragments into canonical records — normalized, deduplicated, and linked in an entity graph.',
        ru: 'Превращает извлечённые фрагменты в канонические записи — нормализация, дедупликация и связь в графе сущностей.',
      },
    },
    api: {
      title: { en: 'API & Server — Fathom', ru: 'API и сервер — Fathom' },
      desc: {
        en: 'Run Fathom headless: REST and SSE endpoints, Prometheus metrics, health probes, and stateless deployment.',
        ru: 'Fathom в headless-режиме: REST и SSE эндпоинты, метрики Prometheus, health-пробы и stateless-деплой.',
      },
    },
    architecture: {
      title: {
        en: 'System Architecture & Rust Core — Fathom',
        ru: 'Архитектура системы и Rust-ядро — Fathom',
      },
      desc: {
        en: 'Full-stack compiled Rust multi-agent architecture. 13 workspace crates, sub-millisecond execution, fail-closed governance.',
        ru: 'Полностековая архитектура на скомпилированном Rust. 13 крейтов, субмиллисекундный диспатч, fail-closed безопасность.',
      },
    },
    benchmarks: {
      title: { en: 'Runtime Benchmarks & Performance — Fathom', ru: 'Бенчмарки и производительность — Fathom' },
      desc: {
        en: 'Measured performance of the Fathom Rust runtime: parallel speedup, HTML and feed throughput, memory and latency numbers.',
        ru: 'Измеренная производительность рантайма Fathom на Rust: параллельное ускорение, пропускная способность по HTML и фидам, память и задержки.',
      },
    },
    blog: {
      title: { en: 'Engineering Blog & Systems Notes — Fathom', ru: 'Инженерный блог и заметки о системах — Fathom' },
      desc: {
        en: 'Engineering notes from building Fathom: fan-out coordination, repo mapping benchmarks, durable memory, and verification pipelines.',
        ru: 'Заметки о разработке Fathom: координация fan-out, бенчмарки repo map, долговременная память и пайплайны верификации.',
      },
    },
    bot: {
      title: {
        en: 'Fathom Bot — Sovereign Autonomous AI Workforce in a Chat App',
        ru: 'Fathom Bot — Суверенные автономные ИИ-сотрудники в корпоративном чате',
      },
      desc: {
        en: 'Every sidebar contact is a real, universal AI coworker running on your private infrastructure. Terminal execution, deep web research, 24/7 routines.',
        ru: 'Каждый контакт в боковой панели — автономный цифровой сотрудник на вашей инфраструктуре. Команды в терминале, веб-разведка, рутины 24/7.',
      },
    },
    'blog/anatomy-of-fan-out': {
      title: { en: 'Anatomy of a fan-out: one question, a fleet of agents — Fathom', ru: 'Анатомия fan-out: один вопрос — флот агентов — Fathom' },
      desc: {
        en: "Deep dive into Fathom's fan-out architecture: plan decomposition, Tokio JoinSet parallelism, token budgeting, and synthesis.",
        ru: 'Глубокий разбор архитектуры fan-out в Fathom: декомпозиция плана, параллелизм на Tokio JoinSet, бюджет токенов и синтез.',
      },
    },
    'blog/benchmarks-repo-map': {
      title: { en: 'The benchmark that caught a 31× slowdown — Fathom', ru: 'Бенчмарк, который поймал замедление в 31 раз — Fathom' },
      desc: {
        en: 'How the fathom bench test suite caught regex recompilation and serialization bottlenecks in the tool layer.',
        ru: 'Как набор бенчмарков fathom bench выявил рекомпиляцию регулярных выражений и узкие места сериализации в слое инструментов.',
      },
    },
    'blog/memory-that-remembers': {
      title: { en: 'A memory that remembers: append-only knowledge — Fathom', ru: 'Память, которая помнит: append-only знания — Fathom' },
      desc: {
        en: 'Append-only semantic knowledge base in SQLite: six-outcome absorb pipeline, hybrid search with freshness decay.',
        ru: 'Семантическая база знаний в SQLite с append-only подходом: пайплайн поглощения с шестью исходами, гибридный поиск с затуханием свежести.',
      },
    },
    'blog/verification-pipeline': {
      title: { en: 'Five gates: verification before side effects — Fathom Blog', ru: 'Пять шлюзов: верификация перед побочными действиями — блог Fathom' },
      desc: {
        en: 'How Fathom verifies data across five progressive gates with append-only audit receipts before committing side effects.',
        ru: 'Как Fathom проверяет данные через пять последовательных шлюзов с append-only аудит-квитанциями до побочных действий.',
      },
    },
    changelog: {
      title: { en: 'Changelog & Release History — Fathom', ru: 'История изменений и релизов — Fathom' },
      desc: {
        en: 'Every Fathom release, dated and measured: new worker capabilities, tool additions, performance work, and fixes.',
        ru: 'Все релизы Fathom с датами и замерами: новые возможности воркеров, инструменты, работа над производительностью и исправления.',
      },
    },
    code: {
      title: { en: 'Code & Tools — Fathom', ru: 'Код и инструменты — Fathom' },
      desc: {
        en: 'How Fathom writes and runs code: AST repo mapping, sandboxed execution, precision edits with rollback, and git worktrees.',
        ru: 'Как Fathom пишет и исполняет код: AST-карта репозитория, исполнение в песочнице, точечные правки с откатом и git worktree.',
      },
    },
    dashboard: {
      title: { en: 'Web Dashboard — Fathom', ru: 'Веб-панель — Fathom' },
      desc: {
        en: 'The Fathom web dashboard: one screen for sessions, worker trees, run timelines, approvals, and Prometheus metrics.',
        ru: 'Веб-панель Fathom: один экран для сессий, дерева воркеров, таймлайнов запусков, подтверждений и метрик Prometheus.',
      },
    },
    demo: {
      title: {
        en: 'Interactive Live Worker Demo — Fathom',
        ru: 'Интерактивное демо воркеров — Fathom',
      },
      desc: {
        en: 'Pick a worker, type a task, and watch it plan, run tools, absorb memory, verify, and deliver.',
        ru: 'Выберите воркера, опишите задачу и смотрите, как он планирует, вызывает инструменты и выдаёт результат.',
      },
    },
    docs: {
      title: {
        en: 'Documentation — Fathom',
        ru: 'Документация — Fathom',
      },
      desc: {
        en: 'Fathom documentation: quickstart, tools catalog, memory system, configuration, HTTP API, and architecture.',
        ru: 'Документация Fathom: быстрый старт, каталог инструментов, память, конфигурация, HTTP API и архитектура.',
      },
    },
    'docs/api': {
      title: { en: 'HTTP API — Fathom', ru: 'HTTP API — Fathom' },
      desc: {
        en: 'Control autonomous worker sessions, coworkers, schedules, memory, governance, computer use, and notifications over HTTP.',
        ru: 'Управление сессиями автономных воркеров, сотрудниками, расписаниями, памятью, управлением, computer use и уведомлениями через HTTP.',
      },
    },
    'docs/bot': {
      title: { en: 'Fathom Bot — Chat App Architecture', ru: 'Fathom Bot — архитектура чат-приложения' },
      desc: {
        en: 'Architecture, drivers, and runtime mechanics of Fathom Bot: a local-first desktop and web messaging client for teams of autonomous AI agents.',
        ru: 'Архитектура, драйверы и механика работы Fathom Bot: локальное десктопное и веб-приложение для общения с командами автономных ИИ-агентов.',
      },
    },
    'docs/architecture': {
      title: { en: 'Architecture — Fathom', ru: 'Архитектура — Fathom' },
      desc: {
        en: "Understand Fathom's Rust worker runtime: agent lifecycle, coordination, tools, memory, persistence, and governance.",
        ru: 'Как устроен рантайм воркеров Fathom на Rust: жизненный цикл агентов, координация, инструменты, память, персистентность и управление.',
      },
    },
    'docs/cli': {
      title: { en: 'CLI Reference — Fathom', ru: 'Справочник CLI — Fathom' },
      desc: {
        en: 'Use the Fathom CLI to run autonomous worker tasks, operate the TUI or server, manage memory and sessions, schedule jobs, and inspect runtime statistics.',
        ru: 'Использование CLI Fathom: запуск задач автономных воркеров, работа с TUI и сервером, управление памятью и сессиями, расписания и статистика рантайма.',
      },
    },
    'docs/configuration': {
      title: { en: 'Configuration — Fathom', ru: 'Конфигурация — Fathom' },
      desc: {
        en: "Configure Fathom's autonomous worker runtime with LLM providers, tools, memory, agents, notifications, and integrations.",
        ru: 'Настройка рантайма автономных воркеров Fathom: провайдеры LLM, инструменты, память, агенты, уведомления и интеграции.',
      },
    },
    'docs/memory': {
      title: { en: 'Memory — Fathom', ru: 'Память — Fathom' },
      desc: {
        en: "Manage Fathom's persistent semantic memory, entity graph, scopes, absorption, distillation, search, and cleanup.",
        ru: 'Управление постоянной семантической памятью Fathom: граф сущностей, области, поглощение, дистилляция, поиск и очистка.',
      },
    },
    'docs/outreach': {
      title: { en: 'Outreach & Leads — Fathom', ru: 'Аутрич и лиды — Fathom' },
      desc: {
        en: "Build verified, enriched lead lists and personalized outreach with Fathom's autonomous worker tools, memory, and CRM sync.",
        ru: 'Создание верифицированных и обогащённых списков лидов и персонализированный аутрич с инструментами воркеров Fathom, памятью и синхронизацией CRM.',
      },
    },
    'docs/personalization': {
      title: { en: 'Personalization — Fathom', ru: 'Персонализация — Fathom' },
      desc: {
        en: 'Tune Fathom worker behavior with profiles, role-specific models, tool policies, personas, and review gates.',
        ru: 'Настройка поведения воркеров Fathom: профили, ролевые модели, политики инструментов, персоны и шлюзы проверки.',
      },
    },
    'docs/protocol-gui': {
      title: { en: 'LSP & IDE Integration — Fathom', ru: 'Интеграция с LSP и IDE — Fathom' },
      desc: {
        en: "Connect Fathom's autonomous worker runtime to language servers and IDE tooling through its built-in LSP integration.",
        ru: 'Подключение рантайма воркеров Fathom к языковым серверам и инструментам IDE через встроенную интеграцию LSP.',
      },
    },
    'docs/quickstart': {
      title: { en: 'Quickstart — Fathom', ru: 'Быстрый старт — Fathom' },
      desc: {
        en: 'Get started with Fathom autonomous AI workers. Server deployment and access are available by request.',
        ru: 'Начало работы с автономными ИИ-воркерами Fathom. Деплой на сервере и доступ — по запросу.',
      },
    },
    'docs/recipes': {
      title: { en: 'Recipes — Fathom', ru: 'Рецепты — Fathom' },
      desc: {
        en: 'Practical Fathom recipes for autonomous worker tasks, outreach, memory, integrations, monitoring, scheduling, and recovery.',
        ru: 'Практические рецепты Fathom для задач автономных воркеров: аутрич, память, интеграции, мониторинг, расписания и восстановление.',
      },
    },
    'docs/tools': {
      title: { en: 'Tools — Fathom', ru: 'Инструменты — Fathom' },
      desc: {
        en: "Reference for Fathom's built-in tools across web, code, files, browser, computer use, memory, security, and coordination.",
        ru: 'Справочник встроенных инструментов Fathom: веб, код, файлы, браузер, computer use, память, безопасность и координация.',
      },
    },
    features: {
      title: { en: 'Capabilities — Fathom Autonomous AI Workers', ru: 'Возможности — автономные ИИ-воркеры Fathom' },
      desc: {
        en: "Explore Fathom's autonomous worker capabilities: research, outreach, code, computer use, scheduling, and memory.",
        ru: 'Возможности автономных воркеров Fathom: ресёрч, аутрич, код, управление компьютером, расписания и память.',
      },
    },
    'features/auto-outreach': {
      title: { en: 'Outreach Workers — Fathom', ru: 'Воркеры аутрича — Fathom' },
      desc: {
        en: 'Autonomous outreach workers ground personalized drafts in verified context, memory, and human approvals.',
        ru: 'Автономные воркеры аутрича строят персонализированные черновики на проверенном контексте, памяти и подтверждениях человека.',
      },
    },
    'features/lead-generation': {
      title: { en: 'Lead Discovery Workflow — Fathom', ru: 'Процесс поиска лидов — Fathom' },
      desc: {
        en: 'Use autonomous workers to discover, verify, and organize contact context as a governed workflow with memory.',
        ru: 'Автономные воркеры находят, проверяют и организуют контекст контактов как управляемый процесс с памятью.',
      },
    },
    'features/research': {
      title: { en: 'Research Workflow — Fathom', ru: 'Процесс ресёрча — Fathom' },
      desc: {
        en: 'Use universal autonomous workers to plan research, delegate tools, preserve evidence in memory, and synthesize.',
        ru: 'Универсальные автономные воркеры планируют исследования, делегируют инструменты, сохраняют доказательства в память и синтезируют.',
      },
    },
    install: {
      title: { en: 'Installation — Fathom', ru: 'Установка — Fathom' },
      desc: {
        en: 'Install Fathom: one static Rust binary for Linux, macOS, and Windows, plus Docker and self-hosted server options.',
        ru: 'Установка Fathom: один статический Rust-бинарник для Linux, macOS и Windows, плюс Docker и self-hosted сервер.',
      },
    },
    integrations: {
      title: { en: 'Integrations — Fathom', ru: 'Интеграции — Fathom' },
      desc: {
        en: 'Configure Fathom workers with CRM, MCP, optional browser and LSP services, notifications, search backends, and exports.',
        ru: 'Настройка интеграций воркеров Fathom: CRM, MCP, опциональные браузерные и LSP-сервисы, уведомления, поисковые бэкенды и экспорт.',
      },
    },
    mcp: {
      title: { en: 'Model Context Protocol (MCP) — Fathom', ru: 'Model Context Protocol (MCP) — Fathom' },
      desc: {
        en: 'Fathom speaks MCP both ways: consume external tools over stdio and HTTP, and expose the built-in worker toolset.',
        ru: 'Fathom работает с MCP в обе стороны: использует внешние инструменты по stdio и HTTP и открывает встроенный набор инструментов воркеров.',
      },
    },
    memory: {
      title: { en: 'Memory & Graph — Fathom', ru: 'Память и граф — Fathom' },
      desc: {
        en: 'Self-hosted semantic memory for autonomous workers: hybrid vector + BM25 search, append-only versioning, and entity graphs.',
        ru: 'Self-hosted семантическая память для автономных воркеров: гибридный векторный + BM25 поиск, append-only версии и графы сущностей.',
      },
    },
    ops: {
      title: { en: 'Scheduled Operations & Automation — Fathom', ru: 'Операции по расписанию и автоматизация — Fathom' },
      desc: {
        en: 'Scheduled and durable operations in Fathom: background jobs that survive restarts, cron schedules, daemons, and hooks.',
        ru: 'Запланированные и устойчивые операции в Fathom: фоновые задачи, переживающие перезапуск, cron-расписания, демоны и хуки.',
      },
    },
    osint: {
      title: { en: 'OSINT Workflow — Fathom', ru: 'OSINT-процессы — Fathom' },
      desc: {
        en: 'OSINT workflow with Fathom: open-source discovery across people, companies, and infrastructure with provenance.',
        ru: 'OSINT-процессы с Fathom: сбор открытых данных по людям, компаниям и инфраструктуре с указанием источников.',
      },
    },
    playground: {
      title: { en: 'GitHub Repository & Code Structure — Fathom', ru: 'Репозиторий на GitHub и структура кода — Fathom' },
      desc: {
        en: 'Technical breakdown of the Fathom GitHub repository: 13 Rust crates, 3 apps, CLI tools, and the internal architecture.',
        ru: 'Технический разбор репозитория Fathom на GitHub: 13 крейтов Rust, 3 приложения, CLI и внутренняя архитектура.',
      },
    },
    pricing: {
      title: {
        en: 'Pricing & Node Deployment — Fathom',
        ru: 'Тарифы и развёртывание нод — Fathom',
      },
      desc: {
        en: 'Predictable sovereign pricing. $79/mo Personal and $200-$500/mo Custom Enterprise Node. Unlimited tokens on your hardware.',
        ru: 'Предсказуемые суверенные тарифы. $79/мес Personal и $200–$500/мес Enterprise Node. Неограниченные токены на вашем оборудовании.',
      },
    },
    profiles: {
      title: { en: 'Profiles & Personas — Fathom', ru: 'Профили и персоны — Fathom' },
      desc: {
        en: 'Worker profiles and personas in Fathom: built-in roles from SDR to maintainer, each with its own toolset and policies.',
        ru: 'Профили и персоны воркеров в Fathom: встроенные роли от SDR до мейнтейнера, каждая со своим набором инструментов и политиками.',
      },
    },
    research: {
      title: { en: 'Research Workflows — Fathom', ru: 'Процессы исследований — Fathom' },
      desc: {
        en: 'Deep research with Fathom: seven fused search backends, source-aware fetching, cross-referencing, and synthesis.',
        ru: 'Глубокие исследования с Fathom: семь объединённых поисковых бэкендов, загрузка с учётом источника, перекрёстная проверка и синтез.',
      },
    },
    security: {
      title: {
        en: 'Security, Governance & Data Sovereignty — Fathom',
        ru: 'Безопасность, управление и суверенитет данных — Fathom',
      },
      desc: {
        en: '100% on-premise sovereign deployment, AES-256-GCM vault, strict sandboxing, zero cloud telemetry, compliance with Uzbekistan Law #547.',
        ru: '100% on-premise суверенное развёртывание, AES-256-GCM хранилище, песочницы, нулевая телеметрия, соответствие ЗРУ № 547.',
      },
    },
    solutions: {
      title: { en: 'Enterprise Solutions — Fathom Autonomous AI Workforce', ru: 'Корпоративные решения — автономный ИИ-штат Fathom' },
      desc: {
        en: 'Turnkey autonomous AI employee solutions for sales outbound, talent scouting, market intelligence, and back-office operations.',
        ru: 'Готовые решения автономных ИИ-сотрудников: исходящие продажи, поиск кандидатов, рыночная разведка и бэкофис-операции.',
      },
    },
    'solutions/agency-whitelabel': {
      title: { en: 'Agency Multi-Tenant Fleets — Fathom Enterprise Solutions', ru: 'Мультитенантные флоты для агентств — корпоративные решения Fathom' },
      desc: {
        en: 'Scale your agency with autonomous AI employee fleets: 12+ isolated client pods, white-label branding, and per-client governance.',
        ru: 'Масштабируйте агентство с флотами автономных ИИ-сотрудников: 12+ изолированных клиентских подов, white-label брендинг и управление по клиентам.',
      },
    },
    'solutions/b2b-outbound': {
      title: { en: 'Autonomous Outbound SDR — Fathom Enterprise Solutions', ru: 'Автономный аутбаунд-SDR — корпоративные решения Fathom' },
      desc: {
        en: 'Autonomous B2B sales development representative: 50 verified decision-makers daily, 0% email bounces, CRM sync.',
        ru: 'Автономный SDR для B2B-продаж: 50 проверенных ЛПР в день, 0% отказов доставки, синхронизация с CRM.',
      },
    },
    'solutions/backoffice-finance': {
      title: { en: 'Back-Office & Invoice Reconciliation — Fathom Solutions', ru: 'Бэкофис и сверка счетов — решения Fathom' },
      desc: {
        en: 'Autonomous back-office finance employee: 3-way invoice matching in QuickBooks and ERPs, PDF OCR extraction, audit receipts.',
        ru: 'Автономный сотрудник финансового бэкофиса: трёхсторонняя сверка счетов в QuickBooks и ERP, OCR-распознавание PDF, аудит-квитанции.',
      },
    },
    'solutions/executive-recruiting': {
      title: { en: 'Technical Talent Scout & Recruiting — Fathom Enterprise Solutions', ru: 'Поиск технических специалистов и рекрутинг — корпоративные решения Fathom' },
      desc: {
        en: 'Autonomous technical talent scout: AST commit mining across GitHub, open-source portfolio analysis, and candidate dossiers.',
        ru: 'Автономный поиск технических специалистов: AST-анализ коммитов на GitHub, оценка open-source портфолио и досье кандидатов.',
      },
    },
    'solutions/market-intelligence': {
      title: { en: 'Market Intelligence & Competitor Tracking — Fathom', ru: 'Рыночная разведка и отслеживание конкурентов — Fathom' },
      desc: {
        en: 'Autonomous real-time market intelligence: 24/7 competitor DOM change detection, pricing sweeps, and signal digests.',
        ru: 'Автономная рыночная разведка в реальном времени: отслеживание изменений DOM конкурентов 24/7, свипы цен и дайджесты сигналов.',
      },
    },
    tools: {
      title: { en: 'Built-in Tools & Integrations Catalog — Fathom', ru: 'Каталог встроенных инструментов и интеграций — Fathom' },
      desc: {
        en: 'The Fathom toolset: 63 built-in Rust tools for web, files, shell, code, browser, memory, and Git — each scoped by policy and approval gates.',
        ru: 'Набор инструментов Fathom: 63 встроенных инструмента на Rust для веба, файлов, шелла, кода, браузера, памяти и Git — каждый ограничен политиками и шлюзами подтверждения.',
      },
    },
    tui: {
      title: { en: 'Terminal UI — Fathom', ru: 'Терминальный интерфейс — Fathom' },
      desc: {
        en: 'The Fathom terminal UI: a Ratatui agent tree, live tool calls, token and cost accounting, session replay.',
        ru: 'Терминальный интерфейс Fathom: дерево агентов на Ratatui, живые вызовы инструментов, учёт токенов и стоимости, реплей сессий.',
      },
    },
    video: {
      title: { en: 'Interactive Video Showreel — Fathom', ru: 'Интерактивный видео-шоурил — Fathom' },
      desc: {
        en: 'Fathom 8-minute showreel: 15 autonomous AI worker scenarios. Sales outbound, market intelligence, talent scouting, and more.',
        ru: 'Восьмиминутный шоурил Fathom: 15 сценариев автономных ИИ-воркеров. Исходящие продажи, рыночная разведка, поиск талантов и другое.',
      },
    },
    'vs-python': {
      title: { en: 'Fathom (Rust) vs. Python AI Frameworks — Benchmark Comparison', ru: 'Fathom (Rust) против Python-фреймворков ИИ — сравнение по бенчмаркам' },
      desc: {
        en: 'Why Fathom was built in Rust: 0.75 ms tool dispatch, 15.4 MB RAM footprint, hardware security vault, and 100+ concurrent workers per server.',
        ru: 'Почему Fathom написан на Rust: диспатч инструментов 0,75 мс, потребление 15,4 МБ RAM, аппаратное хранилище секретов и 100+ одновременных воркеров на сервер.',
      },
    },
    whitepaper: {
      title: {
        en: 'Technical Whitepaper & Formal Specifications — Fathom',
        ru: 'Техническая документация и спецификации — Fathom',
      },
      desc: {
        en: 'Mathematical foundations, multi-agent swarms, DAG orchestration, benchmark methodology, and cryptographic verification receipts.',
        ru: 'Математические основы, многоагентные рои, DAG-оркестрация, методология бенчмарков и криптографические квитанции действий.',
      },
    },
  },
};
