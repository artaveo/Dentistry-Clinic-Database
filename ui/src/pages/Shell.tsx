import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { BarChart3, BellRing, Building2, CalendarDays, CalendarPlus, ClipboardList, DatabaseBackup, KeyRound, Landmark, Package, Info, LayoutDashboard, ListOrdered, LockKeyhole, Network, ScrollText, Search, Settings as SettingsIcon, Stethoscope, Tags, Unlock, UserPlus, UserRound, Users, Wallet } from "lucide-react";
import type { ClinicProfile, SearchHit, SessionInfo } from "../../../shared/ts/contract";
import { SESSION_CHECK_EVENT, onSessionError, rpc } from "../lib/api";
import { formatClock, formatDate } from "../lib/dates";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { rememberBrand } from "../lib/color";
import { LangSwitch } from "./LangSwitch";
import { SystemInfoPage } from "./SystemInfo";
import { BackupPage } from "./Backup";
import { LicensePage } from "./License";
import { LicenseBanner } from "../shell/LicenseBanner";
import { UpdateBanner } from "../shell/UpdateBanner";
import { setCurrentPage } from "../lib/diagnostics";
import { UsersPage } from "./Users";
import { PatientsPage } from "./patients/PatientsPage";
import { AuditPage } from "./Audit";
import { DoctorsPage } from "./Doctors";
import { CatalogPage } from "./CatalogPage";
import { ClinicalListsPage } from "./ClinicalListsPage";
import { CalendarPage } from "./reception/CalendarPage";
import { QueuePage } from "./reception/QueuePage";
import { RecallsPage } from "./reception/RecallsPage";
import { BillingPage } from "./billing/BillingPage";
import { DashboardPage } from "./Dashboard";
import { ReportsPage } from "./reports/ReportsPage";
import { InventoryPage } from "./inventory/InventoryPage";
import type { InventoryPerms } from "./inventory/common";
import type { BillingPerms } from "./billing/common";
import { roleText } from "./reception/labels";
import { ClinicPage } from "./Clinic";
import { SettingsHub, SettingsPage } from "./Settings";
import type { HubSection } from "./Settings";
import { NetworkPage } from "./Network";
import { ConnectionBanner } from "../shell/ConnectionBanner";
import { PasswordDialog } from "./Password";
import { CommandPalette } from "../shell/CommandPalette";
import type { PaletteItem } from "../shell/CommandPalette";
import { NotificationBell } from "../shell/NotificationBell";
import { UserMenu } from "../shell/UserMenu";
import { useModules } from "../shell/modules";
import { Button } from "../ui/Button";
import { Field, PasswordInput } from "../ui/Field";
import { Avatar, ClinicMark, DentalMark } from "../ui/Brand";
import { Notice } from "../ui/Feedback";

