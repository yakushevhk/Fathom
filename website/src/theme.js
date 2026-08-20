// Theme toggle: light / true dark. Initial value is set by an inline
// anti-FOUC script in each layout <head>; this wires the toggle button
// and follows OS preference when the user has no explicit choice.
export function initTheme() {
  const root = document.documentElement;

  document.querySelectorAll('[data-theme-toggle]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const next = root.dataset.theme === 'dark' ? 'light' : 'dark';
      root.dataset.theme = next;
      localStorage.setItem('fathom-theme', next);
    });
  });

  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  mq.addEventListener?.('change', (e) => {
    if (!localStorage.getItem('fathom-theme')) {
      root.dataset.theme = e.matches ? 'dark' : 'light';
    }
  });
}
