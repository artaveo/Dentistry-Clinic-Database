import { useEffect, useMemo, useState } from "react";
import { Ban, CalendarCheck2, CalendarPlus, Check, CheckCheck, ClipboardList, Pencil, Plus, Printer, RotateCcw, Scale, Undo2 } from "lucide-react";
import type { CalendarSystem, ClinicProfile, DocumentInfo, Language, PatientInfo, PlanSnapshot, PlanStatus, ToothNumbering, TreatmentItemInfo, TreatmentPlanInfo } from "../../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../../lib/api";
import { digits, formatDate, latinDigits } from "../../../lib/dates";
import { serviceName, useCatalog, useClinicSettings, usePlanTemplates } from "../../../lib/clinical";
import { tr } from "../../../lib/medical";
import { formatAmount } from "../../../lib/money";
import { todayIso } from "../../../lib/calendar";
import { useI18n } from "../../../i18n";
import { Button, IconButton } from "../../../ui/Button";
import { Card, CardHeader } from "../../../ui/Card";
import { Switch } from "../../../ui/Controls";
import { DateField } from "../../../ui/DateField";
import { Field, Select, Textarea, TextInput } from "../../../ui/Field";
import { Badge, EmptyState, ErrorState, Loading, Notice } from "../../../ui/Feedback";
import { Dialog } from "../../../ui/Overlay";
import { useToast } from "../../../ui/Toast";
import { DocumentPreview } from "../../../print/DocumentPreview";
import { DocumentSheet } from "../../../print/DocumentSheet";
import { useLetterhead } from "../../../print/letterhead";
import { isTooth } from "../../../chart/geometry";
import { teethLabel } from "../../../chart/numbering";
import { AppointmentDialog } from "../../reception/AppointmentDialog";
import { isSolo, useScheduling } from "../../reception/useScheduling";
import { docPatient, LanguagePick, LivePreview } from "../documents/shared";
import { ITEM_TONE, PLAN_TONE, amountInput, itemService, parseAmount, sessionText, surfacesText } from "./labels";
import { SessionDots } from "./SessionDots";

export type BookPerms = { edit: boolean; treat: boolean; createPatients: boolean };

/**
 * Patient profile → «پلن‌های درمان» (5.7, M4): the patient's plans as cards — proposed alternatives side by
 * side, the chosen one in progress — with phases, prices from the catalog (changeable per patient), each
 * item's sessions (✓✓○), recording a session, booking the next one, and the printed estimate.
 */
export function PlansTab({ patient, clinic, canEdit, calendar, booking, clinicName, onChanged }: {
  patient: PatientInfo;
  clinic: ClinicProfile | null;
  canEdit: boolean;
  calendar: CalendarSystem;
  booking: BookPerms;
  clinicName: string;
  onChanged?: () => void;
}) {
  const { t, err, lang } = useI18n();
  const settings = useClinicSettings();
  const numbering = settings?.tooth_numbering ?? "fdi";
  const [plans, setPlans] = useState<TreatmentPlanInfo[] | null>(null);
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);
  const [estimate, setEstimate] = useState<TreatmentPlanInfo[] | null>(null);
  const [showClosed, setShowClosed] = useState(false);
  const load = () =>
    rpc("plans.list", { patient_id: patient.id })
      .then((p) => {
        setPlans(p);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patient.id]);
  const changed = () => {
    load();
    onChanged?.();
  };
  if (error && !plans) return <ErrorState message={error} onRetry={load} />;
  if (!plans) return <Card><Loading /></Card>;
  const proposed = plans.filter((p) => p.status === "proposed");
  const open = plans.filter((p) => p.status !== "cancelled" && p.status !== "completed");
  const closed = plans.filter((p) => p.status === "cancelled" || p.status === "completed");
  const action = canEdit ? <Button variant="primary" icon={Plus} onClick={() => setCreating(true)} data-testid="plan-new">{t("plan.new")}</Button> : undefined;
  return (
    <div className="stack-lg" data-testid="plans-tab">
      <Card flush>
        <CardHeader icon={ClipboardList} title={t("patients.tab.treatmentPlans")} description={t("plan.subtitle")} actions={plans.length ? action : undefined} />
        {proposed.length > 1 && (
          <div className="plan-alternatives" data-testid="plan-alternatives">
            <Notice tone="info" title={t("plan.alternatives").replace("{n}", digits(proposed.length, lang))}>
              <span className="row">
                <span>{t("plan.alternativesHint")}</span>
                <Button size="sm" icon={Scale} onClick={() => setEstimate(proposed)} data-testid="plan-compare-print">{t("plan.printCompare")}</Button>
              </span>
            </Notice>
          </div>
        )}
        {!plans.length && <EmptyState icon={ClipboardList} title={t("plan.none")} action={action}>{t("plan.noneHint")}</EmptyState>}
      </Card>
      {open.map((p) => (
        <PlanCard key={p.id} plan={p} others={proposed.length > 1} patient={patient} clinic={clinic} canEdit={canEdit} calendar={calendar} booking={booking} clinicName={clinicName} numbering={numbering} onChanged={changed} onPrint={() => setEstimate([p])} />
      ))}
      {closed.length > 0 && (
        <Button variant="subtle" onClick={() => setShowClosed((x) => !x)} data-testid="plan-show-closed">
          {showClosed ? t("plan.hideClosed") : t("plan.showClosed").replace("{n}", digits(closed.length, lang))}
        </Button>
      )}
      {showClosed && closed.map((p) => (
        <PlanCard key={p.id} plan={p} others={false} patient={patient} clinic={clinic} canEdit={canEdit} calendar={calendar} booking={booking} clinicName={clinicName} numbering={numbering} onChanged={changed} onPrint={() => setEstimate([p])} />
      ))}
      {creating && <NewPlanDialog patient={patient} count={plans.length} onClose={() => setCreating(false)} onCreated={() => { setCreating(false); changed(); }} />}
      {estimate && <EstimateDialog patient={patient} plans={estimate} calendar={calendar} numbering={numbering} onClose={() => setEstimate(null)} onIssued={changed} />}
    </div>
  );
}

