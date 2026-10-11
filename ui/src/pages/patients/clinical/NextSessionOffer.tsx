import { useEffect, useState } from "react";
import { CalendarPlus } from "lucide-react";
import type { AppointmentInfo, CalendarSystem, ChairInfo, DoctorInfo, NextSessionSuggestion } from "../../../../../shared/ts/contract";
import { rpc } from "../../../lib/api";
import { digits, formatDate } from "../../../lib/dates";
import { useCatalog, useClinicSettings } from "../../../lib/clinical";
import { useI18n } from "../../../i18n";
import { Button } from "../../../ui/Button";
import { Notice } from "../../../ui/Feedback";
import { Dialog } from "../../../ui/Overlay";
import { teethLabel } from "../../../chart/numbering";
import { AppointmentDialog } from "../../reception/AppointmentDialog";
import { itemService } from "./labels";

/**
 * After a visit booked for a session is completed (M4): "session 2 of 3 is done — book session 3?",
 * at the service's usual interval, in one click. Nothing shows when the course is finished or the next
 * session is already booked.
 */
export function NextSessionOffer({ appointment, doctors, chairs, calendar, perms, clinicName, solo, onClose, onBooked }: {
  appointment: AppointmentInfo;
  doctors: DoctorInfo[];
  chairs: ChairInfo[];
  calendar: CalendarSystem;
  perms: { edit: boolean; treat: boolean; createPatients: boolean };
  clinicName: string;
  solo: boolean;
  onClose: () => void;
  onBooked: () => void;
}) {
  const { t, lang } = useI18n();
  const catalog = useCatalog();
  const settings = useClinicSettings();
  const [next, setNext] = useState<NextSessionSuggestion | null | undefined>(undefined);
  const [booking, setBooking] = useState(false);
  const tr = appointment.treatment!;
  useEffect(() => {
    rpc("treatment_items.suggest_next", { item_id: tr.item_id }).then(setNext).catch(() => setNext(null));
  }, [tr.item_id]);
  useEffect(() => {
    if (next === null) onClose();
  }, [next]);
  if (!next) return null;
  const what = `${itemService(catalog, tr, lang)} ${teethLabel(tr.teeth, settings?.tooth_numbering ?? "fdi", lang)}`.trim();
  if (booking) {
    return (
      <AppointmentDialog
        defaults={{
          patient: { id: appointment.patient_id, name: appointment.patient_name, number: appointment.patient_number, phone: appointment.patient_phone },
          date: next.date,
          start: appointment.start_time,
          doctorId: next.doctor_id ?? appointment.doctor_id,
          treatmentItemId: next.item_id,
          sessionNo: next.session_no,
          duration: next.duration_minutes,
          reason: what,
        }}
        doctors={doctors}
        chairs={chairs}
        calendar={calendar}
        perms={perms}
        clinicName={clinicName}
        solo={solo}
        onClose={onClose}
        onSaved={onBooked}
      />
    );
  }
  return (
    <Dialog
      title={t("item.nextTitle")}
      description={`${appointment.patient_name} — ${what}`}
      onClose={onClose}
      testId="next-session-offer"
      footer={
        <>
          <Button onClick={onClose} data-testid="next-later">{t("item.nextLater")}</Button>
          <Button variant="primary" icon={CalendarPlus} onClick={() => setBooking(true)} data-testid="next-book">{t("item.nextBook")}</Button>
        </>
      }
    >
      <Notice tone="info" title={t("item.nextSuggestion").replace("{n}", digits(next.session_no, lang)).replace("{of}", digits(next.sessions_planned, lang))} testId="next-suggestion">
        {t("item.nextHint").replace("{date}", formatDate(`${next.date}T06:30:00Z`, lang, calendar))}
      </Notice>
    </Dialog>
  );
}
