// Regenerates everything derived from the Rust side, between generated markers:
//   - jsx-runtime.d.ts   the TS prop types, from the prop schema
//                        (`kui_core::schema::PROPS`, via the addon's protocol())
//   - ../../docs/props.md the cross-binding reference (JSX / Lua / C names),
//                        with the env reading and the warning codes
//                        (`kui_core::diag::WARNINGS`)
//   - index.d.ts         the `WarningCode` union, from the same table, the
//                        `AccessRole` / `AccessAction` unions from the
//                        core's `Role::ALL` / `AccessAction::ALL`, and the
//                        addon's own surface, from the `#[napi]`
//                        attributes in crates/kui-node/src/lib.rs
//   - jsx-runtime.d.ts   also the `MenuItemRole` union, from `MenuRole::ALL`
// Run after changing either: npm run gen. CI fails on stale output.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

// The addon is built before it is loaded: the schema tables below come from
// the addon `native.cjs` resolves, and loading it first meant every run
// described the *previous* build - a schema row added in Rust reached
// docs/props.md one `npm run gen` late while index.d.ts's `#[napi]` half,
// read from the type defs the build writes, was current (found while
// building backlog C17).
const defsPath = new URL('../../target/napi-type-defs/kui-node', import.meta.url);
let defsSrc;
try {
  defsSrc = readFileSync(defsPath, 'utf8');
} catch {
  // The file is written while the addon compiles, so a target dir that is
  // up to date has it already. If it went missing on its own, a plain
  // `cargo build` says "Fresh" and writes nothing; NAPI_FORCE_BUILD_KUI_NODE
  // is the env var napi-build declares `rerun-if-env-changed` on, so a value
  // that has never been seen before re-runs the build script and the macro.
  console.log('no napi type defs yet - rebuilding the addon to emit them');
  execFileSync('cargo', ['build', '-p', 'kui-node', '--release'], {
    cwd: fileURLToPath(new URL('../../', import.meta.url)),
    env: { ...process.env, NAPI_FORCE_BUILD_KUI_NODE: String(Date.now()) },
    stdio: 'inherit',
  });
  defsSrc = readFileSync(defsPath, 'utf8');
}

const native = createRequire(import.meta.url)('./native.cjs');
const {
  prop, elements, events, resources, warnings, env, theme, metrics, doors,
  accessRoles, accessActions, menuRoles, editKeys, mouseButtons,
} = native.protocol();

const TS_BY_KIND = {
  // A length token resolves to a number in the core, so every f32 row
  // takes one (ADR 0027).
  f32: 'LengthProp',
  color: 'ColorProp',
  flag: 'boolean',
  sizing: 'SizingProp',
  min: 'MinProp',
  msg: 'AppMsg',
  // A tag may be null: the behaviour without a tag on its events.
  tag: 'AppMsg | null',
  str: 'string',
  resource: 'string',
  keyframes: 'KeyframeProp[]',
  enter: 'EnterProp',
};

function field([name, def]) {
  const ts = def.kind === 'enum' ? def.values.map((v) => `'${v}'`).join(' | ') : TS_BY_KIND[def.kind];
  return `  /** ${def.doc} */\n  ${name}?: ${ts};`;
}

const entries = Object.entries(prop).filter(([, d]) => d.kind !== 'custom');
const spec = entries.filter(([, d]) => d.target === 'spec');
const style = entries.filter(([, d]) => d.target === 'style');
const custom = Object.entries(prop).filter(([, d]) => d.kind === 'custom');

// ---------------------------------------------------------------- types --

const block = `// -- generated from the addon's prop schema; edit schema.rs, then \`npm run gen\` --
export interface GeneratedSpecProps {
${spec.map(field).join('\n')}
}

export interface GeneratedStyleProps {
${style.map(field).join('\n')}
}
// -- end generated --`;

