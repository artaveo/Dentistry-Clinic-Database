// What the chart draws on each tooth (M5): the existing findings (blue) and the treatment items —
// planned (red), in progress (orange), completed (green) — turned into per-tooth layers. When two things
// land on the same surface or part, the more advanced one wins: completed › in progress › planned › existing.
import type { ChartInfo, ChartSymbol, ItemStatus, PlanStatus, ServiceInfo, ToothConditionInfo, TreatmentItemInfo } from "../../../shared/ts/contract";
import type { Surface } from "./geometry";

export type ChartState = "existing" | "planned" | "progress" | "done";
export type ChartView = "general" | "ortho" | "implant" | "prosth" | "perio";

const RANK: Record<ChartState, number> = { existing: 1, planned: 2, progress: 3, done: 4 };
export const STATES: ChartState[] = ["existing", "planned", "progress", "done"];

export function itemState(s: ItemStatus): ChartState {
  return s === "completed" ? "done" : s === "in_progress" ? "progress" : "planned";
}

/** The symbols each specialty view shows (M5); "general" shows everything. */
const VIEW_SYMBOLS: Record<Exclude<ChartView, "general" | "perio">, ChartSymbol[]> = {
  ortho: ["bracket"],
  implant: ["implant", "abutment", "implant_crown", "missing", "extraction"],
  prosth: ["crown", "bridge", "denture", "veneer", "implant_crown", "missing", "extraction"],
};

export function viewShows(view: ChartView, symbol: ChartSymbol): boolean {
  return view === "general" || view === "perio" || VIEW_SYMBOLS[view].includes(symbol);
}

/** The specialty view a service belongs to, for the "service first" path. */
export function viewForSymbol(symbol: ChartSymbol): ChartView {
  if (symbol === "bracket") return "ortho";
  if (symbol === "implant" || symbol === "abutment" || symbol === "implant_crown") return "implant";
  if (symbol === "crown" || symbol === "bridge" || symbol === "denture" || symbol === "veneer") return "prosth";
  return "general";
}

export type Mark = {
  key: string;
  kind: "finding" | "item";
  symbol: ChartSymbol;
  state: ChartState;
  teeth: number[];
  surfaces: string;
  findingId?: string;
  item?: TreatmentItemInfo;
  conditionId?: string;
  serviceId?: string;
  abbr?: string | null;
};

export type ToothLayers = {
  /** The tooth is not there (missing, extracted): drawn as a faint outline. */
  ghost: ChartState | null;
  extraction: ChartState | null;
  surfaces: Partial<Record<Surface, ChartState>>;
  crown: ChartState | null;
  veneer: ChartState | null;
  rootCanal: ChartState | null;
  pulpotomy: ChartState | null;
  implant: { fixture: ChartState | null; abutment: ChartState | null; crown: ChartState | null } | null;
  pontic: ChartState | null;
  bracket: ChartState | null;
  sealant: ChartState | null;
  fracture: ChartState | null;
  impacted: ChartState | null;
  lesion: ChartState | null;
  denture: ChartState | null;
  scaling: ChartState | null;
  markers: { abbr: string; state: ChartState }[];
  /** Has something the current view shows. */
  relevant: boolean;
};

function emptyLayers(): ToothLayers {
  return {
    ghost: null, extraction: null, surfaces: {}, crown: null, veneer: null, rootCanal: null, pulpotomy: null, implant: null,
    pontic: null, bracket: null, sealant: null, fracture: null, impacted: null, lesion: null, denture: null, scaling: null,
    markers: [], relevant: false,
  };
}

const best = (a: ChartState | null, b: ChartState): ChartState => (a && RANK[a] >= RANK[b] ? a : b);

/** Which plans' planned items the chart shows: running plans always, a proposed one only while it is the working plan. */
export function planVisible(status: PlanStatus | undefined, planId: string | null, workingPlanId: string | null): boolean {
  if (!planId) return true;
  if (status === "accepted" || status === "in_progress" || status === "completed") return true;
  if (status === "proposed") return planId === workingPlanId;
  return false;
}

/** Every mark of the chart, filtered by the plans shown. */
export function marksOf(chart: ChartInfo, services: Map<string, ServiceInfo>, conditions: Map<string, ToothConditionInfo>, workingPlanId: string | null): Mark[] {
  const planStatus = new Map(chart.plans.map((p) => [p.id, p.status]));
  const marks: Mark[] = [];
  for (const f of chart.findings) {
    const c = conditions.get(f.condition_id);
    marks.push({ key: `f-${f.id}`, kind: "finding", symbol: c?.symbol ?? "marker", state: "existing", teeth: [f.tooth], surfaces: f.surfaces, findingId: f.id, conditionId: f.condition_id, abbr: c?.abbreviation });
  }
  for (const i of chart.items) {
    if (i.status === "cancelled") continue;
    // Work started or done always shows; a planned item only when its plan is on the table.
    if (i.status === "planned" && !planVisible(planStatus.get(i.plan_id ?? ""), i.plan_id, workingPlanId)) continue;
    const s = services.get(i.service_id);
    marks.push({ key: `i-${i.id}`, kind: "item", symbol: s?.chart_symbol ?? "none", state: itemState(i.status), teeth: i.teeth, surfaces: i.surfaces, item: i, serviceId: i.service_id, abbr: s ? s.code.split("-")[0].slice(0, 3) : null });
  }
  return marks;
}

