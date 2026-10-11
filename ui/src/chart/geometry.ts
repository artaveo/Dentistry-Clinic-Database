// Tooth geometry of the dental chart (M5, docs/design/dental-chart.md): anatomical 2.5D teeth drawn from
// normalized outlines. Every shape is written once with u = 0 at the distal side and u = 1 at the mesial
// side, v = 0 at the cemento-enamel junction (CEJ), v = 1 at the incisal/occlusal edge and v = -1 at the
// root apex; `place()` maps it onto a tooth's slot, mirrored so mesial always faces the midline, and
// flipped for the lower jaw. Sizes follow real tooth dimensions (mm), with a fixed offset so even a
// lower incisor stays a comfortable click target.
import type { Dentition } from "../../../shared/ts/contract";

export type ToothType = "central" | "lateral" | "canine" | "premolar1" | "premolar2" | "molar1" | "molar2" | "molar3" | "pA" | "pB" | "pC" | "pD" | "pE";

/** [mesiodistal width, crown height, root length, buccolingual depth] in mm. */
type Dims = [number, number, number, number];

const UPPER: Dims[] = [[8.5, 10.5, 13, 7], [6.5, 9, 13, 6], [7.5, 10, 17, 8], [7, 8.5, 14, 9], [6.5, 8.5, 14, 9], [10, 7.5, 12.5, 11], [9, 7, 11.5, 11], [8.5, 6.5, 11, 10]];
const LOWER: Dims[] = [[5, 9, 12.5, 6], [5.5, 9.5, 14, 6.5], [7, 11, 16, 7.5], [7, 8.5, 14, 7.5], [7, 8, 14.5, 8], [11, 7.5, 14, 10.5], [10.5, 7, 13, 10], [10, 7, 11, 9.5]];
const UPPER_PRIMARY: Dims[] = [[6.5, 6, 10, 5], [5.1, 5.6, 11, 4], [7, 6.5, 13, 7], [7.3, 5.1, 7.5, 9], [8.2, 5.7, 9, 10]];
const LOWER_PRIMARY: Dims[] = [[4, 5, 9, 4], [4.7, 5.2, 10, 4], [5, 6, 11.5, 5], [7.7, 6, 8, 7], [9.9, 5.5, 9.5, 8.7]];
const PERMANENT_TYPES: ToothType[] = ["central", "lateral", "canine", "premolar1", "premolar2", "molar1", "molar2", "molar3"];
const PRIMARY_TYPES: ToothType[] = ["pA", "pB", "pC", "pD", "pE"];

export type ToothInfo = {
  fdi: number;
  /** 1 upper right … 4 lower right (a baby tooth takes its place's quadrant). */
  quadrant: 1 | 2 | 3 | 4;
  /** 1 central incisor … 8 third molar (A … E = 1 … 5 for baby teeth). */
  n: number;
  primary: boolean;
  upper: boolean;
  type: ToothType;
  dims: Dims;
};

export function isTooth(fdi: number): boolean {
  const q = Math.floor(fdi / 10);
  const n = fdi % 10;
  return (q >= 1 && q <= 4 && n >= 1 && n <= 8) || (q >= 5 && q <= 8 && n >= 1 && n <= 5);
}

export function toothInfo(fdi: number): ToothInfo {
  const q = Math.floor(fdi / 10);
  const n = fdi % 10;
  const primary = q >= 5;
  const quadrant = (primary ? q - 4 : q) as 1 | 2 | 3 | 4;
  const upper = quadrant <= 2;
  const dims = primary ? (upper ? UPPER_PRIMARY : LOWER_PRIMARY)[n - 1] : (upper ? UPPER : LOWER)[n - 1];
  return { fdi, quadrant, n, primary, upper, type: primary ? PRIMARY_TYPES[n - 1] : PERMANENT_TYPES[n - 1], dims };
}

