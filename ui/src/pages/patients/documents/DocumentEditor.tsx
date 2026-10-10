import { useEffect, useMemo, useRef, useState } from "react";
import { FileCheck2, RefreshCw } from "lucide-react";
import type { CalendarSystem, DoctorInfo, DocumentContent, DocumentInfo, DocumentKind, DocumentTemplateInfo, Language, Paper, PatientInfo } from "../../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../../lib/api";
import { todayIso } from "../../../lib/calendar";
import { latinDigits, formatDate } from "../../../lib/dates";
import { useFormDraft } from "../../../lib/draft";
import { tr } from "../../../lib/medical";
import { serviceName, useCatalog } from "../../../lib/clinical";
import { useI18n } from "../../../i18n";
import { Button } from "../../../ui/Button";
import { ChipGroup, Segmented, Switch } from "../../../ui/Controls";
import { DateField } from "../../../ui/DateField";
import { Field, Select, Textarea, TextInput } from "../../../ui/Field";
import { Notice } from "../../../ui/Feedback";
import { Dialog } from "../../../ui/Overlay";
import { docT, fillTemplate } from "../../../print/docText";
import { DocumentSheet } from "../../../print/DocumentSheet";
import type { Letterhead } from "../../../print/letterhead";
import { docPatient, LanguagePick, LivePreview } from "./shared";

export const OTHER_KINDS: DocumentKind[] = ["consent", "post_op", "referral", "imaging_request", "certificate", "lab_order", "record_summary"];
const IMAGING = ["opg", "cbct", "periapical", "bitewing", "cephalometric", "occlusal", "blood_cbc", "blood_coagulation", "blood_sugar", "hepatitis_hiv", "other"];
const SHADES = ["A1", "A2", "A3", "A3.5", "A4", "B1", "B2", "B3", "B4", "C1", "C2", "C3", "C4", "D2", "D3", "D4"];
const DEFAULT_PAPER: Record<DocumentKind, Paper> = { prescription: "a5", consent: "a4", post_op: "a5", referral: "a4", imaging_request: "a5", certificate: "a5", lab_order: "a5", record_summary: "a4" };

type V = Record<string, string>;

/** Field values of each kind, as strings (the form-draft store keeps strings, rule 14). */
function emptyValues(kind: DocumentKind): V {
  return {
    kind, doctor_id: "", language: "", paper: DEFAULT_PAPER[kind], template_id: "", title: "", body: "", edited: "", procedure: "", teeth: "",
    to: "", specialty: "", reason: "", summary: "", urgent: "", tests: "", center: "", notes: "", visit_date: todayIso(), rest_days: "", rest_from: "",
    addressee: "", lab: "", work: "", material: "", shade: "", due_date: "", purpose: "",
  };
}

const opt = (s: string) => s.trim() || null;

function content(kind: DocumentKind, v: V): DocumentContent {
  switch (kind) {
    case "consent":
    case "post_op":
      return { kind, template_id: opt(v.template_id), title: v.title.trim(), body: v.body.trim(), procedure: opt(v.procedure), teeth: opt(v.teeth) };
    case "referral":
      return { kind, to: v.to.trim(), specialty: opt(v.specialty), reason: v.reason.trim(), summary: opt(v.summary), urgent: v.urgent === "1" };
    case "imaging_request":
      return { kind, tests: v.tests ? v.tests.split(",") : [], teeth: opt(v.teeth), center: opt(v.center), notes: opt(v.notes) };
    case "certificate":
      return { kind, visit_date: v.visit_date, rest_days: v.rest_days.trim() ? Number(latinDigits(v.rest_days)) : null, rest_from: v.rest_days.trim() ? opt(v.rest_from) ?? v.visit_date : null, addressee: opt(v.addressee), notes: opt(v.notes) };
    case "lab_order":
      return { kind, lab: opt(v.lab), teeth: v.teeth.trim(), work: v.work.trim(), material: opt(v.material), shade: opt(v.shade), due_date: opt(v.due_date), notes: opt(v.notes) };
    case "record_summary":
    case "prescription":
      return { kind: "record_summary", purpose: opt(v.purpose), snapshot: null };
  }
}

