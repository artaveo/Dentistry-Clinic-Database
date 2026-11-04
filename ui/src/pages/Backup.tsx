import { useCallback, useEffect, useState } from "react";
import { CalendarClock, Cloud, CircleCheck, CircleX, DatabaseBackup, FileArchive, HardDrive, HardDriveDownload, History, Usb } from "lucide-react";
import type { BackupEntry, BackupOverview, CalendarSystem, TargetState } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { digits, formatBytes, formatDateTime } from "../lib/dates";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Card, Page, PageHeader } from "../ui/Card";
import { Segmented } from "../ui/Controls";
import { Badge, EmptyState, ErrorState, Loading, Notice, SkeletonRows } from "../ui/Feedback";
import { useToast } from "../ui/Toast";
import { CloudSection } from "./backup/CloudSection";
import { CopyBadge, fileKindLabel, useBackupSettings } from "./backup/common";
import { RestoreSection } from "./backup/RestoreSection";
import { ScheduleSection } from "./backup/ScheduleSection";
import { UsbSection } from "./backup/UsbSection";

type Section = "history" | "usb" | "cloud" | "schedule" | "restore";
const POLL_MS = 5_000;

/**
 * Backup Manager (roadmap 10.2-10.7). The top answers the one question that matters ("is my data
 * safe, and where?") in three lines; everything else sits one tap away in the section switch.
 */
