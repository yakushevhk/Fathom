// Repository page translations (playground.astro — now a technical repo breakdown).
export default {
  nav: {
    playground: { en: 'Repository', ru: 'Репозиторий' },
  },
  repo: {
    hero: {
      badge: { en: 'Repository', ru: 'Репозиторий' },
      title: { en: 'The source,<br /><em class="it">crate by crate.</em>', ru: 'Исходники,<br /><em class="it">крейт за крейтом.</em>' },
      sub: { en: '12 workspace crates, 3 apps, 50+ built-in tools, and a Rust CLI — everything in one public repository.', ru: '12 крейтов, 3 приложения, 50+ встроенных инструментов и Rust CLI — всё в одном публичном репозитории.' },
    },
    workspace: {
      badge: { en: 'Workspace', ru: 'Workspace' },
      title: { en: '12 crates, one binary', ru: '12 крейтов, один бинарник' },
      sub: { en: 'Cargo workspace with resolver 2. Each crate compiles into a single <code>fathom</code> binary, plus optional companion apps.', ru: 'Cargo workspace с resolver 2. Каждый крейт компилируется в один бинарник <code>fathom</code> плюс опциональные приложения.' },
    },
    layers: {
      badge: { en: 'Layers', ru: 'Слои' },
      title: { en: 'How the crates connect', ru: 'Как крейты связаны' },
    },
    apps: {
      badge: { en: 'Apps', ru: 'Приложения' },
      title: { en: 'Three companion apps', ru: 'Три сопутствующих приложения' },
    },
    website: {
      badge: { en: 'Website', ru: 'Сайт' },
      title: { en: 'This site', ru: 'Этот сайт' },
      sub: { en: '47 pages across two builds: the marketing site (Astro 7, 30+ pages, i18n en/ru, Three.js) and technical documentation (MDX).', ru: '47 страниц в двух сборках: маркетинговый сайт (Astro 7, 30+ страниц, i18n en/ru, Three.js) и техническая документация (MDX).' },
    },
    stack: {
      badge: { en: 'Stack', ru: 'Стек' },
      title: { en: 'Dependencies &amp; runtime', ru: 'Зависимости и рантайм' },
    },
    cta: {
      title: { en: 'Browse the full source', ru: 'Посмотреть весь исходный код' },
      sub: { en: 'Every crate, every tool, every test — open in one repository.', ru: 'Каждый крейт, каждый инструмент, каждый тест — открыто в одном репозитории.' },
      github: { en: 'View on GitHub →', ru: 'На GitHub →' },
      docs: { en: 'Documentation', ru: 'Документация' },
    },
  },
};