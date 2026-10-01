import { useEffect, useState } from "react";
import { Cpu, Database, DatabaseBackup, FolderOpen, GitCommitHorizontal, HardDrive, KeyRound, Layers, Monitor, ShieldCheck, ShieldAlert, Activity, Server, LifeBuoy } from "lucide-react";
import type { CalendarSystem, SystemInfo } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { digits, formatBytes, formatDateTime } from "../lib/dates";
import { useI18n } from "../i18n";
import { Card, CardHeader, Page, PageHeader, Stat } from "../ui/Card";
import { Badge, ErrorState, Loading, Notice } from "../ui/Feedback";
import { brandAssets } from "../ui/Brand";

/** Roadmap 1.8 + 2.1b: Artaveo Dental, version, support, and what the owner
 * needs to verify (CPU build, encryption, integrity, backup) without tools. */
export function SystemInfoPage({ version, calendar }: { version: string; calendar?: CalendarSystem }) {
  const { t, err, lang } = useI18n();
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState("");

  const load = () =>
    rpc("system.info", {})
      .then((i) => {
        setInfo(i);
        setError("");
      })
      .catch((e) => {
        // A locked/expired session is the Shell's business (lock screen), not this page's (OF-008).
        if (!isSessionError(e)) setError(err(e));
      });
  useEffect(() => {
    load();
    const id = window.setInterval(load, 5000); // integrity check finishes in the background
    return () => window.clearInterval(id);
  }, [lang]);

  if (error && !info) return <Page><ErrorState message={error} onRetry={load} /></Page>;
  if (!info) return <Page><Loading /></Page>;
  const db = info.database;
  const integrity = info.integrity.status;
  return (
    <Page testId="page-system">
      <PageHeader title={t("nav.system")} description={t("system.subtitle")} />

      <section className="glass about-hero">
        <div className="about-art"><img src={brandAssets.master} alt="Artaveo" /></div>
        <div className="about-text">
          <h2 className="t-title-lg">Artaveo Dental</h2>
          <p className="muted">{t("system.tagline")}</p>
          <div className="about-meta">
            <Badge tone="accent">{t("system.version")} <bdi className="ltr num" data-testid="si-version">{info.app_version}</bdi></Badge>
            <Badge icon={Cpu}><bdi className="ltr" data-testid="si-build-arch">{info.build_arch}</bdi></Badge>
            <Badge icon={Layers}>{t(`system.env.${info.environment}`)}</Badge>
          </div>
          <p className="row subtle t-caption" style={{ marginTop: 8 }}><LifeBuoy size={15} aria-hidden /> {t("system.support")}</p>
        </div>
      </section>

      {info.emulated && <Notice tone="warning" title={t("system.emulatedTitle")} testId="emulated-warning">{t("system.emulated")}</Notice>}

      <div className="stats">
        <Stat
          icon={db?.encrypted ? ShieldCheck : ShieldAlert}
          tone={db?.encrypted ? "success" : "danger"}
          label={t("system.encryption")}
          value={<span data-testid="si-encryption">{db?.encrypted ? t("system.encrypted") : t("system.notEncrypted")}</span>}
          sub={db && <bdi className="ltr">SQLCipher {db.cipher_version}</bdi>}
        />
        <Stat
          icon={Activity}
          tone={integrity === "ok" ? "success" : integrity === "failed" ? "danger" : "info"}
          label={t("system.integrity")}
          value={<span data-testid="si-integrity">{t(`system.integrity.${integrity}`)}</span>}
          sub={info.integrity.checked_at && formatDateTime(info.integrity.checked_at, lang, calendar)}
        />
        <Stat
          icon={DatabaseBackup}
          tone={info.last_backup ? "accent" : "warning"}
          label={t("system.lastBackup")}
          value={<span data-testid="si-last-backup">{info.last_backup ? formatDateTime(info.last_backup.created_at, lang, calendar) : t("system.noBackup")}</span>}
          sub={info.last_backup && <>{t("backup.size")}: <bdi className="ltr num">{formatBytes(info.last_backup.size_bytes, lang)}</bdi></>}
        />
      </div>

      <Card>
        <CardHeader icon={Server} title={t("system.details")} description={t("system.detailsHint")} />
        <dl className="kv" data-testid="system-info">
          <div><dt><GitCommitHorizontal aria-hidden />{t("system.commit")}</dt><dd><bdi className="ltr">{info.git_commit}</bdi></dd></div>
          <div><dt><Cpu aria-hidden />{t("system.buildArch")}</dt><dd><bdi className="ltr">{info.build_arch}</bdi></dd></div>
          <div><dt><Cpu aria-hidden />{t("system.machineArch")}</dt><dd><bdi className="ltr" data-testid="si-machine-arch">{info.machine_arch}</bdi></dd></div>
          <div><dt><Monitor aria-hidden />{t("system.os")}</dt><dd><bdi className="ltr">{info.os}</bdi></dd></div>
          <div><dt><HardDrive aria-hidden />{t("system.computer")}</dt><dd><bdi className="ltr">{info.computer_name}</bdi></dd></div>
          <div><dt><KeyRound aria-hidden />{t("system.keyProtection")}</dt><dd>{db ? t(`system.key.${db.key_protection}`) : "—"}</dd></div>
          <div><dt><Database aria-hidden />{t("system.schema")}</dt><dd className="num">{db ? digits(db.schema_version, lang) : "—"}</dd></div>
          <div><dt><Database aria-hidden />{t("system.dbSize")}</dt><dd><bdi className="ltr num">{db ? formatBytes(db.size_bytes, lang) : "—"}</bdi></dd></div>
          <div><dt><FolderOpen aria-hidden />{t("system.dataDir")}</dt><dd><bdi className="ltr">{info.data_dir}</bdi></dd></div>
        </dl>
      </Card>
      <p className="subtle t-caption" style={{ textAlign: "center" }}>
        Artaveo Dental <bdi className="ltr">{version}</bdi> · © Artaveo
      </p>
    </Page>
  );
}
