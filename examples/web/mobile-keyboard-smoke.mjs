import assert from "node:assert/strict";
const { chromium, webkit } = await import(process.env.KUI_PLAYWRIGHT ?? "playwright");
const base = process.env.KUI_WEB_URL ?? "http://127.0.0.1:8080";

// Desktop WebKit cannot assert that an iOS software keyboard is visible.
// Pin the event ordering/default cancellation here; verify the keyboard
// itself in Safari on the iPhone and iPad simulators.
for (const [name, engine] of Object.entries({ chromium, webkit })) {
  const browser = await engine.launch();
  try {
    for (const viewport of [{ width: 390, height: 780 }, { width: 820, height: 1180 }]) {
      const page = await browser.newPage({ viewport, hasTouch: true, isMobile: true });
      try {
        const errors = [];
        page.on("pageerror", error => errors.push(error.message));
        await page.addInitScript(() => {
          window.addEventListener("touchend", event => {
            window.touchRelease = {
              focus: document.activeElement?.tagName,
              cancelled: event.defaultPrevented,
              trusted: event.isTrusted,
            };
          });
        });
        await page.goto(`${base}/notes.html?renderer=canvas`);
        await page.waitForSelector("#status", { state: "hidden" });
        const canvas = await page.locator("canvas").boundingBox();
        await page.touchscreen.tap(canvas.x + 49, canvas.y + 105);
        assert.deepEqual(await page.evaluate(() => window.touchRelease), {
          focus: "TEXTAREA", cancelled: true, trusted: true,
        }, "the trusted touch focuses the editor and suppresses the canvas default action");
        const editor = page.getByRole("textbox", { name: "Note text" });
        await page.waitForTimeout(50);
        assert.equal(await editor.evaluate(el => el === document.activeElement), true);
        await page.keyboard.insertText("Mobile ✓");
        await page.waitForFunction(() => document.querySelector("textarea").value.includes("Mobile ✓"));
        const expected = await editor.evaluate(el =>
          el.value.slice(0, el.selectionStart) + "\n" + el.value.slice(el.selectionEnd));
        await page.keyboard.press("Enter");
        await page.keyboard.insertText("x");
        await page.keyboard.press("Backspace");
        await page.waitForFunction(value => document.querySelector("textarea").value === value, expected);
        assert.deepEqual(errors, []);
        console.log(name, viewport.width, "trusted touch focus, typing, Enter and Backspace passed");
      } finally {
        await page.close();
      }
    }
  } finally {
    await browser.close();
  }
}
