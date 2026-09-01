// Flat binary IR encoder: turns a jsx-runtime tree into one Float64Array
// instruction stream + one UTF-8 string table, matching the decoder in
// crates/kui-node/src/binary.rs. All opcode/prop ids come from the addon's
// protocol() at construction, so the two sides cannot drift. Buffers are
// reused across frames (no per-frame GC churn); encode() returns subarray
// views that are valid until the next encode() on the same encoder.
//
// Semantics mirror the JSON lowering path exactly: same defaults, same
// prop names, same error cases.

const ALIGN = { start: 0, center: 1, end: 2 };
const FLOAT_PRESET = { parent: 0, viewport: 1, below: 2, above: 3 };

export function createEncoder(P) {
  const OP = P.op;
  const PR = P.prop;
  const VERSION = P.version;

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

  function alignOf(v) {
    const a = ALIGN[v];
    if (a === undefined) throw new Error('align must be start | center | end');
    return a;
  }

  // Encodes a prop list, schema-driven: generic props are written by their
  // protocol kind (no names or shapes hardcoded here); only composites (the
  // pad family, border, overflow bits, float) and the constructor-ordering
  // specials (dir, size) have hand-written stanzas, mirroring binary.rs.
  // `key` rides along as P_KEY; `isRoot` admits `title` (and drops `key`).
  function props(p, key, isRoot) {
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
      f[fi++] = PR.size.id;
      f[fi++] = p.size;
      n++;
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
      const v = p[k];
      if (v == null) continue;
      switch (k) {
        case 'dir':
        case 'size':
          break; // handled above
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
          f[fi++] = PR.float.id;
          if (typeof v === 'string') {
            const preset = FLOAT_PRESET[v];
            if (preset === undefined) throw new Error(`bad float ${JSON.stringify(v)}`);
            f[fi++] = preset;
            f[fi++] = 0; f[fi++] = 0; f[fi++] = 0; // no at
            f[fi++] = 0; f[fi++] = 0; f[fi++] = 0; // no self
            f[fi++] = 0; f[fi++] = 0; // dx dy
            f[fi++] = 0; // fit
          } else {
            f[fi++] = v.anchor === 'viewport' ? 1 : 0;
            const at = Array.isArray(v.at) && v.at.length === 2 ? v.at : null;
            f[fi++] = at ? 1 : 0;
            f[fi++] = at ? alignOf(at[0]) : 0;
            f[fi++] = at ? alignOf(at[1]) : 0;
            const self = Array.isArray(v.self) && v.self.length === 2 ? v.self : null;
            f[fi++] = self ? 1 : 0;
            f[fi++] = self ? alignOf(self[0]) : 0;
            f[fi++] = self ? alignOf(self[1]) : 0;
            f[fi++] = v.dx ?? 0;
            f[fi++] = v.dy ?? 0;
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
        case 'tooltip':
          f[fi++] = PR.tooltip.id;
          strRef(String(v));
          n++;
          break;
        default: {
          // Schema-driven: unknown names (element-level props included) are
          // ignored, matching the JSON path.
          const def = PR[k];
          if (def === undefined || def.kind === 'custom') break;
          if (def.kind === 'flag') {
            if (v) {
              f[fi++] = def.id;
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
            case 'msg':
              strRef(JSON.stringify(v));
              break;
            default:
              throw new Error(`unhandled schema kind ${def.kind} for ${k}`);
          }
          n++;
          break;
        }
      }
    }
    if (pad !== undefined || padX !== undefined || padY !== undefined ||
        padL !== undefined || padR !== undefined || padT !== undefined || padB !== undefined) {
      const base = pad ?? 0;
      const px = padX ?? base;
      const py = padY ?? base;
      f[fi++] = PR.pad.id;
      f[fi++] = padL ?? px;
      f[fi++] = padR ?? px;
      f[fi++] = padT ?? py;
      f[fi++] = padB ?? py;
      n++;
    }
    if (borderW !== undefined) {
      f[fi++] = PR.border.id;
      f[fi++] = borderW;
      f[fi++] = borderColor != null ? color(borderColor) : 0;
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

  // Flattens <span> nesting into (text, flags, color) triples, inheritance
  // matching the Rust collect_spans.
  function collectSpans(node, bold, italic, spanColor, out) {
    if (node == null || typeof node === 'boolean') return;
    if (typeof node === 'string' || typeof node === 'number') {
      out.push([String(node), (bold ? 1 : 0) | (italic ? 2 : 0) | (spanColor !== undefined ? 4 : 0), spanColor ?? 0]);
      return;
    }
    if (Array.isArray(node)) {
      for (const c of node) collectSpans(c, bold, italic, spanColor, out);
      return;
    }
    if (node.type !== 'span') throw new Error('only strings and <span> may nest inside rich <text>');
    const p = node.props ?? {};
    collectSpans(
      node.children,
      bold || !!p.bold,
      italic || !!p.italic,
      p.color != null ? color(p.color) : spanColor,
      out,
    );
  }

  function children(node) {
    if (node == null || typeof node === 'boolean') return;
    if (typeof node === 'string' || typeof node === 'number') {
      // Bare text child: a default-styled text node, same as the JSON path.
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

  function element(el) {
    const p = el.props ?? {};
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
          collectSpans(el.children, false, false, undefined, spans);
          f[fi++] = OP.richText;
          props(p, null, false);
          reserve(8 + spans.length * 5);
          f[fi++] = spans.length;
          for (const [text, flags, c] of spans) {
            strRef(text);
            f[fi++] = flags;
            f[fi++] = c;
          }
        } else {
          f[fi++] = OP.text;
          strRef(collectText(el.children, ''));
          props(p, null, false);
        }
        return;
      case 'span':
        throw new Error('<span> only works inside <text>');
      case 'button':
        f[fi++] = OP.button;
        strRef(collectText(el.children, ''));
        strRef(el.key);
        strRef(p.onClick != null ? JSON.stringify(p.onClick) : null);
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
        f[fi++] = OP.image;
        f[fi++] = Number(id >> 32n);
        f[fi++] = Number(id & 0xffffffffn);
        props(p, null, false);
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
      case 'fragment':
        children(el.children);
        return;
      default:
        throw new Error(`unknown element <${el.type}>`);
    }
  }

  return {
    encode(tree) {
      fi = 0;
      ui = 0;
      reserve(96);
      f[fi++] = VERSION;
      f[fi++] = OP.root;
      if (tree != null && typeof tree === 'object' && !Array.isArray(tree) && tree.type === 'box') {
        props(tree.props ?? {}, null, true);
        children(tree.children);
      } else {
        f[fi++] = 0; // default column root, no props
        children(tree);
      }
      reserve(4);
      f[fi++] = OP.end;
      return { stream: f.subarray(0, fi), strings: u.subarray(0, ui) };
    },
  };
}
