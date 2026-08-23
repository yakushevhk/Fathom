// Auto-imported by switcher.js via import.meta.glob('./groups/*.js').
// Per-prefix translation blocks for install, pricing, changelog, blog, 404,
// and the blog/benchmarks-repo-map post. Each key resolves {en,ru}.
export default {
  install: {
    hero: {
      badge: { en: 'Install', ru: 'Установка' },
      title: { en: 'One binary. Every runtime.', ru: 'Один бинарник. Любой рантайм.' },
      sub: {
        en: 'Fathom is a source-available, self-hosted runtime for autonomous work. Server deployment and access are available by request.',
        ru: 'Fathom — доступный по исходному коду self-hosted рантайм для автономной работы. Развёртывание на сервере и доступ доступны по запросу.',
      },
    },
    contact: {
      title: { en: 'Contact us for access', ru: 'Свяжитесь с нами для доступа' },
      body: {
        en: 'Self-hosted deployment is currently available by request. Get in touch and we\'ll set up a server for you with full access to the runtime, API, and dashboard.',
        ru: 'Self-hosted развёртывание сейчас доступно по запросу. Свяжитесь с нами, и мы настроим для вас сервер с полным доступом к рантайму, API и дашборду.',
      },
      footnote: {
        en: 'We\'ll respond within 24 hours with server credentials and documentation.',
        ru: 'Мы ответим в течение 24 часов с учётными данными сервера и документацией.',
      },
    },
    cta: {
      title: { en: 'Ready to get started?', ru: 'Готовы начать?' },
      sub: { en: 'Email us to request server access and we\'ll get you set up.', ru: 'Напишите нам, чтобы запросить доступ к серверу, и мы всё настроим.' },
      next: { en: 'Contact us →', ru: 'Свяжитесь с нами →' },
      docs: { en: 'Documentation', ru: 'Документация' },
    },
  },
  pricingPage: {
    hero: {
      badge: { en: 'Access', ru: 'Доступ' },
      title: { en: 'Remote AI workers <em class="it">on your team.</em>', ru: 'Удалённые ИИ-воркеры <em class="it">в вашей команде.</em>' },
      sub: { en: 'Fathom deploys on dedicated infrastructure for your team. Contact us for access — we\'ll set up a server with full runtime, API, dashboard, and memory.', ru: 'Fathom разворачивается на выделенной инфраструктуре для вашей команды. Свяжитесь с нами для доступа — мы настроим сервер с полным рантаймом, API, дашбордом и памятью.' },
    },
    contact: {
      title: { en: 'Request access', ru: 'Запросить доступ' },
      body: { en: 'We provision servers for your team with full access to the Fathom runtime, CLI, TUI, API, dashboard, and integrations. Email us to get started.', ru: 'Мы предоставляем серверы для вашей команды с полным доступом к рантайму Fathom, CLI, TUI, API, дашборду и интеграциям. Напишите нам, чтобы начать.' },
      footnote: { en: 'We\'ll respond within 24 hours with server credentials and documentation.', ru: 'Мы ответим в течение 24 часов с учётными данными сервера и документацией.' },
    },
    includes: {
      badge: { en: 'What\'s included', ru: 'Что входит' },
      title: { en: 'Full worker platform', ru: 'Полная платформа воркеров' },
      '1': { name: { en: 'Autonomous workers', ru: 'Автономные воркеры' }, d: { en: 'Research, outreach, code, computer use, and scheduled operations — one runtime, every capability.', ru: 'Исследования, аутрич, код, работа за компьютером и задачи по расписанию — один рантайм, все возможности.' } },
      '2': { name: { en: 'Web dashboard & API', ru: 'Веб-дашборд и API' }, d: { en: 'Full control plane with REST API, SSE streaming, AG-UI compatibility, and the embedded web dashboard.', ru: 'Полная панель управления с REST API, SSE-потоками, AG-UI-совместимостью и встроенным веб-дашбордом.' } },
      '3': { name: { en: 'Durable memory', ru: 'Долговечная память' }, d: { en: 'Hybrid vector + BM25 semantic memory, entity graph, and append-only knowledge that persists across sessions.', ru: 'Гибридная векторная + BM25 семантическая память, граф сущностей и знания, сохраняющиеся между сессиями.' } },
      '4': { name: { en: 'Governance & approvals', ru: 'Управление и одобрения' }, d: { en: 'Approval gates, policy hooks, secret scanning, and audit trails keep every side effect reviewable.', ru: 'Шлюзы одобрения, хуки политик, сканирование секретов и журналы аудита делают каждый побочный эффект проверяемым.' } },
      '5': { name: { en: 'Integrations', ru: 'Интеграции' }, d: { en: 'CRM push (amoCRM, Bitrix24, HubSpot), MCP, notifications (webhook, email, Telegram), and search backends.', ru: 'CRM-синхронизация (amoCRM, Bitrix24, HubSpot), MCP, уведомления (webhook, email, Telegram) и поисковые бэкенды.' } },
      '6': { name: { en: 'Support', ru: 'Поддержка' }, d: { en: 'Server provisioning, configuration assistance, and ongoing operational support from the Fathom team.', ru: 'Предоставление сервера, помощь в настройке и постоянная поддержка от команды Fathom.' } },
    },
    cta: {
      title: { en: 'Ready to deploy workers for your team?', ru: 'Готовы развернуть воркеров для вашей команды?' },
      sub: { en: 'Email us and we\'ll get you set up with a dedicated server environment.', ru: 'Напишите нам, и мы настроим для вас выделенную серверную среду.' },
      access: { en: 'Request access →', ru: 'Запросить доступ →' },
      docs: { en: 'Read the docs', ru: 'Читать документацию' },
    },
  },
  changelog: {
    hero: {
      badge: { en: 'Changelog', ru: 'Журнал изменений' },
      title: { en: 'Shipped, measured, logged.', ru: 'Выпущено, измерено, зафиксировано.' },
      sub: {
        en: 'What changed, when, and why — straight from the product repository. Fleet findings become phases, phases become batches, batches become releases.',
        ru: 'Что изменилось, когда и почему — прямо из репозитория продукта. Находки флота становятся фазами, фазы — батчами, батчи — релизами.',
      },
    },
    note: {
      en: '* Full history ships with your deployment — run <code>fathom</code> on your own infrastructure; every build carries its own log.',
      ru: '* Полная история поставляется с вашим деплоем — запускайте <code>fathom</code> на своей инфраструктуре; каждая сборка несёт свой собственный лог.',
    },
    cta: {
      title: { en: 'Every release is benchmarked.', ru: 'Каждый релиз проходит бенчмарк.' },
      numbers: { en: 'See the numbers', ru: 'Смотреть цифры' },
      blog: { en: 'Read the engineering blog', ru: 'Читать блог об инженерии' },
    },
  },
  blog: {
    hero: {
      badge: { en: 'Engineering Blog', ru: 'Инженерный блог' },
      title: { en: 'Notes from the build.', ru: 'Заметки со стройки.' },
      sub: {
        en: 'How Fathom actually works — benchmarks, memory design, agent orchestration. Written by the people who compile it.',
        ru: 'Как на самом деле работает Fathom — бенчмарки, дизайн памяти, оркестрация агентов. Написано теми, кто его собирает.',
      },
    },
    post: {
      0: {
        cat: { en: 'Benchmarks', ru: 'Бенчмарки' },
        title: { en: 'The benchmark that caught a 31× slowdown', ru: 'Бенчмарк, поймавший замедление в 31×' },
      },
      1: {
        cat: { en: 'Memory', ru: 'Память' },
        title: { en: 'A memory that remembers: append-only knowledge for research agents', ru: 'Память, которая помнит: append-only знания для исследовательских агентов' },
      },
      2: {
        cat: { en: 'Architecture', ru: 'Архитектура' },
        title: { en: 'Anatomy of a fan-out: one question, a fleet of agents', ru: 'Анатомия fan-out: один вопрос — флот агентов' },
      },
      3: {
        cat: { en: 'Verification', ru: 'Верификация' },
        title: { en: 'Five gates: how we verify every email before it hits the CRM', ru: 'Пять шлюзов: как мы проверяем каждый email до попадания в CRM' },
      },
    },
    cta: {
      title: { en: 'Numbers you can re-run.', ru: 'Цифры, которые можно перепроверить.' },
      benchmarks: { en: 'See the benchmarks', ru: 'Смотреть бенчмарки' },
      changelog: { en: 'Read the changelog', ru: 'Читать журнал изменений' },
    },
  },
  nf404: {
    eyebrow: { en: '404', ru: '404' },
    title: { en: 'This page was<br />superseded.', ru: 'Эта страница<br />была заменена.' },
    copy: {
      en: 'Like facts in our memory store, some pages get replaced by newer versions. The one you requested is archived.',
      ru: 'Как и факты в нашем хранилище памяти, некоторые страницы заменяются новыми версиями. Та, которую вы запросили, архивирована.',
    },
    home: { en: 'Back home', ru: 'На главную' },
    docs: { en: 'Documentation', ru: 'Документация' },
  },
  postBench: {
    crumb: {
      blog: { en: 'Blog', ru: 'Блог' },
      cat: { en: 'Benchmarks', ru: 'Бенчмарки' },
      sub: { en: 'repo_map, 31×', ru: 'repo_map, 31×' },
    },
    hero: {
      date: { en: 'August 8, 2026 · 6 min read', ru: '8 августа 2026 · 6 минут чтения' },
      tag: { en: 'Benchmarks', ru: 'Бенчмарки' },
      title: { en: 'The benchmark that caught a 31× slowdown', ru: 'Бенчмарк, поймавший замедление в 31×' },
      intro: {
        en: 'Fathom ships its own benchmark harness: <code>fathom bench</code>. It is not a showcase of flattering numbers — it is an instrument the tool layer runs against itself. In its short life it has already caught two real regressions: a 31× slowdown hiding in the repository-map builder, and four tools that had silently fallen back to one-at-a-time execution.',
        ru: 'Fathom поставляет собственный бенчмарк-стенд: <code>fathom bench</code>. Это не витрина лестных цифр — это инструмент, на котором слой инструментов тестирует сам себя. За короткую жизнь он уже поймал две реальные регрессии: замедление в 31×, скрытое в построителе карты репозитория, и четыре инструмента, незаметно перешедших на последовательное выполнение по одному.',
      },
    },
    hl: {
      title: { en: 'Highlights', ru: 'Основное' },
      item: {
        0: { en: 'repo_map got 31× faster after a benchmark exposed per-file regex compilation', ru: 'repo_map стал в 31× быстрее после того, как бенчмарк вскрыл компиляцию regex на каждый файл' },
        1: { en: 'Four tools were silently serialized — caught only because the harness printed 1.00×', ru: 'Четыре инструмента незаметно сериализовались — поймано только потому, что стенд вывел 1.00×' },
        2: { en: 'Executor overhead amortizes to 753 µs per call in an 8-call batch', ru: 'Накладные расходы экзекутора амортизируются до 753 µs на вызов в батче из 8 вызовов' },
        3: { en: '~531k rows/s for HTML selectors, ~1.1M items/s for RSS parsing', ru: '~531k строк/с для HTML-селекторов, ~1.1M элементов/с для парсинга RSS' },
        4: { en: 'No network, no LLM — the benchmark suite runs in CI', ru: 'Ни сети, ни LLM — бенчмарк-набор работает в CI' },
      },
    },
    h2: {
      0: { en: 'A benchmark is an instrument, not a showcase', ru: 'Бенчмарк — это инструмент, а не витрина' },
      1: { en: '31× from a regex cache', ru: '31× из-за кэша regex' },
      2: { en: 'The silent serialization', ru: 'Тихая сериализация' },
      3: { en: 'The rest of the scoreboard', ru: 'Остальная часть табло' },
      4: { en: 'Run it in CI', ru: 'Запускайте в CI' },
    },
    p: {
      1: {
        en: 'The LLM decides <em>what</em> to do; the tools do the work. In between sits the <code>ToolExecutor</code> — parallel-safe vs sequential classification, path-conflict detection, cascading cancellation, and the serde machinery around every <code>ToolCall</code>. This layer runs on every step of every agent, so its characteristics directly define how long a research run takes.',
        ru: 'LLM решает, <em>что</em> делать; инструменты выполняют работу. Между ними находится <code>ToolExecutor</code> — классификация параллельно-безопасное/последовательное, детекция конфликтов путей, каскадная отмена и serde-механика вокруг каждого <code>ToolCall</code>. Этот слой работает на каждом шаге каждого агента, поэтому его характеристики напрямую определяют, сколько длится исследовательский запуск.',
      },
      2: {
        en: 'That is why the harness is built into the product. Fixtures are created automatically in a temp directory and removed after; each scenario repeats 5–10 times with a warm-up call, and the numbers below are one run on macOS, 10 cores, release build. No network, no LLM — the memory scenario uses an offline TF-IDF embedder, so the whole suite can run in CI.',
        ru: 'Именно поэтому стенд встроен в продукт. Фикстуры автоматически создаются во временном каталоге и затем удаляются; каждый сценарий повторяется 5–10 раз с прогревочным вызовом, а цифры ниже — это один запуск на macOS, 10 ядрах, в release-сборке. Ни сети, ни LLM — сценарий памяти использует офлайн TF-IDF эмбеддер, поэтому весь набор можно запускать в CI.',
      },
      3: {
        en: 'The first version of <code>extract_symbols</code> compiled two regular expressions (<code>regex::Regex::new</code>) <em>per file</em>. For <code>code_symbols</code> with its limit that was tolerable — the loop stops after the first ~25 files. For <code>repo_map</code>, which honestly walks the whole tree, it was a catastrophe: DFA compilation dominated the runtime.',
        ru: 'Первая версия <code>extract_symbols</code> компилировала два регулярных выражения (<code>regex::Regex::new</code>) <em>на каждый файл</em>. Для <code>code_symbols</code> с его лимитом это было терпимо — цикл останавливается после первых ~25 файлов. Для <code>repo_map</code>, который честно обходит всё дерево, это было катастрофой: компиляция DFA доминировала в рантайме.',
      },
      4: {
        en: 'The fix moved the regexes into <code>OnceLock</code> statics (compiled once per process) and parallelized file reads in <code>repo_map</code> via <code>tokio::spawn</code> + <code>join_all</code>, preserving order. An A/B run over 240 synthetic Rust files:',
        ru: 'Исправление перенесло regex в статики <code>OnceLock</code> (компилируются один раз на процесс) и распараллелило чтение файлов в <code>repo_map</code> через <code>tokio::spawn</code> + <code>join_all</code>, сохранив порядок. A/B-прогон по 240 синтетическим Rust-файлам:',
      },
      5: {
        en: 'That is a 31× improvement on a debug build; in release the same tree takes 6.6 ms. The live reference point: before the fix, <code>repo_map</code> over this very project\'s repository (103 files) took 106 ms in release. After the fix — single-digit milliseconds.',
        ru: 'Это улучшение в 31× на debug-сборке; в release то же дерево обрабатывается за 6.6 ms. Живая контрольная точка: до исправления <code>repo_map</code> по репозиторию этого самого проекта (103 файла) занимал 106 ms в release. После исправления — единицы миллисекунд.',
      },
      6: {
        en: 'The second catch was subtler. <code>ToolExecutor</code> splits tools into parallel-safe and sequential; a tool missing from the classification goes sequential "just in case". When <code>web_crawl</code>, <code>web_feed</code>, <code>code_symbols</code> and <code>repo_map</code> landed, nobody added them to the classification — and any batch of several such calls <em>silently ran one at a time</em>, with no errors and no warnings.',
        ru: 'Вторая находка была тоньше. <code>ToolExecutor</code> делит инструменты на параллельно-безопасные и последовательные; инструмент, которого нет в классификации, «на всякий случай» уходит в последовательные. Когда появились <code>web_crawl</code>, <code>web_feed</code>, <code>code_symbols</code> и <code>repo_map</code>, никто не добавил их в классификацию — и любой батч из нескольких таких вызовов <em>незаметно выполнялся по одному</em>, без ошибок и предупреждений.',
      },
      7: {
        en: 'The harness showed it in numbers: a spawn batch of 8 × <code>web_feed</code> produced 1.00× — exactly what sequential execution produces — while <code>parse_html</code> under the same conditions produced ~3×. After adding the tools to <code>parallel_safe</code> (and a classification test to keep them there):',
        ru: 'Стенд показал это в цифрах: spawn-батч из 8 × <code>web_feed</code> дал 1.00× — ровно то, что даёт последовательное выполнение, — тогда как <code>parse_html</code> в тех же условиях дал ~3×. После добавления инструментов в <code>parallel_safe</code> (и теста классификации, чтобы они там остались):',
      },
      8: {
        en: 'The same run measures the machinery around the tools. Serializing a <code>ToolCall</code>\'s arguments costs ~750 ns; the executor\'s overhead over raw registry dispatch is ~2.5 ms for a single-call batch and amortizes to <strong>753 µs per call</strong> in an 8-call batch. The execution layer disappears against the work the tools do.',
        ru: 'Тот же прогон измеряет механику вокруг инструментов. Сериализация аргументов <code>ToolCall</code> стоит ~750 ns; накладные расходы экзекутора поверх обычной диспетчеризации реестра — ~2.5 ms для батча из одного вызова и амортизируются до <strong>753 µs на вызов</strong> в батче из 8 вызовов. Слой выполнения исчезает на фоне работы инструментов.',
      },
      9: {
        en: 'For CPU-bound batches the difference between execution modes is dramatic: 8 × <code>parse_html</code> of a ~1 MB table runs <strong>3.78×</strong> faster under <code>execute_batch_spawn</code> (tokio tasks spread across cores) than sequentially, while <code>join_all</code> — which polls futures on one thread — only helps for I/O waits. HTML selector throughput peaks at ~531k rows/s on small documents (~350k rows/s on large ones), and the quick-xml RSS parser sustains ~1.1M items/s.',
        ru: 'Для CPU-нагруженных батчей разница между режимами выполнения драматична: 8 × <code>parse_html</code> таблицы в ~1 MB работает в <strong>3.78×</strong> быстрее под <code>execute_batch_spawn</code> (задачи tokio распределяются по ядрам), чем последовательно, тогда как <code>join_all</code> — который опрашивает фьючерсы на одном потоке — помогает только при ожидании I/O. Пропускная способность HTML-селекторов пиковая ~531k строк/с на малых документах (~350k строк/с на больших), а quick-xml RSS-парсер держит ~1.1M элементов/с.',
      },
      10: {
        en: 'A realistic mixed turn — four reads, three writes and a grep in one batch — is partitioned automatically: five calls run concurrently, three serialize, and the result vector still matches the original call order. Total wall time: 70.9 ms.',
        ru: 'Реалистичный смешанный ход — четыре чтения, три записи и один grep в одном батче — разбивается автоматически: пять вызовов выполняются конкурентно, три сериализуются, а результирующий вектор по-прежнему совпадает с исходным порядком вызовов. Общее реальное время: 70.9 ms.',
      },
      11: {
        en: 'Everything above reproduces with one command — <code>fathom bench --scenario all</code> — with fixtures that create and clean themselves. The scenarios: dispatch, parallel-io, parallel-cpu, mixed, parse-scale, extract-json, feed-parse, code-map and memory. Because nothing touches the network and nothing calls an LLM, the suite runs in CI on every build — which is exactly how the next silent regression gets caught.',
        ru: 'Всё вышеперечисленное воспроизводится одной командой — <code>fathom bench --scenario all</code> — с фикстурами, которые создаются и очищаются сами. Сценарии: dispatch, parallel-io, parallel-cpu, mixed, parse-scale, extract-json, feed-parse, code-map и memory. Поскольку ничто не касается сети и ничто не вызывает LLM, набор выполняется в CI на каждой сборке — именно так ловится следующая тихая регрессия.',
      },
      12: {
        en: 'And because synthetic numbers are only half the story, <code>fathom stats</code> reads the SQLite tracing of a real session and reports per-tool p50/p95 durations and the batching coefficient from production runs.',
        ru: 'А поскольку синтетические цифры — лишь половина истории, <code>fathom stats</code> читает SQLite-трассировку реальной сессии и сообщает длительности p50/p95 по каждому инструменту и коэффициент батчинга из продакшн-запусков.',
      },
    },
    t2: {
      col0: { en: 'Version', ru: 'Версия' },
      col1: { en: 'Wall time', ru: 'Реальное время' },
      r0: { en: 'Before (regex per file + sequential reads)', ru: 'До (regex на файл + последовательные чтения)' },
      r1: { en: 'After (OnceLock cache + spawned reads)', ru: 'После (кэш OnceLock + распараллеленные чтения)' },
    },
    t3: {
      col0: { en: 'Batch', ru: 'Батч' },
      col1: { en: 'Spawn, before', ru: 'Spawn, до' },
      col2: { en: 'Spawn, after', ru: 'Spawn, после' },
    },
    quote: {
      en: 'Every new read-only tool must ship with a classification test — otherwise parallelism disappears without a trace.<cite>— Fathom, benchmark notes</cite>',
      ru: 'Каждый новый read-only инструмент должен поставляться с тестом классификации — иначе параллелизм бесследно исчезает.<cite>— Fathom, заметки о бенчмарках</cite>',
    },
    related: {
      title: { en: 'Related Posts', ru: 'Похожие статьи' },
      0: { en: 'A memory that remembers: append-only knowledge for research agents', ru: 'Память, которая помнит: append-only знания для исследовательских агентов' },
      1: { en: 'Anatomy of a fan-out: one question, a fleet of agents', ru: 'Анатомия fan-out: один вопрос — флот агентов' },
    },
  },
  postVerify: {
    hero: {
      date: { en: 'August 10, 2026', ru: '10 августа 2026' },
      readTime: { en: '7 min read', ru: '7 мин чтения' },
      tag: { en: 'Verification', ru: 'Верификация' },
      title: { en: 'Five gates: how we verify every email before it hits the CRM', ru: 'Пять шлюзов: как мы проверяем каждый email до попадания в CRM' },
      intro: { en: 'Most lead-gen tools treat email verification as a checkbox. We built a five-stage pipeline with an append-only receipt ledger so you can prove — not just claim — that a contact was verified.', ru: 'Большинство инструментов для генерации лидов рассматривают верификацию email как галочку. Мы построили пятиэтапный конвейер с append-only журналом чеков, чтобы вы могли доказать — а не просто заявить — что контакт был верифицирован.' },
    },
    highlights: {
      title: { en: 'Highlights', ru: 'Основное' },
      0: { en: '<code>verify_email</code> runs five sequential gates: syntax, MX, disposable, role-based, SMTP probe', ru: '<code>verify_email</code> выполняет пять последовательных шлюзов: синтаксис, MX, disposable, role-based, SMTP-зонд' },
      1: { en: 'SMTP probe connects to port 25 and runs HELO → MAIL FROM → RCPT TO — never sends message content', ru: 'SMTP-зонд подключается к порту 25 и выполняет HELO → MAIL FROM → RCPT TO — никогда не отправляет содержимое сообщения' },
      2: { en: 'Append-only verification receipt ledger keyed by (kind, value) — one green can never silence a red', ru: 'Append-only журнал чеков верификации с ключом (тип, значение) — одно зелёное никогда не заглушит красное' },
      3: { en: 'Three verification levels: <code>Verified</code> (SMTP accepted), <code>Partial</code> (syntax+MX ok), <code>Unverified</code>', ru: 'Три уровня верификации: <code>Verified</code> (SMTP принял), <code>Partial</code> (синтаксис+MX ок), <code>Unverified</code>' },
    },
    s1: { title: { en: 'Why five gates?', ru: 'Пять шлюзов — почему?' }, p1: { en: 'A single regex check tells you nothing. An email can pass syntax validation, resolve MX records, and still bounce — because the mailbox doesn\'t exist, the domain is catch-all, or the address is a department alias that never reads cold outreach. Each gate eliminates a different class of false positives.', ru: 'Одна проверка регулярным выражением ничего не говорит. Email может пройти валидацию синтаксиса, разрешить MX-записи и всё равно вернуться — потому что почтовый ящик не существует, домен catch-all или адрес является алиасом отдела, который никогда не читает холодные письма. Каждый шлюз устраняет свой класс ложных срабатываний.' } },
    s2: { title: { en: 'The SMTP handshake', ru: 'SMTP-рукопожатие' }, p1: { en: 'The optional fifth gate connects to the best MX host on port 25 with a 10-second timeout. The dialogue: read the banner (220), say HELO, issue MAIL FROM, then RCPT TO — the actual deliverability signal. The server responds 250 (accepted) or 5xx (rejected). We never send message content — just the envelope. A 4xx response means greylisting, which we mark as inconclusive rather than false-negative.', ru: 'Опциональный пятый шлюз подключается к лучшему MX-хосту на порт 25 с 10-секундным таймаутом. Диалог: читаем баннер (220), говорим HELO, отправляем MAIL FROM, затем RCPT TO — реальный сигнал доставимости. Сервер отвечает 250 (принято) или 5xx (отклонено). Мы никогда не отправляем содержимое сообщения — только конверт. Ответ 4xx означает greylisting, который мы помечаем как inconclusive, а не false-negative.' } },
    s3: { title: { en: 'The receipt ledger', ru: 'Журнал чеков' }, p1: { en: 'Every check writes a typed receipt to an append-only JSONL file. The key is (kind, value) — so a PASS on email_smtp can never mask a FAIL on email_domain_mx. When save_contacts runs, it consults the ledger: SMTP accepted? → Verified. Syntax+MX ok? → Partial. Otherwise → Unverified. This is how the system produces traceable status labels.', ru: 'Каждая проверка записывает типизированный чек в append-only JSONL-файл. Ключ — (тип, значение) — поэтому PASS по email_smtp никогда не замаскирует FAIL по email_domain_mx. Когда save_contacts запускается, он обращается к журналу: SMTP принял? → Verified. Синтаксис+MX ок? → Partial. Иначе → Unverified. Так мы гарантируем честные метки статуса.' } },
    s4: { title: { en: 'Auto-save: the safety net', ru: 'Автосейв: страховочная сетка' }, p1: { en: 'The LLM doesn\'t always remember to call save_contacts. So after every extract_contacts or find_leads call, the runtime automatically persists harvested contacts — with the same dedup and receipt-gated verification. Contacts reach the database even if the model gets distracted, context gets compacted, or the session crashes.', ru: 'LLM не всегда помнит вызвать save_contacts. Поэтому после каждого вызова extract_contacts или find_leads runtime автоматически сохраняет собранные контакты — с той же дедупликацией и верификацией через журнал чеков. Контакты попадают в базу данных, даже если модель отвлеклась, контекст сжат или сессия упала.' } },
    related: {
      title: { en: 'Related Posts', ru: 'Похожие статьи' },
      0: { en: 'A memory that remembers: append-only knowledge for research agents', ru: 'Память, которая помнит: append-only знания для исследовательских агентов' },
      1: { en: 'How benchmarks caught a 31x slowdown before it shipped', ru: 'Как бенчмарки поймали 31x замедление до релиза' },
    },
  },
};
