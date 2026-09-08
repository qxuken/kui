// kui's JSX runtime (automatic transform target, `jsxImportSource: "@qxuken/kui"`).
// Elements are plain data - `{type, key, props, children}` - because the tree
// IS the frame: Ctx.frame() lowers it into the kui IR in one call. Function
// components are called immediately (stateless, Elm-style); Fragments splice
// into their parent's children.

// A symbol, not the string 'fragment', because `<fragment>` is a real kui
// element now (a box a WGSL function paints,
// docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md) and the
// two would be the same `type`. jsx-runtime.d.ts has always declared this
// as a `unique symbol`, so no typed caller could have depended on the
// string. `Symbol.for` rather than `Symbol()` so two copies of this module
// still agree.
export const Fragment = Symbol.for('kui.jsx.fragment');

function flatten(child, out) {
  if (child == null || child === false || child === true) return;
  if (Array.isArray(child)) {
    for (const c of child) flatten(c, out);
    return;
  }
  out.push(child);
}

export function jsx(type, props, key) {
  props ??= {};
  if (typeof type === 'function') {
    return type(key != null ? { ...props, key: String(key) } : props);
  }
  const { children, ...rest } = props;
  const flat = [];
  flatten(children, flat);
  if (type === Fragment) return flat;
  return {
    type,
    key: key != null ? String(key) : undefined,
    props: rest,
    children: flat,
  };
}

export const jsxs = jsx;
export function jsxDEV(type, props, key) {
  return jsx(type, props, key);
}
