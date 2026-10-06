import { useCallback, useEffect, useState } from "react";
import { BellRing, CalendarClock, CalendarPlus, Check, Pencil, Phone, Plus, X } from "lucide-react";
import type { AppointmentInfo, CalendarSystem, ClinicProfile, RecallInfo, RecallKind, RecallStatus } from "../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../lib/api";
import { addDays, daysBetween, todayIso } from "../../lib/calendar";
import { digits, formatDate } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, Page, PageHeader } from "../../ui/Card";
import { DateField } from "../../ui/DateField";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Segmented } from "../../ui/Controls";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { AppointmentDialog } from "./AppointmentDialog";
import { PatientPicker, type PickedPatient } from "./PatientPicker";
import { RECALL_KINDS, RECALL_STATUS_TONE, ruleText } from "./labels";
import { isSolo, useScheduling, type Perms } from "./useScheduling";

type Tab = "call" | "booked" | "closed";
type Window = "all" | "overdue" | "7" | "14" | "30";

const TAB_STATUSES: Record<Tab, RecallStatus[]> = { call: ["pending", "contacted"], booked: ["booked"], closed: ["done", "dismissed"] };

/** The call list (4.6): patients who should come back, who to phone, and what became of it. */
export function RecallsPage({ clinic, perms, clinicName }: { clinic: ClinicProfile | null; perms: Perms; clinicName: string }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const calendar: CalendarSystem = clinic?.calendar_system ?? "shamsi";
  const { doctors, chairs } = useScheduling();
  const [tab, setTab] = useState<Tab>("call");
  const [window_, setWindow] = useState<Window>("all");
  const [items, setItems] = useState<RecallInfo[] | null>(null);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState("");
  const [edit, setEdit] = useState<{ recall?: RecallInfo } | null>(null);
  const [contact, setContact] = useState<RecallInfo | null>(null);
  const [book, setBook] = useState<RecallInfo | null>(null);
  // OF-039: a booked recall's visit can be cancelled or moved from the call list itself.
  const [visit, setVisit] = useState<AppointmentInfo | null>(null);
  const today = todayIso();

  const openVisit = (r: RecallInfo) => {
    if (!r.appointment_id) return;
    rpc("appointments.get", { id: r.appointment_id }).then(setVisit).catch((x) => !isSessionError(x) && toast.error(err(x)));
  };

  const load = useCallback(() => {
    const until = window_ === "overdue" ? addDays(todayIso(), -1) : window_ === "all" ? null : addDays(todayIso(), Number(window_));
    rpc("recalls.list", { patient_id: null, statuses: TAB_STATUSES[tab], due_from: null, due_until: tab === "call" ? until : null, limit: 200, offset: 0 })
      .then((r) => { setItems(r.items); setTotal(r.total); setError(""); })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [tab, window_]);
  useEffect(() => { setItems(null); load(); }, [load]);

  const act = async (r: RecallInfo, status: RecallStatus, note: string | null = null) => {
    try {
      await rpc("recalls.set_status", { id: r.id, version: r.version, status, note });
      toast.success(t(`recall.done.${status}`));
      load();
    } catch (x) {
      if (!isSessionError(x)) toast.error(ruleText(t, x, err));
    }
  };

  const dueText = (r: RecallInfo) => {
    const d = daysBetween(today, r.due_date);
    if (r.status === "pending" || r.status === "contacted") {
      if (d < 0) return <Badge tone="danger" dot>{t("recall.overdue")} {digits(-d, lang)} {t("recall.days")}</Badge>;
      if (d === 0) return <Badge tone="warning" dot>{t("recall.dueToday")}</Badge>;
    }
    return null;
  };

  return (
    <Page testId="page-recalls">
      <PageHeader
        title={t("nav.recalls")}
        description={t("recall.subtitle")}
        actions={perms.edit ? <Button variant="primary" icon={Plus} onClick={() => setEdit({})} data-testid="recall-add-open">{t("recall.add")}</Button> : undefined}
      />
      <div className="cal-toolbar">
        <Segmented<Tab> value={tab} onChange={setTab} label={t("recall.tabs")} options={[{ value: "call", label: t("recall.tab.call"), testId: "recall-tab-call" }, { value: "booked", label: t("recall.tab.booked"), testId: "recall-tab-booked" }, { value: "closed", label: t("recall.tab.closed"), testId: "recall-tab-closed" }]} />
        {tab === "call" && (
          <Select value={window_} onChange={(e) => setWindow(e.target.value as Window)} aria-label={t("recall.window")} data-testid="recall-window">
            <option value="all">{t("recall.window.all")}</option>
            <option value="overdue">{t("recall.window.overdue")}</option>
            <option value="7">{t("recall.window.7")}</option>
            <option value="14">{t("recall.window.14")}</option>
            <option value="30">{t("recall.window.30")}</option>
          </Select>
        )}
      </div>
      <Card flush>
        {error ? <ErrorState message={error} onRetry={load} /> : (
          <div className="table-wrap">
            <table className="table" data-testid="recall-list">
              <thead>
                <tr><th>{t("appt.patient")}</th><th>{t("patient.field.phone")}</th><th>{t("recall.kind")}</th><th>{t("recall.due")}</th><th>{t("recall.lastCall")}</th><th>{t("users.status")}</th><th className="cell-actions"><span className="visually-hidden">{t("recall.actions")}</span></th></tr>
              </thead>
              <tbody>
                {!items && <SkeletonRows cols={7} />}
                {items?.map((r) => (
                  <tr key={r.id} data-testid={`recall-row-${r.patient_number}`}>
                    <td><span className="cell-strong">{r.patient_name}</span><br /><span className="subtle t-caption"><bdi className="ltr num">{r.patient_number}</bdi></span></td>
                    <td><bdi className="ltr" data-testid="recall-phone">{r.patient_phone ?? "—"}</bdi></td>
                    <td><span className="cell-strong">{t(`recall.kind.${r.kind}`)}</span><br /><Badge tone={r.repeat_months ? "accent" : "neutral"}>{r.repeat_months ? t("recall.type.recall") : t("recall.type.follow")}</Badge>{r.repeat_months ? <span className="subtle t-caption"><br />{t("recall.every")} {digits(r.repeat_months, lang)} {t("recall.months")}</span> : null}{r.note ? <span className="subtle t-caption"><br />{r.note}</span> : null}</td>
                    <td>{formatDate(r.due_date + "T06:00:00Z", lang, calendar)}<br />{dueText(r)}</td>
                    <td>{r.last_contacted_at ? <>{formatDate(r.last_contacted_at, lang, calendar)}{r.contact_note ? <span className="subtle t-caption"><br />{r.contact_note}</span> : null}</> : "—"}</td>
                    <td><Badge tone={RECALL_STATUS_TONE[r.status]} dot>{t(`recall.status.${r.status}`)}</Badge></td>
                    <td className="cell-actions">
                      {perms.edit && (r.status === "pending" || r.status === "contacted") && (
                        <div className="row" style={{ justifyContent: "flex-end" }}>
                          <Button size="sm" icon={Phone} onClick={() => setContact(r)} data-testid={`recall-contact-${r.patient_number}`}>{t("recall.contacted")}</Button>
                          <Button size="sm" variant="primary" icon={CalendarPlus} disabled={!doctors?.length} onClick={() => setBook(r)} data-testid={`recall-book-${r.patient_number}`}>{t("recall.book")}</Button>
                          <IconButton icon={Pencil} label={t("common.edit")} size="sm" onClick={() => setEdit({ recall: r })} />
                          <IconButton icon={X} label={t("recall.dismiss")} size="sm" onClick={() => act(r, "dismissed")} data-testid={`recall-dismiss-${r.patient_number}`} />
                        </div>
                      )}
                      {perms.edit && r.status === "dismissed" && <Button size="sm" icon={Check} onClick={() => act(r, "pending")}>{t("recall.reopen")}</Button>}
                      {perms.edit && r.status === "booked" && r.appointment_id && <Button size="sm" icon={CalendarClock} onClick={() => openVisit(r)} data-testid={`recall-visit-${r.patient_number}`}>{t("recall.visit.manage")}</Button>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {items?.length === 0 && <EmptyState icon={BellRing} title={t(`recall.empty.${tab}`)} />}
          </div>
        )}
      </Card>
      {items && total > items.length && <span className="subtle t-caption">{t("recall.showing")} {digits(items.length, lang)} / {digits(total, lang)}</span>}

      {edit && <RecallDialog recall={edit.recall} calendar={calendar} canCreatePatients={perms.createPatients} onClose={() => setEdit(null)} onSaved={load} />}
      {visit && doctors && (
        <AppointmentDialog
          appointment={visit}
          doctors={doctors}
          chairs={chairs}
          calendar={calendar}
          perms={perms}
          clinicName={clinicName}
          solo={isSolo(clinic, doctors)}
          onClose={() => setVisit(null)}
          onSaved={load}
        />
      )}
      {contact && <ContactDialog recall={contact} onClose={() => setContact(null)} onConfirm={(note) => { act(contact, "contacted", note); setContact(null); }} />}
      {book && doctors && (
        <AppointmentDialog
          defaults={{ patient: { id: book.patient_id, name: book.patient_name, number: book.patient_number, phone: book.patient_phone }, recallId: book.id, date: book.due_date < today ? today : book.due_date, reason: t(`recall.kind.${book.kind}`) }}
          doctors={doctors}
          chairs={chairs}
          calendar={calendar}
          perms={perms}
          clinicName={clinicName}
          solo={isSolo(clinic, doctors)}
          onClose={() => setBook(null)}
          onSaved={load}
        />
      )}
    </Page>
  );
}

function ContactDialog({ recall, onClose, onConfirm }: { recall: RecallInfo; onClose: () => void; onConfirm: (note: string | null) => void }) {
  const { t } = useI18n();
  const [note, setNote] = useState("");
  return (
    <Dialog
      title={t("recall.contactTitle")}
      description={<span>{recall.patient_name} · <bdi className="ltr">{recall.patient_phone ?? "—"}</bdi></span>}
      onClose={onClose}
      testId="recall-contact-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Phone} onClick={() => onConfirm(note.trim() || null)} data-testid="recall-contact-confirm">{t("recall.contacted")}</Button></>}
    >
      <Field label={t("recall.contactNote")} hint={t("recall.contactNoteHint")} optional>
        <Textarea rows={3} value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} data-testid="recall-contact-note" />
      </Field>
    </Dialog>
  );
}

/** New or edited recall. */
export function RecallDialog({ recall, patient: fixed, calendar, canCreatePatients, onClose, onSaved }: { recall?: RecallInfo; patient?: PickedPatient; calendar: CalendarSystem; canCreatePatients: boolean; onClose: () => void; onSaved: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [patient, setPatient] = useState<PickedPatient | null>(recall ? { id: recall.patient_id, name: recall.patient_name, number: recall.patient_number, phone: recall.patient_phone } : (fixed ?? null));
  const [kind, setKind] = useState<RecallKind>(recall?.kind ?? "checkup");
  const [due, setDue] = useState(recall?.due_date ?? addDays(todayIso(), 180));
  const [repeat, setRepeat] = useState(recall?.repeat_months ? String(recall.repeat_months) : "");
  const [note, setNote] = useState(recall?.note ?? "");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState("");
  const [patientError, setPatientError] = useState("");
  const months = repeat.trim() ? Number(repeat) : null;
  const repeatError = months !== null && (!Number.isInteger(months) || months < 1 || months > 60) ? t("rule.repeat_months_range") : null;

  const submit = async () => {
    setFailure("");
    if (!patient) return setPatientError(t("rule.required"));
    if (repeatError) return;
    setBusy(true);
    try {
      if (recall) await rpc("recalls.update", { id: recall.id, version: recall.version, kind, due_date: due, repeat_months: months, note: note.trim() || null });
      else await rpc("recalls.create", { patient_id: patient.id, kind, due_date: due, repeat_months: months, note: note.trim() || null });
      toast.success(t("common.saved"));
      onSaved();
      onClose();
    } catch (x) {
      if (isSessionError(x)) return;
      if (x instanceof ApiError && x.field === "patient_id") setPatientError(ruleText(t, x, err));
      else setFailure(ruleText(t, x, err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={recall ? t("recall.edit") : t("recall.add")}
      description={t("recall.addHint")}
      onClose={onClose}
      wide
      testId="recall-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={submit} data-testid="recall-save">{t("common.save")}</Button></>}
    >
      <div className="stack">
        {failure && <Notice tone="danger">{failure}</Notice>}
        {recall ? <div className="patient-chip"><div className="patient-chip-text"><span className="cell-strong">{recall.patient_name}</span></div></div> : <PatientPicker value={patient} onChange={setPatient} canCreate={canCreatePatients} error={patientError || null} testId="recall-patient" />}
        <div className="grid-2">
          <Field label={t("recall.kind")}>
            <Select value={kind} onChange={(e) => setKind(e.target.value as RecallKind)} data-testid="recall-kind">
              {RECALL_KINDS.map((k) => <option key={k} value={k}>{t(`recall.kind.${k}`)}</option>)}
            </Select>
          </Field>
          <Field label={t("recall.due")}>
            <DateField value={due} onChange={setDue} calendar={calendar} label={t("recall.due")} testId="recall-date" yearsAfter={6} />
          </Field>
        </div>
        <Field label={t("recall.repeat")} hint={t("recall.repeatHint")} optional error={repeatError}>
          <TextInput inputMode="numeric" dir="ltr" value={repeat} onChange={(e) => setRepeat(e.target.value)} data-testid="recall-repeat" />
        </Field>
        <Field label={t("recall.note")} optional>
          <Textarea rows={2} value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} data-testid="recall-note" />
        </Field>
      </div>
    </Dialog>
  );
}