export const isMolar = (t: ToothType) => t === "molar1" || t === "molar2" || t === "molar3" || t === "pD" || t === "pE";
export const isAnterior = (t: ToothType) => t === "central" || t === "lateral" || t === "canine" || t === "pA" || t === "pB" || t === "pC";

// ───────────────────────────── layout ─────────────────────────────

/** The chart's own coordinate space; the component scales it to the screen. */
export const CHART_W = 1080;
export const NUM_H = 26;
export const SIDE_H = 150;
export const OCC_H = 66;
const OCC_GAP = 8;
const MID_GAP = 16;
/** Vertical bands, top to bottom. */
export const BANDS = (() => {
  const upperNum = 0;
  const upperSideTop = NUM_H;
  const upperEdge = upperSideTop + SIDE_H;
  const upperOccTop = upperEdge + OCC_GAP;
  const midY = upperOccTop + OCC_H + MID_GAP / 2;
  const lowerOccTop = upperOccTop + OCC_H + MID_GAP;
  const lowerEdge = lowerOccTop + OCC_H + OCC_GAP;
  const lowerNum = lowerEdge + SIDE_H;
  return { upperNum, upperSideTop, upperEdge, upperOccTop, midY, lowerOccTop, lowerEdge, lowerNum, height: lowerNum + NUM_H };
})();

const W_BASE = 18;
const W_PER_MM = 5.6;
const H_PER_MM = 4.9;
const GAP = 3;

export type Slot = {
  fdi: number;
  info: ToothInfo;
  /** Left edge and width in chart units. */
  x: number;
  w: number;
  crownH: number;
  rootH: number;
  depth: number;
  /** Mesial side is on the right (the patient's right half, shown on the left). */
  mesialRight: boolean;
};

/** Every tooth shown for a dentition, left to right per jaw (patient's right on the viewer's left). */
export function chartTeeth(dentition: Dentition, primarySlots: number[]): { upper: number[]; lower: number[] } {
  const slot = (q: number, n: number): number | null => {
    if (dentition === "primary") return n <= 5 ? (q + 4) * 10 + n : null;
    if (dentition === "mixed" && n <= 5 && primarySlots.includes(q * 10 + n)) return (q + 4) * 10 + n;
    return q * 10 + n;
  };
  const side = (q: number, reverse: boolean) => {
    const v: number[] = [];
    for (let n = 1; n <= 8; n++) {
      const t = slot(q, n);
      if (t) v.push(t);
    }
    return reverse ? v.reverse() : v;
  };
  return { upper: [...side(1, true), ...side(2, false)], lower: [...side(4, true), ...side(3, false)] };
}

/** Positions of every tooth: both jaws centred, baby teeth scaled up when they are the whole chart. */
export function layout(dentition: Dentition, primarySlots: number[]): { upper: Slot[]; lower: Slot[]; scale: number } {
  const { upper, lower } = chartTeeth(dentition, primarySlots);
  const width = (list: number[]) => list.reduce((s, t) => s + W_BASE + W_PER_MM * toothInfo(t).dims[0] + GAP, MID_GAP - GAP);
  const widest = Math.max(width(upper), width(lower));
  const scale = dentition === "primary" ? Math.min(1.5, (CHART_W * 0.86) / widest) : Math.min(1, (CHART_W - 24) / widest);
  const row = (list: number[]): Slot[] => {
    let x = (CHART_W - width(list) * scale) / 2;
    const half = list.length / 2;
    return list.map((fdi, i) => {
      const info = toothInfo(fdi);
      const [mm, crown, root, depth] = info.dims;
      const w = (W_BASE + W_PER_MM * mm) * scale;
      const slot: Slot = {
        fdi,
        info,
        x,
        w,
        crownH: crown * H_PER_MM * scale,
        rootH: root * H_PER_MM * scale,
        depth: Math.min(OCC_H - 6, depth * H_PER_MM * scale * 1.05),
        mesialRight: info.quadrant === 1 || info.quadrant === 4,
      };
      x += w + GAP * scale + (i === half - 1 ? MID_GAP * scale : 0);
      return slot;
    });
  };
  return { upper: row(upper), lower: row(lower), scale };
}

