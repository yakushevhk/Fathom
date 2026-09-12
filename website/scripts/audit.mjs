// Dev-only audit: broken links, missing i18n keys, missing ru translations,
// SEO head gaps. Run: node scripts/audit.mjs
import { readFileSync, readdirSync, statSync } from 'fs';
import { join, relative } from 'path';

const ROOT = process.cwd();
const PAGES = join(ROOT, 'src/pages');
const CONTENT = join(ROOT, 'src/content/docs');
const ASSET_RE = /\.(pdf|png|jpe?g|svg|webp|gif|mp4|vtt|xml|txt|ico|json|webmanifest|webm|mov)$/i;

function walk(dir, out = []) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}
const rel = (p) => relative(ROOT, p);

const pageFiles = walk(PAGES).filter((f) => f.endsWith('.astro'));
const docFiles = walk(CONTENT);
const allFiles = [...pageFiles, ...docFiles].filter((f) => !f.includes('[...slug]'));

// ---------- routes ----------
const routes = new Set(['/']);
for (const f of pageFiles) {
  if (f.includes('[...slug]')) continue;
  let r = rel(f).replace(/^src\/pages/, '').replace(/\.astro$/, '').replace(/\/index$/, '');
  routes.add(r === '' ? '/' : r);
}
for (const f of docFiles) routes.add('/docs/' + f.split('/').pop().replace(/\.mdx?$/, ''));

// ---------- 1. broken internal links ----------
const broken = new Map();
for (const f of allFiles) {
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(/(?:href|src)="(\/[^"#?]*)([#?][^"]*)?"/g)) {
    let h = m[1];
    if (h.endsWith('/') && h !== '/') h = h.slice(0, -1);
    if (ASSET_RE.test(h)) continue;
    if (/^\/(assets|_astro|ru)\//.test(h) || h === '/ru') continue;
    if (h.startsWith('//')) continue;
    if (!routes.has(h)) {
      if (!broken.has(h)) broken.set(h, new Set());
      broken.get(h).add(rel(f));
    }
  }
}

// ---------- 2. i18n keys ----------
const usedKeys = new Map(); // key -> files
for (const f of allFiles) {
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(/data-i18n(?:-html|-aria|-placeholder)?="([^"]+)"/g)) {
    if (!usedKeys.has(m[1])) usedKeys.set(m[1], new Set());
    usedKeys.get(m[1]).add(rel(f));
  }
}
const missingKeys = [];
const missingRu = [];
{
  const { translations } = await import('../src/i18n/translations.js');
  const groupFiles = readdirSync(join(ROOT, 'src/i18n/groups')).filter((x) => x.endsWith('.js'));
  const flat = {};
  const flatten = (node, prefix) => {
    for (const k in node) {
      const v = node[k];
      if (v && typeof v === 'object' && !Array.isArray(v)) {
        if (typeof v.en === 'string') flat[prefix ? prefix + '.' + k : k] = v;
        else flatten(v, prefix ? prefix + '.' + k : k);
      }
    }
  };
  flatten(translations, '');
  for (const gf of groupFiles) {
    const mod = await import('../src/i18n/groups/' + gf);
    flatten(mod.default || {}, '');
  }
  for (const [k, files] of usedKeys) {
    if (!flat[k]) missingKeys.push([k, [...files]]);
    else if (!flat[k].ru || !String(flat[k].ru).trim()) missingRu.push([k, [...files]]);
  }
}

// ---------- 3. SEO head ----------
const seo = [];
const DEFAULT_DESC = 'Fathom is a self-hosted Rust runtime for autonomous remote AI workers';
for (const f of pageFiles) {
  const src = readFileSync(f, 'utf8');
  const tm = src.match(/<Layout[^>]*\stitle="([^"]*)"/) || src.match(/title="([^"]*)"/);
  const title = tm ? tm[1] : null;
  const hasDesc = /<Layout[\s\S]{0,400}?description="/.test(src) || /\bdescription="[^"]{40,}"/.test(src);
  const h1 = (src.match(/<h1[\s>]/g) || []).length;
  const agentPage = /AgentPage\.astro/.test(src);
  seo.push({ file: rel(f), title, len: title ? title.length : 0, hasDesc, h1, agentPage });
}

// ---------- 4. alt / a11y ----------
const imgNoAlt = [];
const btnNoName = [];
for (const f of allFiles) {
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(/<img\b[^>]*>/g)) {
    if (!/\balt=/.test(m[0])) imgNoAlt.push([rel(f), m[0].slice(0, 90)]);
  }
  for (const m of src.matchAll(/<button\b([^>]*)>([\s\S]{0,120}?)<\/button>/g)) {
    const attrs = m[1];
    const inner = m[2].replace(/<[^>]*>/g, '').trim();
    if (!inner && !/aria-label/.test(attrs) && !/aria-labelledby/.test(attrs)) {
      btnNoName.push([rel(f), m[0].slice(0, 90).replace(/\s+/g, ' ')]);
    }
  }
}

// ---------- report ----------
const out = [];
out.push('## BROKEN INTERNAL LINKS (' + broken.size + ')');
for (const [h, files] of [...broken].sort()) out.push(`- \`${h}\` ← ${[...files].slice(0, 5).join(', ')}`);

out.push('\n## I18N KEYS MISSING FROM DICTIONARY (' + missingKeys.length + ')');
for (const [k, files] of missingKeys.sort()) out.push(`- \`${k}\` ← ${files.slice(0, 3).join(', ')}`);

out.push('\n## I18N KEYS WITHOUT RU TRANSLATION (' + missingRu.length + ')');
for (const [k, files] of missingRu.sort()) out.push(`- \`${k}\` ← ${files.slice(0, 2).join(', ')}`);

out.push('\n## SEO: no own description');
for (const r of seo.filter((x) => !x.hasDesc && !x.agentPage)) out.push(`- ${r.file}  (title len ${r.len})`);

out.push('\n## SEO: title too long (>65) or too short (<20)');
for (const r of seo.filter((x) => x.title && (x.len > 65 || x.len < 20))) out.push(`- ${r.file}  ${r.len}: ${r.title}`);

out.push('\n## A11Y: h1 count != 1');
for (const r of seo.filter((x) => x.h1 !== 1)) out.push(`- ${r.file}  h1=${r.h1}`);

out.push('\n## A11Y: <img> without alt (' + imgNoAlt.length + ')');
for (const [f, tag] of imgNoAlt) out.push(`- ${f}  ${tag}`);

out.push('\n## A11Y: <button> without accessible name (' + btnNoName.length + ')');
for (const [f, tag] of btnNoName) out.push(`- ${f}  ${tag}`);

console.log(out.join('\n'));
