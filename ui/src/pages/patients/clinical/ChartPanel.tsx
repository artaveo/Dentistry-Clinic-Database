import { useEffect, useMemo, useState } from "react";
import { CalendarCheck2, ClipboardPlus, History, MousePointerClick, Plus, Stethoscope, Trash2, Undo2 } from "lucide-react";
import type { ChartInfo, ClinicProfile, PatientInfo, ServiceInfo, ToothConditionInfo, ToothNumbering } from "../../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../../lib/api";
import { formatDate, latinDigits, digits } from "../../../lib/dates";
import { serviceName, useCatalog, useToothConditions } from "../../../lib/clinical";
import { tr } from "../../../lib/medical";
import { formatAmount } from "../../../lib/money";
import { useI18n } from "../../../i18n";
import { Button, IconButton } from "../../../ui/Button";
import { Card, CardHeader } from "../../../ui/Card";
import { Segmented } from "../../../ui/Controls";
import { Field, Select, TextInput } from "../../../ui/Field";
import { Badge, EmptyState, Notice } from "../../../ui/Feedback";
import { useToast } from "../../../ui/Toast";
import { toothInfo } from "../../../chart/geometry";
import type { Surface } from "../../../chart/geometry";
import { scopeFits } from "../../../chart/model";
import { teethLabel, toothLabel } from "../../../chart/numbering";
import { isSolo, useScheduling } from "../../reception/useScheduling";
import { ServicePicker } from "./ServicePicker";
import { ITEM_TONE, amountInput, itemService, parseAmount, sessionText, surfacesText } from "./labels";
import { SessionDots } from "./SessionDots";

type Mode = "finding" | "plan" | "done";

/**
 * Under the chart: what to do with the selected teeth — record an existing condition (the blue layer),
 * add a service to a plan (red), or record a treatment done today (green) — and, for one tooth, what is
 * on it and its history.
 */
export function ChartPanel(p: {
  patient: PatientInfo;
  chart: ChartInfo;
  clinic: ClinicProfile | null;
  canEdit: boolean;
  selected: number[];
  surfaces: Surface[];
  numbering: ToothNumbering;
  workingPlanId: string | null;
  onWorkingPlan: (id: string | null) => void;
  onServicePicked: (s: ServiceInfo) => void;
  onChanged: () => void;
  onHistory: (fdi: number) => void;
  onToggleSlot: (fdi: number) => void;
}) {
  const { t, lang } = useI18n();
  const [mode, setMode] = useState<Mode>("plan");
  const one = p.selected.length === 1 ? p.selected[0] : null;
  return (
    <div className="chart-panels">
      <Card className="chart-actions" data-testid="chart-actions">
        <CardHeader
          icon={MousePointerClick}
          title={p.selected.length ? t("chart.actionsFor").replace("{teeth}", teethLabel(p.selected, p.numbering, lang)) : t("chart.actions")}
          description={p.selected.length ? undefined : t("chart.actionsHint")}
        />
        {p.canEdit ? (
          <div className="stack">
            <Segmented<Mode>
              value={mode}
              onChange={setMode}
              label={t("chart.actions")}
              block
              options={[
                { value: "plan", label: t("chart.mode.plan"), icon: ClipboardPlus, testId: "chart-mode-plan" },
                { value: "done", label: t("chart.mode.done"), icon: CalendarCheck2, testId: "chart-mode-done" },
                { value: "finding", label: t("chart.mode.finding"), icon: Stethoscope, testId: "chart-mode-finding" },
              ]}
            />
            {mode === "finding" ? <FindingForm {...p} /> : <ItemForm key={mode} {...p} done={mode === "done"} />}
          </div>
        ) : (
          <Notice tone="info">{t("chart.readOnly")}</Notice>
        )}
      </Card>
      <Card className="chart-details" data-testid="chart-details">
        {one ? (
          <ToothDetails {...p} tooth={one} />
        ) : (
          <EmptyState icon={MousePointerClick} title={t("chart.detailsTitle")}>{t("chart.detailsHint")}</EmptyState>
        )}
      </Card>
    </div>
  );
}

