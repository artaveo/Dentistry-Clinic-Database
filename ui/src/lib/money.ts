// Money display (roadmap 2.3, ADR-06): amounts are always integers in the
// smallest unit (×100) in Core/storage; this only formats them for display.
// No monetary feature calls this yet (Billing is Phase 6) — it is tested and
// ready for reuse once one does.
import type { Language } from "../../../shared/ts/contract";
import { digits } from "./dates";

/** `minorUnits` is AFN × 100 (ADR-06). `2,500 AFN` / `۲٬۵۰۰ افغانی`. */
export function formatAmount(minorUnits: number, lang: Language): string {
  const whole = Math.round(minorUnits) / 100;
  const grouped = whole.toLocaleString("en-US", { maximumFractionDigits: 2, minimumFractionDigits: whole % 1 === 0 ? 0 : 2 });
  if (lang === "en") return `${grouped} AFN`;
  const localized = digits(grouped, lang).replace(/,/g, "٬").replace(".", "٫");
  return `${localized} افغانی`;
}
