import { useState } from "react";
import { ClipboardList, Grid3x3, NotebookPen, Pencil, Plus, RotateCcw, Trash2 } from "lucide-react";
import type { ChartSymbol, Language, NoteFields, NoteTemplateInfo, NoteTemplateTexts, PlanTemplateInfo, PlanTemplateItem, TemplateTeeth, ToothConditionInfo, Translations } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { digits } from "../lib/dates";
import { invalidateNoteTemplates, invalidatePlanTemplates, invalidateToothConditions, serviceName, useCatalog } from "../lib/clinical";
import { tr } from "../lib/medical";
import { useI18n } from "../i18n";
import { Button, IconButton } from "../ui/Button";
import { Card, CardHeader } from "../ui/Card";
import { Segmented, Switch } from "../ui/Controls";
import { Field, Select, Textarea, TextInput } from "../ui/Field";
import { Badge, ErrorState, Notice, SkeletonRows } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { TranslationsField } from "../ui/TranslationsField";
import { useToast } from "../ui/Toast";
import { CHART_SYMBOLS } from "../chart/model";
import { NOTE_FIELDS } from "./patients/clinical/NotesTab";
import { emptyTranslations, hasText, useFailure, useList } from "./clinicalListsShared";

// ───────────────────────────── tooth conditions (5.3) ─────────────────────────────

/** The conditions the chart records (caries, filling, crown, missing …): the clinic adds its own. */
export function ConditionsTab() {
  const { t, lang } = useI18n();
  const toast = useToast();
  const fail = useFailure();
  const list = useList(() => rpc("tooth_conditions.list", { include_inactive: true }));
  const [editing, setEditing] = useState<{ c?: ToothConditionInfo } | null>(null);
  const toggle = async (c: ToothConditionInfo, on: boolean) => {
    try {
      await rpc("tooth_conditions.save", { id: c.id, version: c.version, name: c.name, symbol: c.symbol, needs_surface: c.needs_surface, abbreviation: c.abbreviation, is_active: on });
      invalidateToothConditions();
      list.reload();
    } catch (x) {
      if (!isSessionError(x)) toast.error(fail(x));
    }
  };
  if (list.error && !list.items) return <ErrorState message={list.error} onRetry={list.reload} />;
  return (
    <Card flush>
      <CardHeader icon={Grid3x3} title={t("clinical.conditions.title")} description={t("clinical.conditions.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setEditing({})} data-testid="condition-new">{t("clinical.conditions.add")}</Button>} />
      <div className="table-wrap">
        <table className="table" data-testid="condition-list">
          <thead><tr><th>{t("clinical.conditions.name")}</th><th>{t("catalog.chartSymbol")}</th><th>{t("clinical.conditions.surface")}</th><th>{t("clinical.active")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr></thead>
          <tbody>
            {!list.items && <SkeletonRows cols={5} />}
            {list.items?.map((c) => (
              <tr key={c.id} className={c.is_active ? "" : "row-inactive"} data-testid={`condition-row-${c.code}`}>
                <td><span className="cell-strong">{tr(c.name, lang)}</span>{c.abbreviation && <> <Badge>{c.abbreviation}</Badge></>}{!c.is_system && <> <Badge tone="accent">{t("clinical.own")}</Badge></>}</td>
                <td>{t(`chart.symbol.${c.symbol}`)}</td>
                <td>{c.needs_surface ? t("common.yes") : "—"}</td>
                <td><Switch checked={c.is_active} onChange={(on) => toggle(c, on)} label={<span className="visually-hidden">{t("clinical.active")}</span>} testId={`condition-active-${c.code}`} /></td>
                <td className="cell-actions"><IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ c })} data-testid={`condition-edit-${c.code}`} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {editing && <ConditionDialog c={editing.c} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); invalidateToothConditions(); toast.success(t("common.saved")); list.reload(); }} />}
    </Card>
  );
}

