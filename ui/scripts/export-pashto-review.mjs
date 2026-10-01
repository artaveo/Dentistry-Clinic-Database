// Regenerates docs/i18n/pashto-review.md from the UI translation files, so a
// non-technical Pashto speaker can review every UI string in one table
// without touching JSON or code. Re-run whenever fa/ps/en.json change:
//   node ui/scripts/export-pashto-review.mjs
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.resolve(root, "..");
const load = (l) => JSON.parse(fs.readFileSync(path.join(root, `src/i18n/${l}.json`), "utf8"));
const [fa, ps, en] = ["fa", "ps", "en"].map(load);

const escape = (s) => String(s).replace(/\|/g, "\\|").replace(/\n/g, " ");

const rows = Object.keys(fa)
  .map((key) => `| \`${key}\` | ${escape(en[key])} | ${escape(fa[key])} | ${escape(ps[key])} | |`)
  .join("\n");

const out = `# بازبینی متن‌های پشتو — رابط برنامه

> این فایل به‌صورت خودکار از فایل‌های ترجمه برنامه ساخته شده است
> (\`node ui/scripts/export-pashto-review.mjs\`). هر بار که متن‌های رابط
> برنامه تغییر کنند، دوباره ساخته می‌شود. راهنمای بازبینی: [README.md](README.md).

تعداد متن‌ها: ${Object.keys(fa).length}

| کلید (فقط برای توسعه‌دهنده) | English (مرجع) | دری (مرجع — تأییدشده) | پښتو (نیازمند بازبینی) | یادداشت بازبینی |
|---|---|---|---|---|
${rows}
`;

fs.writeFileSync(path.join(repoRoot, "docs/i18n/pashto-review.md"), out);
console.log(`wrote docs/i18n/pashto-review.md (${Object.keys(fa).length} strings)`);
