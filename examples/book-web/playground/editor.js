import { compiledRuntime } from "../runtime/book-runtime.js";

const query = new URLSearchParams(location.search);
const embedded = query.has("embed") && parent !== window;
if (embedded) document.documentElement.dataset.embed = "";
const source = document.querySelector("#source");
const examples = document.querySelector("#example");
const preview = document.querySelector("#preview");
const status = document.querySelector("#status");
const error = document.querySelector("#error");
const runButton = document.querySelector("#run");
const presets = new Set([...examples.options].map(option => option.value));
const encoder = new TextEncoder();
const maximumBytes = 128 * 1024;
let active;
let pending;
let generation = 0;
let selection = 0;
let devtools = false;

function notifyBook(state, fields = {}) {
  if (embedded) parent.postMessage({ type: "kui-example", state, ...fields }, location.origin);
}
function reportSize() {
  notifyBook("size", { width: 1200, minWidth: 320, height: Math.min(2000, document.body.scrollHeight), devtools });
}
new ResizeObserver(reportSize).observe(document.body);

function fail(message, attempt = pending) {
  if (attempt && attempt !== pending && attempt !== active) return;
  if (attempt && attempt === pending) {
    pending.frame.remove();
    pending = undefined;
  }
  preview.setAttribute("aria-busy", "false");
  status.textContent = attempt && attempt === active ? "Preview error" : active ? "Keeping previous view" : "Could not run";
  error.textContent = String(message);
  error.hidden = false;
  if (!active) notifyBook("error", { message: String(message) });
}

function run() {
  pending?.frame.remove();
  pending = undefined;
  const code = source.value;
  if (encoder.encode(code).length > maximumBytes) {
    fail("Keep the source under 128 KiB.");
    return;
  }
  error.hidden = true;
  status.textContent = "Running…";
  preview.setAttribute("aria-busy", "true");
  const frame = document.createElement("iframe");
  frame.title = "Lua preview";
  frame.className = "pending";
  frame.inert = true;
  frame.setAttribute("aria-hidden", "true");
  const url = new URL("view.html", import.meta.url);
  url.searchParams.set("embed", "");
  if (query.get("renderer") === "canvas") url.searchParams.set("renderer", "canvas");
  frame.src = url;
  pending = { frame, code, id: ++generation };
  preview.append(frame);
}

window.addEventListener("message", async event => {
  if (event.origin !== location.origin) return;
  const attempt = [pending, active].find(item => item && event.source === item.frame.contentWindow);
  if (event.data?.type === "kui-playground" && attempt) {
    if (event.data.state === "source" && attempt === pending) {
      try {
        const shared = await compiledRuntime();
        if (attempt !== pending) return;
        const data = { type: "kui-playground", state: "run", code: attempt.code, runner: shared.runner, runtime: shared.url };
        try {
          event.source.postMessage({ ...data, module: shared.module }, location.origin);
        } catch (failure) {
          if (failure.name !== "DataCloneError") throw failure;
          // Older engines can still reuse the downloaded bytes when they
          // cannot clone a compiled Module between same-origin frames.
          event.source.postMessage({ ...data, bytes: shared.bytes }, location.origin);
        }
      } catch (failure) { fail(failure.message, attempt); }
    } else if (event.data.state === "ready" && attempt === pending) {
      active?.frame.remove();
      active = attempt;
      pending = undefined;
      active.frame.className = "";
      active.frame.inert = false;
      active.frame.removeAttribute("aria-hidden");
      active.frame.dataset.active = "";
      devtools = false;
      reportSize();
      preview.setAttribute("aria-busy", "false");
      error.hidden = true;
      status.textContent = source.value === attempt.code ? "Ready" : "Ready · newer edits";
      notifyBook("ready");
    } else if (event.data.state === "error") {
      fail(event.data.message, attempt);
    } else if (event.data.state === "pan") {
      if (embedded) notifyBook("pan", { dy: event.data.dy });
      else window.scrollBy(0, event.data.dy);
    } else if (event.data.state === "devtools" && attempt === active) {
      devtools = event.data.on === true;
      reportSize();
    }
  } else if (embedded && event.source === parent && event.data?.type === "kui-example" && event.data.state === "devtools") {
    active?.frame.contentWindow.postMessage(event.data, location.origin);
  }
});

function draftKey() { return `kui-book/playground/${examples.value}`; }
source.addEventListener("input", () => {
  try { localStorage.setItem(draftKey(), source.value); } catch { storageUnavailable(); }
  if (active) status.textContent = "Edited · Run to update";
});
function storageUnavailable() {
  document.querySelector(".hint").textContent = "Drafts cannot be saved here. Copy your source before closing this tab.";
}
document.addEventListener("keydown", event => {
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    if (!runButton.disabled) run();
  }
});
runButton.addEventListener("click", run);

async function loadExample(reset = false) {
  const revision = ++selection;
  pending?.frame.remove();
  pending = undefined;
  preview.setAttribute("aria-busy", "true");
  status.textContent = "Loading example…";
  runButton.disabled = true;
  source.readOnly = true;
  try {
    const response = await fetch(new URL(`examples/${examples.value}.lua`, import.meta.url), { cache: "no-cache" });
    if (!response.ok) throw new Error(`Example could not load (${response.status})`);
    let code = await response.text();
    if (revision !== selection) return;
    try {
      if (reset) localStorage.removeItem(draftKey());
      else code = localStorage.getItem(draftKey()) ?? code;
    } catch { storageUnavailable(); }
    source.value = code;
    runButton.disabled = false;
    run();
  } catch (failure) { if (revision === selection) fail(failure.message); }
  finally {
    if (revision === selection) {
      source.readOnly = false;
      runButton.disabled = false;
    }
  }
}
examples.value = presets.has(query.get("example")) ? query.get("example") : "counter";
examples.addEventListener("change", () => {
  const url = new URL(location.href);
  url.searchParams.set("example", examples.value);
  history.replaceState(null, "", url);
  loadExample();
});
document.querySelector("#reset").addEventListener("click", () => loadExample(true));
loadExample();
