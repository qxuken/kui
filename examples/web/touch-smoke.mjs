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
        await page.waitForTimeout(100);
        await send("touchEnd", []);
      };
      const row = n => page.getByRole("button", { name: `Note ${n}`, exact: true });
      const title = n => page.getByRole("heading", { name: `Note ${n}`, exact: true });
      const canvas = page.locator("canvas");
      await page.addInitScript(() => {
        window.addEventListener("touchend", () => {
          window.focusAtTouchRelease = document.activeElement?.tagName;
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
        "the editor is focused before the trusted touchend returns");
      await page.waitForTimeout(50);
      assert.equal(await page.evaluate(() => document.activeElement?.tagName), "TEXTAREA",
        "the tap's default action does not steal editor focus");
      await page.keyboard.insertText("Touch keyboard");
      await page.waitForFunction(() => document.querySelector("textarea").value.includes("Touch keyboard"));
      assert.match(await page.getByRole("textbox", { name: "Note text" }).inputValue(), /Touch keyboard/);
      await swipe(editorPoint, 94, 0);
      await page.waitForFunction(() => {
        const editor = document.activeElement;
        return editor.selectionEnd > editor.selectionStart;
      }, undefined, { timeout: 3000 });
      await page.reload();
      await page.waitForSelector("#status", { state: "hidden" });
      const top = () => row("03").evaluate(el => el.getBoundingClientRect().top);
      const flick = point(await row("01").boundingBox());
      await send("touchStart", [flick]);
      for (let step = 1; step <= 5; step++) {
        await page.waitForTimeout(16);
        await send("touchMove", [{ x: flick.x, y: flick.y - 12 * step }]);
      }
      await send("touchEnd", []);
      const released = await top();
      await page.waitForTimeout(160);
      assert.ok(await top() < released - 20, "a flick keeps scrolling after finger release");
      const stop = { x: flick.x, y: flick.y - 30 };
      await send("touchStart", [stop]);
      await page.waitForTimeout(50);
      const stopped = await top();
      await page.waitForTimeout(160);
      assert.equal(await top(), stopped, "a new finger stops momentum immediately");
      await send("touchCancel", []);

      await page.reload();
      await page.waitForSelector("#status", { state: "hidden" });
      await swipe(point(await row("01").boundingBox()), 0, -60);
      await page.waitForTimeout(50);
      const held = await top();
      await page.waitForTimeout(160);
      assert.equal(await top(), held, "holding still before release does not start momentum");
      console.log(renderer, "touch pan, momentum, cancellation, keyboard and text drag passed");
    } finally {
      await page.close();
    }
  }
} finally {
  await browser.close();
}
