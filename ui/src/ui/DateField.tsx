import { useMemo } from "react";
import type { CalendarSystem } from "../../../shared/ts/contract";
import { daysInMonth, fromCalendar, inCalendar, monthsInYear, todayIso } from "../lib/calendar";
import { digits, monthName } from "../lib/dates";
import { useI18n } from "../i18n";
import { Select } from "./Field";

/**
 * Date input in the clinic's calendar (Solar Hijri by default): day, month name and year
 * pickers, so a date is never typed in the wrong calendar. The value in and out is the
 * Gregorian ISO date the Core stores ("2026-10-03"); an impossible date such as 31 Mizan
 * is never emitted — the day snaps to the month's last day instead.
 */
export function DateField({ value, onChange, calendar, label, testId, yearsBefore = 1, yearsAfter = 3, disabled }: {
  value: string;
  onChange: (iso: string) => void;
  calendar: CalendarSystem;
  label: string;
  testId?: string;
  yearsBefore?: number;
  yearsAfter?: number;
  disabled?: boolean;
}) {
  const { lang } = useI18n();
  const current = inCalendar(value || todayIso(), calendar);
  const thisYear = inCalendar(todayIso(), calendar).year;
  const years = useMemo(() => {
    const from = Math.min(thisYear - yearsBefore, current.year);
    const to = Math.max(thisYear + yearsAfter, current.year);
    return Array.from({ length: to - from + 1 }, (_, i) => from + i);
  }, [thisYear, yearsBefore, yearsAfter, current.year]);
  const dayCount = daysInMonth(current.year, current.month, calendar);

  const emit = (year: number, month: number, day: number) => {
    const last = daysInMonth(year, month, calendar);
    onChange(fromCalendar(year, month, Math.min(day, last), calendar) ?? value);
  };

  return (
    <div className="datefield" role="group" aria-label={label} data-testid={testId}>
      <Select value={current.day} disabled={disabled} onChange={(e) => emit(current.year, current.month, Number(e.target.value))} aria-label={`${label}: ${lang === "en" ? "day" : "روز"}`} data-testid={testId && `${testId}-day`}>
        {Array.from({ length: dayCount }, (_, i) => i + 1).map((d) => <option key={d} value={d}>{digits(d, lang)}</option>)}
      </Select>
      <Select value={current.month} disabled={disabled} onChange={(e) => emit(current.year, Number(e.target.value), current.day)} aria-label={`${label}: ${lang === "en" ? "month" : "ماه"}`} data-testid={testId && `${testId}-month`}>
        {Array.from({ length: monthsInYear(current.year, calendar) }, (_, i) => i + 1).map((m) => <option key={m} value={m}>{monthName(calendar, m, lang)}</option>)}
      </Select>
      <Select value={current.year} disabled={disabled} onChange={(e) => emit(Number(e.target.value), current.month, current.day)} aria-label={`${label}: ${lang === "en" ? "year" : "سال"}`} data-testid={testId && `${testId}-year`}>
        {years.map((y) => <option key={y} value={y}>{digits(y, lang)}</option>)}
      </Select>
    </div>
  );
}
