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

function wireInput(wasm, canvas, schedule) {
  const active = new Set();
  const keys = { Control: "ctrl", Meta: "super", " ": "space" };
  for (const type of ["keydown", "keyup"]) {
    document.addEventListener(type, event => {
      if (event.defaultPrevented || event.isComposing || event.target.closest?.("textarea,input")) return;
      if (event.key === "Tab" || (event.target.closest?.("button") && [" ", "Enter"].includes(event.key))) return;
      const down = type === "keydown";
      if (!down && !active.delete(event.code)) return;
      const key = keys[event.key] ?? (event.key.length === 1 ? event.key : event.key.replace(/^Arrow/, "").toLowerCase());
      const mods = Number(event.shiftKey) | (Number(event.ctrlKey) << 1) | (Number(event.altKey) << 2) | (Number(event.metaKey) << 3);
      send(wasm, key);
      if (!wasm.demo_key(mods, Number(down), performance.now() / 1000)) return;
      if (down) active.add(event.code);
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
      wasm.demo_files(kind, event.clientX - rect.x, event.clientY - rect.y, performance.now() / 1000);
      schedule();
    });
  }
}

export function effects() {
  const sounds = new Map();
  const playing = new Map();
  let master = 1;
  let recording;
  let wired = false;
  return (wasm, canvas, schedule) => {
    if (!wired) {
      wireInput(wasm, canvas, schedule);
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
      const current = playing.get(req.id);
      const ended = refused => {
        if (!playing.delete(req.id) && !refused) return;
        wasm.demo_audio_ended(req.id, Number(refused));
        schedule();
      };
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
        case "play": {
          if (!req.bytes) { ended(true); break; }
          let url = sounds.get(req.sound);
          if (!url) {
            url = URL.createObjectURL(new Blob([Uint8Array.from(req.bytes)], { type: "audio/wav" }));
            sounds.set(req.sound, url);
          }
          const audio = new Audio(url);
          audio.loop = req.loop;
          audio.volume = Math.min(1, req.volume * master);
          playing.set(req.id, { audio, volume: req.volume });
          audio.addEventListener("ended", () => ended(false), { once: true });
          audio.play().catch(() => { if (playing.has(req.id)) ended(true); });
          break;
        }
        case "stop":
          current?.audio.pause();
          playing.delete(req.id);
          break;
        case "pause":
          current?.audio.pause();
          break;
        case "resume":
          current?.audio.play().catch(() => ended(true));
          break;
        case "volume":
          if (current) {
            current.volume = req.volume;
            current.audio.volume = Math.min(1, current.volume * master);
          }
          break;
        case "master":
          master = req.volume;
          for (const { audio, volume } of playing.values()) audio.volume = Math.min(1, volume * master);
          break;
        case "unload": {
          const url = sounds.get(req.sound);
          if (url) URL.revokeObjectURL(url);
          sounds.delete(req.sound);
          break;
        }
      }
    }
  };
}
