import { LANGS, DEFAULT_LANG } from './translations.js';

const LANGS_SET = ['en', 'ru'];

// Language is now derived from the URL path:
//   /            -> en (default)
//   /ru/...      -> ru
export function currentLangFromPath() {
  const parts = location.pathname.split('/').filter(Boolean);
  if (parts[0] && LANGS_SET.includes(parts[0])) return parts[0];
  return DEFAULT_LANG;
}

// Build a localized URL for a given lang from the current location.
export function langUrl(pathName, toLang) {
  if (toLang === DEFAULT_LANG) {
    // strip the existing /ru prefix
    const parts = pathName.split('/').filter(Boolean);
    if (parts[0] && LANGS_SET.includes(parts[0])) {
      return '/' + parts.slice(1).join('/');
    }
    return pathName;
  }
  return '/' + toLang + (pathName === '/' ? '' : pathName.startsWith('/') ? pathName : '/' + pathName);
}

export function getLang() {
  if (typeof window === 'undefined') return DEFAULT_LANG;
  return currentLangFromPath();
}

export function resolveKey(key, lang) {
  // Deep resolve from the module-loaded dictionary. Maintained for JS fallback
  // in dev mode (no build-time translation). Returns null if unknown.
  return null;
}

// Mark active lang buttons + label. Does NOT rewrite the page text — in a
// production `astro build` the HTML is already translated server-side.
function updateActive(lang) {
  document.querySelectorAll('[data-lang-option]').forEach((btn) => {
    const optionLang = btn.getAttribute('data-lang-option');
    btn.classList.toggle('active', optionLang === lang);
  });
  const current = document.querySelector('[data-lang-current]');
  if (current) current.textContent = (LANGS[lang] || {}).label || lang.toUpperCase();
  document.documentElement.lang = lang;
}

export function applyLanguage(lang) {
  // In dev (astro dev) the page still contains data-i18n attrs; translate them
  // client-side so navigation feels local. In build output they were already
  // consumed by scripts/i18n-generate.mjs, so this is a no-op.
  updateActive(lang);
}

export function initI18n() {
  const lang = getLang();
  updateActive(lang);

  // language options are navigation links to the same page in another language
  document.querySelectorAll('[data-lang-option]').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      const toLang = btn.getAttribute('data-lang-option');
      if (toLang === lang) {
        closeDropdown();
        return;
      }
      location.assign(langUrl(location.pathname, toLang));
    });
  });

  const toggle = document.querySelector('[data-lang-toggle]');
  const dropdown = document.querySelector('[data-lang-dropdown]');
  if (toggle && dropdown) {
    toggle.addEventListener('click', (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropdown.classList.toggle('open');
    });
    document.addEventListener('click', (e) => {
      if (!dropdown.contains(e.target)) dropdown.classList.remove('open');
    });
  }
}

function closeDropdown() {
  const dropdown = document.querySelector('[data-lang-dropdown]');
  if (dropdown) dropdown.classList.remove('open');
}
