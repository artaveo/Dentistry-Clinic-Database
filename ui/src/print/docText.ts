// Words on a printed document are in the document's language (the patient's), never necessarily the
// screen's: a Pashto-speaking patient gets a Pashto prescription from a Dari interface.
import type { Language, RxItem } from "../../../shared/ts/contract";
import { digits } from "../lib/dates";
import { translate } from "../i18n";

export function docT(lang: Language) {
  return (key: string) => translate(lang, key);
}

/** "۱۵ عدد" / "15" with the language's digits. */
export function num(v: string | number, lang: Language): string {
  return digits(String(v), lang);
}

const SEP: Record<Language, string> = { fa: "، ", ps: "، ", en: ", " };

/**
 * The instructions of one prescription line as a sentence in the patient's language:
 * «روزانه ۳ بار، هر بار ۱ کپسول، بعد از غذا، ۵ روز».
 */
export function rxInstructions(i: RxItem, lang: Language): string {
  const t = docT(lang);
  const parts: string[] = [];
  if (i.times_per_day) {
    const times = num(i.times_per_day, lang);
    parts.push((i.as_needed ? t("rx.sentence.asNeededUpTo") : t("rx.sentence.timesPerDay")).replace("{n}", times));
  } else if (i.as_needed) {
    parts.push(t("rx.sentence.asNeeded"));
  }
  if (i.dose) {
    const unit = /^[\d.½¼¾۰-۹٫]+$/.test(i.dose.trim()) && (i.form === "tablet" || i.form === "capsule") ? ` ${t(`rx.unit.${i.form}`)}` : "";
    parts.push(t("rx.sentence.each").replace("{dose}", `${num(i.dose.trim(), lang)}${unit}`));
  }
  if (i.timing) parts.push(t(`rx.timing.${i.timing}`));
  if (i.days) parts.push(t("rx.sentence.days").replace("{n}", num(i.days, lang)));
  return parts.join(SEP[lang]);
}

/** Fills a template's placeholders when a consent form or instruction sheet is written. */
export function fillTemplate(text: string, values: Record<string, string>): string {
  return text.replace(/\{(patient|doctor|clinic|date|procedure|teeth)\}/g, (_m, k: string) => (values[k] ?? "").trim() || "……");
}