// The stock button reads only the rows it names (`BUTTON_ROWS_JSX`, the
// list the encoder admits and drops the rest with an `unknown-prop`), so
// `ButtonProps` has to be exactly those and not the whole schema. Written
// by hand it fell behind twice - `description` was a row the type rejected
// until alpha.9, `accent` until alpha.10, each found by an app whose tsc
// said no to what docs/props.md advertised (backlog F24, then F37) - so the
// heritage clause comes off the same list. A row added in schema.rs is a
// prop here, and the CI step that reruns this generator and diffs the file
// is what says so the third time.
//
// `key` is `Keyed`, which the interface extends already; every other row is
// a schema prop (`GeneratedSpecProps`) or one of a composite's JSX
// spellings (`CustomSpecProps`), and a row that is neither stops the run
// rather than emitting a `Pick` of a name TypeScript has never heard of.
const specNames = new Set(spec.map(([name]) => name));
const customNames = new Set(custom.flatMap(([, d]) => d.names));
const buttonRows = elements.find((e) => e.name === 'button')?.rows;
if (!buttonRows) throw new Error('the button element admits every row; ButtonProps expects a list');
const fromSpec = [];
const fromCustom = [];
for (const row of buttonRows) {
  if (row === 'key') continue;
  if (specNames.has(row)) fromSpec.push(row);
  else if (customNames.has(row)) fromCustom.push(row);
  else throw new Error(`button row \`${row}\` is in neither the prop schema nor the composites`);
}
const picked = (names) => names.map((n) => `'${n}'`).join(' | ');
const heritage = ['Keyed'];
if (fromSpec.length) heritage.push(`Pick<GeneratedSpecProps, ${picked(fromSpec)}>`);
if (fromCustom.length) heritage.push(`Pick<CustomSpecProps, ${picked(fromCustom)}>`);
const buttonBlock = `// -- generated from the addon's button rows; edit BUTTON_ROWS_JSX in schema.rs, then \`npm run gen\` --
  extends ${heritage.join(',\n    ')}
  // -- end generated --`;

const path = new URL('./jsx-runtime.d.ts', import.meta.url);
const src = readFileSync(path, 'utf8');
const re = /\/\/ -- generated from the addon's prop schema[\s\S]*?\/\/ -- end generated --/;
if (!re.test(src)) throw new Error('generated-region markers not found in jsx-runtime.d.ts');
const buttonRe = /\/\/ -- generated from the addon's button rows[\s\S]*?\/\/ -- end generated --/;
if (!buttonRe.test(src)) throw new Error('button-row markers not found in jsx-runtime.d.ts');
writeFileSync(path, src.replace(re, block).replace(buttonRe, buttonBlock));
console.log(
  `jsx-runtime.d.ts: ${spec.length} spec + ${style.length} style props generated,` +
    ` ButtonProps over ${buttonRows.length} button rows`,
);

// ----------------------------------------------------------------- docs --
// Every table below comes from the core (`PROPS`, `CUSTOM`, `ELEMENTS`,
// `EVENTS`, `RESOURCES`, `ENV_FIELDS`, `C_FIELDS` in schema.rs; `WARNINGS`
// in diag.rs) through protocol(); this file only lays them out.

const TYPE_DOC = {
  f32: 'number, or a `"$length"` token',
  color: 'color (`#hex` or `0xRRGGBBAA`), or a `"$color"` token',
  flag: 'boolean',
  sizing: 'sizing (`number` \\| `"fit"` \\| `"grow"` \\| `"N%"` \\| `"$length"`)',
  min: 'minimum (`number` \\| `"fit"` \\| `"$length"`)',
  msg: 'message (any plain data)',
  tag: 'tag (a message merged into the event under `tag`, or `null` for none)',
  str: 'string',
  resource: 'resource handle',
  keyframes: 'keyframe list (`[{ at?, width?, height?, bg?, radius?, opacity? }, …]`)',
  enter: 'entrance (`{ dx?, dy?, width?, height?, bg?, radius?, opacity? }`)',
};

