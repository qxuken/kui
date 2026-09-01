// Regenerates the schema-derived prop types in jsx-runtime.d.ts from the
// addon's protocol() export (the PROPS table in crates/kui-node/src/schema.rs).
// Run after changing the schema: node gen-types.mjs  (or: npm run gen)
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const native = createRequire(import.meta.url)('./native.cjs');
const { prop } = native.protocol();

const TS_BY_KIND = {
  f32: 'number',
  color: 'ColorProp',
  flag: 'boolean',
  sizing: 'SizingProp',
  msg: 'Msg',
};

function field([name, def]) {
  const ts = def.kind === 'enum' ? def.values.map((v) => `'${v}'`).join(' | ') : TS_BY_KIND[def.kind];
  return `  /** ${def.doc} */\n  ${name}?: ${ts};`;
}

const entries = Object.entries(prop).filter(([, d]) => d.kind !== 'custom');
const spec = entries.filter(([, d]) => d.target === 'spec');
const style = entries.filter(([, d]) => d.target === 'style');

const block = `// -- generated from the addon's prop schema; edit schema.rs, then \`npm run gen\` --
export interface GeneratedSpecProps {
${spec.map(field).join('\n')}
}

export interface GeneratedStyleProps {
${style.map(field).join('\n')}
}
// -- end generated --`;

const path = new URL('./jsx-runtime.d.ts', import.meta.url);
const src = readFileSync(path, 'utf8');
const re = /\/\/ -- generated from the addon's prop schema[\s\S]*?\/\/ -- end generated --/;
if (!re.test(src)) throw new Error('generated-region markers not found in jsx-runtime.d.ts');
writeFileSync(path, src.replace(re, block));
console.log(`jsx-runtime.d.ts: ${spec.length} spec + ${style.length} style props generated`);
