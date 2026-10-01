// Tiny static server: Chromium refuses ES modules from file:// URLs.
import http from "node:http";
import fs from "node:fs";
import path from "node:path";

const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".woff2": "font/woff2", ".css": "text/css" };

export function serve(root) {
  const server = http.createServer((req, res) => {
    const p = path.join(root, decodeURIComponent(new URL(req.url, "http://x").pathname));
    if (!p.startsWith(root) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": types[path.extname(p)] || "application/octet-stream" });
    fs.createReadStream(p).pipe(res);
  });
  return new Promise((ok) => server.listen(0, "127.0.0.1", () => ok({ server, base: `http://127.0.0.1:${server.address().port}` })));
}
