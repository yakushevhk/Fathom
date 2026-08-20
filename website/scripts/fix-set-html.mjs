// fix-set-html.mjs — convert set:html directives to dangerouslySetInnerHTML
import { readFileSync, writeFileSync, readdirSync } from 'fs';
import { join } from 'path';

const DIR = 'src/content/docs';

function fixSetHtml(content) {
  // Match <pre><code set:html={'...'}></code></pre>
  // The content is a template literal starting with {' and ending with '}
  const regex = /<pre><code\s+set:html=\{`([\s\S]*?)`\}><\/code><\/pre>/g;
  
  return content.replace(regex, (match, inner) => {
    // Escape the content for template literal
    const escaped = inner
      .replace(/\\/g, '\\\\')
      .replace(/`/g, '\\`')
      .replace(/\${/g, '\\${');
    return '<pre><code dangerouslySetInnerHTML={{__html: `' + escaped + '`}} /></pre>';
  });
}

const files = readdirSync(DIR).filter(f => f.endsWith('.mdx'));
for (const file of files) {
  const path = join(DIR, file);
  let content = readFileSync(path, 'utf-8');
  const before = content;
  content = fixSetHtml(content);
  if (content !== before) {
    writeFileSync(path, content);
    console.log('Fixed:', file);
  }
}
console.log('Done');