const cell = (s) => String(s).replace(/\|/g, '\\|');
// A warning's doc is the const's doc comment, line breaks and all.
const oneLine = (s) => String(s).replace(/\s+/g, ' ').trim();
const typeOf = (def) => (def.kind === 'enum' ? def.values.map((v) => `\`${v}\``).join(' \\| ') : TYPE_DOC[def.kind]);
const tableOf = (header, rows) =>
  [`| ${header.join(' | ')} |`, `|${header.map(() => '---').join('|')}|`, ...rows.map((r) => `| ${r.map(cell).join(' | ')} |`)].join('\n');

// A binding's key path(s) for one env fact; none is a dash, and the row's
// description says where that binding carries it instead.
const keysCell = (keys) => (keys.length ? keys.map((k) => `\`${k}\``).join(' / ') : '—');
const propRow = ([name, def]) => [`\`${name}\``, `\`${def.lua}\``, def.c, typeOf(def), def.doc];

// A metric's column: the number, or for a row that is the platform's the
// pair `windows / elsewhere` — `stock` and `compact` from the addon are the
// running platform's reading, and printing them would make this file say
// which OS generated it (backlog W13). Both columns take the pair, because
// the schema test pins that `compact()` leaves a platform row alone.
const metricCell = (r, value) => (r.platform ? `${r.platform.windows} / ${r.platform.elsewhere}` : `${value}`);

// A verb table cell: the binding's spelling in code font, the same thing
// in another form as prose, or — in italics, so the reasons read as what
// they are — why the binding has none.
const doorCell = (c) => ('is' in c ? `\`${c.is}\`` : 'as' in c ? c.as : `*none: ${c.no}*`);

const md = `# kui props, elements, events and warnings

*Generated by \`npm run gen\` in \`packages/kui\` from the tables in
\`crates/kui-core/src/schema.rs\` and \`crates/kui-core/src/diag.rs\` — edit
those, not this file.*

One prop schema serves every binding: JSX props are camelCase, Lua keys are
their snake_case, and C uses the \`KuiSpec\` / \`KuiTextStyle\` field named
below. Container props apply to \`<box>\` (and to \`<edit>\` / \`<image>\`
where they make sense); text props apply to \`<text>\` and \`<edit>\`.

## Container props

${tableOf(['JSX', 'Lua', 'C', 'type', 'description'], spec.map(propRow))}

## Text props

${tableOf(['JSX', 'Lua', 'C', 'type', 'description'], style.map(propRow))}

## Composite props (hand-written per binding)

${tableOf(['JSX', 'Lua', 'C', 'description'], custom.map(([, d]) => [d.jsx, d.lua, d.c, d.doc]))}

## Elements

${tableOf(['JSX', 'Lua', 'C', 'notes'], elements.map((e) => [e.jsx, e.lua, e.c, e.doc]))}

## Events

Events are plain data: \`{ origin, window, key, payload }\` from
\`pollEvents()\` in Node, \`on_event(ev)\` in Lua (payload fields plus
\`node_key\`), and \`kui_poll_event\` in C. \`origin\` is which frontend drew
the node (0 = the app, 1+ = an extension) and \`window\` is which OS window
it happened in — an extension draws into all of them, so the two are not
one field. The payload shapes:

${tableOf(['kind', 'payload', 'when'], events.map((e) => [e.kind, e.payload, e.doc]))}

## Resources

${tableOf(['what', 'Node', 'Lua', 'C'], resources.map((r) => [r.what, r.node, r.lua, r.c]))}

Handles are slotmap keys with a generation: a removed resource's handle is
rejected (an image draws nothing, a font shapes as sans) rather than
aliasing whatever took its slot.

## Warnings

A misconfiguration that fails silently — a grow weight with nothing to
split against, two nodes sharing a key, a button with no name — comes back
as data instead: \`{ code, key, message }\`, each distinct (code, node) pair
once. Node collects them on \`app.warnings\` (\`ctx.warnings()\` headless),
Rust drains \`Core::take_warnings\`, C drains \`kui_take_warnings\`, and the
Lua runner prints them. \`code\` is stable — match on it; the message is for
people. In Node the codes are the \`WarningCode\` union.

${tableOf(['code', 'meaning'], warnings.map((w) => [`\`${w.code}\``, oneLine(w.doc)]))}
## Env

