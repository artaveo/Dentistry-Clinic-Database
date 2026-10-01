import { useEffect, useState } from "react";
import type { BackupInfo } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { formatBytes, formatDateTime } from "../lib/dates";
import { useI18n } from "../i18n";

export function BackupPage({ canCreate }: { canCreate: boolean }) {
  const { t, err, lang } = useI18n();
  const [list, setList] = useState<BackupInfo[]>([]);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const load = () => rpc("backup.list", {}).then(setList).catch((e) => setMsg({ ok: false, text: err(e) }));
  useEffect(() => {
    load();
  }, []);

  const backupNow = async () => {
    setBusy(true);
    setMsg(null);
    try {
      await rpc("backup.create", {});
      setMsg({ ok: true, text: t("backup.done") });
      await load();
    } catch (e) {
      setMsg({ ok: false, text: err(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card wide">
      <h2>{t("backup.title")}</h2>
      <p className="muted">{t("backup.hint")}</p>
      {canCreate && (
        <button className="primary" disabled={busy} onClick={backupNow} data-testid="backup-now">
          {busy ? t("backup.running") : t("backup.now")}
        </button>
      )}
      {msg && <div className={msg.ok ? "success" : "error"} data-testid="backup-message">{msg.text}</div>}
      <table data-testid="backup-list" style={{ marginTop: 16 }}>
        <thead>
          <tr><th>{t("backup.date")}</th><th>{t("backup.kind")}</th><th>{t("backup.size")}</th><th>{t("backup.verified")}</th><th>{t("backup.file")}</th></tr>
        </thead>
        <tbody>
          {list.map((b) => (
            <tr key={b.id}>
              <td>{formatDateTime(b.created_at, lang)}</td>
              <td>{t(`backup.kind.${b.kind}`)}</td>
              <td><bdi dir="ltr">{formatBytes(b.size_bytes, lang)}</bdi></td>
              <td>{b.verified ? "✓" : "✗"}</td>
              <td className="ltr">{b.file_name}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
