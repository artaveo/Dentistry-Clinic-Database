// Spike 5: compares the Rust Core calendar (spikes/shamsi) day-by-day with
// ICU (what WebView2 uses) and the candidate JS libraries.
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as jalaali from "jalaali-js";
import { format as fnsFormat } from "date-fns-jalali";
import { CalendarDate, PersianCalendar, toCalendar } from "@internationalized/date";

const spikes = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const [from, to] = [Number(process.argv[2] ?? 1925), Number(process.argv[3] ?? 2125)];
const dump = execFileSync("cargo", ["run", "-q", "-p", "shamsi", "--example", "dump", "--", String(from), String(to)], {
  cwd: spikes, maxBuffer: 1 << 26, encoding: "utf8",
}).trim().split("\n");

const icu = new Intl.DateTimeFormat("en-u-ca-persian-nu-latn", { timeZone: "UTC", year: "numeric", month: "numeric", day: "numeric" });
const persian = new PersianCalendar();
const libs = {
  "ICU / Intl (WebView2)": (y, m, d) => {
    const p = icu.formatToParts(new Date(Date.UTC(y, m - 1, d)));
    const g = (t) => Number(p.find((x) => x.type === t).value);
    return [g("year"), g("month"), g("day")];
  },
  "jalaali-js": (y, m, d) => { const r = jalaali.toJalaali(y, m, d); return [r.jy, r.jm, r.jd]; },
  "date-fns-jalali": (y, m, d) => fnsFormat(new Date(y, m - 1, d, 12), "yyyy-M-d").split("-").map(Number),
  "@internationalized/date": (y, m, d) => { const r = toCalendar(new CalendarDate(y, m, d), persian); return [r.year, r.month, r.day]; },
};

const mismatches = Object.fromEntries(Object.keys(libs).map((k) => [k, []]));
for (const line of dump) {
  const [g, s] = line.split(",");
  const [y, m, d] = g.split("-").map(Number);
  const expected = s.split("-").map(Number);
  for (const [name, fn] of Object.entries(libs)) {
    const got = fn(y, m, d);
    if (got.join() !== expected.join()) mismatches[name].push(`${g}: rust ${s} vs ${got.join("-")}`);
  }
}

console.log(`Compared ${dump.length} days (${from}–${to} Gregorian)`);
let bad = false;
for (const [name, list] of Object.entries(mismatches)) {
  console.log(`${list.length ? "DIFF" : "SAME"}  ${name}: ${list.length} mismatching days${list.length ? "  e.g. " + list.slice(0, 3).join(" | ") : ""}`);
  if (name.startsWith("ICU") && list.some((l) => { const yr = Number(l.slice(0, 4)); return yr >= 1950 && yr <= 2100; })) bad = true;
}

console.log("\nICU month names (why we ship our own tables):");
const d = new Date(Date.UTC(2026, 9, 1));
for (const l of ["fa-AF", "ps-AF", "en-AF", "fa-IR"]) {
  console.log(`  ${l.padEnd(6)} ${new Intl.DateTimeFormat(`${l}-u-ca-persian`, { month: "long", timeZone: "UTC" }).format(d)}`);
}
process.exit(bad ? 1 : 0);
