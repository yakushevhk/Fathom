// Playground page translations (playground.astro). Merged into the global
// dictionary by scripts/i18n-generate.mjs at build time.
export default {
  nav: {
    playground: { en: 'Playground', ru: 'Плейграунд' },
  },
  playground: {
    hero: {
      badge: { en: 'Playground', ru: 'Плейграунд' },
      title: { en: 'Deep research, live.', ru: 'Глубокий исследование вживую.' },
      sub: {
        en: 'Launch a research session, watch the agent fleet work in real time, steer it mid-run, browse long-term memory, manage background jobs and inspect live metrics — the whole platform in one console.',
        ru: 'Запустите исследовательскую сессию, наблюдайте за флотом агентов в реальном времени, корректируйте курс на лету, работайте с долгосрочной памятью, управляйте фоновыми задачами и смотрите живые метрики — вся платформа в одной консоли.',
      },
    },
    tabs: {
      research: { en: 'Research', ru: 'Ресёрч' },
      fleet: { en: 'Fleet', ru: 'Флот' },
      memory: { en: 'Memory', ru: 'Память' },
      jobs: { en: 'Jobs', ru: 'Задачи' },
      metrics: { en: 'Metrics', ru: 'Метрики' },
    },
    fleet: {
      title: { en: 'All agents', ru: 'Все агенты' },
    },
    memory: {
      search: { en: 'Search memories', ru: 'Поиск по памяти' },
      searchPh: {
        en: 'Semantic query (hybrid search)…',
        ru: 'Семантический запрос (гибридный поиск)…',
      },
      listAll: { en: 'List all', ru: 'Показать все' },
      absorb: { en: 'Absorb facts', ru: 'Поглотить факты' },
      source: { en: 'Source', ru: 'Источник' },
      scope: { en: 'Scope', ru: 'Область' },
      facts: { en: 'Facts (one per line)', ru: 'Факты (по одному в строке)' },
      maint: { en: 'Maintenance', ru: 'Обслуживание' },
    },
    jobs: {
      title: { en: 'Background jobs', ru: 'Фоновые задачи' },
      new: { en: 'New job', ru: 'Новая задача' },
      taskPh: { en: 'Task description…', ru: 'Описание задачи…' },
      create: { en: 'Create', ru: 'Создать' },
      log: { en: 'Job log', ru: 'Лог задачи' },
    },
    metrics: {
      title: { en: 'Server metrics', ru: 'Метрики сервера' },
      raw: { en: 'Raw (Prometheus)', ru: 'Сырые (Prometheus)' },
    },
    conn: {
      connecting: { en: 'connecting…', ru: 'подключение…' },
      settings: { en: 'Settings', ru: 'Настройки' },
      url: { en: 'Backend URL', ru: 'URL бекенда' },
      key: { en: 'API key', ru: 'API-ключ' },
      hint: {
        en: 'Start the backend with: fathom serve --port 8080 (an API key enables cross-origin access).',
        ru: 'Запустите бекенд: fathom serve --port 8080 (API-ключ включает кросс-доменный доступ).',
      },
      save: { en: 'Save & reconnect', ru: 'Сохранить и переподключить' },
    },
    composer: {
      placeholder: {
        en: 'What should we research? e.g. “Map the competitive landscape of AI research agents — pricing, features, traction”',
        ru: 'Что исследуем? Например: «Карта конкурентов среди AI research-агентов — цены, функции, traction»',
      },
      run: { en: 'Run research', ru: 'Запустить исследование' },
    },
    sessions: {
      title: { en: 'Sessions', ru: 'Сессии' },
      empty: {
        en: 'No sessions yet — run your first research above.',
        ru: 'Сессий пока нет — запустите первый исследование выше.',
      },
    },
    status: {
      agents: { en: 'agents', ru: 'агенты' },
      tokens: { en: 'tokens', ru: 'токены' },
      elapsed: { en: 'elapsed', ru: 'время' },
      cancel: { en: 'Cancel', ru: 'Отменить' },
    },
    agents: {
      title: { en: 'Agent fleet', ru: 'Флот агентов' },
      empty: {
        en: 'Agents appear here as they spawn.',
        ru: 'Агенты появляются здесь по мере запуска.',
      },
    },
    feed: {
      title: { en: 'Live activity', ru: 'Живая лента' },
      empty: { en: 'Waiting for events…', ru: 'Ожидание событий…' },
      scopeThis: { en: 'This session', ru: 'Эта сессия' },
      scopeAll: { en: 'All sessions', ru: 'Все сессии' },
    },
    agentDetail: {
      title: { en: 'Agent detail', ru: 'Детали агента' },
      session: { en: 'session', ru: 'сессия' },
      parent: { en: 'parent', ru: 'родитель' },
      created: { en: 'created', ru: 'создан' },
      completed: { en: 'completed', ru: 'завершён' },
      summary: { en: 'Summary', ru: 'Сводка' },
      close: { en: 'Close', ru: 'Закрыть' },
      noDetail: { en: 'No extra detail on disk for this agent.', ru: 'Для этого агента нет дополнительных данных на диске.' },
    },
    findings: {
      title: { en: 'Findings', ru: 'Находки' },
      sources: { en: 'sources', ru: 'источники' },
      empty: { en: 'No findings captured in this live view yet.', ru: 'Пока нет находок в этом живом просмотре.' },
    },
    report: {
      title: { en: 'Report', ru: 'Отчёт' },
      copy: { en: 'Copy markdown', ru: 'Копировать markdown' },
      download: { en: 'Download .md', ru: 'Скачать .md' },
      copied: { en: 'Copied ✓', ru: 'Скопировано ✓' },
    },
    memoryExp: {
      history: { en: 'History', ru: 'История' },
      close: { en: 'Close', ru: 'Закрыть' },
      expand: { en: 'expand', ru: 'развернуть' },
    },
    stream: {
      title: { en: 'Model stream', ru: 'Стрим модели' },
    },
    steer: {
      placeholder: {
        en: 'Steer the research — add instructions mid-run…',
        ru: 'Скорректируйте исследование — добавьте инструкции на лету…',
      },
      send: { en: 'Send', ru: 'Отправить' },
    },
    results: {
      title: { en: 'Report', ru: 'Отчёт' },
    },
    main: {
      placeholder: {
        en: 'Run a query or pick a session on the left — the live view opens here.',
        ru: 'Запустите запрос или выберите сессию слева — живой просмотр откроется здесь.',
      },
    },
    footnote: {
      en: 'The playground talks directly to your backend over /api/v1 (REST + SSE). Nothing leaves your machine.',
      ru: 'Плейграунд общается напрямую с вашим бекендом через /api/v1 (REST + SSE). Данные не покидают вашу машину.',
    },
  },
};
