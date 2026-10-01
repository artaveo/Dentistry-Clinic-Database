import { useEffect, useState } from "react";
import { Archive, CalendarClock, CircleCheck, CircleX, DatabaseBackup, FileArchive, History } from "lucide-react";
import type { BackupInfo, CalendarSystem, Settings } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { digits, formatBytes, formatDateTime, formatHour } from "../lib/dates";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Card, Page, PageHeader, Stat } from "../ui/Card";
import { Badge, EmptyState, ErrorState, SkeletonRows } from "../ui/Feedback";
import { useToast } from "../ui/Toast";

export function BackupPage({ canCreate, calendar }: { canCreate: boolean; calendar?: CalendarSystem }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [list, setList] = useState<BackupInfo[] | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const load = () =>
    rpc("backup.list", {})
      .then((l) => {
        setList(l);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
    rpc("settings.get", {}).then(setSettings).catch(() => {});
  }, []);

  const backupNow = async () => {
    setBusy(true);
    try {
      await rpc("backup.create", {});
      toast.success(t("backup.done"));
      await load();
    } catch (e) {
      toast.error(err(e));
    } finally {
      setBusy(false);
    }
  };

  const last = list?.[0];
  return (
    <Page testId="page-backup">
      <PageHeader
        title={t("backup.title")}
        description={t("backup.hint")}
        actions={canCreate && (
          <Button variant="primary" icon={DatabaseBackup} loading={busy} onClick={backupNow} data-testid="backup-now">
            {busy ? t("backup.running") : t("backup.now")}
          </Button>
        )}
      />
      <div className="stats">
        <Stat icon={History} label={t("system.lastBackup")} tone={last ? "accent" : "warning"} value={last ? formatDateTime(last.created_at, lang, calendar) : t("system.noBackup")} sub={last && <>{t("backup.size")}: <bdi className="ltr num">{formatBytes(last.size_bytes, lang)}</bdi></>} />
        <Stat icon={CalendarClock} label={t("backup.daily")} tone="info" value={settings ? t("backup.dailyAt").replace("{time}", formatHour(settings.daily_backup_hour, lang)) : "—"} sub={t("backup.dailyHint")} />
        <Stat icon={Archive} label={t("backup.kept")} tone="success" value={settings ? t("backup.keptValue").replace("{n}", digits(settings.backup_keep_daily, lang)) : "—"} sub={t("backup.keptHint")} />
      </div>
      <Card flush>
        {error ? (
          <ErrorState message={error} onRetry={load} />
        ) : (
          <div className="table-wrap">
            <table className="table" data-testid="backup-list">
              <thead>
                <tr><th>{t("backup.date")}</th><th>{t("backup.kind")}</th><th>{t("backup.size")}</th><th>{t("backup.verified")}</th><th>{t("backup.file")}</th></tr>
              </thead>
              <tbody>
                {!list && <SkeletonRows cols={5} />}
                {list?.map((b) => (
                  <tr key={b.id}>
                    <td className="cell-strong">{formatDateTime(b.created_at, lang, calendar)}</td>
                    <td><Badge tone={b.kind === "manual" ? "accent" : b.kind === "daily" ? "info" : "neutral"}>{t(`backup.kind.${b.kind}`)}</Badge></td>
                    <td className="cell-num"><bdi className="ltr">{formatBytes(b.size_bytes, lang)}</bdi></td>
                    <td>{b.verified ? <Badge tone="success" icon={CircleCheck}>{t("backup.ok")}</Badge> : <Badge tone="danger" icon={CircleX}>{t("backup.notOk")}</Badge>}</td>
                    <td className="subtle"><span className="row" style={{ flexWrap: "nowrap" }}><FileArchive size={16} aria-hidden /><bdi className="ltr">{b.file_name}</bdi></span></td>
                  </tr>
                ))}
              </tbody>
            </table>
            {list?.length === 0 && <EmptyState icon={DatabaseBackup} title={t("backup.empty")}>{t("backup.emptyHint")}</EmptyState>}
          </div>
        )}
      </Card>
    </Page>
  );
}
