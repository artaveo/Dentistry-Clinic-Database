import { useCallback, useEffect, useState } from "react";
import { Users } from "lucide-react";
import type { PresenceViewer } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { useLive } from "../lib/lan";
import { useI18n } from "../i18n";

/** The screen reports "I have this record open" this often; the Core forgets silent screens after 2.5 minutes. */
const HEARTBEAT_MS = 45_000;

/**
 * Who else has this record open (roadmap 7.3). The screen announces itself while the record is
 * shown, and answers with the others, so two people notice each other before they overwrite
 * each other's work. Optimistic locking (`version`) still protects every save.
 */
export function usePresence(entity: string, id: string): PresenceViewer[] {
  const [others, setOthers] = useState<PresenceViewer[]>([]);

  const enter = useCallback(() => {
    rpc("presence.enter", { entity, id })
      .then(setOthers)
      .catch((e) => !isSessionError(e) && setOthers([]));
  }, [entity, id]);

  useEffect(() => {
    enter();
    const beat = window.setInterval(() => !document.hidden && enter(), HEARTBEAT_MS);
    return () => {
      window.clearInterval(beat);
      rpc("presence.leave", { entity, id }).catch(() => {});
    };
  }, [enter, entity, id]);

  useLive(["presence"], () => {
    rpc("presence.who", { entity, id })
      .then(setOthers)
      .catch(() => {});
  });
  return others;
}

export function PresenceBanner({ others }: { others: PresenceViewer[] }) {
  const { t } = useI18n();
  const shown = others.length > 0;
  const names = [...new Set(others.map((o) => o.user_name))].join("، ");
  return (
    <div role="status" aria-atomic="true" data-testid="presence">
      {shown && (
        <div className="presence-banner" data-testid="presence-banner">
          <Users aria-hidden />
          <span>{t("presence.alsoOpen").replace("{names}", names)}</span>
        </div>
      )}
    </div>
  );
}
