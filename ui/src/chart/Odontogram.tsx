import { useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent, ReactNode } from "react";
import type { Dentition } from "../../../shared/ts/contract";
import { BANDS, CHART_W, NUM_H, OCC_H, layout, occlusal, place, shapesOf } from "./geometry";
import type { Placer, Slot, Surface } from "./geometry";
import type { ChartState, ChartView, ToothLayers } from "./model";

/** One label outside the chart: "۳۶ — عصب‌کشی — جلسه ۲ از ۳ — در حال انجام" (owner's idea, M5). */
export type Callout = {
  key: string;
  teeth: number[];
  state: ChartState;
  /** The tooth number(s), shown first and bold. */
  head: string;
  text: string;
  /** An existing condition: labelled only while its tooth is selected (the drawing already shows it). */
  finding?: boolean;
};

export type Span = { key: string; symbol: "bridge" | "denture" | "bracket"; state: ChartState; teeth: number[] };

type Props = {
  dentition: Dentition;
  primarySlots: number[];
  layers: Map<number, ToothLayers>;
  spans: Span[];
  callouts: Callout[];
  labels: "all" | "selected";
  view: ChartView;
  selected: number[];
  selectedSurfaces: Surface[];
  numberOf: (fdi: number) => string;
  titleOf: (fdi: number) => string;
  sideLabels: { right: string; left: string };
  ariaLabel: string;
  onTooth: (fdi: number, mode: "single" | "toggle" | "range") => void;
  onSurface: (fdi: number, s: Surface) => void;
  testId?: string;
};

const ROW_H = 26;
const LABEL_FONT = '500 12px "AfghaniSign", "Artaveo Sans", Vazirmatn, "Segoe UI", sans-serif';
const HEAD_FONT = '700 12px "AfghaniSign", "Artaveo Sans", Vazirmatn, "Segoe UI", sans-serif';
const MAX_ROWS = 4;

let probe: HTMLSpanElement | null = null;
/** Width of a text in the label's own font, measured in the page (the web fonts are the ones on screen). */
function textWidth(s: string, font = LABEL_FONT): number {
  if (typeof document === "undefined") return s.length * 7;
  if (!probe) {
    probe = document.createElement("span");
    probe.setAttribute("aria-hidden", "true");
    // Fixed and off to no side: in a right-to-left page a box far to the left would widen the scroll area.
    probe.style.cssText = "position:fixed;top:0;left:0;visibility:hidden;pointer-events:none;white-space:pre;";
    document.body.appendChild(probe);
  }
  probe.style.font = font;
  probe.textContent = s;
  return probe.getBoundingClientRect().width;
}

type Placed = Callout & { x: number; w: number; row: number; anchor: number; upper: boolean };

/**
 * Labels in rows above (upper jaw) and below (lower jaw) the chart, each as close above its tooth as the
 * labels before it allow; a row that is full sends the label to the next one. Leader lines run under the
 * labels' solid backgrounds, so no line is ever drawn across a label's text.
 */
function placeLabels(items: (Callout & { anchor: number; upper: boolean })[], width: number): { placed: Placed[]; rows: { upper: number; lower: number }; hidden: number } {
  const placed: Placed[] = [];
  let hidden = 0;
  const rows = { upper: 0, lower: 0 };
  for (const upper of [true, false]) {
    const ends: number[] = [];
    const list = items.filter((c) => c.upper === upper).sort((a, b) => a.anchor - b.anchor);
    for (const c of list) {
      // Padding, the status dot, the gaps and the border around the bold tooth number and the words.
      const w = Math.min(width * 0.46, Math.ceil(textWidth(c.head, HEAD_FONT) + textWidth(c.text) + 46));
      const want = Math.max(6, Math.min(width - 6 - w, c.anchor - w / 2));
      let done = false;
      for (let r = 0; r < MAX_ROWS && !done; r++) {
        const end = ends[r] ?? -Infinity;
        const x = Math.max(want, end + 6);
        if (x + w <= width - 6 && x - want <= width * 0.3) {
          placed.push({ ...c, x, w, row: r, anchor: c.anchor, upper });
          ends[r] = x + w;
          done = true;
        }
      }
      if (!done) hidden++;
    }
    rows[upper ? "upper" : "lower"] = ends.length;
  }
  return { placed, rows, hidden };
}

const STATE_VAR: Record<ChartState, string> = {
  existing: "var(--st-existing)",
  planned: "var(--st-planned)",
  progress: "var(--st-progress)",
  done: "var(--st-done)",
};

