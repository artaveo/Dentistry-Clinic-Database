import { useEffect, useRef, useState } from "react";
import type { ClinicProfile, SessionInfo } from "../../../shared/ts/contract";
import { onSessionError, rpc } from "../lib/api";
import { useI18n } from "../i18n";
import { LangSwitch } from "./LangSwitch";
import { SystemInfoPage } from "./SystemInfo";
import { BackupPage } from "./Backup";
import { UsersPage } from "./Users";
import { AuditPage } from "./Audit";
import { ClinicPage } from "./Clinic";
import { SettingsPage } from "./Settings";
import { PasswordPage } from "./Password";
import { CommandPalette } from "../shell/CommandPalette";
import type { PaletteItem } from "../shell/CommandPalette";
import { NotificationBell } from "../shell/NotificationBell";
import { UserMenu } from "../shell/UserMenu";

type Tab = "system" | "backup" | "users" | "audit" | "clinic" | "settings" | "password";

export function Shell({ session, clinicName, onSignOut }: { session: SessionInfo; clinicName: string; onSignOut: () => void }) {
  const { t, err } = useI18n();
  const can = (p: string) => session.permissions.includes(p);
  const tabs: Tab[] = [
    "system",
    ...(can("backup.view") ? (["backup"] as Tab[]) : []),
    ...(can("users.manage") ? (["users"] as Tab[]) : []),
    ...(can("audit.view") ? (["audit"] as Tab[]) : []),
    ...(can("settings.manage") ? (["clinic", "settings"] as Tab[]) : []),
    "password",
  ];
  const [tab, setTab] = useState<Tab>("system");
  const [locked, setLocked] = useState(false);
  const [unlockPw, setUnlockPw] = useState("");
  const [unlockError, setUnlockError] = useState("");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [online, setOnline] = useState(navigator.onLine);
  const [clinic, setClinic] = useState<ClinicProfile | null>(null);
  const lastTouch = useRef(0);

  useEffect(() => {
    rpc("clinic.get", {}).then(setClinic).catch(() => {});
  }, []);

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
    const onOnline = () => setOnline(true);
    const onOffline = () => setOnline(false);
    window.addEventListener("online", onOnline);
    window.addEventListener("offline", onOffline);
    return () => {
      off();
      window.clearInterval(poll);
      window.removeEventListener("keydown", activity);
      window.removeEventListener("pointerdown", activity);
      window.removeEventListener("online", onOnline);
      window.removeEventListener("offline", onOffline);
    };
  }, [locked]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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

  const paletteItems: PaletteItem[] = tabs.map((x) => ({ id: x, label: t(`nav.${x}`), onSelect: () => setTab(x) }));

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="clinic-identity">
          {/* The uploaded logo file is stored (ADR-12 pattern) but serving it to
              the UI needs an attachment endpoint that does not exist until
              Phase 3; the sidebar shows an initial instead until then. */}
          <span className="clinic-logo-fallback">{(clinicName || "A").slice(0, 1)}</span>
          <strong data-testid="shell-clinic-name">{clinicName}</strong>
        </div>
        <nav className="sidebar-nav" role="tablist">
          {tabs.map((x) => (
            <button key={x} role="tab" aria-selected={tab === x} aria-current={tab === x ? "page" : undefined} onClick={() => setTab(x)} data-testid={`tab-${x}`}>
              {t(`nav.${x}`)}
            </button>
          ))}
        </nav>
        <LangSwitch />
      </aside>
      <header className="topbar-v2">
        <div className="breadcrumbs">{t("app.name")} / {t(`nav.${tab}`)}</div>
        <div className="status-strip">
          <span className={`badge ${online ? "ok" : "bad"}`} data-testid="status-online">
            {online ? t("shell.status.online") : t("shell.status.offline")}
          </span>
          {clinic && <span className="badge info" data-testid="status-install-mode">{t(`shell.status.installMode.${clinic.install_mode}`)}</span>}
          <button type="button" className="ghost" onClick={() => setPaletteOpen(true)} data-testid="command-palette-open" title={t("shell.commandPalette.hint")}>
            ⌘ <kbd>Ctrl</kbd>+<kbd>K</kbd>
          </button>
          <NotificationBell />
          <UserMenu displayName={session.user.display_name} roleLabel={t(`role.${session.user.role}`)} onLock={lock} onLogout={logout} />
        </div>
      </header>
      <main className="shell-main">
        {tab === "system" && <SystemInfoPage />}
        {tab === "backup" && <BackupPage canCreate={can("backup.create")} />}
        {tab === "users" && <UsersPage />}
        {tab === "audit" && <AuditPage />}
        {tab === "clinic" && <ClinicPage />}
        {tab === "settings" && <SettingsPage />}
        {tab === "password" && <PasswordPage />}
      </main>
      {paletteOpen && <CommandPalette items={paletteItems} onClose={() => setPaletteOpen(false)} />}
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
