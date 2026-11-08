import { startCanvas } from "./canvas.js";

export async function mountDemo(wasmUrl, { initialize, afterFrame, contentScale, instantiate, panOutside, renderer = "auto" } = {}) {
  const params = new URLSearchParams(location.search);
  const embed = params.has("embed");
  if (embed) document.documentElement.dataset.embed = "";

  const status = document.querySelector("#status");
  const notify = (state, message) => {
    if (embed && window.parent !== window) {
      window.parent.postMessage({ type: "kui-example", state, message }, location.origin);
    }
  };
  try {
    const selected = await startCanvas(document.querySelector("canvas"), wasmUrl, {
      access: document.querySelector("#access"),
      initialize,
      afterFrame,
      contentScale,
      instantiate,
      panOutside: embed ? panOutside : undefined,
      renderer: instantiate ? "canvas" : params.get("renderer") ?? renderer,
    });
    document.querySelector("#renderer").textContent = selected === "wgpu" ? "kui-wgpu" : "Canvas 2D";
    status.hidden = true;
    notify("ready");
  } catch (error) {
    status.textContent = error.message;
    notify("error", error.message);
    throw error;
  }
}