The host facts a view reads: \`ui.env()\` in Rust, \`view(env)\` in Lua,
\`ctx.env()\` / \`win.env()\` in Node. C is the host, so it *writes* them
(\`kui_env_set\`, \`kui_env_set_system\`, \`kui_env_set_window\`,
\`kui_env_set_audio\`, \`kui_env_set_assistive\`) and has no
reading; its column names the argument. A real window's runner refreshes every fact each frame;
headless, \`ctx.setEnv\` in Node and the C setters are the writers, and
the conformance corpus drives its two chrome scenes through them. The
\`from\` column says which Rust struct holds the fact, or that it is derived
or the frame's rather than \`Env\`'s. Two divergences are deliberate:
\`native_controls\` is the \`Rect\` the core holds in Node and a width and
height at the window origin in Lua and C (the shape C's two numbers can
express, and where the macOS traffic lights sit), and \`frame_budget_ms\` is
derived from \`refresh_hz\` but part of the reading everywhere, so no view
restates the 120 Hz fallback.

The \`system.*\` rows are what the *user* set, and every one of them can
also be unknown — which is the default, and what a driver reports for a
fact its platform gives it no way to ask. The Rust runner asks the OS for
all four on macOS and Windows (the appearance through winit, the accent,
motion and locale through AppKit / Win32) and re-asks when the app takes
focus back or the theme changes; on X11 and Wayland it answers the locale
from \`LANG\` and leaves the rest unknown. Every other driver owns its own
window, so it pushes what it knows through its env setter.
Unknown is a reading, not a missing value: the enums
spell it \`"unknown"\` and always have a key, the two values are \`null\` in
Node and an absent key in Lua, and C reads zero as it does everywhere
else. Nothing in the core acts on any of it — a dark appearance repaints
nothing and a reduced motion shortens nothing, because only the view knows
which of its colours is the background and which of its animations carries
meaning. The fifth row, \`system.assistive\`, is not a setting but a fact
of the same shape: whether an accessibility client has asked for the tree,
written by the runner's bridge rather than by a settings query, and the
one reading that changes what a view *says* rather than what it draws.

${tableOf(
  ['field', 'from', 'Node', 'Lua', 'C', 'description'],
  env.map((f) => [`\`${f.name}\``, f.from, keysCell(f.node), keysCell(f.lua), f.c, f.doc]),
)}

## Theme

The colours a view paints with, as roles rather than values, derived from
the two \`system\` facts above: the appearance picks the base and the accent
recolours it ([ADR 0019](adr/0019-a-theme-derived-from-appearance-and-accent.md)).
This is the half \`system.appearance\` was missing — the core still acts on
nothing, but a view that wants to act now has something to act *with*, and
the stock widgets do, so a button, a context menu, a tooltip, a field, a
scrollbar, a focus ring and a \`<text>\` with no \`color\` all follow the OS
without an app writing a line.

Read it as \`ui.theme()\` in Rust, \`env.theme\` in Lua, \`ctx.theme()\` /
\`win.theme()\` in Node and \`kui_theme\` in C — one \`0xRRGGBBAA\` number per
role under the name below, so a role goes straight into a \`bg\` or \`color\`
prop. Three sources, and the default follows the OS for both facts: pin an
accent of the app's own and keep the OS's light/dark
(\`Core::set_accent\`, \`ctx.setAccent\`, \`kui_theme_set_accent\`), or pin a
whole palette that follows nothing (\`Core::set_theme\`, \`ctx.setTheme\`,
\`kui_theme_set\`). A Lua script reads but does not set: it is a guest in
someone else's frame, and the palette is the host's.

An unknown appearance takes the **dark** base — not a guess about the
user, but exactly what kui painted before this table existed, which is
what makes following the OS safe as the default rather than an opt-in.
Beside the roles ride \`appearance\` (which base this came from) and
\`disabled_opacity\` / \`disabledOpacity\` (a multiplier, not a colour).

${tableOf(
  ['role', 'Node', 'dark', 'light', 'description'],
  theme.map((r) => [`\`${r.name}\``, `\`${r.node}\``, `\`${r.dark}\``, `\`${r.light}\``, r.doc]),
)}

