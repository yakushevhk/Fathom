import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';

const SITE = 'https://fathom.uz';
const LOCALES = { en: 'en', ru: 'ru' };
const DEFAULT_LOCALE = 'en';

export default defineConfig({
  site: SITE,
  integrations: [
    mdx(),
    sitemap({
      serialize: (entry) => {
        // Normalize through URL.pathname so locale handling is independent of
        // trailing slashes and never treats `/ru` as an English path.
        const parsed = new URL(entry.url);
        const pathname = parsed.pathname.replace(/\/+$/, '') || '/';
        const locale = pathname === '/ru' || pathname.startsWith('/ru/') ? 'ru' : DEFAULT_LOCALE;
        const cleanPath = locale === 'ru' ? (pathname.slice(3) || '/') : pathname;
        const localizedPath = (lang) => {
          const path = lang === 'ru' ? `/ru${cleanPath === '/' ? '' : cleanPath}` : cleanPath;
          return `${SITE}${path === '/' ? '' : path}`;
        };
        const url = localizedPath(locale);

        // Generate hreflang alternates for each locale from the same clean path.
        const links = Object.entries(LOCALES).map(([lang, hreflang]) => ({
          url: localizedPath(lang),
          lang: hreflang,
        }));

        // Priority map
        let priority = 0.5;
        let changefreq = 'monthly';

        const cleanUrl = `${SITE}${cleanPath === '/' ? '' : cleanPath}`;

        if (cleanUrl === SITE || cleanUrl === `${SITE}/docs`) {
          priority = 1.0;
          changefreq = 'weekly';
        } else if (cleanUrl.startsWith(`${SITE}/features`) || cleanUrl.startsWith(`${SITE}/docs/`)) {
          priority = 0.8;
          changefreq = 'monthly';
        } else if (cleanUrl.startsWith(`${SITE}/agents`)) {
          priority = 0.6;
          changefreq = 'monthly';
        } else if (cleanUrl.startsWith(`${SITE}/blog`)) {
          priority = 0.7;
          changefreq = 'weekly';
        } else if (cleanUrl === `${SITE}/changelog`) {
          priority = 0.5;
          changefreq = 'weekly';
        } else if (cleanUrl === `${SITE}/install`) {
          priority = 0.7;
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
    },
  },
  vite: {
    // Pipeline3D lazy-loads Three.js so the shared page entry stays small.
    // The remaining vendor chunk is Three.js itself (~511 kB minified), which
    // is the intentional WebGL runtime for the homepage demo; keep this
    // narrowly scoped warning floor just above that measured asset rather
    // than hiding unexpectedly large application chunks.
    build: {
      chunkSizeWarningLimit: 525,
    },
    css: {
      preprocessorOptions: {},
    },
  },
});