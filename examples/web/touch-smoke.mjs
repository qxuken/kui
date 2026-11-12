import assert from "node:assert/strict";
const { chromium } = await import(process.env.KUI_PLAYWRIGHT ?? "playwright");
const base = process.env.KUI_WEB_URL ?? "http://127.0.0.1:8080";
const renderers = process.argv.length > 2 ? process.argv.slice(2) : ["canvas", "wgpu"];
const browser = await chromium.launch(process.env.KUI_CHROME_CHANNEL
  ? { channel: process.env.KUI_CHROME_CHANNEL } : {});

try {
  for (const renderer of renderers) {
    const page = await browser.newPage({
      viewport: { width: 390, height: 780 }, deviceScaleFactor: 2,
      hasTouch: true, isMobile: true,
    });
    try {
      const cdp = await page.context().newCDPSession(page);
      const send = (type, touchPoints) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints });
      const point = (box, x = box.width / 2, y = box.height / 2) => ({ x: box.x + x, y: box.y + y });
      const swipe = async (start, dx, dy) => {
        await send("touchStart", [start]);
        for (let step = 1; step <= 8; step++) {
          await send("touchMove", [{ x: start.x + dx * step / 8, y: start.y + dy * step / 8 }]);
        }
        await send("touchEnd", []);
      };
      const row = n => page.getByRole("button", { name: `Note ${n}`, exact: true });
      const title = n => page.getByRole("heading", { name: `Note ${n}`, exact: true });
      const canvas = page.locator("canvas");
      await page.addInitScript(() => {
        window.addEventListener("pointerup", event => {
          if (event.pointerType === "touch") window.focusAtTouchRelease = document.activeElement?.tagName;
        });
      });
      await page.goto(`${base}/notes.html?renderer=${renderer}`);
      await page.waitForSelector("#status", { state: "hidden" });

      const initialCanvas = await canvas.boundingBox();
      const first = await row("01").boundingBox();
      const thirdTop = (await row("03").boundingBox()).y - initialCanvas.y;
      await swipe(point(first), 0, -84);
      await page.waitForFunction(top => {
        const row = document.querySelector('#access button[aria-label="Note 03"]');
        const canvas = document.querySelector("canvas");
        return row.getBoundingClientRect().top - canvas.getBoundingClientRect().top < top - 65;
      }, thirdTop, { timeout: 3000 });
      assert.equal(await title("01").count(), 1, "swiping a row scrolls without opening it");

      const fourth = point(await row("04").boundingBox());
      await send("touchStart", [fourth]);
      await send("touchCancel", []);
      assert.equal(await title("01").count(), 1, "cancelled touch does not click");
      const finger = { ...fourth, id: 1 };
      await send("touchStart", [finger]);
      await send("touchStart", [finger, { x: finger.x + 25, y: finger.y, id: 2 }]);
      await send("touchEnd", []);
      assert.equal(await title("01").count(), 1, "a second finger cancels the pending tap");
      await page.touchscreen.tap(fourth.x, fourth.y);
      await title("04").waitFor({ timeout: 3000 });

      const surface = await canvas.boundingBox();
      const editorPoint = point(surface, 49, 105);
      await page.touchscreen.tap(editorPoint.x, editorPoint.y);
      assert.equal(await page.evaluate(() => window.focusAtTouchRelease), "TEXTAREA",
        "the editor is focused before the trusted tap returns");
      await swipe(editorPoint, 94, 0);
      await page.waitForFunction(() => {
        const editor = document.activeElement;
        return editor.selectionEnd > editor.selectionStart;
      }, undefined, { timeout: 3000 });
      console.log(renderer, "touch pan, cancellation, tap and text drag passed");
    } finally {
      await page.close();
    }
  }
} finally {
  await browser.close();
}
