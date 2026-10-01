// Shared display helpers (prototype of the UI's date/number layer, ADR-07/08/09).
// Calendar fields come from ICU (Intl, persian calendar) — available in
// WebView2/Chromium and Node — but month names come from our own tables:
// ICU's English names are Iranian (Mehr) and its Pashto "full" style
// contains a stray "AP" era; Afghanistan uses Hamal…Hut / وری…کب.
export const MONTHS = {
  fa: ["حمل", "ثور", "جوزا", "سرطان", "اسد", "سنبله", "میزان", "عقرب", "قوس", "جدی", "دلو", "حوت"],
  ps: ["وری", "غویی", "غبرگولی", "چنگاښ", "زمری", "وږی", "تله", "لړم", "لیندۍ", "مرغومی", "سلواغه", "کب"],
  en: ["Hamal", "Sawr", "Jawza", "Saratan", "Asad", "Sunbula", "Mizan", "Aqrab", "Qaws", "Jadi", "Dalw", "Hut"],
};

export function shamsiParts(date, timeZone = "Asia/Kabul") {
  const parts = new Intl.DateTimeFormat("en-u-ca-persian-nu-latn", {
    timeZone, year: "numeric", month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hourCycle: "h23",
  }).formatToParts(date);
  const get = (t) => Number(parts.find((p) => p.type === t).value);
  return { year: get("year"), month: get("month"), day: get("day"), hour: get("hour"), minute: get("minute") };
}

export function toDigits(s, digits) {
  return digits === "latn" ? String(s) : String(s).replace(/[0-9]/g, (d) => "۰۱۲۳۴۵۶۷۸۹"[d]);
}

export function formatShamsi(date, lang, digits) {
  const p = shamsiParts(date);
  const two = (n) => String(n).padStart(2, "0");
  return toDigits(`${p.day} ${MONTHS[lang][p.month - 1]} ${p.year} — ${two(p.hour)}:${two(p.minute)}`, digits);
}

/** Money is stored as integer minor units (ADR-06); AFN is shown without decimals. */
export function formatAfn(minor, lang, digits) {
  const n = new Intl.NumberFormat(lang === "en" ? "en" : "fa-AF", { maximumFractionDigits: 0, numberingSystem: digits })
    .format(Math.round(minor / 100));
  return lang === "en" ? `${n} AFN` : `${n} ؋`;
}
