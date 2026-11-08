import { startCanvas } from "./canvas.js";

export async function mountDemo(wasmUrl) {
  const params = new URLSearchParams(location.search);
  const embed = params.has("embed");
  if (embed) document.documentElement.dataset.embed = "";

  const status = document.querySelector("#status");
  const notify = state => {
    if (embed && window.parent !== window) {
      window.parent.postMessage({ type: "kui-example", state }, location.origin);
    }
  };
  try {
    const selected = await startCanvas(document.querySelector("canvas"), wasmUrl, {
      access: document.querySelector("#access"),
      renderer: params.get("renderer") ?? "auto",
    });
    document.querySelector("#renderer").textContent = selected === "wgpu" ? "kui-wgpu" : "Canvas 2D";
    status.hidden = true;
    notify("ready");
  } catch (error) {
    status.textContent = error.message;
    notify("error");
    throw error;
  }
}
