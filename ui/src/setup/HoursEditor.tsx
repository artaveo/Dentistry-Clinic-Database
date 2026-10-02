import type { DayHours } from "../../../shared/ts/contract";
import { useI18n } from "../i18n";
import { Switch } from "../ui/Controls";
import { TimePicker12 } from "../ui/TimePicker";

/** Afghan week (ADR-21): 0 = Saturday … 6 = Friday. Friday is the default day off. */
export function defaultHours(): DayHours[] {
  return [0, 1, 2, 3, 4, 5, 6].map((day) => ({ day, closed: day === 6, open: day === 6 ? null : "08:00", close: day === 6 ? null : "16:00" }));
}

/** Working hours with 12-hour pickers (OF-007); stored as 24-hour "HH:MM". */
export function HoursEditor({ hours, onChange }: { hours: DayHours[]; onChange: (h: DayHours[]) => void }) {
  const { t } = useI18n();
  const rows = hours.length ? hours : defaultHours();
  const setDay = (day: number, patch: Partial<DayHours>) => onChange(rows.map((h) => (h.day === day ? { ...h, ...patch } : h)));
  return (
    <div className="hours" data-testid="working-hours">
      {rows.map((h) => (
        <div className="hours-row" key={h.day}>
          <span className="day">{t(`wizard.day.${h.day}`)}</span>
          <Switch
            checked={!h.closed}
            onChange={(open) => setDay(h.day, open ? { closed: false, open: "08:00", close: "16:00" } : { closed: true, open: null, close: null })}
            label={<span className="visually-hidden">{t(`wizard.day.${h.day}`)}: {h.closed ? t("wizard.hours.closed") : t("wizard.hours.open")}</span>}
            testId={`day-${h.day}-open`}
          />
          {h.closed ? (
            <span className="closed">{t("wizard.hours.closed")}</span>
          ) : (
            <div className="times">
              <TimePicker12 value={h.open ?? "08:00"} onChange={(v) => setDay(h.day, { open: v })} label={t("wizard.hours.from")} testId={`day-${h.day}-from`} />
              <span className="to">{t("wizard.hours.to")}</span>
              <TimePicker12 value={h.close ?? "16:00"} onChange={(v) => setDay(h.day, { close: v })} label={t("wizard.hours.until")} testId={`day-${h.day}-to`} />
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

/** First open day whose closing time is not after its opening time, if any. */
export function badHours(hours: DayHours[]): boolean {
  return hours.some((h) => !h.closed && (!h.open || !h.close || h.open >= h.close));
}
