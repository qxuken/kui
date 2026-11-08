import assert from "node:assert/strict";
const { chromium } = await import(process.env.KUI_PLAYWRIGHT ?? "playwright");

const base = process.env.KUI_WEB_URL ?? "http://127.0.0.1:8080";
const browser = await chromium.launch(process.env.KUI_CHROME_CHANNEL
  ? { channel: process.env.KUI_CHROME_CHANNEL } : {});
try {
  for (const mode of ["attach", "first-frame", "first-device-lost", "reconnect"]) {
    const page = await browser.newPage({ viewport: { width: 1060, height: 850 } });
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.addInitScript(() => {
      if (!navigator.gpu) Object.defineProperty(navigator, "gpu", { value: {} });
    });
    await page.route("**/notes-wgpu.js", route => route.fulfill({
      contentType: "text/javascript",
      body: `
        import init, * as real from "./notes-wgpu.js?real";
        const mode = ${JSON.stringify(mode)};
        let mounts = 0;
        let frames = 0;
        export default init;
        export async function attach_wgpu(canvas, ...callbacks) {
          mounts++;
          if (mode === "attach" || (mode === "reconnect" && mounts === 2)) {
            canvas.getContext("webgpu");
            window.injectedFailure = mode;
            throw new Error("injected attach failure");
          }
          await real.attach_wgpu(canvas, ...callbacks);
          window.mounts = mounts;
        }
        export function frame_wgpu(...args) {
          frames++;
          if (frames === 1 && mode === "first-frame") {
            window.injectedFailure = mode;
            throw new Error("injected draw failure");
          }
          if (frames === 1 && mode === "first-device-lost") {
            window.injectedFailure = mode;
            return 2;
          }
          if (window.injectLoss) {
            window.injectLoss = false;
            return 2;
          }
          return real.frame_wgpu(...args);
        }
      `,
    }));
    await page.goto(`${base}/notes.html`);
    await page.waitForSelector("#status", { state: "hidden", timeout: 6000 });
    const recover = mode === "reconnect";
    assert.equal(await page.locator("#renderer").textContent(), recover ? "kui-wgpu" : "Canvas 2D");
    if (recover) {
      await page.keyboard.press("Tab");
      await page.keyboard.type("draft ");
      const editor = page.getByRole("textbox", { name: "Note text" });
      await page.waitForFunction(() => document.querySelector("textarea").value.startsWith("draft "));
      const saved = await editor.inputValue();
      await page.evaluate(() => { window.injectLoss = true; });
      await page.mouse.move(200, 200);
      await page.waitForFunction(() => window.mounts === 3);
      assert.equal(await editor.inputValue(), saved);
    }
    assert.equal(await page.evaluate(() => window.injectedFailure), mode);
    const row = await page.getByRole("button", { name: "Note 02", exact: true }).boundingBox();
    await page.mouse.click(row.x + row.width / 2, row.y + row.height / 2);
    await page.getByRole("heading", { name: "Note 02", exact: true }).waitFor();
    assert.deepEqual(errors, []);
    console.log(mode, "loaded and accepted a click");
    await page.close();
  }
} finally {
  await browser.close();
}
