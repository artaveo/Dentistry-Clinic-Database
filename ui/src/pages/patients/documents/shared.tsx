import type { ReactNode } from "react";
import { Eye } from "lucide-react";
import type { DocumentPatient, Language, Paper, PatientInfo } from "../../../../../shared/ts/contract";
import { ageOn } from "../../../lib/dates";
import { useI18n } from "../../../i18n";
import { Segmented } from "../../../ui/Controls";
import { Scaled } from "../../../print/DocumentPreview";

/** The patient as a document will print them (the Core freezes the same facts when it issues it). */
export function docPatient(p: PatientInfo, gender?: string | null): DocumentPatient {
  return {
    name: p.full_name,
    number: p.patient_number,
    father_name: p.father_name,
    gender: gender ?? null,
    age: p.date_of_birth ? ageOn(p.date_of_birth) : p.approximate_age,
    phone: p.phone,
  };
}

/** The document's language: the patient's own by default (5.9 "دستور مصرف به زبان بیمار"). */
export function LanguagePick({ value, onChange, hint, testId }: { value: Language; onChange: (l: Language) => void; hint?: string; testId?: string }) {
  const { t } = useI18n();
  return (
    <div className="field">
      <span className="field-label">{t("docs.language")}</span>
      <Segmented<Language>
        value={value}
        onChange={onChange}
        label={t("docs.language")}
        options={(["fa", "ps", "en"] as Language[]).map((l) => ({ value: l, label: t(`lang.${l}`), testId: testId && `${testId}-${l}` }))}
      />
      {hint && <span className="field-hint">{hint}</span>}
    </div>
  );
}

/** The document as it will print, beside its form, updating as the form changes. */
export function LivePreview({ paper, children }: { paper: Paper; children: ReactNode }) {
  const { t } = useI18n();
  return (
    <aside className="editor-preview" aria-label={t("docs.livePreview")}>
      <span className="editor-preview-label"><Eye size={14} aria-hidden />{t("docs.livePreview")}</span>
      <Scaled paper={paper}>{children}</Scaled>
    </aside>
  );
}
