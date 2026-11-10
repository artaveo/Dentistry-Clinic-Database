import { useEffect, useRef, useState } from "react";
import { PlugZap, ServerOff, Download } from "lucide-react";
import { CONNECTION_DOM_EVENT, UNREACHABLE_DOM_EVENT, onConnection, unpairFromServer } from "../lib/lan";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { useToast } from "../ui/Toast";

type State = "ok" | "offline" | "revoked" | "incompatible";

/**
 * A client computer's link to the clinic server (roadmap 7.2). When the server computer is off the
 * screen says so in one calm bar and keeps trying; nothing is lost, because every saved change is
 * on the server. When the link comes back the pages reload what they show.
 */
export function ConnectionBanner({ onReconnected }: { onReconnected: () => void }) {
  const { t } = useI18n();
  const toast = useToast();
  const [state, setState] = useState<State>("ok");
  const previous = useRef<State>("ok");
  const [serverVersion, setServerVersion] = useState("");

  useEffect(() => {
    const offConn = onConnection((s) => {
      setServerVersion(s.incompatible ?? "");
      setState(s.revoked ? "revoked" : s.incompatible ? "incompatible" : s.online ? "ok" : "offline");
    });
    const unreachable = () => setState((x) => (x === "revoked" ? x : "offline"));
    window.addEventListener(UNREACHABLE_DOM_EVENT, unreachable);
    return () => {
      offConn();
      window.removeEventListener(UNREACHABLE_DOM_EVENT, unreachable);
    };
  }, []);

  useEffect(() => {
    if (previous.current === "offline" && state === "ok") {
      toast.success(t("conn.back"));
      onReconnected();
    }
    previous.current = state;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state]);

  const forget = async () => {
    try {
      await unpairFromServer();
    } finally {
      window.location.reload();
    }
  };

  return (
    <div role="status" aria-atomic="true" data-testid="connection-status">
      {state === "offline" && (
        <div className="conn-banner" data-testid="conn-offline">
          <ServerOff aria-hidden />
          <span><strong>{t("conn.offline.title")}</strong> {t("conn.offline.body")}</span>
        </div>
      )}
      {state === "incompatible" && (
        <div className="conn-banner danger" data-testid="conn-incompatible">
          <Download aria-hidden />
          <span><strong>{t("conn.incompatible.title")}</strong> {t("conn.incompatible.body").replace("{v}", serverVersion)}</span>
        </div>
      )}
      {state === "revoked" && (
        <div className="conn-banner danger" data-testid="conn-revoked">
          <PlugZap aria-hidden />
          <span><strong>{t("conn.revoked.title")}</strong> {t("conn.revoked.body")}</span>
          <span className="spacer" />
          <Button size="sm" onClick={forget} data-testid="conn-forget">{t("lan.client.disconnect")}</Button>
        </div>
      )}
    </div>
  );
}

/** Test hook: the same message the desktop shell sends. */
export function simulateConnection(online: boolean, revoked = false) {
  window.dispatchEvent(new CustomEvent(CONNECTION_DOM_EVENT, { detail: { online, revoked } }));
}