// ───────────────────────────── normalized shapes ─────────────────────────────

type Seg = ["M" | "L", number, number] | ["C", number, number, number, number, number, number] | ["Z"];
const M = (u: number, v: number): Seg => ["M", u, v];
const C = (a: number, b: number, c: number, d: number, u: number, v: number): Seg => ["C", a, b, c, d, u, v];
const L = (u: number, v: number): Seg => ["L", u, v];
const Z: Seg = ["Z"];

const CROWN: Record<"incisor" | "canine" | "premolar" | "molarU" | "molarL" | "molarL1" | "primaryMolar", Seg[]> = {
  incisor: [M(0.17, 0), C(0.08, 0.14, 0.02, 0.4, 0.04, 0.62), C(0.05, 0.84, 0.11, 0.98, 0.27, 1), C(0.42, 1.015, 0.6, 1.015, 0.75, 1), C(0.9, 0.98, 0.96, 0.84, 0.96, 0.62), C(0.98, 0.4, 0.92, 0.14, 0.83, 0), C(0.62, -0.08, 0.38, -0.08, 0.17, 0), Z],
  canine: [M(0.2, 0), C(0.08, 0.14, 0.01, 0.36, 0.03, 0.54), C(0.05, 0.68, 0.16, 0.79, 0.29, 0.87), C(0.38, 0.93, 0.45, 1, 0.5, 1.02), C(0.55, 1, 0.63, 0.92, 0.72, 0.86), C(0.85, 0.78, 0.96, 0.67, 0.97, 0.52), C(0.99, 0.34, 0.92, 0.14, 0.8, 0), C(0.6, -0.08, 0.4, -0.08, 0.2, 0), Z],
  premolar: [M(0.18, 0), C(0.06, 0.14, 0.01, 0.4, 0.04, 0.6), C(0.07, 0.77, 0.2, 0.87, 0.33, 0.93), C(0.42, 0.98, 0.46, 1, 0.5, 1), C(0.56, 1, 0.63, 0.96, 0.7, 0.92), C(0.83, 0.85, 0.95, 0.75, 0.96, 0.58), C(0.98, 0.38, 0.93, 0.14, 0.82, 0), C(0.62, -0.07, 0.38, -0.07, 0.18, 0), Z],
  molarU: [M(0.11, 0), C(0.03, 0.15, -0.01, 0.42, 0.02, 0.63), C(0.04, 0.82, 0.12, 0.97, 0.25, 0.99), C(0.35, 1.01, 0.43, 0.96, 0.5, 0.9), C(0.57, 0.96, 0.66, 1.01, 0.76, 1), C(0.89, 0.98, 0.97, 0.83, 0.98, 0.63), C(1.01, 0.42, 0.97, 0.15, 0.89, 0), C(0.62, -0.06, 0.38, -0.06, 0.11, 0), Z],
  molarL: [M(0.1, 0), C(0.02, 0.15, -0.01, 0.44, 0.02, 0.64), C(0.04, 0.84, 0.13, 0.98, 0.26, 0.99), C(0.36, 1, 0.44, 0.95, 0.5, 0.9), C(0.56, 0.95, 0.65, 1.01, 0.76, 1), C(0.89, 0.98, 0.97, 0.84, 0.98, 0.64), C(1.01, 0.44, 0.98, 0.15, 0.9, 0), C(0.62, -0.06, 0.38, -0.06, 0.1, 0), Z],
  molarL1: [M(0.1, 0), C(0.02, 0.15, -0.01, 0.44, 0.02, 0.64), C(0.04, 0.84, 0.1, 0.98, 0.19, 0.99), C(0.27, 1, 0.32, 0.96, 0.37, 0.91), C(0.42, 0.97, 0.48, 1.01, 0.55, 1), C(0.61, 1, 0.65, 0.95, 0.69, 0.91), C(0.74, 0.97, 0.8, 1, 0.86, 0.98), C(0.94, 0.95, 0.98, 0.82, 0.98, 0.64), C(1.01, 0.44, 0.98, 0.15, 0.9, 0), C(0.62, -0.06, 0.38, -0.06, 0.1, 0), Z],
  primaryMolar: [M(0.12, 0), C(0.0, 0.08, -0.02, 0.36, 0.03, 0.6), C(0.06, 0.82, 0.14, 0.97, 0.27, 0.99), C(0.37, 1, 0.44, 0.95, 0.5, 0.9), C(0.56, 0.95, 0.64, 1, 0.74, 0.99), C(0.87, 0.97, 0.95, 0.82, 0.97, 0.6), C(1.02, 0.36, 1.0, 0.08, 0.88, 0), C(0.62, -0.05, 0.38, -0.05, 0.12, 0), Z],
};