/**
 * The dental chart (5.2–5.4, M5): two views of every tooth — crown and root from the front, and the
 * biting surface with its five clickable surfaces — the existing conditions and the planned, ongoing and
 * finished work in their colours and patterns, and the labels outside it. The drawing is the patient's
 * mouth as the doctor faces it, the same in a right-to-left and a left-to-right interface.
 */
export function Odontogram(p: Props) {
  const uid = useId().replace(/:/g, "");
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(1000);
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(Math.max(320, el.clientWidth)));
    ro.observe(el);
    setWidth(Math.max(320, el.clientWidth));
    return () => ro.disconnect();
  }, []);
  const [hover, setHover] = useState<number | null>(null);
  const [hoverLabel, setHoverLabel] = useState<string | null>(null);
  const [fontsReady, setFontsReady] = useState(0);
  useLayoutEffect(() => {
    document.fonts?.ready.then(() => setFontsReady((x) => x + 1));
  }, []);
  const { upper, lower } = useMemo(() => layout(p.dentition, p.primarySlots), [p.dentition, p.primarySlots]);
  const slots = useMemo(() => new Map([...upper, ...lower].map((s) => [s.fdi, s])), [upper, lower]);
  const s = width / CHART_W;
  const center = (fdi: number) => {
    const sl = slots.get(fdi);
    return sl ? (sl.x + sl.w / 2) * s : null;
  };

  const shown = p.callouts
    .filter((c) => (p.labels === "all" && !c.finding) || c.teeth.some((t) => p.selected.includes(t)))
    .map((c) => {
      const xs = c.teeth.map(center).filter((x): x is number => x != null);
      const up = c.teeth.filter((t) => slots.get(t)?.info.upper).length >= c.teeth.length / 2;
      return xs.length ? { ...c, anchor: xs.reduce((a, b) => a + b, 0) / xs.length, upper: up } : null;
    })
    .filter((c): c is Callout & { anchor: number; upper: boolean } => !!c);
  const { placed, rows, hidden } = useMemo(() => placeLabels(shown, width), [JSON.stringify(shown.map((c) => [c.key, c.anchor, c.text, c.head])), width, fontsReady]);
  const topBand = rows.upper ? rows.upper * ROW_H + 18 : 8;
  const bottomBand = rows.lower ? rows.lower * ROW_H + 18 : 8;
  const chartH = BANDS.height * s;
  const height = topBand + chartH + bottomBand;
  const y = (v: number) => topBand + v * s;

  const click = (fdi: number) => (e: MouseEvent) => p.onTooth(fdi, e.shiftKey ? "range" : e.ctrlKey || e.metaKey ? "toggle" : "single");
  const key = (fdi: number) => (e: KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      p.onTooth(fdi, e.shiftKey ? "range" : e.ctrlKey || e.metaKey ? "toggle" : "single");
    }
  };

  const labelHighlights = new Set(placed.filter((c) => c.key === hoverLabel).flatMap((c) => c.teeth));

  return (
    <div className="odo" ref={box} data-testid={p.testId ?? "odontogram"} data-view={p.view}>
      <svg className="odo-svg" width={width} height={height} viewBox={`0 0 ${width} ${height}`} role="group" aria-label={p.ariaLabel}>
        <Defs uid={uid} />
        {/* Leader lines first: the labels' solid chips are drawn over them. */}
        <g className="odo-leaders">
          {placed.map((c) => {
            const labelY = c.upper ? topBand - 12 - (c.row + 1) * ROW_H + ROW_H / 2 : topBand + chartH + 12 + c.row * ROW_H + ROW_H / 2 - 2;
            const startY = c.upper ? y(BANDS.upperNum) + 2 : y(BANDS.lowerNum + NUM_H) - 2;
            const inside = c.anchor >= c.x + 8 && c.anchor <= c.x + c.w - 8;
            const edge = c.anchor < c.x ? c.x : c.x + c.w;
            const d = inside ? `M${c.anchor} ${startY}V${labelY}` : `M${c.anchor} ${startY}V${labelY}H${edge}`;
            const active = hoverLabel === c.key || c.teeth.some((t) => p.selected.includes(t));
            return (
              <g key={c.key} className={`odo-leader ${active ? "on" : ""}`} style={{ color: STATE_VAR[c.state] }}>
                <path d={d} />
                <circle cx={c.anchor} cy={startY} r={2.4} />
                {c.teeth.length > 1 && (() => {
                  const xs = c.teeth.map(center).filter((x): x is number => x != null);
                  return <path className="odo-span" d={`M${Math.min(...xs)} ${startY}H${Math.max(...xs)}`} />;
                })()}
              </g>
            );
          })}
        </g>

        <g transform={`translate(0 ${topBand}) scale(${s})`}>
          <Guides uid={uid} sideLabels={p.sideLabels} />
          <GumBand uid={uid} slots={upper} />
          <GumBand uid={uid} slots={lower} />
          {[...upper, ...lower].map((slot) => (
            <ToothColumn
              key={slot.fdi}
              uid={uid}
              slot={slot}
              layers={p.layers.get(slot.fdi)}
              view={p.view}
              selected={p.selected.includes(slot.fdi)}
              highlighted={hover === slot.fdi || labelHighlights.has(slot.fdi)}
              selectedSurfaces={p.selected.length === 1 && p.selected[0] === slot.fdi ? p.selectedSurfaces : []}
              title={p.titleOf(slot.fdi)}
              onClick={click(slot.fdi)}
              onKey={key(slot.fdi)}
              onEnter={() => setHover(slot.fdi)}
              onLeave={() => setHover((h) => (h === slot.fdi ? null : h))}
              onSurface={(srf) => p.onSurface(slot.fdi, srf)}
            />
          ))}
          <SpanOverlay spans={p.spans} slots={slots} />
        </g>

        {/* Tooth numbers, outside the drawing (FDI by default, ADR-14), at screen size. */}
        <g className="odo-numbers">
          {[...upper, ...lower].map((slot) => {
            const cx = (slot.x + slot.w / 2) * s;
            const cy = y(slot.info.upper ? BANDS.upperNum + NUM_H / 2 : BANDS.lowerNum + NUM_H / 2);
            const label = p.numberOf(slot.fdi);
            const w = Math.max(24, textWidth(label, '600 12px "Artaveo Sans", sans-serif') + 12);
            const markers = p.layers.get(slot.fdi)?.markers ?? [];
            const sel = p.selected.includes(slot.fdi);
            return (
              <g key={slot.fdi} className={`odo-num ${sel ? "sel" : ""}`} onClick={click(slot.fdi)} data-testid={`tooth-num-${slot.fdi}`}>
                <rect x={cx - w / 2} y={cy - 10} width={w} height={20} rx={10} />
                <text x={cx} y={cy + 4.2} textAnchor="middle" direction="ltr">{label}</text>
                {markers.slice(0, 2).map((m, i) => (
                  <g key={i} className="odo-marker" style={{ color: STATE_VAR[m.state] }} transform={`translate(${cx + w / 2 + 3 + i * 22} ${cy - 8})`}>
                    <rect width={20} height={16} rx={5} />
                    <text x={10} y={11.5} textAnchor="middle" direction="ltr">{m.abbr}</text>
                  </g>
                ))}
              </g>
            );
          })}
        </g>

        <g className="odo-labels">
          {placed.map((c) => {
            const top = c.upper ? topBand - 12 - (c.row + 1) * ROW_H + 2 : topBand + chartH + 12 + c.row * ROW_H;
            return (
              <foreignObject key={c.key} x={c.x} y={top} width={c.w} height={ROW_H - 4}>
                <div
                  className={`odo-callout st-${c.state} ${hoverLabel === c.key ? "hover" : ""}`}
                  title={`${c.head} — ${c.text}`}
                  onMouseEnter={() => setHoverLabel(c.key)}
                  onMouseLeave={() => setHoverLabel((h) => (h === c.key ? null : h))}
                  onClick={() => p.onTooth(c.teeth[0], "single")}
                  data-testid={`callout-${c.key}`}
                >
                  <span className="odo-callout-dot" aria-hidden />
                  <b><bdi>{c.head}</bdi></b>
                  <span className="odo-callout-text">{c.text}</span>
                </div>
              </foreignObject>
            );
          })}
        </g>
      </svg>
      {hidden > 0 && <div className="odo-hidden" data-testid="odo-hidden">+{hidden}</div>}
    </div>
  );
}

