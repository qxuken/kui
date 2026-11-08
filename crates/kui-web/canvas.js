import { editorBridge } from "./editor.js";

const TINT_BYTES = 16 * 1024 * 1024;
const TOUCH_SLOP = 8;

// Canvas 2D paints kui's physical-pixel quads. The app never draws DOM text.
function painter(canvas, restoreCanvas) {
  const surface = canvas.getContext("2d", { alpha: false });
  if (!surface) throw new Error("Canvas 2D is unavailable");
  const staging = document.createElement("canvas");
  const stagingCtx = staging.getContext("2d", { alpha: false });
  staging.addEventListener("contextrestored", restoreCanvas);
  let ctx = surface;
  let stagedRect;
  let damageClip = false;
  const atlas = document.createElement("canvas");
  const atlasCtx = atlas.getContext("2d");
  const mask = document.createElement("canvas");
  const maskCtx = mask.getContext("2d");
  const tints = new Map();
  const textures = new Map();
  let clipped = false;
  let frame = 0;

  const rgba = (r, g, b, a) => `rgba(${r * 255},${g * 255},${b * 255},${a})`;
  const byte = (channel) => Math.round(Math.max(0, Math.min(1, channel)) * 255);
  const round = (x, y, w, h, tl, tr, br, bl) => {
    ctx.beginPath();
    ctx.roundRect(x, y, w, h, [tl, tr, br, bl]);
  };

  // Composite an atlas region once per tint, then draw all its glyphs
  // from the same canvas. Clip destination-in so it preserves other slots.
  function tintRegion(tint, x, y, w, h) {
    const layer = tint.context;
    layer.save();
    layer.beginPath();
    layer.rect(x, y, w, h);
    layer.clip();
    layer.clearRect(x, y, w, h);
    layer.fillStyle = tint.color;
    layer.fillRect(x, y, w, h);
    layer.globalCompositeOperation = "destination-in";
    layer.drawImage(atlas, x, y, w, h, x, y, w, h);
    layer.restore();
  }

  function tintedAtlas(r, g, b) {
    const key = (byte(r) << 16) | (byte(g) << 8) | byte(b);
    let tint = tints.get(key);
    if (!tint) {
      const size = atlas.width;
      const capacity = Math.floor(TINT_BYTES / (4 * size * size));
      if (capacity === 0) return null;
      if (tints.size >= capacity) {
        // Reuse a page only once its colour has gone unused for a frame.
        // With a larger active palette, fall back rather than thrashing.
        let evictedKey;
        for (const [oldKey, page] of tints) {
          if (page.used < frame - 1 && (!tint || page.used < tint.used)) {
            tint = page;
            evictedKey = oldKey;
          }
        }
        if (!tint) return null;
        tints.delete(evictedKey);
      } else {
        const page = document.createElement("canvas");
        page.width = page.height = size;
        tint = { page, context: page.getContext("2d") };
      }
      tint.color = `#${key.toString(16).padStart(6, "0")}`;
      tintRegion(tint, 0, 0, size, size);
      tints.set(key, tint);
    }
    tint.used = frame;
    return tint.page;
  }

  return (memory) => ({
    background(r, g, b, a, x, y, w, h) {
      frame++;
      const partial = w > 0;
      if (partial) {
        const right = Math.min(canvas.width, x + w);
        const bottom = Math.min(canvas.height, y + h);
        x = Math.max(0, x);
        y = Math.max(0, y);
        w = Math.max(0, right - x);
        h = Math.max(0, bottom - y);
      } else {
        [x, y, w, h] = [0, 0, canvas.width, canvas.height];
      }
      // Keep per-quad drawing off the visible canvas, including on
      // full repaints. Only the damaged rectangle is copied back.
      if (a === 1 && stagingCtx && !stagingCtx.isContextLost?.()) {
        if (staging.width !== canvas.width) staging.width = canvas.width;
        if (staging.height !== canvas.height) staging.height = canvas.height;
        ctx = stagingCtx;
        ctx.setTransform(1, 0, 0, 1, -x, -y);
        stagedRect = [x, y, w, h];
      } else if (partial) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(x, y, w, h);
        ctx.clip();
        damageClip = true;
      }
      ctx.fillStyle = rgba(r, g, b, a);
      ctx.fillRect(x, y, w, h);
    },
    present() {
      if (stagedRect) {
        const [x, y, w, h] = stagedRect;
        ctx = surface;
        if (w > 0 && h > 0) ctx.drawImage(staging, 0, 0, w, h, x, y, w, h);
        stagedRect = undefined;
      } else if (damageClip) {
        ctx.restore();
        damageClip = false;
      }
    },
    upload_atlas(ptr, size, x, y, w, h) {
      if (atlas.width !== size || atlas.height !== size) {
        tints.clear();
        atlas.width = atlas.height = size;
        [x, y, w, h] = [0, 0, size, size];
      }
      // Wasm memory can grow between frames. Only borrow it for this upload.
      const bytes = new Uint8ClampedArray(memory().buffer, ptr, size * size * 4);
      atlasCtx.putImageData(new ImageData(bytes, size, size), 0, 0, x, y, w, h);
      for (const tint of tints.values()) tintRegion(tint, x, y, w, h);
    },
    upload_texture(id, ptr, width, height) {
      const page = textures.get(id) ?? document.createElement("canvas");
      if (!textures.has(id)) page.addEventListener("contextrestored", restoreCanvas);
      if (page.width !== width) page.width = width;
      if (page.height !== height) page.height = height;
      const bytes = new Uint8ClampedArray(memory().buffer, ptr, width * height * 4);
      page.getContext("2d").putImageData(new ImageData(bytes, width, height), 0, 0);
      textures.set(id, page);
    },
    drop_texture(id) {
      textures.delete(id);
    },
    image(texture, x, y, w, h, sx, sy, sw, sh, tl, tr, br, bl, r, g, b, a, angle, nearest) {
      if (!sw || !sh || a <= 0) return;
      let source = texture === 0n ? atlas : textures.get(texture);
      if (!source) return;
      if (r !== 1 || g !== 1 || b !== 1) {
        if (mask.width < sw) mask.width = sw;
        if (mask.height < sh) mask.height = sh;
        maskCtx.globalCompositeOperation = "source-over";
        maskCtx.clearRect(0, 0, sw, sh);
        maskCtx.drawImage(source, sx, sy, sw, sh, 0, 0, sw, sh);
        maskCtx.globalCompositeOperation = "source-in";
        maskCtx.fillStyle = rgba(r, g, b, 1);
        maskCtx.fillRect(0, 0, sw, sh);
        source = mask;
        sx = sy = 0;
      }
      ctx.save();
      if (angle !== 0) {
        ctx.translate(x + w / 2, y + h / 2);
        ctx.rotate(angle);
        ctx.translate(-x - w / 2, -y - h / 2);
      }
      if (tl || tr || br || bl) {
        round(x, y, w, h, tl, tr, br, bl);
        ctx.clip();
      }
      ctx.imageSmoothingEnabled = !nearest;
      ctx.globalAlpha = a;
      ctx.drawImage(source, sx, sy, sw, sh, x, y, w, h);
      ctx.restore();
    },
    reset_clip() {
      if (clipped) ctx.restore();
      clipped = false;
    },
    clip(x, y, w, h, tl, tr, br, bl) {
      ctx.save();
      round(x, y, w, h, tl, tr, br, bl);
      ctx.clip();
      clipped = true;
    },
    solid(x, y, w, h, tl, tr, br, bl, r, g, b, a, border, rr, rg, rb, ra) {
      round(x, y, w, h, tl, tr, br, bl);
      if (a > 0) {
        ctx.fillStyle = rgba(r, g, b, a);
        ctx.fill();
      }
      if (border > 0 && ra > 0) {
        ctx.save();
        ctx.clip();
        ctx.lineWidth = border * 2;
        ctx.strokeStyle = rgba(rr, rg, rb, ra);
        ctx.stroke();
        ctx.restore();
      }
    },
    glyph(x, y, w, h, sx, sy, sw, sh, r, g, b, a, colored) {
      if (!sw || !sh || a <= 0) return;
      const source = colored ? atlas : tintedAtlas(r, g, b);
      if (source) {
        if (a !== 1) ctx.globalAlpha = a;
        ctx.drawImage(source, sx, sy, sw, sh, x, y, w, h);
        if (a !== 1) ctx.globalAlpha = 1;
        return;
      }
      if (mask.width < sw) mask.width = sw;
      if (mask.height < sh) mask.height = sh;
      maskCtx.globalCompositeOperation = "source-over";
      maskCtx.clearRect(0, 0, sw, sh);
      maskCtx.drawImage(atlas, sx, sy, sw, sh, 0, 0, sw, sh);
      maskCtx.globalCompositeOperation = "source-in";
      maskCtx.fillStyle = rgba(r, g, b, a);
      maskCtx.fillRect(0, 0, sw, sh);
      ctx.drawImage(mask, 0, 0, sw, sh, x, y, w, h);
    },
  });
}