export function BackupPage({ canCreate, canRestore, calendar, onRestored }: { canCreate: boolean; canRestore: boolean; calendar?: CalendarSystem; onRestored: () => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [overview, setOverview] = useState<BackupOverview | null>(null);
  const [history, setHistory] = useState<BackupEntry[] | null>(null);
  const [section, setSection] = useState<Section>("history");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const { settings, setSettings } = useBackupSettings();

  const load = useCallback(() => {
    Promise.all([rpc("backup.overview", {}), rpc("backup.history", {})])
      .then(([o, h]) => {
        setOverview(o);
        setHistory(h);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    load();
    // A drive plugged in or a folder that came back changes the picture without any click.
    const id = window.setInterval(() => !document.hidden && load(), POLL_MS);
    return () => window.clearInterval(id);
  }, [load]);

  const backupNow = async () => {
    setBusy(true);
    try {
      await rpc("backup.create", {});
      toast.success(t("backup.done"));
      load();
    } catch (e) {
      toast.error(err(e));
    } finally {
      setBusy(false);
    }
  };

  if (error && !overview) return <Page testId="page-backup"><ErrorState message={error} onRetry={load} /></Page>;
  const when = (iso: string) => formatDateTime(iso, lang, calendar);
  const n = (x: number) => digits(x, lang);

  const stateBadge = (s: TargetState, notConfigured: string) =>
    s === "ok" ? <Badge tone="success" icon={CircleCheck}>{t("bk.state.ok")}</Badge>
    : s === "pending" ? <Badge tone="warning">{t("bk.state.pending")}</Badge>
    : s === "failed" ? <Badge tone="danger" icon={CircleX}>{t("bk.state.failed")}</Badge>
    : <Badge>{notConfigured}</Badge>;

  const sections = [
    { value: "history" as const, label: t("bk.section.history"), icon: History, testId: "bk-tab-history" },
    { value: "usb" as const, label: t("bk.section.usb"), icon: Usb, testId: "bk-tab-usb" },
    { value: "cloud" as const, label: t("bk.section.cloud"), icon: Cloud, testId: "bk-tab-cloud" },
    { value: "schedule" as const, label: t("bk.section.schedule"), icon: CalendarClock, testId: "bk-tab-schedule" },
    ...(canRestore ? [{ value: "restore" as const, label: t("bk.section.restore"), icon: HardDriveDownload, testId: "bk-tab-restore" }] : []),
  ];

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

      <Card data-testid="backup-destinations">
        {!overview ? (
          <Loading />
        ) : (
          <>
            <ul className="dest-list">
              <li className="dest-row" data-testid="dest-local">
                <HardDrive aria-hidden />
                <span className="dest-main">
                  <strong>{t("bk.dest.local")}</strong>
                  <span className="subtle t-caption">
                    {overview.last ? t("bk.dest.localInfo").replace("{time}", when(overview.last.created_at)).replace("{n}", n(overview.local_count)).replace("{size}", formatBytes(overview.local_bytes, lang)) : t("system.noBackup")}
                  </span>
                </span>
                {overview.last ? <Badge tone="success" icon={CircleCheck}>{t("bk.state.ok")}</Badge> : <Badge tone="warning">{t("bk.state.none")}</Badge>}
              </li>
              <li className="dest-row" data-testid="dest-usb">
                <Usb aria-hidden />
                <span className="dest-main">
                  <strong>{t("bk.dest.usb")}</strong>
                  <span className="subtle t-caption">
                    {overview.usb === "not_configured" ? t("bk.dest.usbNone") : overview.usb_connected ? t("bk.dest.usbConnected") : overview.usb_pending > 0 ? t("bk.dest.usbWaiting") : t("bk.dest.usbAway")}
                  </span>
                </span>
                {stateBadge(overview.usb, t("bk.state.notSet"))}
                <Button variant="subtle" onClick={() => setSection("usb")}>{overview.usb === "not_configured" ? t("bk.dest.setUp") : t("bk.dest.manage")}</Button>
              </li>
              <li className="dest-row" data-testid="dest-cloud">
                <Cloud aria-hidden />
                <span className="dest-main">
                  <strong>{t("bk.dest.cloud")}</strong>
                  <span className="subtle t-caption">
                    {overview.cloud === "not_configured" ? t("bk.dest.cloudNone") : overview.cloud_status.folder_exists ? t("bk.dest.cloudSync") : t("bk.dest.cloudMissing")}
                  </span>
                </span>
                {stateBadge(overview.cloud, t("bk.state.notSet"))}
                <Button variant="subtle" onClick={() => setSection("cloud")}>{overview.cloud === "not_configured" ? t("bk.dest.setUp") : t("bk.dest.manage")}</Button>
              </li>
            </ul>
            <p className="subtle t-caption dest-next" data-testid="backup-next">{t("bk.next").replace("{time}", when(overview.next_at))}</p>
          </>
        )}
      </Card>

      <Segmented label={t("bk.sections")} value={section} onChange={setSection} options={sections} />

      {section === "history" && (
        <Card flush>
          <div className="table-wrap">
            <table className="table" data-testid="backup-list">
              <thead>
                <tr><th>{t("backup.date")}</th><th>{t("backup.kind")}</th><th>{t("backup.size")}</th><th>{t("backup.verified")}</th><th>{t("bk.dest.usb")}</th><th>{t("bk.dest.cloud")}</th><th>{t("backup.file")}</th></tr>
              </thead>
              <tbody>
                {!history && <SkeletonRows cols={7} />}
                {history?.map((b) => (
                  <tr key={b.info.id}>
                    <td className="cell-strong">{when(b.info.created_at)}</td>
                    <td><Badge tone={b.info.kind === "manual" ? "accent" : b.info.kind === "daily" ? "info" : "neutral"}>{fileKindLabel(t, b.info.kind, b.info.file_name)}</Badge></td>
                    <td className="cell-num"><bdi className="ltr">{formatBytes(b.info.size_bytes, lang)}</bdi></td>
                    <td>{b.info.verified ? <Badge tone="success" icon={CircleCheck}>{t("backup.ok")}</Badge> : <Badge tone="danger" icon={CircleX}>{t("backup.notOk")}</Badge>}</td>
                    <td><CopyBadge status={b.usb} /></td>
                    <td><CopyBadge status={b.cloud} /></td>
                    <td className="subtle"><span className="row" style={{ flexWrap: "nowrap" }}><FileArchive size={16} aria-hidden /><bdi className="ltr">{b.info.file_name}</bdi></span></td>
                  </tr>
                ))}
              </tbody>
            </table>
            {history?.length === 0 && <EmptyState icon={DatabaseBackup} title={t("backup.empty")}>{t("backup.emptyHint")}</EmptyState>}
          </div>
        </Card>
      )}
      {section === "usb" && <UsbSection canManage={canCreate} calendar={calendar} onChanged={load} />}
      {section === "cloud" && (settings ? <CloudSection settings={settings} canEdit={canCreate} onSaved={(s) => { setSettings(s); load(); }} /> : <Loading />)}
      {section === "schedule" && (settings ? <ScheduleSection settings={settings} canEdit={canCreate} onSaved={(s) => { setSettings(s); load(); }} /> : <Loading />)}
      {section === "restore" && canRestore && <RestoreSection calendar={calendar} onRestored={onRestored} />}
      {section === "history" && !canRestore && <Notice tone="info">{t("bk.restoreOwnerOnly")}</Notice>}
    </Page>
  );
}
