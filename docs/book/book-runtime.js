// One URL and cache identity for the book's packed apps and editable views.
let metadata;
let compiled;

export function runtimeInfo() {
  if (!metadata) metadata = (async () => {
    const response = await fetch(new URL("./manifest.json", import.meta.url), { cache: "no-cache" });
    if (!response.ok) throw new Error(`Runtime manifest could not load (${response.status})`);
    const info = await response.json();
    if (typeof info.version !== "string" || !/^[a-f0-9]{64}$/.test(info.cache)) throw new Error("Invalid runtime manifest");
    const url = new URL("./runner.wasm", import.meta.url);
    url.searchParams.set("v", info.cache);
    return { ...info, runner: info.version, url: url.href };
  })().catch(error => { metadata = undefined; throw error; });
  return metadata;
}

export function compiledRuntime() {
  if (!compiled) compiled = (async () => {
    const info = await runtimeInfo();
    const response = await fetch(info.url);
    if (!response.ok) throw new Error(`Lua runtime could not load (${response.status})`);
    const bytes = await response.arrayBuffer();
    return { ...info, bytes, module: await WebAssembly.compile(bytes) };
  })().catch(error => { compiled = metadata = undefined; throw error; });
  return compiled;
}