/** Root shapes: drawn first, the crown covers their base. The first one of a molar is the hidden (palatal) root. */
const ROOTS: Record<string, Seg[][]> = {
  single: [[M(0.21, 0.04), C(0.18, -0.3, 0.3, -0.78, 0.45, -0.98), C(0.49, -1.02, 0.55, -1.01, 0.58, -0.96), C(0.7, -0.74, 0.82, -0.3, 0.79, 0.04), Z]],
  canine: [[M(0.22, 0.04), C(0.18, -0.34, 0.32, -0.82, 0.46, -0.99), C(0.5, -1.02, 0.55, -1, 0.57, -0.96), C(0.7, -0.76, 0.82, -0.34, 0.78, 0.04), Z]],
  forked: [[M(0.2, 0.04), C(0.18, -0.3, 0.24, -0.6, 0.28, -0.8), C(0.3, -0.92, 0.34, -1, 0.38, -0.98), C(0.42, -0.96, 0.44, -0.84, 0.48, -0.7), C(0.52, -0.84, 0.56, -0.96, 0.62, -0.98), C(0.66, -1, 0.7, -0.92, 0.72, -0.8), C(0.76, -0.6, 0.82, -0.3, 0.8, 0.04), Z]],
  upperMolar: [
    [M(0.32, 0.04), C(0.3, -0.3, 0.4, -0.86, 0.5, -1), C(0.6, -0.86, 0.7, -0.3, 0.68, 0.04), Z],
    [M(0.54, 0.04), C(0.58, -0.22, 0.62, -0.3, 0.64, -0.4), C(0.67, -0.62, 0.72, -0.82, 0.78, -0.9), C(0.82, -0.93, 0.86, -0.88, 0.87, -0.8), C(0.9, -0.56, 0.91, -0.24, 0.89, 0.04), Z],
    [M(0.11, 0.04), C(0.1, -0.24, 0.12, -0.5, 0.16, -0.7), C(0.19, -0.8, 0.23, -0.84, 0.27, -0.8), C(0.32, -0.66, 0.4, -0.42, 0.46, 0.04), Z],
  ],
  lowerMolar: [
    [M(0.1, 0.04), L(0.9, 0.04), C(0.9, -0.16, 0.8, -0.3, 0.5, -0.34), C(0.2, -0.3, 0.1, -0.16, 0.1, 0.04), Z],
    [M(0.52, 0.04), C(0.54, -0.3, 0.62, -0.7, 0.7, -0.96), C(0.73, -1.02, 0.8, -1.01, 0.82, -0.94), C(0.88, -0.6, 0.92, -0.3, 0.9, 0.04), Z],
    [M(0.1, 0.04), C(0.08, -0.3, 0.14, -0.66, 0.22, -0.92), C(0.25, -0.99, 0.32, -0.99, 0.35, -0.92), C(0.42, -0.62, 0.5, -0.3, 0.5, 0.04), Z],
  ],
  fused: [[M(0.15, 0.04), C(0.13, -0.35, 0.3, -0.82, 0.5, -0.96), C(0.7, -0.82, 0.87, -0.35, 0.85, 0.04), Z]],
  primaryUpperMolar: [
    [M(0.36, 0.04), C(0.36, -0.4, 0.44, -0.86, 0.5, -0.98), C(0.56, -0.86, 0.64, -0.4, 0.64, 0.04), Z],
    [M(0.62, 0.04), C(0.72, -0.3, 0.9, -0.7, 0.98, -0.94), C(1.0, -1.0, 0.94, -1.0, 0.92, -0.96), C(0.84, -0.7, 0.74, -0.36, 0.86, 0.04), Z],
    [M(0.14, 0.04), C(0.26, -0.36, 0.16, -0.7, 0.08, -0.96), C(0.06, -1.0, 0.0, -1.0, 0.02, -0.94), C(0.1, -0.7, 0.28, -0.3, 0.38, 0.04), Z],
  ],
  primaryLowerMolar: [
    [M(0.12, 0.04), L(0.88, 0.04), C(0.86, -0.1, 0.7, -0.18, 0.5, -0.2), C(0.3, -0.18, 0.14, -0.1, 0.12, 0.04), Z],
    [M(0.6, 0.04), C(0.72, -0.3, 0.9, -0.7, 0.98, -0.96), C(1.0, -1.01, 0.93, -1.01, 0.91, -0.97), C(0.84, -0.72, 0.74, -0.4, 0.86, 0.04), Z],
    [M(0.14, 0.04), C(0.26, -0.4, 0.16, -0.72, 0.09, -0.97), C(0.07, -1.01, 0.0, -1.01, 0.02, -0.96), C(0.1, -0.7, 0.28, -0.3, 0.4, 0.04), Z],
  ],
};

