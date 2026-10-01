// In-app rasteriser for thermal printing (ADR-13, ESC/POS path).
// Renders a DOM element to a PNG of exactly `dots` pixels wide using the
// SVG <foreignObject> → <canvas> technique, i.e. with WebView2's own text
// shaping. The PNG is handed to the Rust Core, which dithers and sends it RAW.

const fontCache = new Map();

async function inlineFonts(css) {
  const urls = [...css.matchAll(/url\("([^"]+\.woff2)"\)/g)].map((m) => m[1]);
  for (const u of urls) {
    if (!fontCache.has(u)) {
      const buf = await (await fetch(u)).arrayBuffer();
      let bin = "";
      new Uint8Array(buf).forEach((b) => (bin += String.fromCharCode(b)));
      fontCache.set(u, `data:font/woff2;base64,${btoa(bin)}`);
    }
    css = css.replaceAll(`url("${u}")`, `url("${fontCache.get(u)}")`);
  }
  return css;
}

/** @returns {Promise<{png: Blob, width: number, height: number}>} */
export async function rasterizeElement(el, dots) {
  await document.fonts.ready;
  const rect = el.getBoundingClientRect();
  // Same-origin <link> and <style> sheets alike; @font-face urls are made absolute
  // against the sheet that declared them, then inlined as data URLs.
  const cssText = [...document.styleSheets]
    .flatMap((sheet) => [...sheet.cssRules].map((r) => r.cssText.replace(/url\("([^"]+)"\)/g, (_, u) => `url("${new URL(u, sheet.href || document.baseURI).href}")`)))
    .join("\n");
  const css = await inlineFonts(cssText);
  const root = document.documentElement;
  // There is no <body> inside <foreignObject>: carry over inherited text styles.
  const b = getComputedStyle(document.body);
  const inherited = ["font-family", "font-size", "line-height", "color"].map((k) => `${k}:${b.getPropertyValue(k)}`).join(";");
  const vars = (root.getAttribute("style") || "").replaceAll('"', "'");
  const xhtml = new XMLSerializer().serializeToString(el);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${rect.width}" height="${rect.height}">
    <foreignObject width="100%" height="100%">
      <div xmlns="http://www.w3.org/1999/xhtml" dir="${root.dir}" lang="${root.lang}" style="${vars};${inherited.replaceAll('"', "'")}">
        <style>${css}</style>${xhtml}
      </div>
    </foreignObject></svg>`;
  const img = new Image();
  img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  await img.decode();
  const scale = dots / rect.width;
  const canvas = document.createElement("canvas");
  canvas.width = dots;
  canvas.height = Math.ceil(rect.height * scale);
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  // Fonts inside an SVG image load asynchronously *after* decode(), and text
  // stays invisible until they do. Redraw until the ink has been stable for
  // 4 consecutive frames (~200 ms); embedded data-URL fonts load well within that.
  let prev = -1;
  let stable = 0;
  for (let i = 0; i < 40; i++) {
    ctx.fillStyle = "#fff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
    const d = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    let ink = 0;
    for (let p = 0; p < d.length; p += 16) ink += d[p] < 128;
    stable = ink === prev ? stable + 1 : 0;
    if (stable >= 4) break;
    prev = ink;
    await new Promise((r) => setTimeout(r, 50));
  }
  const png = await new Promise((ok) => canvas.toBlob(ok, "image/png"));
  return { png, width: canvas.width, height: canvas.height };
}

export async function blobToBase64(blob) {
  const buf = new Uint8Array(await blob.arrayBuffer());
  let bin = "";
  for (let i = 0; i < buf.length; i += 0x8000) bin += String.fromCharCode(...buf.subarray(i, i + 0x8000));
  return btoa(bin);
}
