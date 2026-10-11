import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { ArrowDownRight, ArrowUpRight, FilePlus2, Minus, Save, ScanLine, Trash2 } from "lucide-react";
import type { ChartInfo, PatientInfo, PerioExamInfo, PerioSummary, PerioTooth, ToothNumbering } from "../../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../../lib/api";
import { digits, formatDate, latinDigits } from "../../../lib/dates";
import { useFormDraft } from "../../../lib/draft";
import { useToothConditions } from "../../../lib/clinical";
import { useI18n } from "../../../i18n";
import { Button } from "../../../ui/Button";
import { Field, Select, Textarea } from "../../../ui/Field";
import { EmptyState, ErrorState, Loading, Notice } from "../../../ui/Feedback";
import { useToast } from "../../../ui/Toast";
import { chartTeeth, isMolar, toothInfo } from "../../../chart/geometry";
import { toothLabel } from "../../../chart/numbering";

type Row = "depth" | "recession" | "bleeding" | "plaque";

function blank(tooth: number, missing: boolean, implant: boolean): PerioTooth {
  return { tooth, missing, implant, depth: Array(6).fill(null), recession: Array(6).fill(null), bleeding: Array(6).fill(false), plaque: Array(6).fill(false), mobility: null, furcation: [null, null] };
}

/** Client-side summary while typing (the Core computes the stored one the same way). */
function summarize(teeth: PerioTooth[]): PerioSummary {
  const present = teeth.filter((t) => !t.missing);
  const depths = present.flatMap((t) => t.depth.filter((d): d is number => d != null));
  const all = present.length * 6;
  const pct = (n: number) => (all ? Math.floor((n * 100) / all) : 0);
  return {
    teeth: present.length,
    sites: depths.length,
    mean_depth_x10: depths.length ? Math.floor((depths.reduce((a, b) => a + b, 0) * 10 + depths.length / 2) / depths.length) : 0,
    sites_4_plus: depths.filter((d) => d >= 4).length,
    sites_6_plus: depths.filter((d) => d >= 6).length,
    bleeding_percent: pct(present.reduce((s, t) => s + t.bleeding.filter(Boolean).length, 0)),
    plaque_percent: pct(present.reduce((s, t) => s + t.plaque.filter(Boolean).length, 0)),
  };
}

/** Site indexes in screen order: mesial towards the midline (sites are DB, B, MB, DL, L, ML). */
function siteOrder(fdi: number, lingual: boolean): number[] {
  const q = toothInfo(fdi).quadrant;
  const base = lingual ? [3, 4, 5] : [0, 1, 2];
  return q === 1 || q === 4 ? base : [...base].reverse();
}

/**
 * Periodontal chart (5.5): six sites a tooth — pocket depth, recession, bleeding and plaque — with
 * mobility and furcation, typed site after site (the cursor moves on by itself), a depth graph per jaw
 * and side, and the comparison of two exams.
 */
