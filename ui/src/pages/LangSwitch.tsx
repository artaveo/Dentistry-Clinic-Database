import type { Language } from "../../../shared/ts/contract";
import { useI18n } from "../i18n";

/** Compact language switch for auth screens and the sidebar. */
export function LangSwitch() {
  const { lang, setLang, t } = useI18n();
  return (
    <div className="segmented" role="radiogroup" aria-label={t("shell.language")}>
      {(["fa", "ps", "en"] as Language[]).map((l) => (
        <button key={l} type="button" role="radio" aria-checked={lang === l} aria-pressed={lang === l} onClick={() => setLang(l)} data-testid={`lang-${l}`} lang={l}>
          {t(`lang.${l}`)}
        </button>
      ))}
    </div>
  );
}
