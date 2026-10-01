import { useState } from "react";
import { Bell, BellOff } from "lucide-react";
import { useI18n } from "../i18n";
import { IconButton } from "../ui/Button";
import { EmptyState } from "../ui/Feedback";
import { Popover } from "../ui/Overlay";

/** Notification Bell (roadmap 2.2): infrastructure only — real content
 * (appointment reminders, low stock, backup failures, …) is Phase 9. */
export function NotificationBell() {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <Popover
      open={open}
      onClose={() => setOpen(false)}
      width={340}
      testId="notification-popover"
      trigger={<IconButton icon={Bell} label={t("shell.notifications.title")} aria-expanded={open} onClick={() => setOpen((o) => !o)} data-testid="notification-bell" />}
    >
      <div className="menu-label row">
        <strong>{t("shell.notifications.title")}</strong>
      </div>
      <div className="menu-sep" />
      <EmptyState icon={BellOff} title={t("shell.notifications.empty")}>{t("shell.notifications.emptyHint")}</EmptyState>
    </Popover>
  );
}
