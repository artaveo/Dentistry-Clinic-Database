import { useEffect } from "react";
import { LoaderCircle, ServerOff } from "lucide-react";
import { useI18n } from "../i18n";

/**
 * Shown instead of the app while a client computer cannot reach the clinic server (roadmap 7.2):
 * a clear sentence, and it keeps trying by itself, so the front desk never has to "restart".
 */
export function ServerOffline({ onRetry }: { onRetry: () => void }) {
  const { t } = useI18n();
  useEffect(() => {
    const id = window.setInterval(onRetry, 3_000);
    return () => window.clearInterval(id);
  }, [onRetry]);
  return (
    <div className="auth">
      <div className="glass auth-card server-offline" data-testid="server-offline">
        <div className="empty" role="status" aria-atomic="true">
          <span className="empty-icon"><ServerOff aria-hidden /></span>
          <span className="empty-title">{t("conn.offline.title")}</span>
          <span>{t("conn.offline.body")}</span>
          <span className="row" style={{ justifyContent: "center", color: "var(--text-3)" }}>
            <LoaderCircle className="spin" size={16} aria-hidden />
            <span className="t-caption">{t("conn.offline.waiting")}</span>
          </span>
        </div>
      </div>
    </div>
  );
}
