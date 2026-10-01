import { useEffect, useRef, useState } from "react";
import type { SessionInfo } from "../../../shared/ts/contract";
import { onSessionError, rpc } from "../lib/api";
import { useI18n } from "../i18n";
import { LangSwitch } from "./LangSwitch";
import { SystemInfoPage } from "./SystemInfo";
import { BackupPage } from "./Backup";
import { UsersPage } from "./Users";
import { AuditPage } from "./Audit";
import { SettingsPage } from "./Settings";
import { PasswordPage } from "./Password";

type Tab = "system" | "backup" | "users" | "audit" | "settings" | "password";

export function Shell({ session, clinicName, onSignOut }: { session: SessionInfo; clinicName: string; onSignOut: () => void }) {
  const { t, err } = useI18n();
  const can = (p: string) => session.permissions.includes(p);
  const tabs: Tab[] = [
    "system",
    ...(can("backup.view") ? (["backup"] as Tab[]) : []),
    ...(can("users.manage") ? (["users"] as Tab[]) : []),
    ...(can("audit.view") ? (["audit"] as Tab[]) : []),
    ...(can("settings.manage") ? (["settings"] as Tab[]) : []),
    "password",
  ];
  const [tab, setTab] = useState<Tab>("system");
  const [locked, setLocked] = useState(false);
  const [unlockPw, setUnlockPw] = useState("");
  const [unlockError, setUnlockError] = useState("");
  const lastTouch = useRef(0);

  // Core decides when the session locks; the UI only follows (no bypass).
  useEffect(() => {
    const off = onSessionError((e) => {
      if (e.code === "session_locked") setLocked(true);
      else onSignOut();
    });
    const poll = window.setInterval(() => {
      rpc("session.state", {}).then((s) => setLocked(s.locked)).catch(() => {});
    }, 15000);
    const activity = () => {
      const now = Date.now();
      if (!locked && now - lastTouch.current > 30000) {
        lastTouch.current = now;
        rpc("session.touch", {}).catch(() => {});
      }
    };
    window.addEventListener("keydown", activity);
    window.addEventListener("pointerdown", activity);
    return () => {
      off();
      window.clearInterval(poll);
      window.removeEventListener("keydown", activity);
      window.removeEventListener("pointerdown", activity);
    };
  }, [locked]);

  const lock = () => rpc("session.lock", {}).then(() => setLocked(true)).catch(() => {});
  const unlock = async (e: React.FormEvent) => {
    e.preventDefault();
    setUnlockError("");
    try {
      await rpc("session.unlock", { password: unlockPw });
      setUnlockPw("");
      setLocked(false);
    } catch (x) {
      setUnlockError(err(x));
    }
  };
  const logout = () => rpc("auth.logout", {}).finally(onSignOut);

  return (
    <div className="shell">
      <header className="topbar">
        <div>
          <strong>{t("app.name")}</strong> · <span>{clinicName}</span>
        </div>
        <div className="row">
          <span data-testid="current-user">{session.user.display_name} ({t(`role.${session.user.role}`)})</span>
          <button onClick={lock} data-testid="lock">{t("shell.lock")}</button>
          <button onClick={logout} data-testid="logout">{t("shell.logout")}</button>
        </div>
      </header>
      <main>
        <div className="row" style={{ justifyContent: "space-between", paddingInline: 16 }}>
          <nav className="tabs" role="tablist">
            {tabs.map((x) => (
              <button key={x} role="tab" aria-selected={tab === x} onClick={() => setTab(x)} data-testid={`tab-${x}`}>
                {t(`nav.${x}`)}
              </button>
            ))}
          </nav>
          <LangSwitch />
        </div>
        <div className="content">
          {tab === "system" && <SystemInfoPage />}
          {tab === "backup" && <BackupPage canCreate={can("backup.create")} />}
          {tab === "users" && <UsersPage />}
          {tab === "audit" && <AuditPage />}
          {tab === "settings" && <SettingsPage />}
          {tab === "password" && <PasswordPage />}
        </div>
      </main>
      {locked && (
        <div className="overlay" data-testid="lock-screen">
          <form className="card" onSubmit={unlock}>
            <h1>{t("lock.title")}</h1>
            <p className="muted">{session.user.display_name} — {t("lock.hint")}</p>
            <input type="password" autoFocus value={unlockPw} onChange={(e) => setUnlockPw(e.target.value)} data-testid="unlock-password" />
            {unlockError && <div className="error" role="alert">{unlockError}</div>}
            <div className="actions">
              <button className="primary" type="submit" data-testid="unlock">{t("lock.unlock")}</button>
              <button type="button" onClick={logout}>{t("shell.logout")}</button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}
