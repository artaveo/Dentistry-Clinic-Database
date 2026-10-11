import { useCallback, useEffect, useMemo, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { Activity, Baby, CircleDot, Grid3x3, History, Layers, ScanLine, Smile, Tag, Tags, Wrench, X } from "lucide-react";
import type { ChartInfo, ClinicProfile, PatientInfo, ServiceInfo } from "../../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../../lib/api";
import { useCatalog, useClinicSettings, useToothConditions } from "../../../lib/clinical";
import { tr } from "../../../lib/medical";
import { useI18n } from "../../../i18n";
import { Button, IconButton } from "../../../ui/Button";
import { Card } from "../../../ui/Card";
import { Segmented } from "../../../ui/Controls";
import { Badge, ErrorState, Loading } from "../../../ui/Feedback";
import { Select } from "../../../ui/Field";
import { useToast } from "../../../ui/Toast";
import { Odontogram } from "../../../chart/Odontogram";
import type { Callout } from "../../../chart/Odontogram";
import { chartTeeth, toothInfo } from "../../../chart/geometry";
import type { Surface } from "../../../chart/geometry";
import { itemState, layersOf, marksOf, spansOf, STATES, viewForSymbol } from "../../../chart/model";
import type { ChartState, ChartView } from "../../../chart/model";
import { teethLabel, toothLabel } from "../../../chart/numbering";
import { ChartPanel } from "./ChartPanel";
import { PerioChart } from "./PerioChart";
import { ToothHistory } from "./ToothHistory";
import { itemService, sessionText, stateText, surfacesText } from "./labels";

const VIEWS: { value: ChartView; icon: LucideIcon }[] = [
  { value: "general", icon: Grid3x3 },
  { value: "ortho", icon: Activity },
  { value: "implant", icon: Wrench },
  { value: "prosth", icon: Layers },
  { value: "perio", icon: ScanLine },
];

/**
 * Patient profile → «چارت دندان» (5.2–5.5, M5): the odontogram with its specialty views, selecting
 * teeth and surfaces (quick buttons for jaws and quadrants), recording what is there, planning work
 * service-first or tooth-first, and each tooth's history. The periodontal chart is the «پریو» view.
 */
export function ChartTab({ patient, canEdit, clinic, onChanged }: { patient: PatientInfo; canEdit: boolean; clinic: ClinicProfile | null; onChanged?: () => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const catalog = useCatalog();
  const conditions = useToothConditions();
  const settings = useClinicSettings();
  const numbering = settings?.tooth_numbering ?? "fdi";
  const [chart, setChart] = useState<ChartInfo | null>(null);
  const [error, setError] = useState("");
  const [view, setView] = useState<ChartView>("general");
  const [labels, setLabels] = useState<"all" | "selected">("all");
  const [selected, setSelected] = useState<number[]>([]);
  const [surfaces, setSurfaces] = useState<Surface[]>([]);
  const [anchor, setAnchor] = useState<number | null>(null);
  const [workingPlan, setWorkingPlan] = useState<string | null>(null);
  const [history, setHistory] = useState<number | null>(null);

  const load = useCallback(
    () =>
      rpc("chart.get", { patient_id: patient.id })
        .then((c) => {
          setChart(c);
          setError("");
          // The plan the doctor is working on: the running one, else the newest proposal.
          setWorkingPlan((cur) => {
            if (cur && c.plans.some((p) => p.id === cur && p.status !== "cancelled" && p.status !== "completed")) return cur;
            const running = c.plans.filter((p) => p.status === "accepted" || p.status === "in_progress");
            const proposed = c.plans.filter((p) => p.status === "proposed");
            return (running[running.length - 1] ?? proposed[proposed.length - 1])?.id ?? null;
          });
        })
        .catch((e) => !isSessionError(e) && setError(err(e))),
    [patient.id],
  );
  useEffect(() => {
    load();
  }, [load]);

  const services = useMemo(() => new Map((catalog?.services ?? []).map((s) => [s.id, s])), [catalog]);
  const condMap = useMemo(() => new Map((conditions ?? []).map((c) => [c.id, c])), [conditions]);
  const marks = useMemo(() => (chart ? marksOf(chart, services, condMap, workingPlan) : []), [chart, services, condMap, workingPlan]);
  const layers = useMemo(() => layersOf(marks, view), [marks, view]);
  const spans = useMemo(() => spansOf(marks, view), [marks, view]);
  const order = useMemo(() => (chart ? chartTeeth(chart.dentition, chart.primary_slots) : { upper: [], lower: [] }), [chart]);
  const shownTeeth = useMemo(() => new Set([...order.upper, ...order.lower]), [order]);

  const callouts: Callout[] = useMemo(() => {
    const out: Callout[] = [];
    for (const m of marks) {
      if (!m.teeth.length || !m.teeth.some((x) => shownTeeth.has(x))) continue;
      if (view !== "general" && view !== "perio" && !layersOf([m], view).size) continue;
      const head = teethLabel(m.teeth, numbering, lang);
      if (m.item) {
        const what = [itemService(catalog, m.item, lang), m.item.surfaces ? `(${surfacesText(m.item.surfaces, m.item.teeth[0])})` : ""].filter(Boolean).join(" ");
        const parts = [what, m.item.details ?? "", sessionText(t, lang, m.item), stateText(t, itemState(m.item.status))].filter(Boolean);
        out.push({ key: m.key, teeth: m.teeth, state: m.state, head, text: parts.join(" — ") });
      } else {
        const c = m.conditionId ? condMap.get(m.conditionId) : undefined;
        const what = [tr(c?.name, lang), m.surfaces ? `(${surfacesText(m.surfaces, m.teeth[0])})` : ""].filter(Boolean).join(" ");
        out.push({ key: m.key, teeth: m.teeth, state: "existing", head, text: `${what} — ${stateText(t, "existing")}`, finding: true });
      }
    }
    return out;
  }, [marks, view, numbering, lang, catalog, services, condMap, shownTeeth]);

  // Work for the whole mouth (examination, OPG, scaling) has no tooth to point at: listed under the chart.
  const wholeMouth = marks.filter((m) => m.item && !m.item.teeth.length);

  const onTooth = (fdi: number, mode: "single" | "toggle" | "range") => {
    setSurfaces([]);
    if (mode === "toggle") {
      setSelected((s) => (s.includes(fdi) ? s.filter((x) => x !== fdi) : [...s, fdi]));
      setAnchor(fdi);
      return;
    }
    if (mode === "range" && anchor != null) {
      const row = order.upper.includes(fdi) ? order.upper : order.lower;
      const a = row.indexOf(anchor);
      const b = row.indexOf(fdi);
      if (a >= 0 && b >= 0) {
        setSelected(row.slice(Math.min(a, b), Math.max(a, b) + 1));
        return;
      }
    }
    setSelected((s) => (s.length === 1 && s[0] === fdi ? [] : [fdi]));
    setAnchor(fdi);
  };
  const onSurface = (fdi: number, srf: Surface) => {
    if (!(selected.length === 1 && selected[0] === fdi)) {
      setSelected([fdi]);
      setSurfaces([srf]);
      setAnchor(fdi);
      return;
    }
    setSurfaces((s) => (s.includes(srf) ? s.filter((x) => x !== srf) : [...s, srf]));
  };

  const quick = (key: string) => {
    setSurfaces([]);
    const all = [...order.upper, ...order.lower];
    const q = (n: number) => all.filter((x) => toothInfo(x).quadrant === n);
    const pick: Record<string, number[]> = { upper: order.upper, lower: order.lower, q1: q(1), q2: q(2), q3: q(3), q4: q(4), all, none: [] };
    setSelected(pick[key] ?? []);
  };

  /** Service first (M5): the chart moves to the service's view and selects the teeth of a jaw or whole-mouth service. */
  const onServicePicked = (s: ServiceInfo) => {
    setView(viewForSymbol(s.chart_symbol));
    if (s.tooth_scope === "arch") {
      const inUpper = selected.length > 0 && selected.every((x) => toothInfo(x).upper);
      const inLower = selected.length > 0 && selected.every((x) => !toothInfo(x).upper);
      const hint = `${s.code} ${s.name.en}`.toLowerCase();
      const lower = inLower || (!inUpper && (/lower|\bl$|cl$|pl$/.test(hint) || s.code.endsWith("L")));
      setSelected(lower ? order.lower : order.upper);
      setSurfaces([]);
    } else if (s.tooth_scope === "mouth") {
      setSelected(s.chart_symbol === "bracket" || s.chart_symbol === "denture" ? [...order.upper, ...order.lower] : []);
      setSurfaces([]);
    } else if (s.tooth_scope === "none") {
      setSelected([]);
      setSurfaces([]);
    }
  };

  const setDentition = async (d: ChartInfo["dentition"], slots = chart?.primary_slots ?? []) => {
    if (!chart) return;
    try {
      const c = await rpc("chart.set_dentition", { patient_id: patient.id, dentition: d, primary_slots: slots, version: chart.version });
      setChart(c);
      setSelected([]);
    } catch (x) {
      if (!isSessionError(x)) toast.error(err(x));
    }
  };

  if (error && !chart) return <ErrorState message={error} onRetry={load} />;
  if (!chart || !catalog || !conditions) return <Card><Loading /></Card>;

  const refresh = () => {
    load();
    onChanged?.();
  };
  const workingPlans = chart.plans.filter((p) => p.status !== "cancelled" && p.status !== "completed");

  return (
    <div className="stack-lg" data-testid="chart-tab">
      <Card flush className="chart-card">
        <div className="chart-toolbar">
          <Segmented<ChartView>
            value={view}
            onChange={setView}
            label={t("chart.views")}
            options={VIEWS.map((v) => ({ value: v.value, label: t(`chart.view.${v.value}`), icon: v.icon, testId: `chart-view-${v.value}` }))}
          />
          <span className="grow" />
          {view !== "perio" && (
            <>
              <Segmented<ChartInfo["dentition"]>
                value={chart.dentition}
                onChange={(d) => canEdit && setDentition(d)}
                label={t("chart.dentition")}
                options={[
                  { value: "permanent", label: t("chart.dentition.permanent"), icon: Smile, testId: "chart-dentition-permanent" },
                  { value: "mixed", label: t("chart.dentition.mixed"), icon: CircleDot, testId: "chart-dentition-mixed" },
                  { value: "primary", label: t("chart.dentition.primary"), icon: Baby, testId: "chart-dentition-primary" },
                ]}
              />
              <Segmented<"all" | "selected">
                value={labels}
                onChange={setLabels}
                label={t("chart.labels")}
                options={[
                  { value: "all", label: t("chart.labels.all"), icon: Tags, testId: "chart-labels-all" },
                  { value: "selected", label: t("chart.labels.selected"), icon: Tag, testId: "chart-labels-selected" },
                ]}
              />
            </>
          )}
        </div>
        {view === "perio" ? (
          <PerioChart patient={patient} chart={chart} canEdit={canEdit} numbering={numbering} />
        ) : (
          <>
            <div className="chart-quick" role="group" aria-label={t("chart.quick")}>
              {(["upper", "lower", "q1", "q2", "q3", "q4", "all"] as const).map((k) => (
                <Button key={k} size="sm" variant="subtle" onClick={() => quick(k)} data-testid={`chart-quick-${k}`}>{t(`chart.quick.${k}`)}</Button>
              ))}
              {selected.length > 0 && (
                <span className="chart-selection" data-testid="chart-selection">
                  <Badge tone="accent">{t("chart.selectedN").replace("{n}", teethLabel(selected, numbering, lang))}</Badge>
                  {surfaces.length > 0 && <Badge>{surfacesText(surfaces.join(""), selected[0])}</Badge>}
                  <IconButton icon={X} size="sm" label={t("chart.clearSelection")} onClick={() => quick("none")} data-testid="chart-clear" />
                </span>
              )}
              {workingPlans.length > 1 && (
                <label className="chart-plan-pick">
                  <span className="subtle t-caption">{t("chart.showingPlan")}</span>
                  <Select value={workingPlan ?? ""} onChange={(e) => setWorkingPlan(e.target.value || null)} data-testid="chart-working-plan">
                    {workingPlans.map((p) => <option key={p.id} value={p.id}>{p.title} · {t(`plan.status.${p.status}`)}</option>)}
                  </Select>
                </label>
              )}
            </div>
            <div className="chart-canvas">
              <Odontogram
                dentition={chart.dentition}
                primarySlots={chart.primary_slots}
                layers={layers}
                spans={spans}
                callouts={callouts}
                labels={labels}
                view={view}
                selected={selected}
                selectedSurfaces={surfaces}
                numberOf={(fdi) => toothLabel(fdi, numbering, lang)}
                titleOf={(fdi) => `${t("chart.tooth")} ${toothLabel(fdi, numbering, lang)} — ${t(`chart.type.${toothInfo(fdi).type}`)}`}
                ariaLabel={t("patients.tab.dentalChart")}
                sideLabels={{ right: t("chart.patientRight"), left: t("chart.patientLeft") }}
                onTooth={onTooth}
                onSurface={onSurface}
              />
            </div>
            <Legend />
            {wholeMouth.length > 0 && (
              <div className="chart-mouth" data-testid="chart-whole-mouth">
                <span className="subtle t-caption">{t("chart.wholeMouth")}</span>
                {wholeMouth.map((m) => (
                  <span key={m.key} className={`chart-mouth-chip st-${m.state}`}>
                    <span className="odo-callout-dot" aria-hidden />
                    {itemService(catalog, m.item!, lang)} — {stateText(t, m.state)}
                  </span>
                ))}
              </div>
            )}
          </>
        )}
      </Card>

      {view !== "perio" && (
        <ChartPanel
          patient={patient}
          chart={chart}
          clinic={clinic}
          canEdit={canEdit}
          selected={selected}
          surfaces={surfaces}
          numbering={numbering}
          workingPlanId={workingPlan}
          onWorkingPlan={setWorkingPlan}
          onServicePicked={onServicePicked}
          onChanged={refresh}
          onHistory={(fdi) => setHistory(fdi)}
          onToggleSlot={(fdi) => {
            const place = fdi > 50 ? (Math.floor(fdi / 10) - 4) * 10 + (fdi % 10) : fdi;
            const slots = chart.primary_slots.includes(place) ? chart.primary_slots.filter((x) => x !== place) : [...chart.primary_slots, place];
            setDentition("mixed", slots);
          }}
        />
      )}
      {history != null && <ToothHistory patient={patient} tooth={history} numbering={numbering} onClose={() => setHistory(null)} />}
    </div>
  );
}

/** Colour and pattern of each status (M5) — never colour alone. */
function Legend() {
  const { t } = useI18n();
  const items: (ChartState | "missing")[] = [...STATES, "missing"];
  return (
    <div className="chart-legend" data-testid="chart-legend">
      {items.map((s) => (
        <span key={s} className={`chart-legend-item st-${s}`}>
          <span className="chart-swatch" aria-hidden />
          {t(`chart.state.${s}`)}
        </span>
      ))}
      <span className="chart-legend-hint subtle t-caption"><History size={13} aria-hidden /> {t("chart.legendHint")}</span>
    </div>
  );
}
