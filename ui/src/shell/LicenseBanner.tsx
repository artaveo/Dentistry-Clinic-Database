import { useCallback, useEffect, useState } from "react";
import { ShieldAlert } from "lucide-react";
import type { LicenseStatus } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";

const POLL_MS = 60_000;

/**
 * Roadmap 10.9: when the license no longer allows changes the whole app says so, once, at the top —
 * and in the same breath that nothing is lost and viewing, printing and backups still work.
 */
export function LicenseBanner({ canManage, onOpen }: { canManage: boolean; onOpen: () => void }) {
  const { t } = useI18n();
  const [status, setStatus] = useState<LicenseStatus | null>(null);
  const load = useCallback(() => {
    rpc("license.status", {}).then(setStatus).catch((e) => !isSessionError(e) && setStatus(null));
  }, []);
  useEffect(() => {
    load();
    const id = window.setInterval(() => !document.hidden && load(), POLL_MS);
    window.addEventListener("artaveo:license-changed", load);
    return () => {
      window.clearInterval(id);
      window.removeEventListener("artaveo:license-changed", load);
    };
  }, [load]);
  if (!status?.read_only) return null;
  return (
    <div className="license-banner" role="alert" data-testid="license-banner">
      <ShieldAlert aria-hidden />
      <span>
        <strong>{t(`lic.banner.${status.state}`)}</strong> {t("lic.banner.safe")}
      </span>
      {canManage && <Button size="sm" onClick={onOpen} data-testid="license-banner-open">{t("lic.banner.open")}</Button>}
    </div>
  );
}
