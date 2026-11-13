import { mountDemo } from "./mount.js";
import { appFilesystem } from "./filesystem.js";
import { packedGpu } from "./packed-gpu.js";

const status = document.querySelector("#status");
const main = document.querySelector("main");
const embedded = window.parent !== window && new URLSearchParams(location.search).has("embed");

function notify(state, fields = {}) {
  if (embedded) window.parent.postMessage({ type: "kui-example", state, ...fields }, location.origin);
}

async function readBundle() {
  const response = await fetch("./app.kuiweb", { cache: "no-cache" });
  if (!response.ok) throw new Error(`App bundle fetch failed (${response.status})`);
  const bytes = await response.arrayBuffer();
  const length = new DataView(bytes).getUint32(0, true);
  let offset = 4 + length;
  const manifest = JSON.parse(new TextDecoder().decode(new Uint8Array(bytes, 4, length)));
  if (manifest.schema !== 1) throw new Error("Unsupported web bundle version");
  const files = manifest.files.map(([name, size]) => {
    const data = new Uint8Array(bytes, offset, size);
    offset += size;
    return [name, data];
  });
  if (offset !== bytes.byteLength) throw new Error("Invalid web bundle length");
  const { files: _, ...app } = manifest;
  return { app, files };
}

try {
  const { app, files } = await readBundle();
  document.title = app.name;
  main.querySelector("#access").setAttribute("aria-label", app.name);
  const filesystem = await appFilesystem(app.id, files, error => {
    status.textContent = `Could not save application data. ${error.message}`;
    status.hidden = false;
  });
  const metadata = new TextEncoder().encode(JSON.stringify(app));
  let scale = 1;
  let dock;
  function fit() {
    const minWidth = app.minWidth ?? Math.min(320, app.size[0]);
    scale = Math.min(1, Math.max(innerWidth, 1) / minWidth);
    main.style.zoom = scale;
    main.style.width = `${innerWidth / scale}px`;
    main.style.height = `${innerHeight / scale}px`;
  }
  function reportSize(height) {
    if (dock === height) return;
    dock = height;
    notify("size", {
      width: app.size[0], minWidth: app.minWidth ?? Math.min(320, app.size[0]),
      height: app.size[1] + height, devtools: height > 0,
    });
  }
  window.addEventListener("resize", fit);
  fit();
  reportSize(0);
  let wired = false;
  const runtime = new URL("./runner.wasm", import.meta.url);
  runtime.searchParams.set("v", new URL(import.meta.url).searchParams.get("v") ?? app.runner);
  const gpu = packedGpu(main.querySelector("canvas"), runtime);
  await mountDemo(runtime, {
    contentScale: () => scale,
    instantiate: async (bytes, imports) => {
      await gpu.ready;
      return filesystem.instantiate(bytes, { ...imports, kui_gpu: gpu.imports });
    },
    present: gpu.present,
    panOutside: dy => notify("pan", { dy }),
    initialize(wasm) {
      const ptr = wasm.text_buffer(metadata.length);
      new Uint8Array(wasm.memory.buffer, ptr, metadata.length).set(metadata);
      const error = wasm.select_app();
      if (error) {
        const message = new Uint8Array(wasm.memory.buffer, wasm.text_buffer(error), error);
        throw new Error(new TextDecoder().decode(message));
      }
    },
    afterFrame(wasm, _canvas, schedule) {
      if (embedded && !wired) {
        wired = true;
        window.addEventListener("message", event => {
          if (event.origin !== location.origin || event.source !== window.parent ||
              event.data?.type !== "kui-example" || event.data.state !== "devtools") return;
          wasm.app_set_devtools(Number(event.data.on === true));
          reportSize(wasm.app_devtools_height());
          schedule();
        });
      }
      if (wasm.run_tasks()) schedule();
      filesystem.flush();
      reportSize(wasm.app_devtools_height());
    },
  });
  if (gpu.active()) document.querySelector("#renderer").textContent = "kui-wgpu";
} catch (error) {
  status.textContent = `Could not load application. ${error.message}`;
  status.hidden = false;
  notify("error", { message: error.message });
  throw error;
}
