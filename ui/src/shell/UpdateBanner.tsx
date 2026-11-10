import { useCallback, useEffect, useState } from "react";
import { Download } from "lucide-react";
import { rpc } from "../lib/api";
import { reportUiError } from "../lib/diagnostics";
import { updateCheck, updateInstall } from "../lib/safe";
import { isDesktop } from "../print/engine";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";

const FIRST_CHECK_MS = 30_000;
const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

/**
 * Roadmap 11.2: when a newer signed version exists, the owner is told once, at the top. Installing is
 * Safe (a backup is made first and kept) and the window never installs anything on its own. Quiet when
 * the computer is offline. Only production builds look for updates.
 */
export function UpdateBanner({ canManage }: { canManage: boolean }) {
  const { t } = useI18n();
  const [version, setVersion] = useState<string | null>(null);
  const [step, setStep] = useState<"" | "backup" | "install">("");
  const [failed, setFailed] = useState(false);

  const check = useCallback(async () => {
    try {
      const status = await rpc("app.status", {});
      if (status.environment !== "production") return;
      const u = await updateCheck();
      setVersion(u.available ? u.version : null);
    } catch {
      /* offline or no release yet: nothing to show */
    }
  }, []);

  useEffect(() => {
    if (!isDesktop() || !canManage) return;
    const first = window.setTimeout(check, FIRST_CHECK_MS);
    const every = window.setInterval(check, CHECK_EVERY_MS);
    return () => {
      window.clearTimeout(first);
      window.clearInterval(every);
    };
  }, [canManage, check]);

  const install = async () => {
    setFailed(false);
    try {
      setStep("backup");
      await rpc("update.prepare", {});
      setStep("install");
      await updateInstall();
    } catch (e) {
      reportUiError("update install", e);
      setFailed(true);
      setStep("");
    }
  };

  if (!version) return null;
  return (
    <div className="license-banner" role="status" data-testid="update-banner">
      <Download aria-hidden />
      <span>
        <strong>{t("update.available").replace("{v}", version)}</strong>{" "}
        {step === "backup" ? t("update.backup") : step === "install" ? t("update.installing") : failed ? t("update.failed") : t("update.safe")}
      </span>
      <Button size="sm" variant="primary" loading={step !== ""} onClick={install} data-testid="update-install">{t("update.install")}</Button>
    </div>
  );
}
