import { forwardRef, useEffect, useId, useRef, useState } from "react";
import type { KeyboardEventHandler, ReactNode } from "react";
import { CalendarDays, ChevronLeft, ChevronRight, CircleAlert } from "lucide-react";
import type { CalendarSystem } from "../../../shared/ts/contract";
import { formatTypedDate, fromCalendar, inCalendar, isInMonth, maskTypedDate, monthRange, monthWeeks, parseTypedDate, shiftMonth, todayIso, weekStart, addDays } from "../lib/calendar";
import type { Iso } from "../lib/calendar";
import { WEEKDAYS_SHORT, digits, formatDate, monthName } from "../lib/dates";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { TextInput } from "./Field";
import { FloatingPanel } from "./Overlay";
import type { FloatingCloseReason } from "./Overlay";

// ───────────────────────────── calendar grid (shared with DateRangeField) ─────────────────────────────

export type CalendarGridProps = {
  calendar: CalendarSystem;
  /** Any ISO date inside the month to show first ("" = today's month). */
  initial?: Iso;
  /** The picked day of a single-date calendar. */
  selected?: Iso | null;
  /** A range to paint; `to` may be null while the second end is still being picked. */
  range?: { from: Iso | null; to: Iso | null } | null;
  /** The day under the pointer while a range is half-picked (paints the preview). */
  preview?: Iso | null;
  onHover?: (iso: Iso | null) => void;
  onPick: (iso: Iso) => void;
  /** Days before `min` / after `max` cannot be picked. */
  min?: Iso;
  max?: Iso;
  autoFocus?: boolean;
  testId?: string;
};

type View = "days" | "months" | "years";

/**
 * Month calendar in the clinic's calendar, Saturday first. Click the title for the months, then the
 * year for a page of years. Keyboard: arrows move a day / a week, PageUp/PageDown a month, Home/End
 * the week's ends, Enter or Space picks.
 */
