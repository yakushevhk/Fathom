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
        const url = entry.url.replace(/\/$/, '');

        // Determine locale from URL
        let locale = DEFAULT_LOCALE;
        const pathPart = url.replace(SITE, '');
        const match = pathPart.match(/^\/(ru)\//);
        if (match) {
          locale = match[1];
        }

        // Generate hreflang alternates for each locale
        const links = [];
        for (const [lang, hreflang] of Object.entries(LOCALES)) {
          let alternateUrl;
          if (lang === DEFAULT_LOCALE) {
            // EN: no prefix
            alternateUrl = url;
          } else {
            // Russian locale: /ru/path
            if (pathPart === '' || pathPart === '/') {
              alternateUrl = `${SITE}/${lang}`;
            } else {
              const cleanPath = pathPart.replace(/^\/(ru)/, '');
              alternateUrl = `${SITE}/${lang}${cleanPath}`;
            }
          }
          links.push({
            url: alternateUrl,
            lang: hreflang,
          });
        }

        // Priority map
        let priority = 0.5;
        let changefreq = 'monthly';

        const cleanUrl = url.replace(/^\/(ru)/, '');

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
          url: entry.url,
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
    css: {
      preprocessorOptions: {},
    },
  },
});