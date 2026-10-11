import { useEffect, useState } from "react";
import { BookOpenText, FileHeart, FileSignature, GraduationCap, Pencil, Pill, Plus, RotateCcw, Trash2, Wand2 } from "lucide-react";
import type {
  DocumentKind, DocumentTemplateInfo, DrugInfo, MedicalDetailKind, MedicalQuestionInfo, Paper, RxItem, RxTemplateInfo, RxTiming, SpecialtyInfo, Translations,
} from "../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../lib/api";
import { invalidateSpecialties } from "../lib/clinical";
import { digits, latinDigits } from "../lib/dates";
import { invalidateMedicalQuestions, tr } from "../lib/medical";
import { useI18n } from "../i18n";
import { Button, IconButton } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { Checkbox, ChipGroup, Segmented, Switch } from "../ui/Controls";
import { Field, Select, TextInput } from "../ui/Field";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { TranslationsField } from "../ui/TranslationsField";
import { useToast } from "../ui/Toast";

type Tab = "questions" | "specialties" | "drugs" | "rx" | "templates";
const empty: Translations = { fa: "", ps: "", en: "" };
const hasText = (x: Translations) => !!(x.fa.trim() || x.ps.trim() || x.en.trim());
const GROUPS = ["cardio", "blood", "endocrine", "respiratory", "infectious", "kidney_liver", "neuro", "bone", "cancer", "digestive", "skin", "habits", "women", "allergy", "medication", "surgery", "other"];
const FORMS = ["tablet", "capsule", "syrup", "suspension", "mouthwash", "gel", "cream", "ointment", "injection", "drops", "spray", "other"];
const CLASSES = ["penicillin", "cephalosporin", "nsaid", "tetracycline", "metronidazole", "macrolide", "lincosamide", "opioid", "azole", "paracetamol", "steroid", "local_anesthetic", "antiseptic", "antifungal", "antiviral", "ppi"];
const TIMINGS: RxTiming[] = ["after_food", "before_food", "with_food", "morning", "bedtime"];

/** Error text of a failed save: the rule's message, else the code's. */
function useFailure() {
  const { t, err } = useI18n();
  return (x: unknown) => (x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
}

/**
 * The clinic's own clinical lists (Phase 5A): every list the Core seeds is a starting point the
 * clinic edits here — the medical-history checklist (M1), specialties (M2), the medicine formulary and
 * ready-made prescriptions (5.9), and the texts of consent forms and after-treatment sheets (5.10b).
 */
export function ClinicalListsPage() {
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>("questions");
  return (
    <Page testId="page-clinical">
      <PageHeader title={t("nav.clinical")} description={t("clinical.subtitle")} />
      <Segmented<Tab>
        value={tab}
        onChange={setTab}
        label={t("nav.clinical")}
        options={[
          { value: "questions", label: t("clinical.tab.questions"), icon: FileHeart, testId: "clinical-tab-questions" },
          { value: "specialties", label: t("clinical.tab.specialties"), icon: GraduationCap, testId: "clinical-tab-specialties" },
          { value: "drugs", label: t("clinical.tab.drugs"), icon: Pill, testId: "clinical-tab-drugs" },
          { value: "rx", label: t("clinical.tab.rx"), icon: Wand2, testId: "clinical-tab-rx" },
          { value: "templates", label: t("clinical.tab.templates"), icon: FileSignature, testId: "clinical-tab-templates" },
        ]}
      />
      {tab === "questions" && <QuestionsTab />}
      {tab === "specialties" && <SpecialtiesTab />}
      {tab === "drugs" && <DrugsTab />}
      {tab === "rx" && <RxTemplatesTab />}
      {tab === "templates" && <TemplatesTab />}
    </Page>
  );
}

function useList<T>(load: () => Promise<T[]>) {
  const { err } = useI18n();
  const [items, setItems] = useState<T[] | null>(null);
  const [error, setError] = useState("");
  const reload = () => load().then((v) => { setItems(v); setError(""); }).catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    reload();
  }, []);
  return { items, error, reload };
}

