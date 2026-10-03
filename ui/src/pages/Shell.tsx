import { useCallback, useEffect, useRef, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { BellRing, Building2, CalendarDays, DatabaseBackup, Info, ListOrdered, LockKeyhole, ScrollText, Search, Settings as SettingsIcon, Stethoscope, Unlock, UserRound, Users } from "lucide-react";
import type { ClinicProfile, SessionInfo } from "../../../shared/ts/contract";
import { SESSION_CHECK_EVENT, onSessionError, rpc } from "../lib/api";
import { formatClock, formatDate } from "../lib/dates";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { rememberBrand } from "../lib/color";
import { LangSwitch } from "./LangSwitch";
import { SystemInfoPage } from "./SystemInfo";
import { BackupPage } from "./Backup";
import { UsersPage } from "./Users";
import { PatientsPage } from "./patients/PatientsPage";
import { AuditPage } from "./Audit";
import { DoctorsPage } from "./Doctors";
import { CalendarPage } from "./reception/CalendarPage";
import { QueuePage } from "./reception/QueuePage";
import { RecallsPage } from "./reception/RecallsPage";
import { roleText } from "./reception/labels";
import { ClinicPage } from "./Clinic";
import { SettingsPage } from "./Settings";
import { PasswordDialog } from "./Password";
import { CommandPalette } from "../shell/CommandPalette";
import type { PaletteItem } from "../shell/CommandPalette";
import { NotificationBell } from "../shell/NotificationBell";
import { UserMenu } from "../shell/UserMenu";
import { Button } from "../ui/Button";
import { Field, PasswordInput } from "../ui/Field";
import { Avatar, ClinicMark, DentalMark } from "../ui/Brand";
import { Notice } from "../ui/Feedback";

type Tab = "patients" | "appointments" | "queue" | "recalls" | "doctors" | "clinic" | "users" | "backup" | "audit" | "settings" | "system";
const ICONS: Record<Tab, LucideIcon> = { patients: UserRound, appointments: CalendarDays, queue: ListOrdered, recalls: BellRing, doctors: Stethoscope, clinic: Building2, users: Users, backup: DatabaseBackup, audit: ScrollText, settings: SettingsIcon, system: Info };

/**
 * OF-008/OF-012: real input (mouse, keyboard, wheel) is the single source of
 * "the user is working". It is reported to the Core every HEARTBEAT_MS with
 * how long ago it happened, so the Core's idle timer is exact; background
 * requests never count. The UI asks the Core again exactly when the lock is
 * due (`idle_seconds_left`), so the screen locks on time, not up to a poll later.
 */
const HEARTBEAT_MS = 3_000;
/** Longest wait between two lock checks (also covers a timeout changed elsewhere). */
const MAX_CHECK_MS = 15_000;

export function Shell({
  session,
  clinicName,
  logo,
  version,
  onLogoChange,
  onSignOut,
}: {
  session: SessionInfo;
  clinicName: string;
  logo: string | null;
  version: string;
  onLogoChange: (l: string | null) => void;
  onSignOut: () => void;
}) {
  const { t } = useI18n();
  const can = (p: string) => session.permissions.includes(p);
  const groups = [
    { label: t("nav.group.patients"), tabs: can("patients.view") ? (["patients"] as Tab[]) : [] },
    { label: t("nav.group.reception"), tabs: can("appointments.view") ? (["appointments", "queue", "recalls"] as Tab[]) : [] },
    {
      label: t("nav.group.clinic"),
      tabs: [...(can("doctors.manage") ? (["doctors"] as Tab[]) : []), ...(can("settings.manage") ? (["clinic"] as Tab[]) : [])],
    },
    {
      label: t("nav.group.admin"),
      tabs: [
        ...(can("users.manage") ? (["users"] as Tab[]) : []),
        ...(can("backup.view") ? (["backup"] as Tab[]) : []),
        ...(can("audit.view") ? (["audit"] as Tab[]) : []),
        ...(can("settings.manage") ? (["settings"] as Tab[]) : []),
      ],
    },
    { label: t("nav.group.system"), tabs: ["system"] as Tab[] },
  ].filter((g) => g.tabs.length);
  const tabs = groups.flatMap((g) => g.tabs);
  const perms = { edit: can("appointments.edit"), treat: can("appointments.treat"), createPatients: can("patients.edit"), manageDoctors: can("doctors.manage") };

  const [tab, setTab] = useState<Tab>("system");
  const [locked, setLocked] = useState(session.locked);
  /** Bumped after every unlock: pages remount and reload, so nothing stale survives the lock (OF-008). */
  const [generation, setGeneration] = useState(0);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [online, setOnline] = useState(navigator.onLine);
  const [clinic, setClinic] = useState<ClinicProfile | null>(null);
  const lastActivity = useRef(Date.now());
  const lastReport = useRef(Date.now());
  const lockedRef = useRef(locked);
  lockedRef.current = locked;
  const checkNow = useRef<() => void>(() => {});

  const loadClinic = useCallback(() => {
    rpc("clinic.get", {})
      .then((c) => {
        setClinic(c);
        rememberBrand(c.color_primary, c.color_accent);
      })
      .catch(() => {});
  }, []);
  useEffect(loadClinic, []);

  useEffect(() => {
    const off = onSessionError((e) => {
      if (e.code === "session_locked") setLocked(true);
      else onSignOut();
    });
    const activity = () => {
      lastActivity.current = Date.now();
    };
    const events = ["pointermove", "pointerdown", "keydown", "wheel"] as const;
    events.forEach((ev) => window.addEventListener(ev, activity, { passive: true }));
    let checkTimer = 0;
    const follow = (st: { locked: boolean; idle_seconds_left: number }) => {
      window.clearTimeout(checkTimer);
      if (st.locked) return setLocked(true);
      const ms = Math.min(Math.max(st.idle_seconds_left * 1000 + 400, 1000), MAX_CHECK_MS);
      checkTimer = window.setTimeout(check, ms);
    };
    const check = () => {
      if (lockedRef.current) return;
      rpc("session.state", {}).then(follow).catch(() => {
        window.clearTimeout(checkTimer);
        checkTimer = window.setTimeout(check, 5_000);
      });
    };
    checkNow.current = check;
    check();
    const heartbeat = window.setInterval(() => {
      if (lockedRef.current || lastActivity.current <= lastReport.current) return;
      lastReport.current = Date.now();
      rpc("session.touch", { idle_ms: Math.max(0, Date.now() - lastActivity.current) }).then(follow).catch(() => {});
    }, HEARTBEAT_MS);
    window.addEventListener(SESSION_CHECK_EVENT, check);
    const onOnline = () => setOnline(true);
    const onOffline = () => setOnline(false);
    window.addEventListener("online", onOnline);
    window.addEventListener("offline", onOffline);
    return () => {
      off();
      events.forEach((ev) => window.removeEventListener(ev, activity));
      window.clearInterval(heartbeat);
      window.clearTimeout(checkTimer);
      window.removeEventListener(SESSION_CHECK_EVENT, check);
      window.removeEventListener("online", onOnline);
      window.removeEventListener("offline", onOffline);
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k" && !lockedRef.current) {
        e.preventDefault();
        setPaletteOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const lock = () => rpc("session.lock", {}).then(() => setLocked(true)).catch(() => {});
  const logout = () => rpc("auth.logout", {}).finally(onSignOut);
  const unlocked = () => {
    lastActivity.current = lastReport.current = Date.now();
    lockedRef.current = false;
    setLocked(false);
    setGeneration((g) => g + 1);
    window.setTimeout(() => checkNow.current(), 0);
  };

  const paletteItems: PaletteItem[] = tabs.map((x) => ({ id: x, label: t(`nav.${x}`), icon: ICONS[x], onSelect: () => setTab(x) }));

  return (
    <div className="app-shell">
      <header className="app-header" inert={locked || undefined} aria-hidden={locked || undefined}>
        <div className="clinic-identity" data-testid="clinic-identity">
          <ClinicMark name={clinicName} logo={logo} />
          <div className="clinic-text">
            <span className="clinic-name" data-testid="shell-clinic-name">{clinicName}</span>
            {clinic && <span className="clinic-sub" data-testid="status-install-mode">{t(`shell.status.installMode.${clinic.install_mode}`)}</span>}
          </div>
        </div>
        <button type="button" className="search-trigger" onClick={() => setPaletteOpen(true)} data-testid="command-palette-open" title={t("shell.commandPalette.hint")}>
          <Search aria-hidden />
          <span>{t("shell.commandPalette.trigger")}</span>
          <span className="kbds"><kbd>Ctrl</kbd><kbd>K</kbd></span>
        </button>
        <div className="header-end">
          <span className={`status-pill ${online ? "" : "offline"}`} data-testid="status-online">
            <span className="dot" aria-hidden />
            {online ? t("shell.status.online") : t("shell.status.offline")}
          </span>
          <NotificationBell />
          <UserMenu
            displayName={session.user.display_name}
            roleLabel={roleText(t, session.user.role, session.user.role_label)}
            onPassword={() => setPasswordOpen(true)}
            onLock={lock}
            onLogout={logout}
          />
        </div>
      </header>

      <aside className="app-sidebar" inert={locked || undefined} aria-hidden={locked || undefined}>
        <nav aria-label={t("shell.navigation")}>
          {groups.map((g) => (
            <div className="nav-group" key={g.label}>
              <span className="t-overline">{g.label}</span>
              {g.tabs.map((x) => {
                const Icon = ICONS[x];
                return (
                  <button key={x} type="button" className="nav-item" aria-current={tab === x ? "page" : undefined} onClick={() => setTab(x)} data-testid={`tab-${x}`} title={t(`nav.${x}`)}>
                    <Icon aria-hidden />
                    <span>{t(`nav.${x}`)}</span>
                  </button>
                );
              })}
            </div>
          ))}
        </nav>
        <div className="sidebar-footer">
          <LangSwitch />
          <div className="version">
            <DentalMark size={16} />
            <bdi className="ltr">Artaveo Dental {version}</bdi>
          </div>
        </div>
      </aside>

      <main className="app-main" key={generation} inert={locked || undefined} aria-hidden={locked || undefined}>
        {tab === "patients" && <PatientsPage canEdit={can("patients.edit")} appointments={{ ...perms, view: can("appointments.view") }} clinic={clinic} clinicName={clinicName} />}
        {tab === "appointments" && <CalendarPage clinic={clinic} perms={perms} clinicName={clinicName} />}
        {tab === "queue" && <QueuePage clinic={clinic} perms={perms} clinicName={clinicName} />}
        {tab === "recalls" && <RecallsPage clinic={clinic} perms={perms} clinicName={clinicName} />}
        {tab === "doctors" && <DoctorsPage clinic={clinic} />}
        {tab === "system" && <SystemInfoPage version={version} />}
        {tab === "backup" && <BackupPage canCreate={can("backup.create")} calendar={clinic?.calendar_system} />}
        {tab === "users" && <UsersPage currentUserId={session.user.id} />}
        {tab === "audit" && <AuditPage calendar={clinic?.calendar_system} />}
        {tab === "clinic" && <ClinicPage logo={logo} onLogoChange={onLogoChange} onSaved={loadClinic} />}
        {tab === "settings" && <SettingsPage />}
      </main>

      {paletteOpen && <CommandPalette items={paletteItems} onClose={() => setPaletteOpen(false)} />}
      {passwordOpen && <PasswordDialog onClose={() => setPasswordOpen(false)} />}
      {locked && <LockScreen name={session.user.display_name} onUnlocked={unlocked} onLogout={logout} />}
    </div>
  );
}

function LockScreen({ name, onUnlocked, onLogout }: { name: string; onUnlocked: () => void; onLogout: () => void }) {
  const { t, lang, err } = useI18n();
  const form = useForm({ password: "" }, { password: v.required });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [now, setNow] = useState(new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 10_000);
    return () => window.clearInterval(id);
  }, []);
  const kabul = new Date(now.getTime() + (now.getTimezoneOffset() + 270) * 60_000);

  const unlock = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await rpc("session.unlock", { password: form.values.password });
      onUnlocked();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="lock-screen" data-testid="lock-screen">
      <div className="glass-overlay lock-card">
        <div className="lock-clock num" aria-hidden>{formatClock(kabul.getHours(), kabul.getMinutes(), lang)}</div>
        <div className="lock-date">{formatDate(now.toISOString(), lang)}</div>
        <Avatar name={name} size="lg" />
        <div>
          <div className="t-title">{name}</div>
          <div className="muted row" style={{ justifyContent: "center" }}><LockKeyhole size={15} aria-hidden /> {t("lock.title")}</div>
        </div>
        <form onSubmit={unlock} noValidate>
          {error && <Notice tone="danger">{error}</Notice>}
          <Field label={t("lock.hint")} error={form.error("password") && t(form.error("password")!)}>
            <PasswordInput large autoFocus value={form.values.password} onChange={(e) => form.set("password", e.target.value)} data-testid="unlock-password" />
          </Field>
          <Button type="submit" variant="primary" size="lg" block loading={busy} icon={Unlock} data-testid="unlock">{t("lock.unlock")}</Button>
          <Button variant="subtle" block onClick={onLogout} data-testid="lock-logout">{t("lock.otherUser")}</Button>
        </form>
      </div>
    </div>
  );
}