function Defs({ uid }: { uid: string }) {
  return (
    <defs>
      <linearGradient id={`${uid}-enamel`} x1="0" x2="1" y1="0" y2="0">
        <stop offset="0" stopColor="var(--tooth-e3)" />
        <stop offset=".2" stopColor="var(--tooth-e2)" />
        <stop offset=".5" stopColor="var(--tooth-e1)" />
        <stop offset=".8" stopColor="var(--tooth-e2)" />
        <stop offset="1" stopColor="var(--tooth-e3)" />
      </linearGradient>
      <linearGradient id={`${uid}-cervical`} x1="0" x2="0" y1="0" y2="1">
        <stop offset="0" stopColor="var(--tooth-cervical)" stopOpacity=".55" />
        <stop offset=".45" stopColor="var(--tooth-cervical)" stopOpacity="0" />
      </linearGradient>
      <linearGradient id={`${uid}-root`} x1="0" x2="1" y1="0" y2="0">
        <stop offset="0" stopColor="var(--tooth-r2)" />
        <stop offset=".5" stopColor="var(--tooth-r1)" />
        <stop offset="1" stopColor="var(--tooth-r2)" />
      </linearGradient>
      <radialGradient id={`${uid}-occ`} cx=".5" cy=".45" r=".6">
        <stop offset="0" stopColor="var(--tooth-e1)" />
        <stop offset=".7" stopColor="var(--tooth-e2)" />
        <stop offset="1" stopColor="var(--tooth-e3)" />
      </radialGradient>
      <linearGradient id={`${uid}-gum-u`} x1="0" x2="0" y1="0" y2="1">
        <stop offset="0" stopColor="var(--gum)" stopOpacity="0" />
        <stop offset=".55" stopColor="var(--gum)" stopOpacity=".32" />
        <stop offset="1" stopColor="var(--gum)" stopOpacity=".62" />
      </linearGradient>
      <linearGradient id={`${uid}-gum-l`} x1="0" x2="0" y1="1" y2="0">
        <stop offset="0" stopColor="var(--gum)" stopOpacity="0" />
        <stop offset=".55" stopColor="var(--gum)" stopOpacity=".32" />
        <stop offset="1" stopColor="var(--gum)" stopOpacity=".62" />
      </linearGradient>
      <radialGradient id={`${uid}-shine`} cx=".5" cy=".5" r=".5">
        <stop offset="0" stopColor="#fff" stopOpacity=".75" />
        <stop offset="1" stopColor="#fff" stopOpacity="0" />
      </radialGradient>
      <linearGradient id={`${uid}-metal`} x1="0" x2="1" y1="0" y2="0">
        <stop offset="0" stopColor="var(--metal-2)" />
        <stop offset=".45" stopColor="var(--metal-1)" />
        <stop offset="1" stopColor="var(--metal-3)" />
      </linearGradient>
      {(["existing", "planned", "progress", "done"] as ChartState[]).map((st) => (
        <pattern key={st} id={`${uid}-hatch-${st}`} width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <rect width="5" height="5" style={{ fill: STATE_VAR[st], fillOpacity: 0.16 }} />
          <line x1="0" y1="0" x2="0" y2="5" style={{ stroke: STATE_VAR[st] }} strokeWidth="2.2" />
        </pattern>
      ))}
    </defs>
  );
}