/** Per-tooth layers for the current view. */
export function layersOf(marks: Mark[], view: ChartView): Map<number, ToothLayers> {
  const out = new Map<number, ToothLayers>();
  const get = (t: number) => {
    let l = out.get(t);
    if (!l) out.set(t, (l = emptyLayers()));
    return l;
  };
  for (const m of marks) {
    if (!viewShows(view, m.symbol)) continue;
    for (const t of m.teeth) {
      const l = get(t);
      if (m.symbol !== "none" && m.symbol !== "scaling") l.relevant = true;
      const st = m.state;
      switch (m.symbol) {
        case "surface":
          for (const ch of m.surfaces || "O") l.surfaces[ch as Surface] = best(l.surfaces[ch as Surface] ?? null, st);
          break;
        case "sealant":
          l.sealant = best(l.sealant, st);
          break;
        case "veneer":
          l.veneer = best(l.veneer, st);
          break;
        case "crown":
          l.crown = best(l.crown, st);
          break;
        case "root_canal":
          l.rootCanal = best(l.rootCanal, st);
          break;
        case "pulpotomy":
          l.pulpotomy = best(l.pulpotomy, st);
          break;
        case "extraction":
          if (st === "done") l.ghost = best(l.ghost, "done");
          else l.extraction = best(l.extraction, st);
          break;
        case "missing":
          l.ghost = best(l.ghost, st);
          break;
        case "implant":
        case "abutment":
        case "implant_crown": {
          l.implant ??= { fixture: null, abutment: null, crown: null };
          if (m.symbol === "implant") {
            l.implant.fixture = best(l.implant.fixture, st);
            // An implant found at the exam already carries its crown.
            if (m.kind === "finding") l.implant.crown = best(l.implant.crown, st);
          } else if (m.symbol === "abutment") l.implant.abutment = best(l.implant.abutment, st);
          else l.implant.crown = best(l.implant.crown, st);
          break;
        }
        case "bridge":
          l.pontic = best(l.pontic, st);
          break;
        case "denture":
          l.denture = best(l.denture, st);
          break;
        case "bracket":
          l.bracket = best(l.bracket, st);
          break;
        case "fracture":
          l.fracture = best(l.fracture, st);
          break;
        case "impacted":
          l.impacted = best(l.impacted, st);
          break;
        case "lesion":
          l.lesion = best(l.lesion, st);
          break;
        case "scaling":
          l.scaling = best(l.scaling, st);
          break;
        case "marker":
          if (m.abbr) l.markers.push({ abbr: m.abbr, state: st });
          break;
        default:
          break;
      }
    }
  }
  // A bridge item spans abutments and pontics: its end teeth carry crowns, the ones between are pontics.
  for (const m of marks) {
    if (m.symbol !== "bridge" || m.teeth.length < 2 || !viewShows(view, m.symbol)) continue;
    const sorted = [...m.teeth].sort((a, b) => a - b);
    for (const t of [sorted[0], sorted[sorted.length - 1]]) {
      const l = get(t);
      if (l.pontic === m.state) l.pontic = null;
      l.crown = best(l.crown, m.state);
    }
  }
  return out;
}

/** Bridges and dentures span teeth: their spans, for the connectors drawn across neighbours. */
export function spansOf(marks: Mark[], view: ChartView): { key: string; symbol: "bridge" | "denture" | "bracket"; state: ChartState; teeth: number[] }[] {
  return marks
    .filter((m) => (m.symbol === "bridge" || m.symbol === "denture" || m.symbol === "bracket") && m.teeth.length > 1 && viewShows(view, m.symbol))
    .map((m) => ({ key: m.key, symbol: m.symbol as "bridge" | "denture" | "bracket", state: m.state, teeth: m.teeth }));
}

/** Which tooth places of a service's scope are compatible with the current selection ("tooth first"). */
export function scopeFits(scope: ServiceInfo["tooth_scope"], selected: number[]): boolean {
  switch (scope) {
    case "tooth":
      return selected.length >= 1;
    case "teeth":
      return selected.length >= 2;
    case "quadrant":
    case "arch":
      return selected.length >= 1;
    case "mouth":
    case "none":
      return true;
  }
}

/** Every chart symbol, in the order the catalog and the conditions list offer them. */
export const CHART_SYMBOLS: ChartSymbol[] = [
  "none", "surface", "sealant", "veneer", "crown", "root_canal", "pulpotomy", "extraction", "missing", "implant", "abutment",
  "implant_crown", "bridge", "denture", "bracket", "fracture", "impacted", "lesion", "scaling", "marker",
];
