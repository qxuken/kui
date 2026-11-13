import { mountDemo } from "../runtime/mount.js";
import { appFilesystem } from "../runtime/filesystem.js";
import { packedGpu } from "../runtime/packed-gpu.js";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const notify = (state, fields = {}) => parent.postMessage({ type: "kui-playground", state, ...fields }, location.origin);
let started = false;
let failed = false;

function failure(message) {
  if (failed) return;
  failed = true;
  notify("error", { message: String(message) });
}
window.addEventListener("error", event => {
  failure(event.error?.message ?? event.message);
  event.preventDefault();
});
window.addEventListener("unhandledrejection", event => {
  failure(event.reason?.message ?? event.reason);
  event.preventDefault();
});

window.addEventListener("message", async event => {
  if (started || event.source !== parent || event.origin !== location.origin ||
      event.data?.type !== "kui-playground" || event.data.state !== "run") return;
  started = true;
  try {
    const { code, runner, runtime, module, bytes } = event.data;
    if (typeof code !== "string" || encoder.encode(code).length > 128 * 1024) throw new Error("Source is limited to 128 KiB");
    const wasmModule = module ?? await WebAssembly.compile(bytes);
    const app = { name: "Lua playground", id: "dev.kui.book.playground", version: "1.0.0", runner, entry: "main.lua", size: [640, 480], playground: true };
    const metadata = encoder.encode(JSON.stringify(app));
    const filesystem = await appFilesystem(app.id, [["main.lua", encoder.encode(code)]], error => failure(error.message), {
      persistent: false,
      stderr: line => failure(line),
    });
    const gpu = packedGpu(document.querySelector("canvas"), new URL(runtime));
    let wired = false;
    let dock;
    await mountDemo(runtime, {
      wasmModule,
      instantiate: async (input, imports) => {
        await gpu.ready;
        const { instance } = await filesystem.instantiate(input, { ...imports, kui_gpu: gpu.imports });
        const exports = Object.fromEntries(Object.entries(instance.exports).map(([name, value]) => [name,
          typeof value === "function" && name !== "playground_reset_budget"
            ? (...args) => { instance.exports.playground_reset_budget(); return value(...args); }
            : value,
        ]));
        return { instance: { exports } };
      },
      present: gpu.present,
      panOutside: dy => notify("pan", { dy }),
      initialize(wasm) {
        const ptr = wasm.text_buffer(metadata.length);
        new Uint8Array(wasm.memory.buffer, ptr, metadata.length).set(metadata);
        const error = wasm.select_app();
        if (error) throw new Error(decoder.decode(new Uint8Array(wasm.memory.buffer, wasm.text_buffer(error), error)));
      },
      afterFrame(wasm, _canvas, schedule) {
        if (!wired) {
          wired = true;
          window.addEventListener("message", event => {
            if (event.origin !== location.origin || event.source !== parent || event.data?.type !== "kui-example" || event.data.state !== "devtools") return;
            wasm.app_set_devtools(Number(event.data.on === true));
            schedule();
          });
        }
        if (wasm.run_tasks()) schedule();
        const shown = wasm.app_devtools_height() > 0;
        if (shown !== dock) {
          dock = shown;
          notify("devtools", { on: shown });
        }
        const error = wasm.playground_error();
        if (error) failure(decoder.decode(new Uint8Array(wasm.memory.buffer, wasm.text_buffer(error), error)));
      },
    });
    if (!failed) notify("ready");
    if (gpu.active()) document.querySelector("#renderer").textContent = "kui-wgpu";
  } catch (error) { failure(error.message); }
});
if (parent !== window) notify("source");
else document.querySelector("#status").textContent = "Open the Lua playground to edit and run a view.";
