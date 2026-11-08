import { audioEffects } from "./gallery-audio.js";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function send(wasm, text) {
  const bytes = encoder.encode(text);
  const ptr = wasm.text_buffer(bytes.length);
  new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
}

function download(blob, name) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

const filename = path => path.split(/[\\/]/).pop();

const keyNames = { Control: "ctrl", Meta: "super", " ": "space" };
const punctuation = {
  Backquote: "`", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]",
  Backslash: "\\", Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/",
};

function physicalKey(code) {
  if (/^Key[A-Z]$/.test(code)) return code.at(-1).toLowerCase();
  if (/^Digit[0-9]$/.test(code)) return code.at(-1);
  return punctuation[code] ?? "";
}

function wireInput(wasm, canvas, schedule, contentScale) {
  const active = new Set();
  for (const type of ["keydown", "keyup"]) {
    document.addEventListener(type, event => {
      const down = type === "keydown";
      const id = event.code || event.key;
      if (down) {
        if (event.defaultPrevented || event.isComposing || event.target.closest?.("textarea,input")) return;
        if (event.key === "Tab" || (event.target.closest?.("button") && [" ", "Enter"].includes(event.key))) return;
      } else if (!active.delete(id)) {
        return;
      }
      const key = keyNames[event.key] ?? (event.key.length === 1
        ? event.key : event.key.replace(/^Arrow/, "").toLowerCase());
      const mods = Number(event.shiftKey) | (Number(event.ctrlKey) << 1) | (Number(event.altKey) << 2) | (Number(event.metaKey) << 3);
      send(wasm, `${key}\0${physicalKey(event.code)}`);
      if (!wasm.demo_key(mods, Number(down), performance.now() / 1000)) return;
      if (down) active.add(id);
      event.preventDefault();
      schedule();
    });
  }
  window.addEventListener("blur", () => active.clear());
  for (const [type, kind] of [["dragover", 1], ["drop", 2], ["dragleave", 3]]) {
    canvas.addEventListener(type, event => {
      if (!event.dataTransfer.types.includes("Files")) return;
      event.preventDefault();
      const rect = canvas.getBoundingClientRect();
      const names = Array.from(event.dataTransfer.files, file => file.name);
      send(wasm, JSON.stringify(names.length ? names : ["Files"]));
      wasm.demo_files(kind, (event.clientX - rect.x) / contentScale(), (event.clientY - rect.y) / contentScale(), performance.now() / 1000);
      schedule();
    });
  }
}

export function effects({ audioEnabled, contentScale }) {
  const audio = audioEffects();
  let recording;
  let wired = false;
  return (wasm, canvas, schedule) => {
    if (!wired) {
      wireInput(wasm, canvas, schedule, contentScale);
      if (audioEnabled) {
        for (const type of ["pointerdown", "pointerup", "keydown"]) {
          document.addEventListener(type, audio.unlock, { capture: true });
        }
      }
      wired = true;
    }
    const len = wasm.take_demo_effects();
    if (!len) return;
    const ptr = wasm.text_buffer(len);
    const requests = JSON.parse(decoder.decode(new Uint8Array(wasm.memory.buffer, ptr, len)));
    const complete = (req, ok, text, frames = 1, seconds = 0) => {
      send(wasm, text);
      wasm.demo_capture_result(req.id, Number(ok), canvas.width, canvas.height, frames, seconds);
      schedule();
    };
    for (const req of requests) {
      switch (req.kind) {
        case "copy":
          navigator.clipboard?.writeText(req.text).catch(() => {});
          break;
        case "paste":
          navigator.clipboard?.readText().then(text => {
            send(wasm, text);
            wasm.text_input(3, performance.now() / 1000);
            schedule();
          }).catch(() => {});
          break;
        case "files": {
          const done = names => {
            send(wasm, JSON.stringify(names));
            wasm.demo_files(0, 0, 0, performance.now() / 1000);
            schedule();
          };
          if (req.mode === "save") {
            done([]);
            break;
          }
          const picker = document.createElement("input");
          picker.type = "file";
          picker.accept = req.accept;
          picker.multiple = req.multiple;
          if (req.mode === "folder") picker.setAttribute("webkitdirectory", "");
          picker.addEventListener("change", () => done(Array.from(picker.files, file => file.name)), { once: true });
          picker.addEventListener("cancel", () => done([]), { once: true });
          picker.click();
          break;
        }
        case "still":
        case "copy-image":
          canvas.toBlob(async blob => {
            try {
              if (!blob) throw new Error("The browser could not capture the canvas.");
              const name = filename(req.path);
              if (req.kind === "still") download(blob, name);
              else await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
              complete(req, true, name);
            } catch (error) {
              complete(req, false, error.message);
            }
          });
          break;
        case "record":
          try {
            if (recording) throw new Error("A recording is already running.");
            const stream = canvas.captureStream(25);
            const recorder = new MediaRecorder(stream);
            const chunks = [];
            const start = performance.now();
            recorder.addEventListener("dataavailable", event => chunks.push(event.data));
            recorder.addEventListener("stop", () => {
              stream.getTracks().forEach(track => track.stop());
              recording = undefined;
              const seconds = (performance.now() - start) / 1000;
              const format = recorder.mimeType.includes("mp4") ? "mp4" : "webm";
              const name = filename(req.path).replace(/\.[^.]+$/, `.${format}`);
              download(new Blob(chunks, { type: recorder.mimeType }), name);
              complete(req, true, name, Math.max(1, Math.round(seconds * 25)), seconds);
            });
            recorder.start();
            recording = recorder;
          } catch (error) {
            complete(req, false, error.message);
          }
          break;
        case "stop-recording":
          recording?.stop();
          break;
        default:
          audio.handle(req, refused => {
            wasm.demo_audio_ended(req.id, Number(refused));
            schedule();
          });
      }
    }
  };
}
