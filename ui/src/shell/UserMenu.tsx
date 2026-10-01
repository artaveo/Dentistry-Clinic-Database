import { useState } from "react";
import { ChevronDown, Gauge, KeyRound, LockKeyhole, LogOut, Monitor, Moon, Sun } from "lucide-react";
import type { ThemePreference } from "../../../shared/ts/contract";
import { useI18n } from "../i18n";
import { useTheme } from "../theme";
import { Avatar } from "../ui/Brand";
import { Segmented, Switch } from "../ui/Controls";
import { Popover } from "../ui/Overlay";

/** User menu (roadmap 2.2): who's signed in, appearance, performance mode, password, lock, sign out. */
export function UserMenu({
  displayName,
  roleLabel,
  onPassword,
  onLock,
  onLogout,
}: {
  displayName: string;
  roleLabel: string;
  onPassword: () => void;
  onLock: () => void;
  onLogout: () => void;
}) {
  const { t } = useI18n();
  const { theme, setTheme, perf, setPerf } = useTheme();
  const [open, setOpen] = useState(false);
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
          <Avatar name={displayName} />
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
