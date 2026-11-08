import { ConsoleStdout, File, OpenFile, PreopenDirectory, WASI } from "./wasi/index.js";

export function luaHost() {
  return {
    async instantiate(bytes, imports) {
      const wasi = new WASI([], [], [
        new OpenFile(new File([])),
        ConsoleStdout.lineBuffered(line => console.log(line)),
        ConsoleStdout.lineBuffered(line => console.error(line)),
        new PreopenDirectory("/data", new Map()),
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
    },
  };
}