/** Canal lines (for a root canal treatment), one per visible root: [u at CEJ, u at the apex, v of the apex]. */
const CANALS: Record<string, [number, number, number][]> = {
  single: [[0.5, 0.515, -0.9]],
  canine: [[0.5, 0.515, -0.92]],
  forked: [[0.46, 0.36, -0.9], [0.54, 0.64, -0.9]],
  upperMolar: [[0.5, 0.5, -0.9], [0.64, 0.8, -0.82], [0.34, 0.22, -0.74]],
  lowerMolar: [[0.64, 0.76, -0.88], [0.36, 0.28, -0.86]],
  fused: [[0.5, 0.5, -0.86]],
  primaryUpperMolar: [[0.5, 0.5, -0.88], [0.7, 0.93, -0.88], [0.3, 0.07, -0.88]],
  primaryLowerMolar: [[0.68, 0.94, -0.9], [0.32, 0.06, -0.9]],
};

export type ToothShapes = { crown: Seg[]; roots: Seg[][]; canals: [number, number, number][]; grooves: Seg[][] };

export function shapesOf(info: ToothInfo): ToothShapes {
  const t = info.type;
  const crown =
    t === "canine" || t === "pC" ? CROWN.canine
      : t === "premolar1" || t === "premolar2" ? CROWN.premolar
        : t === "pD" || t === "pE" ? CROWN.primaryMolar
          : t === "molar1" ? (info.upper ? CROWN.molarU : CROWN.molarL1)
            : t === "molar2" || t === "molar3" ? (info.upper ? CROWN.molarU : CROWN.molarL)
              : CROWN.incisor;
  const rootKey =
    t === "canine" || t === "pC" ? "canine"
      : t === "premolar1" && info.upper ? "forked"
        : t === "molar3" ? "fused"
          : t === "molar1" || t === "molar2" ? (info.upper ? "upperMolar" : "lowerMolar")
            : t === "pD" || t === "pE" ? (info.upper ? "primaryUpperMolar" : "primaryLowerMolar")
              : "single";
  const grooves: Seg[][] =
    crown === CROWN.molarL1 ? [[M(0.37, 0.91), C(0.37, 0.8, 0.36, 0.7, 0.37, 0.6)], [M(0.69, 0.91), C(0.69, 0.8, 0.7, 0.7, 0.69, 0.6)]]
      : crown === CROWN.molarU || crown === CROWN.molarL || crown === CROWN.primaryMolar ? [[M(0.5, 0.9), C(0.5, 0.78, 0.49, 0.66, 0.5, 0.56)]]
        : [];
  return { crown, roots: ROOTS[rootKey], canals: CANALS[rootKey], grooves };
}