// U3: seven daily items in the sidebar. Queue and call list are tabs inside "appointments"; everything else
// (clinic, doctors, services, lists, users, backup, licence, audit, network, about) is a section of "settings".
type Tab = "dashboard" | "patients" | "appointments" | "billing" | "inventory" | "reports" | "settings";
type ApptSub = "calendar" | "queue" | "recalls";
type Section = "general" | "doctors" | "catalog" | "clinical" | "clinic" | "users" | "backup" | "license" | "audit" | "network" | "system";
const TAB_ORDER: Tab[] = ["dashboard", "patients", "appointments", "billing", "inventory", "reports", "settings"];
const TAB_ICONS: Record<Tab, LucideIcon> = { dashboard: LayoutDashboard, patients: UserRound, appointments: CalendarDays, billing: Landmark, inventory: Package, reports: BarChart3, settings: SettingsIcon };
const APPT_SUBS: { id: ApptSub; icon: LucideIcon }[] = [{ id: "calendar", icon: CalendarDays }, { id: "queue", icon: ListOrdered }, { id: "recalls", icon: BellRing }];
const SECTION_ORDER: Section[] = ["general", "doctors", "catalog", "clinical", "clinic", "users", "backup", "license", "audit", "network", "system"];
const SECTION_ICONS: Record<Section, LucideIcon> = { general: SettingsIcon, doctors: Stethoscope, catalog: Tags, clinical: ClipboardList, clinic: Building2, users: Users, backup: DatabaseBackup, license: KeyRound, audit: ScrollText, network: Network, system: Info };
const SECTION_GROUP: Record<Section, "clinic" | "admin" | "system"> = { general: "clinic", doctors: "clinic", catalog: "clinic", clinical: "clinic", clinic: "clinic", users: "admin", backup: "admin", license: "admin", audit: "admin", network: "admin", system: "system" };

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
  clientOf,
  clinicName,
  logo,
  version,
  onLogoChange,
  onSignOut,
}: {
  session: SessionInfo;
  /** Name of the clinic server when this computer is a client (Phase 7), else null. */
  clientOf: string | null;
  clinicName: string;
  logo: string | null;
  version: string;
  onLogoChange: (l: string | null) => void;
  onSignOut: () => void;
}) {
  const { t } = useI18n();
  const can = (p: string) => session.permissions.includes(p);
  // The clinic may switch the stock module off (Settings): then its page, reports, alerts and search hits are hidden.
  const modules = useModules();
  const canStock = can("inventory.view") && modules.stock;
  // The same permission checks as before the sidebar was simplified, only grouped differently.
  const tabAllowed: Record<Tab, boolean> = { dashboard: true, patients: can("patients.view"), appointments: can("appointments.view"), billing: can("billing.view"), inventory: canStock, reports: can("reports.view"), settings: true };
  const tabs = TAB_ORDER.filter((x) => tabAllowed[x]);
  const sectionAllowed: Record<Section, boolean> = {
    general: can("settings.manage"),
    doctors: can("doctors.manage"),
    catalog: can("settings.manage"),
    clinical: can("settings.manage"),
    clinic: can("settings.manage"),
    users: can("users.manage"),
    backup: can("backup.view"),
    license: can("settings.manage"),
    audit: can("audit.view"),
    network: can("settings.manage"),
    system: true,
  };
  const sectionIds = SECTION_ORDER.filter((x) => sectionAllowed[x]);
  const billing: BillingPerms = { view: can("billing.view"), edit: can("billing.edit"), void: can("billing.void"), override: can("billing.override"), reports: can("reports.view"), settings: can("settings.manage") };
  const inventory: InventoryPerms = { view: can("inventory.view"), manage: can("inventory.manage"), billingEdit: can("billing.edit"), billingVoid: can("billing.void"), clinicalView: can("clinical.view"), clinicalEdit: can("clinical.edit"), treat: can("appointments.treat"), reports: can("reports.view"), settings: can("settings.manage"), patientsEdit: can("patients.edit") };
  const perms = { edit: can("appointments.edit"), treat: can("appointments.treat"), createPatients: can("patients.edit"), manageDoctors: can("doctors.manage") };

  // U4: reception starts at today's queue, everyone else at the dashboard.
  const startAtQueue = session.user.role === "receptionist" && can("appointments.view");
  const [tab, setTab] = useState<Tab>(startAtQueue ? "appointments" : "dashboard");
  const [apptSub, setApptSub] = useState<ApptSub>(startAtQueue ? "queue" : "calendar");
  const [section, setSection] = useState<Section>("general");
  /** B8: bumped when "patients" is clicked in the menu, so the page remounts and returns to its list. */
  const [patientsNav, setPatientsNav] = useState(0);
  const activeSection: Section = sectionIds.includes(section) ? section : sectionIds[0];
  useEffect(() => setCurrentPage(tab === "settings" ? activeSection : tab === "appointments" && apptSub !== "calendar" ? apptSub : tab), [tab, activeSection, apptSub]);
  /** The patient the global search asked to open (9.7). */
  const [openPatient, setOpenPatient] = useState<{ id: string; n: number } | null>(null);
  const [locked, setLocked] = useState(session.locked);
  /** Bumped after every unlock: pages remount and reload, so nothing stale survives the lock (OF-008). */
  const [generation, setGeneration] = useState(0);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [passwordOpen, setPasswordOpen] = useState(false);
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
      .catch((e) => console.error("clinic.get failed:", String(e)));
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
    return () => {
      off();
      events.forEach((ev) => window.removeEventListener(ev, activity));
      window.clearInterval(heartbeat);
      window.clearTimeout(checkTimer);
      window.removeEventListener(SESSION_CHECK_EVENT, check);
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

  /** Opens a page by name. Old page names still work: "queue"/"recalls" are tabs of appointments, the rest of the old menu are settings sections. */
  const go = (page: string) => {
    if (page === "queue" || page === "recalls") {
      if (!tabAllowed.appointments) return;
      setApptSub(page);
      setTab("appointments");
      return;
    }
    const name = page === "settings" ? "general" : page;
    if ((SECTION_ORDER as string[]).includes(name)) {
      if (!sectionAllowed[name as Section]) return;
      setSection(name as Section);
      setTab("settings");
      return;
    }
    if (!(TAB_ORDER as string[]).includes(page) || !tabAllowed[page as Tab]) return;
    if (page === "appointments") setApptSub("calendar");
    setTab(page as Tab);
  };
  /** A click in the sidebar: starts the page from its first screen (B8: patients returns to its list). */
  const openTab = (x: Tab) => {
    if (x === "patients") {
      setOpenPatient(null);
      setPatientsNav((n) => n + 1);
    } else if (x === "appointments") setApptSub("calendar");
    else if (x === "settings") setSection(sectionIds[0]);
    setTab(x);
  };
  const openHit = (hit: SearchHit) => {
    if (hit.kind === "patient") setOpenPatient((p) => ({ id: hit.id, n: (p?.n ?? 0) + 1 }));
    go(hit.page);
  };
  const paletteItems: PaletteItem[] = [
    ...tabs.map((x) => ({ id: x, label: t(`nav.${x}`), icon: TAB_ICONS[x], onSelect: () => openTab(x) })),
    ...(tabAllowed.appointments ? (["queue", "recalls"] as const).map((x) => ({ id: x, label: t(`nav.${x}`), icon: APPT_SUBS.find((s) => s.id === x)?.icon, onSelect: () => go(x) })) : []),
    ...sectionIds.map((x) => ({ id: x, label: x === "general" ? t("hub.general") : t(`nav.${x}`), icon: SECTION_ICONS[x], onSelect: () => go(x) })),
    // U5: these only open the right page; the last click ("add", "new", "receive") is on that page.
    ...(can("patients.edit") && tabAllowed.patients ? [{ id: "action-new-patient", label: t("palette.newPatient"), icon: UserPlus, group: t("palette.group.actions"), onSelect: () => openTab("patients") }] : []),
    ...(can("appointments.edit") && tabAllowed.appointments ? [{ id: "action-new-appointment", label: t("palette.newAppointment"), icon: CalendarPlus, group: t("palette.group.actions"), onSelect: () => openTab("appointments") }] : []),
    ...(can("billing.edit") && tabAllowed.billing ? [{ id: "action-receive-payment", label: t("palette.receivePayment"), icon: Wallet, group: t("palette.group.actions"), onSelect: () => openTab("billing") }] : []),
  ];
  // Each section embeds the existing page unchanged; `render` runs only for the chosen one.
  const sectionPage: Record<Section, () => ReactNode> = {
    general: () => <SettingsPage />,
    doctors: () => <DoctorsPage clinic={clinic} />,
    catalog: () => <CatalogPage />,
    clinical: () => <ClinicalListsPage />,
    clinic: () => <ClinicPage logo={logo} onLogoChange={onLogoChange} onSaved={loadClinic} />,
    users: () => <UsersPage currentUserId={session.user.id} />,
    backup: () => <BackupPage canCreate={can("backup.create")} canRestore={can("backup.restore")} calendar={clinic?.calendar_system} onRestored={onSignOut} />,
    license: () => <LicensePage calendar={clinic?.calendar_system} canManage={can("settings.manage")} />,
    audit: () => <AuditPage calendar={clinic?.calendar_system} />,
    network: () => <NetworkPage calendar={clinic?.calendar_system} onChanged={loadClinic} />,
    system: () => <SystemInfoPage version={version} />,
  };
  const hubSections: HubSection[] = sectionIds.map((id) => ({ id, group: t(`nav.group.${SECTION_GROUP[id]}`), label: id === "general" ? t("hub.general") : t(`nav.${id}`), icon: SECTION_ICONS[id], render: sectionPage[id] }));

  return (
    <div className="app-shell" data-locked={locked || undefined}>
      <header className="app-header" inert={locked || undefined} aria-hidden={locked || undefined}>
        <div className="clinic-identity" data-testid="clinic-identity">
          <ClinicMark name={clinicName} logo={logo} />
          <div className="clinic-text">
            <span className="clinic-name" data-testid="shell-clinic-name">{clinicName}</span>
            {clinic && <span className="clinic-sub" data-testid="status-install-mode">{t(`shell.status.installMode.${clientOf ? "client" : clinic.install_mode}`)}</span>}
          </div>
        </div>
        <button type="button" className="search-trigger" onClick={() => setPaletteOpen(true)} data-testid="command-palette-open" title={t("shell.commandPalette.hint")}>
          <Search aria-hidden />
          <span>{t("shell.commandPalette.trigger")}</span>
          <span className="kbds"><kbd>Ctrl</kbd><kbd>K</kbd></span>
        </button>
        <div className="header-end">
          <NotificationBell calendar={clinic?.calendar_system ?? "shamsi"} go={go} hideStock={!modules.stock} />
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
          <div className="nav-group nav-flat">
            {tabs.map((x) => {
              const Icon = TAB_ICONS[x];
              return (
                <button key={x} type="button" className="nav-item" aria-current={tab === x ? "page" : undefined} onClick={() => openTab(x)} data-testid={`tab-${x}`} title={t(`nav.${x}`)}>
                  <Icon aria-hidden />
                  <span>{t(`nav.${x}`)}</span>
                </button>
              );
            })}
          </div>
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
        {clientOf && <ConnectionBanner onReconnected={() => setGeneration((g) => g + 1)} />}
        <LicenseBanner canManage={can("settings.manage")} onOpen={() => go("license")} />
        <UpdateBanner canManage={can("settings.manage")} />
        {tab === "dashboard" && <DashboardPage calendar={clinic?.calendar_system ?? "shamsi"} go={go} stock={modules.stock} />}
        {tab === "reports" && <ReportsPage clinic={clinic} canSeeStock={canStock} />}
        {tab === "patients" && <PatientsPage key={patientsNav} openRequest={openPatient} canEdit={can("patients.edit")} billing={billing} clinical={{ view: can("clinical.view"), edit: can("clinical.edit") }} appointments={{ ...perms, view: can("appointments.view") }} clinic={clinic} clinicName={clinicName} />}
        {tab === "appointments" && (
          <div className="appts-wrap">
            <div className="appts-tabs">
              <div className="profile-tabs" role="tablist" aria-label={t("appts.subnav")}>
                {APPT_SUBS.map((s) => (
                  <button key={s.id} type="button" role="tab" className="profile-tab" aria-selected={apptSub === s.id} aria-current={apptSub === s.id ? "page" : undefined} onClick={() => setApptSub(s.id)} data-testid={s.id === "calendar" ? "appts-sub-calendar" : `tab-${s.id}`}>
                    <s.icon aria-hidden />
                    <span>{s.id === "calendar" ? t("appts.sub.calendar") : t(`nav.${s.id}`)}</span>
                  </button>
                ))}
              </div>
            </div>
            {apptSub === "calendar" && <CalendarPage clinic={clinic} perms={perms} clinicName={clinicName} />}
            {apptSub === "queue" && <QueuePage clinic={clinic} perms={perms} clinicName={clinicName} />}
            {apptSub === "recalls" && <RecallsPage clinic={clinic} perms={perms} clinicName={clinicName} />}
          </div>
        )}
        {tab === "billing" && <BillingPage perms={billing} clinic={clinic} />}
        {tab === "inventory" && <InventoryPage perms={inventory} clinic={clinic} />}
        {tab === "settings" && <SettingsHub sections={hubSections} active={activeSection} onSelect={(id) => setSection(id as Section)} />}
      </main>

      {paletteOpen && <CommandPalette items={paletteItems} calendar={clinic?.calendar_system ?? "shamsi"} hideLabCases={!modules.stock} onOpenHit={openHit} onClose={() => setPaletteOpen(false)} />}
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
