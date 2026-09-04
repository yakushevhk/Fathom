export default {
  benchmarks: {
    hero: {
      badge: { en: 'Benchmarks', ru: 'Бенчмарки' },
      title: { en: 'Measured, not claimed.', ru: 'Измерено, а не обещано.' },
      sub: { en: 'Release build on macOS arm64 · 10 cores · 16 × ~2047 KB files. All scenarios are offline — no network, no LLM — so they run in CI.', ru: 'Release-сборка на macOS arm64 · 10 ядер · 16 × ~2047 KB файлов. Все сценарии офлайн — без сети и LLM — поэтому их можно гонять в CI.' },
    },
    labels: {
      seq: { en: 'sequential', ru: 'последовательно' },
      spawnLegend: { en: 'execute_batch_spawn', ru: 'execute_batch_spawn' },
      seqLegend: { en: 'sequential / join_all', ru: 'последовательно / join_all' },
    },
    th: {
      measurement: { en: 'Measurement', ru: 'Измерение' },
      iterations: { en: 'Iterations', ru: 'Итерации' },
      perCall: { en: 'Per call', ru: 'На вызов' },
      tool: { en: 'Tool', ru: 'Инструмент' },
      phase: { en: 'Phase', ru: 'Фаза' },
      success: { en: 'Success', ru: 'Успех' },
      duration: { en: 'Duration', ru: 'Длительность' },
      document: { en: 'Document', ru: 'Документ' },
      rows: { en: 'Rows', ru: 'Строки' },
      avgParse: { en: 'Avg parse', ru: 'Ср. парсинг' },
      throughput: { en: 'Throughput', ru: 'Пропускная способность' },
      query: { en: 'Query', ru: 'Запрос' },
      avg: { en: 'Avg', ru: 'Среднее' },
      feedItems: { en: 'Feed items', ru: 'Элементы фида' },
      itemsPerSec: { en: 'Items/sec', ru: 'Элементов/с' },
      mode: { en: 'Mode', ru: 'Режим' },
      wallTime: { en: 'Wall time', ru: 'Время выполнения' },
      items: { en: 'Items', ru: 'Элементы' },
      batch: { en: 'Batch', ru: 'Батч' },
      absorbed: { en: 'Absorbed', ru: 'Поглощено' },
      perFact: { en: 'Per fact', ru: 'На факт' },
      hybridSearch: { en: 'Hybrid search', ru: 'Гибридный поиск' },
      matches: { en: 'Matches', ru: 'Совпадения' },
      median: { en: 'Median', ru: 'Медиана' },
      storeSize: { en: 'Store size', ru: 'Размер хранилища' },
      fillTime: { en: 'Fill time', ru: 'Время заполнения' },
      searchMedian: { en: 'Search median', ru: 'Медиана поиска' },
      metric: { en: 'Metric', ru: 'Метрика' },
      detail: { en: 'Detail', ru: 'Детали' },
    },
    tabs: {
      dispatch: { en: 'Dispatch', ru: 'Диспетчеризация' },
      dispatch: { en: 'Dispatch', ru: 'Диспетчеризация' },
      io: { en: 'Fathom I/O', ru: 'Fathom I/O' },
      cpu: { en: 'Fathom CPU', ru: 'Fathom CPU' },
      mixed: { en: 'Mixed batch', ru: 'Смешанный батч' },
      parse: { en: 'Parse scale', ru: 'Масштабирование парсинга' },
      memory: { en: 'Memory', ru: 'Память' },
    },
    panels: {
      dispatch: {
        title: { en: 'Tool dispatch overhead', ru: 'Накладные расходы диспетчеризации' },
        sub: { en: "How much machinery sits between the LLM's tool call and the actual work.", ru: 'Сколько машинного кода разделяет вызов инструмента LLM и собственно работу.' },
        note: { en: 'Executor overhead over raw dispatch: 2473 µs per single-call batch; amortized overhead drops to 753 µs per call in an 8-call batch.', ru: 'Overhead исполнителя над сырой диспетчеризацией: 2473 мкс на батч из одного вызова; амортизированный overhead падает до 753 мкс на вызов в батче из 8.' },
      },
      io: {
        title: { en: 'Fathom vs sequential — I/O-bound', ru: 'Fathom vs последовательно — I/O-нагруженные' },
        sub: { en: '16 × file_read of distinct ~2047 KB files.', ru: '16 × file_read несовпадающих файлов по ~2047 КБ.' },
        note: { en: '16/16 calls succeeded (join_all), 16/16 (spawn); all classified parallel-safe.', ru: '16/16 вызовов успешно (join_all), 16/16 (spawn); все классифицированы как параллельно-безопасные.' },
      },
      cpu: {
        title: { en: 'Fathom vs sequential — CPU-bound', ru: 'Fathom vs последовательно — CPU-нагруженные' },
        sub: { en: '8 × parse_html of a ~1 MB table (3000 rows), selector `tr.item`, texts mode.', ru: '8 × parse_html таблицы ~1 МБ (3000 строк), селектор `tr.item`, режим texts.' },
        note: { en: 'join_all shares one thread — spawn spreads CPU work across cores.', ru: 'join_all делит один поток — spawn распределяет CPU-работу по ядрам.' },
      },
      mixed: {
        title: { en: 'Mixed batch — automatic partitioning', ru: 'Смешанный батч — автоматическое разбиение' },
        sub: { en: 'A realistic agent turn: reads (parallel-safe) + writes (sequential) in one batch. 5 ran concurrently, 3 serialized; total 70.9 ms, 8 succeeded. Order preserved.', ru: 'Реалистичный ход агента: чтения (параллельно-безопасные) + записи (последовательно) в одном батче. 5 — конкурентно, 3 — последовательно; суммарно 70.9 мс, 8 успешно. Порядок сохранён.' },
      },
      parse: {
        title: { en: 'parse_html scaling with document size', ru: 'Масштабирование parse_html с размером документа' },
        sub: { en: 'Same selector (`tr.item`, texts mode), increasing document sizes. Up to 531k rows/s.', ru: 'Тот же селектор (`tr.item`, texts-режим), растущий размер документа. До 531k строк/с.' },
      },
      json: {
        title: { en: 'extract_json throughput', ru: 'Пропускная способность extract_json' },
        sub: { en: '~4 MB JSON document with 20,000 objects. Stateless & parallel-safe (re-parsed per call).', ru: '~4 МБ JSON-документ с 20 000 объектов. Без сохранения состояния и параллельно-безопасен (пере-парсится на каждый вызов).' },
      },
      feed: {
        title: { en: 'web_feed (quick-xml) — scaling & parallelism', ru: 'web_feed (quick-xml) — масштабирование и параллелизм' },
        sub: { en: 'Local RSS fixture; tolerance to feed size and CPU-bound speed under parallelism. Up to 1.1M items/s.', ru: 'Локальный RSS-фикстур; устойчивость к размеру фида и CPU-скорости при параллелизме. До 1.1M элементов/с.' },
        note: { en: '8 × web_feed of a ~5 MB feed; 8/8 succeeded both modes.', ru: '8 × web_feed фида ~5 МБ; 8/8 успешно в обоих режимах.' },
      },
      code: {
        title: { en: 'code_symbols / repo_map — symbol extraction', ru: 'code_symbols / repo_map — извлечение символов' },
        sub: { en: '240 Rust files (~14 KB each, 40 fns + 40 structs + impls per file).', ru: '240 Rust-файлов (~14 КБ каждый, 40 функции + 40 структур + impl на файл).' },
        note: { en: '8/8 symbol scans succeeded (join_all), 8/8 (spawn).', ru: '8/8 сканирований символов успешно (join_all), 8/8 (spawn).' },
      },
      memory: {
        title: { en: 'Semantic memory — absorb / search / digest', ru: 'Семантическая память — absorb / search / digest' },
        sub: { en: 'Offline TF-IDF embedder (no network, no LLM); in-memory SQLite.', ru: 'Офлайн TF-IDF эмбеддер (без сети и LLM); in-memory SQLite.' },
        note1: { en: 'Re-absorbing 100 known facts: 93 skipped, 0 created in 5.1 ms (dedup fast path).', ru: 'Повторный absorb 100 известных фактов: 93 пропущено, 0 создано за 5.1 мс (быстрый путь дедупликации).' },
        note2: { en: 'Digest build (relevant + TODOs + recent): 4 relevant memories in 4.76 ms.', ru: 'Сборка дайджеста (релевантное + TODO + недавнее): 4 релевантных воспоминания за 4.76 мс.' },
      },
    },
    repro: {
      title: { en: 'Reproduce', ru: 'Воспроизвести' },
      note: { en: 'Benchmarks use no network and no LLM (offline TF-IDF embedder) — runnable in CI. Numbers above are a snapshot on macOS arm64 (10 cores), release build.', ru: 'Бенчмарки не используют сеть и LLM (офлайн TF-IDF эмбеддер) — их можно гонять в CI. Цифры выше — снимок на macOS arm64 (10 ядер), release-сборка.' },
    },
    quality: {
      title: { en: "Quality & Testing", ru: "Качество и тестирование" },
    },

  fieldnotes: {
      title: { en: 'Field notes: 31× on repo_map', ru: 'Полевые заметки: 31× на repo_map' },
      sub: {
        en: 'Benchmarks are how we catch regressions — and how we find wins. Profiling <code>repo_map</code> revealed a silently-serialized regex pass; moving it to a <code>OnceLock</code>-cached compiler took it from 902 ms to 29.5 ms (31×). The same audit unlocked 3.12× / 5.16× on web_feed and code_symbols under <code>execute_batch_spawn</code>.',
        ru: 'Бенчмарки — это как мы ловим регрессии и находим выигрыши. Профилирование <code>repo_map</code> вскрыло молча сериализованный regex-проход; перевод его на кешируемый <code>OnceLock</code>-компилятор сократил время с 902 мс до 29.5 мс (31×). Тот же аудит дал 3.12× / 5.16× на web_feed и code_symbols под <code>execute_batch_spawn</code>.',
      },
    },
  },
};