/** Mount one Wasm app for the lifetime of its canvas. */
export async function startCanvas(canvas, wasmUrl, { access, renderer = "auto", initialize, afterFrame, contentScale = () => 1, panOutside, instantiate = WebAssembly.instantiate } = {}) {
  let wasm;
  let snapshot;
  let clipboardText;
  let scheduled = false;
  let needsRedraw = false;
  let reconnect;
  let recovering = false;
  const decoder = new TextDecoder();
  const read = (ptr, len) => decoder.decode(new Uint8Array(wasm.memory.buffer, ptr, len));
  const browserCallbacks = {
    access_tree(ptr, len) { snapshot = JSON.parse(read(ptr, len)); },
    clipboard_text(ptr, len) { clipboardText = read(ptr, len); },
  };
  let selectedRenderer = "canvas";
  let frame;
  let firstFrame;
  const seconds = () => performance.now() / 1000;
  if (renderer === "wgpu" || (renderer === "auto" && navigator.gpu)) {
    try {
      if (!navigator.gpu) throw new Error("WebGPU is unavailable");
      const moduleUrl = new URL(wasmUrl, document.baseURI);
      moduleUrl.pathname = moduleUrl.pathname.replace(/\.wasm$/, "-wgpu.js");
      const module = await import(moduleUrl.href);
      wasm = await module.default();
      initialize?.(wasm);
      reconnect = () => module.attach_wgpu(canvas, browserCallbacks.access_tree, browserCallbacks.clipboard_text);
      await reconnect();
      setPlatform();
      frame = (width, height, scale, redraw, w, h) => module.frame_wgpu(width, height, scale, seconds(), w, h);
      firstFrame = paintFrame();
      if (firstFrame === 2) throw new Error("WebGPU device lost before the first frame");
      selectedRenderer = "wgpu";
    } catch (error) {
      if (renderer === "wgpu") throw error;
      console.warn("kui-wgpu startup failed. Using Canvas 2D.", error);
      reconnect = undefined;
      frame = undefined;
      wasm = undefined;
      // A canvas that already supplied a WebGPU context needs a new
      // element before Canvas 2D can take over.
      if (!canvas.getContext("2d", { alpha: false })) {
        const replacement = canvas.cloneNode(false);
        canvas.replaceWith(replacement);
        canvas = replacement;
      }
    }
  }
  if (!frame) {
    const paint = painter(canvas, restoreCanvas);
    const imports = { canvas: { ...paint(() => wasm.memory), ...browserCallbacks } };
    const response = await fetch(wasmUrl);
    if (!response.ok) throw new Error(`Wasm fetch failed (${response.status})`);
    ({ instance: { exports: wasm } } = await instantiate(await response.arrayBuffer(), imports));
    initialize?.(wasm);
    setPlatform();
    frame = (width, height, scale, redraw) => wasm.frame(width, height, scale, seconds(), redraw);
    firstFrame = paintFrame();
  }

  function setPlatform() {
    if (access) wasm.set_mac_conventions(Number(/Mac|iPhone|iPad/.test(navigator.platform)));
  }

  const bridge = access && editorBridge(access, canvas, wasm, schedule, seconds, (cut) => {
    clipboardText = undefined;
    wasm.clipboard_selection(Number(cut), seconds());
    return clipboardText;
  });
  if (bridge) {
    canvas.tabIndex = -1;
    canvas.setAttribute("aria-hidden", "true");
    const host = canvas.parentElement;
    host.addEventListener("focusout", (event) => {
      if (!host.contains(event.relatedTarget) && (event.relatedTarget || document.hasFocus())) bridge.blur();
    });
    const windowFocus = (focused) => {
      wasm.window_focus(Number(focused), seconds());
      schedule();
    };
    window.addEventListener("blur", () => windowFocus(false));
    window.addEventListener("focus", () => windowFocus(host.contains(document.activeElement)));
  }
  function restoreCanvas() {
    needsRedraw = true;
    if (wasm) schedule();
  }
  function schedule() {
    if (scheduled || recovering) return;
    scheduled = true;
    requestAnimationFrame(draw);
  }
  function syncAccess() {
    if (!bridge) return;
    snapshot = undefined;
    wasm.mirror_access();
    bridge.sync(snapshot);
  }
  function paintFrame() {
    const { width, height } = canvas.getBoundingClientRect();
    if (width === 0 || height === 0) return;
    const zoom = contentScale();
    const scale = (window.devicePixelRatio || 1) * zoom;
    const w = Math.max(1, Math.round(width * scale));
    const h = Math.max(1, Math.round(height * scale));
    const redraw = needsRedraw || (!reconnect && (canvas.width !== w || canvas.height !== h));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
    needsRedraw = false;
    return frame(width / zoom, height / zoom, scale, Number(redraw), w, h);
  }

  let recoveryDelay = 0;
  function recoverDevice() {
    recovering = true;
    setTimeout(async () => {
      recoveryDelay = Math.min(2000, Math.max(100, recoveryDelay * 2));
      try {
        await reconnect();
        recovering = false;
        schedule();
      } catch (error) {
        console.warn("Retrying wgpu device recovery.", error);
        recoverDevice();
      }
    }, recoveryDelay);
  }

  let wakeTimer;
  function scheduleWake() {
    if (!wasm.next_frame_at) return;
    clearTimeout(wakeTimer);
    const at = wasm.next_frame_at();
    if (at > 0) wakeTimer = setTimeout(schedule, Math.max(0, (at - seconds()) * 1000));
  }

  function draw() {
    scheduled = false;
    let next;
    try {
      next = paintFrame();
    } catch (error) {
      if (!reconnect) throw error;
      console.warn("wgpu frame failed. Reconnecting.", error);
      recoverDevice();
      return;
    }
    if (next === 2 && reconnect) {
      recoverDevice();
      return;
    }
    if (next === undefined) return;
    recoveryDelay = 0;
    if (next) schedule();
    afterFrame?.(wasm, canvas, schedule);
    scheduleWake();
    syncAccess();
  }

  const move = (event, time) => {
    const rect = canvas.getBoundingClientRect();
    const pointer = wasm.pointer_move((event.clientX - rect.left) / contentScale(), (event.clientY - rect.top) / contentScale(), time);
    canvas.style.cursor = pointer ? "pointer" : "default";
  };
  const leave = (time) => {
    wasm.pointer_leave(time);
    canvas.style.cursor = "default";
    schedule();
  };
  const press = (event, clicks, time) => {
    if (!access?.contains(document.activeElement)) canvas.focus({ preventScroll: true });
    move(event, time);
    wasm.pointer_button(Number(event.button === 2), 1, time, clicks);
    schedule();
  };
  const release = (event, time) => {
    move(event, time);
    wasm.pointer_button(Number(event.button === 2), 0, time, 0);
    schedule();
  };

  // Wait out finger jitter before pressing anything. A swipe over a
  // scrolling region feeds the same latched scroll path as the wheel.
  let touch;
  let otherPointer = null;
  function moveTouch(event, time) {
    if (touch?.start.pointerId !== event.pointerId) return;
    const dx = event.screenX - touch.last.screenX;
    const dy = event.screenY - touch.last.screenY;
    const begins = !touch.moving;
    if ((!dx && !dy) || (begins && Math.hypot(dx, dy) <= TOUCH_SLOP)) return;
    if (touch.pan) {
      if (begins) move(touch.start, time);
      if (!touch.page) {
        const claimed = wasm.scroll_gesture(dx / contentScale(), dy / contentScale(), Number(begins), time);
        if (begins && !claimed && panOutside) touch.page = true;
      }
      if (touch.page) panOutside(-dy);
    } else {
      if (begins) press(touch.start, 1, time);
      move(event, time);
    }
    touch.moving = true;
    touch.last = event;
    schedule();
  }
  function cancelTouch() {
    const cancelled = touch;
    touch = undefined;
    if (!cancelled?.moving) return;
    const time = seconds();
    leave(time);
    if (!cancelled.pan) wasm.pointer_button(0, 0, time, 0);
  }

  canvas.addEventListener("pointermove", (event) => {
    if (event.pointerType === "touch") {
      moveTouch(event, seconds());
    } else if (event.isPrimary && !touch) {
      move(event, seconds());
      schedule();
    }
  });
  canvas.addEventListener("pointerleave", (event) => {
    if (event.isPrimary && event.pointerType !== "touch" && !touch) leave(seconds());
  });
  let mousePressPending = false;
  canvas.addEventListener("pointerdown", (event) => {
    if (event.pointerType === "touch") {
      if (!event.isPrimary) {
        cancelTouch();
        return;
      }
      if (otherPointer !== null) return;
      event.preventDefault();
      canvas.setPointerCapture(event.pointerId);
      const rect = canvas.getBoundingClientRect();
      const pan = !!wasm.prefers_scroll(
        (event.clientX - rect.left) / contentScale(),
        (event.clientY - rect.top) / contentScale(),
        Number(!!panOutside),
      );
      touch = { start: event, last: event, pan, moving: false };
      return;
    }
    if (touch) return;
    if (!event.isPrimary || (event.button !== 0 && event.button !== 2)) return;
    otherPointer = event.pointerId;
    canvas.setPointerCapture(event.pointerId);
    // Pointer events keep capture and movement. Mouse down supplies the
    // browser's click count before a double-click drag can begin.
    if (event.pointerType === "mouse") mousePressPending = true;
    else {
      event.preventDefault();
      press(event, 1, seconds());
    }
  });
  canvas.addEventListener("mousedown", (event) => {
    if (!mousePressPending) return;
    mousePressPending = false;
    if (bridge) event.preventDefault();
    press(event, event.detail, seconds());
  });
  canvas.addEventListener("pointerup", (event) => {
    if (event.pointerType === "touch") {
      if (touch?.start.pointerId !== event.pointerId) return;
      const time = seconds();
      moveTouch(event, time);
      const { start, moving, pan } = touch;
      touch = undefined;
      if (!moving) press(start, 1, time);
      if (!moving || !pan) {
        release(event, time);
        // Keep editor focus inside the trusted tap, so a browser can
        // open its software keyboard without waiting for a frame.
        syncAccess();
      }
      leave(time);
      return;
    }
    if (!event.isPrimary || (event.button !== 0 && event.button !== 2)) return;
    if (touch) return;
    otherPointer = null;
    mousePressPending = false;
    release(event, seconds());
  });
  const cancelPointer = (event) => {
    if (touch?.start.pointerId === event.pointerId) {
      cancelTouch();
      return;
    }
    if (otherPointer !== event.pointerId) return;
    otherPointer = null;
    mousePressPending = false;
    const time = seconds();
    leave(time);
    wasm.pointer_button(0, 0, time, 0);
  };
  canvas.addEventListener("pointercancel", cancelPointer);
  canvas.addEventListener("lostpointercapture", cancelPointer);
  if (bridge) canvas.addEventListener("wheel", (event) => {
    if (event.ctrlKey) return; // Browser zoom remains available.
    const time = seconds();
    move(event, time);
    const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? canvas.clientHeight : 1 / contentScale();
    if (wasm.scroll(-event.deltaX * unit, -event.deltaY * unit, time)) event.preventDefault();
    schedule();
  }, { passive: false });
  canvas.addEventListener("contextmenu", (event) => event.preventDefault());
  canvas.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    wasm.escape(seconds());
    schedule();
  });
  new ResizeObserver(schedule).observe(canvas);
  window.addEventListener("resize", schedule);
  canvas.addEventListener("contextrestored", restoreCanvas);
  if (firstFrame) schedule();
  if (firstFrame !== undefined) {
    afterFrame?.(wasm, canvas, schedule);
    scheduleWake();
    syncAccess();
  }
  return selectedRenderer;
}
