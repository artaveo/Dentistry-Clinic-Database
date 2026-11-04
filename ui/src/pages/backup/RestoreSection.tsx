import { useEffect, useState } from "react";
import { FileArchive, HardDriveDownload, History, TriangleAlert } from "lucide-react";
import type { BackupSource, CalendarSystem, RestoreCandidate, RestorePreview } from "../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../lib/api";
import { digits, formatBytes, formatDateTime } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Checkbox, Segmented } from "../../ui/Controls";
import { Badge, EmptyState, Loading, Notice } from "../../ui/Feedback";
import { Field, PasswordInput } from "../../ui/Field";
import { useToast } from "../../ui/Toast";

/**
 * Restore (roadmap 10.7): choose a backup, see what is in it next to what is here now, then confirm
 * with the Owner's password. A backup that fails validation can not be restored, and a safety copy of
 * the current data is made first, so a wrong choice can be undone.
 */
export function RestoreSection({ calendar, onRestored }: { calendar?: CalendarSystem; onRestored: () => void }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const [source, setSource] = useState<BackupSource>("local");
  const [list, setList] = useState<RestoreCandidate[] | null>(null);
  const [chosen, setChosen] = useState<RestoreCandidate | null>(null);
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  const [password, setPassword] = useState("");
  const [passwordError, setPasswordError] = useState("");
  const [sure, setSure] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    setList(null);
    setChosen(null);
    setPreview(null);
    rpc("backup.restore_list", { source })
      .then(setList)
      .catch((e) => !isSessionError(e) && setError(err(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source]);

  const choose = (c: RestoreCandidate) => {
    setChosen(c);
    setPreview(null);
    setError("");
    setPassword("");
    setSure(false);
    rpc("backup.restore_preview", { source: c.source, file_name: c.file_name })
      .then(setPreview)
      .catch((e) => !isSessionError(e) && setError(err(e)));
  };

  const restore = async () => {
    if (!chosen) return;
    if (!password) return setPasswordError(t("rule.required"));
    setBusy(true);
    setPasswordError("");
    try {
      await rpc("backup.restore", { source: chosen.source, file_name: chosen.file_name, password });
      toast.success(t("bk.restore.done"));
      onRestored();
    } catch (e) {
      if (e instanceof ApiError && e.field === "password") setPasswordError(t(e.rule ? `rule.${e.rule}` : "rule.wrong_password"));
      else toast.error(err(e));
    } finally {
      setBusy(false);
    }
  };

  const when = (iso: string | null) => (iso ? formatDateTime(iso, lang, calendar) : "—");
  const n = (x: number) => digits(x, lang);

  return (
    <div className="stack-lg" data-testid="restore-section">
      <Card>
        <CardHeader icon={HardDriveDownload} title={t("bk.restore.title")} description={t("bk.restore.hint")} />
        <Segmented
          label={t("bk.restore.from")}
          value={source}
          onChange={setSource}
          options={[
            { value: "local", label: t("bk.restore.local"), testId: "restore-src-local" },
            { value: "usb", label: t("bk.restore.usb"), testId: "restore-src-usb" },
            { value: "cloud", label: t("bk.restore.cloud"), testId: "restore-src-cloud" },
          ]}
        />
        {error && <Notice tone="danger">{error}</Notice>}
        {!list ? (
          <Loading />
        ) : list.length === 0 ? (
          <EmptyState icon={FileArchive} title={t("bk.restore.none")}>{t(`bk.restore.none.${source}`)}</EmptyState>
        ) : (
          <ul className="plain-list restore-list" role="radiogroup" aria-label={t("bk.restore.pick")} data-testid="restore-list">
            {list.slice(0, 40).map((c) => (
              <li key={`${c.folder}/${c.file_name}`}>
                <button type="button" role="radio" aria-checked={chosen?.file_name === c.file_name && chosen.folder === c.folder} className="restore-pick" onClick={() => choose(c)} data-testid="restore-item">
                  <FileArchive aria-hidden />
                  <span className="restore-main">
                    <strong>{when(c.created_at)}</strong>
                    <span className="subtle t-caption">{c.file_name.includes("-prerestore.") ? t("bk.kind.prerestore") : t(c.file_name.includes("-daily.") ? "backup.kind.daily" : "backup.kind.manual")}</span>
                  </span>
                  <bdi className="ltr num subtle">{formatBytes(c.size_bytes, lang)}</bdi>
                  {!c.has_key && <Badge tone="warning">{t("bk.restore.noKey")}</Badge>}
                </button>
              </li>
            ))}
          </ul>
        )}
      </Card>

      {chosen && (
        <Card data-testid="restore-preview">
          <CardHeader icon={History} title={t("bk.restore.compare")} description={t("bk.restore.compareHint")} />
          {!preview ? (
            <Loading />
          ) : !preview.valid ? (
            <Notice tone="danger" title={t("bk.restore.invalid")} testId="restore-invalid">{t(`bk.restore.problem.${preview.problem ?? "unreadable"}`)}</Notice>
          ) : (
            <>
              <div className="table-wrap">
                <table className="table compare-table">
                  <thead>
                    <tr><th /><th>{t("bk.restore.thisBackup")}</th><th>{t("bk.restore.now")}</th></tr>
                  </thead>
                  <tbody>
                    <tr><th scope="row">{t("bk.restore.patients")}</th><td className="cell-num">{n(preview.patients)}</td><td className="cell-num">{n(preview.current_patients)}</td></tr>
                    <tr><th scope="row">{t("bk.restore.appointments")}</th><td className="cell-num">{n(preview.appointments)}</td><td className="subtle">—</td></tr>
                    <tr><th scope="row">{t("bk.restore.invoices")}</th><td className="cell-num">{n(preview.invoices)}</td><td className="subtle">—</td></tr>
                    <tr><th scope="row">{t("bk.restore.users")}</th><td className="cell-num">{n(preview.users)}</td><td className="subtle">—</td></tr>
                    <tr><th scope="row">{t("bk.restore.lastActivity")}</th><td className="cell-num">{when(preview.last_activity)}</td><td className="cell-num">{when(preview.current_last_activity)}</td></tr>
                  </tbody>
                </table>
              </div>
              <Notice tone="warning" title={t("bk.restore.warnTitle")} testId="restore-warning">{t("bk.restore.warnBody")}</Notice>
              <Field label={t("bk.restore.password")} hint={t("bk.restore.passwordHint")} error={passwordError || null}>
                <PasswordInput value={password} onChange={(x) => { setPassword(x.target.value); setPasswordError(""); }} autoComplete="current-password" data-testid="restore-password" />
              </Field>
              <Checkbox checked={sure} onChange={setSure} testId="restore-sure">{t("bk.restore.sure")}</Checkbox>
              <div className="row">
                <Button variant="danger" icon={TriangleAlert} loading={busy} disabled={!sure || !password} onClick={restore} data-testid="restore-go">{t("bk.restore.go")}</Button>
              </div>
            </>
          )}
        </Card>
      )}
    </div>
  );
}