/** Midline, occlusal plane and which side is the patient's right. */
function Guides({ sideLabels }: { uid: string; sideLabels: { right: string; left: string } }) {
  return (
    <g className="odo-guides" aria-hidden>
      <line x1={CHART_W / 2} x2={CHART_W / 2} y1={BANDS.upperSideTop - 4} y2={BANDS.lowerNum + 4} className="odo-midline" />
      <line x1={14} x2={CHART_W - 14} y1={BANDS.midY} y2={BANDS.midY} className="odo-occlusal-plane" />
      <text x={22} y={BANDS.midY + 4} className="odo-side" textAnchor="start" direction="ltr">{sideLabels.right}</text>
      <text x={CHART_W - 22} y={BANDS.midY + 4} className="odo-side" textAnchor="end" direction="ltr">{sideLabels.left}</text>
    </g>
  );
}

/** Fill of a mark: solid for existing / in progress / done, hatched for planned (never colour alone). */
function fillOf(uid: string, st: ChartState): string {
  return st === "planned" ? `url(#${uid}-hatch-planned)` : STATE_VAR[st];
}

function ToothColumn({ uid, slot, layers, view, selected, highlighted, selectedSurfaces, title, onClick, onKey, onEnter, onLeave, onSurface }: {
  uid: string;
  slot: Slot;
  layers: ToothLayers | undefined;
  view: ChartView;
  selected: boolean;
  highlighted: boolean;
  selectedSurfaces: Surface[];
  title: string;
  onClick: (e: MouseEvent) => void;
  onKey: (e: KeyboardEvent) => void;
  onEnter: () => void;
  onLeave: () => void;
  onSurface: (s: Surface) => void;
}) {
  const upper = slot.info.upper;
  const P = place(slot, upper ? BANDS.upperEdge : BANDS.lowerEdge);
  const occ = occlusal(slot, upper ? BANDS.upperOccTop : BANDS.lowerOccTop);
  const sh = shapesOf(slot.info);
  const L = layers;
  const dim = view !== "general" && view !== "perio" && !L?.relevant;
  const replaced = !!L?.implant;
  const ghost = !!L?.ghost && !replaced && !L?.pontic && !L?.denture;
  const artificial = !!L?.pontic || (!!L?.denture && !!L?.ghost);
  const crownPath = P.path(sh.crown);
  const clip = `${uid}-clip-${slot.fdi}`;
  const colTop = upper ? BANDS.upperNum : BANDS.lowerOccTop - 2;
  const colBottom = upper ? BANDS.upperOccTop + OCC_H + 2 : BANDS.lowerNum + NUM_H;
  const tilt = L?.impacted ? `rotate(${upper ? -12 : 12} ${P.x(0.5)} ${P.y(0.3)})` : undefined;

  return (
    <g
      className={`odo-tooth ${selected ? "sel" : ""} ${highlighted ? "hl" : ""} ${dim ? "dim" : ""}`}
      data-testid={`tooth-${slot.fdi}`}
      data-selected={selected || undefined}
      role="button"
      tabIndex={0}
      aria-pressed={selected}
      aria-label={title}
      onKeyDown={onKey}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
    >
      <title>{title}</title>
      <rect className="odo-col" x={slot.x - 1.5} y={colTop} width={slot.w + 3} height={colBottom - colTop} rx={10} onClick={onClick} />
      <clipPath id={clip}><path d={crownPath} /></clipPath>

      {/* front view: roots, gum, crown */}
      <g className="odo-side-view" onClick={onClick} transform={tilt}>
        {replaced ? (
          <Implant P={P} uid={uid} layers={L!} crownPath={crownPath} />
        ) : ghost || artificial ? (
          <g className="odo-ghost">
            {sh.roots.map((r, i) => <path key={i} d={P.path(r)} />)}
            {!artificial && <path d={crownPath} />}
          </g>
        ) : (
          sh.roots.map((r, i) => <path key={i} className={i === 0 && sh.roots.length === 3 ? "odo-root back" : "odo-root"} d={P.path(r)} fill={`url(#${uid}-root)`} />)
        )}
        {(!ghost || artificial) && !replaced && (
          <>
            <path className="odo-crown" d={crownPath} fill={`url(#${uid}-enamel)`} />
            <path d={crownPath} fill={`url(#${uid}-cervical)`} className="odo-nopointer" />
            <ellipse className="odo-nopointer" cx={P.x(0.42)} cy={P.y(0.5)} rx={slot.w * 0.16} ry={slot.crownH * 0.26} fill={`url(#${uid}-shine)`} />
            {sh.grooves.map((g, i) => <path key={i} className="odo-groove" d={P.path(g)} />)}
          </>
        )}
        {L && !ghost && <SideMarks P={P} uid={uid} L={L} clip={clip} crownPath={crownPath} canals={sh.canals} replaced={replaced} />}
        {L?.lesion && <circle className="odo-mark" cx={P.x(0.5)} cy={P.y(-1.02)} r={slot.w * 0.16} style={{ fill: STATE_VAR[L.lesion], fillOpacity: 0.35, stroke: STATE_VAR[L.lesion] }} />}
        {L?.impacted && <rect className="odo-impacted" x={slot.x + 1} y={Math.min(P.y(-1), P.y(1)) - 2} width={slot.w - 2} height={Math.abs(P.y(1) - P.y(-1)) + 4} rx={8} style={{ stroke: STATE_VAR[L.impacted] }} />}
        {L?.extraction && <Cross x1={slot.x + slot.w * 0.12} x2={slot.x + slot.w * 0.88} y1={P.y(-0.95)} y2={P.y(0.98)} state={L.extraction} />}
      </g>

      {/* biting surface and its five surfaces */}
      <g className="odo-occ-view">
        {ghost ? (
          <path className="odo-ghost" d={occ.outline} onClick={onClick} />
        ) : (
          <>
            <path className="odo-occ" d={occ.outline} fill={`url(#${uid}-occ)`} />
            {(["O", "M", "D", "B", "L"] as Surface[]).map((srf) => {
              const st = L?.surfaces[srf];
              const on = selectedSurfaces.includes(srf);
              return (
                <path
                  key={srf}
                  className={`odo-surface ${on ? "on" : ""}`}
                  d={occ.regions[srf]}
                  style={st ? { fill: fillOf(uid, st), fillOpacity: st === "planned" ? 1 : 0.88 } : undefined}
                  onClick={(e) => {
                    e.stopPropagation();
                    onSurface(srf);
                  }}
                  data-testid={`surface-${slot.fdi}-${srf}`}
                  data-state={st}
                  data-on={on || undefined}
                />
              );
            })}
            {occ.grooves.map((g, i) => <path key={i} className="odo-fissure" d={g} style={L?.sealant ? { stroke: STATE_VAR[L.sealant], strokeWidth: 2.4 } : undefined} />)}
            {(L?.crown || (replaced && L?.implant?.crown)) && (() => {
              const st = (L?.crown ?? L?.implant?.crown)!;
              return <path className="odo-mark odo-nopointer" d={occ.outline} style={{ fill: fillOf(uid, st), fillOpacity: st === "planned" ? 0.9 : 0.24, stroke: STATE_VAR[st], strokeWidth: 2 }} />;
            })()}
            {replaced && !L?.implant?.crown && <circle className="odo-mark odo-nopointer" cx={occ.cx} cy={occ.cy} r={Math.min(occ.a, occ.b) * 0.45} fill={`url(#${uid}-metal)`} style={{ stroke: STATE_VAR[(L!.implant!.abutment ?? L!.implant!.fixture)!] }} />}
            {L?.veneer && <path className="odo-mark odo-nopointer" d={occ.regions.B} style={{ fill: fillOf(uid, L.veneer), fillOpacity: 0.8 }} />}
          </>
        )}
        {L?.extraction && <Cross x1={occ.cx - occ.a * 0.7} x2={occ.cx + occ.a * 0.7} y1={occ.cy - occ.b * 0.8} y2={occ.cy + occ.b * 0.8} state={L.extraction} />}
      </g>
    </g>
  );
}

