// fix-set-html-v2.mjs — handle both backtick and single-quote set:html
import { readFileSync, writeFileSync, readdirSync } from 'fs';
import { join } from 'path';

const DIR = 'src/content/docs';

function fixSetHtml(content) {
  // Match <pre><code set:html={'...'}></code></pre> (single-quoted content)
  // The content is a string literal with escape sequences like \n
  // We need to match the literal content and convert it
  let result = content;
  
  // Handle single-quoted variant: set:html={'...'}
  const regex1 = /<pre><code\s+set:html=\{'(.*?)'\}><\/code><\/pre>/gs;
  result = result.replace(regex1, (match, inner) => {
    // Convert the literal escape sequences to actual characters
    // \n → newline, \t → tab, \" → ", etc.
    const decoded = inner
      .replace(/\\n/g, '\n')
      .replace(/\\t/g, '\t')
      .replace(/\\"/g, '"');
    
    const escaped = decoded
      .replace(/\\/g, '\\\\')
      .replace(/`/g, '\\`')
      .replace(/\${/g, '\\${');
    
    return '<pre><code dangerouslySetInnerHTML={{__html: `' + escaped + '`}} /></pre>';
  });
  
  // Handle already-backtick variant (from earlier conversion): set:html={`...`}
  const regex2 = /<pre><code\s+set:html=\{`([\s\S]*?)`\}><\/code><\/pre>/g;
  result = result.replace(regex2, (match, inner) => {
    const escaped = inner
      .replace(/\\/g, '\\\\')
      .replace(/`/g, '\\`')
      .replace(/\${/g, '\\${');
    return '<pre><code dangerouslySetInnerHTML={{__html: `' + escaped + '`}} /></pre>';
  });
  
  return result;
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