## Metrics

The sizes the stock widgets are built from, as roles beside the palette's
(backlog T2): the argument for a theme, one axis over — a button of the
app's own and the stock one should agree on a radius and a padding without
either copying a number out of the other. Logical px, applied *before* the
scale factor, which is the renderer's; a metric never scales by itself.
Density is the app's to choose, the way the palette is: the **stock** set
is what the widgets always drew, **compact** is a dense tool's (smaller
text, shallower padding, sharper corners, the titlebar at the platform's
height either way), and every length can be multiplied for a density
slider (\`Metrics::scaled\`, \`scale\` in \`setMetrics\`).

Read it as \`ui.metrics()\` in Rust, \`env.metrics\` in Lua, \`ctx.metrics()\` /
\`win.metrics()\` in Node and \`kui_metrics\` in C; set it with
\`Core::set_metrics\`, \`ctx.setMetrics\` and \`kui_metrics_set\` (a Lua
script reads but does not set, as with the theme). The conformance corpus
runs with the stock set, which is what keeps "the stock button is 15-px
text in 14×8 padding" a sentence about kui rather than about an app.

A value written \`32 / 34\` is the platform's own — Windows first, then
everywhere else — and \`compact\` leaves it alone.

${tableOf(
  ['metric', 'Node', 'stock', 'compact', 'description'],
  metrics.map((r) => [`\`${r.name}\``, `\`${r.node}\``, metricCell(r, r.stock), metricCell(r, r.compact), r.doc]),
)}

## Doors

