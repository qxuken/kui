// What the tarball carries from the repository besides the code: the prop
// reference the README points at, the task index beside it, the changelog,
// and the ADRs the doc comments in `index.d.ts` / `jsx-runtime.d.ts` cite as
// `docs/adr/...` — copied under the same paths, so a citation read out of
// `node_modules` resolves where it points (backlog F18). `npm pack` and
// `npm publish` run this; the copies are git-ignored.
import { cpSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const root = (p) => fileURLToPath(new URL(`../../${p}`, import.meta.url));

cpSync(root('docs/props.md'), here('props.md'));
cpSync(root('CHANGELOG.md'), here('CHANGELOG.md'));
// `howto.md` links its three targets, and the two trees put them in
// different places: it lives in `docs/` in the repository (so `props.md` is
// a sibling, the changelog is up one and the ADRs are under `adr/`) and at
// the package root here (so the changelog is a sibling and the ADRs are
// under `docs/adr/`). One rewrite at copy time, rather than a set of links
// that resolves in only one of the two (backlog F30).
writeFileSync(
  here('howto.md'),
  readFileSync(root('docs/howto.md'), 'utf8')
    .replaceAll('](../CHANGELOG.md', '](CHANGELOG.md')
    .replaceAll('](adr/', '](docs/adr/'),
);
mkdirSync(here('docs/adr'), { recursive: true });
for (const f of readdirSync(root('docs/adr'))) {
  if (f.endsWith('.md')) cpSync(root(`docs/adr/${f}`), here(`docs/adr/${f}`));
}