function checks(kind: DocumentKind): Partial<Record<string, (s: string, all: V) => string | null>> {
  const req = (max: number) => (s: string) => (!s.trim() ? "rule.required" : s.trim().length > max ? "rule.document_text" : null);
  const max = (n: number) => (s: string) => (s.trim().length > n ? "rule.document_text" : null);
  switch (kind) {
    case "consent":
    case "post_op":
      return { title: req(200), body: req(8000), procedure: max(200), teeth: max(100) };
    case "referral":
      return { to: req(200), specialty: max(100), reason: req(1000), summary: max(3000) };
    case "imaging_request":
      return { tests: (s) => (!s ? "rule.document_items" : null), teeth: max(100), center: max(200), notes: (s, all) => (all.tests.split(",").includes("other") && !s.trim() ? "rule.required" : max(1000)(s)) };
    case "certificate":
      return {
        visit_date: (s) => (!s ? "rule.required" : null),
        rest_days: (s) => {
          if (!s.trim()) return null;
          const n = Number(latinDigits(s.trim()));
          return Number.isInteger(n) && n >= 1 && n <= 60 ? null : "rule.rest_days_range";
        },
        addressee: max(200),
        notes: max(1000),
      };
    case "lab_order":
      return { teeth: req(100), work: req(200), lab: max(200), material: max(100), shade: max(20), notes: max(1000) };
    default:
      return { purpose: max(200) };
  }
}

/**
 * Writes one clinical document (5.10, 5.10b) from a form, with a live real-size preview: consent
 * forms and after-treatment sheets from the clinic's editable templates (placeholders filled in),
 * referral, imaging/lab request, medical certificate, lab work order and record summary.
 */
