import { useEffect, useState } from "react";
import type { Language, MedicalAnswer, MedicalAnswerValue, MedicalQuestionInfo, Translations } from "../../../shared/ts/contract";
import { rpc } from "./api";

/** The text of a clinic-editable label in one language (the Core already filled empty languages). */
export function tr(t: Translations | null | undefined, lang: Language): string {
  if (!t) return "";
  return t[lang] || t.fa || t.en || t.ps;
}

let cache: Promise<MedicalQuestionInfo[]> | null = null;

/** Forgets the cached checklist (after the clinic changed its questions). */
export function invalidateMedicalQuestions() {
  cache = null;
}

/**
 * Every checklist question (M1), switched-off ones included — a patient may have answered one before it
 * was switched off, and that answer (an allergy, say) must still show. Fetched once per session.
 */
export function useMedicalQuestions(): MedicalQuestionInfo[] | null {
  const [qs, setQs] = useState<MedicalQuestionInfo[] | null>(null);
  useEffect(() => {
    let live = true;
    cache ??= rpc("medical_questions.list", { include_inactive: true }).catch((e) => {
      cache = null;
      throw e;
    });
    cache.then((v) => live && setQs(v)).catch(() => live && setQs([]));
    return () => {
      live = false;
    };
  }, []);
  return qs;
}

/**
 * One answer as a single form string (the form-draft store keeps strings, rule 14):
 * "" = not asked; otherwise `answer␟detail␟choice`.
 */
const SEP = "\u001f";
export type AnswerParts = { answer: MedicalAnswerValue | ""; detail: string; choice: string };

export function encodeAnswer(a: AnswerParts): string {
  return a.answer ? [a.answer, a.detail, a.choice].join(SEP) : "";
}

export function decodeAnswer(s: string | undefined): AnswerParts {
  if (!s) return { answer: "", detail: "", choice: "" };
  const [answer, detail = "", choice = ""] = s.split(SEP);
  return { answer: answer as MedicalAnswerValue, detail, choice };
}

export function fromAnswer(a: MedicalAnswer): string {
  return encodeAnswer({ answer: a.answer, detail: a.detail_text ?? "", choice: a.detail_choice ?? "" });
}

/** The "yes" answers to alert questions, with their question, in checklist order (the alert banner). */
export function alertsOf(questions: MedicalQuestionInfo[], answers: MedicalAnswer[]): { q: MedicalQuestionInfo; a: MedicalAnswer }[] {
  const byId = new Map(answers.map((a) => [a.question_id, a]));
  return questions.filter((q) => q.alert && byId.get(q.id)?.answer === "yes").map((q) => ({ q, a: byId.get(q.id)! }));
}

/** A checklist question as a short warning: «حامله هستید؟» reads «حامله», with its detail («۵ ماه») after it. */
export function alertLabel(label: string): string {
  return label.replace(/\s*(هستید|هستی|یاست|ئۍ|are you|do you have)?\s*[؟?]\s*$/i, "").trim() || label;
}

/** One alert as text: label plus detail (choice/months), the same everywhere (queue, calendar, profile). */
export function alertText(a: { q: MedicalQuestionInfo; a: MedicalAnswer }, lang: Language, t: (k: string) => string, num: (s: string | number) => string): string {
  const detail = [a.a.detail_text && (a.q.detail_kind === "months" ? `${num(a.a.detail_text)} ${t("medicalHistory.months")}` : a.a.detail_text), a.a.detail_choice && t(`mh.choice.${a.a.detail_choice}`)].filter(Boolean);
  const label = alertLabel(tr(a.q.label, lang));
  return detail.length ? `${label} — ${detail.join("، ")}` : label;
}
