import { useEffect, useState } from "react";
import { ChevronDown, Gauge, Info, KeyRound, LockKeyhole, LogOut, Monitor, Moon, Sun } from "lucide-react";
import type { DoctorInfo, ThemePreference } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { useI18n } from "../i18n";
import { useTheme } from "../theme";
import { DOCTORS_CHANGED, DoctorPic, PersonAvatar } from "../ui/Avatar";
import { Segmented, Switch } from "../ui/Controls";
import { Popover } from "../ui/Overlay";

/** User menu (roadmap 2.2): who's signed in, appearance, performance mode, password, lock, sign out. */
export function UserMenu({
  userId,
  displayName,
  roleLabel,
  onAbout,
  onPassword,
  onLock,
  onLogout,
}: {
  /** The signed-in user, to find the doctor linked to this login (if any). */
  userId?: string;
  displayName: string;
  roleLabel: string;
  /** Only for people whose Settings menu item is hidden: opens "About the app". */
  onAbout?: () => void;
  onPassword: () => void;
  onLock: () => void;
  onLogout: () => void;
}) {
  const { t } = useI18n();
  const { theme, setTheme, perf, setPerf } = useTheme();
  const [open, setOpen] = useState(false);
  // OF-050: a user who is one of the clinic's doctors gets the doctor's picture; others keep the initial.
  const [doctor, setDoctor] = useState<DoctorInfo | null>(null);
  useEffect(() => {
    if (!userId) return;
    let live = true;
    const find = () =>
      rpc("doctors.list", { include_inactive: true })
        .then((list) => live && setDoctor(list.find((d) => d.user_id === userId) ?? null))
        .catch(() => live && setDoctor(null));
    find();
    window.addEventListener(DOCTORS_CHANGED, find);
    return () => {
      live = false;
      window.removeEventListener(DOCTORS_CHANGED, find);
    };
  }, [userId]);
  const close = (fn: () => void) => () => {
    setOpen(false);
    fn();
  };
  return (
    <Popover
      open={open}
      onClose={() => setOpen(false)}
      width={300}
      testId="user-menu"
      trigger={
        <button type="button" className="user-trigger" onClick={() => setOpen((o) => !o)} aria-expanded={open} data-testid="current-user">
          {doctor ? <DoctorPic doctor={doctor} name={displayName} size="md" testId="current-user-avatar" /> : <PersonAvatar kind="user" name={displayName} size="md" decorative testId="current-user-avatar" />}
          <span className="user-text">
            <span className="user-name">{displayName}</span>
            <span className="user-role">{roleLabel}</span>
          </span>
          <ChevronDown aria-hidden />
        </button>
      }
    >
      <div className="menu-section stack" style={{ gap: 10 }}>
        <span className="t-overline">{t("shell.appearance")}</span>
        <Segmented<ThemePreference>
          block
          value={theme}
          onChange={setTheme}
          label={t("shell.appearance")}
          options={[
            { value: "light", label: t("theme.light"), icon: Sun, testId: "theme-toggle-light" },
            { value: "dark", label: t("theme.dark"), icon: Moon, testId: "theme-toggle-dark" },
            { value: "system", label: t("theme.system"), icon: Monitor, testId: "theme-toggle-system" },
          ]}
        />
        <div className="row" style={{ justifyContent: "space-between", flexWrap: "nowrap" }}>
          <span className="row muted" style={{ fontSize: 13 }}><Gauge size={16} aria-hidden /> {t("shell.perf.label")}</span>
          <Switch checked={perf === "reduced"} onChange={(on) => setPerf(on ? "reduced" : "full")} label={<span className="visually-hidden">{t("shell.perf.label")}</span>} testId="perf-toggle" />
        </div>
        <span className="subtle t-caption">{t("shell.perf.hint")}</span>
      </div>
      <div className="menu-sep" />
      {onAbout && (
        <button type="button" className="menu-item" onClick={close(onAbout)} data-testid="menu-about">
          <Info aria-hidden /> {t("shell.about")}
        </button>
      )}
      <button type="button" className="menu-item" onClick={close(onPassword)} data-testid="change-password">
        <KeyRound aria-hidden /> {t("password.title")}
      </button>
      <button type="button" className="menu-item" onClick={close(onLock)} data-testid="lock">
        <LockKeyhole aria-hidden /> {t("shell.lock")}
      </button>
      <div className="menu-sep" />
      <button type="button" className="menu-item danger" onClick={close(onLogout)} data-testid="logout">
        <LogOut aria-hidden className="flip-rtl" /> {t("shell.logout")}
      </button>
    </Popover>
  );
}