export function DocumentEditor({ kind, patient, gender, letterhead, calendar, appointmentId, initial, onClose, onIssued }: {
  kind: DocumentKind;
  patient: PatientInfo;
  gender?: string | null;
  letterhead: Letterhead;
  calendar: CalendarSystem;
  appointmentId?: string | null;
  /** Prefilled values (e.g. the consent template a service suggests). */
  initial?: Partial<V>;
  onClose: () => void;
  onIssued: (d: DocumentInfo) => void;
}) {
  const { t, err, lang: uiLang } = useI18n();
  const catalog = useCatalog();
  const [doctors, setDoctors] = useState<DoctorInfo[] | null>(null);
  const [templates, setTemplates] = useState<DocumentTemplateInfo[]>([]);
  const start = useMemo(() => ({ ...emptyValues(kind), language: patient.preferred_language ?? letterhead.clinic.default_language, ...initial }), []);
  const form = useForm2(start, checks(kind));
  const draft = useFormDraft(`document.${kind}.${patient.id}`, form.values, true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const v = form.values;
  const lang = v.language as Language;

  useEffect(() => {
    rpc("doctors.list", { include_inactive: false }).then((d) => {
      setDoctors(d);
      if (d.length === 1 && !form.values.doctor_id) form.set("doctor_id", d[0].id, true);
    }).catch(() => setDoctors([]));
    if (kind === "consent" || kind === "post_op") {
      rpc("document_templates.list", { kind, include_inactive: false }).then((list) => {
        setTemplates(list);
        if (!form.values.template_id && list[0]) pickTemplate(list[0].id, list);
      }).catch(() => {});
    }
  }, []);

  const doctor = doctors?.find((d) => d.id === v.doctor_id) ?? null;
  // A template's words in the document's language with the patient, doctor, tooth … filled in.
  const fromTemplate = (id: string, list = templates, values: V = form.values) => {
    const tp = list.find((x) => x.id === id);
    if (!tp) return null;
    const l = values.language as Language;
    const fill = (s: string) =>
      fillTemplate(s, {
        patient: patient.full_name,
        doctor: doctors?.find((d) => d.id === values.doctor_id)?.full_name ?? "",
        clinic: letterhead.clinic.name,
        date: formatDate(todayIso() + "T06:00:00Z", l, calendar),
        procedure: values.procedure,
        teeth: values.teeth,
      });
    return { title: tr(tp.title, l), body: fill(tr(tp.body, l)), paper: tp.paper };
  };
  const pickTemplate = (id: string, list = templates) => {
    const f = fromTemplate(id, list);
    form.set("template_id", id, true);
    if (f) {
      form.set("title", f.title, true);
      form.set("body", f.body, true);
      form.set("paper", f.paper, true);
      form.set("edited", "", true);
    }
  };
  // Until the text is edited by hand it follows the language, doctor, procedure and teeth.
  useEffect(() => {
    if ((kind === "consent" || kind === "post_op") && v.template_id && !v.edited) {
      const f = fromTemplate(v.template_id);
      if (f && (f.body !== v.body || f.title !== v.title)) {
        form.set("title", f.title, true);
        form.set("body", f.body, true);
      }
    }
  }, [v.language, v.doctor_id, v.procedure, v.teeth, templates, doctors]);

  const issue = async () => {
    setError("");
    if (!form.validate()) {
      window.setTimeout(() => root.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus(), 0);
      return;
    }
    setBusy(true);
    try {
      const doc = await rpc("documents.issue", { patient_id: patient.id, doctor_id: v.doctor_id || null, appointment_id: appointmentId ?? null, language: lang, paper: v.paper as Paper, content: content(kind, v) });
      draft.clear();
      onIssued(doc);
    } catch (x) {
      if (isSessionError(x)) return;
      if (!form.serverError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(false);
    }
  };

  const e = (k: string) => form.error(k) && t(form.error(k)!);
  const dt = docT(lang);
  const previewDoc: DocumentInfo = {
    id: "draft", number: t("rx.draftNumber"), kind, patient_id: patient.id, patient: docPatient(patient, gender), doctor_id: doctor?.id ?? null,
    doctor: doctor ? { name: doctor.full_name, license_number: doctor.license_number, specialties: [] } : null,
    appointment_id: null, language: lang, paper: v.paper as Paper,
    content: kind === "record_summary" ? { kind: "record_summary", purpose: opt(v.purpose), snapshot: { medical: [dt("doc.summary.fromRecord")], medical_notes: null, visits: [], prescriptions: [] } } : content(kind, v),
    issued_at: new Date().toISOString(), issued_by_name: null, print_count: 0, last_printed_at: null, attachment_id: null, status: "issued", void_reason: null, version: 0,
  };
  const labWorks = catalog?.services.filter((s) => s.lab_required) ?? [];

  return (
    <Dialog
      title={t(`docs.new.${kind}`)}
      description={patient.full_name}
      onClose={onClose}
      wide
      testId="doc-editor"
      footer={
        <>
          <span className="grow" />
          <Button onClick={() => { draft.clear(); onClose(); }} data-testid="doc-cancel">{t("common.cancel")}</Button>
          <Button variant="primary" icon={FileCheck2} loading={busy} onClick={issue} data-testid="doc-issue">{t("docs.issue")}</Button>
        </>
      }
    >
      <div className="editor-layout" ref={root}>
        <div className="stack editor-main">
          {draft.offer && (
            <Notice tone="info" title={t("draft.found.title")} testId="draft-offer">
              <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap" }}>
                <span>{t("draft.found.hint")}</span>
                <span className="row">
                  <Button size="sm" variant="primary" onClick={() => { form.reset({ ...start, ...draft.offer }); draft.settle(); }} data-testid="draft-restore">{t("draft.found.restore")}</Button>
                  <Button size="sm" onClick={() => { draft.clear(); draft.settle(); }} data-testid="draft-discard">{t("draft.found.discard")}</Button>
                </span>
              </div>
            </Notice>
          )}
          {error && <Notice tone="danger" testId="doc-error">{error}</Notice>}
          <div className="grid-2">
            {(doctors?.length ?? 0) > 1 && (
              <Field label={t("docs.doctor")} optional>
                <Select value={v.doctor_id} onChange={(x) => form.set("doctor_id", x.target.value)} data-testid="doc-doctor">
                  <option value="">—</option>
                  {doctors?.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
                </Select>
              </Field>
            )}
            <LanguagePick value={lang} onChange={(l) => form.set("language", l)} testId="doc-language" />
            <div className="field">
              <span className="field-label">{t("print.paper")}</span>
              <Segmented<string> value={v.paper} onChange={(p) => form.set("paper", p)} label={t("print.paper")} options={(["a4", "a5", "a6"] as Paper[]).map((p) => ({ value: p, label: t(`print.paper.${p}`), testId: `doc-paper-${p}` }))} />
            </div>
          </div>

          {(kind === "consent" || kind === "post_op") && (
            <>
              <Field label={t("docs.template")} hint={t("docs.templateHint")}>
                <Select value={v.template_id} onChange={(x) => pickTemplate(x.target.value)} data-testid="doc-template">
                  {templates.map((tp) => <option key={tp.id} value={tp.id}>{tr(tp.title, uiLang)}</option>)}
                </Select>
              </Field>
              <div className="grid-2">
                <Field label={t("docs.procedure")} optional error={e("procedure")}>
                  <TextInput value={v.procedure} onChange={(x) => form.set("procedure", x.target.value)} data-testid="doc-procedure" />
                </Field>
                <Field label={t("docs.teeth")} hint={t("docs.teethHint")} optional error={e("teeth")}>
                  <TextInput dir="ltr" value={v.teeth} onChange={(x) => form.set("teeth", x.target.value)} data-testid="doc-teeth" />
                </Field>
              </div>
              <Field label={t("docs.title")} error={e("title")}>
                <TextInput value={v.title} onChange={(x) => { form.set("title", x.target.value); form.set("edited", "1", true); }} data-testid="doc-title" />
              </Field>
              <Field label={t("docs.text")} hint={v.edited ? undefined : t("docs.textHint")} error={e("body")}>
                <Textarea rows={9} dir={lang === "en" ? "ltr" : "rtl"} value={v.body} onChange={(x) => { form.set("body", x.target.value); form.set("edited", "1", true); }} data-testid="doc-body" />
              </Field>
              {v.edited && v.template_id && (
                <Button size="sm" variant="subtle" icon={RefreshCw} onClick={() => pickTemplate(v.template_id)} data-testid="doc-refill">{t("docs.refill")}</Button>
              )}
            </>
          )}

          {kind === "referral" && (
            <>
              <div className="grid-2">
                <Field label={t("docs.referral.to")} hint={t("docs.referral.toHint")} error={e("to")}>
                  <TextInput value={v.to} onChange={(x) => form.set("to", x.target.value)} data-testid="doc-to" />
                </Field>
                <Field label={t("docs.referral.specialty")} optional error={e("specialty")}>
                  <TextInput value={v.specialty} onChange={(x) => form.set("specialty", x.target.value)} data-testid="doc-specialty" />
                </Field>
              </div>
              <Field label={t("docs.referral.reason")} error={e("reason")}>
                <Textarea rows={3} value={v.reason} onChange={(x) => form.set("reason", x.target.value)} data-testid="doc-reason" />
              </Field>
              <Field label={t("docs.referral.summary")} hint={t("docs.referral.summaryHint")} optional error={e("summary")}>
                <Textarea rows={4} value={v.summary} onChange={(x) => form.set("summary", x.target.value)} data-testid="doc-summary" />
              </Field>
              <Switch checked={v.urgent === "1"} onChange={(on) => form.set("urgent", on ? "1" : "")} label={t("docs.referral.urgent")} testId="doc-urgent" />
            </>
          )}

          {kind === "imaging_request" && (
            <>
              <div className="field">
                <span className="field-label">{t("docs.imaging.tests")}</span>
                <ChipGroup
                  label={t("docs.imaging.tests")}
                  options={IMAGING.map((x) => ({ value: x, label: t(`doc.test.${x}`), testId: `doc-test-${x}` }))}
                  selected={v.tests ? v.tests.split(",") : []}
                  onChange={(next) => form.set("tests", next.join(","))}
                  testId="doc-tests"
                />
                {e("tests") && <div className="field-error" role="alert" data-testid="field-error"><span>{e("tests")}</span></div>}
              </div>
              <div className="grid-2">
                <Field label={t("docs.teeth")} hint={t("docs.teethHint")} optional error={e("teeth")}>
                  <TextInput dir="ltr" value={v.teeth} onChange={(x) => form.set("teeth", x.target.value)} data-testid="doc-teeth" />
                </Field>
                <Field label={t("docs.imaging.center")} optional error={e("center")}>
                  <TextInput value={v.center} onChange={(x) => form.set("center", x.target.value)} data-testid="doc-center" />
                </Field>
              </div>
              <Field label={t("docs.notes")} optional error={e("notes")}>
                <Textarea rows={2} value={v.notes} onChange={(x) => form.set("notes", x.target.value)} data-testid="doc-notes" />
              </Field>
            </>
          )}

          {kind === "certificate" && (
            <>
              <div className="grid-2">
                <Field label={t("docs.certificate.visit")} error={e("visit_date")}>
                  <DateField value={v.visit_date} onChange={(d) => form.set("visit_date", d)} calendar={calendar} label={t("docs.certificate.visit")} testId="doc-visit-date" />
                </Field>
                <Field label={t("docs.certificate.restDays")} hint={t("docs.certificate.restHint")} optional error={e("rest_days")}>
                  <TextInput inputMode="numeric" suffix={t("rx.daysUnit")} value={v.rest_days} onChange={(x) => form.set("rest_days", x.target.value)} data-testid="doc-rest-days" />
                </Field>
                {v.rest_days.trim() && (
                  <Field label={t("docs.certificate.restFrom")}>
                    <DateField value={v.rest_from || v.visit_date} onChange={(d) => form.set("rest_from", d)} calendar={calendar} label={t("docs.certificate.restFrom")} testId="doc-rest-from" />
                  </Field>
                )}
                <Field label={t("docs.certificate.addressee")} hint={t("docs.certificate.addresseeHint")} optional error={e("addressee")}>
                  <TextInput value={v.addressee} onChange={(x) => form.set("addressee", x.target.value)} data-testid="doc-addressee" />
                </Field>
              </div>
              <Field label={t("docs.notes")} optional error={e("notes")}>
                <Textarea rows={2} value={v.notes} onChange={(x) => form.set("notes", x.target.value)} data-testid="doc-notes" />
              </Field>
            </>
          )}

          {kind === "lab_order" && (
            <>
              <div className="grid-2">
                <Field label={t("docs.lab.lab")} optional error={e("lab")}>
                  <TextInput value={v.lab} onChange={(x) => form.set("lab", x.target.value)} data-testid="doc-lab" />
                </Field>
                <Field label={t("docs.teeth")} hint={t("docs.teethHint")} error={e("teeth")}>
                  <TextInput dir="ltr" value={v.teeth} onChange={(x) => form.set("teeth", x.target.value)} data-testid="doc-teeth" />
                </Field>
                <Field label={t("docs.lab.work")} hint={labWorks.length ? t("docs.lab.workHint") : undefined} error={e("work")}>
                  <TextInput value={v.work} list="lab-works" onChange={(x) => form.set("work", x.target.value)} data-testid="doc-work" />
                </Field>
                <Field label={t("docs.lab.material")} optional error={e("material")}>
                  <TextInput value={v.material} onChange={(x) => form.set("material", x.target.value)} data-testid="doc-material" />
                </Field>
                <Field label={t("docs.lab.shade")} optional error={e("shade")}>
                  <Select value={v.shade} onChange={(x) => form.set("shade", x.target.value)} data-testid="doc-shade">
                    <option value="">—</option>
                    {SHADES.map((s) => <option key={s} value={s}>{s}</option>)}
                  </Select>
                </Field>
                <Field label={t("docs.lab.due")} optional>
                  <DateField value={v.due_date || todayIso()} onChange={(d) => form.set("due_date", d)} calendar={calendar} label={t("docs.lab.due")} testId="doc-due" />
                </Field>
              </div>
              <datalist id="lab-works">{labWorks.map((s) => <option key={s.id} value={serviceName(s, catalog, lang)} />)}</datalist>
              <Field label={t("docs.notes")} optional error={e("notes")}>
                <Textarea rows={2} value={v.notes} onChange={(x) => form.set("notes", x.target.value)} data-testid="doc-notes" />
              </Field>
            </>
          )}

          {kind === "record_summary" && (
            <>
              <Notice tone="info">{t("docs.summary.hint")}</Notice>
              <Field label={t("docs.summary.purpose")} hint={t("docs.summary.purposeHint")} optional error={e("purpose")}>
                <TextInput value={v.purpose} onChange={(x) => form.set("purpose", x.target.value)} data-testid="doc-purpose" />
              </Field>
            </>
          )}
        </div>
        <LivePreview paper={v.paper as Paper}>
          <DocumentSheet doc={previewDoc} letterhead={letterhead} calendar={calendar} />
        </LivePreview>
      </div>
    </Dialog>
  );
}

/**
 * Small form state with live checks (like lib/validation's useForm) whose `set` can skip marking a
 * field as touched — values the form fills itself (template text) must not show errors yet.
 */
function useForm2(initial: V, rules: Partial<Record<string, (s: string, all: V) => string | null>>) {
  const [values, setValues] = useState<V>(initial);
  const [touched, setTouched] = useState<Record<string, boolean>>({});
  const [submitted, setSubmitted] = useState(false);
  const [server, setServer] = useState<Record<string, string>>({});
  const errorOf = (k: string) => rules[k]?.(values[k] ?? "", values) ?? server[k] ?? null;
  return {
    values,
    set: (k: string, val: string, silent = false) => {
      setValues((s) => ({ ...s, [k]: val }));
      if (!silent) setTouched((s) => ({ ...s, [k]: true }));
      setServer((s) => ({ ...s, [k]: "" }));
    },
    error: (k: string) => (submitted || touched[k] ? errorOf(k) || null : null),
    validate: () => {
      setSubmitted(true);
      return Object.keys(rules).every((k) => !rules[k]?.(values[k] ?? "", values));
    },
    serverError: (x: unknown) => {
      if (!(x instanceof ApiError) || !x.field || !(x.field in values)) return false;
      setServer((s) => ({ ...s, [x.field!]: x.rule ? `rule.${x.rule}` : `error.${x.code}` }));
      setSubmitted(true);
      return true;
    },
    reset: (next: V) => {
      setValues(next);
      setTouched({});
      setSubmitted(false);
      setServer({});
    },
  };
}
