// The editable page exercises the same prebuilt runner as packaged apps.
// KUI_BOOK_URL / KUI_PLAYWRIGHT / KUI_CHROME_CHANNEL follow smoke.mjs.
import assert from "node:assert/strict";
const engines = await import(process.env.KUI_PLAYWRIGHT ?? "playwright");
const base = process.env.KUI_BOOK_URL ?? "http://127.0.0.1:8080";
const renderer = process.env.KUI_WEB_RENDERER ?? "canvas";
const names = process.argv.slice(2);
const script = `
function init() return { count = 0 } end
function update(message) model.count = model.count + 1 end
function view(env)
  return column { width = "grow", height = "grow", pad = 16,
    button { label = "Count " .. model.count, on_click = { kind = "increment" } },
  }
end
`;

for (const name of names.length ? names : ["chromium", "firefox", "webkit"]) {
  const browser = await engines[name].launch(name === "chromium" && process.env.KUI_CHROME_CHANNEL
    ? { channel: process.env.KUI_CHROME_CHANNEL } : {});
  try {
    const context = await browser.newContext({ viewport: { width: 1100, height: 860 } });
    await context.addInitScript(() => {
      const compile = WebAssembly.compile;
      window.playgroundCompiles = 0;
      WebAssembly.compile = (...args) => { window.playgroundCompiles++; return compile(...args); };
    });
    const page = await context.newPage();
    const errors = [];
    let runtimeRequests = 0;
    const runtimeUrls = new Set();
    page.on("pageerror", error => errors.push(error.message));
    page.on("request", request => {
      if (new URL(request.url()).pathname.endsWith("/runtime/runner.wasm")) {
        runtimeRequests++;
        runtimeUrls.add(request.url());
      }
    });
    await page.goto(`${base}/live/playground/index.html?renderer=${renderer}`);
    const source = page.getByRole("textbox", { name: "Lua source" });
    async function settled(ok = true) {
      await page.waitForFunction(() => document.querySelector("#preview").getAttribute("aria-busy") === "false", undefined, { timeout: 20000 });
      if (ok) {
        assert.equal(await page.locator("#error").evaluate(el => el.hidden), true, await page.locator("#error").textContent());
        assert.match(await page.locator("#status").textContent(), /^Ready/);
      }
    }
    async function run(code, ok = true) {
      await source.fill(code);
      await page.locator("#run").click();
      await settled(ok);
    }
    const view = () => page.frameLocator("iframe[data-active]");
    await settled();
    await view().getByRole("button", { name: "Increment", exact: true }).waitFor();
    await run(script);
    await view().getByRole("button", { name: "Count 0", exact: true }).press("Enter");
    await view().getByRole("button", { name: "Count 1", exact: true }).waitFor();
    await page.locator("#run").click();
    await settled();
    await view().getByRole("button", { name: "Count 0", exact: true }).waitFor();
    const previous = await page.locator("iframe[data-active]").elementHandle();
    for (const [code, message] of [
      ["function view(", "main.lua"],
      ["function view(env) error('broken view') end", "broken view"],
      ["while true do end", "instruction limit"],
      ["kui.spawn(function() while true do end end); function view() return column {} end", "instruction limit"],
      ["local x = string.rep('x', 64 * 1024 * 1024)", "memory"],
    ]) {
      await run(code, false);
      assert.match(await page.locator("#error").textContent(), new RegExp(message));
      assert.equal(await previous.evaluate(el => el.isConnected), true, "failed runs keep the previous instance");
      await view().getByRole("button", { name: "Count 0", exact: true }).waitFor();
    }
    await run("--" + "x".repeat(128 * 1024), false);
    assert.match(await page.locator("#error").textContent(), /128 KiB/);
    const files = `
function init()
  assert(not kui.fs.exists('/data/probe'), 'data leaked from a previous Run')
  kui.fs.write('/data/probe', 'temporary')
  local ok = pcall(kui.fs.write, '/app/main.lua', 'overwritten')
  assert(not ok, 'app files must remain read-only')
  return {}
end
function view() return column { button { label = 'Fresh files' } } end
`;
    await run(files);
    await run(files);
    await view().getByRole("button", { name: "Fresh files", exact: true }).waitFor();
    await source.fill(script);
    await source.press("Control+Enter");
    await settled();
    await view().getByRole("button", { name: "Count 0", exact: true }).waitFor();
    await page.locator("#example").selectOption("layout");
    await settled();
    assert.match(await source.inputValue(), /Make room/);
    await page.locator("#example").selectOption("controls");
    await settled();
    await view().getByRole("textbox", { name: "Your name", exact: true }).waitFor();
    assert.equal(runtimeRequests, 1, "runs and presets reuse one runtime download");
    assert.equal(await page.evaluate(() => window.playgroundCompiles), 1, "runs share the compiled Lua module");
    await source.fill(script);
    await page.locator("#run").click();
    await settled();
    await page.reload();
    await settled();
    assert.equal(await source.inputValue(), script, "reload restores the saved draft");
    await page.locator("#reset").click();
    await settled();
    assert.match(await source.inputValue(), /Hello, /, "reset restores the selected preset");
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.reload();
    await settled();
    assert.equal(await page.locator("#example").inputValue(), "controls");
    await page.goto(`${base}/playground.html?example=layout`);
    const editor = page.frameLocator(".kui-example-viewport iframe");
    await editor.locator("#status").filter({ hasText: /^Ready$/ }).waitFor().catch(async () => {
      assert.equal(await editor.locator("#error").evaluate(el => el.hidden), true, await editor.locator("#error").textContent());
      throw new Error("embedded playground did not become ready");
    });
    assert.match(await editor.getByRole("textbox", { name: "Lua source" }).inputValue(), /Make room/);
    const devtools = page.getByRole("button", { name: "Devtools", exact: true });
    await devtools.click();
    await page.waitForFunction(() => document.querySelector(".kui-example-toolbar button[aria-pressed]")?.getAttribute("aria-pressed") === "true");
    await devtools.click();
    await page.waitForFunction(() => document.querySelector(".kui-example-toolbar button[aria-pressed]")?.getAttribute("aria-pressed") === "false");
    await page.goto(base + "/live/tasks/index.html?renderer=canvas");
    await page.getByRole("textbox", { name: "New task" }).waitFor();
    assert.equal(runtimeUrls.size, 1, "task list and playground use the same content-versioned runtime URL");
    assert.deepEqual(errors, []);

    // Storage is optional for editing, and unavailable for preview files.
    // A failed child module or runtime fetch must report an error and allow
    // retry with a new instance, rather than leave an invisible pending frame.
    const privateContext = await browser.newContext();
    await privateContext.addInitScript(() => {
      for (const name of ["indexedDB", "localStorage"]) {
        Object.defineProperty(window, name, { get() { throw new Error("Storage disabled for test"); } });
      }
    });
    const retry = await privateContext.newPage();
    const childModule = /\/playground\/view\.js$/;
    await retry.route(childModule, route => route.fulfill({ status: 503, body: "unavailable" }));
    await retry.goto(`${base}/live/playground/index.html?renderer=canvas`);
    await retry.locator("#error").waitFor({ state: "visible" });
    assert.equal(await retry.locator("iframe[data-active]").count(), 0);
    await retry.unroute(childModule);
    const binary = /\/runtime\/runner\.wasm\?/;
    await retry.route(binary, route => route.fulfill({ status: 503, body: "unavailable" }));
    await retry.locator("#run").click();
    await retry.locator("#error").filter({ hasText: /503/ }).waitFor();
    await retry.unroute(binary);
    await retry.locator("#run").click();
    await retry.locator("#status").filter({ hasText: /^Ready$/ }).waitFor();
    assert.match(await retry.locator(".hint").textContent(), /cannot be saved/);
    await retry.getByRole("textbox", { name: "Lua source" }).fill(files);
    await retry.locator("#run").click();
    await retry.frameLocator("iframe[data-active]").getByRole("button", { name: "Fresh files", exact: true }).waitFor();
    await privateContext.close();
    console.log(name, renderer, "playground: editing, interaction, fresh state/files, error recovery, limits, caching, presets, mobile and book embed passed");
  } finally { await browser.close(); }
}
