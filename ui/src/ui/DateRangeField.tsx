import { useId, useRef, useState } from "react";
import { CalendarRange, ChevronDown } from "lucide-react";
import type { CalendarSystem } from "../../../shared/ts/contract";
import { formatTypedDate, presetRange, todayIso } from "../lib/calendar";
import type { Iso, RangePresetId } from "../lib/calendar";
import { useI18n } from "../i18n";
import { CalendarGrid, MaskedDateInput } from "./DateField";
import { FloatingPanel } from "./Overlay";
import type { FloatingCloseReason } from "./Overlay";

export type DateRange = { from: Iso; to: Iso };
export type { RangePresetId };

const ALL_PRESETS: RangePresetId[] = ["today", "last7", "thisMonth", "lastMonth", "thisYear"];

/**
 * One control for a period: a single button "۱۴۰۵/۰۸/۰۱ تا ۱۴۰۵/۰۸/۲۰ ▾". Its popup has quick picks
 * (today, last 7 days, this month, last month, this year), two typed boxes and a calendar: click the
 * first day, then the last (a second click before the first swaps them). A preset or the second click
 * applies the range and closes; typing applies at once and keeps the popup open. If a typed start
 * falls after the end (or the end before the start) the other end follows it. Values use the same
 * ISO format as `DateField`.
 */
export function DateRangeField({ value, onChange, calendar, label, testId, presets = ALL_PRESETS, disabled, id, invalid, "aria-describedby": describedBy, min, max }: {
  value: DateRange;
  onChange: (range: DateRange) => void;
  calendar: CalendarSystem;
  label: string;
  testId?: string;
  /** Which quick picks to offer, in order. Default: all five. */
  presets?: RangePresetId[];
  disabled?: boolean;
  /** Set by `<Field>`. */
  id?: string;
  invalid?: boolean;
  "aria-describedby"?: string;
  min?: Iso;
  max?: Iso;
}) {
  const { lang, t } = useI18n();
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState<Iso | null>(null);
  const [hover, setHover] = useState<Iso | null>(null);
  const [revision, setRevision] = useState(0);
  const anchorRef = useRef<HTMLButtonElement>(null);
  const autoId = useId();
  const f = (iso: Iso) => formatTypedDate(iso, calendar, lang);

  const openIt = () => {
    setPending(null);
    setHover(null);
    setOpen(true);
  };
  const close = (reason: FloatingCloseReason) => {
    setOpen(false);
    setPending(null);
    setHover(null);
    if (reason !== "outside") anchorRef.current?.focus();
  };
  const apply = (range: DateRange, andClose: boolean) => {
    onChange(range);
    setRevision((r) => r + 1);
    if (andClose) close("escape");
  };
  const pick = (iso: Iso) => {
    if (pending === null) {
      setPending(iso);
      return;
    }
    apply(iso < pending ? { from: iso, to: pending } : { from: pending, to: iso }, true);
  };
  const typedFrom = (iso: Iso) => onChange({ from: iso, to: value.to && value.to >= iso ? value.to : iso });
  const typedTo = (iso: Iso) => onChange({ from: value.from && value.from <= iso ? value.from : iso, to: iso });

  const summary = value.from && value.to ? (
    <>
      <bdi dir="ltr">{f(value.from)}</bdi> {t("ui.range.to")} <bdi dir="ltr">{f(value.to)}</bdi>
    </>
  ) : (
    <span className="bu-range-empty">{t("ui.range.choose")}</span>
  );
  const summaryText = value.from && value.to ? `${f(value.from)} ${t("ui.range.to")} ${f(value.to)}` : t("ui.range.choose");
  const today = todayIso();

  return (
    <div className="bu-rangefield" data-testid={testId && `${testId}-field`}>
      <button
        ref={anchorRef}
        id={id ?? autoId}
        type="button"
        className="control bu-range-trigger"
        data-invalid={invalid || undefined}
        data-disabled={disabled || undefined}
        disabled={disabled}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={`${label}: ${summaryText}`}
        aria-describedby={describedBy}
        onClick={() => (open ? close("escape") : openIt())}
        data-testid={testId}
      >
        <CalendarRange className="bu-range-icon" aria-hidden />
        <span className="bu-range-text">{summary}</span>
        <ChevronDown className="chevron" aria-hidden />
      </button>
      {open && (
        <FloatingPanel anchorRef={anchorRef} onClose={close} label={label} className="bu-range-pop" testId={testId && `${testId}-popup`}>
          <div className="bu-range-presets" role="group" aria-label={t("ui.range.presets")}>
            {presets.map((p) => {
              const r = presetRange(p, calendar, today);
              const active = r.from === value.from && r.to === value.to;
              const off = (!!min && r.from < min) || (!!max && r.to > max);
              return (
                <button key={p} type="button" className="bu-range-preset" aria-pressed={active} disabled={off} onClick={() => apply(r, true)} data-testid={testId && `${testId}-preset-${p}`}>
                  {t(`ui.range.${p}`)}
                </button>
              );
            })}
          </div>
          <div className="bu-range-main">
            <div className="bu-range-inputs">
              <label className="bu-range-box">
                <span className="t-caption muted">{t("period.from")}</span>
                <MaskedDateInput value={value.from} calendar={calendar} min={min} max={max} revision={revision} aria-label={`${label}: ${t("period.from")}`} onCommit={typedFrom} testId={testId && `${testId}-from`} />
              </label>
              <label className="bu-range-box">
                <span className="t-caption muted">{t("period.to")}</span>
                <MaskedDateInput value={value.to} calendar={calendar} min={min} max={max} revision={revision} aria-label={`${label}: ${t("period.to")}`} onCommit={typedTo} testId={testId && `${testId}-to`} />
              </label>
            </div>
            <CalendarGrid
              calendar={calendar}
              initial={value.from}
              range={pending ? { from: pending, to: null } : { from: value.from || null, to: value.to || null }}
              preview={pending ? hover : null}
              onHover={setHover}
              onPick={pick}
              min={min}
              max={max}
              autoFocus
              testId={testId && `${testId}-cal`}
            />
          </div>
        </FloatingPanel>
      )}
    </div>
  );
}
