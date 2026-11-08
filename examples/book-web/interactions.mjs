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
    const touchPage = await browser.newPage({ viewport: { width: 390, height: 844 }, hasTouch: true });
    await touchPage.goto(new URL("examples/apps/counter.html", base).href);
    await touchPage.getByText("Ready", { exact: true }).waitFor();
    const iframe = await touchPage.locator("iframe").boundingBox();
    const x = Math.round(iframe.x + 12);
    const y = Math.round(iframe.y + iframe.height - 12);
    const cdp = await touchPage.context().newCDPSession(touchPage);
    await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
    await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x, y: y - 80 }] });
    await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
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