function ConditionDialog({ c, onClose, onSaved }: { c?: ToothConditionInfo; onClose: () => void; onSaved: () => void }) {
  const { t } = useI18n();
  const fail = useFailure();
  const [name, setName] = useState<Translations>(c?.name ?? emptyTranslations);
  const [symbol, setSymbol] = useState<ChartSymbol>(c?.symbol ?? "marker");
  const [surface, setSurface] = useState(c?.needs_surface ?? false);
  const [abbr, setAbbr] = useState(c?.abbreviation ?? "");
  const [active, setActive] = useState(c?.is_active ?? true);
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const abbrError = abbr.trim().length > 3 ? t("rule.condition_abbreviation") : symbol === "marker" && !abbr.trim() ? t("clinical.conditions.abbrNeeded") : null;
  const save = async () => {
    setSubmitted(true);
    if (!hasText(name) || abbrError) return;
    setBusy(true);
    try {
      await rpc("tooth_conditions.save", { id: c?.id ?? null, version: c?.version ?? 0, name, symbol, needs_surface: surface, abbreviation: abbr.trim() || null, is_active: active });
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) setError(fail(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={c ? t("clinical.conditions.edit") : t("clinical.conditions.add")} onClose={onClose} wide testId="condition-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="condition-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("clinical.conditions.name")} hint={t("clinical.oneLanguage")} value={name} onChange={setName} error={submitted && !hasText(name) ? t("rule.required") : null} testId="condition-name" />
        <div className="grid-2">
          <Field label={t("catalog.chartSymbol")} hint={t("clinical.conditions.symbolHint")}>
            <Select value={symbol} onChange={(e) => setSymbol(e.target.value as ChartSymbol)} data-testid="condition-symbol">
              {CHART_SYMBOLS.filter((x) => x !== "none").map((x) => <option key={x} value={x}>{t(`chart.symbol.${x}`)}</option>)}
            </Select>
          </Field>
          <Field label={t("clinical.conditions.abbreviation")} hint={t("clinical.conditions.abbreviationHint")} optional={symbol !== "marker"} error={submitted || abbr.trim() ? abbrError : null}>
            <TextInput dir="ltr" value={abbr} maxLength={3} onChange={(e) => setAbbr(e.target.value)} data-testid="condition-abbr" />
          </Field>
        </div>
        <Switch checked={surface} onChange={setSurface} label={t("clinical.conditions.surfaceLabel")} testId="condition-surface" />
        {c && <Switch checked={active} onChange={setActive} label={t("clinical.active")} testId="condition-active" />}
      </div>
    </Dialog>
  );
}

// ───────────────────────────── ready-made notes (5.6) ─────────────────────────────

const EMPTY_FIELDS: NoteFields = { chief_complaint: "", examination: "", diagnosis: "", findings: "", procedure: "", notes: "", follow_up: "", recommendations: "" };

export function NoteTemplatesTab() {
  const { t, lang } = useI18n();
  const toast = useToast();
  const fail = useFailure();
  const list = useList(() => rpc("note_templates.list", { include_inactive: true }));
  const [editing, setEditing] = useState<{ x?: NoteTemplateInfo } | null>(null);
  const toggle = async (x: NoteTemplateInfo, on: boolean) => {
    try {
      await rpc("note_templates.save", { id: x.id, version: x.version, name: x.name, body: x.body, is_active: on });
      invalidateNoteTemplates();
      list.reload();
    } catch (e) {
      if (!isSessionError(e)) toast.error(fail(e));
    }
  };
  if (list.error && !list.items) return <ErrorState message={list.error} onRetry={list.reload} />;
  return (
    <Card flush>
      <CardHeader icon={NotebookPen} title={t("clinical.notes.title")} description={t("clinical.notes.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setEditing({})} data-testid="note-template-new">{t("clinical.notes.add")}</Button>} />
      <div className="table-wrap">
        <table className="table" data-testid="note-template-list">
          <thead><tr><th>{t("clinical.notes.name")}</th><th>{t("clinical.notes.parts")}</th><th>{t("clinical.active")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr></thead>
          <tbody>
            {!list.items && <SkeletonRows cols={4} />}
            {list.items?.map((x) => (
              <tr key={x.id} className={x.is_active ? "" : "row-inactive"} data-testid={`note-template-row-${x.code}`}>
                <td><span className="cell-strong">{tr(x.name, lang)}</span>{x.customized && <> <Badge tone="accent">{t("clinical.templates.customized")}</Badge></>}{!x.is_system && <> <Badge tone="accent">{t("clinical.own")}</Badge></>}</td>
                <td className="t-caption subtle">{NOTE_FIELDS.filter((k) => x.body[lang][k].trim()).map((k) => t(`note.field.${k}`)).join("، ")}</td>
                <td><Switch checked={x.is_active} onChange={(on) => toggle(x, on)} label={<span className="visually-hidden">{t("clinical.active")}</span>} testId={`note-template-active-${x.code}`} /></td>
                <td className="cell-actions"><IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ x })} data-testid={`note-template-edit-${x.code}`} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {editing && <NoteTemplateDialog x={editing.x} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); invalidateNoteTemplates(); toast.success(t("common.saved")); list.reload(); }} />}
    </Card>
  );
}

function NoteTemplateDialog({ x, onClose, onSaved }: { x?: NoteTemplateInfo; onClose: () => void; onSaved: () => void }) {
  const { t, lang: uiLang } = useI18n();
  const fail = useFailure();
  const [name, setName] = useState<Translations>(x?.name ?? emptyTranslations);
  const [body, setBody] = useState<NoteTemplateTexts>(x?.body ?? { fa: { ...EMPTY_FIELDS }, ps: { ...EMPTY_FIELDS }, en: { ...EMPTY_FIELDS } });
  const [lang, setLang] = useState<Language>(uiLang);
  const [active, setActive] = useState(x?.is_active ?? true);
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState<"" | "save" | "reset">("");
  const [error, setError] = useState("");
  const empty = (["fa", "ps", "en"] as Language[]).every((l) => NOTE_FIELDS.every((k) => !body[l][k].trim()));
  const save = async () => {
    setSubmitted(true);
    if (!hasText(name) || empty) return;
    setBusy("save");
    try {
      await rpc("note_templates.save", { id: x?.id ?? null, version: x?.version ?? 0, name, body, is_active: active });
      onSaved();
    } catch (e) {
      if (!isSessionError(e)) setError(fail(e));
    } finally {
      setBusy("");
    }
  };
  const reset = async () => {
    if (!x || !window.confirm(t("clinical.templates.resetConfirm"))) return;
    setBusy("reset");
    try {
      await rpc("note_templates.reset", { id: x.id, version: x.version });
      onSaved();
    } catch (e) {
      if (!isSessionError(e)) setError(fail(e));
    } finally {
      setBusy("");
    }
  };
  return (
    <Dialog title={x ? t("clinical.notes.edit") : t("clinical.notes.add")} onClose={onClose} wide testId="note-template-dialog"
      footer={
        <>
          {x?.is_system && x.customized && <Button variant="subtle" icon={RotateCcw} loading={busy === "reset"} onClick={reset} data-testid="note-template-reset">{t("clinical.templates.reset")}</Button>}
          <span className="grow" />
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" loading={busy === "save"} onClick={save} data-testid="note-template-save">{t("common.save")}</Button>
        </>
      }>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("clinical.notes.name")} hint={t("clinical.oneLanguage")} value={name} onChange={setName} error={submitted && !hasText(name) ? t("rule.required") : null} testId="note-template-name" />
        <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap" }}>
          <span className="field-label">{t("clinical.notes.textIn")}</span>
          <Segmented<Language> value={lang} onChange={setLang} label={t("clinical.notes.textIn")} options={(["fa", "ps", "en"] as Language[]).map((l) => ({ value: l, label: t(`lang.${l}`), testId: `note-template-lang-${l}` }))} />
        </div>
        {submitted && empty && <Notice tone="danger">{t("rule.note_empty")}</Notice>}
        <div className="note-grid" dir={lang === "en" ? "ltr" : "rtl"}>
          {NOTE_FIELDS.map((k) => (
            <Field key={k} label={t(`note.field.${k}`)} optional>
              <Textarea rows={2} value={body[lang][k]} maxLength={4000} onChange={(e) => setBody((b) => ({ ...b, [lang]: { ...b[lang], [k]: e.target.value } }))} data-testid={`note-template-${lang}-${k}`} />
            </Field>
          ))}
        </div>
        <Notice tone="info">{t("clinical.notes.languagesHint")}</Notice>
        {x && <Switch checked={active} onChange={setActive} label={t("clinical.active")} testId="note-template-active" />}
      </div>
    </Dialog>
  );
}

