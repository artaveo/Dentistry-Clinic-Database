import { useEffect, useMemo, useRef, useState } from "react";
import { CheckCheck, ClipboardCheck, FileHeart, RotateCcw, Save, TriangleAlert } from "lucide-react";
import type { MedicalAnswer, MedicalAnswerValue, MedicalHistoryInfo, MedicalQuestionInfo } from "../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../lib/api";
import { digits, formatDate, latinDigits } from "../../lib/dates";
import { useFormDraft } from "../../lib/draft";
import { decodeAnswer, encodeAnswer, fromAnswer, tr, useMedicalQuestions, type AnswerParts } from "../../lib/medical";
import { focusFirstInvalid, useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Field, Textarea, TextInput } from "../../ui/Field";
import { Badge, ErrorState, Loading, Notice } from "../../ui/Feedback";
import { useToast } from "../../ui/Toast";

/**
 * Structured medical history (M1, OF-025): a yes / no / unknown checklist grouped like a paper form,
 * a detail under some "yes" answers, and «سایر توضیحات» at the end. Saving also records that the
 * history was reviewed; "reviewed, nothing changed" does the same without editing.
 */
export function MedicalHistoryTab({ patientId, female, canEdit, onSaved }: { patientId: string; female: boolean; canEdit: boolean; onSaved?: () => void }) {
  const { err } = useI18n();
  const questions = useMedicalQuestions();
  const [history, setHistory] = useState<MedicalHistoryInfo | null>(null);
  const [error, setError] = useState("");

  const load = () =>
    rpc("medical_history.get", { patient_id: patientId })
      .then((h) => {
        setHistory(h);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patientId]);

  if (error && !history) return <ErrorState message={error} onRetry={load} />;
  if (!history || !questions) return <Loading />;
  return (
    <ChecklistForm
      key={history.version}
      patientId={patientId}
      history={history}
      questions={questions}
      female={female}
      canEdit={canEdit}
      onSaved={(h) => {
        setHistory(h);
        onSaved?.();
      }}
    />
  );
}

const key = (q: MedicalQuestionInfo) => `q:${q.id}`;
type Values = Record<string, string>;

