import { useCallback, useEffect, useState } from "react";
import { BellRing, CalendarClock, CalendarPlus, Plus } from "lucide-react";
import type { AppointmentInfo, CalendarSystem, ChairInfo, ClinicProfile, DoctorInfo, PatientInfo, RecallInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { formatDate, formatTime } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Badge, EmptyState, ErrorState, SkeletonRows } from "../../ui/Feedback";
import { AppointmentDialog } from "../reception/AppointmentDialog";
import { RecallDialog } from "../reception/RecallsPage";
import { RECALL_STATUS_TONE, STATUS_TONE } from "../reception/labels";
import { isSolo, type Perms } from "../reception/useScheduling";

/** The patient's visits and call-list entries (profile tab "Appointments", 3.6/4.6). */
export function PatientAppointmentsTab({ patient, perms, clinic, clinicName }: { patient: PatientInfo; perms: Perms & { view: boolean }; clinic: ClinicProfile | null; clinicName: string }) {
  const { t, err, lang } = useI18n();
  const calendar: CalendarSystem = clinic?.calendar_system ?? "shamsi";
  const [items, setItems] = useState<AppointmentInfo[] | null>(null);
  const [recalls, setRecalls] = useState<RecallInfo[]>([]);
  const [doctors, setDoctors] = useState<DoctorInfo[]>([]);
  const [chairs, setChairs] = useState<ChairInfo[]>([]);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<{ appt?: AppointmentInfo } | null>(null);
  const [recallDialog, setRecallDialog] = useState(false);
  const solo = isSolo(clinic, doctors);
  const me = { id: patient.id, name: patient.full_name, number: patient.patient_number, phone: patient.phone };

  const load = useCallback(() => {
    Promise.all([
      rpc("appointments.list", { date_from: null, date_to: null, doctor_id: null, chair_id: null, patient_id: patient.id, statuses: [], limit: 200, offset: 0 }),
      rpc("recalls.list", { patient_id: patient.id, statuses: ["pending", "contacted", "booked"], due_from: null, due_until: null, limit: 50, offset: 0 }),
    ])
      .then(([a, r]) => { setItems(a); setRecalls(r.items); setError(""); })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [patient.id]);
  useEffect(() => { load(); }, [load]);
  useEffect(() => {
    Promise.all([rpc("doctors.list", { include_inactive: false }), rpc("chairs.list", { include_inactive: false })]).then(([d, c]) => { setDoctors(d); setChairs(c); }).catch(() => {});
  }, []);

  return (
    <div className="stack">
      <Card flush>
        <CardHeader
          icon={CalendarClock}
          title={t("patients.tab.appointments")}
          actions={perms.edit && patient.status === "active" && !patient.merged_into_id ? <Button variant="primary" icon={CalendarPlus} disabled={!doctors.length} onClick={() => setDialog({})} data-testid="patient-appt-new">{t("appt.new")}</Button> : undefined}
        />
        {error ? <ErrorState message={error} onRetry={load} /> : (
          <div className="table-wrap">
            <table className="table" data-testid="patient-appointments">
              <thead><tr><th>{t("appt.date")}</th><th>{t("appt.time")}</th>{!solo && <th>{t("appt.doctor")}</th>}<th>{t("appt.reason")}</th><th>{t("users.status")}</th></tr></thead>
              <tbody>
                {!items && <SkeletonRows cols={solo ? 4 : 5} />}
                {items?.map((a) => (
                  <tr key={a.id} onClick={() => setDialog({ appt: a })} style={{ cursor: "pointer" }} data-testid={`patient-appt-${a.id}`}>
                    <td>{formatDate(a.start_at, lang, calendar)}</td>
                    <td>{formatTime(a.start_time, lang)} – {formatTime(a.end_time, lang)}</td>
                    {!solo && <td><span className="dot-swatch" style={{ background: a.doctor_color }} aria-hidden /> {a.doctor_name}</td>}
                    <td>{a.reason ?? "—"}</td>
                    <td><Badge tone={STATUS_TONE[a.status]} dot>{t(`appt.status.${a.status}`)}</Badge></td>
                  </tr>
                ))}
              </tbody>
            </table>
            {items?.length === 0 && <EmptyState icon={CalendarClock} title={t("patientAppt.empty")} />}
          </div>
        )}
      </Card>
      <Card flush>
        <CardHeader icon={BellRing} title={t("patientAppt.recalls")} description={t("patientAppt.recallsHint")} actions={perms.edit ? <Button icon={Plus} onClick={() => setRecallDialog(true)} data-testid="patient-recall-add">{t("recall.add")}</Button> : undefined} />
        <ul className="plain-list" style={{ margin: "0 var(--space-4) var(--space-4)" }} data-testid="patient-recalls">
          {recalls.length === 0 && <li className="subtle">{t("patientAppt.noRecalls")}</li>}
          {recalls.map((r) => (
            <li key={r.id} className="row" style={{ justifyContent: "space-between" }}>
              <span>{t(`recall.kind.${r.kind}`)} · {formatDate(r.due_date + "T06:00:00Z", lang, calendar)}{r.note ? ` · ${r.note}` : ""}</span>
              <Badge tone={RECALL_STATUS_TONE[r.status]} dot>{t(`recall.status.${r.status}`)}</Badge>
            </li>
          ))}
        </ul>
      </Card>
      {dialog && <AppointmentDialog key={dialog.appt?.id ?? "new"} appointment={dialog.appt} defaults={{ patient: me }} doctors={doctors} chairs={chairs} calendar={calendar} perms={perms} clinicName={clinicName} solo={solo} onClose={() => setDialog(null)} onSaved={load} />}
      {recallDialog && <RecallDialog patient={me} calendar={calendar} canCreatePatients={false} onClose={() => setRecallDialog(false)} onSaved={load} />}
    </div>
  );
}
