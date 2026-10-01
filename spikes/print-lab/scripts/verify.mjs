// Machine checks for the RTL PDF spike. Visual checks: open out/*.pdf / *.png.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as pdfjs from "pdfjs-dist/legacy/build/pdf.mjs";

const out = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../out");
const PT_PER_MM = 72 / 25.4;
const report = JSON.parse(fs.readFileSync(path.join(out, "render-report.json"), "utf8"));
const ALLOWED_FONTS = /^[A-Z]{6}\+(Vazirmatn|NotoSansArabic)-/;
let failures = 0;

function check(ok, msg) {
  console.log(`${ok ? "PASS" : "FAIL"}  ${msg}`);
  if (!ok) failures++;
}

async function load(name) {
  const doc = await pdfjs.getDocument({ data: new Uint8Array(fs.readFileSync(path.join(out, `${name}.pdf`))), verbosity: 0 }).promise;
  const page = await doc.getPage(1);
  const text = await page.getTextContent();
  await page.getOperatorList(); // populates commonObjs with font objects
  const fonts = [...new Set(text.items.map((i) => i.fontName))].map((id) => page.commonObjs.get(id).name);
  return { doc, page, items: text.items, joined: text.items.map((i) => i.str).join(""), fonts };
}

for (const job of report.jobs) {
  const { doc, page, joined, fonts, items } = await load(job.name);
  const [, , w, h] = page.view;
  check(doc.numPages === 1, `${job.name}: single page`);
  check(Math.abs(w / PT_PER_MM - job.pageMm[0]) < 0.5 && Math.abs(h / PT_PER_MM - job.pageMm[1]) < 0.5,
    `${job.name}: page ${(w / PT_PER_MM).toFixed(1)}×${(h / PT_PER_MM).toFixed(1)} mm`);
  check(fonts.every((f) => ALLOWED_FONTS.test(f)), `${job.name}: only bundled fonts embedded (${fonts.join(", ")})`);
  if (job.cells) {
    // N-up: N distinct receipts, each inside its own cell of the A4 sheet.
    const cols = job.cells === 4 ? 2 : 1, cw = 210 / cols, ch = 297 / (job.cells / cols);
    for (let n = 0; n < job.cells; n++) {
      const id = `RC-1405-${String(184 + n).padStart(6, "0")}`;
      const it = items.find((i) => i.str.includes(id));
      const x = it && it.transform[4] / PT_PER_MM, top = it && (page.view[3] - it.transform[5]) / PT_PER_MM;
      const [c, r] = [cols - 1 - (n % cols), Math.floor(n / cols)]; // RTL fill order
      check(it && x >= c * cw && x <= (c + 1) * cw && top >= r * ch && top <= (r + 1) * ch,
        `${job.name}: ${id} inside cell ${n} (x=${x?.toFixed(1)}mm, top=${top?.toFixed(1)}mm)`);
      check(job.cellFit[n].fits, `${job.name}: receipt ${n} fits its cell (${job.cellFit[n].spareMm}mm spare)`);
    }
  } else {
    check(joined.includes("RC-1405-000123") && joined.includes("P-000123"), `${job.name}: LTR ids intact inside RTL text`);
  }
  const lang = /-ps|bottom-left/.test(job.name) ? "ps" : /-en/.test(job.name) ? "en" : "fa";
  if (lang === "ps") check(["ښ", "ټ", "ډ", "ې", "ړ"].every((c) => joined.includes(c)), `${job.name}: Pashto letters present`);
  if (lang !== "en" && !job.name.includes("latn")) check(/[۰-۹]/.test(joined), `${job.name}: Persian digits`);
  if (lang !== "en") check(joined.includes("؋"), `${job.name}: Afghani sign rendered with a real glyph`);
}

// Cut marks: visible guide lines exactly on the cell boundaries.
for (const job of report.jobs.filter((j) => j.cells)) {
  const { horizontal, vertical, blankRow } = job.cutInk;
  check(horizontal > 0.3 && blankRow < 0.05, `${job.name}: horizontal cut line at 148.5mm (${(horizontal * 100).toFixed(0)}% inked; blank row ${(blankRow * 100).toFixed(0)}%)`);
  if (job.cells === 4) check(vertical > 0.3, `${job.name}: vertical cut line at 105mm (${(vertical * 100).toFixed(0)}% inked)`);
}

// A4 compact: receipt must land where the clinic configured it.
{
  const { page, items } = await load("receipt-a4-compact-bottom-left");
  const id = items.find((i) => i.str.includes("RC-1405-000123"));
  const xMm = id.transform[4] / PT_PER_MM;
  const topMm = (page.view[3] - id.transform[5]) / PT_PER_MM;
  // Configured: top=170mm, right=126mm, width 72mm → box x ∈ [12, 84] mm.
  check(xMm > 12 && xMm < 84 && topMm > 170 && topMm < 200, `A4 compact offset honoured (id at x=${xMm.toFixed(1)}mm, top=${topMm.toFixed(1)}mm)`);
}

// Thermal raster sources must not exceed the head width.
for (const [name, dots] of [["receipt-80mm-fa", 576], ["receipt-80mm-ps", 576], ["receipt-58mm-ps", 384]]) {
  const png = fs.readFileSync(path.join(out, `${name}.png`));
  const width = png.readUInt32BE(16);
  check(width <= dots && width >= dots - 2, `${name}.png: ${width}px wide for a ${dots}-dot head`);
}

// In-app (foreignObject→canvas) raster must match the browser screenshot.
for (const [name, dots] of [["receipt-80mm-fa", 576], ["receipt-58mm-ps", 384]]) {
  const png = fs.readFileSync(path.join(out, `${name}.inapp.png`));
  const ref = fs.readFileSync(path.join(out, `${name}.png`));
  const [w, h, rh] = [png.readUInt32BE(16), png.readUInt32BE(20), ref.readUInt32BE(20)];
  check(w === dots && Math.abs(h - rh) <= 3, `${name}.inapp.png: ${w}×${h} (screenshot height ${rh})`);
  const { inkRatio, blockDiff } = report.jobs.find((j) => j.name === name).inappDiff;
  // Missing text or a fallback font moves the ink ratio far from 1.
  check(Math.abs(inkRatio - 1) < 0.08,
    `${name}.inapp.png matches screenshot (ink ratio ${inkRatio}, mean block diff ${(blockDiff * 100).toFixed(1)}%)`);
}

console.log(failures ? `\n${failures} check(s) failed` : "\nall checks passed");
process.exit(failures ? 1 : 0);
