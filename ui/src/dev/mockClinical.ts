// The clinical part of the in-browser mock Core (see mockCore.ts): the catalog and tooth conditions from
// the Core's own seed files, and a dental chart per patient — findings, plans and their items — enough
// to work on and take screenshots of the chart tab without the Rust Core. Plans, sessions, perio and
// the timeline answer with empty lists.
import servicesSeed from "../../../core/seeds/services.json";
import conditionsSeed from "../../../core/seeds/tooth_conditions.json";
import type {
  CatalogInfo, ChartInfo, ChartSymbol, ServiceCategoryInfo, ServiceInfo, ToothConditionInfo, ToothFinding, ToothScope, TreatmentItemInfo, TreatmentPlanInfo, Translations,
} from "../../../shared/ts/contract";

type ServiceSeed = { code: string; name: Translations; scope?: ToothScope; surface?: boolean; sessions?: number; interval?: number; lab?: boolean; symbol?: ChartSymbol; variants?: ServiceSeed[] };
type CategorySeed = { code: string; name: Translations; services: ServiceSeed[] };
type ConditionSeed = { code: string; name: Translations; symbol: ChartSymbol; surface?: boolean; abbreviation?: string };

/** Prices the seed-5b script gives the services (afghani); the rest stay unpriced, as in a new clinic. */
const PRICES: Record<string, number> = { "END-01M": 4500, "END-01D": 3500, "PRS-01Z": 9000, "PRS-01P": 6000, "RST-01C": 1200, "RST-02C": 1800, "SRG-01": 800, "IMP-01O": 28000, "IMP-02": 6000, "IMP-03": 9000, "PER-01": 1000, "DX-01": 200, "RAD-02": 700 };

function buildCatalog(): CatalogInfo {
  const categories: ServiceCategoryInfo[] = [];
  const services: ServiceInfo[] = [];
  (servicesSeed as CategorySeed[]).forEach((c, ci) => {
    const catId = `cat/${c.code}`;
    categories.push({ id: catId, code: c.code, name: c.name, specialty_id: null, sort_order: (ci + 1) * 10, is_system: true, is_active: true, version: 1 });
    const add = (s: ServiceSeed, parent: ServiceSeed | null, sort: number) => {
      services.push({
        id: `svc/${s.code}`, code: s.code, category_id: catId, parent_id: parent ? `svc/${parent.code}` : null, name: s.name,
        price: (PRICES[s.code] ?? 0) * 100, specialty_id: null, tooth_scope: s.scope ?? parent?.scope ?? "none",
        needs_surface: s.surface ?? parent?.surface ?? false, sessions: s.sessions ?? parent?.sessions ?? 1,
        session_interval_days: s.interval ?? parent?.interval ?? 7, lab_required: s.lab ?? parent?.lab ?? false,
        chart_symbol: s.symbol ?? parent?.symbol ?? "none", consent_template_id: null, post_op_template_id: null,
        sort_order: sort, is_system: true, is_active: true, version: 1,
      });
    };
    c.services.forEach((s, si) => {
      add(s, null, (si + 1) * 10);
      s.variants?.forEach((v, vi) => add(v, s, (vi + 1) * 10));
    });
  });
  return { categories, services };
}

const catalog = buildCatalog();
const conditions: ToothConditionInfo[] = (conditionsSeed as ConditionSeed[]).map((c, i) => ({
  id: `cond/${c.code}`, code: c.code, name: c.name, symbol: c.symbol, needs_surface: !!c.surface, abbreviation: c.abbreviation ?? null,
  is_system: true, is_active: true, sort_order: (i + 1) * 10, version: 1,
}));

type Chart = { dentition: ChartInfo["dentition"]; primary_slots: number[]; findings: ToothFinding[]; plans: TreatmentPlanInfo[]; items: TreatmentItemInfo[]; version: number };
const charts = new Map<string, Chart>();
const today = () => new Intl.DateTimeFormat("en-CA", { timeZone: "Asia/Kabul" }).format(new Date());
const now = () => new Date().toISOString();
const uuid = () => crypto.randomUUID();

function chartOf(patientId: string): Chart {
  let c = charts.get(patientId);
  if (!c) charts.set(patientId, (c = { dentition: "permanent", primary_slots: [], findings: [], plans: [], items: [], version: 1 }));
  return c;
}

function addFinding(c: Chart, patientId: string, code: string, teeth: number[], surfaces = "", note: string | null = null) {
  for (const tooth of teeth) {
    c.findings.push({ id: uuid(), patient_id: patientId, tooth, surfaces, condition_id: `cond/${code}`, note, recorded_on: today(), recorded_by_name: "داکتر احمد رحیمی", version: 1 });
  }
}

