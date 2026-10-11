import type { Translations } from "../../../shared/ts/contract";
import { useI18n } from "../i18n";
import { Field, Textarea, TextInput } from "./Field";

/**
 * One clinic-editable text in the three languages (a service name, a checklist question, a template).
 * One language is enough — the Core fills the others from it — so only the first box is required.
 */
export function TranslationsField({ label, hint, value, onChange, error, multiline, rows = 3, testId }: {
  label: string;
  hint?: string;
  value: Translations;
  onChange: (t: Translations) => void;
  error?: string | null;
  multiline?: boolean;
  rows?: number;
  testId: string;
}) {
  const { t } = useI18n();
  const box = (k: keyof Translations) => {
    const props = { value: value[k], dir: k === "en" ? "ltr" : "rtl", onChange: (e: { target: { value: string } }) => onChange({ ...value, [k]: e.target.value }), "data-testid": `${testId}-${k}` } as const;
    return multiline ? <Textarea rows={rows} {...props} /> : <TextInput {...props} />;
  };
  return (
    <fieldset className="translations">
      <legend className="field-label">{label}</legend>
      <div className={multiline ? "stack" : "grid-3"}>
        <Field label={t("lang.fa")} error={error ?? null}>{box("fa")}</Field>
        <Field label={t("lang.ps")} optional>{box("ps")}</Field>
        <Field label={t("lang.en")} optional>{box("en")}</Field>
      </div>
      {hint && <span className="field-hint">{hint}</span>}
    </fieldset>
  );
}