/**
 * The gum of one jaw: a soft band behind the roots, fading towards the apices, its edge following the
 * necks of the teeth with a papilla between each two (gives the chart its "mouth").
 */
function GumBand({ uid, slots }: { uid: string; slots: Slot[] }) {
  if (!slots.length) return null;
  const upper = slots[0].info.upper;
  const edge = upper ? BANDS.upperEdge : BANDS.lowerEdge;
  const dir = upper ? -1 : 1;
  const cej = (s: Slot) => edge + dir * s.crownH;
  const depth = Math.max(...slots.map((s) => s.crownH + s.rootH * 0.5));
  const far = edge + dir * depth;
  const f = (n: number) => Math.round(n * 100) / 100;
  // Along the necks, left to right: papilla at each tooth's sides (towards the crown), arch over its middle.
  let d = `M${f(slots[0].x - 3)} ${f(far)}`;
  for (const s of slots) {
    const neck = cej(s);
    const papilla = neck - dir * s.crownH * 0.2;
    const mid = neck + dir * s.rootH * 0.04;
    const ctrl = 2 * mid - papilla;
    d += `L${f(s.x)} ${f(papilla)}Q${f(s.x + s.w / 2)} ${f(ctrl)} ${f(s.x + s.w)} ${f(papilla)}`;
  }
  const last = slots[slots.length - 1];
  d += `L${f(last.x + last.w + 3)} ${f(far)}Z`;
  return <path className="odo-gum" d={d} fill={`url(#${uid}-gum-${upper ? "u" : "l"})`} />;
}

