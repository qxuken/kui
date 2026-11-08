import { ConsoleStdout, Directory, File, OpenFile, PreopenDirectory, WASI } from "./wasi/index.js";
import { ERRNO_ROFS, OFLAGS_CREAT, OFLAGS_TRUNC } from "./wasi/wasi_defs.js";

function filesystemTree(entries, readonly) {
  const root = new Map();
  for (const [path, data] of entries) {
    const parts = path.split("/");
    if (parts.some(part => !part || part === "." || part === ".." || part.includes("\\"))) {
      throw new Error(`Invalid app file path ${path}`);
    }
    const name = parts.pop();
    let dir = root;
    for (const part of parts) {
      if (!dir.has(part)) dir.set(part, new Map());
      dir = dir.get(part);
      if (!(dir instanceof Map)) throw new Error(`Invalid app directory ${path}`);
    }
    if (dir.has(name)) throw new Error(`Duplicate app file ${path}`);
    dir.set(name, data === null ? new Map() : new File(data, { readonly }));
  }
  const contents = map => new Map([...map].map(([name, entry]) => [
    name, entry instanceof Map ? new Directory(contents(entry)) : entry,
  ]));
  return contents(root);
}

function snapshot(directory) {
  const files = new Map();
  function visit(dir, prefix) {
    for (const [name, entry] of dir.contents) {
      const path = prefix + name;
      if (entry instanceof Directory) {
        files.set(path, null);
        visit(entry, path + "/");
      } else {
        files.set(path, entry.data.slice());
      }
    }
  }
  visit(directory, "");
  return files;
}

function openStorage(id) {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(`kui-app:${id}`, 1);
    request.onupgradeneeded = () => request.result.createObjectStore("files");
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

function transaction(db, mode, use) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction("files", mode);
    const result = use(tx.objectStore("files"));
    tx.oncomplete = () => resolve(result);
    tx.onabort = () => reject(tx.error ?? new Error("Storage transaction aborted"));
  });
}

// The book imports this once to carry its old task file into a packed app.
export async function seedAppFile(id, path, data) {
  const db = await openStorage(id);
  try {
    await transaction(db, "readwrite", store => {
      const request = store.getKey(path);
      request.onsuccess = () => {
        if (request.result === undefined) store.put(data, path);
      };
    });
  } finally {
    db.close();
  }
}

function sameFile(a, b) {
  return a === b || (a != null && b != null && a.length === b.length && a.every((byte, i) => byte === b[i]));
}

export async function appFilesystem(id, appFiles, reportError) {
  const db = await openStorage(id);
  const [keys, values] = await transaction(db, "readonly", store => [store.getAllKeys(), store.getAll()]);
  let saved = new Map(keys.result.map((key, i) => [key, values.result[i]]));
  const app = new PreopenDirectory("/app", filesystemTree(appFiles, true));
  const data = new PreopenDirectory("/data", filesystemTree(saved, false));
  const cache = new PreopenDirectory("/cache", new Map());
  const readonly = new WeakSet();
  function mark(dir) {
    readonly.add(dir);
    for (const entry of dir.contents.values()) {
      if (entry instanceof Directory) mark(entry);
      else readonly.add(entry);
    }
  }
  mark(app.dir);
  let dirty = false;
  let saving = false;

  function flush() {
    if (!dirty || saving) return;
    dirty = false;
    saving = true;
    const files = snapshot(data.dir);
    transaction(db, "readwrite", store => {
      for (const key of saved.keys()) {
        if (!files.has(key)) store.delete(key);
      }
      for (const [key, value] of files) {
        if (!sameFile(saved.get(key), value)) store.put(value, key);
      }
    }).then(() => { saved = files; }, reportError).finally(() => {
      saving = false;
      flush();
    });
  }

  return {
    flush,
    async instantiate(bytes, imports) {
      const wasi = new WASI([], [], [
        new OpenFile(new File([])),
        ConsoleStdout.lineBuffered(line => console.log(line)),
        ConsoleStdout.lineBuffered(line => console.error(line)),
        app, data, cache,
      ], { debug: false });
      const fs = { ...wasi.wasiImport };
      const isReadonly = fd => {
        const entry = wasi.fds[fd];
        return readonly.has(entry?.dir ?? entry?.file);
      };
      // The shim enforces readonly writes on File. Keep directory changes
      // and descriptor-based truncation read-only as well.
      for (const name of ["fd_allocate", "fd_filestat_set_size", "fd_filestat_set_times", "fd_write",
        "fd_pwrite", "path_create_directory", "path_filestat_set_times", "path_remove_directory",
        "path_unlink_file", "path_link", "path_rename", "path_symlink", "path_open"]) {
        const call = fs[name];
        if (!call) continue;
        fs[name] = (...args) => {
          const fd = name === "path_symlink" ? args[2] : args[0];
          const writes = name !== "path_open" || (args[4] & (OFLAGS_CREAT | OFLAGS_TRUNC));
          if (writes && (isReadonly(fd) || (name === "path_link" && isReadonly(args[4])) ||
              (name === "path_rename" && isReadonly(args[3])))) return ERRNO_ROFS;
          const result = call(...args);
          if (writes && fd > 2 && result === 0) dirty = true;
          return result;
        };
      }
      const module = await WebAssembly.instantiate(bytes, { ...imports, wasi_snapshot_preview1: fs });
      wasi.initialize(module.instance);
      return module;
    },
  };
}
