import { useState } from "react";
import { ArrowLeft, KeyRound, LockKeyhole, LogIn, UserRound } from "lucide-react";
import type { SessionInfo } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { LangSwitch } from "./LangSwitch";
import { Button } from "../ui/Button";
import { Field, PasswordInput, TextInput } from "../ui/Field";
import { Notice } from "../ui/Feedback";
import { ArtaveoLockup, ClinicMark, brandAssets } from "../ui/Brand";

/** Login and Owner recovery (roadmap 1.6). Artaveo + the clinic's own identity (2.1b). */
export function Login({ clinicName, logo, version, onLogin }: { clinicName: string | null; logo: string | null; version: string; onLogin: (s: SessionInfo) => void }) {
  const { t, err } = useI18n();
  const [mode, setMode] = useState<"login" | "recover">("login");
  const login = useForm({ username: "", password: "" }, { username: v.required, password: v.required });
  const recover = useForm({ key: "", password: "" }, { key: v.required, password: v.password });
  const [error, setError] = useState("");
  const [info, setInfo] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setInfo("");
    const form = mode === "login" ? login : recover;
    if (!form.validate()) return;
    setBusy(true);
    try {
      if (mode === "login") {
        onLogin(await rpc("auth.login", { username: login.values.username, password: login.values.password }));
      } else {
        await rpc("auth.recover_owner", { recovery_key: recover.values.key, new_password: recover.values.password });
        setInfo(t("recover.done"));
        setMode("login");
        recover.reset();
      }
    } catch (x) {
      const placed = mode === "login" ? login.serverError(x) : recover.serverError(x, { recovery_key: "key", new_password: "password" });
      if (!placed) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  const switchMode = () => {
    setMode(mode === "login" ? "recover" : "login");
    setError("");
    setInfo("");
  };

  return (
    <div className="auth-split">
      <aside className="auth-brand-panel" aria-hidden>
        <img className="auth-brand-art" src={brandAssets.master} alt="" />
        <div className="auth-brand-caption">
          <span className="auth-brand-name">Artaveo Dental</span>
          <span>{t("system.tagline")}</span>
        </div>
      </aside>
      <main className="auth-form-panel">
        <div className="auth-top">
          <LangSwitch />
        </div>
        <form className="auth-form" onSubmit={submit} noValidate>
          <div className="auth-clinic">
            <ClinicMark name={clinicName} logo={logo} size="lg" />
            <div className="stack" style={{ gap: 2 }}>
              {clinicName && <span className="t-title" data-testid="clinic-name">{clinicName}</span>}
              <span className="subtle t-caption">{t("login.clinicOf")}</span>
            </div>
          </div>
          <div>
            <h1 className="t-title-lg">{mode === "login" ? t("login.title") : t("recover.title")}</h1>
            <p className="muted">{mode === "login" ? t("login.subtitle") : t("recover.subtitle")}</p>
          </div>
          {info && <Notice tone="success" testId="login-info">{info}</Notice>}
          {error && <Notice tone="danger" testId="login-error">{error}</Notice>}
          {mode === "login" ? (
            <>
              <Field label={t("login.username")} error={login.error("username") && t(login.error("username")!)}>
                <TextInput icon={UserRound} large dir="ltr" autoFocus autoComplete="username" value={login.values.username} onChange={(e) => login.set("username", e.target.value)} data-testid="login-username" />
              </Field>
              <Field label={t("login.password")} error={login.error("password") && t(login.error("password")!)}>
                <PasswordInput icon={LockKeyhole} large value={login.values.password} onChange={(e) => login.set("password", e.target.value)} data-testid="login-password" />
              </Field>
            </>
          ) : (
            <>
              <Field label={t("recover.key")} hint={t("recover.keyHint")} error={recover.error("key") && t(recover.error("key")!)}>
                <TextInput icon={KeyRound} large className="mono-input" dir="ltr" autoFocus value={recover.values.key} onChange={(e) => recover.set("key", e.target.value)} placeholder="ABCDEF-GHIJKL-…" data-testid="recover-key" />
              </Field>
              <Field label={t("recover.newPassword")} hint={t("hint.password")} error={recover.error("password") && t(recover.error("password")!)}>
                <PasswordInput icon={LockKeyhole} large value={recover.values.password} onChange={(e) => recover.set("password", e.target.value)} data-testid="login-password" />
              </Field>
            </>
          )}
          <Button type="submit" variant="primary" size="lg" block loading={busy} icon={mode === "login" ? LogIn : KeyRound} data-testid="login-submit">
            {mode === "login" ? t("login.submit") : t("recover.submit")}
          </Button>
          <Button variant="link" onClick={switchMode} icon={mode === "recover" ? ArrowLeft : undefined} flipIcon data-testid="login-mode" style={{ alignSelf: "center" }}>
            {mode === "login" ? t("login.forgot") : t("recover.back")}
          </Button>
        </form>
        <div className="auth-footer">
          <ArtaveoLockup height={18} />
          <span>·</span>
          <span>{t("system.version")} <bdi className="ltr num">{version}</bdi></span>
        </div>
      </main>
    </div>
  );
}