function Cross({ x1, x2, y1, y2, state }: { x1: number; x2: number; y1: number; y2: number; state: ChartState }) {
  return (
    <g className={`odo-cross st-${state}`} style={{ stroke: STATE_VAR[state] }}>
      <line x1={x1} y1={y1} x2={x2} y2={y2} />
      <line x1={x2} y1={y1} x2={x1} y2={y2} />
    </g>
  );
}

/** Marks on the front view: surfaces (clipped to the crown), crown, veneer, canals, pulpotomy, bracket, fracture, scaling. */
function SideMarks({ P, uid, L, clip, crownPath, canals, replaced }: { P: Placer; uid: string; L: ToothLayers; clip: string; crownPath: string; canals: [number, number, number][]; replaced: boolean }) {
  const { w, crownH } = P.slot;
  const rect = (u0: number, u1: number, v0: number, v1: number) => {
    const xa = P.x(u0);
    const xb = P.x(u1);
    const ya = P.y(v0);
    const yb = P.y(v1);
    return { x: Math.min(xa, xb), y: Math.min(ya, yb), width: Math.abs(xb - xa), height: Math.abs(yb - ya) };
  };
  const zones: Partial<Record<Surface, ReactNode>> = {};
  const S = L.surfaces;
  const style = (st: ChartState) => ({ fill: fillOf(uid, st), fillOpacity: st === "planned" ? 1 : 0.85 });
  if (S.M) zones.M = <rect key="M" {...rect(0.74, 1.08, 0.06, 1.05)} style={style(S.M)} />;
  if (S.D) zones.D = <rect key="D" {...rect(-0.08, 0.26, 0.06, 1.05)} style={style(S.D)} />;
  if (S.O) zones.O = <rect key="O" {...rect(0.16, 0.84, 0.8, 1.08)} style={style(S.O)} />;
  if (S.B) zones.B = <ellipse key="B" cx={P.x(0.5)} cy={P.y(0.5)} rx={w * 0.24} ry={crownH * 0.24} style={style(S.B)} />;
  if (S.L) zones.L = <rect key="L" {...rect(0.22, 0.78, 0.04, 0.2)} style={{ ...style(S.L), fillOpacity: 0.5 }} />;
  const cut = (n: number) => Math.round(n * 100) / 100;
  return (
    <g className="odo-marks odo-nopointer">
      {!replaced && <g clipPath={`url(#${clip})`}>{Object.values(zones)}</g>}
      {L.veneer && !replaced && <ellipse cx={P.x(0.5)} cy={P.y(0.55)} rx={w * 0.36} ry={crownH * 0.4} clipPath={`url(#${clip})`} style={{ fill: fillOf(uid, L.veneer), fillOpacity: 0.55 }} />}
      {(L.crown || L.pontic || L.denture) && !replaced && (() => {
        const st = (L.crown ?? L.pontic ?? L.denture)!;
        return <path d={crownPath} className="odo-crown-mark" style={{ fill: fillOf(uid, st), fillOpacity: st === "planned" ? 0.55 : 0.26, stroke: STATE_VAR[st], strokeDasharray: st === "planned" ? "5 3" : undefined }} />;
      })()}
      {L.rootCanal && !replaced && canals.map(([u0, u1, va], i) => {
        const [x0, y0] = P.pt(u0, 0.05);
        const [x1, y1] = P.pt(u1, va);
        const [cx, cy] = P.pt(u0 + (u1 - u0) * 0.3, va * 0.5);
        return <path key={i} className={`odo-canal st-${L.rootCanal}`} d={`M${cut(x0)} ${cut(y0)}Q${cut(cx)} ${cut(cy)} ${cut(x1)} ${cut(y1)}`} style={{ stroke: STATE_VAR[L.rootCanal!] }} />;
      })}
      {L.pulpotomy && !replaced && <ellipse cx={P.x(0.5)} cy={P.y(0.1)} rx={w * 0.14} ry={crownH * 0.12} style={{ fill: STATE_VAR[L.pulpotomy], fillOpacity: 0.85 }} />}
      {L.bracket && (
        <g>
          <rect x={P.x(0.5) - w * 0.17} y={P.y(0.52) - crownH * 0.12} width={w * 0.34} height={crownH * 0.24} rx={2.5} fill={`url(#${uid}-metal)`} style={{ stroke: STATE_VAR[L.bracket] }} strokeWidth={1.6} />
          <line x1={P.x(0.5) - w * 0.17} x2={P.x(0.5) + w * 0.17} y1={P.y(0.52)} y2={P.y(0.52)} className="odo-bracket-slot" />
        </g>
      )}
      {L.fracture && <polyline className="odo-fracture" points={[[0.32, 0.95], [0.46, 0.78], [0.38, 0.62], [0.56, 0.45], [0.5, 0.28]].map(([u, v]) => P.pt(u, v).map(cut).join(",")).join(" ")} style={{ stroke: STATE_VAR[L.fracture] }} />}
      {L.scaling && <path className="odo-scaling" d={`M${P.x(0.02)} ${P.y(0.02)}Q${P.x(0.5)} ${P.y(-0.12)} ${P.x(0.98)} ${P.y(0.02)}`} style={{ stroke: STATE_VAR[L.scaling] }} />}
    </g>
  );
}

