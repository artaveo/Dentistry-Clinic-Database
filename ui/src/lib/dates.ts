// Display rules (ADR-07/08/09, ADR-21): stored UTC; shown in Kabul time,
// Solar Hijri (default) or Gregorian per clinic setting, with Afghan month
// names; digits per language.
import type { CalendarSystem, Language } from "../../../shared/ts/contract";

const SHAMSI_MONTHS: Record<Language, string[]> = {
  fa: ["حمل", "ثور", "جوزا", "سرطان", "اسد", "سنبله", "میزان", "عقرب", "قوس", "جدی", "دلو", "حوت"],
  ps: ["وری", "غویی", "غبرگولی", "چنگاښ", "زمری", "وږی", "تله", "لړم", "لیندۍ", "مرغومی", "سلواغه", "کب"],
  en: ["Hamal", "Sawr", "Jawza", "Saratan", "Asad", "Sunbula", "Mizan", "Aqrab", "Qaws", "Jadi", "Dalw", "Hut"],
};

const GREGORIAN_MONTHS: Record<Language, string[]> = {
  fa: ["جنوری", "فبروری", "مارچ", "اپریل", "می", "جون", "جولای", "اگست", "سپتمبر", "اکتوبر", "نومبر", "دسمبر"],
  ps: ["جنوري", "فبروري", "مارچ", "اپریل", "می", "جون", "جولای", "اګست", "سپتمبر", "اکتوبر", "نومبر", "دسمبر"],
  en: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
};

export function digits(s: string | number, lang: Language): string {
  return lang === "en" ? String(s) : String(s).replace(/[0-9]/g, (d) => "۰۱۲۳۴۵۶۷۸۹"[Number(d)]);
}

export function formatDateTime(
  iso: string | null | undefined,
  lang: Language,
  calendar: CalendarSystem = "shamsi",
): string {
  if (!iso) return "—";
  const months = calendar === "gregorian" ? GREGORIAN_MONTHS : SHAMSI_MONTHS;
  const ca = calendar === "gregorian" ? "gregory" : "persian";
  const parts = new Intl.DateTimeFormat(`en-u-ca-${ca}-nu-latn`, {
    timeZone: "Asia/Kabul", year: "numeric", month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hourCycle: "h23",
  }).formatToParts(new Date(iso));
  const g = (t: string) => parts.find((p) => p.type === t)?.value ?? "";
  return digits(`${g("day")} ${months[lang][Number(g("month")) - 1]} ${g("year")}، ${g("hour")}:${g("minute")}`, lang).replace("،", lang === "en" ? "," : "،");
}

export function formatBytes(n: number, lang: Language): string {
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  const n2 = digits(v.toFixed(i ? 1 : 0), lang);
  return `${lang === "en" ? n2 : n2.replace(".", "٫")} ${units[i]}`;
}
