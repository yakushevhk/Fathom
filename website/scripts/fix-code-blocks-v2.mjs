// fix-code-blocks-v2.mjs — add missing </pre> tags after dangerouslySetInnerHTML
import { readFileSync, writeFileSync, readdirSync } from 'fs';
import { join } from 'path';

const DIR = 'src/content/docs';

const files = readdirSync(DIR).filter(f => f.endsWith('.mdx'));
for (const file of files) {
  const path = join(DIR, file);
  let content = readFileSync(path, 'utf-8');
  
  // Fix: replace <pre><code dangerouslySetInnerHTML={{...}} /> with <pre><code ... /></pre>
  // if the </pre> is missing
  content = content.replace(
    /<pre><code dangerouslySetInnerHTML=\{\{__html: `([\s\S]*?)`\}\} \/>/g,
    (match, inner) => {
      return '<pre><code dangerouslySetInnerHTML={{__html: `' + inner + '`}} /></pre>';
    }
  );
  
  writeFileSync(path, content);
  console.log('Fixed:', file);
}