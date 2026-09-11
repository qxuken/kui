// The Node half of the headless smoke round: every example package.json's
// `kui.headless` roster names, run with `--headless`, in order, stopping at
// the first that fails. The roster is the one list — the `smoke` binary
// reads `kui.windowed` from the same file for the windowed round, and the
// harness's pin test checks both against the files on disk — so an example
// added here cannot be left out of a round by forgetting a second list.
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(join(here, '..', 'package.json'), 'utf8'));
for (const entry of pkg.kui.headless) {
  const file = join(here, '..', 'dist', `${entry}.mjs`);
  const r = spawnSync(process.execPath, [file, '--headless'], { stdio: 'inherit' });
  if (r.status !== 0) {
    console.error(`${entry}: headless drive failed (exit ${r.status})`);
    process.exit(r.status ?? 1);
  }
}