// ───────────────────────────── ready-made plans (5.7) ─────────────────────────────

const TEETH: TemplateTeeth[] = ["selected", "upper", "lower", "all", "none"];

export function PlanTemplatesTab() {
  const { t, lang } = useI18n();
  const toast = useToast();
  const fail = useFailure();
  const catalog = useCatalog();
  const list = useList(() => rpc("plan_templates.list", { include_inactive: true }));
  const [editing, setEditing] = useState<{ x?: PlanTemplateInfo } | null>(null);
  const toggle = async (x: PlanTemplateInfo, on: boolean) => {
    try {
      await rpc("plan_templates.save", { id: x.id, version: x.version, name: x.name, items: x.items, is_active: on });
      invalidatePlanTemplates();
      list.reload();
    } catch (e) {
      if (!isSessionError(e)) toast.error(fail(e));
    }
  };
  if (list.error && !list.items) return <ErrorState message={list.error} onRetry={list.reload} />;
  return (
    <Card flush>
      <CardHeader icon={ClipboardList} title={t("clinical.plans.title")} description={t("clinical.plans.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setEditing({})} data-testid="plan-template-new">{t("clinical.plans.add")}</Button>} />
      <div className="table-wrap">
        <table className="table" data-testid="plan-template-list">
          <thead><tr><th>{t("clinical.plans.name")}</th><th>{t("clinical.plans.lines")}</th><th>{t("clinical.active")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr></thead>
          <tbody>
            {!list.items && <SkeletonRows cols={4} />}
            {list.items?.map((x) => (
              <tr key={x.id} className={x.is_active ? "" : "row-inactive"} data-testid={`plan-template-row-${x.code}`}>
                <td><span className="cell-strong">{tr(x.name, lang)}</span>{!x.is_system && <> <Badge tone="accent">{t("clinical.own")}</Badge></>}</td>
                <td className="t-caption subtle">{x.items.map((i) => { const s = catalog?.services.find((z) => z.id === i.service_id); return s ? serviceName(s, catalog, lang) : "—"; }).join(" ← ")}</td>
                <td><Switch checked={x.is_active} onChange={(on) => toggle(x, on)} label={<span className="visually-hidden">{t("clinical.active")}</span>} testId={`plan-template-active-${x.code}`} /></td>
                <td className="cell-actions"><IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ x })} data-testid={`plan-template-edit-${x.code}`} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {editing && <PlanTemplateDialog x={editing.x} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); invalidatePlanTemplates(); toast.success(t("common.saved")); list.reload(); }} />}
    </Card>
  );
}

