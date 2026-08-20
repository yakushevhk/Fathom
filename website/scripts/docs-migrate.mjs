// docs-migrate.mjs — convert Astro docs pages to MDX content collection entries
import { readFileSync, writeFileSync, mkdirSync } from 'fs';

const SRC = 'src/pages/docs';
const DST = 'src/content/docs';

// Manually handle pages with set:html or other Astro-specific expressions
const SPECIAL_FIXES = {
  'quickstart': (body) => {
    // Replace the set:html curl command with raw HTML
    const oldTag = /<pre><code\s+set:html=\{.*?\}><\/code><\/pre>/;
    const newTag = `<pre><code>curl -X POST http://localhost:8080/api/v1/sessions \\
  -H <span class="code-string">"Content-Type: application/json"</span> \\
  -d <span class="code-string">'{"query": "Find AI startups in London"}'</span></code></pre>`;
    return body.replace(oldTag, newTag);
  },
};

function convert(id) {
  const filePath = `${SRC}/${id}.astro`;
  let content = readFileSync(filePath, 'utf-8');
  
  const match = content.match(/<DocsLayout[^>]*>([\s\S]*)<\/DocsLayout>/);
  if (!match) {
    console.error(`  FAIL: ${id} — no DocsLayout wrapper`);
    return;
  }
  
  let body = match[1];
  
  // Remove <style> block
  body = body.replace(/<style>[\s\S]*?<\/style>/g, '');
  
  // Apply special fixes
  if (SPECIAL_FIXES[id]) {
    body = SPECIAL_FIXES[id](body);
  }
  
  // Check for remaining Astro expressions that can't be converted
  const astroExpr = body.match(/\{[^}]*\}/);
  if (astroExpr && !body.includes('data-i18n')) {
    console.error(`  FAIL: ${id} — has unhandled Astro expression: ${astroExpr[0].slice(0, 60)}`);
    return;
  }
  
  // Create frontmatter
  const title = id.charAt(0).toUpperCase() + id.slice(1);
  const frontmatter = `---
title: "${title} — Fathom Research"
sidebarId: "${id}"
---
`;
  
  mkdirSync(DST, { recursive: true });
  writeFileSync(`${DST}/${id}.mdx`, frontmatter + body.trim() + '\n');
  console.log(`  OK: ${id} (${body.length} chars)`);
}

// Pages to convert
const pages = [
  'quickstart', 'configuration', 'tools',
  // 'personalization', 'recipes',  // have data arrays
  // 'api', 'architecture', 'cli', 'memory', 'outreach',  // complex data arrays
];

for (const p of pages) {
  convert(p);
}