// ───────────────────────────── placing shapes on the screen ─────────────────────────────

export type Placer = {
  x: (u: number) => number;
  y: (v: number) => number;
  /** Screen point of (u, v). */
  pt: (u: number, v: number) => [number, number];
  path: (segs: Seg[]) => string;
  slot: Slot;
};

/** Maps normalized tooth coordinates onto a slot (mesial towards the midline, roots away from the bite). */
export function place(slot: Slot, edgeY: number): Placer {
  const { x: x0, w, crownH, rootH, mesialRight } = slot;
  const upper = slot.info.upper;
  const cej = upper ? edgeY - crownH : edgeY + crownH;
  const x = (u: number) => x0 + (mesialRight ? u : 1 - u) * w;
  const y = (v: number) => {
    const d = v >= 0 ? v * crownH : v * rootH;
    return upper ? cej + d : cej - d;
  };
  const f = (n: number) => Math.round(n * 100) / 100;
  const path = (segs: Seg[]) =>
    segs
      .map((s) => {
        if (s[0] === "Z") return "Z";
        if (s[0] === "C") return `C${f(x(s[1]))} ${f(y(s[2]))} ${f(x(s[3]))} ${f(y(s[4]))} ${f(x(s[5]))} ${f(y(s[6]))}`;
        return `${s[0]}${f(x(s[1]))} ${f(y(s[2]))}`;
      })
      .join("");
  return { x, y, pt: (u, v) => [x(u), y(v)], path, slot };
}

// ───────────────────────────── occlusal view and its five surfaces ─────────────────────────────

export type Surface = "M" | "O" | "D" | "B" | "L";
export const SURFACES: Surface[] = ["M", "O", "D", "B", "L"];

type Outline = { n: number; lobes: number; amp: number; inner: [number, number] };

function outlineOf(info: ToothInfo): Outline {
  switch (info.type) {
    case "central":
    case "lateral":
    case "pA":
    case "pB":
      return { n: 2.1, lobes: 0, amp: 0, inner: [0.66, 0.24] };
    case "canine":
    case "pC":
      return { n: 1.75, lobes: 0, amp: 0, inner: [0.5, 0.36] };
    case "premolar1":
    case "premolar2":
      return { n: 2.4, lobes: 2, amp: 0.03, inner: [0.5, 0.42] };
    default:
      return { n: info.upper ? 3.1 : 3.4, lobes: info.upper ? 4 : 5, amp: 0.035, inner: [0.54, 0.5] };
  }
}

function superPoint(cx: number, cy: number, a: number, b: number, n: number, theta: number, lobes: number, amp: number): [number, number] {
  const c = Math.cos(theta);
  const s = Math.sin(theta);
  const k = 1 + amp * Math.cos(lobes * theta);
  return [cx + a * k * Math.sign(c) * Math.abs(c) ** (2 / n), cy + b * k * Math.sign(s) * Math.abs(s) ** (2 / n)];
}

const STEPS = 72;

export type Occlusal = {
  outline: string;
  center: string;
  /** Each surface's clickable region (the center is O/I). */
  regions: Record<Surface, string>;
  /** Fissures / incisal edge, decoration. */
  grooves: string[];
  cx: number;
  cy: number;
  a: number;
  b: number;
};

/**
 * The tooth seen from the bite: an outline (superellipse, lobed for the cusps of premolars and
 * molars), the occlusal table in the middle and the four sides split along the diagonals. The buccal
 * side faces the outside of the chart (up for the upper jaw), mesial faces the midline.
 */