export function PerioChart({ patient, chart, canEdit, numbering }: { patient: PatientInfo; chart: ChartInfo; canEdit: boolean; numbering: ToothNumbering }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const conditions = useToothConditions();
  const [exams, setExams] = useState<PerioExamInfo[] | null>(null);
  const [error, setError] = useState("");
  const [current, setCurrent] = useState<string>("new");
  const [compare, setCompare] = useState<string>("");
  const [teeth, setTeeth] = useState<PerioTooth[]>([]);
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);
  const [serverError, setServerError] = useState("");
  const [dirty, setDirty] = useState(false);
  const grid = useRef<HTMLDivElement>(null);
  const order = useMemo(() => chartTeeth(chart.dentition === "primary" ? "permanent" : chart.dentition, chart.primary_slots), [chart.dentition, chart.primary_slots]);
  const allTeeth = [...order.upper, ...order.lower];

  const fresh = () => {
    const missingIds = new Set((conditions ?? []).filter((c) => c.symbol === "missing").map((c) => c.id));
    const implantIds = new Set((conditions ?? []).filter((c) => c.symbol === "implant").map((c) => c.id));
    const missing = new Set(chart.findings.filter((f) => missingIds.has(f.condition_id)).map((f) => f.tooth));
    const implants = new Set(chart.findings.filter((f) => implantIds.has(f.condition_id)).map((f) => f.tooth));
    return allTeeth.map((x) => blank(x, missing.has(x) && !implants.has(x), implants.has(x)));
  };

  const load = () =>
    rpc("perio.list", { patient_id: patient.id })
      .then((list) => {
        setExams(list);
        setError("");
        return list;
      })
      .catch((e) => {
        if (!isSessionError(e)) setError(err(e));
        return null;
      });
  useEffect(() => {
    load().then((list) => {
      if (list?.length) open(list[0], list);
      else setTeeth(fresh());
      if (list && list.length > 1) setCompare(list[1].id);
    });
  }, [patient.id, conditions]);

  const open = (e: PerioExamInfo | "new", list = exams ?? []) => {
    setDirty(false);
    if (e === "new") {
      setCurrent("new");
      setTeeth(fresh());
      setNotes("");
      if (list[0]) setCompare(list[0].id);
      return;
    }
    setCurrent(e.id);
    const byTooth = new Map(e.teeth.map((x) => [x.tooth, x]));
    setTeeth(allTeeth.map((x) => byTooth.get(x) ?? blank(x, false, false)));
    setNotes(e.notes ?? "");
  };

  // Rule 14: a half-typed exam survives a lock, another tab or a closed app.
  const draftValues = useMemo(() => (dirty ? { exam: current, teeth: JSON.stringify(teeth), notes } : { exam: "", teeth: "", notes: "" }), [dirty, current, teeth, notes]);
  const draft = useFormDraft(`perio.${patient.id}`, draftValues, canEdit);
  const restore = () => {
    try {
      setTeeth(JSON.parse(draft.offer!.teeth));
      setNotes(draft.offer!.notes);
      setCurrent(draft.offer!.exam);
      setDirty(true);
    } catch {
      /* a damaged draft is ignored */
    }
    draft.settle();
  };

  const set = (tooth: number, row: Row, site: number, value: number | boolean | null) => {
    setDirty(true);
    setTeeth((list) => list.map((x) => (x.tooth === tooth ? { ...x, [row]: x[row].map((v, i) => (i === site ? value : v)) } : x)));
  };
  const setOne = (tooth: number, patch: Partial<PerioTooth>) => {
    setDirty(true);
    setTeeth((list) => list.map((x) => (x.tooth === tooth ? { ...x, ...patch } : x)));
  };

  /** Florida-probe style entry: a digit moves the cursor to the next site. */
  const advance = (el: HTMLInputElement) => {
    const inputs = Array.from(grid.current?.querySelectorAll<HTMLInputElement>(`input[data-row="${el.dataset.row}"]`) ?? []);
    const i = inputs.indexOf(el);
    inputs[i + 1]?.focus();
    inputs[i + 1]?.select();
  };

  const save = async () => {
    setServerError("");
    setBusy(true);
    try {
      const exam = current === "new" ? null : exams?.find((e) => e.id === current);
      const saved = await rpc("perio.save", { id: exam?.id ?? null, version: exam?.version ?? 0, patient_id: patient.id, exam_date: exam?.exam_date ?? null, doctor_id: exam?.doctor_id ?? null, teeth, notes: notes.trim() || null });
      draft.clear();
      setDirty(false);
      toast.success(t("perio.saved"));
      const list = await load();
      if (list) open(list.find((e) => e.id === saved.id) ?? saved, list);
    } catch (x) {
      if (!isSessionError(x)) setServerError(x instanceof ApiError && x.rule ? `${t(`rule.${x.rule}`)}${x.field ? ` (${x.field})` : ""}` : err(x));
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    const exam = exams?.find((e) => e.id === current);
    if (!exam || !window.confirm(t("perio.deleteConfirm"))) return;
    try {
      await rpc("perio.delete", { id: exam.id, version: exam.version });
      toast.success(t("perio.deleted"));
      const list = await load();
      open(list?.[0] ?? "new", list ?? []);
    } catch (x) {
      if (!isSessionError(x)) toast.error(err(x));
    }
  };

  if (error && !exams) return <ErrorState message={error} onRetry={load} />;
  if (!exams || !conditions) return <Loading />;
  const other = exams.find((e) => e.id === compare && e.id !== current) ?? null;
  const otherByTooth = new Map((other?.teeth ?? []).map((x) => [x.tooth, x]));
  const sum = summarize(teeth);

  return (
    <div className="perio" data-testid="perio-chart">
      <div className="perio-bar">
        <Field label={t("perio.exam")}>
          <Select value={current} onChange={(e) => open(e.target.value === "new" ? "new" : exams.find((x) => x.id === e.target.value)!)} data-testid="perio-exam">
            {canEdit && <option value="new">{t("perio.newExam")}</option>}
            {exams.map((e) => <option key={e.id} value={e.id}>{formatDate(`${e.exam_date}T06:30:00Z`, lang)}{e.doctor_name ? ` · ${e.doctor_name}` : ""}</option>)}
          </Select>
        </Field>
        {exams.length > 0 && (
          <Field label={t("perio.compareWith")} optional>
            <Select value={compare} onChange={(e) => setCompare(e.target.value)} data-testid="perio-compare">
              <option value="">{t("perio.noCompare")}</option>
              {exams.filter((e) => e.id !== current).map((e) => <option key={e.id} value={e.id}>{formatDate(`${e.exam_date}T06:30:00Z`, lang)}</option>)}
            </Select>
          </Field>
        )}
        <span className="grow" />
        <Summary now={sum} before={other?.summary ?? null} />
      </div>
      {draft.offer && (
        <Notice tone="info" title={t("draft.found.title")} testId="draft-offer">
          <span className="row">
            <span>{t("draft.found.hint")}</span>
            <Button size="sm" variant="primary" onClick={restore} data-testid="draft-restore">{t("draft.found.restore")}</Button>
            <Button size="sm" onClick={() => { draft.clear(); draft.settle(); }} data-testid="draft-discard">{t("draft.found.discard")}</Button>
          </span>
        </Notice>
      )}
      {!exams.length && current === "new" && !canEdit ? (
        <EmptyState icon={ScanLine} title={t("perio.none")} />
      ) : (
        <div className="perio-grid-wrap" ref={grid} dir="ltr">
          {(["upper", "lower"] as const).map((arch) => {
            const list = arch === "upper" ? order.upper : order.lower;
            const row = list.map((x) => teeth.find((y) => y.tooth === x)!).filter(Boolean);
            const inner = arch === "upper" ? t("perio.palatal") : t("perio.lingual");
            return (
              <section key={arch} className="perio-arch" aria-label={t(`chart.quick.${arch}`)}>
                <PerioRows teeth={row} lingual={false} label={t("perio.buccal")} other={otherByTooth} canEdit={canEdit} set={set} setOne={setOne} advance={advance} />
                <PerioGraph teeth={row} lingual={false} other={otherByTooth} up={arch === "upper"} />
                <div className="perio-row perio-numbers">
                  <span className="perio-head">{t("chart.tooth")}</span>
                  {row.map((x) => (
                    <button key={x.tooth} type="button" className={`perio-num ${x.missing ? "missing" : ""}`} disabled={!canEdit} onClick={() => setOne(x.tooth, x.missing ? { missing: false } : blank(x.tooth, true, false))} title={x.missing ? t("perio.markPresent") : t("perio.markMissing")} data-testid={`perio-tooth-${x.tooth}`}>
                      <bdi>{toothLabel(x.tooth, numbering, lang)}</bdi>
                      {x.implant && <span className="perio-implant">I</span>}
                    </button>
                  ))}
                </div>
                <PerioGraph teeth={row} lingual other={otherByTooth} up={arch !== "upper"} />
                <PerioRows teeth={row} lingual label={inner} other={otherByTooth} canEdit={canEdit} set={set} setOne={setOne} advance={advance} />
              </section>
            );
          })}
        </div>
      )}
      {canEdit && (
        <div className="stack perio-foot">
          <Field label={t("perio.notes")} optional>
            <Textarea rows={2} value={notes} maxLength={2000} onChange={(e) => { setNotes(e.target.value); setDirty(true); }} data-testid="perio-notes" />
          </Field>
          {serverError && <Notice tone="danger" testId="perio-error">{serverError}</Notice>}
          <div className="row">
            <span className="subtle t-caption">{t("perio.entryHint")}</span>
            <span className="grow" />
            {current !== "new" && <Button variant="subtle" icon={Trash2} onClick={remove} data-testid="perio-delete">{t("common.delete")}</Button>}
            {current !== "new" && <Button icon={FilePlus2} onClick={() => open("new")} data-testid="perio-new">{t("perio.newExam")}</Button>}
            <Button variant="primary" icon={Save} loading={busy} onClick={save} data-testid="perio-save">{t("perio.save")}</Button>
          </div>
        </div>
      )}
    </div>
  );
}

