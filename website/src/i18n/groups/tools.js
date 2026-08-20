// Auto-imported by switcher.js via import.meta.glob('./groups/*.js').
// Each export key nests under the top-level `tools` namespace; pages set
// data-i18n="tools.<...>" attributes that resolve {en,ru}.
export default {
  tools: {
    hero: {
      badge: {
        en: 'Tools',
        ru: 'Инструменты',
      },
      title1: {
        en: '51 built-in tools',
        ru: '51 встроенный инструмент',
      },
      titleSub: {
        en: '(+5 browser via CDP)',
        ru: '(+5 браузерных через CDP)',
      },
      sub: {
        en: 'Every tool implements a single Tool trait and runs in a parallel-safe registry. Read-only tools execute concurrently; writes serialize automatically.',
        ru: 'Каждый инструмент реализует единый трейт Tool и работает в параллельно-безопасном реестре. Read-only инструменты исполняются конкурентно; записи сериализуются автоматически.',
      },
    },
    // Tools categories are rendered from the `categories` array in
    // tools.astro; each index maps to these keys via data-i18n={'tools.cat.'+i+'.name'}.
    cat: {
      web: { name: { en: 'Web', ru: 'Веб' }, desc: { en: 'Multi-source search and fetching across 7 backends.', ru: 'Мульти-источниковый поиск и загрузка через 7 бэкендов.' } },
      parsing: { name: { en: 'Parsing', ru: 'Парсинг' }, desc: { en: 'HTML and JSON extraction from any document.', ru: 'Извлечение HTML и JSON из любого документа.' } },
      code: { name: { en: 'Code', ru: 'Код' }, desc: { en: 'Map codebases and extract symbols.', ru: 'Картирование кодовых баз и извлечение символов.' } },
      files: { name: { en: 'Files', ru: 'Файлы' }, desc: { en: 'Read, write, edit, search files.', ru: 'Чтение, запись, редактирование, поиск файлов.' } },
      exec: { name: { en: 'Execution', ru: 'Исполнение' }, desc: { en: 'Run shell and scripting languages safely.', ru: 'Безопасный запуск shell и скриптовых языков.' } },
      browser: { name: { en: 'Browser (CDP)', ru: 'Браузер (CDP)' }, desc: { en: 'Drive a real Chrome via DevTools Protocol.', ru: 'Управление реальным Chrome через DevTools Protocol.' } },
      vision: { name: { en: 'Vision', ru: 'Vision' }, desc: { en: 'Analyze images with a vision model.', ru: 'Анализ изображений vision-моделью.' } },
      git: { name: { en: 'Git', ru: 'Git' }, desc: { en: 'Status, diffs, and safe commits/pushes.', ru: 'Статус, диффы и безопасные коммиты/пуши.' } },
      pdf: { name: { en: 'PDF', ru: 'PDF' }, desc: { en: 'Extract text and structure from PDFs.', ru: 'Извлечение текста и структуры из PDF.' } },
      osint: { name: { en: 'OSINT / Lead Gen', ru: 'OSINT / Лиды' }, desc: { en: 'Find contacts, companies, and enrichment data.', ru: 'Поиск контактов, компаний и данных для обогащения.' } },
      verify: { name: { en: 'Verification', ru: 'Верификация' }, desc: { en: 'Validate emails, phones, and social profiles.', ru: 'Проверка email, телефонов и соцпрофилей.' } },
      enrich: { name: { en: 'Enrichment', ru: 'Обогащение' }, desc: { en: 'Add structured context to companies and people.', ru: 'Добавление структурированного контекста по компаниям и людям.' } },
      meta: { name: { en: 'Meta', ru: 'Мета' }, desc: { en: 'Spawn children, inspect memory.', ru: 'Запуск детей, просмотр памяти.' } },
      memory: { name: { en: 'Semantic Memory', ru: 'Семантическая память' }, desc: { en: 'Long-term knowledge base + entity graph.', ru: 'Долгосрочная база знаний + граф сущностей.' } },
      control: { name: { en: 'Control plane', ru: 'Control plane' }, desc: { en: 'Operator interaction and session tools.', ru: 'Взаимодействие с оператором и сессионные инструменты.' } },
    },
    mcp: {
      title: { en: 'Plus MCP', ru: 'Плюс MCP' },
      sub: {
        en: 'All 51 built-ins + 5 conditional browser tools are exposed to external MCP clients via mcp-serve, and external MCP servers over stdio or HTTP are auto-discovered and made available to agents.',
        ru: 'Все 51 встроенный инструмент и 5 браузерных инструментов при доступном CDP доступны внешним MCP-клиентам через mcp-serve, а внешние MCP-серверы по stdio или HTTP автоматически обнаруживаются и становятся доступны агентам.',
      },
      docs: { en: 'Full tool docs', ru: 'Полная документация инструментов' },
      mcp: { en: 'MCP both ways', ru: 'MCP в обе стороны' },
    },
    parallel: {
      title: { en: 'Fathom-safe by construction', ru: 'Fathom-безопасен по построению' },
      sub: {
        en: 'Each tool declares whether it is parallel-safe. Read-only tools run concurrently via execute_batch_spawn; writes serialize with path-overlap detection. That is how CPU batches hit 3.78×.',
        ru: 'Каждый инструмент объявляет, Fathom-безопасен ли он. Read-only инструменты работают конкурентно через execute_batch_spawn; записи сериализуются с детекцией перекрытия путей. Так CPU-батчи достигают 3.78×.',
      },
      numbers: { en: 'See the numbers', ru: 'Смотреть цифры' },
    },
  },
};