// ───────────────────────────── checklist questions (M1) ─────────────────────────────

function QuestionsTab() {
  const { t, lang } = useI18n();
  const toast = useToast();
  const fail = useFailure();
  const list = useList(() => rpc("medical_questions.list", { include_inactive: true }));
  const [editing, setEditing] = useState<{ q?: MedicalQuestionInfo } | null>(null);
  const toggle = async (q: MedicalQuestionInfo, on: boolean) => {
    try {
      await rpc("medical_questions.update", { id: q.id, version: q.version, group_code: q.group_code, label: q.label, detail_kind: q.detail_kind, alert: q.alert, female_only: q.female_only, is_active: on, sort_order: q.sort_order });
      invalidateMedicalQuestions();
      list.reload();
    } catch (x) {
      if (!isSessionError(x)) toast.error(fail(x));
    }
  };
  if (list.error && !list.items) return <ErrorState message={list.error} onRetry={list.reload} />;
  return (
    <Card flush>
      <CardHeader icon={FileHeart} title={t("clinical.questions.title")} description={t("clinical.questions.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setEditing({})} data-testid="question-new">{t("clinical.questions.add")}</Button>} />
      <div className="table-wrap">
        <table className="table" data-testid="question-list">
          <thead><tr><th>{t("clinical.questions.label")}</th><th>{t("clinical.questions.group")}</th><th>{t("clinical.questions.detail")}</th><th>{t("clinical.active")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr></thead>
          <tbody>
            {!list.items && <SkeletonRows cols={5} />}
            {list.items?.map((q) => (
              <tr key={q.id} className={q.is_active ? "" : "row-inactive"} data-testid={`question-row-${q.code}`}>
                <td>
                  <span className="cell-strong">{tr(q.label, lang)}</span>
                  <span className="row" style={{ gap: 4, marginTop: 2 }}>
                    {q.alert && <Badge tone="danger" dot>{t("clinical.questions.alert")}</Badge>}
                    {q.female_only && <Badge>{t("clinical.questions.women")}</Badge>}
                    {!q.is_system && <Badge tone="accent">{t("clinical.own")}</Badge>}
                  </span>
                </td>
                <td>{t(`mh.group.${q.group_code}`)}</td>
                <td>{t(`clinical.detail.${q.detail_kind}`)}</td>
                <td><Switch checked={q.is_active} onChange={(on) => toggle(q, on)} label={<span className="visually-hidden">{t("clinical.active")}</span>} testId={`question-active-${q.code}`} /></td>
                <td className="cell-actions">{!q.is_system && <IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ q })} data-testid={`question-edit-${q.code}`} />}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {editing && (
        <QuestionDialog
          q={editing.q}
          onClose={() => setEditing(null)}
          onSaved={() => { setEditing(null); invalidateMedicalQuestions(); toast.success(t("common.saved")); list.reload(); }}
        />
      )}
    </Card>
  );
}

function QuestionDialog({ q, onClose, onSaved }: { q?: MedicalQuestionInfo; onClose: () => void; onSaved: () => void }) {
  const { t } = useI18n();
  const fail = useFailure();
  const [label, setLabel] = useState<Translations>(q?.label ?? empty);
  const [group, setGroup] = useState(q?.group_code ?? "other");
  const [detail, setDetail] = useState<MedicalDetailKind>(q?.detail_kind ?? "none");
  const [alert, setAlert] = useState(q?.alert ?? false);
  const [female, setFemale] = useState(q?.female_only ?? false);
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const save = async () => {
    setSubmitted(true);
    if (!hasText(label)) return;
    setBusy(true);
    try {
      if (q) await rpc("medical_questions.update", { id: q.id, version: q.version, group_code: group, label, detail_kind: detail, alert, female_only: female, is_active: q.is_active, sort_order: q.sort_order });
      else await rpc("medical_questions.create", { group_code: group, label, detail_kind: detail, alert, female_only: female });
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) setError(fail(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={q ? t("clinical.questions.edit") : t("clinical.questions.add")} onClose={onClose} wide testId="question-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="question-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("clinical.questions.label")} hint={t("clinical.oneLanguage")} value={label} onChange={setLabel} error={submitted && !hasText(label) ? t("rule.medical_question_label") : null} testId="question-label" />
        <div className="grid-2">
          <Field label={t("clinical.questions.group")}>
            <Select value={group} onChange={(e) => setGroup(e.target.value)} data-testid="question-group">
              {GROUPS.map((g) => <option key={g} value={g}>{t(`mh.group.${g}`)}</option>)}
            </Select>
          </Field>
          <div className="field">
            <span className="field-label">{t("clinical.questions.detail")}</span>
            <Segmented<MedicalDetailKind> value={detail} onChange={setDetail} label={t("clinical.questions.detail")} options={[{ value: "none", label: t("clinical.detail.none"), testId: "question-detail-none" }, { value: "text", label: t("clinical.detail.text"), testId: "question-detail-text" }]} />
          </div>
        </div>
        <Switch checked={alert} onChange={setAlert} label={t("clinical.questions.alertLabel")} testId="question-alert" />
        <Switch checked={female} onChange={setFemale} label={t("clinical.questions.womenLabel")} testId="question-women" />
      </div>
    </Dialog>
  );
}

// ───────────────────────────── specialties (M2) ─────────────────────────────

function SpecialtiesTab() {
  const { t, lang } = useI18n();
  const toast = useToast();
  const list = useList(() => rpc("specialties.list", { include_inactive: true }));
  const [editing, setEditing] = useState<{ s?: SpecialtyInfo } | null>(null);
  if (list.error && !list.items) return <ErrorState message={list.error} onRetry={list.reload} />;
  return (
    <Card flush>
      <CardHeader icon={GraduationCap} title={t("clinical.specialties.title")} description={t("clinical.specialties.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setEditing({})} data-testid="specialty-new">{t("clinical.specialties.add")}</Button>} />
      <div className="table-wrap">
        <table className="table" data-testid="specialty-list">
          <thead><tr><th>{t("clinical.specialties.name")}</th><th>{t("clinical.active")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr></thead>
          <tbody>
            {!list.items && <SkeletonRows cols={3} />}
            {list.items?.map((s) => (
              <tr key={s.id} className={s.is_active ? "" : "row-inactive"} data-testid={`specialty-row-${s.code}`}>
                <td><span className="cell-strong">{tr(s.label, lang)}</span>{!s.is_system && <> <Badge tone="accent">{t("clinical.own")}</Badge></>}</td>
                <td>{s.is_active ? <Badge tone="success" dot>{t("users.active")}</Badge> : <Badge dot>{t("users.inactive")}</Badge>}</td>
                <td className="cell-actions"><IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ s })} data-testid={`specialty-edit-${s.code}`} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {editing && <SpecialtyDialog s={editing.s} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); invalidateSpecialties(); toast.success(t("common.saved")); list.reload(); }} />}
    </Card>
  );
}

function SpecialtyDialog({ s, onClose, onSaved }: { s?: SpecialtyInfo; onClose: () => void; onSaved: () => void }) {
  const { t } = useI18n();
  const fail = useFailure();
  const [label, setLabel] = useState<Translations>(s?.label ?? empty);
  const [active, setActive] = useState(s?.is_active ?? true);
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const save = async () => {
    setSubmitted(true);
    if (!hasText(label)) return;
    setBusy(true);
    try {
      await rpc("specialties.save", { id: s?.id ?? null, version: s?.version ?? 0, label, is_active: active });
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) setError(fail(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={s ? t("clinical.specialties.edit") : t("clinical.specialties.add")} onClose={onClose} wide testId="specialty-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="specialty-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        {s?.is_system ? (
          <Notice tone="info">{t("clinical.specialties.systemHint")}</Notice>
        ) : (
          <TranslationsField label={t("clinical.specialties.name")} hint={t("clinical.oneLanguage")} value={label} onChange={setLabel} error={submitted && !hasText(label) ? t("rule.required") : null} testId="specialty-label" />
        )}
        <Switch checked={active} onChange={setActive} label={t("clinical.active")} testId="specialty-active" />
      </div>
    </Dialog>
  );
}

// ───────────────────────────── medicines (5.9) ─────────────────────────────

function DrugsTab() {
  const { t, lang } = useI18n();
  const toast = useToast();
  const list = useList(() => rpc("drugs.list", { include_inactive: true }));
  const [editing, setEditing] = useState<{ d?: DrugInfo } | null>(null);
  const [q, setQ] = useState("");
  if (list.error && !list.items) return <ErrorState message={list.error} onRetry={list.reload} />;
  const shown = (list.items ?? []).filter((d) => !q.trim() || `${d.name} ${d.strength ?? ""}`.toLowerCase().includes(q.trim().toLowerCase()));
  return (
    <Card flush>
      <CardHeader icon={Pill} title={t("clinical.drugs.title")} description={t("clinical.drugs.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setEditing({})} data-testid="drug-new">{t("clinical.drugs.add")}</Button>} />
      <div className="catalog-toolbar"><TextInput dir="ltr" placeholder={t("rx.searchPlaceholder")} value={q} onChange={(e) => setQ(e.target.value)} data-testid="drug-search" /></div>
      <div className="table-wrap">
        <table className="table" data-testid="drug-list">
          <thead><tr><th>{t("rx.medicine")}</th><th>{t("rx.formLabel")}</th><th>{t("clinical.drugs.defaults")}</th><th>{t("clinical.drugs.classes")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr></thead>
          <tbody>
            {!list.items && <SkeletonRows cols={5} />}
            {shown.map((d) => (
              <tr key={d.id} className={d.is_active ? "" : "row-inactive"} data-testid={`drug-row-${d.code}`}>
                <td><bdi dir="ltr" className="cell-strong">{d.name}</bdi> <bdi dir="ltr" className="subtle">{d.strength}</bdi>{!d.is_active && <> <Badge dot>{t("users.inactive")}</Badge></>}</td>
                <td>{t(`rx.form.${d.form}`)}</td>
                <td className="subtle t-caption">{[d.times_per_day && t("rx.sentence.timesPerDay").replace("{n}", digits(d.times_per_day, lang)), d.timing && t(`rx.timing.${d.timing}`), d.days && t("rx.sentence.days").replace("{n}", digits(d.days, lang))].filter(Boolean).join("، ") || "—"}</td>
                <td>{d.classes.map((c) => <Badge key={c}>{t(`clinical.class.${c}`)}</Badge>)}</td>
                <td className="cell-actions"><IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ d })} data-testid={`drug-edit-${d.code}`} /></td>
              </tr>
            ))}
          </tbody>
        </table>
        {list.items && shown.length === 0 && <EmptyState icon={Pill} title={t("clinical.none")} />}
      </div>
      {editing && <DrugDialog d={editing.d} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); toast.success(t("common.saved")); list.reload(); }} />}
    </Card>
  );
}

function num(s: string): number | null {
  const v = latinDigits(s.trim());
  return v ? Number(v) : null;
}

function DrugDialog({ d, onClose, onSaved }: { d?: DrugInfo; onClose: () => void; onSaved: () => void }) {
  const { t, lang } = useI18n();
  const fail = useFailure();
  const [v, setV] = useState({
    name: d?.name ?? "", form: d?.form ?? "tablet", strength: d?.strength ?? "", quantity: d?.quantity ?? "", dose: d?.dose ?? "",
    times: d?.times_per_day ? digits(d.times_per_day, lang) : "", timing: d?.timing ?? "", days: d?.days ? digits(d.days, lang) : "",
  });
  const [classes, setClasses] = useState<string[]>(d?.classes ?? []);
  const [asNeeded, setAsNeeded] = useState(d?.as_needed ?? false);
  const [active, setActive] = useState(d?.is_active ?? true);
  const [submitted, setSubmitted] = useState(false);
  const [server, setServer] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const set = (k: keyof typeof v) => (e: { target: { value: string } }) => { setV({ ...v, [k]: e.target.value }); setServer({}); };
  const times = num(v.times), days = num(v.days);
  const errors: Record<string, string | null> = {
    name: !v.name.trim() ? "rule.required" : v.name.trim().length > 120 ? "rule.document_text" : null,
    times_per_day: times != null && (!Number.isInteger(times) || times < 1 || times > 12) ? "rule.rx_dose_range" : null,
    days: days != null && (!Number.isInteger(days) || days < 1 || days > 365) ? "rule.rx_dose_range" : null,
  };
  const show = (k: string) => (server[k] ? t(server[k]) : submitted && errors[k] ? t(errors[k]!) : null);
  const save = async () => {
    setSubmitted(true);
    if (Object.values(errors).some(Boolean)) return;
    setBusy(true);
    try {
      await rpc("drugs.save", {
        id: d?.id ?? null, version: d?.version ?? 0, name: v.name.trim(), form: v.form, strength: v.strength.trim() || null, classes,
        quantity: v.quantity.trim() || null, dose: v.dose.trim() || null, times_per_day: times, timing: (v.timing || null) as RxTiming | null, days, as_needed: asNeeded, is_active: active,
      });
      onSaved();
    } catch (x) {
      if (isSessionError(x)) return;
      if (x instanceof ApiError && x.field) setServer({ [x.field]: x.rule ? `rule.${x.rule}` : `error.${x.code}` });
      else setError(fail(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={d ? t("clinical.drugs.edit") : t("clinical.drugs.add")} onClose={onClose} wide testId="drug-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="drug-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="grid-2">
          <Field label={t("rx.medicine")} hint={t("clinical.drugs.nameHint")} error={show("name")}><TextInput dir="ltr" value={v.name} onChange={set("name")} data-testid="drug-name" /></Field>
          <Field label={t("rx.strength")} optional error={show("strength")}><TextInput dir="ltr" value={v.strength} onChange={set("strength")} data-testid="drug-strength" /></Field>
          <Field label={t("rx.formLabel")} error={show("form")}>
            <Select value={v.form} onChange={set("form")} data-testid="drug-form">{FORMS.map((f) => <option key={f} value={f}>{t(`rx.form.${f}`)}</option>)}</Select>
          </Field>
          <Field label={t("rx.quantity")} optional error={show("quantity")}><TextInput value={v.quantity} onChange={set("quantity")} data-testid="drug-quantity" /></Field>
          <Field label={t("rx.dose")} optional error={show("dose")}><TextInput value={v.dose} onChange={set("dose")} data-testid="drug-dose" /></Field>
          <Field label={t("rx.timesPerDay")} optional error={show("times_per_day")}><TextInput inputMode="numeric" value={v.times} onChange={set("times")} data-testid="drug-times" /></Field>
          <Field label={t("rx.timing")} optional>
            <Select value={v.timing} onChange={set("timing")} data-testid="drug-timing"><option value="">—</option>{TIMINGS.map((x) => <option key={x} value={x}>{t(`rx.timing.${x}`)}</option>)}</Select>
          </Field>
          <Field label={t("rx.days")} optional error={show("days")}><TextInput inputMode="numeric" suffix={t("rx.daysUnit")} value={v.days} onChange={set("days")} data-testid="drug-days" /></Field>
        </div>
        <div className="field">
          <span className="field-label">{t("clinical.drugs.classes")}</span>
          <ChipGroup label={t("clinical.drugs.classes")} options={CLASSES.map((c) => ({ value: c, label: t(`clinical.class.${c}`), testId: `drug-class-${c}` }))} selected={classes} onChange={setClasses} />
          <span className="field-hint">{t("clinical.drugs.classesHint")}</span>
        </div>
        <div className="row" style={{ gap: "var(--space-6)", flexWrap: "wrap" }}>
          <Checkbox checked={asNeeded} onChange={setAsNeeded} testId="drug-prn">{t("rx.asNeeded")}</Checkbox>
          {d && <Switch checked={active} onChange={setActive} label={t("clinical.active")} testId="drug-active" />}
        </div>
      </div>
    </Dialog>
  );
}

// ───────────────────────────── ready-made prescriptions (5.9) ─────────────────────────────

function RxTemplatesTab() {
  const { t, lang } = useI18n();
  const toast = useToast();
  const list = useList(() => rpc("rx_templates.list", { include_inactive: true }));
  const drugs = useList(() => rpc("drugs.list", { include_inactive: false }));
  const [editing, setEditing] = useState<{ x?: RxTemplateInfo } | null>(null);
  if (list.error && !list.items) return <ErrorState message={list.error} onRetry={list.reload} />;
  return (
    <Card flush>
      <CardHeader icon={Wand2} title={t("clinical.rx.title")} description={t("clinical.rx.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setEditing({})} data-testid="rxt-new">{t("clinical.rx.add")}</Button>} />
      <div className="table-wrap">
        <table className="table" data-testid="rxt-list">
          <thead><tr><th>{t("clinical.rx.name")}</th><th>{t("clinical.rx.medicines")}</th><th>{t("clinical.active")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr></thead>
          <tbody>
            {!list.items && <SkeletonRows cols={4} />}
            {list.items?.map((x) => (
              <tr key={x.id} className={x.is_active ? "" : "row-inactive"} data-testid={`rxt-row-${x.code}`}>
                <td className="cell-strong">{tr(x.name, lang)}</td>
                <td><bdi dir="ltr" className="subtle">{x.items.map((i) => i.name).join(" · ")}</bdi></td>
                <td>{x.is_active ? <Badge tone="success" dot>{t("users.active")}</Badge> : <Badge dot>{t("users.inactive")}</Badge>}</td>
                <td className="cell-actions"><IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ x })} data-testid={`rxt-edit-${x.code}`} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {editing && <RxTemplateDialog x={editing.x} drugs={drugs.items ?? []} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); toast.success(t("common.saved")); list.reload(); }} />}
    </Card>
  );
}

function RxTemplateDialog({ x, drugs, onClose, onSaved }: { x?: RxTemplateInfo; drugs: DrugInfo[]; onClose: () => void; onSaved: () => void }) {
  const { t, lang } = useI18n();
  const fail = useFailure();
  const [name, setName] = useState<Translations>(x?.name ?? empty);
  const [items, setItems] = useState<RxItem[]>(x?.items ?? []);
  const [active, setActive] = useState(x?.is_active ?? true);
  const [pick, setPick] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const add = (id: string) => {
    const d = drugs.find((z) => z.id === id);
    if (d) setItems((l) => [...l, { drug_id: d.id, name: d.name, form: d.form, strength: d.strength, quantity: d.quantity, dose: d.dose, times_per_day: d.times_per_day, timing: d.timing, days: d.days, as_needed: d.as_needed, note: null }]);
    setPick("");
  };
  const setLine = (n: number, patch: Partial<RxItem>) => setItems((l) => l.map((i, k) => (k === n ? { ...i, ...patch } : i)));
  const save = async () => {
    setSubmitted(true);
    if (!hasText(name) || !items.length) return;
    setBusy(true);
    try {
      await rpc("rx_templates.save", { id: x?.id ?? null, version: x?.version ?? 0, name, items, is_active: active });
      onSaved();
    } catch (e) {
      if (!isSessionError(e)) setError(fail(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={x ? t("clinical.rx.edit") : t("clinical.rx.add")} onClose={onClose} wide testId="rxt-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="rxt-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("clinical.rx.name")} hint={t("clinical.oneLanguage")} value={name} onChange={setName} error={submitted && !hasText(name) ? t("rule.required") : null} testId="rxt-name" />
        <Field label={t("clinical.rx.addMedicine")} hint={submitted && !items.length ? undefined : t("clinical.rx.addHint")} error={submitted && !items.length ? t("rule.document_items") : null}>
          <Select value={pick} onChange={(e) => add(e.target.value)} data-testid="rxt-add">
            <option value="">—</option>
            {drugs.map((d) => <option key={d.id} value={d.id}>{`${d.name} ${d.strength ?? ""}`.trim()}</option>)}
          </Select>
        </Field>
        <ol className="rx-lines">
          {items.map((i, n) => (
            <li key={n} className="rx-line" data-testid={`rxt-line-${n}`}>
              <div className="row" style={{ justifyContent: "space-between" }}>
                <bdi dir="ltr" className="cell-strong">{[i.name, i.strength].filter(Boolean).join(" ")}</bdi>
                <IconButton icon={Trash2} size="sm" label={t("rx.remove")} onClick={() => setItems((l) => l.filter((_, k) => k !== n))} />
              </div>
              <div className="rx-line-grid">
                <Field label={t("rx.timesPerDay")} optional><TextInput inputMode="numeric" value={i.times_per_day == null ? "" : digits(i.times_per_day, lang)} onChange={(e) => setLine(n, { times_per_day: num(e.target.value) })} /></Field>
                <Field label={t("rx.days")} optional><TextInput inputMode="numeric" value={i.days == null ? "" : digits(i.days, lang)} onChange={(e) => setLine(n, { days: num(e.target.value) })} data-testid={`rxt-line-${n}-days`} /></Field>
                <Field label={t("rx.quantity")} optional><TextInput value={i.quantity ?? ""} onChange={(e) => setLine(n, { quantity: e.target.value || null })} /></Field>
                <Field label={t("rx.timing")} optional>
                  <Select value={i.timing ?? ""} onChange={(e) => setLine(n, { timing: (e.target.value || null) as RxTiming | null })}><option value="">—</option>{TIMINGS.map((z) => <option key={z} value={z}>{t(`rx.timing.${z}`)}</option>)}</Select>
                </Field>
              </div>
            </li>
          ))}
        </ol>
        {x && <Switch checked={active} onChange={setActive} label={t("clinical.active")} testId="rxt-active" />}
      </div>
    </Dialog>
  );
}

// ───────────────────────────── document templates (5.10, 5.10b) ─────────────────────────────

function TemplatesTab() {
  const { t, lang } = useI18n();
  const toast = useToast();
  const list = useList(() => rpc("document_templates.list", { kind: null, include_inactive: true }));
  const [editing, setEditing] = useState<{ x?: DocumentTemplateInfo; kind: DocumentKind } | null>(null);
  if (list.error && !list.items) return <ErrorState message={list.error} onRetry={list.reload} />;
  return (
    <div className="stack-lg">
      {(["consent", "post_op"] as DocumentKind[]).map((kind) => (
        <Card flush key={kind}>
          <CardHeader icon={kind === "consent" ? FileSignature : BookOpenText} title={t(`clinical.templates.${kind}`)} description={t("clinical.templates.hint")} actions={<Button icon={Plus} onClick={() => setEditing({ kind })} data-testid={`tpl-new-${kind}`}>{t("clinical.templates.add")}</Button>} />
          <div className="table-wrap">
            <table className="table" data-testid={`tpl-list-${kind}`}>
              <thead><tr><th>{t("docs.title")}</th><th>{t("print.paper")}</th><th>{t("clinical.active")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr></thead>
              <tbody>
                {!list.items && <SkeletonRows cols={4} />}
                {list.items?.filter((x) => x.kind === kind).map((x) => (
                  <tr key={x.id} className={x.is_active ? "" : "row-inactive"} data-testid={`tpl-row-${x.code}`}>
                    <td><span className="cell-strong">{tr(x.title, lang)}</span>{x.customized && <> <Badge tone="accent">{t("clinical.templates.customized")}</Badge></>}{!x.is_system && <> <Badge tone="accent">{t("clinical.own")}</Badge></>}</td>
                    <td>{t(`print.paper.${x.paper}`)}</td>
                    <td>{x.is_active ? <Badge tone="success" dot>{t("users.active")}</Badge> : <Badge dot>{t("users.inactive")}</Badge>}</td>
                    <td className="cell-actions"><IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ x, kind })} data-testid={`tpl-edit-${x.code}`} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      ))}
      {editing && <TemplateDialog x={editing.x} kind={editing.kind} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); toast.success(t("common.saved")); list.reload(); }} />}
    </div>
  );
}

function TemplateDialog({ x, kind, onClose, onSaved }: { x?: DocumentTemplateInfo; kind: DocumentKind; onClose: () => void; onSaved: () => void }) {
  const { t } = useI18n();
  const fail = useFailure();
  const [title, setTitle] = useState<Translations>(x?.title ?? empty);
  const [body, setBody] = useState<Translations>(x?.body ?? empty);
  const [paper, setPaper] = useState<Paper>(x?.paper ?? (kind === "consent" ? "a4" : "a5"));
  const [active, setActive] = useState(x?.is_active ?? true);
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState<"" | "save" | "reset">("");
  const [error, setError] = useState("");
  const save = async () => {
    setSubmitted(true);
    if (!hasText(title) || !hasText(body)) return;
    setBusy("save");
    try {
      await rpc("document_templates.save", { id: x?.id ?? null, version: x?.version ?? 0, kind, title, body, paper, is_active: active });
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
      await rpc("document_templates.reset", { id: x.id, version: x.version });
      onSaved();
    } catch (e) {
      if (!isSessionError(e)) setError(fail(e));
    } finally {
      setBusy("");
    }
  };
  return (
    <Dialog title={x ? t("clinical.templates.edit") : t("clinical.templates.add")} onClose={onClose} wide testId="tpl-dialog"
      footer={
        <>
          {x?.is_system && x.customized && <Button variant="subtle" icon={RotateCcw} loading={busy === "reset"} onClick={reset} data-testid="tpl-reset">{t("clinical.templates.reset")}</Button>}
          <span className="grow" />
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" loading={busy === "save"} onClick={save} data-testid="tpl-save">{t("common.save")}</Button>
        </>
      }>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <Notice tone="info" title={t("clinical.templates.placeholders")}>{t("clinical.templates.placeholdersHint")}</Notice>
        <TranslationsField label={t("docs.title")} value={title} onChange={setTitle} error={submitted && !hasText(title) ? t("rule.required") : null} testId="tpl-title" />
        <TranslationsField label={t("docs.text")} hint={t("clinical.oneLanguage")} value={body} onChange={setBody} multiline rows={8} error={submitted && !hasText(body) ? t("rule.required") : null} testId="tpl-body" />
        <div className="row" style={{ gap: "var(--space-6)", flexWrap: "wrap", alignItems: "flex-end" }}>
          <div className="field">
            <span className="field-label">{t("print.paper")}</span>
            <Segmented<Paper> value={paper} onChange={setPaper} label={t("print.paper")} options={(["a4", "a5", "a6"] as Paper[]).map((p) => ({ value: p, label: t(`print.paper.${p}`) }))} />
          </div>
          {x && <Switch checked={active} onChange={setActive} label={t("clinical.active")} testId="tpl-active" />}
        </div>
      </div>
    </Dialog>
  );
}
