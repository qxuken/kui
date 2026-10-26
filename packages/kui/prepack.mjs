// What the tarball carries from the repository besides the code: the prop
// reference the README points at, the task index beside it, the changelog,
// the ADRs the doc comments in `index.d.ts` / `jsx-runtime.d.ts` cite as
// `docs/adr/...` — copied under the same paths, so a citation read out of
// `node_modules` resolves where it points (backlog F18) — and the book's
// chapters as text under `docs/book/` (ADR 0071; Kit's component reference
// is `kit/reference.md`, which scripts/gen.nu copies). `npm pack` and
// `npm publish` run this; the copies are git-ignored.
import { cpSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
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
// that resolves in only one of the two (backlog F31).
writeFileSync(
  here('howto.md'),
  readFileSync(root('docs/howto.md'), 'utf8')
    .replaceAll('](../CHANGELOG.md', '](CHANGELOG.md')
    .replaceAll('](adr/', '](docs/adr/')
    .replaceAll('](book/src/', '](docs/book/'),
);
mkdirSync(here('docs/adr'), { recursive: true });
for (const f of readdirSync(root('docs/adr'))) {
  if (f.endsWith('.md')) cpSync(root(`docs/adr/${f}`), here(`docs/adr/${f}`));
}

// The book's chapters, text only: each mdBook `{{#include file:anchor}}`
// (or `rustdoc_include`) replaced by the lines it names, so a chapter reads
// whole without the examples beside it — what the runner carries, too
// (crates/kui-cli/build.rs).
const book = root('docs/book/src');
mkdirSync(here('docs/book'), { recursive: true });
for (const f of readdirSync(book)) {
  if (!f.endsWith('.md')) continue;
  const text = readFileSync(join(book, f), 'utf8').replace(
    /\{\{#(?:rustdoc_)?include ([^}]+)\}\}/g,
    (_, arg) => {
      const [file, anchor] = arg.trim().split(':');
      const lines = readFileSync(join(book, file), 'utf8').split('\n');
      let picked = lines;
      if (anchor) {
        const start = lines.findIndex((l) => l.includes(`ANCHOR: ${anchor}`));
        const end = lines.findIndex((l, i) => i > start && l.includes(`ANCHOR_END: ${anchor}`));
        picked = start < 0 ? [] : lines.slice(start + 1, end < 0 ? undefined : end);
      }
      return picked.filter((l) => !l.includes('ANCHOR')).join('\n');
    },
  );
  writeFileSync(here(`docs/book/${f}`), text);
}
