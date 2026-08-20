// fix-code-blocks.mjs — convert <pre><code> to use dangerouslySetInnerHTML
// to prevent MDX from parsing code content as markdown
import { readFileSync, writeFileSync, readdirSync } from 'fs';
import { join } from 'path';

const DIR = 'src/content/docs';

// Escape special chars for JS template literal (backtick, ${, backslash)
function escapeTemplateLiteral(str) {
  return str
    .replace(/\\/g, '\\\\')
    .replace(/`/g, '\\`')
    .replace(/\${/g, '\\${');
}

function fixCodeBlocks(content) {
  // Match <pre><code>...</code></pre> with optional attributes
  const regex = /<pre><code>([\s\S]*?)<\/code><\/pre>/g;

  return content.replace(regex, (match, inner) => {
    const escaped = escapeTemplateLiteral(inner);
    return '<pre><code dangerouslySetInnerHTML={{__html: `' + escaped + '`}} /></pre>';
  });
}

const files = readdirSync(DIR).filter(f => f.endsWith('.mdx'));
for (const file of files) {
  const path = join(DIR, file);
  let content = readFileSync(path, 'utf-8');
  const fixed = fixCodeBlocks(content);
  writeFileSync(path, fixed);
  console.log('Fixed:', file);
}