/** An implant instead of the root: the threaded fixture, the abutment, and the crown once it is placed. */
function Implant({ P, uid, layers, crownPath }: { P: Placer; uid: string; layers: ToothLayers; crownPath: string }) {
  const im = layers.implant!;
  const f = (n: number) => Math.round(n * 100) / 100;
  const st = im.fixture ?? im.abutment ?? im.crown ?? "existing";
  const body = [
    ["M", 0.37, 0.02], ["L", 0.63, 0.02], ["L", 0.6, -0.78], ["C", 0.58, -0.9, 0.42, -0.9, 0.4, -0.78], ["Z"],
  ] as const;
  const d = body.map((s) => (s[0] === "Z" ? "Z" : s[0] === "C" ? `C${f(P.x(s[1]))} ${f(P.y(s[2]))} ${f(P.x(s[3]))} ${f(P.y(s[4]))} ${f(P.x(s[5]))} ${f(P.y(s[6]))}` : `${s[0]}${f(P.x(s[1]))} ${f(P.y(s[2]))}`)).join("");
  const threads = Array.from({ length: 7 }, (_, i) => -0.1 - i * 0.1);
  return (
    <g className="odo-implant">
      <path d={d} fill={`url(#${uid}-metal)`} style={{ stroke: STATE_VAR[st] }} strokeWidth={1.4} strokeDasharray={im.fixture === "planned" ? "3 2" : undefined} />
      {threads.map((v, i) => (
        <line key={i} className="odo-thread" x1={P.x(0.38 + i * 0.003)} x2={P.x(0.62 - i * 0.003)} y1={P.y(v)} y2={P.y(v - 0.04)} />
      ))}
      {(im.abutment || im.crown) && (
        <path d={`M${f(P.x(0.4))} ${f(P.y(0.02))}L${f(P.x(0.6))} ${f(P.y(0.02))}L${f(P.x(0.56))} ${f(P.y(0.45))}L${f(P.x(0.44))} ${f(P.y(0.45))}Z`} fill={`url(#${uid}-metal)`} style={{ stroke: STATE_VAR[(im.abutment ?? im.crown)!] }} strokeWidth={1.2} />
      )}
      {im.crown ? (
        <>
          <path className="odo-crown" d={crownPath} fill={`url(#${uid}-enamel)`} />
          <path d={crownPath} className="odo-crown-mark" style={{ fill: fillOf(uid, im.crown), fillOpacity: im.crown === "planned" ? 0.55 : 0.22, stroke: STATE_VAR[im.crown], strokeDasharray: im.crown === "planned" ? "5 3" : undefined }} />
        </>
      ) : (
        <path className="odo-ghost" d={crownPath} />
      )}
    </g>
  );
}

