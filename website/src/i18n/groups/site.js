// Auto-imported by switcher.js via import.meta.glob('./groups/*.js').
// Per-prefix translation blocks for install, pricing, changelog, blog, 404,
// and the blog/benchmarks-repo-map post. Each key resolves {en,ru}.
export default {
  install: {
    hero: {
      badge: { en: 'Install', ru: 'Установка' },
      title: { en: 'One binary. Every runtime.', ru: 'Один бинарник. Любой рантайм.' },
      sub: {
        en: 'CLI, TUI, HTTP server, web dashboard and MCP endpoint ship in a single binary. Install with one command, build from source with Cargo, or run it in Docker — pick your path.',
        ru: 'CLI, TUI, HTTP-сервер, веб-дашборд и MCP-эндпоинт поставляются в одном бинарнике. Установите одной командой, соберите из исходников через Cargo или запустите в Docker — выбирайте свой путь.',
      },
    },
    api: {
      0: { d: { en: 'One-line installer. Builds the release binary, installs it to /usr/local/bin (or $PREFIX/bin), creates the default config directory — and optionally installs the systemd service unit with INSTALL_SYSTEMD=1.', ru: 'Установщик в одну строку. Собирает релизный бинарник, устанавливает его в /usr/local/bin (или $PREFIX/bin), создаёт каталог конфигурации по умолчанию — и по желанию устанавливает systemd-юнит с INSTALL_SYSTEMD=1.' } },
      1: { d: { en: 'Build from source with the Rust toolchain. Release builds are compiled with LTO and stripped — a slim, self-contained binary.', ru: 'Сборка из исходников с Rust-инструментарий. Релизные сборки компилируются с LTO и stripped — компактный автономный бинарник.' } },
      2: { d: { en: 'Multi-stage image: rust:1.97-bookworm builder → debian:bookworm-slim runtime. Runs as a non-root user; output and databases live in /data.', ru: 'Многоступенчатый образ: builder rust:1.97-bookworm → рантайм debian:bookworm-slim. Запускается от non-root пользователя; выходные данные и базы живут в /data.' } },
      3: { d: { en: 'Publishes port 8080 with restart policy and named volumes: research-data for output, contacts and memory databases, research-config for config.toml.', ru: 'Публикует порт 8080 с политикой перезапуска и именованными томами: research-data для выходных данных, баз контактов и памяти, research-config для config.toml.' } },
    },
    systemd: {
      badge: { en: 'Linux service', ru: 'Linux-сервис' },
      title: { en: 'Run it under systemd', ru: 'Запуск под systemd' },
      sub: {
        en: 'The unit starts <code class="ic">fathom serve</code> on port 8080 with <code class="ic">Restart=on-failure</code> and a hardening sandbox: <code class="ic">NoNewPrivileges</code>, <code class="ic">PrivateTmp</code>, <code class="ic">ProtectSystem=full</code>, <code class="ic">ProtectHome=read-only</code> — writes are restricted to <code class="ic">/var/lib/fathom</code>.',
        ru: 'Модуль запускает <code class="ic">fathom serve</code> на порту 8080 с <code class="ic">Restart=on-failure</code> и усиленной изоляцией: <code class="ic">NoNewPrivileges</code>, <code class="ic">PrivateTmp</code>, <code class="ic">ProtectSystem=full</code>, <code class="ic">ProtectHome=read-only</code> — запись ограничена каталогом <code class="ic">/var/lib/fathom</code>.',
      },
    },
    deps: {
      badge: { en: 'Optional dependencies', ru: 'Опциональные зависимости' },
      title: { en: 'Nothing required, everything upgradeable', ru: 'Ничего обязательного, всё расширяемо' },
      sub: {
        en: 'The binary works standalone. These tools unlock extra capabilities — each one is detected automatically at runtime, and every absence has a graceful fallback.',
        ru: 'Бинарник работает автономно. Эти инструменты открывают дополнительные возможности — каждый обнаруживается автоматически во время выполнения, и их отсутствие всегда имеет корректный запасной вариант.',
      },
      colTool: { en: 'Tool', ru: 'Инструмент' },
      colEnables: { en: 'What it enables', ru: 'Что даёт' },
      0: { enables: { en: 'PDF and DOCX report export (HTML / JSON export always works)', ru: 'Экспорт отчётов в PDF и DOCX (экспорт HTML / JSON работает всегда)' } },
      1: { enables: { en: 'speeds up the grep tool; built-in fallback when absent', ru: 'ускоряет инструмент grep; встроенный запасной вариант при отсутствии' } },
      2: { enables: { en: 'REPL and code-execution tools', ru: 'инструменты REPL и выполнения кода' } },
      3: { enables: { en: 'Up to 5 browser tools; auto-detected via PARALLEL_CDP_ENDPOINT, plus up to 6 computer tools via COMPUTER_URL', ru: 'До 5 браузерных через PARALLEL_CDP_ENDPOINT и до 6 компьютерных через COMPUTER_URL' } },
    },
    req: {
      badge: { en: 'Requirements', ru: 'Требования' },
      title: { en: 'Small footprint, no surprises', ru: 'Малый след, без сюрпризов' },
    },
    env: {
      badge: { en: 'Environment', ru: 'Окружение' },
      title: { en: 'Env vars reference', ru: 'Справочник env-переменных' },
      sub: {
        en: 'Environment variables override config-file values. API keys for external services are only ever read from the environment or your local config.',
        ru: 'Переменные окружения переопределяют значения из конфигурационного файла. API-ключи внешних сервисов всегда читаются только из окружения или локальной конфигурации.',
      },
      colVar: { en: 'Variable', ru: 'Переменная' },
      colPurpose: { en: 'Purpose', ru: 'Назначение' },
      0: { purpose: { en: 'Comma-separated keys protecting /api/v1/*. Required when serve binds to a non-loopback address.', ru: 'Ключи через запятую, защищающие /api/v1/*. Требуется, когда serve привязан к не-loopback адресу.' } },
      1: { purpose: { en: 'Per-client HTTP API requests per minute (default: 120).', ru: 'HTTP API-запросов на клиента в минуту (по умолчанию: 120).' } },
      2: { purpose: { en: 'Chrome DevTools Protocol endpoint for the browser tools (default: localhost:9222).', ru: 'Эндпоинт Chrome DevTools Protocol для браузерных инструментов (по умолчанию: localhost:9222).' } },
      3: { purpose: { en: 'API key for the vision model used on screenshots and images.', ru: 'API-ключ для vision-модели, используемой на скриншотах и изображениях.' } },
      4: { purpose: { en: 'Vision model identifier.', ru: 'Идентификатор vision-модели.' } },
      5: { purpose: { en: '2GIS Catalog API key for business-directory search.', ru: 'API-ключ 2GIS Catalog для поиска в деловом каталоге.' } },
      6: { purpose: { en: 'Google Places API key for business-directory search.', ru: 'API-ключ Google Places для поиска в деловом каталоге.' } },
      7: { purpose: { en: 'Yandex Maps API key for business-directory search.', ru: 'API-ключ Yandex Maps для поиска в деловом каталоге.' } },
      8: { purpose: { en: 'X (Twitter) bearer token for the social-search tool.', ru: 'Bearer-токен X (Twitter) для инструмента поиска по соцсетям.' } },
      9: { purpose: { en: 'Config file path override (default: ~/.fathom/config.toml).', ru: 'Переопределение пути к конфигурационному файлу (по умолчанию: ~/.fathom/config.toml).' } },
      10: { purpose: { en: 'Semantic memory SQLite database path override.', ru: 'Переопределение пути к SQLite-базе семантической памяти.' } },
      11: { purpose: { en: 'Durable jobs SQLite database path override.', ru: 'Переопределение пути к SQLite-базе устойчивых задач.' } },
      12: { purpose: { en: 'Log level filter (default: info).', ru: 'Фильтр уровня логов (по умолчанию: info).' } },
    },
    verify: {
      badge: { en: 'Verify', ru: 'Проверка' },
      title: { en: 'Check the installation', ru: 'Проверьте установку' },
      sub: {
        en: '<code class="ic">bench</code> exercises the tool-execution layer with zero network and zero LLM calls — a safe smoke test on any machine. Then start the server or the TUI.',
        ru: '<code class="ic">bench</code> проверяет слой выполнения инструментов без сети и без обращений к LLM — безопасный smoke-тест на любой машине. Затем запустите сервер или TUI.',
      },
      bench: { en: 'Benchmarks', ru: 'Бенчмарки' },
      cli: { en: 'CLI reference', ru: 'Справочник CLI' },
    },
    cta: {
      title: { en: 'Installed? Run your first research.', ru: 'Установили? Запустите первое исследование.' },
      sub: { en: 'Configure the LLM key, pick a search backend and launch a session in under five minutes.', ru: 'Настройте LLM-ключ, выберите поисковый бэкенд и запустите сессию меньше чем за пять минут.' },
      next: { en: 'Next: quickstart →', ru: 'Далее: быстрый старт →' },
      cli: { en: 'CLI reference', ru: 'Справочник CLI' },
    },
  },
  pricing: {
    hero: {
      badge: { en: 'Pricing', ru: 'Цены' },
      title: { en: 'Distributed individually.', ru: 'Распространяется индивидуально.' },
      sub: { en: 'every deployment ships the full binary — 51 always-registered tools + up to 5 CDP browser tools + up to 6 computer tools, 12 workspace crates, no usage metering', ru: 'каждый деплой поставляет полный бинарник — 51 всегда зарегистрированный + до 5 CDP-браузерных + до 6 компьютерных, 12 крейтов, без учёта использования' },
    },
    requestAccess: { en: 'Request access', ru: 'Запросить доступ' },
    tier: {
      price: { en: 'Custom <span>per deployment</span>', ru: 'Индивидуально <span>за деплой</span>' },
      0: {
        name: { en: 'Starter', ru: 'Starter' },
        desc: { en: 'For the individual researcher who wants the whole toolbox on one machine.', ru: 'Для индивидуального исследователя, которому нужен весь набор инструментов на одной машине.' },
        feat: {
          0: { en: '<kbd>Seats</kbd> 1', ru: '<kbd>Места</kbd> 1' },
          1: { en: '<kbd>Tools</kbd> 51 always + up to 5 CDP + up to 6 computer, no metering', ru: '<kbd>Инструменты</kbd> 51 всегда + до 5 CDP + до 6 компьютерных, без учёта' },
          2: { en: '<kbd>Search</kbd> 7 backends, smart fusion', ru: '<kbd>Поиск</kbd> 7 бэкендов, умное слияние' },
          3: { en: '<kbd>Memory</kbd> Local SQLite + embeddings', ru: '<kbd>Память</kbd> Локальный SQLite + эмбеддинги' },
          4: { en: '<kbd>Support</kbd> Community', ru: '<kbd>Поддержка</kbd> Сообщество' },
        },
      },
      1: {
        name: { en: 'Team', ru: 'Team' },
        desc: { en: 'Shared contact database, CRM sync and API access for a small research team.', ru: 'Общая база контактов, синхронизация с CRM и API-доступ для небольшой исследовательской команды.' },
        feat: {
          0: { en: '<kbd>Seats</kbd> Up to 10', ru: '<kbd>Места</kbd> До 10' },
          1: { en: '<kbd>Contacts</kbd> PostgreSQL ContactDb', ru: '<kbd>Контакты</kbd> PostgreSQL ContactDb' },
          2: { en: '<kbd>CRM</kbd> amoCRM · Bitrix24 · HubSpot', ru: '<kbd>CRM</kbd> amoCRM · Bitrix24 · HubSpot' },
          3: { en: '<kbd>Access</kbd> HTTP API + dashboard', ru: '<kbd>Доступ</kbd> HTTP API + дашборд' },
          4: { en: '<kbd>Support</kbd> Priority', ru: '<kbd>Поддержка</kbd> Приоритетная' },
          5: { en: '<kbd>SLA</kbd> 8×5', ru: '<kbd>SLA</kbd> 8×5' },
        },
      },
      2: {
        name: { en: 'Enterprise', ru: 'Enterprise' },
        desc: { en: 'On-prem isolation, compliance and extensions for regulated workloads.', ru: 'Изоляция on-prem, соответствие требованиям и расширения для регулируемых нагрузок.' },
        feat: {
          0: { en: '<kbd>Seats</kbd> Custom', ru: '<kbd>Места</kbd> Индивидуально' },
          1: { en: '<kbd>Deploy</kbd> On-prem · multi-process isolation', ru: '<kbd>Деплой</kbd> On-prem · изоляция процессов' },
          2: { en: '<kbd>Compliance</kbd> ZDR · SSO · DPA', ru: '<kbd>Соответствие</kbd> ZDR · SSO · DPA' },
          3: { en: '<kbd>Extend</kbd> Custom tools + MCP servers', ru: '<kbd>Расширение</kbd> Пользовательские инструменты + MCP-серверы' },
          4: { en: '<kbd>Support</kbd> Dedicated engineer', ru: '<kbd>Поддержка</kbd> Выделенный инженер' },
          5: { en: '<kbd>SLA</kbd> 24×7', ru: '<kbd>SLA</kbd> 24×7' },
        },
      },
    },
    enterprise: {
      title: { en: 'Air-gapped deployment', ru: 'Изолированное (air-gapped) развёртывание' },
      sub: {
        en: 'Run the full pipeline with zero external calls: local models over an OpenAI-compatible endpoint, offline TF-IDF embeddings, and contacts that never leave your perimeter.',
        ru: 'Запустите весь конвейер без внешних обращений: локальные модели через OpenAI-совместимый эндпоинт, офлайн TF-IDF эмбеддинги и контакты, которые никогда не покидают ваш периметр.',
      },
      chip: {
        0: { en: 'Zero Data Retention', ru: 'Zero Data Retention' },
        1: { en: 'SSO / SAML', ru: 'SSO / SAML' },
        2: { en: 'DPA', ru: 'DPA' },
        3: { en: 'On-prem', ru: 'On-prem' },
        4: { en: 'Custom models', ru: 'Пользовательские модели' },
      },
      cta: { en: 'Talk to us →', ru: 'Свяжитесь с нами →' },
    },
    compare: {
      badge: { en: 'Compare', ru: 'Сравнение' },
      title: { en: 'Same binary, different scale', ru: 'Тот же бинарник, другой масштаб' },
      sub: {
        en: 'Every tier ships all 51 always-registered tools + up to 5 CDP browser tools + up to 6 computer tools — plans differ in seats, storage, integration depth and support, never in capability.',
        ru: 'Каждый тариф поставляет 51 всегда зарегистрированный инструмент, до 5 CDP-браузерных и до 6 компьютерных при настройке — планы различаются местами, хранилищем, глубиной интеграции и поддержкой, но не функциональностью.',
      },
      tab: {
        tools: { en: 'Tools', ru: 'Инструменты' },
        memory: { en: 'Memory', ru: 'Память' },
        ops: { en: 'Ops', ru: 'Ops' },
        support: { en: 'Support', ru: 'Поддержка' },
      },
    },
    faq: {
      badge: { en: 'FAQ', ru: 'FAQ' },
      title: { en: 'Questions, answered', ru: 'Вопросы и ответы' },
      0: {
        q: { en: 'How is it distributed?', ru: 'Как это распространяется?' },
        a: {
          en: 'Fathom Research is a closed product distributed individually — no public package registries or app stores. You receive a single static binary containing the CLI, TUI, HTTP server, dashboard, MCP server and worker processes. Every deployment is complete and identical: 51 always-registered tools + up to 5 CDP browser tools + up to 6 computer tools, 12 workspace crates, no usage metering.',
          ru: 'Fathom Research — закрытый продукт, распространяемый индивидуально — без публичных реестров пакетов и app stores. Вы получаете единый статический бинарник, содержащий CLI, TUI, HTTP-сервер, дашборд, MCP-сервер и воркер-процессы. Каждый деплой полный и идентичный: 51 всегда зарегистрированный + до 5 CDP-браузерных + до 6 компьютерных, 12 крейтов, без учёта использования.',
        },
      },
      1: {
        q: { en: 'Do you store my data?', ru: 'Вы храните мои данные?' },
        a: {
          en: 'No. Everything — contacts, memory, jobs, reports — lives on your own infrastructure: local SQLite or your PostgreSQL. Nothing leaves your machines unless you push it to your own CRM, and there is no telemetry on your data.',
          ru: 'Нет. Всё — контакты, память, задачи, отчёты — живёт на вашей инфраструктуре: локальный SQLite или ваш PostgreSQL. Ничто не покидает ваши машины, если вы сами не выгрузите это в свой CRM, и телеметрии по вашим данным нет.',
        },
      },
      2: {
        q: { en: 'Which LLM providers are supported?', ru: 'Какие LLM-провайдеры поддерживаются?' },
        a: {
          en: 'Any OpenAI-compatible endpoint: seven providers work out of the box, and you can plug in self-hosted or custom models. Per-role model routing lets each agent persona run on its own model.',
          ru: 'Любой OpenAI-совместимый эндпоинт: семь провайдеров работают из коробки, и вы можете подключить self-hosted или пользовательские модели. Маршрутизация моделей по ролям позволяет каждой персоне агента работать на своей модели.',
        },
      },
      3: {
        q: { en: 'Can agents run air-gapped?', ru: 'Могут ли агенты работать офлайн (air-gapped)?' },
        a: {
          en: 'Yes. Offline TF-IDF embeddings (512 dimensions, no network), local models behind an OpenAI-compatible endpoint and on-device processing let the full pipeline run without any external calls.',
          ru: 'Да. Офлайн TF-IDF эмбеддинги (512 измерений, без сети), локальные модели за OpenAI-совместимым эндпоинтом и обработка на устройстве позволяют всему конвейеру работать без внешних обращений.',
        },
      },
    },
    cta: {
      title: { en: 'Request your deployment', ru: 'Запросите свой деплой' },
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
        en: 'How Fathom Research actually works — benchmarks, memory design, agent orchestration. Written by the people who compile it.',
        ru: 'Как на самом деле работает Fathom Research — бенчмарки, дизайн памяти, оркестрация агентов. Написано теми, кто его собирает.',
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
        en: 'Fathom Research ships its own benchmark harness: <code>fathom bench</code>. It is not a showcase of flattering numbers — it is an instrument the tool layer runs against itself. In its short life it has already caught two real regressions: a 31× slowdown hiding in the repository-map builder, and four tools that had silently fallen back to one-at-a-time execution.',
        ru: 'Fathom Research поставляет собственный бенчмарк-стенд: <code>fathom bench</code>. Это не витрина лестных цифр — это инструмент, на котором слой инструментов тестирует сам себя. За короткую жизнь он уже поймал две реальные регрессии: замедление в 31×, скрытое в построителе карты репозитория, и четыре инструмента, незаметно перешедших на последовательное выполнение по одному.',
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
      en: 'Every new read-only tool must ship with a classification test — otherwise parallelism disappears without a trace.<cite>— Fathom Research, benchmark notes</cite>',
      ru: 'Каждый новый read-only инструмент должен поставляться с тестом классификации — иначе параллелизм бесследно исчезает.<cite>— Fathom Research, заметки о бенчмарках</cite>',
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
    s3: { title: { en: 'The receipt ledger', ru: 'Журнал чеков' }, p1: { en: 'Every check writes a typed receipt to an append-only JSONL file. The key is (kind, value) — so a PASS on email_smtp can never mask a FAIL on email_domain_mx. When save_contacts runs, it consults the ledger: SMTP accepted? → Verified. Syntax+MX ok? → Partial. Otherwise → Unverified. This is how we guarantee honest status labels.', ru: 'Каждая проверка записывает типизированный чек в append-only JSONL-файл. Ключ — (тип, значение) — поэтому PASS по email_smtp никогда не замаскирует FAIL по email_domain_mx. Когда save_contacts запускается, он обращается к журналу: SMTP принял? → Verified. Синтаксис+MX ок? → Partial. Иначе → Unverified. Так мы гарантируем честные метки статуса.' } },
    s4: { title: { en: 'Auto-save: the safety net', ru: 'Автосейв: страховочная сетка' }, p1: { en: 'The LLM doesn\'t always remember to call save_contacts. So after every extract_contacts or find_leads call, the runtime automatically persists harvested contacts — with the same dedup and receipt-gated verification. Contacts reach the database even if the model gets distracted, context gets compacted, or the session crashes.', ru: 'LLM не всегда помнит вызвать save_contacts. Поэтому после каждого вызова extract_contacts или find_leads runtime автоматически сохраняет собранные контакты — с той же дедупликацией и верификацией через журнал чеков. Контакты попадают в базу данных, даже если модель отвлеклась, контекст сжат или сессия упала.' } },
    related: {
      title: { en: 'Related Posts', ru: 'Похожие статьи' },
      0: { en: 'A memory that remembers: append-only knowledge for research agents', ru: 'Память, которая помнит: append-only знания для исследовательских агентов' },
      1: { en: 'How benchmarks caught a 31x slowdown before it shipped', ru: 'Как бенчмарки поймали 31x замедление до релиза' },
    },
  },
};
