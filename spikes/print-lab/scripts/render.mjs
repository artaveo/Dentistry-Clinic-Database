// Spike 2 (RTL PDF), 3 (thermal raster source), 4 (A4 compact print).
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { launch } from "./browser.mjs";
import { serve } from "./serve.mjs";

const here = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const out = path.join(here, "out");
// The template is the one the Tauri spike app ships (single source of truth).
const root = path.resolve(here, "..");
const DIST = "/tauri-shell/dist/";
fs.mkdirSync(out, { recursive: true });

const PX_PER_MM = 96 / 25.4;
// Printable dot widths at 203 dpi (8 dots/mm).
const THERMAL_DOTS = { "80": 576, "58": 384 };

const jobs = [
  // Continuous-roll receipts: page height = content height (no blank paper).
  { name: "receipt-80mm-fa", q: "paper=80&lang=fa", pageW: 80, margin: 4, png: true },
  { name: "receipt-80mm-ps", q: "paper=80&lang=ps", pageW: 80, margin: 4, png: true },
  { name: "receipt-80mm-en", q: "paper=80&lang=en", pageW: 80, margin: 4 },
  { name: "receipt-80mm-fa-latn", q: "paper=80&lang=fa&digits=latn", pageW: 80, margin: 4 },
  { name: "receipt-58mm-ps", q: "paper=58&lang=ps", pageW: 58, margin: 5, png: true },
  // Fixed sheets.
  { name: "receipt-a6-fa", q: "paper=a6&lang=fa", pageW: 105, pageH: 148, margin: 4 },
  { name: "receipt-a4-compact-top-right", q: "paper=a4&lang=fa&top=12&right=12", pageW: 210, pageH: 297, margin: 0 },
  { name: "receipt-a4-compact-bottom-left", q: "paper=a4&lang=ps&top=170&right=126", pageW: 210, pageH: 297, margin: 0 },
  // Small paper on a normal printer: real A5/A6 pages, and N-up A4 with cut marks.
  { name: "receipt-a5-fa", q: "paper=a5&lang=fa", pageW: 148, pageH: 210, margin: 6 },
  { name: "receipt-a6-ps", q: "paper=a6&lang=ps", pageW: 105, pageH: 148, margin: 4 },
  { name: "sheet-2up-fa", page: "sheet.html", q: "layout=2&lang=fa", pageW: 210, pageH: 297, margin: 0, cells: 2 },
  { name: "sheet-4up-ps", page: "sheet.html", q: "layout=4&lang=ps", pageW: 210, pageH: 297, margin: 0, cells: 4 },
];

