import { useMemo, useState } from "react";
import { CalendarClock, Printer, Save } from "lucide-react";
import type { AppointmentInfo, CalendarSystem, ChairInfo, DoctorInfo } from "../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../lib/api";
import { fromMinutes, toMinutes, todayIso } from "../../lib/calendar";
import { digits, formatTime } from "../../lib/dates";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { DateField } from "../../ui/DateField";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Badge, Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { TimePicker12 } from "../../ui/TimePicker";
import { useToast } from "../../ui/Toast";
import { AppointmentCardDialog } from "./AppointmentCard";
import { PatientPicker, type PickedPatient } from "./PatientPicker";
import { StatusActions } from "./StatusActions";
import { MOVABLE, SCHEDULE_RULES, STATUS_TONE } from "./labels";

export type Defaults = { date?: string; start?: string; doctorId?: string; chairId?: string; patient?: PickedPatient | null; recallId?: string | null; reason?: string };

const DURATIONS = [10, 15, 20, 30, 45, 60, 90, 120, 180];

type Values = { doctor_id: string; chair_id: string; date: string; start_time: string; duration: string; reason: string; notes: string };

/**
 * Create or edit one appointment (4.3): patient, doctor, chair, date, time and length. The Core
 * refuses double booking and times outside the doctor's schedule; the exact reason lands under
 * the field it is about, and for a schedule reason reception can book on purpose.
 */
