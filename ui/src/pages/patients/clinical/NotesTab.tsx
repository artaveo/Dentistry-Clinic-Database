import { useEffect, useMemo, useState } from "react";
import { FilePenLine, Lock, NotebookPen, PenLine, Plus, Signature, Trash2, Wand2 } from "lucide-react";
import type { CalendarSystem, ClinicalNoteInfo, ClinicProfile, NoteFields, NoteTemplateInfo, PatientInfo, TreatmentItemInfo } from "../../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../../lib/api";
import { formatDate, formatDateTime } from "../../../lib/dates";
import { useFormDraft } from "../../../lib/draft";
import { useCatalog, useClinicSettings, useNoteTemplates } from "../../../lib/clinical";
import { tr } from "../../../lib/medical";
import { todayIso } from "../../../lib/calendar";
import { useI18n } from "../../../i18n";
import { Button } from "../../../ui/Button";
import { Card, CardHeader } from "../../../ui/Card";
import { DateField } from "../../../ui/DateField";
import { Field, Select, Textarea } from "../../../ui/Field";
import { Badge, EmptyState, ErrorState, Loading, Notice } from "../../../ui/Feedback";
import { Dialog } from "../../../ui/Overlay";
import { useToast } from "../../../ui/Toast";
import { isSolo, useScheduling } from "../../reception/useScheduling";
import { itemTitle } from "./labels";

export const NOTE_FIELDS: (keyof NoteFields)[] = ["chief_complaint", "examination", "diagnosis", "findings", "procedure", "notes", "follow_up", "recommendations"];
const EMPTY: NoteFields = { chief_complaint: "", examination: "", diagnosis: "", findings: "", procedure: "", notes: "", follow_up: "", recommendations: "" };

/**
 * Patient profile → «یادداشت‌های بالینی» (5.6): one note per visit with its eight parts, written from a
 * ready-made note or from scratch, kept as a draft until the doctor signs it; a signed note is locked and
 * later changes are dated addenda.
 */
export function NotesTab({ patient, clinic, canEdit, calendar, initialItem, onChanged }: {
  patient: PatientInfo;
  clinic: ClinicProfile | null;
  canEdit: boolean;
  calendar: CalendarSystem;
  /** Open a new note for this treatment session straight away. */
  initialItem?: TreatmentItemInfo | null;
  onChanged?: () => void;
}) {
  const { t, err } = useI18n();
  const [notes, setNotes] = useState<ClinicalNoteInfo[] | null>(null);
  const [error, setError] = useState("");
  const [editing, setEditing] = useState<{ note?: ClinicalNoteInfo; item?: TreatmentItemInfo | null } | null>(initialItem ? { item: initialItem } : null);
  const load = () =>
    rpc("notes.list", { patient_id: patient.id })
      .then((n) => {
        setNotes(n);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patient.id]);
  const changed = () => {
    load();
    onChanged?.();
  };
  if (error && !notes) return <ErrorState message={error} onRetry={load} />;
  const action = canEdit ? <Button variant="primary" icon={Plus} onClick={() => setEditing({})} data-testid="note-new">{t("note.new")}</Button> : undefined;
  return (
    <div className="stack-lg" data-testid="notes-tab">
      <Card flush>
        <CardHeader icon={NotebookPen} title={t("patients.tab.clinicalNotes")} description={t("note.subtitle")} actions={notes?.length ? action : undefined} />
        {!notes ? <Loading /> : !notes.length && <EmptyState icon={NotebookPen} title={t("note.none")} action={action}>{t("note.noneHint")}</EmptyState>}
      </Card>
      {notes?.map((n) => <NoteCard key={n.id} note={n} canEdit={canEdit} calendar={calendar} onEdit={() => setEditing({ note: n })} onChanged={changed} />)}
      {editing && <NoteEditor patient={patient} clinic={clinic} calendar={calendar} note={editing.note} item={editing.item ?? null} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); changed(); }} />}
    </div>
  );
}

