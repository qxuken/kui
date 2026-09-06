// What the tarball carries from the repository besides the code: the prop
// reference the README points at, the changelog, and the ADRs the doc
// comments in `index.d.ts` / `jsx-runtime.d.ts` cite as `docs/adr/...` —
// copied under the same paths, so a citation read out of `node_modules`
// resolves where it points (backlog F18). `npm pack` and `npm publish` run
// this; the copies are git-ignored.
import { cpSync, mkdirSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const root = (p) => fileURLToPath(new URL(`../../${p}`, import.meta.url));

cpSync(root('docs/props.md'), here('props.md'));
cpSync(root('CHANGELOG.md'), here('CHANGELOG.md'));
mkdirSync(here('docs/adr'), { recursive: true });
for (const f of readdirSync(root('docs/adr'))) {
  if (f.endsWith('.md')) cpSync(root(`docs/adr/${f}`), here(`docs/adr/${f}`));
}
