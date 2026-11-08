import { migrateAppFile } from "./filesystem.js";

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
await import("./web-app.js");
