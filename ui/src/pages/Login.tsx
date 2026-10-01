import { useState } from "react";
import type { SessionInfo } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { useI18n } from "../i18n";
import { LangSwitch } from "./LangSwitch";

export function Login({ clinicName, onLogin }: { clinicName: string | null; onLogin: (s: SessionInfo) => void }) {
  const { t, err } = useI18n();
  const [mode, setMode] = useState<"login" | "recover">("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [key, setKey] = useState("");
  const [error, setError] = useState("");
  const [info, setInfo] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setInfo("");
    setBusy(true);
    try {
      if (mode === "login") {
        onLogin(await rpc("auth.login", { username, password }));
      } else {
        await rpc("auth.recover_owner", { recovery_key: key, new_password: password });
        setInfo(t("recover.done"));
        setMode("login");
        setPassword("");
      }
    } catch (e) {
      setError(err(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="center">
      <form className="card" onSubmit={submit}>
        <LangSwitch />
        <h1>{t("app.name")}</h1>
        {clinicName && <p className="muted" data-testid="clinic-name">{clinicName}</p>}
        <h2>{mode === "login" ? t("login.title") : t("recover.title")}</h2>
        {mode === "login" ? (
          <>
            <label htmlFor="u">{t("login.username")}</label>
            <input id="u" className="ltr" autoFocus value={username} onChange={(e) => setUsername(e.target.value)} data-testid="login-username" />
          </>
        ) : (
          <>
            <label htmlFor="k">{t("recover.key")}</label>
            <input id="k" className="mono" value={key} onChange={(e) => setKey(e.target.value)} data-testid="recover-key" />
          </>
        )}
        <label htmlFor="p">{mode === "login" ? t("login.password") : t("recover.newPassword")}</label>
        <input id="p" type="password" value={password} onChange={(e) => setPassword(e.target.value)} data-testid="login-password" />
        {error && <div className="error" role="alert" data-testid="login-error">{error}</div>}
        {info && <div className="success" data-testid="login-info">{info}</div>}
        <div className="actions">
          <button className="primary" type="submit" disabled={busy} data-testid="login-submit">
            {mode === "login" ? t("login.submit") : t("recover.submit")}
          </button>
        </div>
        <p>
          <button type="button" className="link" onClick={() => { setMode(mode === "login" ? "recover" : "login"); setError(""); }} data-testid="login-mode">
            {mode === "login" ? t("login.forgot") : t("recover.back")}
          </button>
        </p>
      </form>
    </div>
  );
}
