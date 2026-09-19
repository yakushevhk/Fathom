// NOTE: this module carries the full translation dictionary (heavy). Layouts
// should import language navigation from './langnav.js' instead — this file is
// only loaded lazily in dev to translate untranslated pages in place.
import { LANGS, DEFAULT_LANG, LANGS_SET } from './langnav.js';
import { unifiedTranslations as translations } from './unified.js';

export { LANGS, DEFAULT_LANG, LANGS_SET, langUrl, currentLangFromPath, getLang, applyLanguage, initI18n } from './langnav.js';
import { langUrl, currentLangFromPath, getLang, applyLanguage, initI18n } from './langnav.js';

export function resolveKey(key, lang) {
  // Deep resolve from the module-loaded dictionary. Used by the dev-mode
  // in-place translator. Returns null if unknown.
  const parts = key.split('.');
  let obj = translations;
  for (const part of parts) {
    if (obj == null || typeof obj !== 'object') return null;
    obj = obj[part];
  }
  if (obj && typeof obj === 'object' && lang in obj) return obj[lang];
  return null;
}

// Dev-mode helper: translate all remaining data-i18n elements in place.
// Loaded lazily by langnav.initI18n when untranslated attributes are present.
export function translateDomInPlace(lang) {
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const v = resolveKey(el.getAttribute('data-i18n'), lang);
    if (v != null) el.textContent = String(v);
  });
  document.querySelectorAll('[data-i18n-html]').forEach((el) => {
    const v = resolveKey(el.getAttribute('data-i18n-html') || el.getAttribute('data-i18n'), lang);
    if (v != null) el.innerHTML = String(v);
  });
  document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
    const v = resolveKey(el.getAttribute('data-i18n-placeholder'), lang);
    if (v != null) el.setAttribute('placeholder', String(v));
  });
  document.querySelectorAll('[data-i18n-aria]').forEach((el) => {
    const v = resolveKey(el.getAttribute('data-i18n-aria'), lang);
    if (v != null) el.setAttribute('aria-label', String(v));
  });
}