function PlanTemplateDialog({ x, onClose, onSaved }: { x?: PlanTemplateInfo; onClose: () => void; onSaved: () => void }) {
  const { t, lang } = useI18n();
  const fail = useFailure();
  const catalog = useCatalog();
  const [name, setName] = useState<Translations>(x?.name ?? emptyTranslations);
  const [items, setItems] = useState<PlanTemplateItem[]>(x?.items ?? []);
  const [active, setActive] = useState(x?.is_active ?? true);
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const leafs = catalog ? catalog.services.filter((s) => !catalog.services.some((v) => v.parent_id === s.id)) : [];
  const setLine = (n: number, patch: Partial<PlanTemplateItem>) => setItems((l) => l.map((i, k) => (k === n ? { ...i, ...patch } : i)));
  const save = async () => {
    setSubmitted(true);
    if (!hasText(name) || !items.length) return;
    setBusy(true);
    try {
      await rpc("plan_templates.save", { id: x?.id ?? null, version: x?.version ?? 0, name, items, is_active: active });
      onSaved();
    } catch (e) {
      if (!isSessionError(e)) setError(fail(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={x ? t("clinical.plans.edit") : t("clinical.plans.add")} onClose={onClose} wide testId="plan-template-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="plan-template-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("clinical.plans.name")} hint={t("clinical.oneLanguage")} value={name} onChange={setName} error={submitted && !hasText(name) ? t("rule.required") : null} testId="plan-template-name" />
        <Field label={t("clinical.plans.addLine")} hint={t("clinical.plans.addLineHint")} error={submitted && !items.length ? t("rule.plan_template_items") : null}>
          <Select value="" onChange={(e) => e.target.value && setItems((l) => [...l, { service_id: e.target.value, teeth: "selected", phase: l.length ? l[l.length - 1].phase : 1 }])} data-testid="plan-template-add">
            <option value="">—</option>
            {catalog?.categories.map((c) => (
              <optgroup key={c.id} label={tr(c.name, lang)}>
                {leafs.filter((s) => s.category_id === c.id).map((s) => <option key={s.id} value={s.id}>{serviceName(s, catalog, lang)}</option>)}
              </optgroup>
            ))}
          </Select>
        </Field>
        <ol className="plan-template-lines">
          {items.map((i, n) => {
            const s = catalog?.services.find((z) => z.id === i.service_id);
            return (
              <li key={n} className="row" data-testid={`plan-template-line-${n}`}>
                <span className="cell-strong grow">{s ? serviceName(s, catalog, lang) : "—"}</span>
                <Select value={String(i.phase)} onChange={(e) => setLine(n, { phase: Number(e.target.value) })} aria-label={t("plan.phase")}>
                  {[1, 2, 3, 4, 5].map((p) => <option key={p} value={p}>{t("plan.phaseN").replace("{n}", digits(p, lang))}</option>)}
                </Select>
                <Select value={i.teeth} onChange={(e) => setLine(n, { teeth: e.target.value as TemplateTeeth })} aria-label={t("plan.teeth")} data-testid={`plan-template-teeth-${n}`}>
                  {TEETH.map((x) => <option key={x} value={x}>{t(`plan.templateTeeth.${x}`)}</option>)}
                </Select>
                <IconButton icon={Trash2} size="sm" label={t("common.delete")} onClick={() => setItems((l) => l.filter((_, k) => k !== n))} />
              </li>
            );
          })}
        </ol>
        {x && <Switch checked={active} onChange={setActive} label={t("clinical.active")} testId="plan-template-active" />}
      </div>
    </Dialog>
  );
}