export function AppointmentDialog({ appointment, defaults, doctors, chairs, calendar, perms, clinicName, solo = false, onClose, onSaved }: {
  appointment?: AppointmentInfo;
  defaults?: Defaults;
  doctors: DoctorInfo[];
  chairs: ChairInfo[];
  calendar: CalendarSystem;
  perms: { edit: boolean; treat: boolean; createPatients: boolean };
  clinicName: string;
  /** Solo Clinic Mode (2.6): the one doctor is chosen automatically and not shown. */
  solo?: boolean;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const editing = !!appointment;
  const movable = !appointment || MOVABLE.includes(appointment.status);
  const readOnly = !!appointment && !["scheduled", "confirmed", "checked_in", "in_treatment"].includes(appointment.status);
  const initialDuration = appointment ? toMinutes(appointment.end_time) - toMinutes(appointment.start_time) : 30;
  const durations = DURATIONS.includes(initialDuration) ? DURATIONS : [...DURATIONS, initialDuration].sort((a, b) => a - b);

  const [patient, setPatient] = useState<PickedPatient | null>(
    appointment ? { id: appointment.patient_id, name: appointment.patient_name, number: appointment.patient_number, phone: appointment.patient_phone } : (defaults?.patient ?? null),
  );
  const [patientError, setPatientError] = useState("");
  const [current, setCurrent] = useState<AppointmentInfo | undefined>(appointment);
  const form = useForm<Values>(
    {
      doctor_id: appointment?.doctor_id ?? defaults?.doctorId ?? doctors[0]?.id ?? "",
      chair_id: appointment?.chair_id ?? defaults?.chairId ?? "",
      date: appointment?.date ?? defaults?.date ?? todayIso(),
      start_time: appointment?.start_time ?? defaults?.start ?? "09:00",
      duration: String(initialDuration),
      reason: appointment?.reason ?? defaults?.reason ?? "",
      notes: appointment?.notes ?? "",
    },
    {
      doctor_id: (s) => (s ? null : "rule.required"),
      duration: (s, all) => (toMinutes(all.start_time) + Number(s) > 1440 ? "rule.time_range" : null),
    },
  );
  const [busy, setBusy] = useState<"save" | "reschedule" | null>(null);
  const [error, setError] = useState("");
  const [offerOverride, setOfferOverride] = useState<string | null>(null);
  const [card, setCard] = useState(false);

  const doctor = doctors.find((d) => d.id === form.values.doctor_id);
  // A doctor limited to some chairs only offers those.
  const chairChoices = useMemo(() => chairs.filter((c) => !doctor?.chair_ids.length || doctor.chair_ids.includes(c.id) || c.id === form.values.chair_id), [chairs, doctor, form.values.chair_id]);
  const e = (k: keyof Values) => form.error(k) && t(form.error(k)!);

  const endTime = fromMinutes(Math.min(1440, toMinutes(form.values.start_time) + Number(form.values.duration)));

  const submit = async (mode: "save" | "reschedule", override = false) => {
    setError("");
    setOfferOverride(null);
    setPatientError("");
    if (!patient) return setPatientError(t("rule.required"));
    if (!form.validate()) return;
    const v = form.values;
    const body = { doctor_id: v.doctor_id, chair_id: v.chair_id || null, date: v.date, start_time: v.start_time, end_time: endTime, override_schedule: override };
    setBusy(mode);
    try {
      if (!current) {
        await rpc("appointments.create", { ...body, patient_id: patient.id, reason: v.reason.trim() || null, notes: v.notes.trim() || null, recall_id: defaults?.recallId ?? null });
        toast.success(t("appt.created"));
      } else if (mode === "reschedule") {
        await rpc("appointments.reschedule", { ...body, id: current.id, version: current.version });
        toast.success(t("appt.rescheduled"));
      } else {
        await rpc("appointments.update", { ...body, id: current.id, version: current.version, reason: v.reason.trim() || null, notes: v.notes.trim() || null });
        toast.success(t("common.saved"));
      }
      onSaved();
      onClose();
    } catch (x) {
      if (isSessionError(x)) return;
      if (x instanceof ApiError && x.field === "patient_id") setPatientError(x.rule ? t(`rule.${x.rule}`) : err(x));
      else if (x instanceof ApiError && x.rule && SCHEDULE_RULES.includes(x.rule)) {
        form.serverError(x, { date: "date", start_time: "start_time" });
        setOfferOverride(mode);
      } else if (!form.serverError(x, { end_time: "duration" })) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(null);
    }
  };

  const changed = (a: AppointmentInfo) => {
    setCurrent(a);
    onSaved();
    if (!["scheduled", "confirmed", "checked_in", "in_treatment"].includes(a.status)) onClose();
  };

  return (
    <>
      <Dialog
        title={editing ? t("appt.edit") : t("appt.new")}
        description={current ? <span className="row"><Badge tone={STATUS_TONE[current.status]} dot testId="appt-status-badge">{t(`appt.status.${current.status}`)}</Badge>{current.queue_number != null && <span className="subtle">{t("queue.ticket")} <bdi className="num">{current.queue_number}</bdi></span>}</span> : t("appt.newHint")}
        onClose={onClose}
        wide
        testId="appointment-dialog"
        footer={
          <>
            {current && <Button variant="subtle" icon={Printer} onClick={() => setCard(true)} data-testid="appt-print">{t("appt.card.print")}</Button>}
            <span className="grow" />
            <Button onClick={onClose}>{t("common.close")}</Button>
            {current && movable && perms.edit && (
              <Button icon={CalendarClock} loading={busy === "reschedule"} onClick={() => submit("reschedule")} data-testid="appt-reschedule">{t("appt.reschedule")}</Button>
            )}
            {!readOnly && perms.edit && (
              <Button variant="primary" icon={Save} loading={busy === "save"} onClick={() => submit("save")} data-testid="appt-save">{editing ? t("common.save") : t("appt.book")}</Button>
            )}
          </>
        }
      >
        <div className="stack">
          {error && <Notice tone="danger" testId="appt-error">{error}</Notice>}
          {offerOverride && (
            <Notice tone="warning" title={t("appt.override.title")} testId="appt-override">
              <span className="stack" style={{ gap: 8 }}>
                <span>{t("appt.override.hint")}</span>
                <Button size="sm" onClick={() => submit(offerOverride as "save" | "reschedule", true)} data-testid="appt-override-confirm">{t("appt.override.confirm")}</Button>
              </span>
            </Notice>
          )}
          {editing ? (
            <div className="patient-chip"><div className="patient-chip-text"><span className="cell-strong">{patient?.name}</span><span className="subtle t-caption"><bdi className="ltr num">{patient?.number}</bdi>{patient?.phone ? <> · <bdi className="ltr">{patient.phone}</bdi></> : null}</span></div></div>
          ) : (
            <PatientPicker value={patient} onChange={setPatient} canCreate={perms.createPatients} error={patientError || null} />
          )}
          <div className="grid-2">
            {!solo && (
              <Field label={t("appt.doctor")} error={e("doctor_id")}>
                <Select value={form.values.doctor_id} disabled={!movable || readOnly} onChange={(x) => { form.set("doctor_id", x.target.value); form.set("chair_id", ""); }} data-testid="appt-doctor">
                  {doctors.map((d) => <option key={d.id} value={d.id}>{d.full_name}{d.specialty ? ` — ${d.specialty}` : ""}</option>)}
                </Select>
              </Field>
            )}
            <Field label={t("appt.chair")} optional error={e("chair_id")}>
              <Select value={form.values.chair_id} disabled={!movable || readOnly} onChange={(x) => form.set("chair_id", x.target.value)} data-testid="appt-chair">
                <option value="">—</option>
                {chairChoices.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
              </Select>
            </Field>
            <Field label={t("appt.date")} error={e("date")}>
              <DateField value={form.values.date} onChange={(d) => form.set("date", d)} calendar={calendar} label={t("appt.date")} testId="appt-date" disabled={!movable || readOnly} />
            </Field>
            <Field label={t("appt.start")} error={e("start_time")}>
              <TimePicker12 value={form.values.start_time} onChange={(x) => form.set("start_time", x)} label={t("appt.start")} testId="appt-start" />
            </Field>
            <Field label={t("appt.duration")} hint={`${t("appt.endsAt")} ${formatTime(endTime, lang)}`} error={e("duration")}>
              <Select value={form.values.duration} disabled={!movable || readOnly} onChange={(x) => form.set("duration", x.target.value)} data-testid="appt-duration">
                {durations.map((m) => <option key={m} value={m}>{digits(m, lang)} {t("common.minutes")}</option>)}
              </Select>
            </Field>
            <Field label={t("appt.reason")} optional error={e("reason")}>
              <TextInput value={form.values.reason} maxLength={200} disabled={readOnly} onChange={(x) => form.set("reason", x.target.value)} data-testid="appt-reason" />
            </Field>
          </div>
          <Field label={t("appt.notes")} optional error={e("notes")}>
            <Textarea rows={2} value={form.values.notes} maxLength={1000} disabled={readOnly} onChange={(x) => form.set("notes", x.target.value)} data-testid="appt-notes" />
          </Field>
          {current && (
            <div className="row" data-testid="appt-status-actions">
              <StatusActions appt={current} perms={perms} calendar={calendar} onChanged={changed} size="md" />
            </div>
          )}
          {current?.cancel_reason && <Notice tone="info">{t("appt.cancel.reason")}: {current.cancel_reason}</Notice>}
        </div>
      </Dialog>
      {card && current && <AppointmentCardDialog appointment={current} clinicName={clinicName} calendar={calendar} onClose={() => setCard(false)} />}
    </>
  );
}
