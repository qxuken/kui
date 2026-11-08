import { editorBridge } from "./editor.js";

// The overlay is a sibling surface so it can extend below the owner.
export function createPopup(stage, owner, contentScale) {
  const layer = document.createElement("div");
  layer.className = "popup-overlay";
  layer.hidden = true;
  const canvas = document.createElement("canvas");
  canvas.setAttribute("aria-hidden", "true");
  const access = document.createElement("div");
  access.className = "popup-access";
  access.setAttribute("role", "group");
  access.setAttribute("aria-label", "Popup");
  layer.append(canvas, access);
  stage.append(layer);

  let config;
  let wasm;
  let schedule;
  let bridge;
  let ownerPress;
  let popupPress;
  let armed = false;
  let redraw = true;
  const decoder = new TextDecoder();
  const seconds = () => performance.now() / 1000;
  const contains = (e, rect) => e.clientX >= rect.left && e.clientX < rect.right &&
    e.clientY >= rect.top && e.clientY < rect.bottom;
  const overPopup = e => contains(e, canvas.getBoundingClientRect());
  const overAnchor = (e) => {
    const rect = owner.getBoundingClientRect();
    const scale = contentScale();
    const [x, y, w, h] = config.anchor;
    return contains(e, { left: rect.left + x * scale, right: rect.left + (x + w) * scale,
      top: rect.top + y * scale, bottom: rect.top + (y + h) * scale });
  };

  function pointer(kind, event) {
    const rect = canvas.getBoundingClientRect();
    const scale = contentScale();
    const hand = wasm.demo_popup_pointer(kind, (event.clientX - rect.left) / scale,
      (event.clientY - rect.top) / scale, seconds());
    canvas.style.cursor = hand ? "pointer" : "default";
    schedule();
  }
  function dismiss(escape = false) {
    if (!config) return;
    wasm.demo_popup_dismiss(Number(escape), seconds());
    schedule();
  }

  stage.addEventListener("pointerdown", event => {
    if (!event.isPrimary) return;
    if (config && !layer.contains(event.target)) {
      event.preventDefault();
      event.stopImmediatePropagation();
      dismiss();
    } else if (event.target === owner && event.button === 0) {
      ownerPress = event.pointerId;
    }
  }, true);
  owner.addEventListener("pointermove", event => {
    if (armed && ownerPress === event.pointerId) pointer(overPopup(event) ? 0 : 1, event);
  });
  owner.addEventListener("pointerup", event => {
    if (event.pointerId !== ownerPress) return;
    ownerPress = undefined;
    if (!armed) return;
    armed = false;
    if (overPopup(event)) {
      // The owner's pointer capture kept the opening press. Synthesize
      // the popup's own click at the release, as the desktop host does.
      pointer(0, event);
      pointer(2, event);
      pointer(3, event);
    } else if (!overAnchor(event)) {
      dismiss();
    }
  });
  for (const type of ["pointercancel", "lostpointercapture"]) {
    owner.addEventListener(type, event => {
      if (ownerPress !== event.pointerId) return;
      ownerPress = undefined;
      if (armed) dismiss();
      armed = false;
    });
    canvas.addEventListener(type, event => {
      if (popupPress !== event.pointerId) return;
      popupPress = undefined;
      pointer(1, event);
      pointer(3, event);
      dismiss();
    });
  }
  canvas.addEventListener("pointerdown", event => {
    if (!event.isPrimary || event.button !== 0) return;
    event.preventDefault();
    popupPress = event.pointerId;
    canvas.setPointerCapture(popupPress);
    pointer(0, event);
    pointer(2, event);
  });
  canvas.addEventListener("pointermove", event => pointer(overPopup(event) ? 0 : 1, event));
  canvas.addEventListener("pointerleave", event => pointer(1, event));
  canvas.addEventListener("pointerup", event => {
    if (popupPress !== event.pointerId) return;
    popupPress = undefined;
    const inside = overPopup(event);
    pointer(inside ? 0 : 1, event);
    pointer(3, event);
    if (!inside && !overAnchor(event)) dismiss();
  });
  canvas.addEventListener("contextmenu", event => event.preventDefault());
  canvas.addEventListener("contextrestored", () => { redraw = true; schedule(); });
  document.addEventListener("keydown", event => {
    if (config && event.key === "Escape") {
      event.preventDefault();
      event.stopImmediatePropagation();
      dismiss(true);
    }
  }, true);
  window.addEventListener("blur", () => dismiss());

  return {
    canvas,
    get open() { return !!config; },
    afterFrame(exports, requestFrame, drawOverlay) {
      wasm = exports;
      schedule = requestFrame;
      const len = wasm.demo_popup_changes();
      if (len) {
        const ptr = wasm.text_buffer(len);
        config = JSON.parse(decoder.decode(new Uint8Array(wasm.memory.buffer, ptr, len)));
        layer.hidden = !config;
        armed = !!config && ownerPress !== undefined;
        if (!config) {
          popupPress = undefined;
          bridge?.sync({ nodes: [], focus: null });
        }
        redraw = true;
      }
      if (!config) return 0;

      const { anchor, width, height } = config;
      const x = Math.min(anchor[0], Math.max(0, owner.clientWidth - width));
      const y = anchor[1] + anchor[3];
      Object.assign(layer.style, { left: `${x}px`, top: `${y}px`, width: `${width}px`, height: `${height}px` });
      const scale = (window.devicePixelRatio || 1) * contentScale();
      const w = Math.max(1, Math.round(width * scale));
      const h = Math.max(1, Math.round(height * scale));
      if (canvas.width !== w) { canvas.width = w; redraw = true; }
      if (canvas.height !== h) { canvas.height = h; redraw = true; }
      const result = drawOverlay(() => wasm.demo_popup_frame(width, height, scale, seconds(), Number(redraw)));
      redraw = false;
      if (!bridge) {
        bridge = editorBridge(access, owner, {
          ...wasm, focus_node: () => {}, activate_node: wasm.demo_popup_activate,
        }, schedule, seconds, () => undefined);
      }
      if (result.snapshot) {
        // Expose actions while leaving DOM keyboard focus with the owner.
        const nodes = result.snapshot.nodes.map(node => ({ ...node, focusable: false }));
        bridge.sync({ ...result.snapshot, nodes, focus: null });
      }
      if (result.next) schedule();
      return y + height;
    },
  };
}