function PlanCard({ plan, others, patient, clinic, canEdit, calendar, booking, clinicName, numbering, onChanged, onPrint }: {
  plan: TreatmentPlanInfo;
  others: boolean;
  patient: PatientInfo;
  clinic: ClinicProfile | null;
  canEdit: boolean;
  calendar: CalendarSystem;
  booking: BookPerms;
  clinicName: string;
  numbering: ToothNumbering;
  onChanged: () => void;
  onPrint: () => void;
}) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [renaming, setRenaming] = useState(false);
  const [accepting, setAccepting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const phases = useMemo(() => [...new Set(plan.items.map((i) => i.phase))].sort(), [plan.items]);
  const closed = plan.status === "completed" || plan.status === "cancelled";
  const openItems = plan.items.filter((i) => i.status === "planned" || i.status === "in_progress").length;
  const status = async (to: PlanStatus, extra: { reason?: string | null; cancel_alternatives?: boolean } = {}) => {
    try {
      await rpc("plans.set_status", { id: plan.id, version: plan.version, status: to, reason: extra.reason ?? null, cancel_alternatives: !!extra.cancel_alternatives });
      toast.success(t(`plan.done.${to}`));
      onChanged();
    } catch (x) {
      if (!isSessionError(x)) toast.error(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
      throw x;
    }
  };
  return (
    <Card flush className={`plan-card st-${plan.status}`} data-testid={`plan-${plan.id}`}>
      <header className="plan-head">
        <div className="plan-titles">
          <h2 className="t-title">{plan.title}</h2>
          <span className="row plan-meta">
            <Badge tone={PLAN_TONE[plan.status]} dot testId={`plan-status-${plan.id}`}>{t(`plan.status.${plan.status}`)}</Badge>
            <span className="subtle t-caption">{t("plan.createdOn").replace("{date}", formatDate(plan.created_at, lang, calendar))}{plan.created_by_name ? ` · ${plan.created_by_name}` : ""}</span>
            {plan.superseded_by && <span className="subtle t-caption">{t("plan.superseded")}</span>}
            {plan.cancel_reason && <span className="subtle t-caption">{t("plan.cancelReason")}: {plan.cancel_reason}</span>}
          </span>
          {plan.notes && <p className="plan-notes">{plan.notes}</p>}
        </div>
        <div className="plan-total">
          <span className="subtle t-caption">{t("plan.total")}</span>
          <span className="plan-total-value num" data-testid={`plan-total-${plan.id}`}>{formatAmount(plan.total, lang)}</span>
        </div>
      </header>
      <div className="plan-actions row">
        {canEdit && plan.status === "proposed" && <Button size="sm" variant="primary" icon={Check} onClick={() => (others ? setAccepting(true) : status("accepted").catch(() => {}))} data-testid={`plan-accept-${plan.id}`}>{t("plan.accept")}</Button>}
        {canEdit && plan.status === "accepted" && !plan.items.some((i) => i.sessions_done > 0) && <Button size="sm" icon={Undo2} onClick={() => status("proposed").catch(() => {})} data-testid={`plan-unaccept-${plan.id}`}>{t("plan.unaccept")}</Button>}
        {canEdit && (plan.status === "accepted" || plan.status === "in_progress") && openItems === 0 && plan.items.length > 0 && <Button size="sm" icon={CheckCheck} onClick={() => status("completed").catch(() => {})} data-testid={`plan-complete-${plan.id}`}>{t("plan.complete")}</Button>}
        <Button size="sm" icon={Printer} onClick={onPrint} disabled={!plan.items.length} data-testid={`plan-print-${plan.id}`}>{t("plan.print")}</Button>
        {canEdit && !closed && <Button size="sm" variant="subtle" icon={Pencil} onClick={() => setRenaming(true)} data-testid={`plan-edit-${plan.id}`}>{t("plan.edit")}</Button>}
        <span className="grow" />
        {canEdit && !closed && <Button size="sm" variant="subtle" icon={Ban} onClick={() => setCancelling(true)} data-testid={`plan-cancel-${plan.id}`}>{t("plan.cancel")}</Button>}
        {canEdit && plan.status === "cancelled" && <Button size="sm" variant="subtle" icon={RotateCcw} onClick={() => status("proposed").catch(() => {})} data-testid={`plan-reopen-${plan.id}`}>{t("plan.reopen")}</Button>}
      </div>
      {plan.items.length ? (
        <div className="table-wrap">
          <table className="table plan-table" data-testid={`plan-items-${plan.id}`}>
            <thead>
              <tr>
                <th>{t("plan.service")}</th>
                <th>{t("plan.teeth")}</th>
                <th>{t("plan.sessionsCol")}</th>
                <th className="num">{t("plan.price")}</th>
                <th>{t("plan.statusCol")}</th>
                <th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th>
              </tr>
            </thead>
            {phases.map((ph) => (
              <tbody key={ph}>
                <tr className="plan-phase-row"><td colSpan={6}>{t("plan.phaseN").replace("{n}", digits(ph, lang))}</td></tr>
                {plan.items.filter((i) => i.phase === ph).map((i) => (
                  <ItemRow key={i.id} item={i} plan={plan} patient={patient} clinic={clinic} canEdit={canEdit && !closed} calendar={calendar} booking={booking} clinicName={clinicName} numbering={numbering} onChanged={onChanged} />
                ))}
              </tbody>
            ))}
          </table>
        </div>
      ) : (
        <p className="subtle plan-empty">{t("plan.noItems")}</p>
      )}
      {renaming && <RenameDialog plan={plan} onClose={() => setRenaming(false)} onSaved={() => { setRenaming(false); onChanged(); }} />}
      {accepting && (
        <ConfirmDialog
          title={t("plan.acceptTitle")}
          text={t("plan.acceptHint")}
          switchLabel={t("plan.setAsideOthers")}
          confirm={t("plan.accept")}
          onClose={() => setAccepting(false)}
          onConfirm={(others) => status("accepted", { cancel_alternatives: others }).then(() => setAccepting(false))}
          testId="plan-accept-dialog"
        />
      )}
      {cancelling && (
        <ReasonDialog title={t("plan.cancelTitle")} hint={t("plan.cancelHint")} confirm={t("plan.cancel")} onClose={() => setCancelling(false)} onConfirm={(reason) => status("cancelled", { reason }).then(() => setCancelling(false))} testId="plan-cancel-dialog" />
      )}
    </Card>
  );
}

function ItemRow({ item, plan, patient, clinic, canEdit, calendar, booking, clinicName, numbering, onChanged }: {
  item: TreatmentItemInfo;
  plan: TreatmentPlanInfo | null;
  patient: PatientInfo;
  clinic: ClinicProfile | null;
  canEdit: boolean;
  calendar: CalendarSystem;
  booking: BookPerms;
  clinicName: string;
  numbering: ToothNumbering;
  onChanged: () => void;
}) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const catalog = useCatalog();
  const [editing, setEditing] = useState(false);
  const [recording, setRecording] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [book, setBook] = useState(false);
  const open = item.status === "planned" || item.status === "in_progress";
  const remaining = item.sessions_planned - item.sessions_done - item.upcoming.length;
  const setStatus = async (to: TreatmentItemInfo["status"], reason: string | null = null) => {
    try {
      await rpc("treatment_items.set_status", { id: item.id, version: item.version, status: to, reason });
      toast.success(t(`item.done.${to}`));
      onChanged();
    } catch (x) {
      if (!isSessionError(x)) toast.error(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
      throw x;
    }
  };
  return (
    <tr className={`item-row st-${item.status}`} data-testid={`item-${item.id}`}>
      <td>
        <span className="cell-strong">{itemService(catalog, item, lang)}</span>
        {item.details && <span className="subtle t-caption item-details"> · {item.details}</span>}
        {item.notes && <span className="t-caption item-notes">{item.notes}</span>}
      </td>
      <td className="item-teeth"><bdi>{[teethLabel(item.teeth, numbering, lang), surfacesText(item.surfaces, item.teeth[0])].filter(Boolean).join(" ") || t("plan.wholeMouth")}</bdi></td>
      <td>
        <span className="item-sessions">
          {item.sessions_planned > 1 && <SessionDots item={item} />}
          <span className="t-caption">{sessionText(t, lang, item) || t("chart.sessions").replace("{n}", digits(item.sessions_planned, lang))}</span>
        </span>
        {item.upcoming[0] && <span className="t-caption subtle item-next" data-testid={`item-next-${item.id}`}>{t("plan.nextVisit").replace("{date}", formatDate(`${item.upcoming[0].date}T06:30:00Z`, lang, calendar))}</span>}
      </td>
      <td className="num">{item.units > 1 && <span className="subtle t-caption">{digits(item.units, lang)} × </span>}{item.unit_price ? formatAmount(item.unit_price * item.units, lang) : <Badge tone="warning">{t("catalog.noPrice")}</Badge>}</td>
      <td>
        <Badge tone={ITEM_TONE[item.status]} dot>{t(`item.status.${item.status}`)}</Badge>
        {item.status === "completed" && !item.billed && <span className="t-caption subtle item-billing"> · {t("item.readyForInvoice")}</span>}
      </td>
      <td className="cell-actions">
        <span className="row item-actions">
          {canEdit && open && <IconButton icon={CalendarCheck2} size="sm" label={t("item.recordSession")} onClick={() => setRecording(true)} data-testid={`item-record-${item.id}`} />}
          {booking.edit && open && remaining > 0 && <IconButton icon={CalendarPlus} size="sm" label={t("item.bookSession")} onClick={() => setBook(true)} data-testid={`item-book-${item.id}`} />}
          {canEdit && open && <IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing(true)} data-testid={`item-edit-${item.id}`} />}
          {canEdit && open && <IconButton icon={CheckCheck} size="sm" label={t("item.complete")} onClick={() => window.confirm(t("item.completeConfirm")) && setStatus("completed").catch(() => {})} data-testid={`item-complete-${item.id}`} />}
          {canEdit && open && <IconButton icon={Ban} size="sm" label={t("item.cancel")} onClick={() => setCancelling(true)} data-testid={`item-cancel-${item.id}`} />}
          {canEdit && item.status === "cancelled" && (!plan || (plan.status !== "cancelled" && plan.status !== "completed")) && <IconButton icon={Undo2} size="sm" label={t("item.restore")} onClick={() => setStatus("planned").catch(() => {})} data-testid={`item-restore-${item.id}`} />}
        </span>
        {editing && <ItemDialog item={item} clinic={clinic} numbering={numbering} onClose={() => setEditing(false)} onSaved={() => { setEditing(false); onChanged(); }} />}
        {recording && (
          <RecordSessionDialog
            item={item}
            patient={patient}
            clinic={clinic}
            calendar={calendar}
            booking={booking}
            clinicName={clinicName}
            numbering={numbering}
            onClose={() => setRecording(false)}
            onRecorded={onChanged}
          />
        )}
        {book && <BookSession item={item} patient={patient} clinic={clinic} calendar={calendar} booking={booking} clinicName={clinicName} onClose={() => setBook(false)} onBooked={onChanged} />}
        {cancelling && <ReasonDialog title={t("item.cancelTitle")} hint={t("item.cancelHint")} confirm={t("item.cancel")} onClose={() => setCancelling(false)} onConfirm={(reason) => setStatus("cancelled", reason).then(() => setCancelling(false))} testId="item-cancel-dialog" />}
      </td>
    </tr>
  );
}

