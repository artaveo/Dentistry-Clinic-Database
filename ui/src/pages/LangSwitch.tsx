import type { Language } from "../../../shared/ts/contract";
import { useI18n } from "../i18n";

export function LangSwitch() {
  const { lang, setLang, t } = useI18n();
  return (
    <div className="lang-switch">
      {(["fa", "ps", "en"] as Language[]).map((l) => (
        <button key={l} aria-pressed={lang === l} onClick={() => setLang(l)} data-testid={`lang-${l}`}>
          {t(`lang.${l}`)}
        </button>
      ))}
    </div>
  );
}
