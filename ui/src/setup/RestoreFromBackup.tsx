import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, FileArchive, FolderOpen, HardDriveDownload, LoaderCircle, RefreshCw } from "lucide-react";
import type { RestoreCandidate } from "../../../shared/ts/contract";
import { ApiError, rpc } from "../lib/api";
import { formatBytes, formatDateTime } from "../lib/dates";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Badge, EmptyState, Notice } from "../ui/Feedback";
import { Field, TextInput } from "../ui/Field";

/**
 * Bring a clinic to a new computer (roadmap 10.7): the backup is found on a connected USB drive or in
 * a folder (for example the cloud-synced one) and opened with the Recovery Key printed when the clinic
 * was first set up. Nothing about the old computer is needed.
 */
export function RestoreFromBackup({ onBack, onDone }: { onBack: () => void; onDone: () => void }) {
  const { t, lang, err } = useI18n();
  const [found, setFound] = useState<RestoreCandidate[] | null>(null);
  const [picked, setPicked] = useState<RestoreCandidate | null>(null);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState("");
  const form = useForm({ folder: "", recovery_key: "" }, { recovery_key: v.required });

  const scan = useCallback((folder: string) => {
    setFound(null);
    rpc("backup.scan", { folder: folder.trim() || null })
      .then((list) => {
        setFound(list);
        setPicked((p) => p ?? list[0] ?? null);
      })
      .catch(() => setFound([]));
  }, []);
  useEffect(() => scan(""), [scan]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setProblem("");
    if (!form.validate() || !picked) return;
    setBusy(true);
    try {
      await rpc("backup.restore_new", { folder: picked.folder, file_name: picked.file_name, recovery_key: form.values.recovery_key.trim() });
      onDone();
    } catch (x) {
      if (!form.serverError(x)) setProblem(x instanceof ApiError && x.code === "backup_invalid" ? t(`bk.restore.problem.${x.detail.split(": ").pop()}`) : err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="stack" onSubmit={submit} noValidate data-testid="restore-new">
      <div className="setup-head">
        <h1 className="t-title-lg">{t("restoreNew.title")}</h1>
        <p className="muted">{t("restoreNew.hint")}</p>
      </div>

      <div className="stack">
        <div className="row" style={{ justifyContent: "space-between" }}>
          <span className="t-overline">{t("restoreNew.found")}</span>
          <Button variant="subtle" size="sm" icon={found ? RefreshCw : LoaderCircle} disabled={!found} onClick={() => scan(form.values.folder)} data-testid="restore-new-search">{t("connect.again")}</Button>
        </div>
        {found && found.length > 0 && (
          <ul className="plain-list restore-list" role="radiogroup" aria-label={t("restoreNew.found")} data-testid="restore-new-list">
            {found.slice(0, 20).map((c) => (
              <li key={`${c.folder}/${c.file_name}`}>
                <button type="button" role="radio" aria-checked={picked?.file_name === c.file_name && picked.folder === c.folder} className="restore-pick" onClick={() => setPicked(c)} data-testid="restore-new-item">
                  <FileArchive aria-hidden />
                  <span className="restore-main">
                    <strong>{c.created_at ? formatDateTime(c.created_at, lang) : c.file_name}</strong>
                    <span className="subtle t-caption"><bdi className="ltr">{c.folder}</bdi></span>
                  </span>
                  <bdi className="ltr num subtle">{formatBytes(c.size_bytes, lang)}</bdi>
                  {!c.has_key && <Badge tone="warning">{t("bk.restore.noKey")}</Badge>}
                </button>
              </li>
            ))}
          </ul>
        )}
        {found && found.length === 0 && <EmptyState icon={HardDriveDownload} title={t("restoreNew.none")}>{t("restoreNew.noneHint")}</EmptyState>}
        <Field label={t("restoreNew.folder")} hint={t("restoreNew.folderHint")} optional>
          <TextInput icon={FolderOpen} dir="ltr" placeholder="C:\Users\…\Google Drive" value={form.values.folder} onChange={(x) => form.set("folder", x.target.value)} onBlur={() => scan(form.values.folder)} data-testid="restore-new-folder" />
        </Field>
      </div>

      <Field label={t("restoreNew.key")} hint={t("restoreNew.keyHint")} error={form.error("recovery_key") && t(form.error("recovery_key")!)}>
        <TextInput large dir="ltr" autoComplete="off" spellCheck={false} placeholder="XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXXXX" value={form.values.recovery_key} onChange={(x) => form.set("recovery_key", x.target.value.toUpperCase())} onBlur={() => form.blur("recovery_key")} data-testid="restore-new-key" />
      </Field>

      {problem && <Notice tone="danger" testId="restore-new-problem">{problem}</Notice>}
      <div className="setup-actions">
        <Button variant="subtle" icon={ArrowLeft} flipIcon onClick={onBack} data-testid="restore-new-back">{t("setup.back")}</Button>
        <span className="spacer" />
        <Button type="submit" variant="primary" icon={HardDriveDownload} loading={busy} disabled={!picked} data-testid="restore-new-go">{t("restoreNew.go")}</Button>
      </div>
    </form>
  );
}
