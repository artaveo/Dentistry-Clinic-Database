// «لابراتوار» — work sent to a dental lab (8.6): crown, bridge, denture … from the day it is planned to
// the day it is fitted. A case that is late is flagged; its cost counts towards what the clinic owes the
// lab, and the report sets that cost beside what the patient's treatment was invoiced for.
import { useEffect, useState } from "react";
import { Ban, CalendarClock, Check, FlaskConical, PackageCheck, Pencil, Plus, Send, TriangleAlert } from "lucide-react";
import type { CalendarSystem, LabCaseInfo, LabReport, LabStatus, TreatmentItemInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { afnInput, amountRule, parseAfn } from "../../lib/billing";
import { addDays, monthRange, todayIso } from "../../lib/calendar";
import { serviceName, useCatalog } from "../../lib/clinical";
import { digits, formatDate } from "../../lib/dates";
import { LAB_TONE, labQ, parseTeeth, teethText } from "../../lib/inventory";
import { useLive } from "../../lib/lan";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader, Stat } from "../../ui/Card";
import { Segmented } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { AmountInput } from "../billing/common";
import { PatientPicker } from "../reception/PatientPicker";
import type { PickedPatient } from "../reception/PatientPicker";
import { useScheduling } from "../reception/useScheduling";
import { useAction, useSupplierOptions } from "./common";
import type { InventoryPerms } from "./common";

type View = "open" | "late" | "done" | "all";

const NEXT: Partial<Record<LabStatus, { to: LabStatus; icon: typeof Send }>> = {
  planned: { to: "sent", icon: Send },
  sent: { to: "received", icon: PackageCheck },
  received: { to: "delivered", icon: Check },
};

