// Keep input and the app instance on the original canvas when the GPU
// surface goes away. Only the drawing surface changes.
export function packedGpu(canvas, runtime) {
  let surface;
  let renderer;
  let failure;
  let memory;
  let wake;

  async function attach() {
    if (!navigator.gpu || new URLSearchParams(location.search).get("renderer") === "canvas") return;
    try {
      const url = new URL("runner-gpu.js", runtime);
      url.search = runtime.search;
      const module = await import(url.href);
      const binary = new URL("runner-gpu_bg.wasm", url);
      binary.search = url.search;
      await module.default({ module_or_path: binary });
      surface = document.createElement("canvas");
      surface.width = canvas.width;
      surface.height = canvas.height;
      surface.setAttribute("aria-hidden", "true");
      surface.style.cssText = "position:absolute;inset:0;pointer-events:none";
      canvas.after(surface);
      renderer = await module.PackedRenderer.attach(surface, message => {
        failure = message || "WebGPU device lost";
        wake?.();
      });
    } catch (error) {
      fallback(error);
    }
  }

  function fallback(error) {
    console.warn("kui-wgpu failed. Using Canvas 2D.", error);
    surface?.remove();
    renderer?.free();
    renderer = undefined;
    document.querySelector("#renderer").textContent = "Canvas 2D";
  }

  function consume(operation) {
    if (failure !== undefined) return 2;
    try {
      return operation();
    } catch (error) {
      failure = error ?? "GPU renderer failed";
      return 2;
    }
  }

  const view = (ptr, len) => new Uint8Array(memory.buffer, ptr, len);
  const decoder = new TextDecoder();
  return {
    ready: attach(),
    active: () => renderer !== undefined,
    imports: {
      upload_atlas(size, x, y, w, h, ptr, len) {
        consume(() => renderer.upload_atlas(size, x, y, w, h, view(ptr, len)));
      },
      upload_texture(id, rev, w, h, ptr, len) {
        consume(() => renderer.upload_texture(id, rev, w, h, view(ptr, len)));
      },
      upload_fragment(id, ptr, len) {
        consume(() => renderer.upload_fragment(id, decoder.decode(view(ptr, len))));
      },
      render(ptr, len) {
        return consume(() => renderer.render(view(ptr, len), surface.width, surface.height));
      },
    },
    present(wasm, width, height, scale, time, redraw, backingWidth, backingHeight, schedule) {
      if (renderer) {
        wake = schedule;
        if (failure === undefined) {
          memory = wasm.memory;
          if (surface.width !== backingWidth) surface.width = backingWidth;
          if (surface.height !== backingHeight) surface.height = backingHeight;
          const next = wasm.frame_for_gpu(width, height, scale, time);
          if (next !== 2) return next;
        }
        fallback(failure ?? "could not present the packed frame");
        redraw = 1;
      }
      return wasm.frame(width, height, scale, time, redraw);
    },
  };
}
