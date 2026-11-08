import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";
const { chromium } = await import(process.env.KUI_PLAYWRIGHT ?? "playwright");
const dir = process.env.KUI_BOOK_DIR ?? "target/book";
const base = process.env.KUI_BOOK_URL ?? "http://127.0.0.1:8080";
function* walk(root) {
  for (const entry of fs.readdirSync(root, {withFileTypes:true})) {
    const full = path.join(root, entry.name);
    if (entry.isDirectory()) yield* walk(full);
    else if (entry.name.endsWith(".html")) yield full;
  }
}
const ids = new Set();
for (const file of walk(dir)) {
  for (const m of fs.readFileSync(file, "utf8").matchAll(/gallery\.html\?example=([\w/-]+)/g)) ids.add(m[1]);
}
assert.ok(ids.size, "the built book has live demos");
const list = [...ids];
const bad = [];
const browser = await chromium.launch(process.env.KUI_CHROME_CHANNEL ? { channel: process.env.KUI_CHROME_CHANNEL } : {});
try {
  await Promise.all(Array.from({length: 3}, async () => {
    const page = await browser.newPage({viewport:{width:800,height:540}});
    let errors = [];
    page.on("pageerror", error => errors.push(error.message));
    for (;;) {
      const id = list.shift();
      if (!id) break;
      errors = [];
      try {
        await page.goto(base + "/live/gallery.html?renderer=wgpu&example=" + id,
          {waitUntil:"domcontentloaded",timeout:8000});
        await page.waitForFunction(() => document.querySelector("#status")?.hidden || document.querySelector("#status")?.textContent !== "Loading example…", undefined, {timeout:8000});
        assert.equal(await page.locator("#status").evaluate(el => el.hidden), true, await page.locator("#status").textContent());
        assert.deepEqual(errors, []);
        console.log("OK",id);
      } catch (e) {
        const why = [id, e.message.slice(0,180), errors];
        bad.push(why);
        console.log("FAIL",JSON.stringify(why));
      }
    }
  }));
  console.log("total",ids.size,"failed",bad.length);
  assert.deepEqual(bad, []);
} finally {await browser.close();}