export function LabTab({ perms, calendar, canCreatePatients }: { perms: InventoryPerms; calendar: CalendarSystem; canCreatePatients: boolean }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const labs = useSupplierOptions("lab");
  const [view, setView] = useState<View>("open");
  const [lab, setLab] = useState("");
  const [rows, setRows] = useState<LabCaseInfo[] | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const [editing, setEditing] = useState<LabCaseInfo | "new" | null>(null);
  const [moving, setMoving] = useState<{ item: LabCaseInfo; to: LabStatus } | null>(null);
  const reload = () => setTick((x) => x + 1);
  useLive(["lab_cases", "suppliers"], reload);
  const write = perms.manage || perms.clinicalEdit;

  useEffect(() => {
    rpc("lab_cases.list", labQ({ lab_id: lab || null, delayed_only: view === "late" }))
      .then((r) => {
        // Open = still on its way (a case back from the lab waits to be fitted); done = fitted or cancelled.
        setRows(view === "open" ? r.filter((c) => c.status !== "delivered" && c.status !== "cancelled") : view === "done" ? r.filter((c) => c.status === "delivered" || c.status === "cancelled") : r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [view, lab, tick]);
  if (error && !rows) return <ErrorState message={error} onRetry={reload} />;
  const money = (v: number) => formatAmount(v, lang);
  const late = rows?.filter((c) => c.delayed).length ?? 0;
  return (
    <div className="stack" data-testid="lab-tab">
      {late > 0 && view !== "late" && (
        <Notice tone="warning" title={t("labcase.lateTitle")} testId="lab-late-notice">
          {t("labcase.lateText").replace("{n}", digits(late, lang))}
        </Notice>
      )}
      <Card flush>
        <CardHeader
          icon={FlaskConical}
          title={t("inventory.tab.lab")}
          description={t("labcase.hint")}
          actions={write ? <Button variant="primary" icon={Plus} onClick={() => setEditing("new")} data-testid="lab-add-open">{t("labcase.new")}</Button> : undefined}
        />
        <div className="row toolbar">
          <Segmented<View>
            value={view}
            onChange={setView}
            label={t("labcase.view")}
            options={[
              { value: "open", label: t("labcase.view.open"), testId: "lab-view-open" },
              { value: "late", label: t("labcase.view.late"), testId: "lab-view-late" },
              { value: "done", label: t("labcase.view.done"), testId: "lab-view-done" },
              { value: "all", label: t("labcase.view.all"), testId: "lab-view-all" },
            ]}
          />
          <Select value={lab} onChange={(e) => setLab(e.target.value)} aria-label={t("labcase.lab")} data-testid="lab-filter">
            <option value="">{t("labcase.allLabs")}</option>
            {labs.list?.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}
          </Select>
        </div>
        <div className="table-wrap">
          <table className="table" data-testid="lab-table">
            <thead>
              <tr>
                <th>{t("doc.patient")}</th>
                <th>{t("labcase.work")}</th>
                <th>{t("labcase.lab")}</th>
                <th>{t("labcase.dates")}</th>
                <th>{t("labcase.status")}</th>
                <th className="num">{t("labcase.cost")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {!rows && <SkeletonRows cols={7} />}
              {rows?.map((c) => {
                const next = NEXT[c.status];
                const open = c.status === "planned" || c.status === "sent" || c.status === "received";
                return (
                  <tr key={c.id} data-testid={`lab-row-${c.id}`}>
                    <td className="nowrap">
                      <span className="cell-strong">{c.patient_name}</span>
                      <div className="t-caption subtle"><bdi className="ltr num">{c.number}</bdi></div>
                    </td>
                    <td className="lab-work">
                      {c.work_type}
                      <div className="t-caption subtle">
                        {c.teeth.length > 0 && <>{t("labcase.teeth")} <bdi className="ltr num">{teethText(c.teeth)}</bdi></>}
                        {c.shade && <> · {t("labcase.shade")} <bdi className="ltr">{c.shade}</bdi></>}
                      </div>
                    </td>
                    <td className="nowrap">{c.lab_name}{c.doctor_name && <div className="t-caption subtle">{c.doctor_name}</div>}</td>
                    <td>
                      <span className="nowrap cell-strong">{c.received_date || c.expected_date ? formatDate(c.received_date ?? c.expected_date, lang, calendar) : "—"}</span>
                      <div className="t-caption subtle">{c.received_date ? t("labcase.received") : c.expected_date ? t("labcase.expected") : ""}{c.sent_date && <> · {t("labcase.sent")} <span className="nowrap">{formatDate(c.sent_date, lang, calendar)}</span></>}</div>
                    </td>
                    <td className="nowrap">
                      <Badge tone={LAB_TONE[c.status]} dot testId={`lab-status-${c.id}`}>{t(`labcase.status.${c.status}`)}</Badge>
                      {c.delayed && <div><Badge tone="danger" icon={TriangleAlert} testId={`lab-late-${c.id}`}>{t("labcase.late").replace("{n}", digits(c.days_late, lang))}</Badge></div>}
                    </td>
                    <td className="num nowrap">{c.cost ? money(c.cost) : "—"}</td>
                    <td className="cell-actions">
                      {write && next && <Button size="sm" icon={next.icon} onClick={() => setMoving({ item: c, to: next.to })} title={t(`labcase.action.${next.to}`)} data-testid={`lab-next-${c.id}`}>{t(`labcase.next.${next.to}`)}</Button>}
                      {write && open && <IconButton icon={Pencil} label={t("common.edit")} onClick={() => setEditing(c)} data-testid={`lab-edit-${c.id}`} />}
                      {write && open && <IconButton icon={Ban} label={t("labcase.action.cancelled")} onClick={() => setMoving({ item: c, to: "cancelled" })} data-testid={`lab-cancel-${c.id}`} />}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        {rows && !rows.length && <EmptyState icon={FlaskConical} title={t(view === "late" ? "labcase.none.late" : "labcase.none")}>{t("labcase.noneHint")}</EmptyState>}
      </Card>

      <LabReportCard calendar={calendar} tick={tick} />

      {editing && <LabCaseDialog item={editing === "new" ? undefined : editing} calendar={calendar} canCreatePatients={canCreatePatients} onClose={() => setEditing(null)} onDone={() => { setEditing(null); toast.success(t("labcase.saved")); reload(); }} />}
      {moving && <StatusDialog item={moving.item} to={moving.to} calendar={calendar} onClose={() => setMoving(null)} onDone={() => { setMoving(null); toast.success(t("labcase.statusSaved")); reload(); }} />}
    </div>
  );
}

// ───────────────────────────── a case ─────────────────────────────

function LabCaseDialog({ item, calendar, canCreatePatients, onClose, onDone }: { item?: LabCaseInfo; calendar: CalendarSystem; canCreatePatients: boolean; onClose: () => void; onDone: () => void }) {
  const { t, lang } = useI18n();
  const labs = useSupplierOptions("lab");
  const { doctors } = useScheduling();
  const catalog = useCatalog();
  const [patient, setPatient] = useState<PickedPatient | null>(item ? { id: item.patient_id, name: item.patient_name, number: item.patient_number } : null);
  const [patientError, setPatientError] = useState<string | null>(null);
  const [treatments, setTreatments] = useState<TreatmentItemInfo[]>([]);
  const [treatment, setTreatment] = useState(item?.treatment_item_id ?? "");
  const [expected, setExpected] = useState<string | null>(item?.expected_date ?? null);
  const { busy, error, run } = useAction();
  const form = useForm(
    { lab: item?.lab_id ?? "", doctor: item?.doctor_id ?? "", teeth: item ? teethText(item.teeth) : "", work: item?.work_type ?? "", shade: item?.shade ?? "", cost: item?.cost ? afnInput(item.cost) : "", note: item?.note ?? "" },
    { lab: (s) => (s ? null : "rule.required"), work: (s) => (s.trim() ? null : "rule.required"), teeth: (s) => (parseTeeth(s) ? null : "labcase.teethInvalid"), cost: (s) => (s.trim() ? amountRule(s, true) : null) },
  );
  useEffect(() => {
    if (!form.values.lab && labs.list?.length) form.set("lab", labs.list[0].id);
  }, [labs.list]);
  useEffect(() => {
    if (!patient || item) return;
    rpc("treatments.list", { patient_id: patient.id }).then((r) => setTreatments(r.filter((x) => x.status !== "cancelled"))).catch(() => setTreatments([]));
  }, [patient?.id]);

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    const ok = form.validate();
    if (!patient) setPatientError(t("rule.required"));
    if (!ok || !patient) return;
    const common = {
      lab_id: form.values.lab,
      doctor_id: form.values.doctor || null,
      teeth: parseTeeth(form.values.teeth) ?? [],
      work_type: form.values.work.trim(),
      shade: form.values.shade.trim() || null,
      expected_date: expected,
      cost: parseAfn(form.values.cost) ?? 0,
      note: form.values.note.trim() || null,
    };
    void run(
      async () => {
        if (item) await rpc("lab_cases.update", { id: item.id, version: item.version, ...common });
        else await rpc("lab_cases.create", { patient_id: patient.id, treatment_item_id: treatment || null, ...common });
        onDone();
      },
      (x) => form.serverError(x, { lab_id: "lab", doctor_id: "doctor", work_type: "work" }),
    );
  };
  return (
    <Dialog
      title={t(item ? "labcase.edit" : "labcase.new")}
      description={item?.number}
      onClose={onClose}
      wide
      testId="lab-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={FlaskConical} loading={busy} onClick={submit} data-testid="lab-save">{t("common.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        {item ? (
          <Field label={t("doc.patient")}><TextInput value={`${item.patient_name} · ${item.patient_number}`} readOnly /></Field>
        ) : (
          <PatientPicker value={patient} onChange={(p) => { setPatient(p); setPatientError(null); setTreatment(""); }} canCreate={canCreatePatients} error={patientError} testId="lab-patient" />
        )}
        <div className="grid-2">
          <Field label={t("labcase.lab")} error={form.error("lab") && t(form.error("lab")!)} hint={labs.list && !labs.list.length ? t("labcase.noLabs") : undefined}>
            <Select value={form.values.lab} onChange={(e) => form.set("lab", e.target.value)} data-testid="lab-lab">
              <option value="">{t("labcase.pickLab")}</option>
              {labs.list?.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}
            </Select>
          </Field>
          <Field label={t("labcase.doctor")} optional>
            <Select value={form.values.doctor} onChange={(e) => form.set("doctor", e.target.value)} data-testid="lab-doctor">
              <option value="">—</option>
              {doctors?.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
            </Select>
          </Field>
        </div>
        <div className="grid-2">
          <Field label={t("labcase.work")} hint={t("labcase.workHint")} error={form.error("work") && t(form.error("work")!)}>
            <TextInput value={form.values.work} onChange={(e) => form.set("work", e.target.value)} maxLength={100} data-testid="lab-work" />
          </Field>
          <Field label={t("labcase.teeth")} hint={t("labcase.teethHint")} error={form.error("teeth") && t(form.error("teeth")!)} optional>
            <TextInput value={form.values.teeth} onChange={(e) => form.set("teeth", e.target.value)} dir="ltr" data-testid="lab-teeth" />
          </Field>
        </div>
        <div className="grid-3">
          <Field label={t("labcase.shade")} optional><TextInput value={form.values.shade} onChange={(e) => form.set("shade", e.target.value)} maxLength={30} dir="ltr" data-testid="lab-shade" /></Field>
          <Field label={t("labcase.cost")} hint={t("labcase.costHint")} error={form.error("cost") && t(form.error("cost")!)} optional><AmountInput value={form.values.cost} onChange={(v) => form.set("cost", v)} testId="lab-cost" /></Field>
          <Field label={t("labcase.expected")} optional>
            {expected === null ? (
              <Button onClick={() => setExpected(addDays(todayIso(), 7))} data-testid="lab-expected-set"><CalendarClock aria-hidden />{t("labcase.setExpected")}</Button>
            ) : (
              <div className="row">
                <DateField value={expected} onChange={setExpected} calendar={calendar} label={t("labcase.expected")} testId="lab-expected" yearsBefore={0} yearsAfter={1} />
                <IconButton icon={Ban} label={t("stock.expiry.clear")} onClick={() => setExpected(null)} />
              </div>
            )}
          </Field>
        </div>
        {!item && patient && treatments.length > 0 && (
          <Field label={t("labcase.treatment")} hint={t("labcase.treatmentHint")} optional>
            <Select value={treatment} onChange={(e) => setTreatment(e.target.value)} data-testid="lab-treatment">
              <option value="">—</option>
              {treatments.map((x) => {
                const svc = catalog?.services.find((s) => s.id === x.service_id);
                return <option key={x.id} value={x.id}>{svc ? serviceName(svc, catalog, lang) : x.service_id}{x.teeth.length ? ` · ${teethText(x.teeth)}` : ""}</option>;
              })}
            </Select>
          </Field>
        )}
        <Field label={t("stock.item.note")} optional><Textarea rows={2} value={form.values.note} onChange={(e) => form.set("note", e.target.value)} maxLength={500} data-testid="lab-note" /></Field>
      </form>
    </Dialog>
  );
}

function StatusDialog({ item, to, calendar, onClose, onDone }: { item: LabCaseInfo; to: LabStatus; calendar: CalendarSystem; onClose: () => void; onDone: () => void }) {
  const { t } = useI18n();
  const [date, setDate] = useState(todayIso());
  const { busy, error, run } = useAction();
  const form = useForm({ cost: item.cost ? afnInput(item.cost) : "", reason: "" }, {
    cost: (s) => (to === "received" && s.trim() ? amountRule(s, true) : null),
    reason: (s) => (to === "cancelled" && !s.trim() ? "rule.required" : null),
  });
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.validate()) return;
    void run(
      async () => {
        await rpc("lab_cases.set_status", { id: item.id, version: item.version, status: to, date: to === "cancelled" ? null : date, cost: to === "received" && form.values.cost.trim() ? parseAfn(form.values.cost) : null, reason: to === "cancelled" ? form.values.reason.trim() : null });
        onDone();
      },
      (x) => form.serverError(x),
    );
  };
  return (
    <Dialog
      title={t(`labcase.action.${to}`)}
      description={`${item.number} · ${item.patient_name} · ${item.work_type}`}
      onClose={onClose}
      testId="lab-status-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant={to === "cancelled" ? "danger" : "primary"} loading={busy} onClick={submit} data-testid="lab-status-save">{t(`labcase.action.${to}`)}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        {to !== "cancelled" && <Field label={t(`labcase.date.${to}`)}><DateField value={date} onChange={setDate} calendar={calendar} label={t(`labcase.date.${to}`)} testId="lab-status-date" yearsBefore={1} yearsAfter={0} /></Field>}
        {to === "received" && (
          <Field label={t("labcase.finalCost")} hint={t("labcase.finalCostHint")} error={form.error("cost") && t(form.error("cost")!)} optional>
            <AmountInput value={form.values.cost} onChange={(v) => form.set("cost", v)} testId="lab-status-cost" autoFocus />
          </Field>
        )}
        {to === "cancelled" && (
          <Field label={t("labcase.cancelReason")} error={form.error("reason") && t(form.error("reason")!)}>
            <TextInput value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="lab-status-reason" autoFocus />
          </Field>
        )}
      </form>
    </Dialog>
  );
}

// ───────────────────────────── lab cost report ─────────────────────────────

function LabReportCard({ calendar, tick }: { calendar: CalendarSystem; tick: number }) {
  const { t, lang, err } = useI18n();
  const [from, setFrom] = useState(monthRange(todayIso(), calendar).from);
  const [to, setTo] = useState(todayIso());
  const [report, setReport] = useState<LabReport | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    if (from > to) return;
    rpc("lab_cases.report", { from, to })
      .then((r) => {
        setReport(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [from, to, tick]);
  const money = (v: number) => formatAmount(v, lang);
  return (
    <Card flush data-testid="lab-report">
      <CardHeader icon={FlaskConical} title={t("labcase.report")} description={t("labcase.reportHint")} />
      <div className="row pad" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
        <Field label={t("period.from")}><DateField value={from} onChange={setFrom} calendar={calendar} label={t("period.from")} testId="labreport-from" yearsBefore={3} yearsAfter={0} /></Field>
        <Field label={t("period.to")}><DateField value={to} onChange={setTo} calendar={calendar} label={t("period.to")} testId="labreport-to" yearsBefore={3} yearsAfter={0} /></Field>
        {from > to && <Notice tone="warning">{t("rule.period_range")}</Notice>}
      </div>
      {error && <ErrorState message={error} />}
      {report && (
        <>
          <div className="stat-grid pad">
            <Stat icon={FlaskConical} label={t("labcase.report.cost")} value={money(report.total_cost)} tone="warning" testId="labreport-cost" />
            <Stat icon={Check} label={t("labcase.report.billed")} value={money(report.total_billed)} tone="success" testId="labreport-billed" />
          </div>
          <div className="table-wrap">
            <table className="table" data-testid="labreport-table">
              <thead>
                <tr>
                  <th>{t("labcase.number")}</th>
                  <th>{t("doc.patient")}</th>
                  <th>{t("labcase.work")}</th>
                  <th>{t("labcase.lab")}</th>
                  <th>{t("labcase.received")}</th>
                  <th className="num">{t("labcase.cost")}</th>
                  <th className="num">{t("labcase.report.billedShort")}</th>
                </tr>
              </thead>
              <tbody>
                {report.rows.map((r) => (
                  <tr key={r.case_id}>
                    <td><bdi className="ltr num">{r.number}</bdi></td>
                    <td>{r.patient_name}</td>
                    <td>{r.work_type}</td>
                    <td>{r.lab_name}</td>
                    <td>{r.received_date ? formatDate(r.received_date, lang, calendar) : "—"}</td>
                    <td className="num">{money(r.cost)}</td>
                    <td className="num">{r.billed === null ? "—" : money(r.billed)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {!report.rows.length && <p className="muted pad">{t("labcase.report.none")}</p>}
        </>
      )}
    </Card>
  );
}
