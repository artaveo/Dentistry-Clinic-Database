import { useEffect, useMemo, useRef, useState } from "react";
import { FileCheck2, Pill, Plus, Search, Trash2, TriangleAlert, Wand2 } from "lucide-react";
import type { CalendarSystem, DoctorInfo, DocumentInfo, DrugInfo, Language, Paper, PatientInfo, RxItem, RxTemplateInfo, RxTiming, RxWarning } from "../../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../../lib/api";
import { digits, latinDigits } from "../../../lib/dates";
import { useFormDraft } from "../../../lib/draft";
import { tr } from "../../../lib/medical";
import { useI18n } from "../../../i18n";
import { Button, IconButton } from "../../../ui/Button";
import { Checkbox, Segmented } from "../../../ui/Controls";
import { Field, Select, Textarea, TextInput } from "../../../ui/Field";
import { Notice } from "../../../ui/Feedback";
import { Dialog } from "../../../ui/Overlay";
import { rxInstructions } from "../../../print/docText";
import { DocumentSheet } from "../../../print/DocumentSheet";
import type { Letterhead } from "../../../print/letterhead";
import { MedicalAlertBanner } from "../MedicalAlertBanner";
import { docPatient, LanguagePick, LivePreview } from "./shared";

const FORMS = ["tablet", "capsule", "syrup", "suspension", "mouthwash", "gel", "cream", "ointment", "injection", "drops", "spray", "other"];
const TIMINGS: RxTiming[] = ["after_food", "before_food", "with_food", "morning", "bedtime"];

function fromDrug(d: DrugInfo): RxItem {
  return { drug_id: d.id, name: d.name, form: d.form, strength: d.strength, quantity: d.quantity, dose: d.dose, times_per_day: d.times_per_day, timing: d.timing, days: d.days, as_needed: d.as_needed, note: null };
}

/** Client-side checks of one line (the Core checks again and names the field: `items.<n>.<field>`). */
function lineErrors(i: RxItem): Partial<Record<keyof RxItem, string>> {
  const e: Partial<Record<keyof RxItem, string>> = {};
  if (!i.name.trim()) e.name = "rule.required";
  else if (i.name.trim().length > 120) e.name = "rule.document_text";
  if (i.times_per_day != null && (i.times_per_day < 1 || i.times_per_day > 12)) e.times_per_day = "rule.rx_dose_range";
  if (i.days != null && (i.days < 1 || i.days > 365)) e.days = "rule.rx_dose_range";
  for (const [k, max] of [["strength", 40], ["quantity", 30], ["dose", 30], ["note", 200]] as const) if ((i[k] ?? "").length > max) e[k] = "rule.document_text";
  return e;
}

/**
 * New prescription (5.9): medicines from the clinic's formulary or typed, one-click ready-made
 * prescriptions, instructions written as a sentence in the patient's language, warnings against the
 * patient's medical checklist that must be acknowledged, and a live real-size preview. Kept as a
 * draft while unfinished (rule 14).
 */
