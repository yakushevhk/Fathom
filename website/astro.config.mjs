import { defineConfig } from 'astro/config';
import vercel from '@astrojs/vercel';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';

const SITE = 'https://fathom.uz';
const LOCALES = { en: 'en', ru: 'ru' };
const DEFAULT_LOCALE = 'en';

const ALL_CANONICAL_PATHS = [
  '/',
  '/agents/browser',
  '/agents/cleaning',
  '/agents/coder',
  '/agents/coordinator',
  '/agents/extracting',
  '/agents/memoring',
  '/agents/operator',
  '/agents/outreach',
  '/agents/researcher',
  '/agents/reviewer',
  '/agents/searching',
  '/agents/structuring',
  '/api',
  '/architecture',
  '/benchmarks',
  '/blog',
  '/blog/anatomy-of-fan-out',
  '/blog/benchmarks-repo-map',
  '/blog/memory-that-remembers',
  '/blog/verification-pipeline',
  '/changelog',
  '/code',
  '/dashboard',
  '/demo',
  '/docs',
  '/docs/api',
  '/docs/architecture',
  '/docs/cli',
  '/docs/configuration',
  '/docs/memory',
  '/docs/outreach',
  '/docs/personalization',
  '/docs/protocol-gui',
  '/docs/quickstart',
  '/docs/recipes',
  '/docs/tools',
  '/features',
  '/features/auto-outreach',
  '/features/lead-generation',
  '/features/research',
  '/install',
  '/integrations',
  '/mcp',
  '/memory',
  '/ops',
  '/osint',
  '/playground',
  '/pricing',
  '/profiles',
  '/research',
  '/security',
  '/solutions',
  '/solutions/agency-whitelabel',
  '/solutions/b2b-outbound',
  '/solutions/backoffice-finance',
  '/solutions/executive-recruiting',
  '/solutions/market-intelligence',
  '/tools',
  '/tui',
  '/video',
  '/vs-python',
  '/whitepaper',
];

export default defineConfig({
  site: SITE,
  adapter: vercel(),
  integrations: [
    mdx(),
    sitemap({
      customPages: ALL_CANONICAL_PATHS.map((p) => (p === '/' ? `${SITE}/ru` : `${SITE}/ru${p}`)),
      filter: (page) => {
        const url = page.toLowerCase();
        if (url.includes('/404') || url.includes('/deck/page_')) return false;
        return true;
      },
      serialize: (entry) => {
        const parsed = new URL(entry.url);
        const pathname = parsed.pathname.replace(/\/+$/, '') || '/';
        const locale = pathname === '/ru' || pathname.startsWith('/ru/') ? 'ru' : DEFAULT_LOCALE;
        const cleanPath = locale === 'ru' ? (pathname.slice(3) || '/') : pathname;
        const localizedPath = (lang) => {
          const path = lang === 'ru' ? `/ru${cleanPath === '/' ? '' : cleanPath}` : cleanPath;
          return `${SITE}${path === '/' ? '' : path}`;
        };
        const url = localizedPath(locale);

        // Generate hreflang alternates for each locale plus x-default
        const links = [
          ...Object.entries(LOCALES).map(([lang, hreflang]) => ({
            url: localizedPath(lang),
            lang: hreflang,
          })),
          {
            url: localizedPath('en'),
            lang: 'x-default',
          },
        ];

        // Granular priority and change frequency map
        let priority = 0.6;
        let changefreq = 'monthly';

        const cleanUrl = `${SITE}${cleanPath === '/' ? '' : cleanPath}`;

        if (cleanUrl === SITE) {
          priority = 1.0;
          changefreq = 'weekly';
        } else if (
          cleanUrl === `${SITE}/solutions` ||
          cleanUrl === `${SITE}/whitepaper` ||
          cleanUrl === `${SITE}/pricing` ||
          cleanUrl === `${SITE}/docs`
        ) {
          priority = 0.9;
          changefreq = 'weekly';
        } else if (
          cleanUrl.startsWith(`${SITE}/solutions/`) ||
          cleanUrl.startsWith(`${SITE}/features`) ||
          cleanUrl === `${SITE}/vs-python` ||
          cleanUrl === `${SITE}/demo` ||
          cleanUrl === `${SITE}/video` ||
          cleanUrl === `${SITE}/security` ||
          cleanUrl === `${SITE}/memory` ||
          cleanUrl === `${SITE}/architecture` ||
          cleanUrl.startsWith(`${SITE}/docs/`)
        ) {
          priority = 0.8;
          changefreq = 'monthly';
        } else if (
          cleanUrl === `${SITE}/blog` ||
          cleanUrl.startsWith(`${SITE}/blog/`) ||
          cleanUrl === `${SITE}/benchmarks` ||
          cleanUrl === `${SITE}/tools` ||
          cleanUrl === `${SITE}/install` ||
          cleanUrl === `${SITE}/code` ||
          cleanUrl === `${SITE}/playground` ||
          cleanUrl === `${SITE}/osint` ||
          cleanUrl === `${SITE}/ops` ||
          cleanUrl === `${SITE}/profiles` ||
          cleanUrl === `${SITE}/research` ||
          cleanUrl === `${SITE}/integrations` ||
          cleanUrl === `${SITE}/mcp` ||
          cleanUrl === `${SITE}/tui` ||
          cleanUrl === `${SITE}/api` ||
          cleanUrl === `${SITE}/dashboard`
        ) {
          priority = 0.7;
          changefreq = cleanUrl.startsWith(`${SITE}/blog`) ? 'weekly' : 'monthly';
        } else if (cleanUrl === `${SITE}/changelog`) {
          priority = 0.6;
          changefreq = 'weekly';
        } else if (cleanUrl.startsWith(`${SITE}/agents/`)) {
          priority = 0.6;
          changefreq = 'monthly';
        }

        return {
          url,
          changefreq,
          priority,
          lastmod: new Date().toISOString().split('T')[0],
          links,
        };
      },
    }),
  ],
  i18n: {
    locales: ['en', 'ru'],
    defaultLocale: 'en',
    routing: {
      prefixDefaultLocale: false,
      redirectToDefaultLocale: false,
    },
  },
  vite: {
    build: {
      chunkSizeWarningLimit: 525,
    },
    css: {
      preprocessorOptions: {},
    },
  },
});