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

const ROMAN = ["I", "II", "III", "IV", "V"];

/** The tooth's place in its quadrant, counted from the midline: 1–8, baby teeth A–E (Palmer) or I–V (Zsigmondy). */
function inQuadrant(fdi: number, roman: boolean): string {
  const n = fdi % 10;
  if (fdi < 50) return String(n);
  return roman ? ROMAN[n - 1] : String.fromCharCode(64 + n);
}

/** The quadrant's corner mark around a tooth's place, as dentists write it: 6┘ └6 ┌6 6┐. */
function cornered(fdi: number, mark: string): string {
  switch (Math.floor(fdi / 10)) {
    case 1: case 5: return `${mark}┘`;
    case 2: case 6: return `└${mark}`;
    case 3: case 7: return `┌${mark}`;
    default: return `${mark}┐`;
  }
}

/** Palmer: the tooth's number in its quadrant (A–E for baby teeth) with the quadrant's corner mark. */
export function palmer(fdi: number): string {
  return cornered(fdi, inQuadrant(fdi, false));
}

/** Zsigmondy: 1–8 in each quadrant, baby teeth in Roman numerals I–V, with the quadrant's corner mark. */
export function quadrant(fdi: number): string {
  return cornered(fdi, inQuadrant(fdi, true));
}

/**
 * The corner mark must stay on its side of the number (6┘ is upper right, └6 upper left), also inside
 * right-to-left text: the label is isolated as left-to-right (LRI … PDI, invisible).
 */
const ltr = (s: string) => `⁦${s}⁩`;

/** One tooth's label in the clinic's numbering, with the interface's digits. */
export function toothLabel(fdi: number, numbering: ToothNumbering, lang: Language): string {
  if (numbering === "universal") return digits(universal(fdi), lang);
  if (numbering === "palmer") return ltr(digits(palmer(fdi), lang));
  if (numbering === "quadrant") return ltr(digits(quadrant(fdi), lang));
  return digits(String(fdi), lang);
}

/**
 * The number printed over a tooth on the chart. Its position already says the quadrant, so the
 * quadrant systems drop the corner mark: 8 … 1 | 1 … 8 on each jaw.
 */
export function chartNumber(fdi: number, numbering: ToothNumbering, lang: Language): string {
  if (numbering === "palmer" || numbering === "quadrant") return digits(inQuadrant(fdi, numbering === "quadrant"), lang);
  return toothLabel(fdi, numbering, lang);
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
  const range = (a: number, b: number) =>
    numbering === "palmer" || numbering === "quadrant"
      ? ltr(digits(cornered(a, `${inQuadrant(a, numbering === "quadrant")}–${inQuadrant(b, numbering === "quadrant")}`), lang))
      : `${toothLabel(a, numbering, lang)}–${toothLabel(b, numbering, lang)}`;
  return runs
    .map((r) => (r.length > 2 ? range(r[0], r[r.length - 1]) : r.map((t) => toothLabel(t, numbering, lang)).join(sep)))
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