function addItem(c: Chart, p: { patient_id: string; plan_id: string | null; service_id: string; teeth: number[]; surfaces: string; doctor_id: string | null; unit_price: number | null; phase: number; sessions: number; details: string | null; notes: string | null; perform_now: boolean }, done = 0) {
  const svc = catalog.services.find((s) => s.id === p.service_id);
  if (!svc) throw new Error("unknown service");
  const perTooth = svc.tooth_scope === "tooth" && p.teeth.length > 1 ? p.teeth.map((t) => [t]) : [p.teeth];
  for (const teeth of perTooth) {
    const sessionsDone = p.perform_now ? p.sessions : done;
    c.items.push({
      id: uuid(), plan_id: p.plan_id, patient_id: p.patient_id, service_id: p.service_id, teeth, surfaces: p.surfaces, doctor_id: p.doctor_id,
      unit_price: p.unit_price ?? svc.price, units: 1, phase: p.phase, status: p.perform_now ? "completed" : sessionsDone ? "in_progress" : "planned",
      sessions_planned: p.sessions, sessions_done: sessionsDone, sessions: [], upcoming: [], details: p.details, notes: p.notes, est_start: null, est_end: null,
      completed_at: p.perform_now ? now() : null, cancel_reason: null, billed: false, sort_order: c.items.length + 1, created_at: now(), version: 1,
    });
  }
}

function newPlan(c: Chart, patientId: string, title: string): TreatmentPlanInfo {
  const plan: TreatmentPlanInfo = {
    id: uuid(), patient_id: patientId, title, status: "proposed", notes: null, created_at: now(), created_by_name: "داکتر احمد رحیمی",
    accepted_at: null, completed_at: null, cancelled_at: null, cancel_reason: null, superseded_by: null, items: [], total: 0, version: 1,
  };
  c.plans.push(plan);
  return plan;
}

/** The first seeded patient gets the chart of ui/scripts/seed-5b.mjs: findings, a running plan, a root canal under way. */
export function seedClinical(patientId: string) {
  const c = chartOf(patientId);
  addFinding(c, patientId, "filling", [16], "MO");
  addFinding(c, patientId, "filling", [26], "O");
  addFinding(c, patientId, "caries", [36], "OD");
  addFinding(c, patientId, "caries", [47], "B");
  addFinding(c, patientId, "crown", [11]);
  addFinding(c, patientId, "root_canal", [11]);
  addFinding(c, patientId, "missing", [18, 28, 38]);
  const plan = newPlan(c, patientId, "پلان درمان ۱");
  plan.status = "accepted";
  const base = { patient_id: patientId, plan_id: plan.id, surfaces: "", doctor_id: "d1", unit_price: null, details: null, notes: null, perform_now: false };
  addItem(c, { ...base, service_id: "svc/END-01D", teeth: [36], phase: 1, sessions: 2 }, 1);
  addItem(c, { ...base, service_id: "svc/RST-01C", teeth: [47], surfaces: "B", phase: 1, sessions: 1 });
  addItem(c, { ...base, service_id: "svc/PRS-01Z", teeth: [36], phase: 2, sessions: 2 });
  addItem(c, { ...base, service_id: "svc/IMP-01O", teeth: [46], phase: 2, sessions: 1 });
  addItem(c, { ...base, service_id: "svc/PER-01", teeth: [], phase: 1, sessions: 1 });
}

const brief = (c: Chart, patientId: string): ChartInfo => ({
  patient_id: patientId, dentition: c.dentition, primary_slots: c.primary_slots, dentition_chosen: true, findings: c.findings,
  items: c.items, plans: c.plans.map((x) => ({ id: x.id, title: x.title, status: x.status })), version: c.version,
});

/** Answers a clinical method, or `undefined` when the method is not one of these. */
export function clinicalCall(method: string, p: any): unknown {
  switch (method) {
    case "catalog.get":
      return catalog;
    case "tooth_conditions.list":
      return conditions;
    case "specialties.list":
    case "plan_templates.list":
    case "note_templates.list":
    case "perio.list":
    case "chart.tooth_history":
    case "treatment_items.open":
      return [];
    case "plans.list":
      return chartOf(p.patient_id).plans.map((x) => ({ ...x, items: chartOf(p.patient_id).items.filter((i) => i.plan_id === x.id) }));
    case "chart.get":
      return brief(chartOf(p.patient_id), p.patient_id);
    case "chart.set_dentition": {
      const c = chartOf(p.patient_id);
      c.dentition = p.dentition;
      c.primary_slots = p.primary_slots;
      c.version++;
      return brief(c, p.patient_id);
    }
    case "chart.add_findings": {
      const c = chartOf(p.patient_id);
      addFinding(c, p.patient_id, String(p.condition_id).replace("cond/", ""), p.teeth, p.surfaces, p.note);
      return null;
    }
    case "chart.remove_finding": {
      for (const c of charts.values()) c.findings = c.findings.filter((f) => f.id !== p.id);
      return null;
    }
    case "plans.create": {
      const c = chartOf(p.patient_id);
      return newPlan(c, p.patient_id, p.title);
    }
    case "treatment_items.add": {
      addItem(chartOf(p.patient_id), p);
      return null;
    }
  }
  return undefined;
}