function Summary({ now, before }: { now: PerioSummary; before: PerioSummary | null }) {
  const { t, lang } = useI18n();
  const mm = (x10: number) => digits((x10 / 10).toFixed(1), lang);
  const delta = (a: number, b: number, lowerIsBetter = true) => {
    if (a === b) return <Minus size={12} aria-hidden />;
    const better = lowerIsBetter ? a < b : a > b;
    const Icon = a > b ? ArrowUpRight : ArrowDownRight;
    return <Icon size={13} className={better ? "better" : "worse"} aria-label={better ? t("perio.better") : t("perio.worse")} />;
  };
  const chip = (label: string, value: string, d?: ReactNode, testId?: string) => (
    <span className="perio-stat" data-testid={testId}><span className="subtle t-caption">{label}</span><b className="num">{value}</b>{d}</span>
  );
  return (
    <div className="perio-summary" data-testid="perio-summary">
      {chip(t("perio.meanDepth"), `${mm(now.mean_depth_x10)} ${t("perio.mm")}`, before ? delta(now.mean_depth_x10, before.mean_depth_x10) : null, "perio-mean")}
      {chip(t("perio.sites4"), digits(now.sites_4_plus, lang), before ? delta(now.sites_4_plus, before.sites_4_plus) : null)}
      {chip(t("perio.sites6"), digits(now.sites_6_plus, lang), before ? delta(now.sites_6_plus, before.sites_6_plus) : null)}
      {chip(t("perio.bleeding"), `${digits(now.bleeding_percent, lang)}٪`.replace("٪", lang === "en" ? "%" : "٪"), before ? delta(now.bleeding_percent, before.bleeding_percent) : null, "perio-bop")}
      {chip(t("perio.plaque"), `${digits(now.plaque_percent, lang)}${lang === "en" ? "%" : "٪"}`, before ? delta(now.plaque_percent, before.plaque_percent) : null)}
    </div>
  );
}

