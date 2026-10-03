import { Printer } from "lucide-react";
import type { AppointmentInfo, CalendarSystem } from "../../../../shared/ts/contract";
import { formatDate, formatTime, digits } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Overlay";

/**
 * The appointment card the patient takes home (4.6). Plain browser printing for now; the
 * real print engine with paper sizes and thermal printers is Phase 6 (ADR-13).
 */
export function AppointmentCardDialog({ appointment: a, clinicName, calendar, onClose }: { appointment: AppointmentInfo; clinicName: string; calendar: CalendarSystem; onClose: () => void }) {
  const { t, lang } = useI18n();
  const print = () => {
    document.body.classList.add("printing-card");
    const done = () => {
      document.body.classList.remove("printing-card");
      window.removeEventListener("afterprint", done);
    };
    window.addEventListener("afterprint", done);
    window.print();
  };
  return (
    <Dialog
      title={t("appt.card.title")}
      onClose={onClose}
      testId="appointment-card-dialog"
      footer={
        <>
          <Button onClick={onClose} data-testid="appt-card-close">{t("common.close")}</Button>
          <Button variant="primary" icon={Printer} onClick={print} data-testid="appt-card-print">{t("appt.card.print")}</Button>
        </>
      }
    >
      <div className="print-card" data-testid="appointment-card">
        <div className="print-card-clinic">{clinicName}</div>
        <div className="print-card-title">{t("appt.card.title")}</div>
        <dl className="kv">
          <div><dt>{t("appt.patient")}</dt><dd>{a.patient_name} <bdi className="ltr num">({a.patient_number})</bdi></dd></div>
          <div><dt>{t("appt.doctor")}</dt><dd>{a.doctor_name}</dd></div>
          <div><dt>{t("appt.date")}</dt><dd data-testid="appointment-card-date">{formatDate(a.start_at, lang, calendar)}</dd></div>
          <div><dt>{t("appt.time")}</dt><dd>{formatTime(a.start_time, lang)} – {formatTime(a.end_time, lang)}</dd></div>
          {a.chair_name && <div><dt>{t("appt.chair")}</dt><dd>{a.chair_name}</dd></div>}
          {a.reason && <div><dt>{t("appt.reason")}</dt><dd>{a.reason}</dd></div>}
        </dl>
        <div className="print-card-foot">{t("appt.card.note")} · {digits(a.date, lang)}</div>
      </div>
    </Dialog>
  );
}
