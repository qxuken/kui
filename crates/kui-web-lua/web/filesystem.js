import { ConsoleStdout, Directory, File, OpenFile, PreopenDirectory, WASI } from "./wasi/index.js";
import { ERRNO_ROFS, OFLAGS_CREAT, OFLAGS_TRUNC } from "./wasi/wasi_defs.js";

function filesystemTree(entries, readonly) {
  const root = new Map();
  for (const [path, data] of entries) {
    const parts = path.split("/");
    if (parts.some(part => !part || part === "." || part === ".." || (readonly && part.includes("\\")))) {
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

function storedFiles(db, use, paths) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction("files", "readwrite");
    const store = tx.objectStore("files");
    const keys = store.getAllKeys();
    const values = paths ? [...paths].map(path => [path, store.get(path)]) : store.getAll();
    const last = paths ? values.at(-1)?.[1] ?? keys : values;
    let result;
    let failure;
    last.onsuccess = () => {
      try {
        // A save checks all names for descendants, but only reads the contents
        // of changed paths and their parents.
        const files = new Map(keys.result.map((key, i) => [key, paths ? undefined : values.result[i]]));
        if (paths) {
          for (const [path, request] of values) {
            if (files.has(path)) files.set(path, request.result);
          }
        }
        result = use(store, files);
      } catch (error) {
        failure = error;
        tx.abort();
      }
    };
    tx.oncomplete = () => resolve(result);
    tx.onabort = () => reject(failure ?? tx.error ?? new Error("Storage transaction aborted"));
  });
}

export async function migrateAppFile(id, path, marker, source, merge) {
  const sourceId = new Uint8Array(await crypto.subtle.digest("SHA-256", source));
  const db = await openStorage(id);
  try {
    const paths = new Set([path, marker, ...parents(path), ...parents(marker)]);
    await storedFiles(db, (store, files) => {
      if (sameFile(files.get(marker), sourceId)) return;
      const directories = parentPaths(files.keys());
      if ([path, marker].some(key => files.get(key) === null || directories.has(key) ||
          [...parents(key)].some(parent => files.get(parent) != null))) {
        throw new Error("Migration needs a file, but found a directory");
      }
      const existing = files.get(path);
      if (!sameFile(existing, source)) store.put(merge(existing, source), path);
      // Record which source was merged without retaining a second copy of it.
      store.put(sourceId, marker);
    }, paths);
  } finally {
    db.close();
  }
}

function sameFile(a, b) {
  return a === b || (a != null && b != null && a.length === b.length && a.every((byte, i) => byte === b[i]));
}

function* parents(path) {
  for (let end = path.length; end > 0;) {
    const slash = path.lastIndexOf("/", end - 1);
    if (slash < 0) return;
    yield path.slice(0, slash);
    end = slash;
  }
}

function parentPaths(paths) {
  const directories = new Set();
  for (const path of paths) {
    for (const parent of parents(path)) directories.add(parent);
  }
  return directories;
}

function recoverFiles(store, files) {
  const directories = parentPaths(files.keys());
  const recovered = [];
  for (const path of directories) {
    const data = files.get(path);
    if (data == null) continue;
    // Older runners could put a file and its descendants in the same store.
    // Keep the subtree and move the file beside it under an unused name.
    let name = `${path}.kui-recovered`;
    for (let i = 2; files.has(name) || directories.has(name); i++) name = `${path}.kui-recovered-${i}`;
    files.set(path, null);
    files.set(name, data);
    store.put(null, path);
    store.put(data, name);
    recovered.push(`${path} → ${name}`);
  }
  return { files, recovered };
}

function changedFiles(saved, files) {
  const changes = new Map();
  for (const key of saved.keys()) {
    if (!files.has(key)) changes.set(key, undefined);
  }
  for (const [key, value] of files) {
    if (!sameFile(saved.get(key), value)) changes.set(key, value);
  }
  return changes;
}

function saveFiles(store, current, saved, changes, directories) {
  const conflicts = new Set();
  const next = new Map(current);
  for (const [key, value] of changes) {
    const remote = current.get(key);
    if (!sameFile(saved.get(key), remote) && !sameFile(value, remote)) conflicts.add(key);
    if (value === undefined) next.delete(key);
    else next.set(key, value);
  }

  for (const parent of parentPaths(next.keys())) {
    if ((directories.has(parent) || changes.has(parent)) &&
        (next.get(parent) != null || ((saved.has(parent) || changes.has(parent)) && !next.has(parent)))) {
      conflicts.add(parent);
    }
  }
  // A save can contain both sides of a rename. Persist all edits in the
  // snapshot together so a conflict cannot commit only the deletion.
  if (conflicts.size) return conflicts;

  for (const [key, value] of changes) {
    if (sameFile(current.get(key), value)) continue;
    if (value === undefined) store.delete(key);
    else store.put(value, key);
  }
  return conflicts;
}

export async function appFilesystem(id, appFiles, reportError, { persistent = true, stderr = line => console.error(line) } = {}) {
  const app = new PreopenDirectory("/app", filesystemTree(appFiles, true));
  const db = persistent ? await openStorage(id) : undefined;
  let saved;
  let data;
  try {
    const { files, recovered } = db ? await storedFiles(db, recoverFiles) : { files: new Map(), recovered: [] };
    if (recovered.length) console.warn(`Recovered conflicting application files: ${recovered.join(", ")}`);
    saved = files;
    data = new PreopenDirectory("/data", filesystemTree(saved, false));
  } catch (error) {
    db?.close();
    throw error;
  }
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
    if (!db) return;
    if (!dirty || saving) return;
    dirty = false;
    const files = snapshot(data.dir);
    const changes = changedFiles(saved, files);
    if (!changes.size) return;
    const directories = parentPaths(changes.keys());
    const paths = new Set([...directories, ...changes.keys()]);
    saving = true;
    storedFiles(db, (store, current) => saveFiles(store, current, saved, changes, directories), paths).then(conflicts => {
      // The next diff still uses the local tree, including edits another tab
      // prevented from being saved. Otherwise it could remove unseen files.
      saved = files;
      if (conflicts.size) {
        reportError(new Error(`Another tab changed ${[...conflicts].join(", ")}. This save was skipped. Reload to get the latest files.`));
      }
    }, reportError).finally(() => {
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
        ConsoleStdout.lineBuffered(stderr),
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
          if (writes && fd > 2 && result === 0 && !dirty) {
            dirty = true;
            // Coalesce writes in this turn and start saving before the next frame.
            queueMicrotask(flush);
          }
          return result;
        };
      }
      const loaded = await WebAssembly.instantiate(bytes, { ...imports, wasi_snapshot_preview1: fs });
      const instance = loaded.instance ?? loaded;
      wasi.initialize(instance);
      return { instance, module: loaded.module ?? bytes };
    },
  };
}
