// Display rules (ADR-07/08/09, ADR-21): stored UTC; shown in Kabul time,
// Solar Hijri (default) or Gregorian per clinic setting, with Afghan month
// names; digits per language. Every clock in the product is 12-hour with
// ق.ظ/ب.ظ (Dari), غ.م/غ.و (Pashto), AM/PM (English) — OF-007.
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

/** Month name (1–12) in the clinic's calendar and the UI language. */
export function monthName(calendar: CalendarSystem, month: number, lang: Language): string {
  return (calendar === "gregorian" ? GREGORIAN_MONTHS : SHAMSI_MONTHS)[lang][month - 1];
}

/** Morning / afternoon markers (owner decision, ADR-07). */
export const PERIODS: Record<Language, { am: string; pm: string }> = {
  fa: { am: "ق.ظ", pm: "ب.ظ" },
  ps: { am: "غ.م", pm: "غ.و" },
  en: { am: "AM", pm: "PM" },
};

const AT: Record<Language, string> = { fa: "ساعت ", ps: "ساعت ", en: "" };

export function digits(s: string | number, lang: Language): string {
  return lang === "en" ? String(s) : String(s).replace(/[0-9]/g, (d) => "۰۱۲۳۴۵۶۷۸۹"[Number(d)]);
}

/** Accepts Persian/Arabic-Indic digits too (ADR-09: input takes both). */
export function latinDigits(s: string): string {
  return s.replace(/[۰-۹]/g, (d) => String("۰۱۲۳۴۵۶۷۸۹".indexOf(d))).replace(/[٠-٩]/g, (d) => String("٠١٢٣٤٥٦٧٨٩".indexOf(d)));
}

/** 24-hour (0–23) → 12-hour parts. */
export function to12(hour24: number): { hour: number; pm: boolean } {
  const pm = hour24 >= 12;
  const h = hour24 % 12;
  return { hour: h === 0 ? 12 : h, pm };
}

/** 12-hour parts → 24-hour (0–23). */
export function to24(hour12: number, pm: boolean): number {
  return (hour12 % 12) + (pm ? 12 : 0);
}

/** "1:14 ق.ظ" / "1:14 AM" from 24-hour hour and minute. */
export function formatClock(hour24: number, minute: number, lang: Language): string {
  const { hour, pm } = to12(hour24);
  const p = PERIODS[lang];
  return digits(`${hour}:${String(minute).padStart(2, "0")}`, lang) + " " + (pm ? p.pm : p.am);
}

/** "HH:MM" (stored 24-hour) → "8:00 ق.ظ". */
export function formatTime(hhmm: string | null | undefined, lang: Language): string {
  if (!hhmm) return "—";
  const [h, m] = hhmm.split(":").map(Number);
  return formatClock(h, m, lang);
}

/** A whole hour (0–23), e.g. the daily backup hour → "7:00 ب.ظ". */
export function formatHour(hour24: number, lang: Language): string {
  return formatClock(hour24, 0, lang);
}

function parts(iso: string, calendar: CalendarSystem) {
  const ca = calendar === "gregorian" ? "gregory" : "persian";
  const p = new Intl.DateTimeFormat(`en-u-ca-${ca}-nu-latn`, {
    timeZone: "Asia/Kabul", year: "numeric", month: "numeric", day: "numeric", hour: "numeric", minute: "2-digit", hourCycle: "h23",
  }).formatToParts(new Date(iso));
  const g = (t: string) => Number(p.find((x) => x.type === t)?.value ?? "0");
  return { year: g("year"), month: g("month"), day: g("day"), hour: g("hour") % 24, minute: g("minute") };
}

/** "۱۰ میزان ۱۴۰۵" / "10 Mizan 1405". */
export function formatDate(iso: string | null | undefined, lang: Language, calendar: CalendarSystem = "shamsi"): string {
  if (!iso) return "—";
  const d = parts(iso, calendar);
  const months = calendar === "gregorian" ? GREGORIAN_MONTHS : SHAMSI_MONTHS;
  return digits(`${d.day} ${months[lang][d.month - 1]} ${d.year}`, lang);
}

/** One natural phrase (OF-006): "۱۰ میزان ۱۴۰۵، ساعت ۱:۱۴ ق.ظ" / "10 Mizan 1405, 1:14 AM". */
export function formatDateTime(iso: string | null | undefined, lang: Language, calendar: CalendarSystem = "shamsi"): string {
  if (!iso) return "—";
  const d = parts(iso, calendar);
  return `${formatDate(iso, lang, calendar)}${lang === "en" ? "," : "،"} ${AT[lang]}${formatClock(d.hour, d.minute, lang)}`;
}

/** Time of day only, 12-hour, in Kabul time. */
export function formatTimeOf(iso: string | null | undefined, lang: Language): string {
  if (!iso) return "—";
  const d = parts(iso, "gregorian");
  return formatClock(d.hour, d.minute, lang);
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