export function occlusal(slot: Slot, top: number): Occlusal {
  const o = outlineOf(slot.info);
  const cx = slot.x + slot.w / 2;
  const cy = top + OCC_H / 2;
  const a = (slot.w / 2) * 0.93;
  const b = Math.min(slot.depth / 2, OCC_H / 2 - 3);
  const ia = a * o.inner[0];
  const ib = b * o.inner[1];
  const outerAt = (t: number) => superPoint(cx, cy, a, b, o.n, t, o.lobes, o.amp);
  const innerAt = (t: number) => superPoint(cx, cy, ia, ib, o.n, t, 0, 0);
  const f = (p: [number, number]) => `${Math.round(p[0] * 100) / 100} ${Math.round(p[1] * 100) / 100}`;
  const ring = (at: (t: number) => [number, number]) => {
    const pts: string[] = [];
    for (let i = 0; i < STEPS; i++) pts.push(f(at((i / STEPS) * Math.PI * 2)));
    return `M${pts.join("L")}Z`;
  };
  const corner = Math.atan2(b, a);
  const sector = (from: number, to: number) => {
    const pts: string[] = [];
    const n = 18;
    for (let i = 0; i <= n; i++) pts.push(f(outerAt(from + ((to - from) * i) / n)));
    for (let i = n; i >= 0; i--) pts.push(f(innerAt(from + ((to - from) * i) / n)));
    return `M${pts.join("L")}Z`;
  };
  const right = sector(-corner, corner);
  const bottom = sector(corner, Math.PI - corner);
  const left = sector(Math.PI - corner, Math.PI + corner);
  const topS = sector(Math.PI + corner, 2 * Math.PI - corner);
  const upper = slot.info.upper;
  const regions: Record<Surface, string> = {
    O: ring(innerAt),
    M: slot.mesialRight ? right : left,
    D: slot.mesialRight ? left : right,
    B: upper ? topS : bottom,
    L: upper ? bottom : topS,
  };
  const t = slot.info.type;
  const grooves: string[] = [];
  const r = (n: number) => Math.round(n * 100) / 100;
  if (t === "central" || t === "lateral" || t === "pA" || t === "pB") {
    grooves.push(`M${r(cx - ia * 0.9)} ${r(cy)}Q${r(cx)} ${r(cy + (upper ? -ib : ib) * 0.6)} ${r(cx + ia * 0.9)} ${r(cy)}`);
  } else if (t === "premolar1" || t === "premolar2") {
    grooves.push(`M${r(cx - ia * 0.85)} ${r(cy)}C${r(cx - ia * 0.3)} ${r(cy - ib * 0.25)} ${r(cx + ia * 0.3)} ${r(cy + ib * 0.25)} ${r(cx + ia * 0.85)} ${r(cy)}`);
  } else if (t !== "canine" && t !== "pC") {
    grooves.push(`M${r(cx - ia * 0.9)} ${r(cy + ib * 0.1)}C${r(cx - ia * 0.3)} ${r(cy - ib * 0.3)} ${r(cx + ia * 0.3)} ${r(cy + ib * 0.3)} ${r(cx + ia * 0.9)} ${r(cy - ib * 0.1)}`);
    grooves.push(`M${r(cx - ia * 0.1)} ${r(cy - ib * 0.95)}L${r(cx)} ${r(cy)}L${r(cx + ia * 0.15)} ${r(cy + ib * 0.95)}`);
  }
  return { outline: ring(outerAt), center: regions.O, regions, grooves, cx, cy, a, b };
}

/** Names a surface by the tooth: incisal / labial on front teeth, occlusal / buccal on back teeth; palatal above. */
export function surfaceKey(s: Surface, info: ToothInfo): string {
  if (s === "O") return isAnterior(info.type) ? "I" : "O";
  if (s === "B") return isAnterior(info.type) ? "F" : "B";
  if (s === "L") return info.upper ? "P" : "L";
  return s;
}
