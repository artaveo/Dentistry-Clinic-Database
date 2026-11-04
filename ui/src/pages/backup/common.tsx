import { useCallback, useEffect, useState } from "react";
import { CircleCheck, CircleX, Clock3, Minus } from "lucide-react";
import type { BackupSettings, CopyStatus } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { useI18n } from "../../i18n";
import { Badge } from "../../ui/Feedback";

/** The backup plan, shared by the schedule and cloud sections so each one saves the whole object. */
export function useBackupSettings() {
  const { err } = useI18n();
  const [settings, setSettings] = useState<BackupSettings | null>(null);
  const [error, setError] = useState("");
  const load = useCallback(() => {
    rpc("backup.settings_get", {})
      .then((s) => {
        setSettings(s);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(load, [load]);
  return { settings, setSettings, error, reload: load };
}

/** What happened to the USB or cloud copy of one backup. */
export function CopyBadge({ status }: { status: CopyStatus | null }) {
  const { t } = useI18n();
  if (!status) return <span className="subtle" aria-label={t("bk.copy.none")}>—</span>;
  if (status === "done") return <Badge tone="success" icon={CircleCheck}>{t("bk.copy.done")}</Badge>;
  if (status === "pending") return <Badge tone="warning" icon={Clock3}>{t("bk.copy.pending")}</Badge>;
  if (status === "failed") return <Badge tone="danger" icon={CircleX}>{t("bk.copy.failed")}</Badge>;
  return <Badge icon={Minus}>{t("bk.copy.skipped")}</Badge>;
}

export function fileKindLabel(t: (k: string) => string, kind: string, fileName: string): string {
  if (fileName.includes("-prerestore.")) return t("bk.kind.prerestore");
  return t(`backup.kind.${kind}`);
}