export function CalendarGrid({ calendar, initial, selected, range, preview, onHover, onPick, min, max, autoFocus, testId }: CalendarGridProps) {
  const { lang, t } = useI18n();
  const today = todayIso();
  const start = initial || selected || today;
  const [view, setView] = useState<View>("days");
  const [cursor, setCursor] = useState<Iso>(() => monthRange(start, calendar).from);
  const [focusIso, setFocusIso] = useState<Iso>(start);
  const gridRef = useRef<HTMLDivElement>(null);
  const moveFocus = useRef(false);
  const cur = inCalendar(cursor, calendar);
  const nowYmd = inCalendar(today, calendar);
  const tabIso = isInMonth(focusIso, cursor, calendar) ? focusIso : cursor;

  useEffect(() => {
    if (!autoFocus) return;
    gridRef.current?.querySelector<HTMLElement>('[data-iso][tabindex="0"]')?.focus();
  }, []);

  useEffect(() => {
    if (!moveFocus.current) return;
    moveFocus.current = false;
    gridRef.current?.querySelector<HTMLElement>(`[data-iso="${focusIso}"]`)?.focus();
  });

  const shift = (dir: 1 | -1) => setCursor((c) => shiftMonth(c, calendar, dir * (view === "days" ? 1 : view === "months" ? 12 : 144)));
  const yearBase = Math.floor(cur.year / 12) * 12;
  const title =
    view === "days" ? `${monthName(calendar, cur.month, lang)} ${digits(cur.year, lang)}` : view === "months" ? digits(cur.year, lang) : `${digits(yearBase, lang)} – ${digits(yearBase + 11, lang)}`;
  const onTitle = () => setView((v) => (v === "days" ? "months" : v === "months" ? "years" : "days"));

  const onKeyDown: KeyboardEventHandler<HTMLDivElement> = (e) => {
    if (view !== "days") return;
    const forward = lang === "en" ? "ArrowRight" : "ArrowLeft"; // later days sit on the left in RTL
    let next: Iso | null = null;
    if (e.key === forward) next = addDays(focusIso, 1);
    else if (e.key === (forward === "ArrowRight" ? "ArrowLeft" : "ArrowRight")) next = addDays(focusIso, -1);
    else if (e.key === "ArrowDown") next = addDays(focusIso, 7);
    else if (e.key === "ArrowUp") next = addDays(focusIso, -7);
    else if (e.key === "PageDown") next = shiftMonth(focusIso, calendar, 1);
    else if (e.key === "PageUp") next = shiftMonth(focusIso, calendar, -1);
    else if (e.key === "Home") next = weekStart(focusIso);
    else if (e.key === "End") next = addDays(weekStart(focusIso), 6);
    if (!next) return;
    e.preventDefault();
    setFocusIso(next);
    if (!isInMonth(next, cursor, calendar)) setCursor(monthRange(next, calendar).from);
    moveFocus.current = true;
  };

  // The painted range: both ends, or one end and the day under the pointer.
  const rangeEnds = range && range.from ? { a: range.from, b: range.to ?? preview ?? null } : null;
  const lo = rangeEnds && rangeEnds.b ? (rangeEnds.a < rangeEnds.b ? rangeEnds.a : rangeEnds.b) : (rangeEnds?.a ?? null);
  const hi = rangeEnds && rangeEnds.b ? (rangeEnds.a < rangeEnds.b ? rangeEnds.b : rangeEnds.a) : (rangeEnds?.a ?? null);

  return (
    <div className="bu-cal" role="group" aria-label={t("ui.cal.dialog")} data-testid={testId} onKeyDown={onKeyDown} onMouseLeave={() => onHover?.(null)}>
      <div className="bu-cal-head">
        <button type="button" className="bu-cal-nav" onClick={() => shift(-1)} aria-label={t("ui.cal.prev")} title={t("ui.cal.prev")}>
          <ChevronLeft className="flip-rtl" aria-hidden />
        </button>
        <button type="button" className="bu-cal-title" onClick={onTitle} aria-label={`${title} — ${t("ui.cal.pickMonth")}`} data-testid={testId && `${testId}-title`}>
          {title}
        </button>
        <button type="button" className="bu-cal-nav" onClick={() => shift(1)} aria-label={t("ui.cal.next")} title={t("ui.cal.next")}>
          <ChevronRight className="flip-rtl" aria-hidden />
        </button>
      </div>
      <div className="bu-cal-body" ref={gridRef}>
        {view === "days" && (
          <>
            <div className="bu-cal-week" aria-hidden>
              {WEEKDAYS_SHORT[lang].map((w, i) => <span key={i}>{w}</span>)}
            </div>
            <div className="bu-cal-days">
              {monthWeeks(cursor, calendar).flat().map((iso) => {
                const c = inCalendar(iso, calendar);
                const out = !isInMonth(iso, cursor, calendar);
                const off = (!!min && iso < min) || (!!max && iso > max);
                const isEnd = iso === lo || iso === hi;
                const on = iso === selected || isEnd;
                const mid = lo !== null && hi !== null && iso > lo && iso < hi;
                const cls = ["bu-cal-day", out && "out", iso === today && "today", on && "on", mid && "mid", iso === lo && lo !== hi && "first", iso === hi && lo !== hi && "last"].filter(Boolean).join(" ");
                return (
                  <button
                    key={iso}
                    type="button"
                    className={cls}
                    data-iso={iso}
                    tabIndex={iso === tabIso ? 0 : -1}
                    disabled={off}
                    aria-pressed={on}
                    aria-current={iso === today ? "date" : undefined}
                    aria-label={formatDate(iso, lang, calendar)}
                    onClick={() => {
                      setFocusIso(iso);
                      onPick(iso);
                    }}
                    onMouseEnter={() => onHover?.(iso)}
                    data-testid={testId && `${testId}-day-${iso}`}
                  >
                    {digits(c.day, lang)}
                  </button>
                );
              })}
            </div>
          </>
        )}
        {view === "months" && (
          <div className="bu-cal-cells">
            {Array.from({ length: 12 }, (_, i) => i + 1).map((m) => (
              <button
                key={m}
                type="button"
                className={["bu-cal-cell", cur.year === nowYmd.year && m === nowYmd.month && "today", m === cur.month && "on"].filter(Boolean).join(" ")}
                aria-pressed={m === cur.month}
                onClick={() => {
                  const iso = fromCalendar(cur.year, m, 1, calendar);
                  if (iso) {
                    setCursor(iso);
                    setFocusIso(iso);
                  }
                  setView("days");
                }}
              >
                {monthName(calendar, m, lang)}
              </button>
            ))}
          </div>
        )}
        {view === "years" && (
          <div className="bu-cal-cells">
            {Array.from({ length: 12 }, (_, i) => yearBase + i).map((y) => (
              <button
                key={y}
                type="button"
                className={["bu-cal-cell", y === nowYmd.year && "today", y === cur.year && "on"].filter(Boolean).join(" ")}
                aria-pressed={y === cur.year}
                onClick={() => {
                  const iso = fromCalendar(y, cur.month, 1, calendar);
                  if (iso) setCursor(iso);
                  setView("months");
                }}
              >
                {digits(y, lang)}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ───────────────────────────── typed, masked date input ─────────────────────────────

export type MaskedDateInputProps = {
  /** The Gregorian ISO date ("2026-10-03") or "". */
  value: Iso;
  /** Called as soon as the typed text is a real day (inside `min`/`max`). */
  onCommit: (iso: Iso) => void;
  /** If given, an emptied box means "no date" and calls this; otherwise an empty box snaps back on blur. */
  onClear?: () => void;
  calendar: CalendarSystem;
  min?: Iso;
  max?: Iso;
  disabled?: boolean;
  /** Error already decided by the form (red border); the box also reds itself for text it cannot read. */
  invalid?: boolean;
  id?: string;
  "aria-label"?: string;
  "aria-describedby"?: string;
  testId?: string;
  suffix?: ReactNode;
  onKeyDown?: KeyboardEventHandler<HTMLInputElement>;
  /** Bump to throw away unfinished text and show `value` again (after a pick in the calendar). */
  revision?: number;
  /** Reports whether the box currently holds text that is not a usable date. */
  onInvalidChange?: (invalid: boolean) => void;
};

/** One text box for a date in the clinic's calendar: mask 1405/08/20, Persian or ASCII digits, own error line. */
export const MaskedDateInput = forwardRef<HTMLInputElement, MaskedDateInputProps>(function MaskedDateInput(p, ref) {
  const { lang, t } = useI18n();
  const msgId = useId();
  const fmt = (iso: string) => (iso ? formatTypedDate(iso, p.calendar, lang) : "");
  const [draft, setDraft] = useState(() => fmt(p.value));
  const [touched, setTouched] = useState(false);

  // A date that changed from outside replaces the text, unless the text already means that date.
  useEffect(() => {
    setDraft((prev) => (parseTypedDate(prev, p.calendar) === p.value ? prev : fmt(p.value)));
  }, [p.value]);
  // A new calendar, language or revision always rewrites the text.
  useEffect(() => {
    setDraft(fmt(p.value));
  }, [p.calendar, lang, p.revision]);

  const parsed = draft.trim() ? parseTypedDate(draft, p.calendar) : null;
  const outOfRange = parsed !== null && ((!!p.min && parsed < p.min) || (!!p.max && parsed > p.max));
  const status: "empty" | "ok" | "format" | "range" = !draft.trim() ? "empty" : parsed === null ? "format" : outOfRange ? "range" : "ok";
  const bad = status === "format" || status === "range";
  const complete = draft.length >= 10;
  const showError = bad && (touched || complete || status === "range");

  const reportRef = useRef(p.onInvalidChange);
  reportRef.current = p.onInvalidChange;
  useEffect(() => {
    reportRef.current?.(bad);
  }, [bad]);

  const handle = (raw: string) => {
    const next = maskTypedDate(raw, lang);
    setDraft(next);
    if (!next) {
      p.onClear?.();
      return;
    }
    const iso = parseTypedDate(next, p.calendar);
    if (iso && !(p.min && iso < p.min) && !(p.max && iso > p.max) && iso !== p.value) p.onCommit(iso);
  };

  return (
    <div className="bu-date-input">
      <TextInput
        ref={ref}
        id={p.id}
        dir="ltr"
        inputMode="numeric"
        autoComplete="off"
        spellCheck={false}
        placeholder={formatTypedDate(todayIso(), p.calendar, lang)}
        value={draft}
        disabled={p.disabled}
        invalid={p.invalid || showError}
        aria-label={p["aria-label"]}
        aria-describedby={showError ? msgId : p["aria-describedby"]}
        onChange={(e) => handle(e.target.value)}
        onBlur={() => {
          setTouched(true);
          if (status === "empty" && !p.onClear) setDraft(fmt(p.value));
        }}
        onKeyDown={p.onKeyDown}
        suffix={p.suffix}
        data-testid={p.testId}
      />
      {showError && (
        <div className="field-error bu-date-msg" id={msgId} role="alert">
          <CircleAlert aria-hidden />
          <span>{status === "range" ? t("ui.date.outOfRange") : t("ui.date.invalid")}</span>
        </div>
      )}
    </div>
  );
});

// ───────────────────────────── DateField ─────────────────────────────

/**
 * Date input in the clinic's calendar (Solar Hijri by default): a typed, masked box (۱۴۰۵/۰۸/۲۰,
 * ASCII or Persian digits) with a button that opens a calendar. Typing always works; text that is not
 * a real day turns the box red with the reason. The value in and out is the Gregorian ISO date the
 * Core stores ("2026-10-03"). `value=""` shows an empty box.
 *
 * `yearsBefore` / `yearsAfter` are kept so old callers compile; typing needs no year list.
 * Use `min` / `max` for a real limit (a birth date is not in the future) and `clearable` when "no date" is allowed.
 */
export function DateField({ value, onChange, calendar, label, testId, disabled, id, invalid, "aria-describedby": describedBy, min, max, clearable, onInvalidChange }: {
  value: string;
  onChange: (iso: string) => void;
  calendar: CalendarSystem;
  label: string;
  testId?: string;
  yearsBefore?: number;
  yearsAfter?: number;
  disabled?: boolean;
  /** Set by `<Field>` (id, error state and described-by are passed down by it). */
  id?: string;
  invalid?: boolean;
  "aria-describedby"?: string;
  min?: Iso;
  max?: Iso;
  /** Allow an empty box: clearing it calls `onChange("")` and the popup offers "Clear". */
  clearable?: boolean;
  onInvalidChange?: (invalid: boolean) => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [revision, setRevision] = useState(0);
  const anchorRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const autoId = useId();
  const close = (reason: FloatingCloseReason) => {
    setOpen(false);
    if (reason !== "outside") inputRef.current?.focus();
  };
  const finish = (iso: string) => {
    onChange(iso);
    setRevision((r) => r + 1);
    close("escape");
  };
  const today = todayIso();
  const todayOff = (!!min && today < min) || (!!max && today > max);

  return (
    <div className="bu-datefield" ref={anchorRef} role="group" aria-label={label} data-testid={testId}>
      <MaskedDateInput
        ref={inputRef}
        id={id ?? autoId}
        value={value}
        calendar={calendar}
        min={min}
        max={max}
        disabled={disabled}
        invalid={invalid}
        aria-label={label}
        aria-describedby={describedBy}
        testId={testId && `${testId}-input`}
        revision={revision}
        onCommit={onChange}
        onClear={clearable ? () => onChange("") : undefined}
        onInvalidChange={onInvalidChange}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" && e.altKey) {
            e.preventDefault();
            setOpen(true);
          }
        }}
        suffix={
          <button type="button" className="control-action" disabled={disabled} aria-haspopup="dialog" aria-expanded={open} aria-label={`${t("ui.date.open")}: ${label}`} title={t("ui.date.open")} onClick={() => setOpen((o) => !o)} data-testid={testId && `${testId}-open`}>
            <CalendarDays aria-hidden />
          </button>
        }
      />
      {open && (
        <FloatingPanel anchorRef={anchorRef} onClose={close} label={label} className="bu-date-pop" testId={testId && `${testId}-popup`}>
          <CalendarGrid calendar={calendar} initial={value} selected={value || null} onPick={finish} min={min} max={max} autoFocus testId={testId && `${testId}-cal`} />
          <div className="bu-pop-foot">
            <Button variant="subtle" size="sm" disabled={todayOff} onClick={() => finish(today)} data-testid={testId && `${testId}-today`}>{t("ui.date.today")}</Button>
            {clearable && value && <Button variant="subtle" size="sm" onClick={() => finish("")} data-testid={testId && `${testId}-clear`}>{t("ui.date.clear")}</Button>}
          </div>
        </FloatingPanel>
      )}
    </div>
  );
}
