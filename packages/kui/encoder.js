// Flat binary IR encoder: turns a jsx-runtime tree into one Float64Array
// instruction stream + one UTF-8 string table, matching the decoder in
// crates/kui-node/src/binary.rs. All opcode/prop ids come from the addon's
// protocol() at construction, so the two sides cannot drift. Buffers are
// reused across frames (no per-frame GC churn); encode() returns subarray
// views that are valid until the next encode() on the same encoder.
//
// This is also the only place a view is validated: a malformed tree is
// rejected here, in JS, before anything crosses the boundary — so the
// messages name the element or the prop and what it would have accepted.

// Turns a protocol name list into its name -> wire index map.
const indexOf = (names) => Object.fromEntries(names.map((n, i) => [n, i]));

export function createEncoder(P) {
  const OP = P.op;
  const PR = P.prop;
  const VERSION = P.version;
  // `kind` on a `windows` entry, as the index the addon decodes: the
  // core's list (`WindowKind::ALL`), not a copy — a window is not a node,
  // but its kinds are still one list (see `docs/adr/0004-multi-window.md`).
  const WINDOW_KINDS = P.windowKinds;
  // Value tables come from the addon too, so "below" cannot mean one thing
  // here and another in kui-core.
  const ALIGN = indexOf(P.align);
  const FLOAT_PRESET = indexOf(P.floatPreset);
  const IMAGE_SAMPLING = indexOf(P.imageSampling);
  const IMAGE_FIT = indexOf(P.imageFit);
  const PRESET_NAMES = P.floatPreset.join(' | ');

  // The allow-list a view is checked against, straight off the protocol: the
  // schema rows, every JSX spelling of every composite, and — per element —
  // the props the element lowers itself (`<edit initial>`, `<image src>`).
  // A name outside it has no id, so the encoder would silently drop it; it
  // collects those instead, and encode() hands them back for the addon to
  // warn about.
  const KNOWN = new Set();
  for (const [name, def] of Object.entries(PR)) {
    // A composite's row name is its identity on the wire, not a prop a view
    // writes (`overflow` is `clip` / `scrollX` / `scrollY`), so those rows
    // contribute their spellings and nothing else.
    if (def.kind === 'custom') for (const n of def.names) KNOWN.add(n);
    else KNOWN.add(name);
  }
  const OWN = new Map((P.elements ?? []).map((e) => [e.name, new Set(e.own ?? [])]));
  // An element that reads only some rows names them (`ElementDef::jsx_rows`):
  // the check admits those and its own, and the lowering writes only those,
  // so a row it would drop — `radius` on the stock button — is warned about
  // rather than silently lost, and never crosses the wire either. `null` is
  // every row.
  const ROWS = new Map((P.elements ?? []).map((e) => [e.name, e.rows ? new Set(e.rows) : null]));
  // The stock button's prop list: its admitted rows less the click, which
  // rides beside the list as its own field.
  const BUTTON_ROWS = new Set([...(ROWS.get('button') ?? [])].filter((k) => k !== 'onClick'));
  // `<span>` is part of the text element (its props are read by collectSpans),
  // and the hud is the graph's other spelling.
  const ELEMENT_OF = { span: 'text', latencyHud: 'latencyGraph' };
  let unknown = [];
  // A `$name` in a colour or length slot rides as the prop's id with this
  // bit set and the token's index in the value slot (ADR 0027, decision
  // 4). The roles come first in each index space — `$surface` is index 1
  // of the colours whatever the app declared — then the app's own, in the
  // order `setTokens` declared them; `index.js` builds that map and hands
  // it to every encode, since one encoder serves every surface.
  const TOKEN_TAG = P.tokenTag;
  const ROLE_TOKENS = new Map();
  (P.tokenRoles?.colors ?? []).forEach((name, i) => ROLE_TOKENS.set(name, { kind: 'color', index: i }));
  (P.tokenRoles?.lengths ?? []).forEach((name, i) => ROLE_TOKENS.set(name, { kind: 'length', index: i }));
  const COLOR_ROLES = (P.tokenRoles?.colors ?? []).length;
  const LENGTH_ROLES = (P.tokenRoles?.lengths ?? []).length;
  let tokens = null;
  let unknownTokens = [];

  // Whether a prop value is a reference: a string starting with `$`.
  const isRef = (v) => typeof v === 'string' && v.length > 1 && v.charCodeAt(0) === 36;

  // The wire index of `$name` in the `kind` space, or `undefined` for a
  // name that resolved to nothing or to the other kind — reported like an
  // unknown prop, and the slot is left out so the core keeps its default.
  function tokenRef(v, kind) {
    const name = v.slice(1);
    const hit = ROLE_TOKENS.get(name) ?? tokens?.get(name);
    if (hit === undefined || hit.kind !== kind) {
      unknownTokens.push([name, kind]);
      return undefined;
    }
    return hit.index;
  }

  function checkProps(type, p) {
    const element = ELEMENT_OF[type] ?? type;
    const own = OWN.get(element);
    if (own === undefined) return; // a JSX <>…</>, or an element that already threw
    const rows = ROWS.get(element) ?? KNOWN;
    for (const k in p) {
      if (rows.has(k) || own.has(k)) continue;
      unknown.push([element, k]);
    }
  }

  let f = new Float64Array(1 << 14);
  let u = new Uint8Array(1 << 16);
  let fi = 0;
  let ui = 0;
  const textEncoder = new TextEncoder();
  const colorCache = new Map();

  function reserve(n) {
    if (fi + n <= f.length) return;
    let len = f.length;
    while (fi + n > len) len *= 2;
    const next = new Float64Array(len);
    next.set(f);
    f = next;
  }

  // Writes a (offset, len) string ref; null/undefined = absent.
  function strRef(s) {
    if (s == null) {
      f[fi++] = -1;
      f[fi++] = 0;
      return;
    }
    for (;;) {
      const r = textEncoder.encodeInto(s, u.subarray(ui));
      if (r.read === s.length) {
        f[fi++] = ui;
        f[fi++] = r.written;
        ui += r.written;
        return;
      }
      const next = new Uint8Array(u.length * 2);
      next.set(u);
      u = next;
    }
  }

  function color(v) {
    if (typeof v === 'number') return v >>> 0;
    let c = colorCache.get(v);
    if (c !== undefined) return c;
    const hex = typeof v === 'string' && v.startsWith('#') ? v.slice(1) : null;
    let expanded;
    if (hex && hex.length === 3) expanded = hex[0] + hex[0] + hex[1] + hex[1] + hex[2] + hex[2] + 'ff';
    else if (hex && hex.length === 6) expanded = hex + 'ff';
    else if (hex && hex.length === 8) expanded = hex;
    else throw new Error(`bad color ${JSON.stringify(v)}`);
    c = parseInt(expanded, 16) >>> 0;
    if (Number.isNaN(c)) throw new Error(`bad color ${JSON.stringify(v)}`);
    colorCache.set(v, c);
    return c;
  }

  // Writes (mode, value) for a sizing prop.
  function sizing(v) {
    if (typeof v === 'number') {
      f[fi++] = 0;
      f[fi++] = v;
    } else if (v === 'fit') {
      f[fi++] = 1;
      f[fi++] = 0;
    } else if (v === 'grow') {
      f[fi++] = 2;
      f[fi++] = 1;
    } else if (typeof v === 'string' && v.endsWith('%')) {
      f[fi++] = 3;
      f[fi++] = parseFloat(v) / 100;
    } else if (v && typeof v === 'object' && typeof v.grow === 'number') {
      f[fi++] = 2;
      f[fi++] = v.grow;
    } else if (v && typeof v === 'object' && typeof v.percent === 'number') {
      f[fi++] = 3;
      f[fi++] = v.percent;
    } else {
      throw new Error(`bad sizing ${JSON.stringify(v)} (fit | grow | number | "N%")`);
    }
  }

  // Writes (mode, value) for a min prop: a number, or "fit" for the node's
  // own fit size on that axis.
  function min(v) {
    if (typeof v === 'number') {
      f[fi++] = 0;
      f[fi++] = v;
    } else if (v === 'fit') {
      f[fi++] = 1;
      f[fi++] = 0;
    } else {
      throw new Error(`bad min ${JSON.stringify(v)} (number | "fit")`);
    }
  }

  function alignOf(v) {
    const a = ALIGN[v];
    if (a === undefined) throw new Error(`align must be ${P.align.join(' | ')}`);
    return a;
  }

  function presetOf(v) {
    const p = FLOAT_PRESET[v];
    if (p === undefined) throw new Error(`bad float preset ${JSON.stringify(v)} (${PRESET_NAMES})`);
    return p;
  }

  // Writes the "declared" flag and the two align indices of a float attach
  // point; a missing one still writes three slots so the stream is fixed-width.
  function attach(v) {
    const ok = Array.isArray(v) && v.length === 2;
    f[fi++] = ok ? 1 : 0;
    f[fi++] = ok ? alignOf(v[0]) : 0;
    f[fi++] = ok ? alignOf(v[1]) : 0;
  }

  // One float offset component, with its own flag: dx and dy are declared
  // separately, so `{ anchor: 'below', dx }` keeps the preset's dy.
  function offset(v) {
    f[fi++] = v === undefined ? 0 : 1;
    f[fi++] = v ?? 0;
  }

  // Encodes a prop list, schema-driven: generic props are written by their
  // protocol kind (no names or shapes hardcoded here); only composites (the
  // pad family, border, overflow bits, float) and the constructor-ordering
  // specials (dir, size) have hand-written stanzas, mirroring binary.rs.
  // `key` rides along as P_KEY; `isRoot` admits `title`, `alwaysOnTop` and `windows` (and drops `key`);
  // `admit`, when given, is the only names written (a closed composite's
  // rows — the rest were already reported by checkProps).
  function props(p, key, isRoot, admit) {
    const np = fi++;
    let n = 0;
    // dir and size first: the decoder constructs spec/style from them.
    if (p.dir !== undefined && p.dir !== 'column') {
      if (p.dir !== 'row') throw new Error(`bad dir ${JSON.stringify(p.dir)} (row | column)`);
      f[fi++] = PR.dir.id;
      f[fi++] = 1;
      n++;
    }
    if (p.size != null) {
      if (isRef(p.size)) {
        const i = tokenRef(p.size, 'length');
        if (i !== undefined) {
          f[fi++] = PR.size.id | TOKEN_TAG;
          f[fi++] = i;
          n++;
        }
      } else {
        f[fi++] = PR.size.id;
        f[fi++] = p.size;
        n++;
      }
    }
    if (key != null && !isRoot) {
      f[fi++] = PR.key.id;
      strRef(key);
      n++;
    }
    let pad, padX, padY, padL, padR, padT, padB;
    let borderW, borderColor;
    let overflow = 0;
    for (const k in p) {
      // Enough for the widest single stanza (float writes 12 slots).
      reserve(16);
      const v = p[k];
      if (v === undefined) continue;
      if (admit !== undefined && !admit.has(k)) continue;
      if (v === null) {
        // A null tag (onKey / onDrag / onHover / onLayout) still declares
        // the behaviour, just with no `tag` on its events; every other
        // null means absent.
        const def = PR[k];
        if (def === undefined || def.kind !== 'tag') continue;
      }
      switch (k) {
        case 'dir':
        case 'size':
          break; // handled above
        // A data index rather than a name: `open_indexed` on the other
        // side, and the row above's `key` loses to it there.
        case 'index':
          if (isRoot) break;
          f[fi++] = PR.index.id;
          f[fi++] = v;
          n++;
          break;
        case 'pad': pad = v; break;
        case 'padX': padX = v; break;
        case 'padY': padY = v; break;
        case 'padL': padL = v; break;
        case 'padR': padR = v; break;
        case 'padT': padT = v; break;
        case 'padB': padB = v; break;
        case 'borderW': borderW = v; break;
        case 'borderColor': borderColor = v; break;
        case 'clip': if (v) overflow |= 1; break;
        case 'scrollX': if (v) overflow |= 2; break;
        case 'scrollY': if (v) overflow |= 4; break;
        case 'float': {
          // The preset and each override as declared; what a preset attaches
          // to, and what an absent override falls back to, is the decoder's
          // call (FloatConfig::build).
          f[fi++] = PR.float.id;
          if (typeof v === 'string') {
            f[fi++] = presetOf(v);
            attach(null);
            attach(null);
            offset(undefined);
            offset(undefined);
            f[fi++] = 0; // fit
          } else {
            f[fi++] = v.anchor === undefined ? 0 : presetOf(v.anchor);
            attach(v.at);
            attach(v.self);
            offset(v.dx);
            offset(v.dy);
            f[fi++] = v.fit ? 1 : 0;
          }
          n++;
          break;
        }
        case 'keyFocus':
          if (v) {
            f[fi++] = PR.keyFocus.id;
            n++;
          }
          break;
        case 'title':
          if (isRoot) {
            f[fi++] = PR.title.id;
            strRef(v);
            n++;
          }
          break;
        case 'alwaysOnTop':
          // Root only, like `title`; a flag, like `keyFocus`.
          if (isRoot && v) {
            f[fi++] = PR.alwaysOnTop.id;
            n++;
          }
          break;
        case 'windows':
          // Root only, like `title`: a count, then per window its name,
          // kind (0 = normal, 1 = popup), width, height (0 = the default
          // size), whether it activates, and the anchor rect a popup is
          // placed against. An entry may be just a name.
          if (isRoot && Array.isArray(v)) {
            f[fi++] = PR.windows.id;
            f[fi++] = v.length;
            for (const w of v) {
              reserve(12);
              const d = typeof w === 'string' ? { name: w } : w;
              if (d == null || typeof d.name !== 'string') {
                throw new Error(`bad windows entry ${JSON.stringify(w)} (a name, or { name, kind?, anchor?, width?, height?, activates? })`);
              }
              // An entry is plain data with a fixed shape, not a node's loose
              // prop bag, so a value that does nothing is refused rather than
              // dropped: a kind kui does not have would otherwise open a
              // normal window and read as the popup having worked.
              const kind = WINDOW_KINDS.indexOf(d.kind ?? 'normal');
              if (kind < 0) {
                throw new Error(`windows entry ${JSON.stringify(d.name)} has kind ${JSON.stringify(d.kind)}; the kinds are ${WINDOW_KINDS.map((k) => JSON.stringify(k)).join(' and ')}`);
              }
              const a = d.anchor ?? {};
              strRef(d.name);
              f[fi++] = kind;
              f[fi++] = d.width ?? 0;
              f[fi++] = d.height ?? 0;
              // Unsaid is 2: the kind's own default, decided by the core
              // (a popup does not take OS focus unless asked).
              f[fi++] = d.activates == null ? 2 : d.activates ? 1 : 0;
              f[fi++] = a.x ?? 0;
              f[fi++] = a.y ?? 0;
              f[fi++] = a.w ?? 0;
              f[fi++] = a.h ?? 0;
            }
            n++;
          }
          break;
        case 'tooltip':
          f[fi++] = PR.tooltip.id;
          strRef(String(v));
          n++;
          break;
        default: {
          // Schema-driven: unknown names (element-level props included) are
          // ignored — an element's own props live beside the node's.
          const def = PR[k];
          if (def === undefined || def.kind === 'custom') break;
          if (def.kind === 'flag') {
            if (v) {
              f[fi++] = def.id;
              n++;
            }
            break;
          }
          // A reference on a colour, length or sizing row: the tagged id
          // and the index, or nothing at all for a name that did not
          // resolve (reported through `unknownTokens`).
          if (isRef(v) && (def.kind === 'f32' || def.kind === 'color' || def.kind === 'sizing')) {
            const i = tokenRef(v, def.kind === 'color' ? 'color' : 'length');
            if (i !== undefined) {
              f[fi++] = def.id | TOKEN_TAG;
              f[fi++] = i;
              n++;
            }
            break;
          }
          f[fi++] = def.id;
          switch (def.kind) {
            case 'f32':
              f[fi++] = v;
              break;
            case 'color':
              f[fi++] = color(v);
              break;
            case 'enum': {
              const i = def.values.indexOf(v);
              if (i < 0)
                throw new Error(`bad value ${JSON.stringify(v)} for ${k} (one of ${def.values.join(' | ')})`);
              f[fi++] = i;
              break;
            }
            case 'sizing':
              sizing(v);
              break;
            case 'min':
              min(v);
              break;
            case 'msg':
            case 'tag':
            case 'keyframes':
            case 'enter':
              strRef(JSON.stringify(v));
              break;
            case 'str':
            case 'resource':
              strRef(String(v));
              break;
            default:
              throw new Error(`unhandled schema kind ${def.kind} for ${k}`);
          }
          n++;
          break;
        }
      }
    }
    // The shorthand family as declared: a set mask then the seven values in
    // the same order. What an absent edge falls back to is PadShorthand's
    // call in kui-core, not this encoder's.
    reserve(16);
    const padded = [pad, padX, padY, padL, padR, padT, padB];
    let padSet = 0;
    let padRefs = 0;
    for (let i = 0; i < padded.length; i++) {
      if (padded[i] === undefined) continue;
      if (isRef(padded[i])) {
        // An edge naming a token: its index rides in the value slot and a
        // second mask says so; one that did not resolve is left unset.
        const t = tokenRef(padded[i], 'length');
        if (t === undefined) {
          padded[i] = undefined;
          continue;
        }
        padded[i] = t;
        padRefs |= 1 << i;
      }
      padSet |= 1 << i;
    }
    if (padSet) {
      f[fi++] = padRefs ? PR.pad.id | TOKEN_TAG : PR.pad.id;
      f[fi++] = padSet;
      if (padRefs) f[fi++] = padRefs;
      for (const v of padded) f[fi++] = v ?? 0;
      n++;
    }
    if (borderW !== undefined) {
      // Tagged when either half is a reference: a flags word (1 the
      // width, 2 the colour) then the two slots.
      let flags = 0;
      let w = borderW;
      let c = borderColor != null ? borderColor : 0;
      if (isRef(w)) {
        const t = tokenRef(w, 'length');
        w = t ?? 0;
        if (t !== undefined) flags |= 1;
      }
      if (isRef(c)) {
        const t = tokenRef(c, 'color');
        c = t ?? 0;
        if (t !== undefined) flags |= 2;
      } else if (c !== 0) {
        c = color(c);
      }
      f[fi++] = flags ? PR.border.id | TOKEN_TAG : PR.border.id;
      if (flags) f[fi++] = flags;
      f[fi++] = w;
      f[fi++] = c;
      n++;
    }
    if (overflow) {
      f[fi++] = PR.overflow.id;
      f[fi++] = overflow;
      n++;
    }
    f[np] = n;
  }

  function collectText(node, out) {
    if (node == null || typeof node === 'boolean') return out;
    if (typeof node === 'string') return out + node;
    if (typeof node === 'number') return out + String(node);
    if (Array.isArray(node)) {
      for (const c of node) out = collectText(c, out);
      return out;
    }
    return collectText(node.children, out);
  }

  function hasSpan(node) {
    if (Array.isArray(node)) return node.some(hasSpan);
    return node != null && typeof node === 'object' && node.type === 'span';
  }

  // A span colour as the wire carries it: `{ v, ref }` — the `u32`, or a
  // token index with `ref` set; `undefined` for none (a reference that
  // did not resolve inherits, like no colour at all).
  function spanColor(v, inherited) {
    if (v == null) return inherited;
    if (isRef(v)) {
      const i = tokenRef(v, 'color');
      return i === undefined ? inherited : { v: i, ref: true };
    }
    return { v: color(v), ref: false };
  }

  // Flattens <span> nesting into (text, flags, color, bg) quads, inheritance
  // matching the Rust collect_spans. Flags: 1 bold, 2 italic, 4 has color,
  // 8 underline, 16 strikethrough, 32 has bg, 64 the colour is a token
  // index, 128 the bg is.
  function collectSpans(node, st, out) {
    if (node == null || typeof node === 'boolean') return;
    if (typeof node === 'string' || typeof node === 'number') {
      const flags =
        (st.bold ? 1 : 0) |
        (st.italic ? 2 : 0) |
        (st.color !== undefined ? 4 : 0) |
        (st.underline ? 8 : 0) |
        (st.strikethrough ? 16 : 0) |
        (st.bg !== undefined ? 32 : 0) |
        (st.color?.ref ? 64 : 0) |
        (st.bg?.ref ? 128 : 0);
      out.push([String(node), flags, st.color?.v ?? 0, st.bg?.v ?? 0]);
      return;
    }
    if (Array.isArray(node)) {
      for (const c of node) collectSpans(c, st, out);
      return;
    }
    if (node.type !== 'span') throw new Error('only strings and <span> may nest inside rich <text>');
    const p = node.props ?? {};
    collectSpans(
      node.children,
      {
        bold: st.bold || !!p.bold,
        italic: st.italic || !!p.italic,
        color: spanColor(p.color, st.color),
        underline: st.underline || !!p.underline,
        strikethrough: st.strikethrough || !!p.strikethrough,
        bg: spanColor(p.bg, st.bg),
      },
      out,
    );
  }

  function children(node) {
    if (node == null || typeof node === 'boolean') return;
    if (typeof node === 'string' || typeof node === 'number') {
      // Bare text child: a default-styled text node.
      reserve(8);
      f[fi++] = OP.text;
      strRef(String(node));
      f[fi++] = 0;
      return;
    }
    if (Array.isArray(node)) {
      for (const c of node) children(c);
      return;
    }
    element(node);
  }

  // A JSX <>…</> that reached here as an object rather than being spliced
  // by jsx(): splice it now. Keyed on the runtime's own sentinel, which is
  // a symbol — the kui element named `fragment` is a different thing and is
  // handled in the switch below.
  const JSX_FRAGMENT = Symbol.for('kui.jsx.fragment');

  function element(el) {
    if (el.type === JSX_FRAGMENT) {
      children(el.children);
      return;
    }
    const p = el.props ?? {};
    checkProps(el.type, p);
    reserve(96);
    switch (el.type) {
      case 'box':
        f[fi++] = OP.open;
        props(p, el.key, false);
        children(el.children);
        reserve(4);
        f[fi++] = OP.close;
        return;
      case 'text':
        if (hasSpan(el.children)) {
          const spans = [];
          collectSpans(el.children, {}, spans);
          f[fi++] = OP.richText;
          props(p, null, false);
          reserve(8 + spans.length * 6);
          f[fi++] = spans.length;
          for (const [text, flags, c, bg] of spans) {
            strRef(text);
            f[fi++] = flags;
            f[fi++] = c;
            f[fi++] = bg;
          }
        } else {
          f[fi++] = OP.text;
          strRef(collectText(el.children, ''));
          props(p, null, false);
        }
        return;
      case 'slot': {
        // A position an extension fills, in place (ADR 0014). `name` is the
        // full `namespace/slot`: the namespace the host loaded the plugin
        // under (`addExtension`, or a window's `extensions` option) and the
        // slot in the plugin's own vocabulary. `params` is whatever the
        // plugin should read this frame — plain data, declared every frame,
        // retained by nobody, exactly like `onClick`'s payload.
        if (typeof p.name !== 'string' || p.name === '') {
          throw new Error('<slot> needs a name ("namespace/slot")');
        }
        if (!p.name.includes('/')) {
          throw new Error(`bad slot name ${JSON.stringify(p.name)} (a full "namespace/slot")`);
        }
        for (const k of Object.keys(p)) {
          if (k !== 'name' && k !== 'params' && k !== 'children') {
            throw new Error(`<slot> takes name and params, not ${JSON.stringify(k)} — it is a position, not a box`);
          }
        }
        reserve(6);
        f[fi++] = OP.slot;
        strRef(p.name);
        strRef(p.params != null ? JSON.stringify(p.params) : null);
        return;
      }
      case 'span':
        throw new Error('<span> only works inside <text>');
      case 'button':
        f[fi++] = OP.button;
        strRef(collectText(el.children, ''));
        strRef(el.key);
        strRef(p.onClick != null ? JSON.stringify(p.onClick) : null);
        // The access rows the stock button admits, and only those: the
        // decoder reads them over `widgets::button_spec`, so the button
        // keeps its look and takes its name, description, tooltip and
        // disabled state from the view.
        props(p, null, false, BUTTON_ROWS);
        return;
      case 'edit': {
        const label = el.key ?? p.id;
        if (label == null) throw new Error('<edit> needs a key or id prop (state is retained by key)');
        f[fi++] = OP.edit;
        strRef(label);
        strRef(p.initial ?? '');
        f[fi++] = (p.multiline ? 1 : 0) | (p.autofocus ? 2 : 0);
        props(p, null, false);
        return;
      }
      case 'image': {
        if (typeof p.src !== 'string') throw new Error('<image> needs a src (an id from addImage)');
        const id = BigInt('0x' + p.src);
        // The two rows that say how the pixels meet the box
        // (docs/adr/0025-the-image-is-the-canvas.md, decision 4); absent
        // is the first entry of each table.
        const sampling = p.sampling == null ? 0 : IMAGE_SAMPLING[p.sampling];
        if (sampling === undefined) throw new Error(`sampling must be ${P.imageSampling.join(' | ')}`);
        const fit = p.fit == null ? 0 : IMAGE_FIT[p.fit];
        if (fit === undefined) throw new Error(`fit must be ${P.imageFit.join(' | ')}`);
        reserve(6);
        f[fi++] = OP.image;
        f[fi++] = Number(id >> 32n);
        f[fi++] = Number(id & 0xffffffffn);
        f[fi++] = sampling;
        f[fi++] = fit;
        props(p, null, false);
        return;
      }
      case 'polygon': {
        // Up to eight [x, y] pairs; the fill is `bg`, a schema row the
        // props pass writes into the style, and the core decides the box
        // (docs/adr/0025-the-image-is-the-canvas.md, decision 6). More
        // than eight are sent and dropped by the core with its warning.
        const pts = p.points;
        if (!Array.isArray(pts) || pts.length < 3) throw new Error('<polygon> needs at least three points');
        reserve(4 + pts.length * 2);
        f[fi++] = OP.polygon;
        f[fi++] = pts.length;
        for (const pt of pts) {
          if (!Array.isArray(pt) || pt.length !== 2 || typeof pt[0] !== 'number' || typeof pt[1] !== 'number') {
            throw new Error(`bad point ${JSON.stringify(pt)} for <polygon> (an [x, y] pair)`);
          }
          f[fi++] = pt[0];
          f[fi++] = pt[1];
        }
        props(p, el.key, false);
        return;
      }
      case 'fragment': {
        // A box the registered WGSL `src` paints. An open node, so its
        // children paint over it and it closes like any parent
        // (docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md).
        if (typeof p.src !== 'string') throw new Error('<fragment> needs a src (an id from addFragment)');
        const params = p.params ?? [];
        if (!Array.isArray(params)) throw new Error('<fragment> params must be an array of numbers');
        for (const n of params) {
          if (typeof n !== 'number' || !Number.isFinite(n)) throw new Error(`<fragment> bad param ${JSON.stringify(n)}`);
        }
        const id = BigInt('0x' + p.src);
        // The image the function samples through `kui_sample` (backlog V1,
        // docs/adr/0025-the-image-is-the-canvas.md decision 7); absent is
        // two zero words.
        if (p.image != null && typeof p.image !== 'string') throw new Error('<fragment> image must be an id from addImage');
        const image = p.image == null ? 0n : BigInt('0x' + p.image);
        reserve(6 + params.length);
        f[fi++] = OP.fragment;
        f[fi++] = Number(id >> 32n);
        f[fi++] = Number(id & 0xffffffffn);
        f[fi++] = Number(image >> 32n);
        f[fi++] = Number(image & 0xffffffffn);
        f[fi++] = params.length;
        for (const n of params) f[fi++] = n;
        props(p, el.key, false);
        children(el.children);
        reserve(4);
        f[fi++] = OP.close;
        return;
      }
      case 'line': {
        // `from`/`to` or `points`, each an [x, y] pair; `width` is the
        // stroke width and `color` the stroke colour, the latter a schema
        // row the props pass writes into the style. `width` also rides
        // the props pass as a sizing, harmlessly: the core sizes a line
        // to its own box (docs/adr/0010-a-segment-primitive.md).
        let pts = p.points;
        if (pts == null) {
          if (!Array.isArray(p.from) || !Array.isArray(p.to)) throw new Error('<line> needs from and to, or points');
          pts = [p.from, p.to];
        }
        if (!Array.isArray(pts) || pts.length < 2) throw new Error('<line> needs at least two points');
        if (p.width !== undefined && typeof p.width !== 'number') throw new Error(`bad width ${JSON.stringify(p.width)} for <line> (a stroke width in px)`);
        reserve(6 + pts.length * 2);
        f[fi++] = OP.line;
        f[fi++] = pts.length;
        for (const pt of pts) {
          if (!Array.isArray(pt) || pt.length !== 2 || typeof pt[0] !== 'number' || typeof pt[1] !== 'number') {
            throw new Error(`bad point ${JSON.stringify(pt)} for <line> (an [x, y] pair)`);
          }
          f[fi++] = pt[0];
          f[fi++] = pt[1];
        }
        f[fi++] = typeof p.width === 'number' ? p.width : 1;
        f[fi++] = p.curve ? 1 : 0;
        props(p, el.key, false);
        return;
      }
      case 'cells': {
        // rows × cols cells as four entries each — codepoint, fg, bg,
        // flags — in a Uint32Array or a plain array, row-major; the
        // stream carries three slots a cell (codepoint | flags << 21, fg,
        // bg). The style rows ride the props pass; the cursor names a
        // cell to paint under its glyph.
        const rows = p.rows | 0;
        const cols = p.cols | 0;
        if (!(rows > 0 && cols > 0)) throw new Error('<cells> needs rows and cols');
        const cells = p.cells;
        const n = rows * cols;
        if (cells == null || cells.length !== n * 4) {
          throw new Error(`<cells> needs a cells array of ${n * 4} entries (four per cell) for ${rows}×${cols}, got ${cells?.length}`);
        }
        reserve(13 + n * 3);
        f[fi++] = OP.cells;
        f[fi++] = rows;
        f[fi++] = cols;
        const cur = Array.isArray(p.cursorAt) && p.cursorAt.length === 2 ? p.cursorAt : null;
        f[fi++] = cur ? 1 : 0;
        f[fi++] = cur ? cur[0] | 0 : 0;
        f[fi++] = cur ? cur[1] | 0 : 0;
        const shape = p.cursorShape == null ? 0 : ['block', 'bar', 'underline'].indexOf(p.cursorShape);
        if (shape < 0) throw new Error(`bad cursorShape ${JSON.stringify(p.cursorShape)} for <cells> (block, bar or underline)`);
        f[fi++] = shape;
        f[fi++] = p.cursorColor != null ? color(p.cursorColor) : 0xffffffff;
        // The absolute line row 0 is: what makes a selection in a
        // scrolling terminal keep its ends (ADR 0017, decision 4).
        f[fi++] = typeof p.originLine === 'number' ? p.originLine : 0;
        f[fi++] = n;
        for (let i = 0; i < n; i++) {
          const ch = cells[i * 4] >>> 0;
          const flags = cells[i * 4 + 3] & 0xff;
          f[fi++] = (ch & 0x1fffff) + flags * 0x200000;
          f[fi++] = cells[i * 4 + 1] >>> 0;
          f[fi++] = cells[i * 4 + 2] >>> 0;
        }
        props(p, el.key, false);
        return;
      }
      case 'audio': {
        if (typeof p.src !== 'string') throw new Error('<audio> needs a src (an id from addSound)');
        const id = BigInt('0x' + p.src);
        f[fi++] = OP.audio;
        strRef(el.key == null ? null : String(el.key));
        f[fi++] = Number(id >> 32n);
        f[fi++] = Number(id & 0xffffffffn);
        // 4 is `finish`: removal releases the playback instead of stopping it.
        f[fi++] = (p.loop ? 1 : 0) | (p.paused ? 2 : 0) | (p.finish ? 4 : 0);
        f[fi++] = typeof p.volume === 'number' ? p.volume : -1;
        strRef(p.tag !== undefined ? JSON.stringify(p.tag) : null);
        return;
      }
      case 'titlebar': {
        const kids = el.children;
        const empty = kids == null || (Array.isArray(kids) && kids.length === 0);
        f[fi++] = OP.titlebar;
        strRef(p.title);
        f[fi++] = empty ? 0 : 1;
        if (!empty) {
          children(kids);
          reserve(4);
          f[fi++] = OP.close;
        }
        return;
      }
      case 'windowButtons':
        f[fi++] = OP.windowButtons;
        return;
      case 'menuBar':
        // The menu rides on the element, as one JSON blob: it nests, and
        // every row has five optional fields, so the addon parses it with
        // the same reader `openMenu`'s items go through rather than a
        // second hand-written stanza that could disagree with it.
        f[fi++] = OP.menuBar;
        strRef(JSON.stringify(p.menu ?? []));
        return;
      case 'latencyGraph':
        f[fi++] = OP.latencyGraph;
        return;
      case 'latencyHud': {
        const at = Array.isArray(p.at) && p.at.length === 2 ? p.at : ['end', 'end'];
        f[fi++] = OP.latencyHud;
        f[fi++] = alignOf(at[0]);
        f[fi++] = alignOf(at[1]);
        return;
      }

      case undefined:
        throw new Error('element without a type — did it come from kui/jsx-runtime?');
      default:
        throw new Error(`unknown element <${el.type}>`);
    }
  }

  return {
    // `tokenMap` is the surface's `Map<name, { kind, index }>` from its
    // `setTokens` (`index.js`), or nothing for a surface that declared none
    // — the roles still resolve.
    encode(tree, tokenMap) {
      fi = 0;
      ui = 0;
      unknown = [];
      unknownTokens = [];
      tokens = tokenMap ?? null;
      reserve(96);
      f[fi++] = VERSION;
      f[fi++] = OP.root;
      if (tree != null && typeof tree === 'object' && !Array.isArray(tree) && tree.type === 'box') {
        checkProps('box', tree.props ?? {});
        props(tree.props ?? {}, null, true);
        children(tree.children);
      } else {
        f[fi++] = 0; // default column root, no props
        children(tree);
      }
      reserve(4);
      f[fi++] = OP.end;
      // `unknown` is the names this pass had no id for — `[element, name]`
      // pairs the caller hands to `warnUnknownProps` — and `unknownTokens`
      // the `$name`s that resolved to nothing, `[name, kind]` pairs for
      // `warnUnknownTokens`. Like the buffers, both are valid until the
      // next encode().
      return { stream: f.subarray(0, fi), strings: u.subarray(0, ui), unknown, unknownTokens };
    },
    // One `<text>` element and nothing else — what `measureText` sends, so
    // a measured label is flattened and its style read by the same code
    // that lowers the drawn one. `content` is what a `<text>` would hold
    // (strings, numbers, `<span>`s), `style` its props.
    encodeText(content, style, tokenMap) {
      fi = 0;
      ui = 0;
      unknown = [];
      unknownTokens = [];
      tokens = tokenMap ?? null;
      reserve(96);
      f[fi++] = VERSION;
      element({ type: 'text', props: style ?? {}, children: content });
      return { stream: f.subarray(0, fi), strings: u.subarray(0, ui), unknown, unknownTokens };
    },
    // How many role indices sit in front of the app's own, per kind: what
    // `index.js` adds to a declaration's position to get its wire index.
    roleCounts: { colors: COLOR_ROLES, lengths: LENGTH_ROLES },
    // Whether a name is a role's, under Node's spelling: what the core
    // refuses in a declaration (`reserved-token`), so `index.js` gives it
    // no index either — an index it took would shift every name after it.
    isRole: (name) => ROLE_TOKENS.has(name),
    // The kind a role is, or `undefined` for a name that is none: what a
    // derived token's source may be besides an earlier colour (ADR 0028).
    roleKind: (name) => ROLE_TOKENS.get(name)?.kind,
  };
}
