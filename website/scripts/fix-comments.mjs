// fix-comments.mjs — convert HTML comments <!-- --> to MDX {/* */} comments
import { readFileSync, writeFileSync, readdirSync } from 'fs';
import { join } from 'path';

const DIR = 'src/content/docs';

function fixComments(content) {
  // Replace <!-- comment --> with {/* comment */}
  return content.replace(/<!--([\s\S]*?)-->/g, '{/*$1*/}');
}

const files = readdirSync(DIR).filter(f => f.endsWith('.mdx'));
for (const file of files) {
  const path = join(DIR, file);
  let content = readFileSync(path, 'utf-8');
  const before = content;
  content = fixComments(content);
  if (content !== before) {
    writeFileSync(path, content);
    console.log('Fixed:', file);
  }
}
console.log('Done');