The verbs — what an app or a host *calls* on its context, as against what
it declares in the tree above — one row per verb across the four bindings
(\`schema::DOORS\`, backlog B1). A cell is the binding's spelling (a
\`kui_*\` function; a method on both of Node's classes, or on the one it
is prefixed with; a function on Lua's \`env\`), the same thing in another
form (a prop, a reading, a callback, a constructor option), or — in
italics — the reason the binding has none. The reasons are the point: the
bindings are not one surface. A Lua script is a guest in the host's frame
([ADR 0014](adr/0014-slots-an-extension-fills-in-place.md)) whose env is
a reading, so registering, driving, pacing and reading back are the
host's; Node's \`Ctx\` drives a headless core and its \`KuiWindow\` is
driven by the runner, so the driver's half is on \`Ctx\` alone; and a
Node host never paints, so the renderer's rows are C's.

Each binding's own test pins its column both ways: every spelling here is
a door there, and every door there is a row here — so a verb added to one
binding is a row with its three other cells, or a red test.

${tableOf(
  ['verb', 'C', 'Node', 'Lua', 'description'],
  doors.map((d) => [`\`${d.rust}\``, doorCell(d.c), doorCell(d.node), doorCell(d.lua), d.doc]),
)}
`;

writeFileSync(new URL('../../docs/props.md', import.meta.url), md);
console.log(
  `docs/props.md: ${spec.length + style.length} schema rows, ${custom.length} composites, ${elements.length} elements, ${events.length} events, ${warnings.length} warnings, ${env.length} env fields, ${theme.length} theme roles, ${metrics.length} metrics, ${doors.length} doors`,
);

// ------------------------------------------------------ the warning codes --
// The `code` a Warning carries, as a union the app can `switch` over. The
// doc on each arm is the Rust const's own doc comment, re-wrapped.

const wrapDoc = (doc, width = 74) => {
  const lines = [];
  let line = '';
  for (const word of oneLine(doc).split(' ')) {
    if (line && line.length + 1 + word.length > width) {
      lines.push(line);
      line = word;
    } else {
      line = line ? `${line} ${word}` : word;
    }
  }
  if (line) lines.push(line);
  return `  /** ${lines.join('\n   *  ')} */`;
};

const warningBlock = [
  "// -- generated from the core's warning codes; edit crates/kui-core/src/diag.rs, then `npm run gen` --",
  'export type WarningCode =',
  ...warnings.map((w) => `${wrapDoc(w.doc)}\n  | '${w.code}'`),
].join('\n') + ';\n// -- end generated --';

const indexPath = new URL('./index.d.ts', import.meta.url);
let indexSrc = readFileSync(indexPath, 'utf8');
const warningRe = /\/\/ -- generated from the core's warning codes[\s\S]*?\/\/ -- end generated --/;
if (!warningRe.test(indexSrc)) throw new Error('warning-code markers not found in index.d.ts');
indexSrc = indexSrc.replace(warningRe, warningBlock);
console.log(`index.d.ts: ${warnings.length} warning codes generated`);

// The name lists a reader or a request is spelled from. Hand-written, the
// role union missed `terminal` for a release: `Role::ALL` grew and nothing
// said the union had to. Each is one line per name, wrapped like a
// hand-written union would be.
const union = (names) => {
  const lines = [];
  let line = ' ';
  for (const n of names) {
    const piece = ` | '${n}'`;
    if (line.length + piece.length > 78) {
      lines.push(line);
      line = ' ';
    }
    line += piece;
  }
  lines.push(line);
  return lines.join('\n');
};
const accessBlock = [
  "// -- generated from the core's access lists; edit Role::ALL / AccessAction::ALL in crates/kui-core/src/access.rs, then `npm run gen` --",
  '/** Every role a node of the tree can report: the ones a view declares',
  ' *  (`role`), the ones the core derives (a `cells` grid is a `terminal`,',
  ' *  an editor a text input, a scrolling box a scroll view, the root the',
  ' *  window). */',
  `export type AccessRole =\n${union(accessRoles)};`,
  '',
  '/** What assistive technology can ask of a node (`access(key, action)`). */',
  `export type AccessAction =\n${union(accessActions)};`,
  '// -- end generated --',
].join('\n');
const accessRe = /\/\/ -- generated from the core's access lists[\s\S]*?\/\/ -- end generated --/;
if (!accessRe.test(indexSrc)) throw new Error('access-list markers not found in index.d.ts');
indexSrc = indexSrc.replace(accessRe, accessBlock);
console.log(`index.d.ts: ${accessRoles.length} access roles, ${accessActions.length} access actions generated`);

const inputBlock = [
  "// -- generated from the core's input lists; edit EditKey::ALL / MouseButton::NAMED in crates/kui-core/src/input.rs, then `npm run gen` --",
  '/** The buttons `ctx.mouse` takes by name; anything else is a code. */',
  `export type MouseButtonName =
${union(mouseButtons)};`,
  '',
  '/** The editing keys `ctx.key` takes, the spelling the corpus steps use. */',
  `export type EditKeyName =
