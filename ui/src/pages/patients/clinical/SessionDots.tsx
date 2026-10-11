import { Check } from "lucide-react";
import type { AppointmentTreatment, TreatmentItemInfo } from "../../../../../shared/ts/contract";
import { digits } from "../../../lib/dates";
import { useI18n } from "../../../i18n";

/**
 * A course's progress (M4): ✓ for every session done, a ringed dot for a session already booked, an
 * empty dot for what is left — "جلسه ۲ از ۳" at a glance in the profile, the plan and the queue.
 */
export function SessionDots({ item, treatment, size = "md" }: { item?: TreatmentItemInfo; treatment?: AppointmentTreatment; size?: "sm" | "md" }) {
  const { t, lang } = useI18n();
  const planned = item?.sessions_planned ?? treatment?.sessions_planned ?? 1;
  const done = item?.sessions_done ?? treatment?.sessions_done ?? 0;
  const booked = item?.upcoming.length ?? 0;
  const current = treatment?.session_no ?? null;
  const n = Math.min(planned, 24);
  const label = t("chart.progressOf").replace("{done}", digits(done, lang)).replace("{of}", digits(planned, lang));
  return (
    <span className={`session-dots ${size}`} role="img" aria-label={label} title={label} data-testid="session-dots" data-done={done} data-planned={planned}>
      {Array.from({ length: n }, (_, i) => {
        const k = i + 1;
        const cls = k <= done ? "done" : current === k || (current == null && k <= done + booked) ? "booked" : "left";
        return <span key={i} className={`session-dot ${cls}`}>{cls === "done" && <Check aria-hidden />}</span>;
      })}
      {planned > n && <span className="session-more">+{digits(planned - n, lang)}</span>}
    </span>
  );
}