/** Edit one item: teeth, surfaces, price for this patient (the catalog's by default), phase, sessions, details. */
function ItemDialog({ item, clinic, numbering, onClose, onSaved }: { item: TreatmentItemInfo; clinic: ClinicProfile | null; numbering: ToothNumbering; onClose: () => void; onSaved: () => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const catalog = useCatalog();
  const { doctors } = useScheduling();
  const solo = isSolo(clinic, doctors);
  const service = catalog?.services.find((s) => s.id === item.service_id);
  const [teeth, setTeeth] = useState(item.teeth.join(", "));
  const [surfaces, setSurfaces] = useState(item.surfaces);
  const [price, setPrice] = useState(amountInput(item.unit_price));
  const [phase, setPhase] = useState(String(item.phase));
  const [sessions, setSessions] = useState(String(item.sessions_planned));
  const [details, setDetails] = useState(item.details ?? "");
  const [notes, setNotes] = useState(item.notes ?? "");
  const [doctorId, setDoctorId] = useState(item.doctor_id ?? "");
  const [estStart, setEstStart] = useState(item.est_start ?? "");
  const [estEnd, setEstEnd] = useState(item.est_end ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [server, setServer] = useState<Record<string, string>>({});
  const parsedTeeth = latinDigits(teeth).split(/[\s,،]+/).filter(Boolean).map(Number);
  const errors: Record<string, string | null> = {
    teeth: parsedTeeth.some((x) => !isTooth(x)) ? t("rule.tooth_number") : null,
    surfaces: /[^MODBLIFPmodblifp]/.test(surfaces) ? t("rule.tooth_surface") : null,
    unit_price: parseAmount(price, latinDigits) === null ? t("rule.item_price") : null,
    sessions_planned: (() => { const n = Number(latinDigits(sessions)); return !Number.isInteger(n) || n < Math.max(1, item.sessions_done) || n > 60 ? t("rule.item_sessions") : null; })(),
  };
  const show = (k: string) => errors[k] ?? server[k] ?? null;
  const save = async () => {
    setError("");
    if (Object.values(errors).some(Boolean)) return;
    setBusy(true);
    try {
      await rpc("treatment_items.update", {
        id: item.id, version: item.version, teeth: parsedTeeth, surfaces: surfaces.toUpperCase(), doctor_id: solo ? item.doctor_id : doctorId || null,
        unit_price: parseAmount(price, latinDigits)!, phase: Number(phase), sessions_planned: Number(latinDigits(sessions)),
        details: details.trim() || null, notes: notes.trim() || null, est_start: estStart || null, est_end: estEnd || null,
      });
      toast.success(t("common.saved"));
      onSaved();
    } catch (x) {
      if (isSessionError(x)) return;
      if (x instanceof ApiError && x.field && x.rule) setServer({ [x.field]: t(`rule.${x.rule}`) });
      else setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t("item.edit")}
      description={service ? serviceName(service, catalog, lang) : undefined}
      onClose={onClose}
      testId="item-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="item-save">{t("common.save")}</Button></>}
    >
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="grid-2">
          <Field label={t("plan.teeth")} hint={t("item.teethHint").replace("{example}", teethLabel([36, 37], numbering, lang))} error={show("teeth")}>
            <TextInput dir="ltr" value={teeth} onChange={(e) => { setTeeth(e.target.value); setServer({}); }} data-testid="item-teeth" />
          </Field>
          {service?.needs_surface && (
            <Field label={t("item.surfaces")} hint={t("item.surfacesHint")} error={show("surfaces")}>
              <TextInput dir="ltr" value={surfaces} maxLength={5} onChange={(e) => { setSurfaces(e.target.value); setServer({}); }} data-testid="item-surfaces" />
            </Field>
          )}
          <Field label={t("plan.unitPrice")} hint={service?.price ? t("plan.catalogPrice").replace("{price}", formatAmount(service.price, lang)) : t("plan.noCatalogPrice")} error={show("unit_price")}>
            <TextInput inputMode="numeric" suffix={lang === "en" ? "AFN" : "افغانی"} value={price} onChange={(e) => setPrice(e.target.value)} data-testid="item-edit-price" />
          </Field>
          <Field label={t("plan.phase")}>
            <Select value={phase} onChange={(e) => setPhase(e.target.value)} data-testid="item-edit-phase">
              {[1, 2, 3, 4, 5].map((n) => <option key={n} value={n}>{t("plan.phaseN").replace("{n}", digits(n, lang))}</option>)}
            </Select>
          </Field>
          <Field label={t("plan.sessions")} hint={item.sessions_done ? t("item.sessionsDone").replace("{n}", digits(item.sessions_done, lang)) : t("plan.sessionsHint")} error={show("sessions_planned")}>
            <TextInput inputMode="numeric" dir="ltr" value={sessions} onChange={(e) => setSessions(e.target.value)} data-testid="item-edit-sessions" />
          </Field>
          {!solo && doctors && doctors.length > 1 && (
            <Field label={t("appt.doctor")} optional>
              <Select value={doctorId} onChange={(e) => setDoctorId(e.target.value)} data-testid="item-edit-doctor">
                <option value="">—</option>
                {doctors.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
              </Select>
            </Field>
          )}
          <Field label={t("plan.details")} hint={t("plan.detailsHint")} optional>
            <TextInput value={details} maxLength={80} onChange={(e) => setDetails(e.target.value)} data-testid="item-edit-details" />
          </Field>
          {item.sessions_planned > 3 && (
            <>
              <Field label={t("item.estStart")} optional><DateField value={estStart} onChange={setEstStart} calendar={clinic?.calendar_system ?? "shamsi"} label={t("item.estStart")} testId="item-est-start" /></Field>
              <Field label={t("item.estEnd")} optional><DateField value={estEnd} onChange={setEstEnd} calendar={clinic?.calendar_system ?? "shamsi"} label={t("item.estEnd")} testId="item-est-end" /></Field>
            </>
          )}
        </div>
        <Field label={t("plan.notes")} optional>
          <Textarea rows={2} value={notes} maxLength={1000} onChange={(e) => setNotes(e.target.value)} data-testid="item-edit-notes" />
        </Field>
      </div>
    </Dialog>
  );
}

/**
 * Records a session (M4) and, when sessions are left and none is booked, offers the next one after the
 * service's usual interval.
 */
export function RecordSessionDialog({ item, patient, clinic, calendar, booking, clinicName, numbering, onClose, onRecorded }: {
  item: TreatmentItemInfo;
  patient: PatientInfo;
  clinic: ClinicProfile | null;
  calendar: CalendarSystem;
  booking: BookPerms;
  clinicName: string;
  numbering: ToothNumbering;
  onClose: () => void;
  onRecorded: () => void;
}) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const catalog = useCatalog();
  const { doctors } = useScheduling();
  const solo = isSolo(clinic, doctors);
  const [date, setDate] = useState(todayIso());
  const [doctorId, setDoctorId] = useState(item.doctor_id ?? "");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [next, setNext] = useState<{ item: TreatmentItemInfo; date: string; session: number; duration: number; doctor: string | null } | null>(null);
  const [booking2, setBooking2] = useState(false);
  useEffect(() => {
    if (!doctorId && doctors?.length) setDoctorId(doctors[0].id);
  }, [doctors]);
  const n = item.sessions_done + 1;
  const save = async () => {
    setError("");
    setBusy(true);
    try {
      const r = await rpc("treatment_items.record_session", { item_id: item.id, version: item.version, appointment_id: null, doctor_id: doctorId || null, performed_on: date, note: note.trim() || null });
      toast.success(r.item.status === "completed" ? t("item.sessionDoneLast") : t("item.sessionDone").replace("{n}", digits(r.session.session_no, lang)));
      onRecorded();
      if (r.next && booking.edit) setNext({ item: r.item, date: r.next.date, session: r.next.session_no, duration: r.next.duration_minutes, doctor: r.next.doctor_id });
      else onClose();
    } catch (x) {
      if (!isSessionError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(false);
    }
  };
  if (booking2 && next) {
    return (
      <AppointmentDialogFor
        patient={patient}
        clinic={clinic}
        calendar={calendar}
        booking={booking}
        clinicName={clinicName}
        defaults={{ date: next.date, doctorId: next.doctor ?? undefined, treatmentItemId: next.item.id, sessionNo: next.session, duration: next.duration, reason: `${itemService(catalog, next.item, lang)} ${teethLabel(next.item.teeth, numbering, lang)}`.trim() }}
        onClose={onClose}
        onSaved={onRecorded}
      />
    );
  }
  return (
    <Dialog
      title={next ? t("item.nextTitle") : t("item.recordTitle").replace("{n}", digits(n, lang)).replace("{of}", digits(item.sessions_planned, lang))}
      description={`${itemService(catalog, item, lang)} ${teethLabel(item.teeth, numbering, lang)}`}
      onClose={onClose}
      testId="session-dialog"
      footer={
        next ? (
          <>
            <Button onClick={onClose} data-testid="next-later">{t("item.nextLater")}</Button>
            <Button variant="primary" icon={CalendarPlus} onClick={() => setBooking2(true)} data-testid="next-book">{t("item.nextBook")}</Button>
          </>
        ) : (
          <>
            <Button onClick={onClose}>{t("common.cancel")}</Button>
            <Button variant="primary" icon={CalendarCheck2} loading={busy} onClick={save} data-testid="session-save">{t("item.recordSession")}</Button>
          </>
        )
      }
    >
      {next ? (
        <Notice tone="info" title={t("item.nextSuggestion").replace("{n}", digits(next.session, lang)).replace("{of}", digits(next.item.sessions_planned, lang))} testId="next-suggestion">
          {t("item.nextHint").replace("{date}", formatDate(`${next.date}T06:30:00Z`, lang, calendar))}
        </Notice>
      ) : (
        <div className="stack">
          {error && <Notice tone="danger" testId="session-error">{error}</Notice>}
          <div className="row"><SessionDots item={item} /><span className="t-caption">{sessionText(t, lang, item)}</span></div>
          <div className="grid-2">
            <Field label={t("item.performedOn")}>
              <DateField value={date} onChange={setDate} calendar={calendar} label={t("item.performedOn")} testId="session-date" />
            </Field>
            {!solo && doctors && doctors.length > 1 && (
              <Field label={t("appt.doctor")}>
                <Select value={doctorId} onChange={(e) => setDoctorId(e.target.value)} data-testid="session-doctor">
                  {doctors.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
                </Select>
              </Field>
            )}
          </div>
          <Field label={t("item.sessionNote")} hint={t("item.sessionNoteHint")} optional>
            <TextInput value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} data-testid="session-note" />
          </Field>
        </div>
      )}
    </Dialog>
  );
}

/** Book a session of an item: the visit is linked to it ("session 2 of 3") with the suggested date. */
function BookSession({ item, patient, clinic, calendar, booking, clinicName, onClose, onBooked }: { item: TreatmentItemInfo; patient: PatientInfo; clinic: ClinicProfile | null; calendar: CalendarSystem; booking: BookPerms; clinicName: string; onClose: () => void; onBooked: () => void }) {
  const { lang } = useI18n();
  const catalog = useCatalog();
  const settings = useClinicSettings();
  const [suggestion, setSuggestion] = useState<{ date?: string; doctor?: string; duration?: number; session?: number } | null>(null);
  useEffect(() => {
    rpc("treatment_items.suggest_next", { item_id: item.id })
      .then((s) => setSuggestion(s ? { date: s.date, doctor: s.doctor_id ?? undefined, duration: s.duration_minutes, session: s.session_no } : {}))
      .catch(() => setSuggestion({}));
  }, [item.id]);
  if (!suggestion) return null;
  return (
    <AppointmentDialogFor
      patient={patient}
      clinic={clinic}
      calendar={calendar}
      booking={booking}
      clinicName={clinicName}
      defaults={{ date: suggestion.date, doctorId: suggestion.doctor ?? item.doctor_id ?? undefined, treatmentItemId: item.id, sessionNo: suggestion.session, duration: suggestion.duration, reason: `${itemService(catalog, item, lang)} ${teethLabel(item.teeth, settings?.tooth_numbering ?? "fdi", lang)}`.trim() }}
      onClose={onClose}
      onSaved={onBooked}
    />
  );
}

function AppointmentDialogFor({ patient, clinic, calendar, booking, clinicName, defaults, onClose, onSaved }: {
  patient: PatientInfo;
  clinic: ClinicProfile | null;
  calendar: CalendarSystem;
  booking: BookPerms;
  clinicName: string;
  defaults: { date?: string; doctorId?: string; treatmentItemId: string; sessionNo?: number; duration?: number; reason?: string };
  onClose: () => void;
  onSaved: () => void;
}) {
  const { doctors, chairs } = useScheduling();
  if (!doctors) return null;
  return (
    <AppointmentDialog
      defaults={{ ...defaults, patient: { id: patient.id, name: patient.full_name, number: patient.patient_number, phone: patient.phone } }}
      doctors={doctors}
      chairs={chairs}
      calendar={calendar}
      perms={booking}
      clinicName={clinicName}
      solo={isSolo(clinic, doctors)}
      onClose={onClose}
      onSaved={onSaved}
    />
  );
}

function NewPlanDialog({ patient, count, onClose, onCreated }: { patient: PatientInfo; count: number; onClose: () => void; onCreated: () => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const templates = usePlanTemplates();
  const catalog = useCatalog();
  const settings = useClinicSettings();
  const [title, setTitle] = useState(t("plan.defaultTitle").replace("{n}", digits(count + 1, lang)));
  const [templateId, setTemplateId] = useState("");
  const [teeth, setTeeth] = useState("");
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState("");
  const template = templates?.find((x) => x.id === templateId);
  const needsTeeth = !!template?.items.some((i) => i.teeth === "selected");
  const parsed = latinDigits(teeth).split(/[\s,،]+/).filter(Boolean).map(Number);
  const teethError = needsTeeth && (!parsed.length || parsed.some((x) => !isTooth(x))) ? t(parsed.length ? "rule.tooth_number" : "rule.item_teeth") : null;
  const titleError = !title.trim() ? t("rule.required") : null;
  const create = async () => {
    setSubmitted(true);
    setError("");
    if (titleError || teethError) return;
    setBusy(true);
    try {
      await rpc("plans.create", { patient_id: patient.id, title: title.trim(), notes: notes.trim() || null, template_id: templateId || null, teeth: needsTeeth ? parsed : [] });
      toast.success(t("plan.created"));
      onCreated();
    } catch (x) {
      if (!isSessionError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t("plan.new")}
      description={patient.full_name}
      onClose={onClose}
      testId="plan-new-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Plus} loading={busy} onClick={create} data-testid="plan-create">{t("plan.create")}</Button></>}
    >
      <div className="stack">
        {error && <Notice tone="danger" testId="plan-new-error">{error}</Notice>}
        <Field label={t("plan.title")} error={titleError}>
          <TextInput value={title} maxLength={120} onChange={(e) => setTitle(e.target.value)} data-testid="plan-title" />
        </Field>
        <Field label={t("plan.template")} hint={t("plan.templateHint")} optional>
          <Select value={templateId} onChange={(e) => setTemplateId(e.target.value)} data-testid="plan-template">
            <option value="">{t("plan.emptyPlan")}</option>
            {templates?.map((x) => <option key={x.id} value={x.id}>{tr(x.name, lang)}</option>)}
          </Select>
        </Field>
        {template && (
          <ul className="plain-list plan-template-preview" data-testid="plan-template-preview">
            {template.items.map((i, k) => {
              const s = catalog?.services.find((x) => x.id === i.service_id);
              return <li key={k}><Badge>{t("plan.phaseN").replace("{n}", digits(i.phase, lang))}</Badge> {s ? serviceName(s, catalog, lang) : "—"} <span className="subtle t-caption">· {t(`plan.templateTeeth.${i.teeth}`)}</span></li>;
            })}
          </ul>
        )}
        {needsTeeth && (
          <Field label={t("plan.templateTeethField")} hint={t("item.teethHint").replace("{example}", digits("36", lang))} error={submitted || teeth.trim() ? teethError : null}>
            <TextInput dir="ltr" value={teeth} onChange={(e) => setTeeth(e.target.value)} placeholder={settings?.tooth_numbering === "fdi" ? "36" : ""} data-testid="plan-teeth" />
          </Field>
        )}
        <Field label={t("plan.notes")} optional>
          <Textarea rows={2} value={notes} maxLength={2000} onChange={(e) => setNotes(e.target.value)} data-testid="plan-notes" />
        </Field>
      </div>
    </Dialog>
  );
}

function RenameDialog({ plan, onClose, onSaved }: { plan: TreatmentPlanInfo; onClose: () => void; onSaved: () => void }) {
  const { t, err } = useI18n();
  const [title, setTitle] = useState(plan.title);
  const [notes, setNotes] = useState(plan.notes ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const save = async () => {
    if (!title.trim()) return setError(t("rule.required"));
    setBusy(true);
    try {
      await rpc("plans.update", { id: plan.id, version: plan.version, title: title.trim(), notes: notes.trim() || null });
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("plan.edit")} onClose={onClose} testId="plan-rename-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="plan-rename-save">{t("common.save")}</Button></>}>
      <div className="stack">
        <Field label={t("plan.title")} error={error || null}>
          <TextInput value={title} maxLength={120} onChange={(e) => { setTitle(e.target.value); setError(""); }} data-testid="plan-rename-title" />
        </Field>
        <Field label={t("plan.notes")} optional>
          <Textarea rows={3} value={notes} maxLength={2000} onChange={(e) => setNotes(e.target.value)} />
        </Field>
      </div>
    </Dialog>
  );
}

function ConfirmDialog({ title, text, switchLabel, confirm, onClose, onConfirm, testId }: { title: string; text: string; switchLabel: string; confirm: string; onClose: () => void; onConfirm: (flag: boolean) => Promise<unknown>; testId: string }) {
  const { t } = useI18n();
  const [flag, setFlag] = useState(true);
  const [busy, setBusy] = useState(false);
  return (
    <Dialog
      title={title}
      onClose={onClose}
      testId={testId}
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={() => { setBusy(true); onConfirm(flag).catch(() => {}).finally(() => setBusy(false)); }} data-testid={`${testId}-confirm`}>{confirm}</Button></>}
    >
      <div className="stack">
        <p>{text}</p>
        <Switch checked={flag} onChange={setFlag} label={switchLabel} testId={`${testId}-switch`} />
      </div>
    </Dialog>
  );
}

function ReasonDialog({ title, hint, confirm, onClose, onConfirm, testId }: { title: string; hint: string; confirm: string; onClose: () => void; onConfirm: (reason: string | null) => Promise<unknown>; testId: string }) {
  const { t } = useI18n();
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <Dialog
      title={title}
      description={hint}
      onClose={onClose}
      testId={testId}
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="danger" loading={busy} onClick={() => { setBusy(true); onConfirm(reason.trim() || null).catch(() => {}).finally(() => setBusy(false)); }} data-testid={`${testId}-confirm`}>{confirm}</Button></>}
    >
      <Field label={t("plan.reason")} optional>
        <TextInput value={reason} maxLength={300} onChange={(e) => setReason(e.target.value)} data-testid={`${testId}-reason`} />
      </Field>
    </Dialog>
  );
}

/** The printed estimate (5.7) on the shared print engine: language, validity, notes, a live preview, then issue (TP-…) and print. */
function EstimateDialog({ patient, plans, calendar, numbering, onClose, onIssued }: { patient: PatientInfo; plans: TreatmentPlanInfo[]; calendar: CalendarSystem; numbering: ToothNumbering; onClose: () => void; onIssued: () => void }) {
  const { t, err, lang: uiLang } = useI18n();
  const catalog = useCatalog();
  const letterhead = useLetterhead();
  const [language, setLanguage] = useState<Language>(patient.preferred_language ?? uiLang);
  const [days, setDays] = useState("30");
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [issued, setIssued] = useState<DocumentInfo | null>(null);
  const daysN = days.trim() ? Number(latinDigits(days)) : null;
  const daysError = daysN !== null && (!Number.isInteger(daysN) || daysN < 1 || daysN > 365) ? t("rule.rest_days_range") : null;
  if (!letterhead || !catalog) return null;
  const snapshot = (p: TreatmentPlanInfo): PlanSnapshot => ({
    plan_id: p.id,
    title: p.title,
    total: p.total,
    items: p.items.filter((i) => i.status !== "cancelled").map((i) => {
      const s = catalog.services.find((x) => x.id === i.service_id);
      return { phase: i.phase, code: s?.code ?? "", service: s ? serviceName(s, catalog, language) : "", teeth: i.teeth, surfaces: i.surfaces, sessions: i.sessions_planned, units: i.units, unit_price: i.unit_price, total: i.unit_price * i.units, status: i.status };
    }),
  });
  const draft: DocumentInfo = {
    id: "draft", number: t("rx.draftNumber"), kind: "treatment_plan", patient_id: patient.id, patient: docPatient(patient), doctor_id: null, doctor: null, appointment_id: null,
    language, paper: "a4", content: { kind: "treatment_plan", plan_ids: plans.map((p) => p.id), plans: plans.map(snapshot), numbering, valid_days: daysN, notes: notes.trim() || null },
    issued_at: new Date().toISOString(), issued_by_name: null, print_count: 0, last_printed_at: null, attachment_id: null, status: "issued", void_reason: null, version: 0,
  };
  const issue = async () => {
    if (daysError) return;
    setBusy(true);
    setError("");
    try {
      const d = await rpc("documents.issue", { patient_id: patient.id, doctor_id: null, appointment_id: null, language, paper: "a4", content: { kind: "treatment_plan", plan_ids: plans.map((p) => p.id), plans: [], numbering, valid_days: daysN, notes: notes.trim() || null } });
      setIssued(d);
      onIssued();
    } catch (x) {
      if (!isSessionError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(false);
    }
  };
  if (issued) {
    return (
      <DocumentPreview
        title={`${t("doc.kind.treatment_plan")} ${issued.number}`}
        paper="a4"
        fileStem={issued.number}
        sheet={<DocumentSheet doc={issued} letterhead={letterhead} calendar={calendar} />}
        onPrinted={async (pdf) => { const after = await rpc("documents.mark_printed", { id: issued.id, pdf }); setIssued(after); }}
        onClose={onClose}
        testId="estimate-preview"
      />
    );
  }
  return (
    <Dialog
      title={plans.length > 1 ? t("plan.printCompare") : t("plan.print")}
      description={patient.full_name}
      onClose={onClose}
      wide
      testId="estimate-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Printer} loading={busy} onClick={issue} data-testid="estimate-issue">{t("plan.issueEstimate")}</Button></>}
    >
      <div className="editor-layout">
        <div className="stack editor-main">
          {error && <Notice tone="danger">{error}</Notice>}
          <LanguagePick value={language} onChange={setLanguage} testId="estimate-language" />
          <Field label={t("plan.validDays")} hint={t("plan.validDaysHint")} optional error={daysError}>
            <TextInput inputMode="numeric" dir="ltr" value={days} onChange={(e) => setDays(e.target.value)} data-testid="estimate-days" />
          </Field>
          <Field label={t("plan.estimateNotes")} optional>
            <Textarea rows={3} value={notes} maxLength={1000} onChange={(e) => setNotes(e.target.value)} data-testid="estimate-notes" />
          </Field>
          <Notice tone="info">{t("plan.estimateFrozen")}</Notice>
        </div>
        <LivePreview paper="a4"><DocumentSheet doc={draft} letterhead={letterhead} calendar={calendar} /></LivePreview>
      </div>
    </Dialog>
  );
}

export { ItemRow };
