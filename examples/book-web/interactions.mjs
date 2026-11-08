// Run against a served release book, like smoke.mjs.
import assert from "node:assert/strict";

const { chromium, webkit } = await import(process.env.KUI_PLAYWRIGHT ?? "playwright");
const base = new URL(process.env.KUI_BOOK_URL ?? "http://127.0.0.1:8080/");
const useWebKit = process.env.KUI_WEBKIT === "1";
const browser = useWebKit ? await webkit.launch() : await chromium.launch(
  process.env.KUI_CHROME_CHANNEL ? { channel: process.env.KUI_CHROME_CHANNEL } : {},
);

try {
  const page = await browser.newPage({ viewport: { width: 1180, height: 850 } });
  page.setDefaultTimeout(8000);
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));

  async function chapter(path) {
    await page.goto(new URL(path, base).href);
    await page.getByText("Ready", { exact: true }).waitFor();
    return page.frameLocator("iframe");
  }

  async function click(locator) {
    const rect = await locator.boundingBox();
    assert.ok(rect);
    await page.mouse.click(rect.x + rect.width / 2, rect.y + rect.height / 2);
  }

  const mux = await chapter("examples/apps/splitmux.html");
  const muxCanvas = mux.locator("canvas");
  await muxCanvas.click({ position: { x: 200, y: 200 } });
  const option = async (key, code) => {
    const event = { key, code, altKey: true, bubbles: true, cancelable: true };
    await muxCanvas.dispatchEvent("keydown", event);
    await muxCanvas.dispatchEvent("keyup", event);
  };
  await option("√", "KeyV");
  await mux.locator('[aria-label="1  4 pane(s)"]').waitFor({ state: "attached" });
  await option("†", "KeyT");
  await mux.locator('[aria-label="2  1 pane(s)"]').waitFor({ state: "attached" });
  await option("¡", "Digit1");
  await mux.locator('[aria-label^="1 focused"]').waitFor({ state: "attached" });
  console.log("Option chords use the key position when macOS supplies a symbol");

  const devtools = page.getByRole("button", { name: "Devtools", exact: true });
  const iframe = page.locator("iframe");
  const originalHeight = (await iframe.boundingBox()).height;
  await devtools.click();
  await page.getByRole("button", { name: "Devtools", pressed: true }).waitFor();
  await mux.locator('[aria-label="Facts"]').waitFor({ state: "attached" });
  assert.ok((await iframe.boundingBox()).height > originalHeight);
  await mux.locator('[aria-label="1  4 pane(s)"]').waitFor({ state: "attached" });
  await devtools.click();
  await page.getByRole("button", { name: "Devtools", pressed: false }).waitFor();
  await mux.locator('[aria-label="Facts"]').waitFor({ state: "detached" });
  assert.equal((await iframe.boundingBox()).height, originalHeight);
  console.log("Devtools extends the existing demo and preserves its state");

  await page.setViewportSize({ width: 390, height: 844 });
  const loaders = await chapter("examples/apps/loaders.html");
  const startButton = await loaders.getByRole("button", { name: "start", exact: true }).boundingBox();
  assert.ok(startButton.height >= 30, JSON.stringify(startButton));
  assert.equal(await loaders.locator("main").evaluate(el => el.style.zoom), "1");
  console.log("Loader controls remain full-size in a narrow embed");

  const source = page.locator("main > pre > code").first();
  assert.ok(await source.evaluate(code => code.scrollWidth <= code.clientWidth),
    "the source listing wraps to the narrow chapter");
  await page.setViewportSize({ width: 1120, height: 950 });
  await page.evaluate(() => localStorage.setItem("mdbook-sidebar", "hidden"));
  await chapter("examples/features/align.html");
  assert.ok(await page.locator(".kui-example").evaluate(panel => {
    const box = panel.getBoundingClientRect();
    return [...document.querySelectorAll(".nav-wide-wrapper .nav-chapters")].every(link => {
      const rail = link.getBoundingClientRect();
      return rail.width === 0 || rail.right <= box.left || rail.left >= box.right;
    });
  }), "the fixed chapter arrows clear the wide demo");
  const footer = page.locator(".nav-wrapper");
  await footer.getByRole("link", { name: "Previous chapter" }).waitFor();
  await footer.getByRole("link", { name: "Next chapter" }).waitFor();
  await page.setViewportSize({ width: 1180, height: 850 });
  console.log("Wide demos clear chapter navigation and narrow source listings wrap");

  await page.setViewportSize({ width: 300, height: 844 });
  const popup = await chapter("examples/features/popup.html");
  const owner = popup.locator("main > canvas");
  const field = popup.getByRole("button", { name: "Alloy", exact: true });
  const overlay = popup.locator(".popup-overlay");
  const row = name => overlay.getByRole("button", { name, exact: true });
  const rowColor = locator => locator.evaluate(el => {
    const canvas = el.closest(".popup-overlay").querySelector("canvas");
    const bounds = canvas.getBoundingClientRect();
    const row = el.getBoundingClientRect();
    const x = (row.left - bounds.left + 6) * canvas.width / bounds.width;
    const y = (row.top + row.height / 2 - bounds.top) * canvas.height / bounds.height;
    return Array.from(canvas.getContext("2d").getImageData(Math.floor(x), Math.floor(y), 1, 1).data);
  });
  await click(field);
  await row("Aluminium").waitFor({ state: "attached" });
  const selectedColor = await rowColor(row("Aluminium"));
  assert.notDeepEqual(await rowColor(row("Beryllium")), selectedColor);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await overlay.waitFor({ state: "hidden" });
  await click(field);
  await row("Beryllium").waitFor({ state: "attached" });
  assert.deepEqual(await rowColor(row("Beryllium")), selectedColor);
  await page.keyboard.press("Escape");
  await overlay.waitFor({ state: "hidden" });

  await page.mouse.move(0, 0);
  await chapter("examples/features/popup.html");
  const anchor = await paintedPopupField(owner);
  await page.mouse.move(anchor.x, anchor.y);
  await page.mouse.down();
  await row("Osmium").waitFor({ state: "attached" });
  const choice = await row("Osmium").boundingBox();
  const ownerBounds = await owner.boundingBox();
  assert.ok(choice.y > ownerBounds.y + ownerBounds.height, "the menu reaches below the owner");
  await page.mouse.move(choice.x + choice.width / 2, choice.y + choice.height / 2, { steps: 6 });
  await page.mouse.up();
  await overlay.waitFor({ state: "hidden" });
  await click(field);
  await row("Osmium").waitFor({ state: "attached" });
  assert.deepEqual(await rowColor(row("Osmium")), selectedColor);
  await click(field);
  await overlay.waitFor({ state: "hidden" });
  await click(field);
  await overlay.waitFor();
  await page.getByRole("heading", { name: "features/popup.rs" }).click();
  await overlay.waitFor({ state: "hidden" });
  console.log("The anchored popup selects by key and drag-release, then dismisses outside");

  await page.setViewportSize({ width: 390, height: 844 });
  const reflow = await chapter("examples/apps/loaders.html");
  await click(reflow.getByRole("button", { name: "start", exact: true }));
  const running = reflow.getByRole("button", { name: /^(pause|again|resume)$/ });
  await running.waitFor();
  const narrowWidth = (await page.locator(".kui-example iframe").boundingBox()).width;
  for (const width of [1120, 390]) {
    await page.setViewportSize({ width, height: 950 });
    await page.waitForFunction(({ wide, narrowWidth }) => {
      const iframe = document.querySelector(".kui-example iframe");
      const width = iframe.getBoundingClientRect().width;
      const canvas = iframe.contentDocument.querySelector("canvas");
      return (wide ? width > narrowWidth + 200 : Math.abs(width - narrowWidth) < 1) &&
        Math.abs(iframe.closest(".kui-example").clientWidth - width) < 1 &&
        Math.abs(canvas.getBoundingClientRect().width - width) < 1;
    }, { wide: width === 1120, narrowWidth });
  }
  await running.waitFor();
  await page.setViewportSize({ width: 1180, height: 850 });
  console.log("The live canvas expands and shrinks with its container without a reload");

  const extensions = await chapter("extensions.html");
  await click(extensions.getByRole("button", { name: "Buy milk", exact: true }));
  await extensions.getByText("picked Buy milk", { exact: false }).waitFor();
  await click(extensions.getByRole("button", { name: "New note", exact: true }));
  await extensions.getByRole("button", { name: "Note 4", exact: true }).waitFor();
  await click(extensions.getByRole("button", { name: "Stamp (0)", exact: true }));
  await extensions.getByRole("button", { name: "Stamp (1)", exact: true }).waitFor();

  const tasks = await chapter("runner.html");
  await click(tasks.getByRole("button", { name: "Add", exact: true }));
  await tasks.getByText("Name the task first", { exact: true }).waitFor();
  await click(tasks.getByRole("textbox"));
  await page.frames().at(-1).waitForFunction(() => document.activeElement?.tagName === "INPUT");
  await page.keyboard.type("Keep the source");
  await page.keyboard.press("Enter");
  const task = tasks.getByRole("checkbox", { name: "Keep the source", exact: true });
  await task.waitFor();
  await page.waitForFunction(() => localStorage.getItem("kui-book/tasks.json")?.includes("Keep the source"));
  await page.getByRole("button", { name: "Reload example" }).click();
  await page.getByText("Ready", { exact: true }).waitFor();
  await task.waitFor();
  await click(task);
  await tasks.getByRole("checkbox", { name: "Keep the source", checked: true }).waitFor();
  await click(tasks.getByRole("button", { name: "Clear done", exact: true }));
  await task.waitFor({ state: "detached" });
  await page.waitForFunction(() => localStorage.getItem("kui-book/tasks.json") === "[]");
  console.log("Lua extension slots, task validation, keyboard submission and saved file");

  const deck = await chapter("presentations.html");
  const next = deck.getByRole("button", { name: "Next", exact: true });
  const previous = deck.getByRole("button", { name: "Previous", exact: true });
  await click(next);
  await deck.getByText("Make the explanation executable.", { exact: true }).waitFor();
  await click(next);
  await click(deck.getByRole("button", { name: "Add", exact: true }));
  await deck.getByText("Count: 1", { exact: true }).waitFor();
  await click(previous);
  await deck.getByText("Make the explanation executable.", { exact: true }).waitFor();
  await click(next);
  await deck.getByText("Count: 1", { exact: true }).waitFor();
  console.log("The chapter's Lua deck reveals, advances and retains slide state");

  const transition = await chapter("examples/features/transition.html");
  const canvas = await page.locator("iframe").boundingBox();
  const first = transition.getByRole("button", { name: "0%", exact: true });
  const before = await first.boundingBox();
  const scroll = () => page.evaluate(() => scrollY);
  const start = await scroll();
  await page.mouse.move(canvas.x + canvas.width / 2, canvas.y + canvas.height / 2);
  await page.mouse.wheel(0, 100);
  await page.waitForTimeout(300);
  assert.ok((await first.boundingBox()).y < before.y);
  assert.equal(await scroll(), start);
  for (let i = 0; i < 3; i++) {
    await page.mouse.wheel(0, 200);
    await page.waitForTimeout(150);
  }
  assert.ok(await scroll() > start);
  console.log("Wheel goes to the demo, then the chapter");

  await page.setViewportSize({ width: 390, height: 844 });
  const wheel = await chapter("examples/features/wheel_feel.html");
  const bounds = await page.locator(".kui-example-viewport").evaluate(element => ({
    frame: element.firstElementChild.getBoundingClientRect().width,
    visible: element.clientWidth,
    overflow: element.scrollWidth - element.clientWidth,
  }));
  assert.ok(bounds.frame <= bounds.visible + 1, JSON.stringify(bounds));
  assert.equal(bounds.overflow, 0);
  await click(wheel.getByRole("button", { name: "preset", exact: true }));
  await wheel.locator('[aria-label="relaxed"]').waitFor({ state: "attached" });
  console.log("Narrow embed fits and its scaled controls receive clicks");

  if (!useWebKit) {
    const touchPage = await browser.newPage({
      viewport: { width: 300, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true,
    });
    const cdp = await touchPage.context().newCDPSession(touchPage);
    const touch = (type, point) => cdp.send("Input.dispatchTouchEvent", {
      type, touchPoints: point ? [point] : [],
    });
    await touchPage.goto(new URL("examples/features/popup.html", base).href);
    await touchPage.getByText("Ready", { exact: true }).waitFor();
    const touchDemo = touchPage.frameLocator("iframe");
    const touchOverlay = touchDemo.locator(".popup-overlay");
    const touchRow = name => touchOverlay.getByRole("button", { name, exact: true });
    const start = await paintedPopupField(touchDemo.locator("main > canvas"));
    await touch("touchStart", start);
    await touch("touchMove", { x: start.x, y: start.y + 14 });
    await touchRow("Osmium").waitFor({ state: "attached" });
    const target = await touchRow("Osmium").boundingBox();
    const highlight = await rowColor(touchRow("Aluminium"));
    assert.notDeepEqual(await rowColor(touchRow("Osmium")), highlight);
    await touch("touchMove", { x: target.x + target.width / 2, y: target.y + target.height / 2 });
    await touch("touchEnd");
    await touchOverlay.waitFor({ state: "hidden" });
    await touchDemo.getByRole("button", { name: "Alloy", exact: true }).evaluate(el => el.click());
    await touchRow("Osmium").waitFor({ state: "attached" });
    assert.deepEqual(await rowColor(touchRow("Osmium")), highlight);
    console.log("Touch drag-release from the painted field selects outside the narrow owner");

    await touchPage.goto(new URL("examples/apps/counter.html", base).href);
    await touchPage.getByText("Ready", { exact: true }).waitFor();
    const iframe = await touchPage.locator("iframe").boundingBox();
    const x = Math.round(iframe.x + 12);
    const y = Math.round(iframe.y + iframe.height - 12);
    await touch("touchStart", { x, y });
    await touch("touchMove", { x, y: y - 80 });
    await touch("touchEnd");
    await touchPage.waitForFunction(() => scrollY > 0);
    await touchPage.close();
    console.log("A swipe on the demo background scrolls the chapter");
  }

  await page.setViewportSize({ width: 900, height: 650 });
  await page.addInitScript(() => {
    window.loops = [];
    const start = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function(...args) {
      if (this.loop) window.loops.push([this.context, this.buffer]);
      return start.apply(this, args);
    };
  });
  await page.goto(new URL("live/gallery.html?example=features/audio&renderer=canvas", base).href);
  await page.locator("#status").waitFor({ state: "hidden" });
  await click(page.getByRole("button", { name: "hum: off" }));
  await page.waitForFunction(() => window.loops.some(([context]) => context.state === "running"));
  const rendered = await page.evaluate(async () => {
    const [[, buffer]] = window.loops;
    const offline = new OfflineAudioContext(1, buffer.length * 3, buffer.sampleRate);
    const source = offline.createBufferSource();
    source.buffer = buffer;
    source.loop = true;
    source.connect(offline.destination);
    source.start();
    const output = (await offline.startRendering()).getChannelData(0);
    let maxError = 0;
    for (let i = buffer.length; i < output.length; i++) {
      maxError = Math.max(maxError, Math.abs(output[i] - output[i % buffer.length]));
    }
    return { samples: buffer.length, maxError };
  });
  assert.equal(rendered.samples, 2000);
  assert.ok(rendered.maxError < 1e-5, JSON.stringify(rendered));
  await click(page.getByRole("button", { name: "hum: on" }));
  await page.getByRole("button", { name: "hum: off" }).waitFor();
  assert.deepEqual(errors, []);
  console.log("Hum uses a continuous 2,000-sample loop");
} finally {
  await browser.close();
}

async function paintedPopupField(owner) {
  const bounds = await owner.boundingBox();
  const point = await owner.evaluate(canvas => {
    const x = Math.floor(canvas.width / 2);
    const pixels = canvas.getContext("2d").getImageData(x, 0, 1, canvas.height).data;
    // At this column only the Alloy field uses the theme's sunken fill.
    const rows = [];
    for (let y = 0; y < canvas.height; y++) {
      const [r, g, b, a] = pixels.subarray(y * 4, y * 4 + 4);
      if (a === 255 && ((r === 14 && g === 16 && b === 22) || (r === 236 && g === 238 && b === 242))) {
        rows.push(y);
      }
    }
    if (rows.length < 5) return null;
    return { x: (x + 0.5) / canvas.width, y: (rows[Math.floor(rows.length / 2)] + 0.5) / canvas.height };
  });
  assert.ok(point, "the Alloy field is visible in the owner canvas");
  return { x: bounds.x + point.x * bounds.width, y: bounds.y + point.y * bounds.height };
}
