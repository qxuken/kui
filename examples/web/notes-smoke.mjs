import assert from "node:assert/strict";
const { chromium, firefox, webkit } = await import(process.env.KUI_PLAYWRIGHT ?? "playwright");

const engines = { chromium, firefox, webkit };
const base = process.env.KUI_WEB_URL ?? "http://127.0.0.1:8080";
const renderer = process.env.KUI_WEB_RENDERER ?? "canvas";
const rendererNames = { canvas: "Canvas 2D", wgpu: "kui-wgpu" };
assert.ok(Object.hasOwn(rendererNames, renderer), `unknown renderer ${renderer}`);
const gpu = renderer !== "canvas";
const requested = process.argv.slice(2);
for (const name of requested.length ? requested : Object.keys(engines)) {
  const engine = engines[name];
  assert.ok(engine, `unknown browser ${name}`);
  const browser = await engine.launch(name === "chromium" && process.env.KUI_CHROME_CHANNEL
    ? { channel: process.env.KUI_CHROME_CHANNEL } : {});
  try {
    const page = await browser.newPage({viewport:{width:1060,height:850}});
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.addInitScript((renderer) => {
      if (renderer === "wgpu") {
        window.gpuDraws = [];
        const proto = GPURenderPassEncoder.prototype;
        const draw = proto.draw;
        proto.draw = function (vertices, instances, ...offsets) {
          window.gpuDraws.push(instances);
          draw.call(this, vertices, instances, ...offsets);
        };
        return;
      }
      const proto = CanvasRenderingContext2D.prototype;
      const composite = Object.getOwnPropertyDescriptor(proto, "globalCompositeOperation");
      window.tintComposites = 0;
      window.visibleBackgrounds = 0;
      window.fullBlits = 0;
      const fillRect = proto.fillRect;
      proto.fillRect = function (x, y, w, h) {
        if (this.canvas.isConnected) window.visibleBackgrounds++;
        fillRect.call(this, x, y, w, h);
      };
      const drawImage = proto.drawImage;
      proto.drawImage = function (...args) {
        const [x, y, w, h] = args.slice(-4);
        if (this.canvas.isConnected && args.length === 9 && x === 0 && y === 0
            && w === this.canvas.width && h === this.canvas.height) window.fullBlits++;
        drawImage.apply(this, args);
      };
      Object.defineProperty(proto, "globalCompositeOperation", {
        ...composite,
        set(value) {
          if (value === "source-in" || value === "destination-in") window.tintComposites++;
          composite.set.call(this, value);
        },
      });
    }, renderer);
    await page.goto(`${base}/notes.html?renderer=${renderer}`);
    await page.waitForSelector("#status",{state:"hidden"});
    const editor = page.getByRole("textbox",{name:"Note text"});
    const expectValue = async (value) => {
      await page.waitForFunction(v => document.querySelector("#access textarea")?.value === v, value, {timeout:3000});
      assert.equal(await editor.inputValue(), value);
    };
    const expectFocus = (label) => page.waitForFunction(
      value => document.activeElement?.getAttribute("aria-label") === value, label, {timeout:3000});
    const expectNote = (n) => page.getByRole("heading",{name:`Note ${n}`,exact:true}).waitFor();
    const primary = await page.evaluate(() => /Mac/.test(navigator.platform)) ? "Meta" : "Control";
    const selectAll = () => page.keyboard.press(`${primary}+a`);
    const pixels = () => gpu
      ? page.locator("canvas").screenshot().then(image => image.toString("base64"))
      : page.locator("canvas").evaluate(c => {
        const r = devicePixelRatio;
        const data = c.getContext("2d").getImageData(47*r,95*r,480*r,105*r).data;
        let hash = 0;
        for (let i=0; i<data.length; i+=4) hash = (Math.imul(hash,31)+data[i])|0;
        return hash;
      });
    const transfer = (type, value) => editor.evaluate((el, {type,value}) => {
      const clipboardData = new DataTransfer();
      if (value) clipboardData.setData("text/plain", value);
      const ev = new Event(type,{bubbles:true,cancelable:true});
      Object.defineProperty(ev, "clipboardData", {value:clipboardData});
      el.dispatchEvent(ev);
      return clipboardData.getData("text/plain");
    }, {type,value});
    assert.equal(await page.getByRole("list",{name:"Notes"}).count(),1);
    assert.equal(await page.getByRole("listitem").count(),32);
    await page.keyboard.press("Tab");
    await expectFocus("Note text");
    const initial = await pixels();
    const box = await page.locator("canvas").boundingBox();
    const tintCount = await page.evaluate(() => window.tintComposites);
    await page.mouse.move(box.x + 49, box.y + 105);
    await page.mouse.down();
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const { visibleBackgrounds, fullBlits, gpuDrawCount } = await page.evaluate(() => ({
      visibleBackgrounds: window.visibleBackgrounds,
      fullBlits: window.fullBlits,
      gpuDrawCount: window.gpuDraws?.length,
    }));
    await page.mouse.move(box.x + 215, box.y + 105, { steps: 8 });
    await page.mouse.up();
    await page.waitForFunction(() => {
      const el = document.activeElement;
      return el.selectionEnd > el.selectionStart;
    });
    if (gpu) {
      const draws = await page.evaluate(() => window.gpuDraws);
      assert.ok(draws.length > gpuDrawCount && gpuDrawCount > 0);
      assert.ok(draws.slice(gpuDrawCount).every(count => count >= draws[gpuDrawCount - 1]),
        "the GPU renderer redraws the complete scene during selection");
    } else {
      assert.equal(await page.evaluate(() => window.tintComposites), tintCount,
        "drag selection reuses tinted glyphs from the unchanged atlas");
      assert.equal(await page.evaluate(() => window.visibleBackgrounds), visibleBackgrounds,
        "drag selection composites a patch onto the visible canvas");
      assert.equal(await page.evaluate(() => window.fullBlits), fullBlits,
        "drag selection copies only the changed region from the staging canvas");
    }
    assert.notEqual(await pixels(), initial, "drag selection paints a visible highlight");
    await page.mouse.dblclick(box.x + 77, box.y + 105);
    await page.waitForFunction(() => {
      const el = document.querySelector("#access textarea");
      return el.value.slice(el.selectionStart, el.selectionEnd) === "little";
    }, undefined, {timeout:3000});
    await selectAll();
    await page.keyboard.type("Hello notes");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Second line");
    await expectValue("Hello notes\nSecond line");
    assert.notEqual(await pixels(), initial);

    await selectAll();
    await page.waitForFunction(() => {const el=document.activeElement; return el.selectionStart===0 && el.selectionEnd===el.value.length;});
    assert.equal(await transfer("copy"),"Hello notes\nSecond line");
    assert.equal(await transfer("cut"),"Hello notes\nSecond line");
    await expectValue("");
    await transfer("paste","Pasted line\ncafé ");
    await expectValue("Pasted line\ncafé ");

    if (name === "chromium") {
      const cdp = await page.context().newCDPSession(page);
      await cdp.send("Input.imeSetComposition", {text:"é",selectionStart:1,selectionEnd:1});
      await page.waitForTimeout(50);
      assert.equal(await editor.inputValue(),"Pasted line\ncafé é","the browser holds native preedit");
      await cdp.send("Input.insertText", {text:"é"});
      await expectValue("Pasted line\ncafé é");
    } else {
      await editor.evaluate(el => {
        for (const [type,data] of [["compositionstart",""],["compositionupdate","e"],["compositionupdate","é"],["compositionend","é"]]) {
          el.dispatchEvent(new CompositionEvent(type,{data,bubbles:true}));
        }
        el.dispatchEvent(new InputEvent("beforeinput",{inputType:"insertText",data:"é",bubbles:true,cancelable:true}));
      });
      await expectValue("Pasted line\ncafé é");
    }
    await page.keyboard.type("!");
    await expectValue("Pasted line\ncafé é!");
    await editor.evaluate(el => el.setSelectionRange(12,16,"forward"));
    await page.waitForTimeout(40);
    assert.equal(await transfer("copy"), "café");
    await page.keyboard.press("End");

    await page.keyboard.press("Tab");
    await expectFocus("Save note");
    await page.keyboard.press("Enter");
    await page.getByText("Saved in this tab",{exact:true}).waitFor({state:"attached",timeout:4000});
    await page.keyboard.press("Tab");
    await expectFocus("Note 01");
    await page.keyboard.press("ArrowDown");
    await expectFocus("Note 02");
    await page.keyboard.press("Enter");
    await expectNote("02");
    await expectValue("This is note 02.\n\nMake it yours.");
    const list = page.getByRole("list",{name:"Notes"});
    const row = (n) => list.getByRole("button",{name:`Note ${n}`,exact:true});
    const third = await row("03").boundingBox();
    await page.mouse.click(third.x + third.width / 2, third.y + third.height / 2);
    await expectNote("03");
    await row("01").evaluate(el => el.click()); // DOM activation is the access-action path.
    await expectNote("01");
    await expectValue("Pasted line\ncafé é!");
    await editor.focus();
    await selectAll();
    await transfer("paste", "A😀B\ncafé");
    await expectValue("A😀B\ncafé");
    await editor.evaluate(el => el.setSelectionRange(1, 3));
    await page.waitForTimeout(40);
    assert.equal(await transfer("copy"), "😀", "UTF-16 selection maps to core UTF-8");

    const canvas = await page.locator("canvas").boundingBox();
    const before = await row("10").boundingBox();
    await page.mouse.move(canvas.x + canvas.width - 95, canvas.y + 300);
    await page.mouse.wheel(0,550);
    await page.waitForFunction(({top}) => {
      const button = document.querySelector('#access button[aria-label="Note 10"]');
      return button.getBoundingClientRect().top < top - 100;
    }, {top:before.y}, {timeout:3000});
    await row("22").focus();
    await page.keyboard.press("Enter");
    await expectNote("22");
    await expectValue("This is note 22.\n\nMake it yours.");
    await page.setViewportSize({width:430,height:780});
    await page.waitForFunction(() => document.querySelector("canvas").width === 398*devicePixelRatio);
    assert.deepEqual(errors,[]);

    if (name === "chromium") {
      const cdp = await page.context().newCDPSession(page);
      const {nodes} = await cdp.send("Accessibility.getFullAXTree");
      for (const [role,label] of [["textBox","Note text"],["button","Save note"],["list","Notes"]]) {
        assert.ok(nodes.some(n => !n.ignored && n.role?.value?.toLowerCase() === role.toLowerCase()
          && n.name?.value === label), `AX tree contains ${role} ${label}`);
      }
    }
    console.log(name,browser.version(),rendererNames[renderer],"all notes flows passed");
  } finally {
    await browser.close();
  }
}
