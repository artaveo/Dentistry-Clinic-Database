import { useEffect, useMemo, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { CalendarCheck2, Check, ChevronDown, ClipboardPlus, History, Plus, Stethoscope, Trash2, Undo2, X } from "lucide-react";
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
import { Badge, Notice } from "../../../ui/Feedback";
import { Dialog, Popover } from "../../../ui/Overlay";
import { useToast } from "../../../ui/Toast";
import { toothInfo } from "../../../chart/geometry";
import type { Surface } from "../../../chart/geometry";
import { scopeFits } from "../../../chart/model";
import { teethLabel, toothLabel } from "../../../chart/numbering";
import { isSolo, useScheduling } from "../../reception/useScheduling";
import { ServicePicker } from "./ServicePicker";
import { ITEM_TONE, amountInput, itemService, parseAmount, sessionText, surfacesText } from "./labels";
import { SessionDots } from "./SessionDots";

export type ChartMode = "finding" | "plan" | "done";
export type QuickPick = "upper" | "lower" | "q1" | "q2" | "q3" | "q4" | "all" | "none";

export const MODES: { value: ChartMode; icon: LucideIcon; state: "existing" | "planned" | "done" }[] = [
  { value: "finding", icon: Stethoscope, state: "existing" },
  { value: "plan", icon: ClipboardPlus, state: "planned" },
  { value: "done", icon: CalendarCheck2, state: "done" },
];

/**
 * Under the chart, kept at the bottom of the screen while the chart is in view (v0.6.1): ① what is
 * selected (or how to select), quick selection of a jaw or quadrant, and ② the three kinds of record in
 * the chart's own colours — an existing condition (blue), work for the plan (red), work done today
 * (green). Each opens the record dialog, so the chart never scrolls away.
 */
export function ChartSelectionBar(p: { selected: number[]; surfaces: Surface[]; numbering: ToothNumbering; canEdit: boolean; onQuick: (k: QuickPick) => void; onOpen: (m: ChartMode) => void }) {
  const { t, lang } = useI18n();
  const [quick, setQuick] = useState(false);
  const has = p.selected.length > 0;
  return (
    <div className="chart-selbar" data-testid="chart-selbar">
      <span className={`bench-step-n ${has ? "on" : ""}`} aria-hidden>{digits(1, lang)}</span>
      {has ? (
        <span className="chart-selection" data-testid="chart-selection" aria-live="polite">
          <span className="bench-teeth"><bdi>{teethLabel(p.selected, p.numbering, lang)}</bdi></span>
          {p.surfaces.length > 0 && <Badge tone="accent">{surfacesText(p.surfaces.join(""), p.selected[0])}</Badge>}
          <IconButton icon={X} size="sm" label={t("chart.clearSelection")} onClick={() => p.onQuick("none")} data-testid="chart-clear" />
        </span>
      ) : (
        <span className="bench-hint">{t("chart.step.teethHint")}</span>
      )}
      <Popover
        open={quick}
        onClose={() => setQuick(false)}
        align="start"
        width={280}
        testId="chart-quick"
        trigger={<Button size="sm" variant="subtle" icon={ChevronDown} onClick={() => setQuick((x) => !x)} data-testid="chart-quick-open">{t("chart.quick")}</Button>}
      >
        <div className="chart-quick" role="group" aria-label={t("chart.quick")}>
          {(["upper", "lower", "q1", "q2", "q3", "q4", "all"] as const).map((k) => (
            <button key={k} type="button" className="chip-toggle" onClick={() => { p.onQuick(k); setQuick(false); }} data-testid={`chart-quick-${k}`}>{t(`chart.quick.${k}`)}</button>
          ))}
        </div>
      </Popover>
      <span className="grow" />
      {p.canEdit && (
        <span className="chart-selbar-modes" role="group" aria-label={t("chart.step.mode")}>
          <span className="bench-step-n" aria-hidden>{digits(2, lang)}</span>
          {MODES.map((m) => (
            <Button key={m.value} size="sm" variant="secondary" className={`selbar-mode st-${m.state}`} icon={m.icon} onClick={() => p.onOpen(m.value)} data-testid={`chart-mode-${m.value}`}>
              {t(`chart.mode.${m.value}`)}
            </Button>
          ))}
        </span>
      )}
    </div>
  );
}

/**
 * The record dialog (v0.6.1): the teeth in its title, ① the kind of record (switchable here too, with a
 * one-line explanation each), ② which condition or service, then one save button that names the work
 * and the teeth. Rarely changed options (plan, phase, doctor, details) wait behind «گزینه‌های بیشتر».
 * Saving closes it, so the result is seen on the chart at once.
 */
export function ChartRecordDialog(p: {
  patient: PatientInfo;
  chart: ChartInfo;
  clinic: ClinicProfile | null;
  selected: number[];
  surfaces: Surface[];
  numbering: ToothNumbering;
  mode: ChartMode;
  onMode: (m: ChartMode) => void;
  workingPlanId: string | null;
  onWorkingPlan: (id: string | null) => void;
  onServicePicked: (s: ServiceInfo) => void;
  onPickSurface: (s: Surface) => void;
  onChanged: () => void;
  onClose: () => void;
}) {
  const { t, lang } = useI18n();
  const where = teethLabel(p.selected, p.numbering, lang);
  const saved = () => {
    p.onChanged();
    p.onClose();
  };
  return (
    <Dialog
      wide
      testId="chart-actions"
      onClose={p.onClose}
      title={where ? <>{t(p.selected.length === 1 ? "chart.recordForTooth" : "chart.recordFor")} <bdi className="bench-teeth">{where}</bdi></> : t("chart.recordTitle")}
      description={
        !where
          ? t("chart.recordNoTeeth")
          : p.selected.length === 1
            ? `${t(`chart.type.${toothInfo(p.selected[0]).type}`)} · ${t(`chart.quadrant.${toothInfo(p.selected[0]).quadrant}`)}`
            : undefined
      }
    >
      <ol className="bench-steps">
        <Step n={1} done title={t("chart.step.mode")}>
          <div className="bench-modes" role="radiogroup" aria-label={t("chart.step.mode")}>
            {MODES.map((m) => (
              <button
                key={m.value}
                type="button"
                role="radio"
                aria-checked={p.mode === m.value}
                className={`bench-mode st-${m.state}`}
                onClick={() => p.onMode(m.value)}
                title={t(`chart.mode.${m.value}Desc`)}
                data-testid={`record-mode-${m.value}`}
              >
                <span className="bench-mode-icon" aria-hidden><m.icon size={16} /></span>
                <span className="bench-mode-title">{t(`chart.mode.${m.value}`)}</span>
              </button>
            ))}
          </div>
          <p className="bench-hint">{t(`chart.mode.${p.mode}Desc`)}</p>
        </Step>
        <Step n={2} done={false} title={t(p.mode === "finding" ? "chart.step.condition" : "chart.step.service")} last>
          {p.mode === "finding" ? <FindingForm {...p} onSaved={saved} /> : <ItemForm key={p.mode} {...p} onSaved={saved} done={p.mode === "done"} />}
        </Step>
      </ol>
    </Dialog>
  );
}

function Step({ n, done, title, last, children }: { n: number; done: boolean; title: string; last?: boolean; children: React.ReactNode }) {
  const { lang } = useI18n();
  return (
    <li className={`bench-step ${done ? "done" : ""} ${last ? "last" : ""}`}>
      <span className="bench-step-n" aria-hidden>{done ? <Check size={14} strokeWidth={3} /> : digits(n, lang)}</span>
      <div className="bench-step-body">
        <h3 className="bench-step-title">{title}</h3>
        {children}
      </div>
    </li>
  );
}

const SURFACES: Surface[] = ["B", "M", "O", "D", "L"];

/**
 * The tooth's five surfaces as a small cross, the way the biting surface is drawn on the chart: buccal on
 * top, lingual below, mesial towards the midline. The same selection as clicking the chart.
 */
function SurfacePicker({ tooth, surfaces, onSurface }: { tooth: number; surfaces: Surface[]; onSurface: (s: Surface) => void }) {
  const { t } = useI18n();
  const info = toothInfo(tooth);
  // On the chart the patient's right is on the left: mesial points right in quadrants 1 and 4.
  const mesialRight = info.quadrant === 1 || info.quadrant === 4;
  return (
    <div className={`surface-cross ${mesialRight ? "m-right" : "m-left"}`} role="group" aria-label={t("chart.surfaces")} dir="ltr" data-testid="surface-picker">
      {SURFACES.map((s) => (
        <button
          key={s}
          type="button"
          className={`surface-btn s-${s}`}
          aria-pressed={surfaces.includes(s)}
          title={t(`chart.surface.${s}`)}
          onClick={() => onSurface(s)}
          data-testid={`surface-pick-${s}`}
        >
          {surfacesText(s, tooth)}
        </button>
      ))}
    </div>
  );
}

/** Surfaces needed: the cross and its hint; with several teeth the same surfaces go on each, drawn for the first. */
function SurfaceNeed({ testId, selected, surfaces, onSurface }: { testId: string; selected: number[]; surfaces: Surface[]; onSurface: (s: Surface) => void }) {
  const { t } = useI18n();
  return (
    <div className="surface-need" data-testid={testId}>
      {selected.length > 0 && <SurfacePicker tooth={selected[0]} surfaces={surfaces} onSurface={onSurface} />}
      <p className="bench-hint">{t(surfaces.length ? "chart.surfacesHint" : "chart.pickSurfaces")}</p>
    </div>
  );
}

function FindingForm(p: { patient: PatientInfo; selected: number[]; surfaces: Surface[]; numbering: ToothNumbering; onPickSurface: (s: Surface) => void; onSaved: () => void }) {
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
      p.onSaved();
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
      {condition?.needs_surface && (needSurface ? (
        <SurfaceNeed testId="finding-need-surface" selected={p.selected} surfaces={p.surfaces} onSurface={p.onPickSurface} />
      ) : (
        <SurfaceNeed testId="finding-surfaces" selected={p.selected} surfaces={p.surfaces} onSurface={p.onPickSurface} />
      ))}
      {condition && (
        <Field label={t("chart.note")} optional>
          <TextInput value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} data-testid="finding-note" />
        </Field>
      )}
      {error && <Notice tone="danger" testId="finding-error">{error}</Notice>}
      <Button variant="primary" block icon={Plus} loading={busy} disabled={!condition || !p.selected.length || needSurface} onClick={save} data-testid="finding-save">
        {!p.selected.length
          ? t("chart.pickTeeth")
          : condition
            ? t("chart.recordFindingAs").replace("{what}", tr(condition.name, lang)).replace("{teeth}", teethLabel(p.selected, p.numbering, lang))
            : t("chart.pickCondition")}
      </Button>
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
  onPickSurface: (s: Surface) => void;
  onSaved: () => void;
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
  const needSurface = !!service?.needs_surface && service.tooth_scope === "tooth";
  const surfaceProblem = needSurface && !p.surfaces.length;
  const priceMinor = parseAmount(price, latinDigits);
  const sessionsN = Number(latinDigits(sessions));
  const errors = {
    price: priceMinor === null ? t("rule.item_price") : null,
    sessions: !Number.isInteger(sessionsN) || sessionsN < 1 || sessionsN > 60 ? t("rule.item_sessions") : null,
  };
  const splits = scope === "tooth" && p.selected.length > 1;
  const total = (priceMinor ?? 0) * (scope === "tooth" ? Math.max(1, p.selected.length) : scope === "teeth" ? Math.max(1, p.selected.length) : 1);
  const plan = plans.find((x) => x.id === planId);
  const doctor = doctors?.find((d) => d.id === doctorId);
  const moreSummary = [
    !p.done && (plan ? plan.title : t("chart.newPlan")),
    !p.done && t("chart.phaseN").replace("{n}", digits(phase, lang)),
    !solo && doctors && doctors.length > 1 && doctor?.full_name,
    details.trim(),
  ].filter(Boolean).join(" · ");

  const add = async () => {
    setError("");
    if (!service || teethProblem || surfaceProblem || errors.price || errors.sessions) return;
    setBusy(true);
    try {
      let planFor: string | null = null;
      if (!p.done) {
        planFor = planId || null;
        if (!planFor) {
          const created = await rpc("plans.create", { patient_id: p.patient.id, title: t("plan.defaultTitle").replace("{n}", digits(p.chart.plans.length + 1, lang)), notes: null, template_id: null, teeth: [] });
          planFor = created.id;
          p.onWorkingPlan(planFor);
        }
      }
      const teeth = scope === "mouth" || scope === "none" ? (service.chart_symbol === "bracket" || service.chart_symbol === "denture" ? p.selected : []) : p.selected;
      await rpc("treatment_items.add", {
        patient_id: p.patient.id,
        plan_id: planFor,
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
      p.onSaved();
    } catch (x) {
      if (!isSessionError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(false);
    }
  };

  const name = service ? serviceName(service, catalog, lang) : "";
  const where = teethLabel(p.selected, p.numbering, lang);
  return (
    <div className="stack" data-testid={p.done ? "done-form" : "item-form"}>
      <ServicePicker catalog={catalog} selectedTeeth={p.selected} value={service?.id ?? null} onPick={pick} testId={p.done ? "done-service" : "item-service"} />
      {service && (
        <div className="item-form-body stack">
          <div className="picked-service" data-testid="picked-service">
            <span className="cell-strong">{name}</span>
            <span className="subtle t-caption">
              {t(`catalog.scope.${service.tooth_scope}`)}
              {p.selected.length > 0 && <> · <bdi>{where}</bdi></>}
              {needSurface && p.surfaces.length > 0 && ` · ${surfacesText(p.surfaces.join(""), p.selected[0])}`}
            </span>
          </div>
          {teethProblem && <Notice tone="warning" testId="item-teeth-problem">{teethProblem}</Notice>}
          {needSurface && !teethProblem && (
            <SurfaceNeed testId={surfaceProblem ? "item-surface-problem" : "item-surfaces"} selected={p.selected} surfaces={p.surfaces} onSurface={p.onPickSurface} />
          )}
          {splits && <Notice tone="info">{t("chart.splitHint").replace("{n}", digits(p.selected.length, lang))}</Notice>}
          <div className="grid-2">
            <Field label={t("plan.unitPrice")} hint={service.price ? t("plan.catalogPrice").replace("{price}", formatAmount(service.price, lang)) : t("plan.noCatalogPrice")} error={errors.price}>
              <TextInput inputMode="numeric" suffix={lang === "en" ? "AFN" : "افغانی"} value={price} onChange={(e) => setPrice(e.target.value)} data-testid="item-price" />
            </Field>
            <Field label={t("plan.sessions")} hint={t("plan.sessionsHint")} error={errors.sessions}>
              <TextInput inputMode="numeric" dir="ltr" value={sessions} onChange={(e) => setSessions(e.target.value)} data-testid="item-sessions" />
            </Field>
          </div>
          {p.done && (
            <Field label={t("plan.result")} optional>
              <TextInput value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} data-testid="item-note" />
            </Field>
          )}
          <details className="bench-more" data-testid="item-more">
            <summary>
              <ChevronDown size={16} aria-hidden className="bench-more-chevron" />
              <span className="bench-more-title">{t("chart.more")}</span>
              {moreSummary && <span className="bench-more-summary">{moreSummary}</span>}
            </summary>
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
            </div>
          </details>
          {error && <Notice tone="danger" testId="item-error">{error}</Notice>}
          {total > 0 && (
            <div className="bench-total">
              <span className="subtle">{t("plan.estimate")}</span>
              <b className="num">{formatAmount(total, lang)}</b>
            </div>
          )}
          <Button variant="primary" block icon={p.done ? CalendarCheck2 : Plus} loading={busy} onClick={add} data-testid={p.done ? "done-save" : "item-add"}>
            {(p.done ? t("chart.recordDoneAs") : t("chart.addToPlanAs")).replace("{what}", name).replace("{teeth}", where ? ` — ${where}` : "")}
          </Button>
        </div>
      )}
    </div>
  );
}

/** What is on one tooth now — its conditions and its work — with its history and, in a mixed mouth, baby or adult. */
export function ToothDetails(p: { patient: PatientInfo; chart: ChartInfo; canEdit: boolean; tooth: number; numbering: ToothNumbering; onChanged: () => void; onHistory: (fdi: number) => void; onToggleSlot: (fdi: number) => void }) {
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
    <Card className="chart-details" data-testid="chart-details">
      <div className="stack" data-testid={`tooth-details-${p.tooth}`}>
        <CardHeader
          title={<>{t("chart.tooth")} <bdi>{toothLabel(p.tooth, p.numbering, lang)}</bdi></>}
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
    </Card>
  );
}