function PerioRows({ teeth, lingual, label, other, canEdit, set, setOne, advance }: {
  teeth: PerioTooth[];
  lingual: boolean;
  label: string;
  other: Map<number, PerioTooth>;
  canEdit: boolean;
  set: (tooth: number, row: Row, site: number, value: number | boolean | null) => void;
  setOne: (tooth: number, patch: Partial<PerioTooth>) => void;
  advance: (el: HTMLInputElement) => void;
}) {
  const { t, lang } = useI18n();
  const side = lingual ? "l" : "b";
  const num = (x: PerioTooth, row: "depth" | "recession", site: number) => {
    const v = x[row][site];
    const before = other.get(x.tooth)?.[row][site];
    const sev = row === "depth" && v != null ? (v >= 6 ? "sev6" : v >= 4 ? "sev4" : "") : "";
    return (
      <input
        key={site}
        className={`perio-cell ${sev}`}
        inputMode="numeric"
        maxLength={row === "recession" ? 3 : 2}
        disabled={!canEdit || x.missing}
        value={v == null ? "" : digits(v, lang)}
        title={before != null ? `${t("perio.before")}: ${before}` : undefined}
        data-row={`${row}-${side}`}
        data-testid={`perio-${row}-${x.tooth}-${site}`}
        onFocus={(e) => e.target.select()}
        onChange={(e) => {
          const raw = latinDigits(e.target.value).replace(/[^\d-]/g, "");
          if (raw === "" || raw === "-") return set(x.tooth, row, site, null);
          const n = Number(raw);
          const ok = row === "depth" ? n >= 0 && n <= 15 : n >= -5 && n <= 15;
          if (!ok) return;
          set(x.tooth, row, site, n);
          // A one-digit depth is complete: move on (Florida-probe style).
          if (row === "depth" && raw.length === 1 && n > 1) advance(e.target);
        }}
      />
    );
  };
  const toggle = (x: PerioTooth, row: "bleeding" | "plaque", site: number) => (
    <button
      key={site}
      type="button"
      className={`perio-dot ${row} ${x[row][site] ? "on" : ""}`}
      aria-pressed={x[row][site]}
      aria-label={t(`perio.${row}`)}
      disabled={!canEdit || x.missing}
      onClick={() => set(x.tooth, row, site, !x[row][site])}
      data-testid={`perio-${row}-${x.tooth}-${site}`}
    />
  );
  const rows: { key: string; head: string; cell: (x: PerioTooth) => ReactNode }[] = [
    { key: "depth", head: t("perio.depth"), cell: (x) => siteOrder(x.tooth, lingual).map((s) => num(x, "depth", s)) },
    { key: "recession", head: t("perio.recession"), cell: (x) => siteOrder(x.tooth, lingual).map((s) => num(x, "recession", s)) },
    { key: "bleeding", head: t("perio.bleeding"), cell: (x) => siteOrder(x.tooth, lingual).map((s) => toggle(x, "bleeding", s)) },
    { key: "plaque", head: t("perio.plaque"), cell: (x) => siteOrder(x.tooth, lingual).map((s) => toggle(x, "plaque", s)) },
    {
      key: "furcation",
      head: t("perio.furcation"),
      cell: (x) =>
        isMolar(toothInfo(x.tooth).type) ? (
          <Stepper value={x.furcation[lingual ? 1 : 0]} disabled={!canEdit || x.missing} label={t("perio.furcation")} onChange={(g) => setOne(x.tooth, { furcation: x.furcation.map((v, i) => (i === (lingual ? 1 : 0) ? g : v)) })} testId={`perio-furcation-${x.tooth}-${lingual ? 1 : 0}`} />
        ) : null,
    },
  ];
  if (!lingual)
    rows.unshift({
      key: "mobility",
      head: t("perio.mobility"),
      cell: (x) => <Stepper value={x.mobility} disabled={!canEdit || x.missing} label={t("perio.mobility")} onChange={(g) => setOne(x.tooth, { mobility: g })} testId={`perio-mobility-${x.tooth}`} />,
    });
  const ordered = lingual ? [...rows].reverse() : rows;
  return (
    <div className={`perio-rows ${lingual ? "lingual" : "buccal"}`}>
      <div className="perio-side-label">{label}</div>
      {ordered.map((r) => (
        <div key={r.key} className={`perio-row r-${r.key}`}>
          <span className="perio-head">{r.head}</span>
          {teeth.map((x) => <span key={x.tooth} className={`perio-tooth ${x.missing ? "missing" : ""}`}>{r.cell(x)}</span>)}
        </div>
      ))}
    </div>
  );
}