function FindingForm(p: { patient: PatientInfo; selected: number[]; surfaces: Surface[]; numbering: ToothNumbering; onChanged: () => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const conditions = useToothConditions();
  const [condition, setCondition] = useState<ToothConditionInfo | null>(null);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const active = (conditions ?? []).filter((c) => c.is_active);
  const needSurface = !!condition?.needs_surface && !p.surfaces.length;
  const save = async () => {
    setError("");
    if (!condition || !p.selected.length || needSurface) return;
    setBusy(true);
    try {
      await rpc("chart.add_findings", { patient_id: p.patient.id, condition_id: condition.id, teeth: p.selected, surfaces: condition.needs_surface ? p.surfaces.join("") : "", note: note.trim() || null });
      toast.success(t("chart.findingSaved"));
      setNote("");
      p.onChanged();
    } catch (x) {
      if (!isSessionError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="stack" data-testid="finding-form">
      <div className="condition-grid" role="listbox" aria-label={t("chart.condition")}>
        {active.map((c) => (
          <button key={c.id} type="button" role="option" aria-selected={condition?.id === c.id} className="condition-option" onClick={() => setCondition(c)} data-testid={`condition-${c.code}`}>
            <span className={`condition-icon sym-${c.symbol}`} aria-hidden>{c.abbreviation ?? ""}</span>
            <span>{tr(c.name, lang)}</span>
          </button>
        ))}
      </div>
      {condition && needSurface && <Notice tone="info" testId="finding-need-surface">{t("chart.pickSurfaces")}</Notice>}
      {!p.selected.length && <Notice tone="info">{t("chart.pickTeeth")}</Notice>}
      <Field label={t("chart.note")} optional>
        <TextInput value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} data-testid="finding-note" />
      </Field>
      {error && <Notice tone="danger" testId="finding-error">{error}</Notice>}
      <div className="row">
        <span className="grow" />
        <Button variant="primary" icon={Plus} loading={busy} disabled={!condition || !p.selected.length || needSurface} onClick={save} data-testid="finding-save">
          {t("chart.recordFinding").replace("{teeth}", teethLabel(p.selected, p.numbering, lang) || "—")}
        </Button>
      </div>
    </div>
  );
}

function ItemForm(p: {
  patient: PatientInfo;
  chart: ChartInfo;
  clinic: ClinicProfile | null;
  selected: number[];
  surfaces: Surface[];
  numbering: ToothNumbering;
  workingPlanId: string | null;
  onWorkingPlan: (id: string | null) => void;
  onServicePicked: (s: ServiceInfo) => void;
  onChanged: () => void;
  done: boolean;
}) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const catalog = useCatalog();
  const { doctors } = useScheduling();
  const solo = isSolo(p.clinic, doctors);
  const [service, setService] = useState<ServiceInfo | null>(null);
  const plans = p.chart.plans.filter((x) => x.status !== "cancelled" && x.status !== "completed");
  const [planId, setPlanId] = useState<string>(p.workingPlanId ?? plans[plans.length - 1]?.id ?? "");
  const [phase, setPhase] = useState("1");
  const [price, setPrice] = useState("");
  const [sessions, setSessions] = useState("1");
  const [details, setDetails] = useState("");
  const [note, setNote] = useState("");
  const [doctorId, setDoctorId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!doctorId && doctors?.length) setDoctorId(doctors[0].id);
  }, [doctors]);
  useEffect(() => {
    if (p.workingPlanId) setPlanId(p.workingPlanId);
  }, [p.workingPlanId]);

  const pick = (s: ServiceInfo) => {
    setService(s);
    setPrice(amountInput(s.price));
    setSessions(String(s.sessions));
    p.onServicePicked(s);
  };
  const scope = service?.tooth_scope;
  const teethProblem = service && !scopeFits(service.tooth_scope, p.selected) ? t(`chart.scopeHint.${service.tooth_scope}`) : null;
  const surfaceProblem = service?.needs_surface && service.tooth_scope === "tooth" && !p.surfaces.length ? t("chart.pickSurfaces") : null;
  const priceMinor = parseAmount(price, latinDigits);
  const sessionsN = Number(latinDigits(sessions));
  const errors = {
    price: priceMinor === null ? t("rule.item_price") : null,
    sessions: !Number.isInteger(sessionsN) || sessionsN < 1 || sessionsN > 60 ? t("rule.item_sessions") : null,
  };
  const splits = scope === "tooth" && p.selected.length > 1;
  const total = (priceMinor ?? 0) * (scope === "tooth" ? Math.max(1, p.selected.length) : scope === "teeth" ? Math.max(1, p.selected.length) : 1);

  const add = async () => {
    setError("");
    if (!service || teethProblem || surfaceProblem || errors.price || errors.sessions) return;
    setBusy(true);
    try {
      let plan: string | null = null;
      if (!p.done) {
        plan = planId || null;
        if (!plan) {
          const created = await rpc("plans.create", { patient_id: p.patient.id, title: t("plan.defaultTitle").replace("{n}", digits(p.chart.plans.length + 1, lang)), notes: null, template_id: null, teeth: [] });
          plan = created.id;
          p.onWorkingPlan(plan);
        }
      }
      const teeth = scope === "mouth" || scope === "none" ? (service.chart_symbol === "bracket" || service.chart_symbol === "denture" ? p.selected : []) : p.selected;
      await rpc("treatment_items.add", {
        patient_id: p.patient.id,
        plan_id: plan,
        service_id: service.id,
        teeth,
        surfaces: service.needs_surface ? p.surfaces.join("") : "",
        doctor_id: solo ? (doctors?.[0]?.id ?? null) : doctorId || null,
        unit_price: priceMinor,
        phase: Number(phase),
        sessions: sessionsN,
        details: details.trim() || null,
        notes: p.done ? note.trim() || null : null,
        est_start: null,
        est_end: null,
        perform_now: p.done,
        appointment_id: null,
      });
      toast.success(p.done ? t("chart.doneSaved") : t("chart.itemAdded"));
      setDetails("");
      setNote("");
      p.onChanged();
    } catch (x) {
      if (!isSessionError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="stack" data-testid={p.done ? "done-form" : "item-form"}>
      <ServicePicker catalog={catalog} selectedTeeth={p.selected} value={service?.id ?? null} onPick={pick} testId={p.done ? "done-service" : "item-service"} />
      {service && (
        <div className="item-form-body stack">
          <div className="picked-service" data-testid="picked-service">
            <span className="cell-strong">{serviceName(service, catalog, lang)}</span>
            <span className="subtle t-caption">
              {t(`catalog.scope.${service.tooth_scope}`)}
              {p.selected.length > 0 && ` · ${teethLabel(p.selected, p.numbering, lang)}`}
              {p.surfaces.length > 0 && ` · ${surfacesText(p.surfaces.join(""), p.selected[0])}`}
            </span>
          </div>
          {teethProblem && <Notice tone="warning" testId="item-teeth-problem">{teethProblem}</Notice>}
          {surfaceProblem && <Notice tone="warning" testId="item-surface-problem">{surfaceProblem}</Notice>}
          {splits && <Notice tone="info">{t("chart.splitHint").replace("{n}", digits(p.selected.length, lang))}</Notice>}
          <div className="grid-2">
            {!p.done && (
              <Field label={t("chart.plan")} hint={plans.length ? undefined : t("chart.newPlanHint")}>
                <Select value={planId} onChange={(e) => { setPlanId(e.target.value); p.onWorkingPlan(e.target.value || null); }} data-testid="item-plan">
                  <option value="">{t("chart.newPlan")}</option>
                  {plans.map((x) => <option key={x.id} value={x.id}>{x.title} · {t(`plan.status.${x.status}`)}</option>)}
                </Select>
              </Field>
            )}
            {!p.done && (
              <Field label={t("plan.phase")} hint={t("plan.phaseHint")}>
                <Segmented value={phase} onChange={setPhase} label={t("plan.phase")} options={["1", "2", "3", "4", "5"].map((n) => ({ value: n, label: digits(n, lang), testId: `item-phase-${n}` }))} />
              </Field>
            )}
            <Field label={t("plan.unitPrice")} hint={service.price ? t("plan.catalogPrice").replace("{price}", formatAmount(service.price, lang)) : t("plan.noCatalogPrice")} error={errors.price}>
              <TextInput inputMode="numeric" suffix={lang === "en" ? "AFN" : "افغانی"} value={price} onChange={(e) => setPrice(e.target.value)} data-testid="item-price" />
            </Field>
            <Field label={t("plan.sessions")} hint={t("plan.sessionsHint")} error={errors.sessions}>
              <TextInput inputMode="numeric" dir="ltr" value={sessions} onChange={(e) => setSessions(e.target.value)} data-testid="item-sessions" />
            </Field>
            {!solo && doctors && doctors.length > 1 && (
              <Field label={t("appt.doctor")}>
                <Select value={doctorId} onChange={(e) => setDoctorId(e.target.value)} data-testid="item-doctor">
                  {doctors.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
                </Select>
              </Field>
            )}
            <Field label={t("plan.details")} hint={t("plan.detailsHint")} optional>
              <TextInput value={details} maxLength={80} onChange={(e) => setDetails(e.target.value)} data-testid="item-details" />
            </Field>
            {p.done && (
              <Field label={t("plan.result")} optional>
                <TextInput value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} data-testid="item-note" />
              </Field>
            )}
          </div>
          {error && <Notice tone="danger" testId="item-error">{error}</Notice>}
          <div className="row">
            {total > 0 && <span className="subtle">{t("plan.estimate")}: <b className="num">{formatAmount(total, lang)}</b></span>}
            <span className="grow" />
            <Button variant="primary" icon={p.done ? CalendarCheck2 : Plus} loading={busy} onClick={add} data-testid={p.done ? "done-save" : "item-add"}>
              {p.done ? t("chart.recordDone") : t("chart.addToPlan")}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

function ToothDetails(p: { patient: PatientInfo; chart: ChartInfo; canEdit: boolean; tooth: number; numbering: ToothNumbering; onChanged: () => void; onHistory: (fdi: number) => void; onToggleSlot: (fdi: number) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const catalog = useCatalog();
  const conditions = useToothConditions();
  const info = toothInfo(p.tooth);
  const findings = p.chart.findings.filter((f) => f.tooth === p.tooth);
  const items = p.chart.items.filter((i) => i.teeth.includes(p.tooth));
  const plans = useMemo(() => new Map(p.chart.plans.map((x) => [x.id, x])), [p.chart.plans]);
  const mixedSlot = p.chart.dentition === "mixed" && info.n <= 5;
  const remove = async (id: string, version: number) => {
    if (!window.confirm(t("chart.removeFindingConfirm"))) return;
    try {
      await rpc("chart.remove_finding", { id, version });
      toast.success(t("chart.findingRemoved"));
      p.onChanged();
    } catch (x) {
      if (!isSessionError(x)) toast.error(err(x));
    }
  };
  return (
    <div className="stack" data-testid={`tooth-details-${p.tooth}`}>
      <CardHeader
        title={`${t("chart.tooth")} ${toothLabel(p.tooth, p.numbering, lang)}`}
        description={t(`chart.type.${info.type}`) + " · " + t(`chart.quadrant.${info.quadrant}`)}
        actions={<Button size="sm" icon={History} onClick={() => p.onHistory(p.tooth)} data-testid="tooth-history-open">{t("chart.history")}</Button>}
      />
      {mixedSlot && p.canEdit && (
        <Button size="sm" variant="subtle" icon={Undo2} onClick={() => p.onToggleSlot(p.tooth)} data-testid="tooth-toggle-primary">
          {info.primary ? t("chart.showPermanent") : t("chart.showPrimary")}
        </Button>
      )}
      <section>
        <h3 className="t-subtitle">{t("chart.findings")}</h3>
        {findings.length ? (
          <ul className="plain-list tooth-list">
            {findings.map((f) => {
              const c = conditions?.find((x) => x.id === f.condition_id);
              return (
                <li key={f.id} className="row" data-testid={`finding-${f.id}`}>
                  <span className="chart-swatch st-existing" aria-hidden />
                  <span className="cell-strong">{tr(c?.name, lang)}</span>
                  {f.surfaces && <Badge>{surfacesText(f.surfaces, f.tooth)}</Badge>}
                  <span className="subtle t-caption">{formatDate(`${f.recorded_on}T06:30:00Z`, lang)}</span>
                  {f.note && <span className="t-caption">{f.note}</span>}
                  <span className="grow" />
                  {p.canEdit && <IconButton icon={Trash2} size="sm" label={t("chart.removeFinding")} onClick={() => remove(f.id, f.version)} data-testid={`finding-remove-${f.id}`} />}
                </li>
              );
            })}
          </ul>
        ) : (
          <p className="subtle t-caption">{t("chart.noFindings")}</p>
        )}
      </section>
      <section>
        <h3 className="t-subtitle">{t("chart.work")}</h3>
        {items.length ? (
          <ul className="plain-list tooth-list">
            {items.map((i) => (
              <li key={i.id} className="row" data-testid={`tooth-item-${i.id}`}>
                <Badge tone={ITEM_TONE[i.status]} dot>{t(`item.status.${i.status}`)}</Badge>
                <span className="cell-strong">{itemService(catalog, i, lang)}</span>
                {i.surfaces && <Badge>{surfacesText(i.surfaces, p.tooth)}</Badge>}
                {i.sessions_planned > 1 && <SessionDots item={i} />}
                <span className="subtle t-caption">{[sessionText(t, lang, i), i.plan_id ? plans.get(i.plan_id)?.title : t("plan.noPlan")].filter(Boolean).join(" · ")}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="subtle t-caption">{t("chart.noWork")}</p>
        )}
      </section>
    </div>
  );
}
