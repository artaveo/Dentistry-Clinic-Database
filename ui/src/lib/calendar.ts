// Calendar arithmetic for the appointment screens (ADR-21, Phase 4). Dates
// travel as clinic-local ISO strings ("2026-10-03", always Gregorian, like the
// Core stores them); the clinic's calendar (Solar Hijri by default) only
// decides how months and years are cut and named. The Afghan week starts on
// Saturday (day 0) and ends on Friday (day 6).
import { CalendarDate, GregorianCalendar, PersianCalendar, toCalendar } from "@internationalized/date";
import type { CalendarSystem, Language } from "../../../shared/ts/contract";
import { digits, latinDigits } from "./dates";

export type Iso = string;

const GREGORIAN = new GregorianCalendar();
const PERSIAN = new PersianCalendar();
const calendarOf = (c: CalendarSystem) => (c === "gregorian" ? GREGORIAN : PERSIAN);

function parse(iso: Iso): CalendarDate {
  const [y, m, d] = iso.split("-").map(Number);
  return new CalendarDate(y, m, d);
}

function format(d: CalendarDate): Iso {
  const g = toCalendar(d, GREGORIAN);
  return `${String(g.year).padStart(4, "0")}-${String(g.month).padStart(2, "0")}-${String(g.day).padStart(2, "0")}`;
}

/** Today's date at the clinic (Kabul time), whatever the computer's own zone is. */
export function todayIso(now: Date = new Date()): Iso {
  return new Intl.DateTimeFormat("en-CA", { timeZone: "Asia/Kabul", year: "numeric", month: "2-digit", day: "2-digit" }).format(now);
}

export function addDays(iso: Iso, n: number): Iso {
  return format(parse(iso).add({ days: n }));
}

export function daysBetween(from: Iso, to: Iso): number {
  return Math.round((Date.UTC(...ymd(to)) - Date.UTC(...ymd(from))) / 86_400_000);
}

function ymd(iso: Iso): [number, number, number] {
  const [y, m, d] = iso.split("-").map(Number);
  return [y, m - 1, d];
}

/** 0 = Saturday … 6 = Friday. */
export function weekdayIndex(iso: Iso): number {
  const [y, m, d] = ymd(iso);
  const sunday0 = new Date(Date.UTC(y, m, d)).getUTCDay();
  return (sunday0 + 1) % 7;
}

/** The Saturday on or before `iso`. */
export function weekStart(iso: Iso): Iso {
  return addDays(iso, -weekdayIndex(iso));
}

export function weekDays(iso: Iso): Iso[] {
  const start = weekStart(iso);
  return Array.from({ length: 7 }, (_, i) => addDays(start, i));
}

export type Ymd = { year: number; month: number; day: number };

/** The date as year/month/day in the clinic's calendar. */
export function inCalendar(iso: Iso, calendar: CalendarSystem): Ymd {
  const c = toCalendar(parse(iso), calendarOf(calendar));
  return { year: c.year, month: c.month, day: c.day };
}

export function daysInMonth(year: number, month: number, calendar: CalendarSystem): number {
  return new CalendarDate(calendarOf(calendar), year, month, 1).calendar.getDaysInMonth(new CalendarDate(calendarOf(calendar), year, month, 1));
}

export function monthsInYear(year: number, calendar: CalendarSystem): number {
  const d = new CalendarDate(calendarOf(calendar), year, 1, 1);
  return d.calendar.getMonthsInYear(d);
}

/** ISO date of a year/month/day in the clinic's calendar, or `null` if no such day exists. */
export function fromCalendar(year: number, month: number, day: number, calendar: CalendarSystem): Iso | null {
  if (!Number.isInteger(year) || !Number.isInteger(month) || !Number.isInteger(day)) return null;
  if (month < 1 || month > monthsInYear(year, calendar) || day < 1 || day > daysInMonth(year, month, calendar)) return null;
  return format(new CalendarDate(calendarOf(calendar), year, month, day));
}

/** First and last day (ISO) of the month containing `iso`, in the clinic's calendar. */
export function monthRange(iso: Iso, calendar: CalendarSystem): { from: Iso; to: Iso; year: number; month: number } {
  const { year, month } = inCalendar(iso, calendar);
  const from = fromCalendar(year, month, 1, calendar)!;
  const to = fromCalendar(year, month, daysInMonth(year, month, calendar), calendar)!;
  return { from, to, year, month };
}

/** A date in the previous/next month (same day where possible, otherwise the month's last day). */
export function shiftMonth(iso: Iso, calendar: CalendarSystem, n: number): Iso {
  const c = toCalendar(parse(iso), calendarOf(calendar)).add({ months: n });
  return format(c);
}

