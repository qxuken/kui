import { ConsoleStdout, File, OpenFile, PreopenDirectory, WASI } from "./wasi/index.js";

export function luaHost() {
  const files = new Map();
  const key = "kui-book/tasks.json";
  const filename = "tasks.json";
  let saved;
  try { saved = localStorage.getItem(key); } catch {}
  if (typeof saved === "string") {
    files.set(filename, new File(new TextEncoder().encode(saved)));
  }

  return {
    async instantiate(bytes, imports) {
      const wasi = new WASI([], [], [
        new OpenFile(new File([])),
        ConsoleStdout.lineBuffered(line => console.log(line)),
        ConsoleStdout.lineBuffered(line => console.error(line)),
        new PreopenDirectory("/data", files),
      ]);
      const module = await WebAssembly.instantiate(bytes, {
        ...imports,
        wasi_snapshot_preview1: wasi.wasiImport,
      });
      wasi.initialize(module.instance);
      return module;
    },
    afterFrame(wasm, _canvas, schedule) {
      if (!wasm.demo_run_tasks()) return;
      schedule();
      const file = files.get(filename);
      if (!file) return;
      const text = new TextDecoder().decode(file.data);
      if (text !== saved) {
        saved = text;
        try { localStorage.setItem(key, text); } catch {}
      }
    },
  };
}