const { server, base } = await serve(root);
const browser = await launch();
const report = [];
try {
  for (const job of jobs) {
    const paper = new URLSearchParams(job.q).get("paper");
    const dsf = THERMAL_DOTS[paper] ? THERMAL_DOTS[paper] / ((job.pageW - 2 * job.margin) * PX_PER_MM) : 1;
    const page = await browser.newPage({ deviceScaleFactor: dsf });
    await page.goto(`${base}${DIST}${job.page ?? "receipt.html"}?${job.q}`);
    await page.waitForSelector("body[data-ready='1']", { state: "attached" });
    await page.emulateMedia({ media: "print" });

    const contentMm = job.cells ? 0 : (await page.locator("#r").boundingBox()).height / PX_PER_MM;
    if (job.cells) {
      // Every receipt must fit inside its cell; overflow would be cut off.
      job.cellFit = await page.evaluate(() =>
        [...document.querySelectorAll(".cell")].map((c) => {
          const cr = c.getBoundingClientRect(), rr = c.querySelector(".receipt").getBoundingClientRect();
          return { fits: rr.bottom <= cr.bottom + 0.5 && rr.left >= cr.left - 0.5 && rr.right <= cr.right + 0.5, spareMm: Number(((cr.bottom - rr.bottom) * 25.4 / 96).toFixed(1)) };
        }));
    }
    const heightMm = job.pageH ?? Math.ceil(contentMm + 2 * job.margin);
    const m = `${job.margin}mm`;
    await page.pdf({
      path: path.join(out, `${job.name}.pdf`),
      width: `${job.pageW}mm`,
      height: `${heightMm}mm`,
      margin: { top: m, bottom: m, left: m, right: m },
      printBackground: true,
    });
    if (job.cells) {
      // Cut marks as printed pixels: ink coverage along each cut line
      // (dashed ⇒ roughly half the line is dark) vs. a blank reference row.
      // RTL documents right-align the 210mm body inside the wider viewport.
      const sheet = await page.locator("body").boundingBox();
      const shot = (await page.screenshot({ clip: sheet })).toString("base64");
      job.cutInk = await page.evaluate(async ([b64, cells]) => {
        const img = new Image(); img.src = `data:image/png;base64,${b64}`; await img.decode();
        const c = new OffscreenCanvas(img.width, img.height); const x = c.getContext("2d"); x.drawImage(img, 0, 0);
        const data = x.getImageData(0, 0, img.width, img.height).data;
        // A hairline is anti-aliased across ±1px: look 2px either side.
        const px = (X, Y) => data[(Y * img.width + X) * 4] < 200;
        const near = [-2, -1, 0, 1, 2];
        // The viewport is wider than the sheet: scan only the 210×297mm area.
        const mmPx = 96 / 25.4, sw = Math.floor(210 * mmPx), sh = Math.floor(297 * mmPx);
        const darkRow = (Y) => { let d = 0; for (let X = 0; X < sw; X++) d += near.some((o) => px(X, Y + o)); return d / sw; };
        const darkCol = (X) => { let d = 0; for (let Y = 0; Y < sh; Y++) d += near.some((o) => px(X + o, Y)); return d / sh; };
        const out = { horizontal: darkRow(Math.round(148.5 * mmPx)), blankRow: darkRow(Math.round(146 * mmPx)) };
        if (cells === 4) out.vertical = darkCol(Math.round(105 * mmPx));
        return out;
      }, [shot, job.cells]);
    }
    if (job.png) {
      // Clip to exactly the printer's dot width (element boxes round outward).
      const box = await page.locator("#r").boundingBox();
      await page.screenshot({
        path: path.join(out, `${job.name}.png`),
        clip: { x: box.x, y: box.y, width: THERMAL_DOTS[paper] / dsf, height: box.height },
      });
    }
    if (job.png) {
      // Same raster produced *inside* the page, as the app does before ESC/POS.
      const b64 = await page.evaluate(async (dots) => {
        const { rasterizeElement, blobToBase64 } = await import("./raster.js");
        const { png } = await rasterizeElement(document.getElementById("r"), dots);
        return blobToBase64(png);
      }, THERMAL_DOTS[paper]);
      fs.writeFileSync(path.join(out, `${job.name}.inapp.png`), Buffer.from(b64, "base64"));
      // Pixel comparison (1-bit, like the printer) against the screenshot.
      const shot = fs.readFileSync(path.join(out, `${job.name}.png`)).toString("base64");
      job.inappDiff = await page.evaluate(async ([a, b]) => {
        const load = async (src) => { const i = new Image(); i.src = src; await i.decode(); return i; };
        const [ia, ib] = await Promise.all([load(`data:image/png;base64,${a}`), load(`data:image/png;base64,${b}`)]);
        const w = Math.min(ia.width, ib.width), h = Math.min(ia.height, ib.height);
        const px = (img) => { const c = new OffscreenCanvas(w, h); const x = c.getContext("2d"); x.fillStyle = "#fff"; x.fillRect(0, 0, w, h); x.drawImage(img, 0, 0); return x.getImageData(0, 0, w, h).data; };
        const [da, db] = [px(ia), px(ib)];
        // Anti-aliasing shifts strokes by a sub-pixel, so compare 8×8 block
        // darkness (layout/font) and total ink (missing text, fallback font).
        const B = 8;
        let inkA = 0, inkB = 0, blocks = 0, blockDiff = 0;
        for (let by = 0; by + B <= h; by += B) for (let bx = 0; bx + B <= w; bx += B) {
          let sa = 0, sb = 0;
          for (let y = by; y < by + B; y++) for (let x = bx; x < bx + B; x++) {
            const i = (y * w + x) * 4;
            sa += 255 - da[i]; sb += 255 - db[i];
          }
          inkA += sa; inkB += sb;
          if (sa || sb) { blocks++; blockDiff += Math.abs(sa - sb) / (B * B * 255); }
        }
        return { inkRatio: Number((inkA / inkB).toFixed(3)), blockDiff: Number((blockDiff / blocks).toFixed(4)) };
      }, [b64, shot]);
    }
    report.push({ name: job.name, pageMm: [job.pageW, heightMm], dsf: Number(dsf.toFixed(4)), inappDiff: job.inappDiff, cells: job.cells, cellFit: job.cellFit, cutInk: job.cutInk });
    await page.close();
  }
} finally {
  await browser.close();
  server.close();
}
fs.writeFileSync(path.join(out, "render-report.json"), JSON.stringify({ browser: browser.version(), jobs: report }, null, 2));
console.table(report);