/** A grade 0–3 (mobility, furcation) on one small button: each click steps to the next grade, then empty. */
function Stepper({ value, disabled, label, onChange, testId }: { value: number | null; disabled: boolean; label: string; onChange: (v: number | null) => void; testId: string }) {
  const { lang } = useI18n();
  const next = value == null ? 0 : value >= 3 ? null : value + 1;
  return (
    <button type="button" className={`perio-step g${value ?? "n"}`} disabled={disabled} aria-label={label} title={label} onClick={() => onChange(next)} data-testid={testId}>
      {value == null ? "–" : digits(value, lang)}
    </button>
  );
}

/** Pocket depth below the gum margin as bars, the margin as a line; the compared exam dashed. */
function PerioGraph({ teeth, lingual, other, up }: { teeth: PerioTooth[]; lingual: boolean; other: Map<number, PerioTooth>; up: boolean }) {
  const cell = 58;
  const h = 64;
  const base = up ? h - 18 : 18;
  const k = 3.6;
  const w = teeth.length * cell;
  const dir = up ? -1 : 1;
  const pts: string[] = [];
  const otherPts: string[] = [];
  const bars: ReactNode[] = [];
  teeth.forEach((x, ti) => {
    siteOrder(x.tooth, lingual).forEach((s, si) => {
      const cx = ti * cell + 9.5 + si * 19.5;
      if (x.missing) return;
      const rec = x.recession[s] ?? 0;
      const d = x.depth[s];
      const margin = base + dir * rec * k;
      if (d != null) {
        const bottom = margin + dir * d * k;
        bars.push(<line key={`${x.tooth}-${s}`} x1={cx} x2={cx} y1={margin} y2={bottom} className={`perio-bar-line ${d >= 6 ? "sev6" : d >= 4 ? "sev4" : ""}`} />);
        pts.push(`${cx},${bottom}`);
      }
      const o = other.get(x.tooth);
      if (o && !o.missing && o.depth[s] != null) otherPts.push(`${cx},${base + dir * ((o.recession[s] ?? 0) + o.depth[s]!) * k}`);
    });
  });
  return (
    <svg className="perio-graph" viewBox={`0 0 ${w} ${h}`} width={w} height={h} aria-hidden>
      {[3, 6].map((mm) => <line key={mm} x1={0} x2={w} y1={base + dir * mm * k} y2={base + dir * mm * k} className={`perio-grid-line mm${mm}`} />)}
      <line x1={0} x2={w} y1={base} y2={base} className="perio-cej" />
      {bars}
      {otherPts.length > 1 && <polyline points={otherPts.join(" ")} className="perio-line before" />}
      {pts.length > 1 && <polyline points={pts.join(" ")} className="perio-line" />}
      {teeth.map((x, i) => x.missing && <rect key={x.tooth} x={i * cell + 2} y={2} width={cell - 4} height={h - 4} rx={6} className="perio-missing" />)}
    </svg>
  );
}

