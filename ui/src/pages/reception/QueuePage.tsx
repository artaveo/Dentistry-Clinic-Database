import { useCallback, useEffect, useMemo, useState } from "react";
import { Clock, DoorOpen, Footprints, Stethoscope, UserRoundCheck } from "lucide-react";
import type { AppointmentInfo, CalendarSystem, ClinicProfile } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { todayIso } from "../../lib/calendar";
import { digits, formatDate, formatTime } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Page, PageHeader } from "../../ui/Card";
import { Select, TextInput, Field } from "../../ui/Field";
import { Badge, EmptyState, ErrorState, Loading, Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { AppointmentDialog } from "./AppointmentDialog";
import { PatientPicker, type PickedPatient } from "./PatientPicker";
import { StatusActions } from "./StatusActions";
import { STATUS_TONE, ruleText } from "./labels";
import { isSolo, useScheduling, type Perms } from "./useScheduling";

const POLL_MS = 10_000;

/**
 * "Today's queue" (4.5) for reception and doctors: who is expected, waiting, in treatment and done,
 * with one-tap next steps and walk-in registration. It refreshes every few seconds; on a network
 * of several computers a change made at the front desk shows up at the doctor's computer the same
 * way (instant push over the network is Phase 7).
 */
export function QueuePage({ clinic, perms, clinicName }: { clinic: ClinicProfile | null; perms: Perms; clinicName: string }) {
  const { t, err, lang } = useI18n();
  const calendar: CalendarSystem = clinic?.calendar_system ?? "shamsi";
  const { doctors, chairs, error: setupError, reload } = useScheduling();
  const [items, setItems] = useState<AppointmentInfo[] | null>(null);
  const [error, setError] = useState("");
  const [doctorId, setDoctorId] = useState("");
  const [walkIn, setWalkIn] = useState(false);
  const [open, setOpen] = useState<AppointmentInfo | null>(null);
  const [today, setToday] = useState(todayIso());

  const load = useCallback(() => {
    const d = todayIso();
    setToday(d);
    rpc("appointments.list", { date_from: d, date_to: d, doctor_id: doctorId || null, chair_id: null, patient_id: null, statuses: [], limit: 1000, offset: 0 })
      .then((a) => { setItems(a); setError(""); })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [doctorId]);

  useEffect(() => {
    load();
    const id = window.setInterval(() => !document.hidden && load(), POLL_MS);
    return () => window.clearInterval(id);
  }, [load]);

  const groups = useMemo(() => {
    const by = (s: AppointmentInfo["status"][]) => (items ?? []).filter((a) => s.includes(a.status));
    return {
      expected: by(["scheduled", "confirmed"]).sort((a, b) => a.start_time.localeCompare(b.start_time)),
      waiting: by(["checked_in"]).sort((a, b) => (a.queue_number ?? 0) - (b.queue_number ?? 0)),
      treating: by(["in_treatment"]).sort((a, b) => (a.treatment_started_at ?? "").localeCompare(b.treatment_started_at ?? "")),
      done: by(["completed"]).sort((a, b) => (b.completed_at ?? "").localeCompare(a.completed_at ?? "")),
      closed: by(["cancelled", "no_show"]),
    };
  }, [items]);
  const solo = isSolo(clinic, doctors);

  if (setupError) return <Page><ErrorState message={err(setupError)} onRetry={reload} /></Page>;
  if (error && !items) return <Page><ErrorState message={error} onRetry={load} /></Page>;
  if (!items || !doctors) return <Page><Loading /></Page>;

  const columns: { key: keyof typeof groups; icon: typeof Clock; list: AppointmentInfo[] }[] = [
    { key: "expected", icon: Clock, list: groups.expected },
    { key: "waiting", icon: DoorOpen, list: groups.waiting },
    { key: "treating", icon: Stethoscope, list: groups.treating },
    { key: "done", icon: UserRoundCheck, list: groups.done },
  ];

  return (
    <Page testId="page-queue">
      <PageHeader
        title={t("nav.queue")}
        description={`${t("queue.subtitle")} · ${formatDate(today + "T06:00:00Z", lang, calendar)}`}
        actions={
          <>
            {!solo && (
              <Select value={doctorId} onChange={(e) => setDoctorId(e.target.value)} aria-label={t("appt.doctor")} data-testid="queue-doctor">
                <option value="">{t("cal.allDoctors")}</option>
                {doctors.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
              </Select>
            )}
            {perms.edit && <Button variant="primary" icon={Footprints} disabled={!doctors.length} onClick={() => setWalkIn(true)} data-testid="walk-in-open">{t("queue.walkIn")}</Button>}
          </>
        }
      />
      <div className="queue-board" data-testid="queue-board">
        {columns.map(({ key, icon: Icon, list }) => (
          <section key={key} className={`queue-col q-${key}`} data-testid={`queue-${key}`} aria-label={t(`queue.${key}`)}>
            <header className="queue-head">
              <Icon aria-hidden />
              <span className="t-subtitle">{t(`queue.${key}`)}</span>
              <Badge testId={`queue-count-${key}`}>{digits(list.length, lang)}</Badge>
            </header>
            <div className="queue-list">
              {list.map((a) => (
                <article key={a.id} className={`queue-card st-${a.status}`} data-testid={`queue-card-${a.id}`}>
                  <button type="button" className="queue-card-main" onClick={() => setOpen(a)} data-testid={`queue-open-${a.id}`}>
                    <span className="row queue-card-top">
                      {a.queue_number != null && <span className="queue-ticket" title={t("queue.ticket")} data-testid={`queue-ticket-${a.id}`}>{digits(a.queue_number, lang)}</span>}
                      <span className="cell-strong queue-name">{a.patient_name}</span>
                      <Badge tone={STATUS_TONE[a.status]} dot>{t(`appt.status.${a.status}`)}</Badge>
                    </span>
                    <span className="subtle t-caption">
                      {a.is_walk_in ? t("queue.walkInBadge") : `${formatTime(a.start_time, lang)} – ${formatTime(a.end_time, lang)}`}
                      {solo ? (a.chair_name ? ` · ${a.chair_name}` : "") : <>{" · "}<span className="dot-swatch" style={{ background: a.doctor_color }} aria-hidden /> {a.doctor_name}{a.chair_name ? ` · ${a.chair_name}` : ""}</>}
                    </span>
                    {a.reason && <span className="t-caption">{a.reason}</span>}
                  </button>
                  <div className="row queue-actions">
                    <StatusActions appt={a} perms={perms} calendar={calendar} onChanged={load} only={["confirmed", "checked_in", "in_treatment", "completed", "no_show"]} />
                  </div>
                </article>
              ))}
              {!list.length && <EmptyState icon={Icon} title={t(`queue.empty.${key}`)} />}
            </div>
          </section>
        ))}
      </div>
      {groups.closed.length > 0 && (
        <details className="queue-closed" data-testid="queue-closed">
          <summary>{t("queue.closed")} ({digits(groups.closed.length, lang)})</summary>
          <ul className="plain-list">
            {groups.closed.map((a) => (
              <li key={a.id}><button type="button" className="link-btn" onClick={() => setOpen(a)}>{a.patient_name}</button> · {formatTime(a.start_time, lang)} · <Badge tone={STATUS_TONE[a.status]}>{t(`appt.status.${a.status}`)}</Badge></li>
            ))}
          </ul>
        </details>
      )}
      {walkIn && <WalkInDialog doctors={doctors.map((d) => ({ id: d.id, name: d.full_name }))} chairs={chairs} defaultDoctor={doctorId || doctors[0]?.id || ""} canCreate={perms.createPatients} solo={solo} onClose={() => setWalkIn(false)} onAdded={() => { setWalkIn(false); load(); }} />}
      {open && <AppointmentDialog appointment={open} doctors={doctors} chairs={chairs} calendar={calendar} perms={perms} clinicName={clinicName} solo={solo} onClose={() => setOpen(null)} onSaved={load} />}
    </Page>
  );
}

function WalkInDialog({ doctors, chairs, defaultDoctor, canCreate, solo, onClose, onAdded }: { doctors: { id: string; name: string }[]; chairs: { id: string; name: string }[]; defaultDoctor: string; canCreate: boolean; solo: boolean; onClose: () => void; onAdded: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [patient, setPatient] = useState<PickedPatient | null>(null);
  const [doctor, setDoctor] = useState(defaultDoctor);
  const [chair, setChair] = useState("");
  const [reason, setReason] = useState("");
  const [failure, setFailure] = useState("");
  const [patientError, setPatientError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setFailure("");
    if (!patient) return setPatientError(t("rule.required"));
    setPatientError("");
    setBusy(true);
    try {
      const a = await rpc("appointments.walk_in", { patient_id: patient.id, doctor_id: doctor, chair_id: chair || null, reason: reason.trim() || null, duration_minutes: null });
      toast.success(`${t("queue.walkInAdded")} ${a.queue_number ?? ""}`.trim());
      onAdded();
    } catch (x) {
      if (isSessionError(x)) return;
      const msg = ruleText(t, x, err);
      if (x && typeof x === "object" && "field" in x && (x as { field: string | null }).field === "patient_id") setPatientError(msg);
      else setFailure(msg);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("queue.walkIn")}
      description={t("queue.walkInHint")}
      onClose={onClose}
      testId="walk-in-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={Footprints} loading={busy} onClick={submit} data-testid="walk-in-confirm">{t("queue.walkInAdd")}</Button>
        </>
      }
    >
      <div className="stack">
        {failure && <Notice tone="danger">{failure}</Notice>}
        <PatientPicker value={patient} onChange={setPatient} canCreate={canCreate} error={patientError || null} testId="walkin-patient" />
        <div className={solo ? "" : "grid-2"}>
          {!solo && (
            <Field label={t("appt.doctor")}>
              <Select value={doctor} onChange={(e) => setDoctor(e.target.value)} data-testid="walk-in-doctor">
                {doctors.map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
              </Select>
            </Field>
          )}
          <Field label={t("appt.chair")} optional>
            <Select value={chair} onChange={(e) => setChair(e.target.value)} data-testid="walk-in-chair">
              <option value="">—</option>
              {chairs.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </Select>
          </Field>
        </div>
        <Field label={t("appt.reason")} optional>
          <TextInput value={reason} maxLength={200} onChange={(e) => setReason(e.target.value)} data-testid="walk-in-reason" />
        </Field>
      </div>
    </Dialog>
  );
}