function ChecklistForm({ patientId, history, questions, female, canEdit, onSaved }: { patientId: string; history: MedicalHistoryInfo; questions: MedicalQuestionInfo[]; female: boolean; canEdit: boolean; onSaved: (h: MedicalHistoryInfo) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const root = useRef<HTMLDivElement>(null);
  const [busy, setBusy] = useState<"" | "save" | "review">("");
  const [error, setError] = useState("");

  const answered = useMemo(() => new Map(history.answers.map((a) => [a.question_id, a])), [history]);
  // Switched-off questions stay visible while the patient still has an answer to them; women's questions only for women.
  const shown = questions.filter((q) => (q.is_active || answered.has(q.id)) && (!q.female_only || female || answered.has(q.id)));
  const initial = useMemo(() => {
    const v: Values = { notes: history.notes ?? "" };
    for (const q of shown) v[key(q)] = answered.has(q.id) ? fromAnswer(answered.get(q.id)!) : "";
    return v;
  }, [history]);

  const checks: Partial<Record<string, (value: string) => string | null>> = { notes: (x) => (x.trim().length > 4000 ? "rule.medical_detail_length" : null) };
  for (const q of shown) {
    checks[key(q)] = (raw) => {
      const a = decodeAnswer(raw);
      if (a.answer !== "yes") return null;
      if (q.detail_kind === "months" && a.detail.trim()) {
        const n = Number(latinDigits(a.detail.trim()));
        if (!Number.isInteger(n) || n < 1 || n > 10) return "rule.medical_months_range";
      }
      if (a.detail.trim().length > 300) return "rule.medical_detail_length";
      return null;
    };
  }
  const form = useForm<Values>(initial, checks);
  const draft = useFormDraft<Values>(`medical_history.${patientId}`, form.values, canEdit);

  const setAnswer = (q: MedicalQuestionInfo, next: Partial<AnswerParts>) => {
    const cur = decodeAnswer(form.values[key(q)]);
    form.set(key(q), encodeAnswer({ ...cur, ...next }));
  };
  const unanswered = shown.filter((q) => !decodeAnswer(form.values[key(q)]).answer);
  const yesCount = shown.filter((q) => decodeAnswer(form.values[key(q)]).answer === "yes").length;

  const save = async () => {
    setError("");
    if (!form.validate()) {
      window.setTimeout(() => focusFirstInvalid(root.current), 0);
      return;
    }
    setBusy("save");
    try {
      const answers: MedicalAnswer[] = shown
        .map((q) => ({ q, a: decodeAnswer(form.values[key(q)]) }))
        .filter(({ a }) => !!a.answer)
        .map(({ q, a }) => ({ question_id: q.id, answer: a.answer as MedicalAnswerValue, detail_text: a.detail.trim() || null, detail_choice: a.choice || null }));
      const saved = await rpc("medical_history.update", { patient_id: patientId, version: history.version, answers, notes: form.values.notes.trim() || null });
      draft.clear();
      toast.success(t("medicalHistory.saved"));
      onSaved(saved);
    } catch (x) {
      if (isSessionError(x)) return;
      const map = Object.fromEntries(shown.map((q) => [`answers.${q.id}`, key(q)]));
      if (form.serverError(x, map)) window.setTimeout(() => focusFirstInvalid(root.current), 0);
      else setError(x instanceof ApiError && x.code === "conflict" ? t("medicalHistory.conflict") : err(x));
    } finally {
      setBusy("");
    }
  };

  const review = async () => {
    setBusy("review");
    try {
      const h = await rpc("medical_history.review", { patient_id: patientId, version: history.version });
      toast.success(t("medicalHistory.reviewed"));
      onSaved(h);
    } catch (x) {
      if (!isSessionError(x)) setError(err(x));
    } finally {
      setBusy("");
    }
  };

  // Groups in checklist order, each with its questions.
  const groups: { code: string; items: MedicalQuestionInfo[] }[] = [];
  for (const q of shown) {
    const g = groups.find((x) => x.code === q.group_code);
    if (g) g.items.push(q);
    else groups.push({ code: q.group_code, items: [q] });
  }

  return (
    <Card>
      <CardHeader
        icon={FileHeart}
        title={t("medicalHistory.title")}
        description={t("medicalHistory.subtitle")}
        actions={
          history.reviewed_at ? (
            <span className="subtle t-caption" data-testid="mh-reviewed">
              {t("medicalHistory.lastReview").replace("{date}", formatDate(history.reviewed_at, lang)).replace("{name}", history.reviewed_by_name ?? "—")}
            </span>
          ) : (
            <Badge tone="warning" dot>{t("medicalHistory.never")}</Badge>
          )
        }
      />
      <div className="stack" ref={root} data-testid="medical-history-form">
        {draft.offer && (
          <Notice tone="info" title={t("draft.found.title")} testId="draft-offer">
            <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap" }}>
              <span>{t("draft.found.hint")}</span>
              <span className="row">
                <Button size="sm" variant="primary" onClick={() => { form.reset({ ...initial, ...draft.offer }); draft.settle(); }} data-testid="draft-restore">{t("draft.found.restore")}</Button>
                <Button size="sm" onClick={() => { draft.clear(); draft.settle(); }} data-testid="draft-discard">{t("draft.found.discard")}</Button>
              </span>
            </div>
          </Notice>
        )}
        {history.version > 0 && history.review_due && (
          <Notice tone="warning" title={t("medicalHistory.reviewDue")} testId="mh-review-due">
            <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap" }}>
              <span>{t("medicalHistory.reviewDueHint")}</span>
              {canEdit && <Button size="sm" icon={ClipboardCheck} loading={busy === "review"} onClick={review} data-testid="mh-review">{t("medicalHistory.reviewNoChange")}</Button>}
            </div>
          </Notice>
        )}
        {error && <Notice tone="danger" testId="mh-error">{error}</Notice>}

        <div className="mh-toolbar">
          <span className="subtle t-caption" data-testid="mh-progress">
            {t("medicalHistory.progress").replace("{done}", digits(shown.length - unanswered.length, lang)).replace("{all}", digits(shown.length, lang)).replace("{yes}", digits(yesCount, lang))}
          </span>
          {canEdit && unanswered.length > 0 && (
            <Button size="sm" variant="subtle" icon={CheckCheck} onClick={() => unanswered.forEach((q) => setAnswer(q, { answer: "no" }))} data-testid="mh-rest-no">
              {t("medicalHistory.restNo")}
            </Button>
          )}
        </div>

        <div className="mh-groups">
          {groups.map((g) => (
            <section key={g.code} className="mh-group" data-testid={`mh-group-${g.code}`}>
              <h3 className="mh-group-title">{t(`mh.group.${g.code}`)}</h3>
              {g.items.map((q) => (
                <QuestionRow key={q.id} q={q} value={decodeAnswer(form.values[key(q)])} error={form.error(key(q))} disabled={!canEdit} onChange={(next) => setAnswer(q, next)} />
              ))}
            </section>
          ))}
        </div>

        <Field label={t("medicalHistory.notes")} hint={t("medicalHistory.notesHint")} error={form.error("notes") && t(form.error("notes")!)} optional>
          <Textarea value={form.values.notes} onChange={(e) => form.set("notes", e.target.value)} disabled={!canEdit} rows={3} data-testid="mh-notes" />
        </Field>

        {form.invalidCount > 0 && <Notice tone="danger" testId="form-error-summary">{t("form.errorsSummary").replace("{n}", digits(form.invalidCount, lang))}</Notice>}
        {canEdit && (
          <div className="row" style={{ justifyContent: "flex-end", flexWrap: "wrap" }}>
            <Button icon={RotateCcw} onClick={() => { form.reset(initial); draft.clear(); }} data-testid="mh-reset">{t("medicalHistory.undo")}</Button>
            <Button variant="primary" icon={Save} loading={busy === "save"} onClick={save} data-testid="mh-save">{t("common.save")}</Button>
          </div>
        )}
      </div>
    </Card>
  );
}

const VALUES: MedicalAnswerValue[] = ["yes", "no", "unknown"];

function QuestionRow({ q, value, error, disabled, onChange }: { q: MedicalQuestionInfo; value: AnswerParts; error: string | null; disabled: boolean; onChange: (next: Partial<AnswerParts>) => void }) {
  const { t, lang } = useI18n();
  const yes = value.answer === "yes";
  const detailLabel = tr(q.detail_label, lang);
  return (
    <div className={`mh-row ${yes ? "is-yes" : ""} ${yes && q.alert ? "is-alert" : ""}`} data-testid={`mh-q-${q.code}`} data-answer={value.answer || "none"}>
      <div className="mh-row-main">
        <span className="mh-label">
          {q.alert && <TriangleAlert className="mh-alert-icon" aria-label={t("medicalHistory.alertQuestion")} />}
          <span>{tr(q.label, lang)}</span>
        </span>
        <div className="tri" role="radiogroup" aria-label={tr(q.label, lang)}>
          {VALUES.map((v) => (
            <button
              key={v}
              type="button"
              role="radio"
              aria-checked={value.answer === v}
              className={`tri-${v}`}
              disabled={disabled}
              // Pressing the chosen answer again clears it back to "not asked".
              onClick={() => onChange(value.answer === v ? { answer: "", detail: "", choice: "" } : { answer: v })}
              data-testid={`mh-${q.code}-${v}`}
            >
              {t(`medicalHistory.answer.${v}`)}
            </button>
          ))}
        </div>
      </div>
      {yes && q.detail_kind !== "none" && (
        <div className="mh-detail">
          {(q.detail_kind === "text" || q.detail_kind === "text_choice") && (
            <Field label={detailLabel} error={error && t(error)} optional>
              <TextInput value={value.detail} onChange={(e) => onChange({ detail: e.target.value })} disabled={disabled} data-testid={`mh-${q.code}-detail`} />
            </Field>
          )}
          {q.detail_kind === "months" && (
            <Field label={detailLabel} hint={t("medicalHistory.monthsHint")} error={error && t(error)} optional>
              <TextInput inputMode="numeric" suffix={t("medicalHistory.months")} value={value.detail} onChange={(e) => onChange({ detail: e.target.value })} disabled={disabled} data-testid={`mh-${q.code}-detail`} />
            </Field>
          )}
          {(q.detail_kind === "choice" || q.detail_kind === "text_choice") && (
            <div className="field">
              <span className="field-label">{q.detail_kind === "choice" ? detailLabel : t("medicalHistory.route")}</span>
              <div className="tri" role="radiogroup" aria-label={detailLabel}>
                {q.choices.map((c) => (
                  <button key={c} type="button" role="radio" aria-checked={value.choice === c} disabled={disabled} onClick={() => onChange({ choice: value.choice === c ? "" : c })} data-testid={`mh-${q.code}-choice-${c}`}>
                    {t(`mh.choice.${c}`)}
                  </button>
                ))}
              </div>
            </div>
          )}
          {q.alert_note && <p className="mh-note">{tr(q.alert_note, lang)}</p>}
        </div>
      )}
    </div>
  );
}
