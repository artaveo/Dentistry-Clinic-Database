import { useI18n } from "../i18n";

type Key = "color_primary" | "color_secondary" | "color_accent";

/** The clinic's three colours as clickable swatches with their hex value. */
export function ColorSwatches({ primary, secondary, accent, onChange }: { primary: string; secondary: string; accent: string; onChange: (k: Key, v: string) => void }) {
  const { t } = useI18n();
  const items: [Key, string, string, string][] = [
    ["color_primary", primary, t("wizard.branding.colorPrimary"), "color-primary"],
    ["color_secondary", secondary, t("wizard.branding.colorSecondary"), "color-secondary"],
    ["color_accent", accent, t("wizard.branding.colorAccent"), "color-accent"],
  ];
  return (
    <div className="field">
      <span className="field-label">{t("wizard.branding.preview")}</span>
      <div className="swatches">
        {items.map(([k, value, label, testId]) => (
          <label className="swatch" key={k}>
            <span className="chip" style={{ background: value }} aria-hidden />
            <span className="swatch-text">
              <span>{label}</span>
              <code>{value}</code>
            </span>
            <input type="color" value={value} onChange={(e) => onChange(k, e.target.value)} aria-label={label} data-testid={testId} />
          </label>
        ))}
      </div>
      <span className="field-hint">{t("hint.colors")}</span>
    </div>
  );
}
