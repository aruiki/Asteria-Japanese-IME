// 原稿HTMLを画像化する。製品のビルドや画像・MSIの書き換えは行わない。
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";
import { mkdir, writeFile } from "node:fs/promises";
const require = createRequire(process.env.ASTERIA_RENDER_MODULE_ROOT
  ? join(process.env.ASTERIA_RENDER_MODULE_ROOT, "package.json") : import.meta.url);
const { chromium } = require("playwright");
const here = dirname(fileURLToPath(import.meta.url));
const output = join(here, "assets");
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true,
  ...(process.env.ASTERIA_RENDER_CHANNEL ? {channel:process.env.ASTERIA_RENDER_CHANNEL} : {}) });
const receipts = [];
try {
  for (const [format, width, height] of [["hero",1600,720],["social",1200,630],["portrait",1080,1350]]) {
    const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
    const errors = [];
    page.on("pageerror", error => errors.push(String(error)));
    await page.goto(pathToFileURL(join(here, "creative.html")).href + "?format=" + format);
    await page.evaluate(async () => {
      await document.fonts.ready;
      await Promise.all([...document.images].map(image => image.decode()));
    });
    const layout = await page.evaluate(() => {
      const art = document.querySelector(".art").getBoundingClientRect();
      const blocks = [...document.querySelectorAll(".brand,.eyebrow,h1,.lead,.bottom,.features,.url")];
      return blocks.filter(el => getComputedStyle(el).display !== "none").map(el => {
        const r = el.getBoundingClientRect();
        return { text:el.textContent.trim(),x:r.x,y:r.y,width:r.width,height:r.height,
          inside:r.x>=0 && r.y>=0 && r.right<=art.right && r.bottom<=art.bottom };
      });
    });
    if (errors.length || layout.some(el => !el.inside)) throw new Error(JSON.stringify({format,errors,layout}));
    await page.locator(".art").screenshot({ path:join(output, format+".png") });
    receipts.push({format,width,height,layout,errors});
    await page.close();
  }
} finally { await browser.close(); }
if (process.env.ASTERIA_RENDER_RECEIPT) {
  await writeFile(process.env.ASTERIA_RENDER_RECEIPT, JSON.stringify(receipts,null,2)+"\n");
}
console.log("Rendered hero (1600x720), social (1200x630), portrait (1080x1350).");
