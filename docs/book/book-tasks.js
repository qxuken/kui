import { migrateAppFile } from "./filesystem.js";
import { runtimeInfo } from "./book-runtime.js";

async function migrateTasks() {
  const key = "kui-book/tasks.json";
  const legacy = localStorage.getItem(key);
  if (legacy === null) return;

  function parseTasks(json) {
    const tasks = JSON.parse(json);
    if (!Array.isArray(tasks) || tasks.some(task =>
      typeof task?.text !== "string" || typeof task.done !== "boolean")) {
      throw new Error("Expected a list of saved tasks");
    }
    return tasks;
  }

  const oldTasks = parseTasks(legacy);
  const encoder = new TextEncoder();
  await migrateAppFile("dev.kui.book.tasks", "tasks.json", ".kui-book-tasks-migration", encoder.encode(legacy), current => {
    const currentTasks = current ? parseTasks(new TextDecoder().decode(current)) : [];
    return encoder.encode(JSON.stringify([...oldTasks, ...currentTasks]));
  });
  if (localStorage.getItem(key) === legacy) localStorage.removeItem(key);
}

try {
  await migrateTasks();
} catch (error) {
  console.warn("Could not migrate saved book tasks", error);
}
try {
  const { cache } = await runtimeInfo();
  const entry = new URL("./web-app.js", import.meta.url);
  entry.searchParams.set("v", cache);
  await import(entry.href);
} catch (error) {
  const status = document.querySelector("#status");
  status.textContent = `Could not load application. ${error.message}`;
  status.hidden = false;
  if (parent !== window) parent.postMessage({ type: "kui-example", state: "error", message: error.message }, location.origin);
  throw error;
}
