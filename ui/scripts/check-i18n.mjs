// Fails when fa/ps/en translation files do not have exactly the same keys,
// or when the UI references a key that does not exist (DoD rule 4).
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const load = (l) => JSON.parse(fs.readFileSync(path.join(root, `src/i18n/${l}.json`), "utf8"));
const [fa, ps, en] = ["fa", "ps", "en"].map(load);
let bad = 0;
for (const [name, dict] of [["ps", ps], ["en", en]]) {
  for (const k of Object.keys(fa)) if (!(k in dict)) { console.error(`${name}: missing ${k}`); bad++; }
  for (const k of Object.keys(dict)) if (!(k in fa)) { console.error(`${name}: extra ${k}`); bad++; }
  for (const [k, v] of Object.entries(dict)) if (!String(v).trim()) { console.error(`${name}: empty ${k}`); bad++; }
}
// Static t("…") keys used in the source must exist.
const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => e.isDirectory() ? walk(path.join(d, e.name)) : [path.join(d, e.name)]);
for (const f of walk(path.join(root, "src")).filter((f) => f.endsWith(".tsx"))) {
  for (const m of fs.readFileSync(f, "utf8").matchAll(/\bt\("([a-zA-Z0-9_.]+)"\)/g)) {
    if (!(m[1] in fa)) { console.error(`${path.relative(root, f)}: unknown key ${m[1]}`); bad++; }
  }
}
// Every error code of the contract needs a message.
const contract = fs.readFileSync(path.join(root, "../shared/ts/contract.ts"), "utf8");
const codes = contract.match(/export type ErrorCode = ([^;]+);/)[1].match(/"([a-z_]+)"/g).map((s) => s.slice(1, -1));
for (const c of codes) if (!(`error.${c}` in fa)) { console.error(`missing error.${c}`); bad++; }
console.log(bad ? `${bad} i18n problem(s)` : `i18n ok: ${Object.keys(fa).length} keys × 3 languages, ${codes.length} error codes`);
process.exit(bad ? 1 : 0);
