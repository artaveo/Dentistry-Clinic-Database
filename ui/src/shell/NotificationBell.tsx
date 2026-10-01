import { useState } from "react";
import { useI18n } from "../i18n";

/** Notification Bell (roadmap 2.2): infrastructure only — real content
 * (appointment reminders, low stock, backup failures, …) is Phase 9. */
export function NotificationBell() {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <div style={{ position: "relative" }}>
      <button
        type="button"
        className="ghost icon-button"
        aria-label={t("shell.notifications.title")}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        data-testid="notification-bell"
      >
        🔔
      </button>
      {open && (
        <div className="popover" role="dialog" data-testid="notification-popover">
          <strong>{t("shell.notifications.title")}</strong>
          <div className="empty-state">{t("shell.notifications.empty")}</div>
        </div>
      )}
    </div>
  );
}
