// The DOM receives browser editing gestures. The stock kui editor owns the
// value, selection and undo history, and paints all of the visible text.
const edits = {
  ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down",
  Home: "home", End: "end", PageUp: "pageup", PageDown: "pagedown",
  Backspace: "backspace", Delete: "delete", Enter: "enter",
};
const inputEdits = {
  deleteContentBackward: "backspace", deleteContentForward: "delete",
  deleteWordBackward: "backspace", deleteWordForward: "delete",
  insertLineBreak: "enter", insertParagraph: "enter", historyUndo: "undo", historyRedo: "redo",
};
const textMode = { insert: 0, preedit: 1, commit: 2, paste: 3, replace: 4 };
const editor = (el) => el instanceof HTMLTextAreaElement || el instanceof HTMLInputElement;
const tags = { button: "button", checkbox: "button", textInput: "input", multilineTextInput: "textarea" };
const roles = { checkbox: "checkbox", list: "list", listItem: "listitem", heading: "heading", group: "group" };

export function editorBridge(root, canvas, wasm, schedule, seconds, clipboard) {
  const elements = new Map();
  const editorStates = new WeakMap();
  const encoder = new TextEncoder();
  const mac = /Mac|iPhone|iPad/.test(navigator.platform);
  let applying = false;
  let composing = null;
  let completion = null;

  function send(value, method, kind) {
    const bytes = encoder.encode(value);
    const ptr = wasm.text_buffer(bytes.length);
    new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
    wasm[method](kind, seconds());
    schedule();
  }
  const text = (value, kind) => send(value, "text_input", kind);
  const edit = (key, mods = 0) => send(key, "edit_key", mods);
  const keyOf = (target) => target instanceof HTMLElement && target.dataset.kuiKey;

  function create(node) {
    const el = document.createElement(tags[node.role] ?? "div");
    el.dataset.kuiKey = node.key;
    if (el instanceof HTMLButtonElement) el.type = "button";
    if (editor(el)) {
      el.autocomplete = "off";
      el.spellcheck = false;
      el.setAttribute("autocapitalize", "off");
    }
    return el;
  }

  function position(el, rect) {
    const [x, y, w, h] = rect;
    Object.assign(el.style, {
      left: `${x}px`, top: `${y}px`, width: `${Math.max(w, 2)}px`, height: `${Math.max(h, 2)}px`,
    });
  }

  function attribute(el, name, value) {
    if (value == null || value === "") el.removeAttribute(name);
    else if (el.getAttribute(name) !== value) el.setAttribute(name, value);
  }

  function sync(snapshot) {
    completion = null;
    if (!snapshot) return;
    applying = true;
    try {
      const seen = new Set();
      const nextChild = new Map();
      for (const node of snapshot.nodes) {
        let el = elements.get(node.key);
        if (!el) {
          el = create(node);
          elements.set(node.key, el);
        }
        seen.add(node.key);
        const parent = elements.get(node.parent) ?? root;
        const before = nextChild.has(parent) ? nextChild.get(parent) : parent.firstChild;
        if (el !== before) parent.insertBefore(el, before);
        nextChild.set(parent, el.nextSibling);
        attribute(el, "role", roles[node.role]);
        attribute(el, "aria-level", node.role === "heading" ? "2" : null);
        attribute(el, "aria-description", node.description);
        if (node.role === "checkbox") {
          attribute(el, "aria-checked", node.mixed ? "mixed" : String(node.checked ?? false));
        }
        attribute(el, "aria-current", node.selected ? "true" : null);
        attribute(el, "aria-live", node.live === "off" ? null : node.live);
        if (el instanceof HTMLButtonElement || editor(el)) {
          el.disabled = node.disabled;
          el.tabIndex = node.focusable ? 0 : -1;
          attribute(el, "aria-label", node.name);
          position(el, node.key === snapshot.focus && editor(el) && snapshot.caret
            ? snapshot.caret : node.rect);
        } else if (node.role === "staticText" || node.role === "heading" || node.live !== "off") {
          if (el.textContent !== (node.name ?? "")) el.textContent = node.name ?? "";
        } else {
          attribute(el, "aria-label", node.name);
        }
        if (editor(el) && el !== composing) {
          const value = node.value ?? "";
          if (el.value !== value) el.value = value;
          if (node.selection) {
            const [start, end, direction] = node.selection;
            if (el.selectionStart !== start || el.selectionEnd !== end
                || (start !== end && el.selectionDirection !== direction)) {
              el.setSelectionRange(start, end, direction);
            }
            editorStates.set(el, { value, start, end, direction: el.selectionDirection });
          }
        }
      }
      for (const [key, el] of elements) {
        if (seen.has(key)) continue;
        if (composing === el) composing = null;
        el.remove();
        elements.delete(key);
      }
      const focused = elements.get(snapshot.focus);
      if (focused && focused.tabIndex >= 0 && document.activeElement !== focused) {
        focused.focus({ preventScroll: true });
      } else if (!focused && root.contains(document.activeElement)) {
        canvas.focus({ preventScroll: true });
      }
    } finally {
      applying = false;
    }
  }

  root.addEventListener("focusin", (event) => {
    const key = keyOf(event.target);
    if (applying || !key) return;
    wasm.focus_node(BigInt(key), seconds());
    schedule();
  });
  root.addEventListener("click", (event) => {
    if (!(event.target instanceof HTMLButtonElement)) return;
    wasm.activate_node(BigInt(keyOf(event.target)), seconds());
    schedule();
  });
  root.addEventListener("keydown", (event) => {
    if (event.isComposing || composing) return;
    completion = null;
    if (event.key === "Escape") {
      event.preventDefault();
      wasm.escape(seconds());
      schedule();
      return;
    }
    if (!editor(event.target)) {
      const list = event.target.closest('[role="list"]');
      if (list && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
        const items = [...list.querySelectorAll("button:not(:disabled)")];
        const next = items[items.indexOf(event.target) + (event.key === "ArrowDown" ? 1 : -1)];
        event.preventDefault();
        next?.focus({ preventScroll: true });
      }
      return;
    }
    const primary = mac ? event.metaKey : event.ctrlKey && !event.altKey;
    const key = event.key.toLowerCase();
    let action = edits[event.key];
    if (primary && key === "a") action = "selectall";
    if (primary && key === "z") action = event.shiftKey ? "redo" : "undo";
    if (primary && key === "y") action = "redo";
    if (!action) return;
    event.preventDefault();
    edit(action, Number(event.shiftKey) | (Number(event.altKey) << 1)
      | (Number(primary) << 2) | (Number(event.ctrlKey) << 3));
  });

  function compositionTail(event) {
    return completion && completion.target === event.target
      && (event.inputType === "insertFromComposition"
        || (event.inputType === "insertText" && event.data === completion.data));
  }
  root.addEventListener("beforeinput", (event) => {
    if (!editor(event.target) || composing || event.isComposing || !event.cancelable) return;
    if (compositionTail(event)) {
      event.preventDefault();
    } else if (event.inputType === "insertText" && event.data != null) {
      event.preventDefault();
      text(event.data, textMode.insert);
    } else if (inputEdits[event.inputType]) {
      event.preventDefault();
      edit(inputEdits[event.inputType], event.inputType.startsWith("deleteWord") ? 2 : 0);
    }
  });
  root.addEventListener("input", (event) => {
    if (!editor(event.target) || composing || event.isComposing || compositionTail(event)) return;
    // Spell correction and some platform input methods can mutate the
    // textarea without a cancellable beforeinput.
    text(event.target.value, textMode.replace);
  });
  root.addEventListener("select", (event) => {
    const el = event.target;
    const state = editorStates.get(el);
    if (applying || composing || !state || el !== document.activeElement || el.value !== state.value) return;
    const start = el.selectionStart, end = el.selectionEnd, direction = el.selectionDirection;
    if (start === state.start && end === state.end && direction === state.direction) return;
    const backward = direction === "backward";
    wasm.select_text(backward ? end : start, backward ? start : end, seconds());
    schedule();
  });
  root.addEventListener("compositionstart", (event) => {
    if (editor(event.target)) composing = event.target;
  });
  root.addEventListener("compositionupdate", (event) => {
    if (composing === event.target) text(event.data, textMode.preedit);
  });
  root.addEventListener("compositionend", (event) => {
    if (composing !== event.target) return;
    composing = null;
    completion = { target: event.target, data: event.data };
    text(event.data, event.data ? textMode.commit : textMode.preedit);
  });
  root.addEventListener("paste", (event) => {
    if (!editor(event.target) || !event.clipboardData) return;
    event.preventDefault();
    text(event.clipboardData.getData("text/plain"), textMode.paste);
  });
  for (const kind of ["copy", "cut"]) {
    root.addEventListener(kind, (event) => {
      if (!editor(event.target) || !event.clipboardData) return;
      event.preventDefault();
      const value = clipboard(kind === "cut");
      if (value !== undefined) event.clipboardData.setData("text/plain", value);
      schedule();
    });
  }

  return {
    sync,
    blur() {
      if (applying) return;
      if (composing) {
        composing = null;
        text("", textMode.preedit);
      }
      wasm.blur(seconds());
      schedule();
    },
  };
}