function NoteCard({ note, canEdit, calendar, onEdit, onChanged }: { note: ClinicalNoteInfo; canEdit: boolean; calendar: CalendarSystem; onEdit: () => void; onChanged: () => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [adding, setAdding] = useState(false);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const signed = note.status === "signed";
  const sign = async () => {
    if (!window.confirm(t("note.signConfirm"))) return;
    try {
      await rpc("notes.sign", { id: note.id, version: note.version });
      toast.success(t("note.signed"));
      onChanged();
    } catch (x) {
      if (!isSessionError(x)) toast.error(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    }
  };
  const remove = async () => {
    if (!window.confirm(t("note.deleteConfirm"))) return;
    try {
      await rpc("notes.delete", { id: note.id, version: note.version });
      toast.success(t("note.deleted"));
      onChanged();
    } catch (x) {
      if (!isSessionError(x)) toast.error(err(x));
    }
  };
  const addendum = async () => {
    if (!text.trim()) return;
    setBusy(true);
    try {
      await rpc("notes.add_addendum", { note_id: note.id, text: text.trim() });
      toast.success(t("note.addendumSaved"));
      setText("");
      setAdding(false);
      onChanged();
    } catch (x) {
      if (!isSessionError(x)) toast.error(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Card className={`note-card ${signed ? "signed" : "draft"}`} data-testid={`note-${note.id}`}>
      <header className="note-head">
        <div className="note-titles">
          <span className="t-title">{formatDate(`${note.visit_date}T06:30:00Z`, lang, calendar)}</span>
          <span className="subtle t-caption">{[note.doctor_name, note.created_by_name && t("note.writtenBy").replace("{name}", note.created_by_name)].filter(Boolean).join(" · ")}</span>
        </div>
        {signed ? (
          <Badge tone="success" icon={Lock} testId={`note-status-${note.id}`}>{t("note.statusSigned")}</Badge>
        ) : (
          <Badge tone="warning" icon={PenLine} testId={`note-status-${note.id}`}>{t("note.statusDraft")}</Badge>
        )}
      </header>
      <dl className="note-fields">
        {NOTE_FIELDS.filter((k) => note.fields[k].trim()).map((k) => (
          <div key={k} className="note-field">
            <dt>{t(`note.field.${k}`)}</dt>
            <dd>{note.fields[k]}</dd>
          </div>
        ))}
      </dl>
      {signed && <p className="subtle t-caption note-signed-line">{t("note.signedBy").replace("{name}", note.signed_by_name ?? "—").replace("{date}", formatDateTime(note.signed_at, lang, calendar))}</p>}
      {note.addenda.length > 0 && (
        <ol className="note-addenda" data-testid={`note-addenda-${note.id}`}>
          {note.addenda.map((a) => (
            <li key={a.id}>
              <span className="note-addendum-meta">{t("note.addendum")} · {formatDateTime(a.created_at, lang, calendar)}{a.created_by_name ? ` · ${a.created_by_name}` : ""}</span>
              <span>{a.text}</span>
            </li>
          ))}
        </ol>
      )}
      {canEdit && (
        <div className="row note-actions">
          {!signed && <Button size="sm" icon={FilePenLine} onClick={onEdit} data-testid={`note-edit-${note.id}`}>{t("common.edit")}</Button>}
          {!signed && <Button size="sm" variant="primary" icon={Signature} onClick={sign} data-testid={`note-sign-${note.id}`}>{t("note.sign")}</Button>}
          {signed && !adding && <Button size="sm" icon={Plus} onClick={() => setAdding(true)} data-testid={`note-addendum-${note.id}`}>{t("note.addAddendum")}</Button>}
          <span className="grow" />
          {!signed && <Button size="sm" variant="subtle" icon={Trash2} onClick={remove} data-testid={`note-delete-${note.id}`}>{t("note.deleteDraft")}</Button>}
        </div>
      )}
      {adding && (
        <div className="stack note-addendum-form">
          <Field label={t("note.addendum")} hint={t("note.addendumHint")}>
            <Textarea rows={2} value={text} maxLength={2000} onChange={(e) => setText(e.target.value)} data-testid="note-addendum-text" />
          </Field>
          <div className="row">
            <span className="grow" />
            <Button size="sm" onClick={() => setAdding(false)}>{t("common.cancel")}</Button>
            <Button size="sm" variant="primary" loading={busy} disabled={!text.trim()} onClick={addendum} data-testid="note-addendum-save">{t("note.saveAddendum")}</Button>
          </div>
        </div>
      )}
    </Card>
  );
}

/** Write or continue a note; kept as a form draft while unsaved (rule 14) and as a saved draft until signed. */
export function NoteEditor({ patient, clinic, calendar, note, item, onClose, onSaved }: {
  patient: PatientInfo;
  clinic: ClinicProfile | null;
  calendar: CalendarSystem;
  note?: ClinicalNoteInfo;
  item: TreatmentItemInfo | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const templates = useNoteTemplates();
  const catalog = useCatalog();
  const settings = useClinicSettings();
  const { doctors } = useScheduling();
  const solo = isSolo(clinic, doctors);
  const [openItems, setOpenItems] = useState<TreatmentItemInfo[]>([]);
  useEffect(() => {
    rpc("treatment_items.open", { patient_id: patient.id }).then(setOpenItems).catch(() => {});
  }, [patient.id]);
  const [values, setValues] = useState<Record<string, string>>(() => ({
    ...(note?.fields ?? EMPTY),
    visit_date: note?.visit_date ?? todayIso(),
    doctor_id: note?.doctor_id ?? "",
    item_id: note?.item_id ?? item?.id ?? "",
    template_id: note?.template_id ?? "",
  }));
  useEffect(() => {
    if (!values.doctor_id && doctors?.length) setValues((v) => ({ ...v, doctor_id: doctors[0].id }));
  }, [doctors]);
  const [busy, setBusy] = useState<"" | "save" | "sign">("");
  const [error, setError] = useState("");
  const [submitted, setSubmitted] = useState(false);
  // Rule 14: an unsaved note survives a lock, another page or a closed app.
  const draft = useFormDraft(`note.${patient.id}.${note?.id ?? "new"}`, values, true);
  const set = (k: string, v: string) => setValues((x) => ({ ...x, [k]: v }));
  const empty = NOTE_FIELDS.every((k) => !values[k]?.trim());
  const apply = (tp: NoteTemplateInfo) => {
    const body = tp.body[lang];
    const filled = NOTE_FIELDS.some((k) => values[k]?.trim());
    if (filled && !window.confirm(t("note.templateReplace"))) return;
    setValues((v) => ({ ...v, ...body, template_id: tp.id }));
  };
  const items = useMemo(() => (item && !openItems.some((i) => i.id === item.id) ? [item, ...openItems] : openItems), [openItems, item]);
  const save = async (sign: boolean) => {
    setSubmitted(true);
    setError("");
    if (empty) return;
    if (sign && !window.confirm(t("note.signConfirm"))) return;
    setBusy(sign ? "sign" : "save");
    try {
      const fields = Object.fromEntries(NOTE_FIELDS.map((k) => [k, values[k] ?? ""])) as NoteFields;
      await rpc("notes.save", {
        id: note?.id ?? null, version: note?.version ?? 0, patient_id: patient.id, appointment_id: note?.appointment_id ?? null, item_id: values.item_id || null,
        doctor_id: solo ? (doctors?.[0]?.id ?? null) : values.doctor_id || null, visit_date: values.visit_date, fields, template_id: values.template_id || null, sign,
      });
      draft.clear();
      toast.success(sign ? t("note.signed") : t("note.saved"));
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy("");
    }
  };
  return (
    <Dialog
      title={note ? t("note.edit") : t("note.new")}
      description={patient.full_name}
      onClose={onClose}
      wide
      testId="note-editor"
      footer={
        <>
          {submitted && empty && <span className="field-error" data-testid="note-empty">{t("rule.note_empty")}</span>}
          <span className="grow" />
          <Button onClick={() => { draft.clear(); onClose(); }} data-testid="note-cancel">{t("common.cancel")}</Button>
          <Button icon={FilePenLine} loading={busy === "save"} onClick={() => save(false)} data-testid="note-save">{t("note.saveDraft")}</Button>
          <Button variant="primary" icon={Signature} loading={busy === "sign"} onClick={() => save(true)} data-testid="note-save-sign">{t("note.saveSign")}</Button>
        </>
      }
    >
      <div className="stack">
        {draft.offer && (
          <Notice tone="info" title={t("draft.found.title")} testId="draft-offer">
            <span className="row">
              <span>{t("draft.found.hint")}</span>
              <Button size="sm" variant="primary" onClick={() => { setValues({ ...values, ...draft.offer! }); draft.settle(); }} data-testid="draft-restore">{t("draft.found.restore")}</Button>
              <Button size="sm" onClick={() => { draft.clear(); draft.settle(); }} data-testid="draft-discard">{t("draft.found.discard")}</Button>
            </span>
          </Notice>
        )}
        {error && <Notice tone="danger" testId="note-error">{error}</Notice>}
        {templates && templates.length > 0 && (
          <div className="note-templates">
            <span className="field-label"><Wand2 size={14} aria-hidden /> {t("note.templates")}</span>
            <div className="chip-group" role="group" aria-label={t("note.templates")}>
              {templates.map((tp) => (
                <button key={tp.id} type="button" className="chip-toggle" aria-pressed={values.template_id === tp.id} onClick={() => apply(tp)} data-testid={`note-template-${tp.code}`}>{tr(tp.name, lang)}</button>
              ))}
            </div>
          </div>
        )}
        <div className="grid-3">
          <Field label={t("note.visitDate")}>
            <DateField value={values.visit_date} onChange={(d) => set("visit_date", d)} calendar={calendar} label={t("note.visitDate")} testId="note-date" />
          </Field>
          {!solo && doctors && doctors.length > 1 && (
            <Field label={t("appt.doctor")}>
              <Select value={values.doctor_id} onChange={(e) => set("doctor_id", e.target.value)} data-testid="note-doctor">
                {doctors.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
              </Select>
            </Field>
          )}
          {items.length > 0 && (
            <Field label={t("note.treatment")} hint={t("note.treatmentHint")} optional>
              <Select value={values.item_id} onChange={(e) => set("item_id", e.target.value)} data-testid="note-item">
                <option value="">—</option>
                {items.map((i) => <option key={i.id} value={i.id}>{itemTitle(t, catalog, i, settings?.tooth_numbering ?? "fdi", lang)}</option>)}
              </Select>
            </Field>
          )}
        </div>
        <div className="note-grid">
          {NOTE_FIELDS.map((k) => (
            <Field key={k} label={t(`note.field.${k}`)} optional>
              <Textarea rows={k === "examination" || k === "procedure" ? 4 : 2} value={values[k] ?? ""} maxLength={4000} invalid={submitted && empty} onChange={(e) => set(k, e.target.value)} data-testid={`note-${k}`} />
            </Field>
          ))}
        </div>
        <Notice tone="info">{t("note.lockHint")}</Notice>
      </div>
    </Dialog>
  );
}
