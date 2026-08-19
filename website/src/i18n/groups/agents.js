export default {
  agents: {
    researcher: {
      hero: {
        eyebrow: { en: 'AGENT 01 / 07', ru: 'АГЕНТ 01 / 07' },
        title: { en: 'Researcher', ru: 'Исследователь' },
        sub: { en: 'The field worker of the outreach pipeline. It searches the open web, reads primary sources and brings back cited facts plus raw contacts — the raw truth every personalized message is later written from.', ru: 'Полевой работник конвейера outreach. Он ищет в открытом вебе, читает первоисточники и приносит цитируемые факты плюс сырые контакты — ту сырую правду, из которой позже пишется каждое персональное сообщение.' },
      },
      what: { eyebrow: { en: 'What it does', ru: 'Что делает' }, title: { en: 'One loop, built to gather truth', ru: 'Один цикл, созданный для сбора правды' } },
      how: { eyebrow: { en: 'How it works', ru: 'Как это работает' }, title: { en: 'From query to cited findings', ru: 'От запроса до цитируемых находок' } },
      tools: { eyebrow: { en: 'Tools', ru: 'Инструменты' }, title: { en: 'What the Researcher can call', ru: 'Что может вызывать Researcher' } },
      cap: {
        '0': {
          n: { en: 'The agent loop', ru: 'Агентный цикл' },
          d: { en: 'LLM → tools → repeat, up to 50 iterations per agent — with true token streaming: text deltas arrive live, tool calls are assembled from stream fragments, and a broken stream falls back to a plain completion instead of failing.', ru: 'LLM → инструменты → повтор, до 50 итераций на агента — с настоящим потоковым выводом токенов: текстовые дельты приходят в реальном времени, вызовы инструментов собираются из фрагментов потока, а при обрыве потока автоматически переключается на обычное завершение вместо ошибки.' },
        },
        '1': {
          n: { en: 'OSINT workflow, codified', ru: 'OSINT-воркфлоу в коде' },
          d: { en: 'The Researcher role prompt runs a fixed arc: locate targets (search_business_directory, find_leads) → harvest (parse_corporate_site, extract_contacts, search_social) → verify (verify_email, verify_phone, suggest_emails) → persist (save_contacts).', ru: 'Промпт роли Researcher выполняет фиксированную последовательность: поиск целей (search_business_directory, find_leads) → сбор (parse_corporate_site, extract_contacts, search_social) → проверка (verify_email, verify_phone, suggest_emails) → сохранение (save_contacts).' },
        },
        '2': {
          n: { en: 'Hierarchical fan-out', ru: 'Иерархическое распределение' },
          d: { en: 'spawn_agent grows a tree of sub-agents up to max_depth, with explicit context handoff and inherited token budgets. background mode detaches a child and delivers its result later as a notice — the parent keeps working.', ru: 'spawn_agent выращивает дерево подагентов до max_depth с явной передачей контекста и унаследованными бюджетами токенов. Фоновый режим отсоединяет дочернего агента и доставляет его результат позже в виде уведомления — родитель продолжает работу.' },
        },
        '3': {
          n: { en: 'Cite or it did not happen', ru: 'Без источника не считается' },
          d: { en: 'A source-quality hierarchy ranks academic above official above news above blogs above forums. Every finding records its URL; sources.md dedupes citations across the whole fleet.', ru: 'Иерархия качества источников: академические выше официальных выше новостей выше блогов выше форумов. Каждая находка сохраняет свой URL; sources.md дедуплицирует цитаты по всему флоту.' },
        },
        '4': {
          n: { en: 'Multi-format intake', ru: 'Многоформатный ввод' },
          d: { en: 'web_fetch and web_crawl for pages, web_feed for RSS / Atom / sitemaps (~1.1M items/s in the built-in bench), analyze_image for screenshots and documents, pdf_extract for filings, code_symbols + repo_map for codebases.', ru: 'web_fetch и web_crawl для страниц, web_feed для RSS / Atom / карт сайта (~1,1 млн элементов/с во встроенном бенчмарке), analyze_image для скриншотов и документов, pdf_extract для отчётов, code_symbols + repo_map для кодовых баз.' },
        },
        '5': {
          n: { en: 'Untrusted by design', ru: 'Недоверие по дизайну' },
          d: { en: 'Fetched pages are framed as untrusted data and scanned for prompt-injection patterns before entering context; the SSRF guard blocks internal IPs on every fetch and every redirect hop.', ru: 'Полученные страницы рассматриваются как недоверенные данные и сканируются на паттерны промпт-инъекций перед попаданием в контекст; SSRF-защита блокирует внутренние IP при каждом запросе и каждом перенаправлении.' },
        },
      },
      flow: {
        '0': {
          t: { en: 'Plan', ru: 'Планирование' },
          d: { en: 'The coordinator decomposes the request into 2–5 non-overlapping subtasks and persists them; lead-gen queries get per-task contact quotas.', ru: 'Координатор разбивает запрос на 2–5 непересекающихся подзадач и сохраняет их; запросы на генерацию лидов получают квоты контактов на каждую задачу.' },
        },
        '1': {
          t: { en: 'Fan out', ru: 'Распределение' },
          d: { en: 'Each subtask becomes a Researcher — an in-process JoinSet task or an isolated OS worker process — with its share of the session token budget.', ru: 'Каждая подзадача становится Researcher — задачей JoinSet в процессе или изолированным процессом ОС — со своей долей бюджета токенов сессии.' },
        },
        '2': {
          t: { en: 'Research loop', ru: 'Цикл исследования' },
          d: { en: 'Search → fetch → extract → record the source URL, again and again. Children spawn via spawn_agent up to max_depth; background children report back as notices.', ru: 'Поиск → загрузка → извлечение → запись URL источника, снова и снова. Дочерние агенты порождаются через spawn_agent до max_depth; фоновые дочерние агенты отчитываются уведомлениями.' },
        },
        '3': {
          t: { en: 'Reflect', ru: 'Анализ' },
          d: { en: 'A count-based check closes lead-gen quotas with a gap-filling round; Goal Mode\'s LLM judge replans up to replan_rounds times until the goal is covered.', ru: 'Проверка по количеству закрывает квоты генерации лидов раундом заполнения пробелов; LLM-судья Goal Mode перепланирует до replan_rounds раз, пока цель не будет покрыта.' },
        },
        '4': {
          t: { en: 'Hand off', ru: 'Передача' },
          d: { en: 'Budget-capped summaries, structured findings and deduped sources flow into synthesis, memory and the contact DB — personalization fuel for outreach.', ru: 'Резюме с ограничением бюджета, структурированные находки и дедуплицированные источники поступают в синтез, память и базу контактов — топливо персонализации для аутрича.' },
        },
      },
      hood: {
        eyebrow: { en: 'Under the hood', ru: 'Под капотом' },
        title: { en: 'The runtime underneath', ru: 'Рантайм в основе' },
        keyCol: { en: 'Key', ru: 'Ключ' },
        detailCol: { en: 'Detail', ru: 'Описание' },
        '0': {
          k: { en: 'agent loop', ru: 'агентный цикл' },
          d: { en: 'LLM → tool calls → repeat, max_iterations default 50; steering messages and background notices injected at turn boundaries', ru: 'LLM → вызовы инструментов → повтор, max_iterations по умолчанию 50; управляющие сообщения и фоновые уведомления подставляются на границах ходов' },
        },
        '1': {
          k: { en: 'streaming', ru: 'стриминг' },
          d: { en: 'stream: true — live text deltas, tool calls assembled from fragments, automatic non-streaming fallback on broken or empty streams', ru: 'stream: true — текстовые дельты в реальном времени, вызовы инструментов собираются из фрагментов, автоматическое переключение на нестриминговый режим при обрыве или пустом потоке' },
        },
        '2': {
          k: { en: 'spawn_agent', ru: 'spawn_agent' },
          d: { en: 'hierarchy up to max_depth (default 2); roles researcher / analyst / verifier / writer; explicit context handoff; background mode', ru: 'иерархия до max_depth (по умолчанию 2); роли researcher / analyst / verifier / writer; явная передача контекста; фоновый режим' },
        },
        '3': {
          k: { en: 'fan-out', ru: 'распределение' },
          d: { en: 'coordinator splits the query into 2–5 subtasks; JoinSet or OS-process workers (use_multiprocess); max_agents 20', ru: 'координатор разбивает запрос на 2–5 подзадач; JoinSet или процессы ОС (use_multiprocess); max_agents 20' },
        },
        '4': {
          k: { en: 'budgets', ru: 'бюджеты' },
          d: { en: 'session_token_limit split across each batch (floor 4,096 tokens); children inherit what is left of the parent\'s cap', ru: 'session_token_limit распределяется по каждому батчу (минимум 4096 токенов); дочерние агенты наследуют остаток родительского лимита' },
        },
        '5': {
          k: { en: 'doom loop', ru: 'бесконечный цикл' },
          d: { en: '3 identical tool calls in a row → first offense nudges the agent, second offense stops it', ru: '3 одинаковых вызова инструмента подряд → первое нарушение предупреждает агента, второе — останавливает' },
        },
        '6': {
          k: { en: 'goal mode', ru: 'режим цели' },
          d: { en: 'LLM judge compares results with the goal; up to replan_rounds (default 1) gap-filling rounds, at most 3 new subtasks each', ru: 'LLM-судья сравнивает результаты с целью; до replan_rounds (по умолчанию 1) раундов заполнения пробелов, не более 3 новых подзадач в каждом' },
        },
        '7': {
          k: { en: 'fetch safety', ru: 'безопасность загрузки' },
          d: { en: '2 MiB body cap, 50,000-char text truncation, injection scan, SSRF guard with ≤ 5 redirects re-validated per hop', ru: 'Ограничение тела 2 МБ, обрезка текста до 50 000 символов, сканирование на инъекции, SSRF-защита с ≤ 5 перенаправлениями, перепроверяемыми на каждом шаге' },
        },
        '8': {
          k: { en: 'prompt', ru: 'промпт' },
          d: { en: '3 cache tiers (stable / context / volatile) for prefix-cache hits; memory digest injected at depth 0', ru: '3 уровня кеша (stable / context / volatile) для попаданий в префикс-кеш; дайджест памяти подставляется на глубине 0' },
        },
        '9': {
          k: { en: 'stall guard', ru: 'защита от простоя' },
          d: { en: 'no progress for 450s → warning; 1200s → the agent is cancelled', ru: 'нет прогресса 450 с → предупреждение; 1200 с → агент отменяется' },
        },
      },
      feeds: {
        eyebrow: { en: 'Feeds the outreach pipeline', ru: 'Работает на аутрич-пайплайн' },
        title: { en: 'Cited facts become personal lines', ru: 'Цитируемые факты становятся персональными строками' },
        body: {
          en: 'Everything the Researcher gathers is personalization material: cited facts, source URLs, raw contacts with provenance. Extracted contacts are auto-persisted with the page they came from; verified emails and phones land in the contact DB and sync to the CRM without duplicates. Downstream, the outreach step writes each message against exactly these facts — a hiring round, a funding event, a tech migration — so the first line is never generic.',
          ru: 'Всё, что собирает Researcher, — это материал для персонализации: цитируемые факты, URL источников, сырые контакты с историей происхождения. Извлечённые контакты автоматически сохраняются вместе со страницей, с которой они были получены; верифицированные email и телефоны попадают в базу контактов и синхронизируются с CRM без дубликатов. Далее этап аутрича пишет каждое сообщение на основе именно этих фактов — найм, раунд финансирования, технологическая миграция — так что первая строка никогда не бывает шаблонной.',
        },
      },
      cta: {
        title: { en: 'Truth in. Personal message out.', ru: 'Правда на входе. Личное сообщение на выходе.' },
        auto: { en: 'Auto outreach →', ru: 'Автоаутрич →' },
        prev: { en: '← Outreach', ru: '← Аутрич' },
        next: { en: 'Searching →', ru: 'Поиск →' },
      },
    },
    searching: {
      hero: {
        eyebrow: { en: 'AGENT 04 / 07', ru: 'АГЕНТ 04 / 07' },
        title: { en: 'Searching', ru: 'Поиск' },
        sub: { en: 'A list is only as good as its coverage. Searching runs every query across seven search backends at once and fuses the results into one deduplicated, ranked pool — so the outreach list is complete before a single message is drafted.', ru: 'Список настолько хорош, насколько широк его охват. Searching прогоняет каждый запрос сразу по семи поисковым бэкендам и объединяет результаты в один дедуплицированный, ранжированный пул — так что список для аутрича полон ещё до черновика первого сообщения.' },
      },
      what: { eyebrow: { en: 'What it does', ru: 'Что делает' }, title: { en: 'Seven engines, one result set', ru: 'Семь движков, один набор результатов' } },
      how: { eyebrow: { en: 'How it works', ru: 'Как это работает' }, title: { en: 'From one query to a fused list', ru: 'От одного запроса до слитого списка' } },
      flow: {
        "0": { "t": { en: 'Pick the surface', ru: 'Выбор поверхности' } },
        "1": { "t": { en: 'Fan out to backends', ru: 'Разветвление по бэкендам' } },
        "2": { "t": { en: 'Guard & fetch', ru: 'Защита и загрузка' } },
        "3": { "t": { en: 'Fuse & rank', ru: 'Слияние и ранжирование' } },
        "4": { "t": { en: 'Feed forward', ru: 'Передача дальше' } },
      },
      hood: {
        eyebrow: { en: 'Under the hood', ru: 'Под капотом' },
        title: { en: 'Backends, fusion and safety', ru: 'Бэкенды, объединение результатов и безопасность' },
        keyCol: { en: 'Key', ru: 'Ключ' },
        detailCol: { en: 'Detail', ru: 'Описание' },
      },
      tools: { eyebrow: { en: 'Tools', ru: 'Инструменты' }, title: { en: 'What Searching can call', ru: 'Что может вызывать Searching' } },
      feeds: { eyebrow: { en: 'Feeds the outreach pipeline', ru: 'Работает на аутрич-пайплайн' }, title: { en: 'Coverage first, message second', ru: 'Сначала охват, потом сообщение' } },
      cta: {
        title: { en: 'A complete list, ready to write.', ru: 'Полный список, готовый к письму.' },
        auto: { en: 'Auto outreach →', ru: 'Автоаутрич →' },
      },
    },
    extracting: {
      hero: {
        eyebrow: { en: 'AGENT 02 / 07', ru: 'АГЕНТ 02 / 07' },
        title: { en: 'Extracting', ru: 'Извлечение' },
        sub: { en: 'Every page hides contacts. The Extracting agent pulls them out — emails, phones, social profiles, people, companies — and turns raw HTML into the structured fuel your outreach runs on. Nothing reaches your pipeline as prose.', ru: 'Каждая страница прячет контакты. Агент Extracting вытаскивает их — email, телефоны, соцпрофили, людей, компании — и превращает сырой HTML в структурированное топливо вашего аутрича. В пайплайн ничего не попадает в виде простого текста.' },
      },
      what: { eyebrow: { en: 'What it does', ru: 'Что делает' }, title: { en: 'Raw pages in, structured contacts out', ru: 'Сырые страницы на входе, структурированные контакты на выходе' } },
      how: { eyebrow: { en: 'How it works', ru: 'Как это работает' }, title: { en: 'Six passes over every source', ru: 'Шесть проходов по каждому источнику' } },
      flow: {
        "0": { "t": { en: 'Ingest', ru: 'Приём' } },
        "1": { "t": { en: 'Mine', ru: 'Добыча' } },
        "2": { "t": { en: 'Deobfuscate', ru: 'Деобфускация' } },
        "3": { "t": { en: 'Normalize', ru: 'Нормализация' } },
        "4": { "t": { en: 'Entities', ru: 'Сущности' } },
        "5": { "t": { en: 'Emit', ru: 'Выдача' } },
      },
      hood: {
        eyebrow: { en: 'Under the hood', ru: 'Под капотом' },
        title: { en: 'Regexes, thresholds and confidence', ru: 'Регулярные выражения, пороги и оценка достоверности' },
        keyCol: { en: 'Mechanism', ru: 'Механизм' },
        detailCol: { en: 'Detail', ru: 'Описание' },
      },
      tools: { eyebrow: { en: 'Tools', ru: 'Инструменты' }, title: { en: 'The extraction toolbox', ru: 'Набор инструментов извлечения' } },
      feeds: { eyebrow: { en: 'Feeds the outreach pipeline', ru: 'Работает на аутрич-пайплайн' }, title: { en: 'Extraction is outreach fuel', ru: 'Извлечение — топливо аутрича' } },
      cta: {
        title: { en: 'From raw pages to pipeline-ready contacts', ru: 'От сырых страниц до контактов, готовых к пайплайну' },
        auto: { en: 'Auto outreach', ru: 'Автоаутрич' },
      },
    },
    structuring: {
      hero: {
        eyebrow: { en: 'AGENT 03 / 07', ru: 'АГЕНТ 03 / 07' },
        title: { en: 'Structuring', ru: 'Структурирование' },
        sub: { en: 'A lead you cannot address is noise. The Structuring agent turns extracted fragments into one canonical contact record — normalized, deduplicated, linked into an entity graph — so outreach always knows exactly who it is writing to.', ru: 'Лид, к которому нельзя обратиться, — это шум. Агент Structuring превращает извлечённые фрагменты в одну каноническую запись контакта — нормализованную, дедуплицированную, связанную в граф сущностей — чтобы аутрич всегда точно знал, кому пишет.' },
      },
      what: { eyebrow: { en: 'What it does', ru: 'Что делает' }, title: { en: 'Canonical records from scattered fragments', ru: 'Канонические записи из разрозненных фрагментов' } },
      how: { eyebrow: { en: 'How it works', ru: 'Как это работает' }, title: { en: 'From tool output to canonical contact', ru: 'От вывода инструмента до канонического контакта' } },
      flow: {
        "0": { "t": { en: 'Trigger', ru: 'Триггер' } },
        "1": { "t": { en: 'Autosave', ru: 'Автосохранение' } },
        "2": { "t": { en: 'Normalize', ru: 'Нормализация' } },
        "3": { "t": { en: 'Find-or-insert', ru: 'Найти или вставить' } },
        "4": { "t": { en: 'Graph', ru: 'Граф' } },
        "5": { "t": { en: 'Sync', ru: 'Синхронизация' } },
      },
      hood: {
        eyebrow: { en: 'Under the hood', ru: 'Под капотом' },
        title: { en: 'Schema, keys and merge rules', ru: 'Схема, ключи и правила слияния' },
        keyCol: { en: 'Mechanism', ru: 'Механизм' },
        detailCol: { en: 'Detail', ru: 'Описание' },
      },
      tools: { eyebrow: { en: 'Tools', ru: 'Инструменты' }, title: { en: 'What the agent runs', ru: 'Что запускает агент' } },
      feeds: { eyebrow: { en: 'Feeds the outreach pipeline', ru: 'Работает на аутрич-пайплайн' }, title: { en: 'A structured contact is an addressee', ru: 'Структурированный контакт — это адресат' }, body: { en: 'Outreach never writes "to whom it may concern". It writes to Ann Petrova, CTO at Acme, ann@acme.io — with her LinkedIn for context, tags for segmentation and notes for the opening hook. Because Structuring merged every source into one record, the draft lands once, in the right inbox — not three times in three spellings.', ru: 'Аутрич никогда не пишет «уважаемый господин». Он пишет Анне Петровой, техническому директору Acme, ann@acme.io — с её LinkedIn для контекста, тегами для сегментации и заметками для вступительного крючка. Поскольку Structuring объединил каждый источник в одну запись, черновик ложится один раз и в правильный ящик — а не трижды в трёх написаниях.' } },
      cta: {
        title: { en: 'Every message needs a canonical addressee', ru: 'Каждому сообщению нужен канонический адресат' },
        prev: { en: '← Extracting', ru: '← Извлечение' },
        next: { en: 'Cleaning →', ru: 'Очистка →' },
        auto: { en: 'Auto outreach', ru: 'Автоаутрич' },
      },
      cap: {
        "0": {
          n: { en: 'save_contacts', ru: 'save_contacts' },
          t: { en: 'Persist, never duplicate', ru: 'Сохранять, никогда не дублировать' },
          d: { en: 'Writes harvested contacts into SQLite or PostgreSQL, deduplicating against existing records: same normalized email or phone means a merge, not a second row.', ru: 'Записывает собранные контакты в SQLite или PostgreSQL с дедупликацией: одинаковый нормализованный email или телефон означает слияние, а не вторую строку.' }
        },
        "1": {
          n: { en: 'deterministic autosave', ru: 'deterministic autosave' },
          t: { en: 'The model cannot forget', ru: 'Модель не может забыть' },
          d: { en: 'The runtime persists the metadata of every successful extract_contacts / find_leads itself.', ru: 'Рантайм сам сохраняет метаданные каждого успешного extract_contacts / find_leads.' }
        },
        "2": {
          n: { en: 'ContactDb schema', ru: 'ContactDb schema' },
          t: { en: 'Five tables, one dossier', ru: 'Пять таблиц, одно досье' },
          d: { en: 'contacts, social_profiles, companies, tags and notes — with provenance, timestamps and crm_id.', ru: 'contacts, social_profiles, companies, tags и notes — с происхождением, временными метками и crm_id.' }
        },
        "3": {
          n: { en: 'normalization', ru: 'normalization' },
          t: { en: 'One spelling per contact', ru: 'Одно написание на контакт' },
          d: { en: 'normalize_email trims and lower-cases; normalize_phone keeps ASCII digits only.', ru: 'normalize_email обрезает и приводит к нижнему регистру; normalize_phone оставляет только ASCII-цифры.' }
        },
        "4": {
          n: { en: 'merge semantics', ru: 'merge semantics' },
          t: { en: 'Records grow, never split', ru: 'Записи растут, не дробятся' },
          d: { en: 'The older record wins: blank fields fill from new data, tags append, duplicates deleted.', ru: 'Старая запись побеждает: пустые поля заполняются, теги добавляются, дубли удаляются.' }
        },
        "5": {
          n: { en: 'entity graph', ru: 'entity graph' },
          t: { en: 'Person ↔ company ↔ location', ru: 'Человек ↔ компания ↔ локация' },
          d: { en: 'Typed relations like works_at, leads and located_in connect entities; BFS up to depth 4.', ru: 'Типизированные связи works_at, leads и located_in соединяют сущности; BFS до глубины 4.' }
        },
      },
      toolNames: {
        "0": { en: 'save_contacts', ru: 'save_contacts' },
        "1": { en: 'extract_contacts', ru: 'extract_contacts' },
        "2": { en: 'find_leads', ru: 'find_leads' },
        "3": { en: 'search_social', ru: 'search_social' },
        "4": { en: 'verify_email', ru: 'verify_email' },
        "5": { en: 'verify_phone', ru: 'verify_phone' },
        "6": { en: 'enrich_company', ru: 'enrich_company' },
        "7": { en: 'enrich_person', ru: 'enrich_person' },
        "8": { en: 'memory', ru: 'memory' },
        "9": { en: 'spawn_agent', ru: 'spawn_agent' }
      },
    },
    cleaning: {
      hero: {
        eyebrow: { en: 'AGENT 05 / 07', ru: 'АГЕНТ 05 / 07' },
        title: { en: 'Cleaning', ru: 'Очистка' },
        sub: { en: 'Every bounce burns budget and sender reputation. The Cleaning agent verifies each email, phone and profile before it reaches your sequence — so outreach spends every message on a contact that actually exists.', ru: 'Каждый возврат сжигает бюджет и репутацию отправителя. Агент Cleaning проверяет каждый email, телефон и профиль, прежде чем они попадут в вашу последовательность, — чтобы аутрич тратил каждое сообщение на реально существующий контакт.' },
      },
      what: { eyebrow: { en: 'What it does', ru: 'Что делает' }, title: { en: 'Verification before a single send', ru: 'Проверка до первой отправки' } },
      how: { eyebrow: { en: 'How it works', ru: 'Как это работает' }, title: { en: 'Six gates between harvest and send', ru: 'Шесть ворот между сбором и отправкой' } },
      flow: {
        "0": { "t": { en: 'Candidates', ru: 'Кандидаты' } },
        "1": { "t": { en: 'Email checks', ru: 'Проверки email' } },
        "2": { "t": { en: 'SMTP probe', ru: 'SMTP-проба' } },
        "3": { "t": { en: 'Phone checks', ru: 'Проверки телефона' } },
        "4": { "t": { en: 'Social checks', ru: 'Проверки соцсетей' } },
        "5": { "t": { en: 'Rank & ship', ru: 'Ранжирование и отдача' } },
      },
      hood: {
        eyebrow: { en: 'Under the hood', ru: 'Под капотом' },
        title: { en: 'Thresholds, filters and formulas', ru: 'Пороги, фильтры и формулы' },
        keyCol: { en: 'Mechanism', ru: 'Механизм' },
        detailCol: { en: 'Detail', ru: 'Описание' },
      },
      results: { eyebrow: { en: 'Results', ru: 'Результаты' }, title: { en: 'What cleaning buys you', ru: 'Что вам даёт очистка' } },
      tools: { eyebrow: { en: 'Tools', ru: 'Инструменты' }, title: { en: 'The verification toolbox', ru: 'Набор инструментов проверки' } },
      feeds: { eyebrow: { en: 'Feeds the outreach pipeline', ru: 'Работает на аутрич-пайплайн' }, title: { en: 'A clean list is budget that stays yours', ru: 'Чистый список — это бюджет, который остаётся у вас' } },
      cta: {
        title: { en: 'Clean lists convert. Dirty lists bounce.', ru: 'Чистые списки конвертируют. Грязные — возвращаются.' },
        auto: { en: 'Auto outreach', ru: 'Автоаутрич' },
      },
    },
    memoring: {
      hero: {
        eyebrow: { en: 'Agent 06 / 07', ru: 'Агент 06 / 07' },
        title: { en: 'Memoring', ru: 'Память' },
        sub: { en: 'Long-term memory for the whole pipeline. Every verified contact, company fact and past touch is absorbed into an append-only knowledge base — so the next run, and the next outreach message, starts from everything already known instead of zero.', ru: 'Долгосрочная память для всего пайплайна. Каждый проверенный контакт, факт о компании и прошлые касания поглощаются в базу знаний с возможностью только добавления (append-only), — так что следующий прогон и следующее сообщение аутрича начинаются со всего уже известного вместо нуля.' },
      },
      what: { eyebrow: { en: 'What it does', ru: 'Что делает' }, title: { en: 'Six tools, one knowledge base', ru: 'Шесть инструментов, одна база знаний' } },
      tiers: { eyebrow: { en: 'Three tiers', ru: 'Три уровня' }, title: { en: 'Profile, skills, archive', ru: 'Профиль, навыки, архив' } },
      how: { eyebrow: { en: 'How it works', ru: 'Как это работает' }, title: { en: 'The absorb pipeline', ru: 'Пайплайн поглощения' } },
      flow: {
        "0": { "t": { en: 'Validate', ru: 'Валидация' } },
        "1": { "t": { en: 'Secret scan', ru: 'Скан секретов' } },
        "2": { "t": { en: 'Consolidate', ru: 'Консолидация' } },
        "3": { "t": { en: 'Dedup & classify', ru: 'Дедуп и классификация' } },
        "4": { "t": { en: 'Apply verdict', ru: 'Применить вердикт' } },
      },
      hood: {
        eyebrow: { en: 'Under the hood', ru: 'Под капотом' },
        title: { en: 'Append-only, verifiable, fast', ru: 'Append-only, проверяемо, быстро' },
        keyCol: { en: 'Detail', ru: 'Описание' },
        detailCol: { en: 'Implementation', ru: 'Реализация' },
      },
      tools: { eyebrow: { en: 'Tools', ru: 'Инструменты' }, title: { en: 'The Memoring toolset', ru: 'Набор инструментов Memoring' } },
      feeds: { eyebrow: { en: 'Feeds the outreach pipeline', ru: 'Работает на аутрич-пайплайн' }, title: { en: 'Outreach that remembers every touch', ru: 'Аутрич, который помнит каждое касание' } },
      cta: {
        title: { en: 'Next: Outreach — the finale of the chain', ru: 'Дальше: Outreach — финал цепи' },
      },
    },
    outreach: {
      hero: {
        eyebrow: { en: 'Agent 07 / 07', ru: 'Агент 07 / 07' },
        title: { en: 'Outreach', ru: 'Аутрич' },
        sub: { en: 'The finale of the chain. Everything six agents researched, extracted, verified and remembered becomes the one thing that matters — a personal message a human actually wants to read. Approved before it ships, written per recipient, then synced to your CRM or exported.', ru: 'Финал цепи. Всё, что шесть агентов исследовали, извлекли, проверили и запомнили, становится одной важной вещью — личным сообщением, которое человек действительно захочет прочитать. Утверждается перед отправкой, пишется под получателя, затем синхронизируется с CRM или экспортируется.' },
      },
      what: { eyebrow: { en: 'What it does', ru: 'Что делает' }, title: { en: 'From verified leads to ready drafts', ru: 'От проверенных лидов до готовых черновиков' } },
      how: { eyebrow: { en: 'How it works', ru: 'Как это работает' }, title: { en: 'Five steps to a ready-to-send draft', ru: 'Пять шагов до готового к отправке черновика' } },
      flow: {
        "0": { "t": { en: 'Ground', ru: 'Основание' } },
        "1": { "t": { en: 'Compose', ru: 'Составление' } },
        "2": { "t": { en: 'Approve', ru: 'Согласование' } },
        "3": { "t": { en: 'Deliver', ru: 'Доставка' } },
        "4": { "t": { en: 'Watch', ru: 'Наблюдение' } },
      },
      hood: {
        eyebrow: { en: 'Under the hood', ru: 'Под капотом' },
        title: { en: 'A draft you can audit', ru: 'Черновик, который можно аудитировать' },
        keyCol: { en: 'Detail', ru: 'Описание' },
        detailCol: { en: 'Implementation', ru: 'Реализация' },
      },
      tools: { eyebrow: { en: 'Tools', ru: 'Инструменты' }, title: { en: 'The Outreach toolset', ru: 'Набор инструментов Outreach' } },
      feeds: { eyebrow: { en: 'Feeds the outreach pipeline', ru: 'Работает на аутрич-пайплайн' }, title: { en: 'The finale: final assembly and delivery', ru: 'Финал: итоговая сборка и доставка' } },
      cta: {
        title: { en: 'Run the whole pipeline — end with outreach', ru: 'Запустите весь пайплайн — и завершите аутричем' },
      },
    },
  },
};
