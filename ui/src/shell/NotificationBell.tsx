import { useCallback, useEffect, useRef, useState } from "react";
import { Bell, BellOff, CheckCheck, Info, TriangleAlert } from "lucide-react";
import type { CalendarSystem, NotificationItem, NotificationList } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { digits, formatDate, formatTime } from "../lib/dates";
import { tr } from "../lib/medical";
import { formatAmount } from "../lib/money";
import { useI18n } from "../i18n";
import type { Language } from "../../../shared/ts/contract";
import { Button, IconButton } from "../ui/Button";
import { Switch } from "../ui/Controls";
import { EmptyState } from "../ui/Feedback";
import { Popover } from "../ui/Overlay";

/** How often the list is asked for again while the app is open. */
const POLL_MS = 60_000;
const OS_KEY = "artaveo.osNotifications";

function osEnabled(): boolean {
  try {
    return localStorage.getItem(OS_KEY) === "1" && typeof Notification !== "undefined" && Notification.permission === "granted";
  } catch {
    return false;
  }
}

/** The sentence of one notification, in the user's language, from the Core's typed pieces. */
export function notificationText(i: NotificationItem, t: (k: string) => string, lang: Language, calendar: CalendarSystem): string {
  let detail = i.detail ?? "";
  if (i.kind === "installment_due" && detail) detail = formatAmount(Number(detail), lang);
  else if ((i.kind === "lab_delayed" || i.kind === "backup_overdue" || i.kind === "usb_missing" || i.kind === "license_expiring" || i.kind === "license_verify") && detail) detail = digits(detail, lang);
  else if (i.kind === "appointment_reminder" && detail) detail = formatTime(detail, lang);
  const key = i.kind === "backup_overdue" && !i.detail ? "notif.msg.backup_never" : `notif.msg.${i.kind}`;
  return t(key)
    .replace("{subject}", tr(i.subject, lang))
    .replace("{detail}", detail)
    .replace("{date}", i.date ? formatDate(i.date, lang, calendar) : "");
}

/** Notification Bell (roadmap 2.2) with its content (9.6): what needs attention, quietest changes first read. */
export function NotificationBell({ calendar, go }: { calendar: CalendarSystem; go: (page: string) => void }) {
  const { t, lang } = useI18n();
  const [open, setOpen] = useState(false);
  const [list, setList] = useState<NotificationList | null>(null);
  const [os, setOs] = useState(osEnabled());
  const seen = useRef<Set<string> | null>(null);
  // `t` is a new function on every render; the poll must not restart each time, so it reads the latest through a ref.
  const latest = useRef({ t, lang, calendar });
  latest.current = { t, lang, calendar };

  const load = useCallback(() => {
    rpc("notifications.list", {})
      .then((l) => {
        setList(l);
        // A system toast only for what is new since the last look, never for the backlog at start-up.
        const fresh = l.items.filter((i) => !i.read && seen.current && !seen.current.has(i.key));
        seen.current = new Set(l.items.map((i) => i.key));
        if (osEnabled()) {
          for (const i of fresh.slice(0, 3)) {
            const { t, lang, calendar } = latest.current;
            try {
              new Notification(t(`notif.kind.${i.kind}`), { body: notificationText(i, t, lang, calendar), tag: i.key });
            } catch {
              /* the system may refuse; the bell still shows it */
            }
          }
        }
      })
      .catch((e) => {
        if (!isSessionError(e)) setList((x) => x);
      });
  }, []);

  useEffect(() => {
    load();
    const id = window.setInterval(load, POLL_MS);
    return () => window.clearInterval(id);
  }, [load]);
  useEffect(() => {
    if (open) load();
  }, [open, load]);

  const read = (keys: string[]) =>
    rpc("notifications.mark_read", { keys })
      .then(setList)
      .catch(() => {});
  const unread = list?.unread ?? 0;
  const label = unread > 0 ? t("shell.notifications.unreadLabel").replace("{n}", digits(unread, lang)) : t("shell.notifications.title");

  const toggleOs = async (on: boolean) => {
    try {
      if (on && typeof Notification !== "undefined" && Notification.permission !== "granted") {
        if ((await Notification.requestPermission()) !== "granted") return;
      }
      localStorage.setItem(OS_KEY, on ? "1" : "0");
    } catch {
      return;
    }
    setOs(on);
  };

  return (
    <Popover
      open={open}
      onClose={() => setOpen(false)}
      width={380}
      testId="notification-popover"
      trigger={
        <span className="bell-wrap">
          <IconButton icon={Bell} label={label} aria-expanded={open} onClick={() => setOpen((o) => !o)} data-testid="notification-bell" />
          {unread > 0 && <span className="bell-badge num" aria-hidden data-testid="notification-count">{unread > 99 ? "99+" : digits(unread, lang)}</span>}
          {/* One calm announcement of the whole fact, never a bare number. */}
          <span className="sr-only" role="status" aria-atomic="true">{unread > 0 ? label : ""}</span>
        </span>
      }
    >
      <div className="menu-label row" style={{ justifyContent: "space-between" }}>
        <strong>{t("shell.notifications.title")}</strong>
        {unread > 0 && <Button size="sm" variant="subtle" icon={CheckCheck} onClick={() => read([])} data-testid="notification-read-all">{t("shell.notifications.readAll")}</Button>}
      </div>
      <div className="menu-sep" />
      {!list || list.items.length === 0 ? (
        <EmptyState icon={BellOff} title={t("shell.notifications.empty")}>{t("shell.notifications.emptyHint")}</EmptyState>
      ) : (
        <ul className="notif-list" data-testid="notification-list">
          {list.items.map((i) => (
            <li key={i.key}>
              <button
                type="button"
                className="notif-item"
                data-read={i.read || undefined}
                data-level={i.level}
                onClick={() => {
                  if (!i.read) read([i.key]);
                  setOpen(false);
                  go(i.page);
                }}
                data-testid={`notification-${i.kind}`}
              >
                <span className="notif-icon" aria-hidden>{i.level === "info" ? <Info /> : <TriangleAlert />}</span>
                <span className="notif-body">
                  <span className="notif-kind">
                    {t(`notif.kind.${i.kind}`)}
                    {i.level === "danger" && <em>{t("notif.urgent")}</em>}
                    {!i.read && <span className="notif-dot" aria-label={t("shell.notifications.new")} />}
                  </span>
                  <span className="notif-text">{notificationText(i, t, lang, calendar)}</span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {typeof Notification !== "undefined" && (
        <>
          <div className="menu-sep" />
          <div className="notif-foot">
            <Switch checked={os} onChange={toggleOs} label={t("shell.notifications.os")} testId="notification-os" />
          </div>
        </>
      )}
    </Popover>
  );
}
