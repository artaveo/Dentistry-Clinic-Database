import { ChevronDown } from "lucide-react";
import { useI18n } from "../i18n";
import { PERIODS, digits, to12, to24 } from "../lib/dates";
import { Segmented } from "./Controls";

const HOURS = [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const MINUTES = Array.from({ length: 12 }, (_, i) => i * 5);

function HourSelect({ hour12, onChange, label, testId }: { hour12: number; onChange: (h: number) => void; label: string; testId?: string }) {
  const { lang } = useI18n();
  return (
    <div className="control">
      <select value={hour12} onChange={(e) => onChange(Number(e.target.value))} aria-label={label} data-testid={testId}>
        {HOURS.map((h) => <option key={h} value={h}>{digits(h, lang)}</option>)}
      </select>
      <ChevronDown className="chevron" aria-hidden />
    </div>
  );
}

function Period({ pm, onChange, testId }: { pm: boolean; onChange: (pm: boolean) => void; testId?: string }) {
  const { lang, t } = useI18n();
  return (
    <Segmented<"am" | "pm">
      value={pm ? "pm" : "am"}
      onChange={(v) => onChange(v === "pm")}
      label={t("time.period")}
      options={[
        { value: "am", label: PERIODS[lang].am, testId: testId && `${testId}-am` },
        { value: "pm", label: PERIODS[lang].pm, testId: testId && `${testId}-pm` },
      ]}
    />
  );
}

/**
 * 12-hour time input (OF-007): hour 1–12, minutes in 5-minute steps and
 * ق.ظ/ب.ظ. The value in and out stays the stored 24-hour "HH:MM".
 */
export function TimePicker12({ value, onChange, label, testId }: { value: string; onChange: (v: string) => void; label: string; testId?: string }) {
  const { lang, t } = useI18n();
  const [hh, mm] = (value || "08:00").split(":").map(Number);
  const { hour, pm } = to12(hh);
  const minute = MINUTES.includes(mm) ? mm : Math.round(mm / 5) * 5 % 60;
  const emit = (h12: number, m: number, isPm: boolean) => onChange(`${String(to24(h12, isPm)).padStart(2, "0")}:${String(m).padStart(2, "0")}`);
  return (
    <div className="timepicker" role="group" aria-label={label}>
      <span className="timepicker-clock" dir="ltr">
      <HourSelect hour12={hour} onChange={(h) => emit(h, minute, pm)} label={t("time.hour")} testId={testId && `${testId}-hour`} />
      <span className="sep">:</span>
      <div className="control">
        <select value={minute} onChange={(e) => emit(hour, Number(e.target.value), pm)} aria-label={t("time.minute")} data-testid={testId && `${testId}-minute`}>
          {MINUTES.map((m) => <option key={m} value={m}>{digits(String(m).padStart(2, "0"), lang)}</option>)}
        </select>
        <ChevronDown className="chevron" aria-hidden />
      </div>
      </span>
      <Period pm={pm} onChange={(p) => emit(hour, minute, p)} testId={testId} />
    </div>
  );
}

/** A whole hour (0–23 stored), shown and picked as 12-hour, e.g. the daily backup time. */
export function HourPicker12({ value, onChange, label, testId }: { value: number; onChange: (h: number) => void; label: string; testId?: string }) {
  const { t, lang } = useI18n();
  const { hour, pm } = to12(value);
  return (
    <div className="timepicker" role="group" aria-label={label}>
      <span className="timepicker-clock" dir="ltr">
        <HourSelect hour12={hour} onChange={(h) => onChange(to24(h, pm))} label={t("time.hour")} testId={testId && `${testId}-hour`} />
        <span className="sep">:</span>
        <span className="subtle">{digits("00", lang)}</span>
      </span>
      <Period pm={pm} onChange={(p) => onChange(to24(hour, p))} testId={testId} />
    </div>
  );
}
