import { useEffect, useState } from "react";
import type { Settings } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { useI18n } from "../i18n";

export function SettingsPage() {
  const { t, err } = useI18n();
  const [s, setS] = useState<Settings | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  useEffect(() => {
    rpc("settings.get", {}).then(setS).catch((e) => setMsg({ ok: false, text: err(e) }));
  }, []);
  if (!s) return <div className="muted">{t("common.loading")}</div>;

  const num = (k: keyof Settings) => (
    <input type="number" className="ltr" value={s[k]} onChange={(e) => setS({ ...s, [k]: Number(e.target.value) })} data-testid={`setting-${k}`} />
  );
  const save = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      setS(await rpc("settings.update", s));
      setMsg({ ok: true, text: t("common.saved") });
    } catch (x) {
      setMsg({ ok: false, text: err(x) });
    }
  };
  return (
    <form className="card" onSubmit={save}>
      <h2>{t("settings.title")}</h2>
      <label>{t("settings.timeout")}</label>{num("session_timeout_minutes")}
      <label>{t("settings.backupHour")}</label>{num("daily_backup_hour")}
      <label>{t("settings.keep")}</label>{num("backup_keep_daily")}
      {msg && <div className={msg.ok ? "success" : "error"} data-testid="settings-message">{msg.text}</div>}
      <div className="actions"><button className="primary" type="submit" data-testid="settings-save">{t("common.save")}</button></div>
    </form>
  );
}
