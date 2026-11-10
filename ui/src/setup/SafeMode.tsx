import { useState } from "react";
import { FileText, KeyRound, RefreshCw, ShieldAlert, History } from "lucide-react";
import { useI18n } from "../i18n";
import { formatDateTime } from "../lib/dates";
import { revealExport } from "../lib/api";
import { safeEnterRestore, safeRecover, safeReport, safeRetry, type StartupState } from "../lib/safe";
import { Button } from "../ui/Button";
import { Notice } from "../ui/Feedback";
import { RestoreFromBackup } from "./RestoreFromBackup";

type Safe = Extract<StartupState, { safe: true }>;

/**
 * Safe Startup Mode (roadmap 11.3). The clinic did not open: nothing is changed on its own. The user
 * can try again, bring back the newest valid backup of this computer (the damaged files are kept in the
 * `quarantine` folder), restore a backup with the Recovery Key, or write a report for support.
 */
export function SafeMode({ state, onResolved }: { state: Safe; onResolved: () => void }) {
  const { t, lang } = useI18n();
  const [restoring, setRestoring] = useState(state.recovering);
  const [busy, setBusy] = useState<"" | "retry" | "recover" | "report">("");
  const [message, setMessage] = useState<{ tone: "success" | "danger" | "info"; text: string } | null>(null);

  const retry = async () => {
    setBusy("retry");
    setMessage(null);
    try {
      const s = await safeRetry();
      if (!s.safe) onResolved();
      else setMessage({ tone: "danger", text: t("safe.stillFailing") });
    } finally {
      setBusy("");
    }
  };

  const recover = async () => {
    setBusy("recover");
    setMessage(null);
    try {
      const r = await safeRecover();
      if (r.ok) onResolved();
      else setMessage({ tone: "danger", text: t("safe.stillFailing") });
    } catch (e) {
      setMessage({ tone: "danger", text: String(e).includes("no_valid_backup") ? t("safe.noBackup") : t("error.internal") });
    } finally {
      setBusy("");
    }
  };

  const report = async () => {
    setBusy("report");
    try {
      const r = await safeReport();
      await revealExport(r.path);
      setMessage({ tone: "success", text: `${t("safe.reportDone")} ${r.file_name}` });
    } catch {
      setMessage({ tone: "danger", text: t("error.internal") });
    } finally {
      setBusy("");
    }
  };

  const openRestore = async () => {
    await safeEnterRestore();
    setRestoring(true);
  };

  if (restoring) {
    return (
      <div className="auth">
        <div className="glass auth-card" data-testid="safe-restore">
          <RestoreFromBackup
            onBack={() => setRestoring(false)}
            onDone={async () => {
              const s = await safeRetry();
              if (!s.safe) onResolved();
              else setRestoring(false);
            }}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="auth">
      <div className="glass auth-card stack" role="alert" data-testid="safe-mode">
        <div className="setup-head">
          <h1 className="t-title-lg"><ShieldAlert aria-hidden /> {t("safe.title")}</h1>
          <p className="muted">{t("safe.subtitle")}</p>
        </div>
        <Notice tone="warning" title={t(`safe.reason.${state.reason}`)} testId="safe-reason">
          <span className="subtle">{t("safe.dataKept")}</span>
        </Notice>
        {message && <Notice tone={message.tone} testId="safe-message">{message.text}</Notice>}
        <div className="stack">
          <Button variant="primary" icon={RefreshCw} loading={busy === "retry"} onClick={retry} data-testid="safe-retry">{t("safe.retry")}</Button>
          {state.can_recover && (
            <>
              <Button icon={History} loading={busy === "recover"} onClick={recover} data-testid="safe-recover">{t("safe.recover")}</Button>
              <span className="subtle t-caption">{t("safe.recoverHint")}</span>
            </>
          )}
          {state.reason !== "newer_than_app" && (
            <Button icon={KeyRound} onClick={openRestore} data-testid="safe-restore-key">{t("safe.restoreKey")}</Button>
          )}
          <Button variant="subtle" icon={FileText} loading={busy === "report"} onClick={report} data-testid="safe-report">{t("safe.report")}</Button>
        </div>
        <details>
          <summary className="subtle t-caption">{t("safe.detail")}</summary>
          <bdi className="ltr t-caption" data-testid="safe-detail">{state.detail}</bdi>
        </details>
        <span className="subtle t-caption">{formatDateTime(new Date().toISOString(), lang)}</span>
      </div>
    </div>
  );
}
