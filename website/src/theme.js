// Theme toggle: light / true dark. Initial value is set by an inline
// anti-FOUC script in each layout <head>; this wires the toggle button
// and follows OS preference when the user has no explicit choice.
function updateMetaTheme(theme) {
  const color = theme === 'dark' ? '#0a0a0a' : '#fcfbfa';
  document.querySelectorAll('meta[name="theme-color"]').forEach((meta) => {
    meta.setAttribute('content', color);
  });
  document.documentElement.style.colorScheme = theme;
}

export function initTheme() {
  const root = document.documentElement;

  document.querySelectorAll('[data-theme-toggle]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const next = root.dataset.theme === 'dark' ? 'light' : 'dark';
      root.dataset.theme = next;
      localStorage.setItem('fathom-theme', next);
      updateMetaTheme(next);
    });
  });

  // Cross-tab synchronization
  window.addEventListener('storage', (e) => {
    if ((e.key === 'fathom-theme' || e.key === 'Fathom-theme') && e.newValue) {
      root.dataset.theme = e.newValue;
      updateMetaTheme(e.newValue);
    }
  });

  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  mq.addEventListener?.('change', (e) => {
    if (!localStorage.getItem('fathom-theme') && !localStorage.getItem('Fathom-theme')) {
      const next = e.matches ? 'dark' : 'light';
      root.dataset.theme = next;
      updateMetaTheme(next);
    }
  });
}

