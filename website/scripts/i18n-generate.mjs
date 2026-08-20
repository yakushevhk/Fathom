// i18n build-time generator
// After `astro build`, takes each EN page in dist/, and emits localized
// copies under dist/ru/<path>:
//   - substitutes text for every data-i18n / data-i18n-html / -aria / -placeholder key
//   - sets <html lang>, canonical + hreflang
//   - rewrites internal href="/..." to "/<lang>/..." and <a href> / link URLs
// Uses parse5 (available transitively) for robust HTML handling.
import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync, statSync } from 'fs';
import { join, dirname, relative, sep } from 'path';
import * as parse5 from 'parse5';

const DIST = 'dist';
const SRC = 'src';
const LANGUAGES = ['ru'];
const DEFAULT_LANG = 'en';
const SITE = 'https://fathom.uz';

// ---------- load dictionary ----------
import { translations as baseT } from '../src/i18n/translations.js';
import { readdirSync as _r } from 'fs';
const groupFiles = readdirSync(join(SRC, 'i18n/groups')).filter((f) => f.endsWith('.js'));
let groupT = {};
for (const f of groupFiles) {
  const m = await import('../src/i18n/groups/' + f);
  groupT = deepMerge(groupT, m.default || {});
}
function deepMerge(a, b) {
  const out = { ...a };
  for (const k in b) {
    if (b[k] && typeof b[k] === 'object' && !Array.isArray(b[k]) && a[k] && typeof a[k] === 'object' && !Array.isArray(a[k])) {
      out[k] = deepMerge(a[k], b[k]);
    } else out[k] = b[k];
  }
  return out;
}
const dict = deepMerge(baseT, groupT);
const flat = {};
(function walk(node, prefix) {
  for (const k in node) {
    const v = node[k];
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      if (typeof v.en === 'string') {
        flat[prefix ? prefix + '.' + k : k] = v;
      } else {
        walk(v, prefix ? prefix + '.' + k : k);
      }
    }
  }
})(dict, '');

function resolveKey(key, lang) {
  const n = flat[key];
  if (!n) return null;
  const v = n[lang];
  return v != null ? v : (n[DEFAULT_LANG] ?? null);
}

// ---------- file walking ----------
function collectHtmls(dir, base) {
  const out = [];
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    const b = join(base, e);
    if (statSync(p).isDirectory()) {
      if (e === 'ru') continue;
      out.push(...collectHtmls(p, b));
    } else if (e.endsWith('.html')) {
      out.push(b);
    }
  }
  return out;
}

// relative url path without leading index.html
function urlPath(absPath) {
  let rel = relative(DIST, absPath).replace(/\\/g, '/');
  if (rel.endsWith('index.html')) rel = rel.slice(0, -'index.html'.length);
  return '/' + rel;
}

// ---------- parse5 walk ----------
function walkNodes(node, fn) {
  fn(node);
  if (node.childNodes) for (const c of node.childNodes) walkNodes(c, fn);
  if (node.content) walkNodes(node.content, fn); // template
}

function getAttr(node, name) {
  return node.attrs ? node.attrs.find((a) => a.name === name) : undefined;
}
function setAttrVal(node, name, value) {
  const a = getAttr(node, name);
  if (a) a.value = value;
  else node.attrs.push({ name, value });
}
function rmAttr(node, name) {
  if (!node.attrs) return;
  node.attrs = node.attrs.filter((a) => a.name !== name);
}

