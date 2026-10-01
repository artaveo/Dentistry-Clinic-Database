import { useState } from "react";
import type { Language } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { formatDateTime } from "../lib/dates";
import { useI18n } from "../i18n";
import { LangSwitch } from "./LangSwitch";

/** First-run wizard: clinic → owner → Recovery Key (shown once, must be confirmed). */
export function Setup({ onDone }: { onDone: () => void }) {
  const { t, err, lang } = useI18n();
  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [clinic, setClinic] = useState("");
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [repeat, setRepeat] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [confirmed, setConfirmed] = useState(false);

  const create = async () => {
    setError("");
    if (password !== repeat) return setError(t("error.mismatch"));
    setBusy(true);
    try {
      const r = await rpc("app.setup", {
        clinic_name: clinic,
        owner_username: username,
        owner_display_name: displayName || username,
        owner_password: password,
        language: lang as Language,
      });
      setRecoveryKey(r.recovery_key);
      setStep(3);
    } catch (e) {
      setError(err(e));
    } finally {
      setBusy(false);
    }
  };

  if (step === 3) {
    return (
      <div className="center">
        <div className="card wide">
          <h1>{t("recovery.title")}</h1>
          <p className="warn">{t("recovery.warning")}</p>
          <div className="key mono" data-testid="recovery-key">{recoveryKey}</div>
          <div className="actions">
            <button onClick={() => window.print()}>{t("recovery.print")}</button>
          </div>
          <label className="row">
            <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} data-testid="recovery-confirm" />
            {t("recovery.confirm")}
          </label>
          <div className="actions">
            <button className="primary" disabled={!confirmed} onClick={onDone} data-testid="recovery-continue">{t("recovery.continue")}</button>
          </div>
        </div>
        <div className="print-only">
          <h1>{t("recovery.sheetTitle")}</h1>
          <p>{t("recovery.sheetClinic")}: {clinic}</p>
          <p>{t("recovery.sheetDate")}: {formatDateTime(new Date().toISOString(), lang)}</p>
          <div className="key mono">{recoveryKey}</div>
          <p>{t("recovery.warning")}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="center">
      <div className="card">
        <LangSwitch />
        <h1>{t("setup.title")}</h1>
        <p className="muted">{t("setup.intro")}</p>
        {step === 1 ? (
          <>
            <label htmlFor="clinic">{t("setup.clinicName")}</label>
            <input id="clinic" value={clinic} onChange={(e) => setClinic(e.target.value)} data-testid="setup-clinic" />
            <div className="actions">
              <button className="primary" disabled={!clinic.trim()} onClick={() => setStep(2)} data-testid="setup-next">{t("setup.next")}</button>
            </div>
          </>
        ) : (
          <>
            <h2>{t("setup.owner")}</h2>
            <label htmlFor="u">{t("login.username")}</label>
            <input id="u" className="ltr" autoComplete="off" value={username} onChange={(e) => setUsername(e.target.value)} data-testid="setup-username" />
            <label htmlFor="d">{t("login.displayName")}</label>
            <input id="d" value={displayName} onChange={(e) => setDisplayName(e.target.value)} data-testid="setup-display" />
            <label htmlFor="p">{t("login.password")}</label>
            <input id="p" type="password" value={password} onChange={(e) => setPassword(e.target.value)} data-testid="setup-password" />
            <label htmlFor="r">{t("login.passwordRepeat")}</label>
            <input id="r" type="password" value={repeat} onChange={(e) => setRepeat(e.target.value)} data-testid="setup-repeat" />
            {error && <div className="error" role="alert">{error}</div>}
            <div className="actions">
              <button onClick={() => setStep(1)}>{t("setup.back")}</button>
              <button className="primary" disabled={busy || !username || !password} onClick={create} data-testid="setup-create">
                {busy ? t("setup.creating") : t("setup.create")}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