/** Saturday-first weeks that cover the whole month; days outside it are included so rows are full. */
export function monthWeeks(iso: Iso, calendar: CalendarSystem): Iso[][] {
  const { from, to } = monthRange(iso, calendar);
  const weeks: Iso[][] = [];
  for (let start = weekStart(from); start <= to; start = addDays(start, 7)) weeks.push(weekDays(start));
  return weeks;
}

export function isInMonth(iso: Iso, anchor: Iso, calendar: CalendarSystem): boolean {
  const a = inCalendar(iso, calendar);
  const b = inCalendar(anchor, calendar);
  return a.year === b.year && a.month === b.month;
}

// ───────────────────────────── times of day ─────────────────────────────

/** "HH:MM" (24-hour, as the Core stores it) → minutes after midnight. */
export function toMinutes(hhmm: string): number {
  const [h, m] = hhmm.split(":").map(Number);
  return h * 60 + m;
}

export function fromMinutes(min: number): string {
  return `${String(Math.floor(min / 60)).padStart(2, "0")}:${String(min % 60).padStart(2, "0")}`;
}

/** Clinic-local clock now as minutes after midnight. */
export function nowMinutes(now: Date = new Date()): number {
  const p = new Intl.DateTimeFormat("en-GB", { timeZone: "Asia/Kabul", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).formatToParts(now);
  const get = (t: string) => Number(p.find((x) => x.type === t)?.value ?? "0");
  return (get("hour") % 24) * 60 + get("minute");
}

// ───────────────────────── typed dates and range presets ─────────────────────────

const pad2 = (n: number) => String(n).padStart(2, "0");

/** First and last day (ISO) of the year containing `iso`, in the clinic's calendar. */
export function yearRange(iso: Iso, calendar: CalendarSystem): { from: Iso; to: Iso; year: number } {
  const { year } = inCalendar(iso, calendar);
  const last = monthsInYear(year, calendar);
  return { from: fromCalendar(year, 1, 1, calendar)!, to: fromCalendar(year, last, daysInMonth(year, last, calendar), calendar)!, year };
}

/** "1405/08/20" (digits per language) for the typed date field; "" for an empty or malformed ISO date. */
export function formatTypedDate(iso: Iso, calendar: CalendarSystem, lang: Language): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(iso)) return "";
  const { year, month, day } = inCalendar(iso, calendar);
  return digits(`${String(year).padStart(4, "0")}/${pad2(month)}/${pad2(day)}`, lang);
}

/**
 * Reads typed text such as "1405/08/20", "۱۴۰۵-۸-۲۰" or "14050820" in the clinic's calendar
 * (Persian, Arabic-Indic and ASCII digits; any non-digit separator). `null` if it is not a real day.
 */
export function parseTypedDate(text: string, calendar: CalendarSystem): Iso | null {
  const s = latinDigits(text).replace(/[\u200c\u200e\u200f]/g, "").trim();
  const m = /^(\d{4})\D+(\d{1,2})\D+(\d{1,2})$/.exec(s) ?? /^(\d{4})(\d{2})(\d{2})$/.exec(s);
  if (!m) return null;
  const year = Number(m[1]);
  if (year < 1) return null;
  return fromCalendar(year, Number(m[2]), Number(m[3]), calendar);
}

/**
 * Live mask for the typed date: keeps digits only, puts the "/" in by itself (year 4, month 2, day 2),
 * pads a one-digit month the moment a separator is typed after it, and shows the digits of `lang`.
 */
export function maskTypedDate(raw: string, lang: Language): string {
  const lim = [4, 2, 2];
  const v = ["", "", ""];
  let f = 0;
  for (const ch of latinDigits(raw)) {
    if (ch >= "0" && ch <= "9") {
      if (v[f].length >= lim[f]) {
        if (f === 2) break;
        f++;
      }
      v[f] += ch;
    } else if (v[f].length > 0 && f < 2) {
      if (f > 0 && v[f].length === 1) v[f] = "0" + v[f];
      f++;
    }
  }
  let out = v[0];
  for (let i = 1; i <= f; i++) out += "/" + v[i];
  return digits(out, lang);
}

export type RangePresetId = "today" | "last7" | "thisMonth" | "lastMonth" | "thisYear";

/** The inclusive ISO range of a quick-pick ("last7" = today and the 6 days before). */
export function presetRange(id: RangePresetId, calendar: CalendarSystem, today: Iso = todayIso()): { from: Iso; to: Iso } {
  switch (id) {
    case "today":
      return { from: today, to: today };
    case "last7":
      return { from: addDays(today, -6), to: today };
    case "thisMonth": {
      const r = monthRange(today, calendar);
      return { from: r.from, to: r.to };
    }
    case "lastMonth": {
      const r = monthRange(shiftMonth(monthRange(today, calendar).from, calendar, -1), calendar);
      return { from: r.from, to: r.to };
    }
    case "thisYear": {
      const r = yearRange(today, calendar);
      return { from: r.from, to: r.to };
    }
  }
}
