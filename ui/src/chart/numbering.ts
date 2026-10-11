// Tooth numbers as the clinic shows them (ADR-14). The Core always stores FDI (ISO 3950); Universal and
// Palmer are display only.
import type { Language, ToothNumbering } from "../../../shared/ts/contract";
import { digits } from "../lib/dates";

/** Universal: 1–32 for adult teeth (upper right third molar = 1), A–T for baby teeth. */
export function universal(fdi: number): string {
  const q = Math.floor(fdi / 10);
  const n = fdi % 10;
  switch (q) {
    case 1: return String(9 - n);
    case 2: return String(8 + n);
    case 3: return String(25 - n);
    case 4: return String(24 + n);
    case 5: return String.fromCharCode(65 + 5 - n);
    case 6: return String.fromCharCode(65 + 4 + n);
    case 7: return String.fromCharCode(65 + 15 - n);
    default: return String.fromCharCode(65 + 14 + n);
  }
}

/** Palmer: the tooth's number in its quadrant (A–E for baby teeth) with the quadrant's corner mark. */
export function palmer(fdi: number): string {
  const q = Math.floor(fdi / 10);
  const n = fdi % 10;
  const mark = q >= 5 ? String.fromCharCode(64 + n) : String(n);
  switch (q) {
    case 1: case 5: return `${mark}┘`;
    case 2: case 6: return `└${mark}`;
    case 3: case 7: return `┌${mark}`;
    default: return `${mark}┐`;
  }
}

/** One tooth's label in the clinic's numbering, with the interface's digits. */
export function toothLabel(fdi: number, numbering: ToothNumbering, lang: Language): string {
  if (numbering === "universal") return digits(universal(fdi), lang);
  if (numbering === "palmer") return palmer(fdi);
  return digits(String(fdi), lang);
}

/** Several teeth: "۲۴–۲۶" for neighbours in one quadrant, else a list. */
export function teethLabel(teeth: number[], numbering: ToothNumbering, lang: Language): string {
  if (!teeth.length) return "";
  const sorted = [...teeth].sort((a, b) => a - b);
  const sep = lang === "en" ? ", " : "، ";
  const runs: number[][] = [];
  for (const t of sorted) {
    const last = runs[runs.length - 1];
    if (last && t === last[last.length - 1] + 1 && Math.floor(t / 10) === Math.floor(last[0] / 10)) last.push(t);
    else runs.push([t]);
  }
  return runs
    .map((r) => (r.length > 2 ? `${toothLabel(r[0], numbering, lang)}–${toothLabel(r[r.length - 1], numbering, lang)}` : r.map((t) => toothLabel(t, numbering, lang)).join(sep)))
    .join(sep);
}

/** Surfaces as the doctor reads them: "MOD" with I/F/P on the teeth they apply to. */
export function surfacesText(surfaces: string, fdi: number | undefined): string {
  if (!surfaces) return "";
  const front = fdi != null && [1, 2, 3].includes(fdi % 10);
  const upper = fdi != null && [1, 2, 5, 6].includes(Math.floor(fdi / 10));
  return [...surfaces]
    .map((c) => (c === "O" && front ? "I" : c === "B" && front ? "F" : c === "L" && upper ? "P" : c))
    .join("");
}