/** Bridges joined across their teeth, the wire through brackets, the denture base under its teeth. */
function SpanOverlay({ spans, slots }: { spans: Span[]; slots: Map<number, Slot> }) {
  return (
    <g className="odo-spans odo-nopointer">
      {spans.map((sp) => {
        const list = sp.teeth.map((t) => slots.get(t)).filter((x): x is Slot => !!x).sort((a, b) => a.x - b.x);
        if (list.length < 2) return null;
        const upper = list[0].info.upper;
        const edge = upper ? BANDS.upperEdge : BANDS.lowerEdge;
        const st = STATE_VAR[sp.state];
        if (sp.symbol === "bridge") {
          const yv = (sl: Slot) => (upper ? edge - sl.crownH * 0.55 : edge + sl.crownH * 0.55);
          const d = list.map((sl, i) => `${i ? "L" : "M"}${sl.x + sl.w / 2} ${yv(sl)}`).join("");
          return <path key={sp.key} className={`odo-bridge st-${sp.state}`} d={d} style={{ stroke: st }} />;
        }
        if (sp.symbol === "bracket") {
          const yv = (sl: Slot) => (upper ? edge - sl.crownH * 0.52 : edge + sl.crownH * 0.52);
          const d = list.map((sl, i) => `${i ? "L" : "M"}${sl.x + sl.w / 2} ${yv(sl)}`).join("");
          return <path key={sp.key} className="odo-wire" d={d} style={{ stroke: st }} />;
        }
        const x0 = list[0].x - 2;
        const x1 = list[list.length - 1].x + list[list.length - 1].w + 2;
        const h = 14;
        const yb = upper ? edge - Math.max(...list.map((s) => s.crownH)) - 6 : edge + Math.max(...list.map((s) => s.crownH)) + 6 - h;
        return <rect key={sp.key} className="odo-denture" x={x0} y={yb} width={x1 - x0} height={h} rx={7} style={{ stroke: st }} />;
      })}
    </g>
  );
}
