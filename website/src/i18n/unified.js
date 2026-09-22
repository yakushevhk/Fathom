// Unified dictionary loader exporting merged translations across base + all groups
import { translations as baseTranslations } from './translations.js';
import bot from './groups/bot.js';
import demo from './groups/demo.js';
import meta from './groups/meta.js';
import postdocs from './groups/postdocs.js';
import site from './groups/site.js';
import solutions from './groups/solutions.js';
import tech from './groups/tech.js';
import whitepaper from './groups/whitepaper.js';

function deepMerge(a, b) {
  const out = { ...a };
  for (const k in b) {
    if (b[k] && typeof b[k] === 'object' && !Array.isArray(b[k]) && a[k] && typeof a[k] === 'object' && !Array.isArray(a[k])) {
      out[k] = deepMerge(a[k], b[k]);
    } else {
      out[k] = b[k];
    }
  }
  return out;
}

export const unifiedTranslations = [bot, demo, meta, postdocs, site, solutions, tech, whitepaper].reduce(
  (acc, grp) => deepMerge(acc, grp),
  baseTranslations
);

export default unifiedTranslations;
