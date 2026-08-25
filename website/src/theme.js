// Theme toggle: light / true dark. Initial value is set by an inline
// anti-FOUC script in each layout <head>; this wires the toggle button
// and follows OS preference when the user has no explicit choice.
function updateMetaTheme(theme) {
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) {
    meta.setAttribute('content', theme === 'dark' ? '#0a0a0a' : '#fcfbfa');
  }
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

  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  mq.addEventListener?.('change', (e) => {
    if (!localStorage.getItem('fathom-theme')) {
      const next = e.matches ? 'dark' : 'light';
      root.dataset.theme = next;
      updateMetaTheme(next);
    }
  });
}

