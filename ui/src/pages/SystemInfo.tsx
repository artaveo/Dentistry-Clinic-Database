import { useEffect, useState } from "react";
import type { SystemInfo } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { digits, formatBytes, formatDateTime } from "../lib/dates";
import { useI18n } from "../i18n";

/** Roadmap 1.8: lets the owner verify version, CPU build, encryption and backup without tools. */
export function SystemInfoPage() {
  const { t, err, lang } = useI18n();
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    const load = () => rpc("system.info", {}).then(setInfo).catch((e) => setError(err(e)));
    load();
    const id = window.setInterval(load, 5000); // integrity check finishes in the background
    return () => window.clearInterval(id);
  }, [lang]);

  if (error) return <div className="error">{error}</div>;
  if (!info) return <div className="muted">{t("common.loading")}</div>;
  const db = info.database;
  return (
    <div className="card wide">
      <h2>{t("system.title")}</h2>
      {info.emulated && <p className="warn" data-testid="emulated-warning">{t("system.emulated")}</p>}
      <dl data-testid="system-info">
        <dt>{t("system.version")}</dt><dd className="ltr" data-testid="si-version">{info.app_version}</dd>
        <dt>{t("system.commit")}</dt><dd className="ltr">{info.git_commit}</dd>
        <dt>{t("system.buildArch")}</dt><dd className="ltr" data-testid="si-build-arch">{info.build_arch}</dd>
        <dt>{t("system.machineArch")}</dt><dd className="ltr" data-testid="si-machine-arch">{info.machine_arch}</dd>
        <dt>{t("system.os")}</dt><dd className="ltr">{info.os}</dd>
        <dt>{t("system.computer")}</dt><dd className="ltr">{info.computer_name}</dd>
        <dt>{t("system.environment")}</dt><dd className="ltr">{info.environment}</dd>
        <dt>{t("system.encryption")}</dt>
        <dd data-testid="si-encryption">
          {db?.encrypted ? t("system.encrypted") : t("system.notEncrypted")} <span className="ltr muted">SQLCipher {db?.cipher_version} · SQLite {db?.sqlite_version}</span>
        </dd>
        <dt>{t("system.keyProtection")}</dt><dd>{db ? t(`system.key.${db.key_protection}`) : "—"}</dd>
        <dt>{t("system.schema")}</dt><dd>{db ? digits(db.schema_version, lang) : "—"}</dd>
        <dt>{t("system.dbSize")}</dt><dd><bdi dir="ltr">{db ? formatBytes(db.size_bytes, lang) : "—"}</bdi></dd>
        <dt>{t("system.integrity")}</dt><dd data-testid="si-integrity">{t(`system.integrity.${info.integrity.status}`)} {info.integrity.checked_at && <span className="muted">{formatDateTime(info.integrity.checked_at, lang)}</span>}</dd>
        <dt>{t("system.lastBackup")}</dt>
        <dd data-testid="si-last-backup">{info.last_backup ? <>{formatDateTime(info.last_backup.created_at, lang)} · <bdi dir="ltr">{formatBytes(info.last_backup.size_bytes, lang)}</bdi></> : t("system.noBackup")}</dd>
        <dt>{t("system.dataDir")}</dt><dd className="ltr">{info.data_dir}</dd>
      </dl>
    </div>
  );
}