export function PrescriptionEditor({ patient, letterhead, calendar, onClose, onIssued }: {
  patient: PatientInfo;
  letterhead: Letterhead;
  calendar: CalendarSystem;
  onClose: () => void;
  onIssued: (d: DocumentInfo) => void;
}) {
  const { t, err, lang: uiLang } = useI18n();
  const [doctors, setDoctors] = useState<DoctorInfo[] | null>(null);
  const [drugs, setDrugs] = useState<DrugInfo[]>([]);
  const [templates, setTemplates] = useState<RxTemplateInfo[]>([]);
  const defaultLang: Language = patient.preferred_language ?? letterhead.clinic.default_language;
  const [doctorId, setDoctorId] = useState("");
  const [language, setLanguage] = useState<Language>(defaultLang);
  const [paper, setPaper] = useState<Paper>("a5");
  const [items, setItems] = useState<RxItem[]>([]);
  const [notes, setNotes] = useState("");
  const [warnings, setWarnings] = useState<RxWarning[]>([]);
  const [ack, setAck] = useState<string[]>([]);
  const [submitted, setSubmitted] = useState(false);
  const [serverErrors, setServerErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    Promise.all([rpc("doctors.list", { include_inactive: false }), rpc("drugs.list", { include_inactive: false }), rpc("rx_templates.list", { include_inactive: false })])
      .then(([d, dr, tp]) => {
        setDoctors(d);
        setDrugs(dr);
        setTemplates(tp);
        // A doctor signed in as themselves writes their own prescriptions; a one-doctor clinic never picks.
        setDoctorId((cur) => cur || (d.length === 1 ? d[0].id : ""));
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, []);

  // Rule 14: an unfinished prescription survives a lock, another page or a closed app.
  const draftValues = useMemo(() => ({ doctor_id: doctorId, language, paper, notes, items: JSON.stringify(items) }), [doctorId, language, paper, notes, items]);
  const draft = useFormDraft(`prescription.${patient.id}`, draftValues, true);
  const restore = () => {
    const o = draft.offer!;
    setDoctorId(o.doctor_id);
    setLanguage(o.language as Language);
    setPaper(o.paper as Paper);
    setNotes(o.notes);
    try {
      setItems(JSON.parse(o.items));
    } catch {
      /* a damaged draft keeps the empty list */
    }
    draft.settle();
  };

  // The checklist warnings follow the medicines as they change.
  useEffect(() => {
    if (!items.length) {
      setWarnings([]);
      return;
    }
    const id = window.setTimeout(() => {
      rpc("prescriptions.check", { patient_id: patient.id, items: items.filter((i) => i.name.trim()) })
        .then(setWarnings)
        .catch(() => {});
    }, 250);
    return () => window.clearTimeout(id);
  }, [items]);

  const setLine = (n: number, patch: Partial<RxItem>) => {
    setItems((list) => list.map((x, i) => (i === n ? { ...x, ...patch } : x)));
    setServerErrors({});
  };
  const removeLine = (n: number) => setItems((list) => list.filter((_, i) => i !== n));
  const addDrug = (d: DrugInfo) => setItems((list) => [...list, fromDrug(d)]);
  const addFree = (name: string) => setItems((list) => [...list, { drug_id: null, name, form: "tablet", strength: null, quantity: null, dose: null, times_per_day: null, timing: null, days: null, as_needed: false, note: null }]);
  const applyTemplate = (tp: RxTemplateInfo) => {
    if (items.length && !window.confirm(t("rx.templateReplace"))) return;
    // Template lines point at the formulary; a medicine the clinic switched off still comes by name.
    setItems(tp.items.map((i) => ({ ...i })));
  };

  const solo = (doctors?.length ?? 0) <= 1;
  const lineErrs = items.map(lineErrors);
  const unacked = warnings.filter((w) => !ack.includes(w.key));
  const invalid = !items.length || lineErrs.some((e) => Object.keys(e).length) || (!solo && !doctorId);
  const doctor = doctors?.find((d) => d.id === doctorId);

  const issue = async () => {
    setSubmitted(true);
    setError("");
    if (invalid || unacked.length) {
      window.setTimeout(() => root.current?.querySelector<HTMLElement>('[aria-invalid="true"], .rx-warning:not(.acked)')?.scrollIntoView({ block: "center", behavior: "smooth" }), 0);
      return;
    }
    setBusy(true);
    try {
      const doc = await rpc("documents.issue", {
        patient_id: patient.id,
        doctor_id: doctorId || doctors?.[0]?.id || null,
        appointment_id: null,
        language,
        paper,
        content: { kind: "prescription", items, notes: notes.trim() || null, acknowledged: ack },
      });
      draft.clear();
      onIssued(doc);
    } catch (x) {
      if (isSessionError(x)) return;
      if (x instanceof ApiError && x.field) {
        setServerErrors({ [x.field]: x.rule ? `rule.${x.rule}` : `error.${x.code}` });
        if (x.rule === "rx_warning_not_acknowledged") rpc("prescriptions.check", { patient_id: patient.id, items }).then(setWarnings).catch(() => {});
      } else setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  const fieldErr = (n: number, k: keyof RxItem) => {
    const e = serverErrors[`items.${n}.${k}`] ?? ((submitted || k !== "name") && lineErrs[n]?.[k]);
    return e ? t(e) : null;
  };

  const previewDoc: DocumentInfo = {
    id: "draft", number: t("rx.draftNumber"), kind: "prescription", patient_id: patient.id, patient: docPatient(patient), doctor_id: doctor?.id ?? null,
    doctor: doctor ? { name: doctor.full_name, license_number: doctor.license_number, specialties: [] } : null,
    appointment_id: null, language, paper, content: { kind: "prescription", items: items.filter((i) => i.name.trim()), notes: notes.trim() || null, acknowledged: [] },
    issued_at: new Date().toISOString(), issued_by_name: null, print_count: 0, last_printed_at: null, attachment_id: null, status: "issued", void_reason: null, version: 0,
  };

  return (
    <Dialog
      title={t("rx.new")}
      description={patient.full_name}
      onClose={onClose}
      wide
      testId="rx-editor"
      footer={
        <>
          {submitted && unacked.length > 0 && <span className="field-error" data-testid="rx-unacked">{t("rx.ackFirst")}</span>}
          <span className="grow" />
          <Button onClick={() => { draft.clear(); onClose(); }} data-testid="rx-cancel">{t("common.cancel")}</Button>
          <Button variant="primary" icon={FileCheck2} loading={busy} onClick={issue} data-testid="rx-issue">{t("rx.issue")}</Button>
        </>
      }
    >
      <div className="editor-layout" ref={root}>
        <div className="stack editor-main">
          <MedicalAlertBanner patientId={patient.id} hideReview />
          {draft.offer && (
            <Notice tone="info" title={t("draft.found.title")} testId="draft-offer">
              <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap" }}>
                <span>{t("draft.found.hint")}</span>
                <span className="row">
                  <Button size="sm" variant="primary" onClick={restore} data-testid="draft-restore">{t("draft.found.restore")}</Button>
                  <Button size="sm" onClick={() => { draft.clear(); draft.settle(); }} data-testid="draft-discard">{t("draft.found.discard")}</Button>
                </span>
              </div>
            </Notice>
          )}
          {error && <Notice tone="danger" testId="rx-error">{error}</Notice>}
          <div className="grid-2">
            {!solo && (
              <Field label={t("rx.doctor")} error={(submitted && !doctorId) || serverErrors.doctor_id ? t(serverErrors.doctor_id ?? "rule.doctor_required") : null}>
                <Select value={doctorId} onChange={(e) => setDoctorId(e.target.value)} data-testid="rx-doctor">
                  <option value="">—</option>
                  {doctors?.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
                </Select>
              </Field>
            )}
            <LanguagePick value={language} onChange={setLanguage} hint={t("rx.languageHint")} testId="rx-language" />
            <div className="field">
              <span className="field-label">{t("print.paper")}</span>
              <Segmented<Paper> value={paper} onChange={setPaper} label={t("print.paper")} options={[{ value: "a5", label: t("print.paper.a5"), testId: "rx-paper-a5" }, { value: "a4", label: t("print.paper.a4"), testId: "rx-paper-a4" }]} />
            </div>
          </div>

          {templates.length > 0 && (
            <div className="field">
              <span className="field-label row" style={{ gap: 6 }}><Wand2 size={15} aria-hidden />{t("rx.templates")}</span>
              <div className="chip-group">
                {templates.map((tp) => <button key={tp.id} type="button" className="chip-toggle" onClick={() => applyTemplate(tp)} data-testid={`rx-template-${tp.code}`}>{tr(tp.name, uiLang)}</button>)}
              </div>
            </div>
          )}

          <DrugSearch drugs={drugs} onPick={addDrug} onFree={addFree} />

          {items.length === 0 && <div className="rx-empty"><Pill aria-hidden /><span>{t("rx.empty")}</span></div>}
          <ol className="rx-lines" data-testid="rx-lines">
            {items.map((i, n) => {
              const mine = warnings.filter((w) => w.item_index === n);
              return (
                <li key={n} className="rx-line" data-testid={`rx-line-${n}`}>
                  <div className="rx-line-head">
                    <span className="rx-line-no">{digits(n + 1, uiLang)}</span>
                    <Field label={t("rx.medicine")} error={fieldErr(n, "name")} className="grow">
                      <TextInput dir="ltr" value={i.name} onChange={(e) => setLine(n, { name: e.target.value, drug_id: e.target.value === i.name ? i.drug_id : null })} data-testid={`rx-line-${n}-name`} />
                    </Field>
                    <Field label={t("rx.strength")} optional error={fieldErr(n, "strength")}>
                      <TextInput dir="ltr" value={i.strength ?? ""} onChange={(e) => setLine(n, { strength: e.target.value || null })} data-testid={`rx-line-${n}-strength`} />
                    </Field>
                    <IconButton icon={Trash2} label={t("rx.remove")} onClick={() => removeLine(n)} data-testid={`rx-line-${n}-remove`} />
                  </div>
                  <div className="rx-line-grid">
                    <Field label={t("rx.formLabel")}>
                      <Select value={i.form} onChange={(e) => setLine(n, { form: e.target.value })} data-testid={`rx-line-${n}-form`}>
                        {FORMS.map((f) => <option key={f} value={f}>{t(`rx.form.${f}`)}</option>)}
                      </Select>
                    </Field>
                    <Field label={t("rx.dose")} optional error={fieldErr(n, "dose")}>
                      <TextInput value={i.dose ?? ""} onChange={(e) => setLine(n, { dose: e.target.value || null })} data-testid={`rx-line-${n}-dose`} />
                    </Field>
                    <Field label={t("rx.timesPerDay")} optional error={fieldErr(n, "times_per_day")}>
                      <TextInput inputMode="numeric" value={i.times_per_day == null ? "" : digits(i.times_per_day, uiLang)} onChange={(e) => setLine(n, { times_per_day: e.target.value.trim() ? Number(latinDigits(e.target.value)) || 0 : null })} data-testid={`rx-line-${n}-times`} />
                    </Field>
                    <Field label={t("rx.timing")} optional>
                      <Select value={i.timing ?? ""} onChange={(e) => setLine(n, { timing: (e.target.value || null) as RxTiming | null })} data-testid={`rx-line-${n}-timing`}>
                        <option value="">—</option>
                        {TIMINGS.map((x) => <option key={x} value={x}>{t(`rx.timing.${x}`)}</option>)}
                      </Select>
                    </Field>
                    <Field label={t("rx.days")} optional error={fieldErr(n, "days")}>
                      <TextInput inputMode="numeric" suffix={t("rx.daysUnit")} value={i.days == null ? "" : digits(i.days, uiLang)} onChange={(e) => setLine(n, { days: e.target.value.trim() ? Number(latinDigits(e.target.value)) || 0 : null })} data-testid={`rx-line-${n}-days`} />
                    </Field>
                    <Field label={t("rx.quantity")} optional error={fieldErr(n, "quantity")}>
                      <TextInput value={i.quantity ?? ""} onChange={(e) => setLine(n, { quantity: e.target.value || null })} data-testid={`rx-line-${n}-quantity`} />
                    </Field>
                  </div>
                  <div className="row rx-line-foot">
                    <Checkbox checked={i.as_needed} onChange={(v) => setLine(n, { as_needed: v })} testId={`rx-line-${n}-prn`}>{t("rx.asNeeded")}</Checkbox>
                    <TextInput className="grow" placeholder={t("rx.notePlaceholder")} value={i.note ?? ""} onChange={(e) => setLine(n, { note: e.target.value || null })} data-testid={`rx-line-${n}-note`} />
                  </div>
                  {rxInstructions(i, language) && <div className="rx-line-sentence" lang={language} dir={language === "en" ? "ltr" : "rtl"} data-testid={`rx-line-${n}-sentence`}>{rxInstructions(i, language)}</div>}
                  {mine.map((w) => (
                    <div key={w.key} className={`rx-warning ${w.severity} ${ack.includes(w.key) ? "acked" : ""}`} role="alert" data-testid={`rx-warning-${w.key}`}>
                      <TriangleAlert aria-hidden />
                      <div className="grow">
                        <div className="rx-warning-text">{t(`rxw.${w.rule}`).replace("{drug}", i.name).replace("{detail}", w.detail ?? "")}</div>
                        <Checkbox checked={ack.includes(w.key)} onChange={(v) => setAck((a) => (v ? [...a, w.key] : a.filter((x) => x !== w.key)))} testId={`rx-ack-${w.key}`}>{t("rx.ack")}</Checkbox>
                      </div>
                    </div>
                  ))}
                </li>
              );
            })}
          </ol>
          <Field label={t("rx.notes")} hint={t("rx.notesHint")} optional>
            <Textarea rows={2} value={notes} onChange={(e) => setNotes(e.target.value)} data-testid="rx-notes" />
          </Field>
        </div>
        <LivePreview paper={paper}>
          <DocumentSheet doc={previewDoc} letterhead={letterhead} calendar={calendar} />
        </LivePreview>
      </div>
    </Dialog>
  );
}

/** Find a medicine of the formulary by name; Enter on an unknown name adds it as typed. */
function DrugSearch({ drugs, onPick, onFree }: { drugs: DrugInfo[]; onPick: (d: DrugInfo) => void; onFree: (name: string) => void }) {
  const { t } = useI18n();
  const [q, setQ] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const query = q.trim().toLowerCase();
  const hits = query ? drugs.filter((d) => `${d.name} ${d.strength ?? ""}`.toLowerCase().includes(query)).slice(0, 8) : drugs.slice(0, 8);
  const choose = (d: DrugInfo | null) => {
    if (d) onPick(d);
    else if (q.trim()) onFree(q.trim());
    setQ("");
    setOpen(false);
    setActive(0);
  };
  return (
    <div className="combo" data-testid="rx-drug-search">
      <TextInput
        icon={Search}
        dir="ltr"
        placeholder={t("rx.searchPlaceholder")}
        value={q}
        onFocus={() => setOpen(true)}
        onBlur={() => window.setTimeout(() => setOpen(false), 150)}
        onChange={(e) => { setQ(e.target.value); setOpen(true); setActive(0); }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => Math.min(a + 1, hits.length)); }
          if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => Math.max(a - 1, 0)); }
          if (e.key === "Enter") { e.preventDefault(); choose(hits[active] ?? null); }
          if (e.key === "Escape") setOpen(false);
        }}
        data-testid="rx-drug-input"
        aria-autocomplete="list"
      />
      {open && (hits.length > 0 || q.trim()) && (
        <ul className="combo-list" role="listbox">
          {hits.map((d, i) => (
            <li key={d.id} role="option" aria-selected={i === active} onMouseDown={(e) => { e.preventDefault(); choose(d); }} data-testid={`rx-drug-${d.code}`}>
              <Pill aria-hidden />
              <bdi dir="ltr" className="cell-strong">{d.name}</bdi>
              <bdi dir="ltr" className="subtle">{d.strength}</bdi>
              <span className="subtle t-caption">{t(`rx.form.${d.form}`)}</span>
            </li>
          ))}
          {q.trim() && !hits.some((d) => d.name.toLowerCase() === query) && (
            <li role="option" aria-selected={active === hits.length} onMouseDown={(e) => { e.preventDefault(); choose(null); }} data-testid="rx-drug-free">
              <Plus aria-hidden />
              <span>{t("rx.addTyped").replace("{name}", q.trim())}</span>
            </li>
          )}
        </ul>
      )}
    </div>
  );
}