${union(editKeys)};`,
  '// -- end generated --',
].join('\n');
const inputRe = /\/\/ -- generated from the core's input lists[\s\S]*?\/\/ -- end generated --/;
if (!inputRe.test(indexSrc)) throw new Error('input-list markers not found in index.d.ts');
indexSrc = indexSrc.replace(inputRe, inputBlock);
console.log(`index.d.ts: ${editKeys.length} edit keys, ${mouseButtons.length} mouse buttons generated`);

const menuBlock = [
  "// -- generated from the core's menu roles; edit MenuRole::ALL in crates/kui-core/src/menu.rs, then `npm run gen` --",
  '/** What a menu row is: the app\'s own (`custom`), a divider, or one of',
  ' *  the standard rows the core performs itself. The same spelling a',
  ' *  `menu` message reports back. */',
  `export type MenuItemRole =\n${union(menuRoles)};`,
  '// -- end generated --',
].join('\n');
{
  const jsxPath = new URL('./jsx-runtime.d.ts', import.meta.url);
  const jsxSrc = readFileSync(jsxPath, 'utf8');
  const menuRe = /\/\/ -- generated from the core's menu roles[\s\S]*?\/\/ -- end generated --/;
  if (!menuRe.test(jsxSrc)) throw new Error('menu-role markers not found in jsx-runtime.d.ts');
  writeFileSync(jsxPath, jsxSrc.replace(menuRe, menuBlock));
  console.log(`jsx-runtime.d.ts: ${menuRoles.length} menu roles generated`);
}

// ------------------------------------------------------- the addon surface --
// napi-rs derives a TypeScript signature for every `#[napi]` item from the
// Rust one and writes them out; crates/kui-node's build.rs points it at
// target/napi-type-defs, so any build of the addon leaves them there. That
// file is the authority for the generated region of index.d.ts: `Ctx` and
// `KuiWindow` are 39 shared methods each, generated in Rust by one
// `core_methods!` list, and this keeps the copy a JS user actually reads from
// being a third hand-written one. Where the derived type is too loose — every
// `Json` parameter and return — the Rust side names the real one with
// `#[napi(ts_args_type = ...)]` / `ts_return_type`, so the rich types below
// (`AccessTree`, `UiEvent<A>[]`, `PlayOptions`, ...) survive.
//
// This file only lays the defs out. It refuses a kind it has never seen
// rather than dropping it silently, which is the whole point of the exercise.

// Method lines arrive with one leading space and JSDoc lines with none, so
// that `/**`, ` * …` and ` */` land aligned under a two-space indent. A blank
// JSDoc line comes through as ` * `, which would check in trailing space.
const indent = (def) =>
  def
    .split('\n')
    .map((line) => ('  ' + (line.startsWith(' *') ? line : line.replace(/^ /, ''))).replace(/\s+$/, ''))
    .join('\n');

const classes = new Map();
const parts = [];
let members = 0;
for (const line of defsSrc.trim().split('\n')) {
  const item = JSON.parse(line);
  if (item.kind === 'fn') {
    parts.push({ text: `${item.js_doc}export declare ${item.def}` });
    members += 1;
  } else if (item.kind === 'struct') {
    const cls = { name: item.name, doc: item.js_doc, body: item.def ? [item.def] : [] };
    classes.set(item.name, cls);
    parts.push({ cls });
  } else if (item.kind === 'impl') {
    const cls = classes.get(item.name);
    if (!cls) throw new Error(`impl for an unknown class: ${item.name}`);
    cls.body.push(item.def);
  } else {
    throw new Error(`unhandled napi type-def kind ${item.kind} (${item.name}) - teach gen-types.mjs about it`);
  }
}

const nativeBlock = [
  "// -- generated from the addon's `#[napi]` surface; edit crates/kui-node/src/lib.rs, then `npm run gen` --",
  ...parts.map((p) => {
    if (!p.cls) return p.text;
    const body = p.cls.body.join('\n');
    members += (body.match(/^ [A-Za-z]/gm) ?? []).length;
    return `${p.cls.doc}export declare class ${p.cls.name} {\n${indent(body)}\n}`;
  }),
  '// -- end generated --',
].join('\n\n');

const nativeRe = /\/\/ -- generated from the addon's `#\[napi\]` surface[\s\S]*?\/\/ -- end generated --/;
if (!nativeRe.test(indexSrc)) throw new Error('generated-region markers not found in index.d.ts');
writeFileSync(indexPath, indexSrc.replace(nativeRe, nativeBlock));
console.log(`index.d.ts: ${classes.size} classes, ${members} members generated from the #[napi] surface`);
