// Regenerates docs/i18n/pashto-review.md from the UI translation files, so a
// non-technical Pashto speaker can review every UI string in one table
// without touching JSON or code. Re-run whenever fa/ps/en.json change:
//   node ui/scripts/export-pashto-review.mjs [previous ps.json] [version]
// With the previous release's ps.json (e.g. from `git show v0.5.0:ui/src/i18n/ps.json`), the strings
// new since then are listed first, so the reviewer can start with them. The Pashto of the clinic's
// starter lists of Phase 5B (tooth conditions, note templates, ready-made plans) follows.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.resolve(root, "..");
const load = (l) => JSON.parse(fs.readFileSync(path.join(root, `src/i18n/${l}.json`), "utf8"));
const [fa, ps, en] = ["fa", "ps", "en"].map(load);
const [baselineFile, version] = process.argv.slice(2);
const baseline = baselineFile ? JSON.parse(fs.readFileSync(baselineFile, "utf8")) : null;

const escape = (s) => String(s ?? "").replace(/\|/g, "\\|").replace(/\n/g, " ");
const row = (key) => `| \`${key}\` | ${escape(en[key])} | ${escape(fa[key])} | ${escape(ps[key])} | |`;
const head = `| کلید (فقط برای توسعه‌دهنده) | English (مرجع) | دری (مرجع — تأییدشده) | پښتو (نیازمند بازبینی) | یادداشت بازبینی |
|---|---|---|---|---|`;

const keys = Object.keys(fa);
const fresh = baseline ? keys.filter((k) => !(k in baseline) || baseline[k] !== ps[k]) : [];

// The clinic's starter lists (seeds) carry their own Pashto: names and the note templates' texts.
const seed = (f) => JSON.parse(fs.readFileSync(path.join(repoRoot, "core/seeds", f), "utf8"));
const seedRows = [];
for (const c of seed("tooth_conditions.json")) seedRows.push([`condition.${c.code}`, c.name]);
for (const p of seed("plan_templates.json")) seedRows.push([`plan_template.${p.code}`, p.name]);
for (const n of seed("note_templates.json")) {
  seedRows.push([`note_template.${n.code}`, n.name]);
  for (const field of Object.keys(n.body.fa)) {
    if (n.body.fa[field] || n.body.ps[field]) seedRows.push([`note_template.${n.code}.${field}`, { en: n.body.en[field], fa: n.body.fa[field], ps: n.body.ps[field] }]);
  }
}
const seedTable = seedRows.map(([key, t]) => `| \`${key}\` | ${escape(t.en)} | ${escape(t.fa)} | ${escape(t.ps)} | |`).join("\n");

const out = `# بازبینی متن‌های پشتو — رابط برنامه

> این فایل به‌صورت خودکار از فایل‌های ترجمه برنامه ساخته شده است
> (\`node ui/scripts/export-pashto-review.mjs\`). هر بار که متن‌های رابط
> برنامه تغییر کنند، دوباره ساخته می‌شود. راهنمای بازبینی: [README.md](README.md).

تعداد متن‌ها: ${keys.length}${fresh.length ? ` — تازه در ${version ?? "این نسخه"}: ${fresh.length}` : ""}
${fresh.length ? `
## ۱. متن‌های تازه ${version ?? "این نسخه"} (اول این‌ها را ببینید)

${head}
${fresh.map(row).join("\n")}
` : ""}
## ${fresh.length ? "۲. " : ""}فهرست‌های اولیه بالینی (وضعیت دندان، قالب یادداشت، پلان آماده)

این متن‌ها نسخه اولیه‌اند و کلینیک در «فهرست‌های بالینی» می‌تواند تغییرشان دهد. اصلاح پشتو در \`core/seeds/*.json\` اعمال می‌شود.

${head}
${seedTable}

## ${fresh.length ? "۳. " : ""}همه متن‌های رابط

${head}
${keys.map(row).join("\n")}
`;

fs.writeFileSync(path.join(repoRoot, "docs/i18n/pashto-review.md"), out);
console.log(`wrote docs/i18n/pashto-review.md (${keys.length} strings, ${fresh.length} new, ${seedRows.length} seed texts)`);
