// migrate-complex-docs.mjs — migrate remaining docs pages with data arrays
import { readFileSync, writeFileSync, unlinkSync, existsSync, mkdirSync } from 'fs';

const SRC = 'src/pages/docs';
const DST = 'src/content/docs';

const pages = ['memory', 'personalization', 'outreach', 'cli', 'architecture', 'api', 'recipes'];

function escapeTemplateLiteral(str) {
  return str.replace(/\\/g, '\\\\').replace(/`/g, '\\`').replace(/\${/g, '\\${');
}

function convertPage(id) {
  const filePath = `${SRC}/${id}.astro`;
  if (!existsSync(filePath)) {
    console.log(`  SKIP: ${id} — file not found`);
    return false;
  }
  
  let content = readFileSync(filePath, 'utf-8');
  
  // Extract frontmatter data arrays (const name = [...])
  const dataArrays = [];
  const fmMatch = content.match(/^---\n([\s\S]*?)\n---/);
  if (fmMatch) {
    const fm = fmMatch[1];
    // Find all const declarations with array literals
    const arrayRegex = /const\s+(\w+)\s*=\s*\[([\s\S]*?)\];/g;
    let match;
    while ((match = arrayRegex.exec(fm)) !== null) {
      dataArrays.push({ name: match[1], value: match[0] });
    }
  }
  
  // Extract the body (between <DocsLayout ...> and </DocsLayout>)
  const bodyMatch = content.match(/<DocsLayout[^>]*>([\s\S]*)<\/DocsLayout>/);
  if (!bodyMatch) {
    console.log(`  FAIL: ${id} — no DocsLayout wrapper`);
    return false;
  }
  
  let body = bodyMatch[1];
  
  // Remove <style> block
  body = body.replace(/<style>[\s\S]*?<\/style>/g, '');
  
  // Preserve the data arrays as export const in MDX
  let exportVars = '';
  for (const arr of dataArrays) {
    exportVars += `export ${arr.value}\n\n`;
  }
  
  // Convert code blocks to dangerouslySetInnerHTML to prevent MDX parsing issues
  body = body.replace(/<pre><code>([\s\S]*?)<\/code><\/pre>/g, (match, inner) => {
    const escaped = escapeTemplateLiteral(inner);
    return '<pre><code dangerouslySetInnerHTML={{__html: `' + escaped + '`}} /></pre>';
  });
  
  // Create frontmatter
  const title = id.charAt(0).toUpperCase() + id.slice(1);
  const frontmatter = `---
title: "${title} — Fathom Research"
sidebarId: "${id}"
---
  
`;
  
  mkdirSync(DST, { recursive: true });
  const mdxContent = frontmatter + exportVars + body.trim() + '\n';
  writeFileSync(`${DST}/${id}.mdx`, mdxContent);
  console.log(`  OK: ${id} (${mdxContent.length} chars, ${dataArrays.length} data arrays)`);
  return true;
}

console.log('Migrating docs pages to MDX...');
for (const p of pages) {
  const ok = convertPage(p);
  if (ok) {
    // Delete the .astro file
    unlinkSync(`${SRC}/${p}.astro`);
    console.log(`  DELETED: ${SRC}/${p}.astro`);
  }
}
console.log('Done!');