export default {
  osint: {
    hero: {
      badge: { en: 'OSINT Playbook', ru: 'OSINT-плейбук' },
      title: { en: 'Every lead, sourced<br />and verified.', ru: 'Каждый лид — с источником<br />и проверкой.' },
      sub: { en: 'An automated OSINT pipeline: parallel agents source companies and people across registries, maps, corporate sites and social networks, extract and verify every email and phone, then merge it all into one clean contact database — synced straight to your CRM.', ru: 'Автоматический OSINT-пайплайн: параллельные агенты черпают компании и людей из реестров, карт, корпоративных сайтов и соцсетей, извлекают и проверяют каждый email и телефон, затем сливают всё в единую чистую базу контактов — синхронизированную с CRM.' },
    },
    sources: {
      badge: { en: 'Data sources', ru: 'Источники данных' },
      title: { en: 'Where the leads come from', ru: 'Откуда берутся лиды' },
      "0": { name: { en: 'Business registries', ru: 'Бизнес-реестры' }, desc: { en: 'rusprofile.ru, list-org.com, sbis.ru, companies.rbc.ru — these aggregators expose registry data: INN/OGRN, founders and executives.', ru: 'rusprofile.ru, list-org.com, sbis.ru, companies.rbc.ru — эти агрегаторы открывают данные реестров: ИНН/ОГРН, учредители и руководители.' } },
      "1": { name: { en: 'Maps & directories', ru: 'Карты и каталоги' }, desc: { en: '2GIS (CIS), Yandex Maps, Yellow Pages — companies with addresses, phones and opening hours.', ru: '2GIS (СНГ), Яндекс Карты, Yellow Pages — компании с адресами, телефонами и часами работы.' } },
      "2": { name: { en: 'Corporate sites', ru: 'Корпоративные сайты' }, desc: { en: 'Team and about pages, contacts, press releases — plus Schema.org markup and JSON-LD blocks.', ru: 'Страницы команды и «о компании», контакты, пресс-релизы — плюс разметка Schema.org и JSON-LD.' } },
      "3": { name: { en: 'Social networks', ru: 'Соцсети' }, desc: { en: 'LinkedIn profiles, X/Twitter search, Telegram channels — with anti-bot limits handled honestly.', ru: 'Профили LinkedIn, поиск X/Twitter, каналы Telegram — с честной обработкой антибот-лимитов.' } },
      "4": { name: { en: 'News & mentions', ru: 'Новости и упоминания' }, desc: { en: 'Serper News and Google News RSS — funding rounds, hires and launches.', ru: 'Serper News и Google News RSS — раунды финансирования, наймы и запуски.' } },
    },
    pipeline: {
      badge: { en: 'Pipeline', ru: 'Пайплайн' },
      title: { en: 'Seven stages, one query', ru: 'Семь стадий, один запрос' },
    },
    stage: {
      "0": { title: { en: 'Plan', ru: 'План' }, desc: { en: 'The coordinator decomposes the query into parallel subtasks.', ru: 'Координатор разбивает запрос на параллельные подзадачи.' } },
      "1": { title: { en: 'Extract', ru: 'Извлечение' }, desc: { en: 'extract_contacts pulls emails, phones, socials and companies from every page.', ru: 'extract_contacts достаёт email, телефоны, соцсети и компании со страниц.' } },
      "2": { title: { en: 'Verify', ru: 'Проверка' }, desc: { en: 'verify_email, verify_phone and verify_social_profile filter the noise and score confidence.', ru: 'verify_email, verify_phone и verify_social_profile отсеивают шум и оценивают уверенность.' } },
      "3": { title: { en: 'Enrich', ru: 'Обогащение' }, desc: { en: 'enrich_company and enrich_person add industry, size, location, role and buying signals.', ru: 'enrich_company и enrich_person добавляют отрасль, размер, локацию, роль и сигналы спроса.' } },
      "4": { title: { en: 'ContactDb', ru: 'ContactDb' }, desc: { en: 'SQLite or PostgreSQL store; dedup and merge on normalized emails and phones.', ru: 'Хранилище SQLite или PostgreSQL; дедуп и слияние по нормализованным email и телефонам.' } },
      "5": { title: { en: 'CRM', ru: 'CRM' }, desc: { en: 'When configured, hand off approved records to a CRM adapter with crm_id-aware deduplication.', ru: 'При настройке передавайте одобренные записи в CRM-адаптер с дедупликацией по crm_id.' } },
      "6": { title: { en: 'Report', ru: 'Отчёт' }, desc: { en: 'summary.md plus a contact table, exported to CSV, vCard, JSON or Excel.', ru: 'summary.md плюс таблица контактов, экспорт в CSV, vCard, JSON или Excel.' } },
    },
    verify: {
      badge: { en: 'Verification', ru: 'Верификация' },
      title: { en: 'Nothing unverified gets saved', ru: 'Ничего непроверенного не сохраняется' },
      email: { en: 'Bounced emails and dead numbers burn outreach budget. Every candidate passes MX checks, disposable-domain and role-based filters before it earns a confidence score — and obfuscated addresses like <code class="ic">name [at] domain [dot] com</code> are decoded automatically, with a lower score than plain ones.', ru: 'Возвраты и «мёртвые» номера сжигают бюджет аутрича. Каждый кандидат проходит MX-проверки, фильтры disposable-доменов и ролевых ящиков, прежде чем получит confidence — а обфусцированные адреса вида <code class="ic">name [at] domain [dot] com</code> декодируются автоматически, с меньшим баллом, чем обычные.' },
    },
    phones: {
      badge: { en: 'Phones & patterns', ru: 'Телефоны и паттерны' },
      title: { en: 'E.164 phones, inferred emails', ru: 'Телефоны E.164, выведенные email' },
      desc: { en: 'Phones are normalized to E.164 via libphonenumber with country and mobile/landline detection. When only a name and a domain are known, <code class="ic">suggest_emails</code> builds candidate addresses from name permutations and the domain\u2019s existing pattern — up to 9 variants to verify.', ru: 'Телефоны нормализуются в E.164 через libphonenumber с определением страны и мобильный/городской. Когда известны только имя и домен, <code class="ic">suggest_emails</code> строит кандидатов из перестановок имени и существующего паттерна домена — до 9 вариантов для проверки.' },
    },
    storage: {
      badge: { en: 'Storage', ru: 'Хранилище' },
      title: { en: 'ContactDb — one canonical record', ru: 'ContactDb — одна каноническая запись' },
      desc: { en: 'Contact-oriented workflows can persist records in SQLite; PostgreSQL is available behind the same interface when configured. Dedup and merge run on normalized emails and phones, so re-runs and overlapping sources never create double records. Tags and notes attach per contact; the whole schema is yours to query.', ru: 'Контакты попадают в SQLite из коробки — или в PostgreSQL для больших баз, с тем же интерфейсом. Дедуп и слияние работают по нормализованным email и телефонам, так что повторные прогоны и пересекающиеся источники не создают дублей. Теги и заметки прикрепляются к контакту; вся схема открыта для запросов.' },
    },
    schema: {
      table: { en: 'Table', ru: 'Таблица' },
      columns: { en: 'Columns', ru: 'Колонки' },
    },
    queries: {
      badge: { en: 'Query examples', ru: 'Примеры запросов' },
      title: { en: 'One command, a folder of leads', ru: 'Одна команда — папка лидов' },
      0: { label: { en: 'Moscow — IT CEOs', ru: 'Москва — IT-CEO' } },
      1: { label: { en: 'Berlin — SaaS Series A–B', ru: 'Берлин — SaaS Series A–B' } },
      2: { label: { en: 'Dubai — fintech market', ru: 'Дубай — fintech-рынок' } },
    },
    ethics: {
      badge: { en: 'Ethics & compliance', ru: 'Этика и комплаенс' },
      title: { en: 'Responsible by default', ru: 'Ответственно по умолчанию' },
      "0": { title: { en: 'GDPR & 152-ФЗ', ru: 'GDPR и 152-ФЗ' }, desc: { en: 'Personal data processed in line with GDPR and 152-ФЗ.', ru: 'Персональные данные обрабатываются в соответствии с GDPR и 152-ФЗ.' } },
      "1": { title: { en: 'Public data only', ru: 'Только публичные данные' }, desc: { en: 'Nothing scraped behind logins, paywalls or private APIs.', ru: 'Ничего не скрейпится за логинами, пейволлами или закрытыми API.' } },
      "2": { title: { en: 'robots.txt & rate limits', ru: 'robots.txt и лимиты' }, desc: { en: 'Crawling respects robots.txt and built-in throttling.', ru: 'Краулинг уважает robots.txt и встроенное ограничение.' } },
      "3": { title: { en: 'Honest statuses', ru: 'Честные статусы' }, desc: { en: 'Uncertainty is explicit — never silent guesses.', ru: 'Неопределённость выражается явно — никаких молчаливых догадок.' } },
      "4": { title: { en: 'LinkedIn HTTP 999', ru: 'LinkedIn HTTP 999' }, desc: { en: 'LinkedIn answers bots with HTTP 999; agents surface the block.', ru: 'LinkedIn отвечает ботам HTTP 999; агенты показывают блокировку.' } },
      "5": { title: { en: 'No spam', ru: 'Без спама' }, desc: { en: 'Contacts are for qualified outreach, not bulk mailings.', ru: 'Контакты предназначены для точечного аутрича, а не массовых рассылок.' } },
    },
    cta: {
      title: { en: 'See the full lead-generation workflow', ru: 'Смотреть полный рабочий процесс генерации лидов' },
      start: { en: 'Lead generation →', ru: 'Генерация лидов →' },
    },
  },
};