// ---------- translate a page ----------
function translateHtml(html, lang, langRoot, pageUrl) {
  const doc = parse5.parse(html);
  // Determine translated-text nodes. Collect elements that are direct text containers.
  // We mutate node.attrs + replace text nodes.
  walkNodes(doc, (node) => {
    if (node.tagName === 'html') {
      setAttrVal(node, 'lang', lang);
      return;
    }
  });

  // validate data-i18n elements & replace text
  function processElement(node) {
    if (!node.tagName) return;
    const keys = node.attrs || [];
    const i18n = getAttr(node, 'data-i18n');
    if (i18n) {
      const val = resolveKey(i18n.value, lang);
      if (val != null && val !== false) {
        replaceText(node, val);
      }
      rmAttr(node, 'data-i18n');
    }
    const htmlKey = getAttr(node, 'data-i18n-html');
    if (htmlKey) {
      const val = resolveKey(htmlKey.value, lang);
      if (val != null) {
        node.childNodes = [parse5.parseFragment(val)];
      }
      rmAttr(node, 'data-i18n-html');
      rmAttr(node, 'data-i18n'); // safety
    }
    const ph = getAttr(node, 'data-i18n-placeholder');
    if (ph) {
      const val = resolveKey(ph.value, lang);
      if (val != null) setAttrVal(node, 'placeholder', val);
      rmAttr(node, 'data-i18n-placeholder');
    }
    const aria = getAttr(node, 'data-i18n-aria');
    if (aria) {
      const val = resolveKey(aria.value, lang);
      if (val != null) setAttrVal(node, 'aria-label', val);
      rmAttr(node, 'data-i18n-aria');
    }
    // rewrite href
    const href = getAttr(node, 'href');
    if (href) href.value = rewriteInternal(href.value, langRoot);
  }

  function replaceText(node, text) {
    // keep child inline codes? For simplicity replace all children with text node.
    const frag = parse5.parseFragment(text);
    node.childNodes = frag.childNodes;
  }

  walkNodes(doc, (node) => {
    if (node.tagName) processElement(node);
  });

  // insert canonical + hreflang into head
  const head = findTag(doc, 'head');
  if (head) {
    insertCanonicalAndHreflang(head, pageUrl, langRoot);
  }

  return parse5.serialize(doc);
}

function insertCanonicalAndHreflang(head, pageUrl, langRoot) {
  const oldLinks = head.childNodes.filter((n) => n.tagName === 'link' && ['canonical', 'alternate'].includes(getAttr(n, 'rel')?.value));
  head.childNodes = head.childNodes.filter((n) => !oldLinks.includes(n));
  const mk = (rel, href, ext) => {
    const l = { nodeName: 'link', tagName: 'link', attrs: [], childNodes: [] };
    l.attrs.push({ name: 'rel', value: rel });
    if (ext) { for (const k in ext) l.attrs.push({ name: k, value: ext[k] }); }
    l.attrs.push({ name: 'href', value: href });
    return l;
  };
  const alts = [
    mk('alternate', SITE + pageUrl, { hreflang: 'en' }),
    mk('alternate', SITE + '/ru' + pageUrl, { hreflang: 'ru' }),
  ];
  head.childNodes.push(mk('canonical', SITE + langRoot + pageUrl));
  for (const a of alts) head.childNodes.push(a);
}

function findTag(node, name) {
  let res = null;
  walkNodes(node, (n) => { if (!res && n.tagName === name) res = n; });
  return res;
}

function rewriteInternal(href, langRoot) {
  if (!href) return href;
  if (href.startsWith('http') || href.startsWith('//') || href.startsWith('mailto:') ||
      href.startsWith('tel:') || href.startsWith('#') || href.startsWith('data:') ||
      href.includes('google') || href.includes('fonts') || href.includes('github')) return href;
  if (href.startsWith('/assets/') || href.startsWith('/favicon') || href.startsWith('/_astro/')) return href;
  // avoid double prefix
  if (href.startsWith('/ru/')) return href;
  return langRoot + (href.startsWith('/') ? href : '/' + href);
}

// ---------- main ----------
const enHtmls = collectHtmls(DIST, DIST);
console.log('EN pages found:', enHtmls.length);

for (const file of enHtmls) {
  const html = readFileSync(file, 'utf-8');
  const path = urlPath(file); // e.g. "/docs/outreach/"
  for (const lang of LANGUAGES) {
    const langRoot = '/' + lang;
    // target file: dist/ru/<rel>
    const rel = relative(DIST, file).replace(/\\/g, '/');
    const outAbs = join(DIST, lang, rel);
    const translated = translateHtml(html, lang, langRoot, path);
    mkdirSync(dirname(outAbs), { recursive: true });
    writeFileSync(outAbs, translated);
    console.log('  ->', lang, path);
  }
}
console.log('done');
