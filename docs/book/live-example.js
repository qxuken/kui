// The link also opens the demo directly when scripts are unavailable.
for (const link of document.querySelectorAll("a[data-kui-example]")) {
  const example = document.createElement("section");
  example.className = "kui-example";
  example.setAttribute("aria-label", link.textContent);
  link.replaceWith(example);

  const toolbar = document.createElement("div");
  toolbar.className = "kui-example-toolbar";
  const reload = document.createElement("button");
  reload.type = "button";
  reload.textContent = "Reload example";
  const status = document.createElement("span");
  status.className = "kui-example-status";
  status.setAttribute("role", "status");
  toolbar.append(link, reload, status);

  const viewport = document.createElement("div");
  viewport.className = "kui-example-viewport";
  viewport.style.height = `${link.dataset.kuiExample}px`;
  example.append(toolbar, viewport);

  const url = new URL(link.href);
  url.searchParams.set("embed", "");
  let frame;
  let timeout;
  function load() {
    clearTimeout(timeout);
    status.textContent = "Loading example…";
    viewport.setAttribute("aria-busy", "true");
    frame = document.createElement("iframe");
    frame.title = link.textContent;
    frame.src = url.href;
    viewport.replaceChildren(frame);
    timeout = setTimeout(() => {
      status.textContent = "The example is still loading. You can reload it.";
    }, 15_000);
  }
  window.addEventListener("message", event => {
    if (event.origin !== url.origin || event.source !== frame?.contentWindow ||
        event.data?.type !== "kui-example") return;
    if (event.data.state === "pan" && Number.isFinite(event.data.dy)) {
      window.scrollBy(0, event.data.dy);
    }
    if (event.data.state === "size") {
      const { width, height } = event.data;
      if (Number.isFinite(width) && width >= 1 && width <= 2000 &&
          Number.isFinite(height) && height >= 1 && height <= 1500) {
        example.style.setProperty("--kui-example-width", `${width + 2}px`);
        viewport.style.aspectRatio = `${width} / ${height}`;
        viewport.style.height = "auto";
        viewport.style.maxHeight = `${height}px`;
      }
    }
    if (event.data.state === "ready" || event.data.state === "error") {
      clearTimeout(timeout);
      viewport.removeAttribute("aria-busy");
      status.textContent = event.data.state === "ready" ? "Ready"
        : event.data.message ? `Could not load the example. ${event.data.message}`
        : "The example could not load.";
    }
  });
  reload.addEventListener("click", load);
  load();
}
