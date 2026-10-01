import { useState } from "react";
import type { ThemePreference } from "../../../shared/ts/contract";
import { useI18n } from "../i18n";
import { useTheme } from "../theme";

/** User menu (roadmap 2.2): who's signed in, theme + performance mode, lock, sign out. */
export function UserMenu({
  displayName,
  roleLabel,
  onLock,
  onLogout,
}: {
  displayName: string;
  roleLabel: string;
  onLock: () => void;
  onLogout: () => void;
}) {
  const { t } = useI18n();
  const { theme, setTheme, perf, setPerf } = useTheme();
  const [open, setOpen] = useState(false);
  return (
    <div style={{ position: "relative" }}>
      <button type="button" className="ghost" onClick={() => setOpen((o) => !o)} aria-expanded={open} data-testid="current-user">
        {displayName} ({roleLabel})
      </button>
      {open && (
        <div className="popover" data-testid="user-menu">
          <div className="muted" style={{ marginBottom: 8 }}>{t("shell.appearance")}</div>
          <div className="segmented" role="radiogroup" aria-label={t("shell.appearance")}>
            {(["light", "dark", "system"] as ThemePreference[]).map((th) => (
              <button key={th} type="button" aria-pressed={theme === th} onClick={() => setTheme(th)} data-testid={`theme-toggle-${th}`}>
                {t(`theme.${th}`)}
              </button>
            ))}
          </div>
          <label style={{ marginTop: 12 }}>{t("shell.perf.label")}</label>
          <select value={perf} onChange={(e) => setPerf(e.target.value as "full" | "reduced")} data-testid="perf-toggle">
            <option value="full">{t("shell.perf.full")}</option>
            <option value="reduced">{t("shell.perf.reduced")}</option>
          </select>
          <div className="actions">
            <button
              onClick={() => {
                setOpen(false);
                onLock();
              }}
              data-testid="lock"
            >
              {t("shell.lock")}
            </button>
            <button
              onClick={() => {
                setOpen(false);
                onLogout();
              }}
              data-testid="logout"
            >
              {t("shell.logout")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
