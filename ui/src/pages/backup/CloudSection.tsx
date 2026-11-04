import { useState } from "react";
import { Cloud, CloudOff, FolderOpen, Save, ShieldCheck } from "lucide-react";
import type { BackupSettings, CloudStatus } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { digits, latinDigits } from "../../lib/dates";
import { useForm, v } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardFooter, CardHeader } from "../../ui/Card";
import { Switch } from "../../ui/Controls";
import { Badge, Notice } from "../../ui/Feedback";
import { Field, TextInput } from "../../ui/Field";
import { useToast } from "../../ui/Toast";

/**
 * Cloud copy (roadmap 10.6, ADR-17): the encrypted backup is written into the clinic's own
 * Google Drive / OneDrive folder; their sync app uploads it, also later when the internet returns.
 */
export function CloudSection({ settings, canEdit, onSaved }: { settings: BackupSettings; canEdit: boolean; onSaved: (s: BackupSettings) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [images, setImages] = useState(settings.cloud_include_images);
  const [status, setStatus] = useState<CloudStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const form = useForm(
    {
      folder: settings.cloud_folder ?? "",
      keep_daily: digits(settings.cloud_keep_daily, lang),
      keep_weekly: digits(settings.cloud_keep_weekly, lang),
    },
    {
      keep_daily: v.intRange(1, 365, "rule.backup_keep_range"),
      keep_weekly: v.intRange(0, 104, "rule.backup_retention_range"),
    },
  );
  const e = (k: keyof typeof form.values) => form.error(k) && t(form.error(k)!);

  const save = async (ev: React.FormEvent) => {
    ev.preventDefault();
    if (!form.validate()) return;
    setBusy(true);
    try {
      const saved = await rpc("backup.settings_save", {
        ...settings,
        cloud_folder: form.values.folder.trim() || null,
        cloud_include_images: images,
        cloud_keep_daily: Number(latinDigits(form.values.keep_daily)),
        cloud_keep_weekly: Number(latinDigits(form.values.keep_weekly)),
      });
      onSaved(saved);
      setStatus(await rpc("backup.cloud_check", {}));
      toast.success(t("common.saved"));
    } catch (x) {
      if (!form.serverError(x, { cloud_folder: "folder" })) toast.error(err(x));
    } finally {
      setBusy(false);
    }
  };

  const provider = status ? t(`bk.cloud.provider.${status.provider}`) : "";

  return (
    <form className="stack-lg" onSubmit={save} noValidate data-testid="cloud-section">
      <Card>
        <CardHeader icon={Cloud} title={t("bk.cloud.title")} description={t("bk.cloud.hint")} />
        <Notice tone="info" title={t("bk.cloud.privateTitle")}>{t("bk.cloud.privateBody")}</Notice>
        <Field label={t("bk.cloud.folder")} hint={t("bk.cloud.folderHint")} error={e("folder")}>
          <TextInput icon={FolderOpen} dir="ltr" placeholder="C:\Users\…\Google Drive" value={form.values.folder} onChange={(x) => form.set("folder", x.target.value)} disabled={!canEdit} data-testid="cloud-folder" />
        </Field>
        {status && (
          <div className="row" role="status" data-testid="cloud-status">
            {status.folder === null ? (
              <Badge icon={CloudOff}>{t("bk.cloud.off")}</Badge>
            ) : status.folder_exists && status.writable ? (
              <Badge tone="success" icon={ShieldCheck}>{t("bk.cloud.ready").replace("{provider}", provider)}</Badge>
            ) : (
              <Badge tone="danger" icon={CloudOff}>{t("bk.cloud.missing")}</Badge>
            )}
            {status.sync_app_found === false && <Badge tone="warning">{t("bk.cloud.noApp")}</Badge>}
          </div>
        )}
        <Switch checked={images} onChange={setImages} disabled={!canEdit} label={t("bk.cloud.images")} testId="cloud-images" />
        <p className="subtle t-caption" style={{ margin: 0 }}>{t("bk.cloud.imagesHint")}</p>
      </Card>
      <Card>
        <CardHeader icon={Cloud} title={t("bk.cloud.keepTitle")} description={t("bk.cloud.keepHint")} />
        <div className="grid-2">
          <Field label={t("bk.keep.daily")} error={e("keep_daily")}>
            <TextInput inputMode="numeric" suffix={t("bk.keep.days")} value={form.values.keep_daily} onChange={(x) => form.set("keep_daily", x.target.value)} disabled={!canEdit} data-testid="cloud-keep-daily" />
          </Field>
          <Field label={t("bk.keep.weekly")} error={e("keep_weekly")}>
            <TextInput inputMode="numeric" suffix={t("bk.keep.weeks")} value={form.values.keep_weekly} onChange={(x) => form.set("keep_weekly", x.target.value)} disabled={!canEdit} data-testid="cloud-keep-weekly" />
          </Field>
        </div>
        {canEdit && (
          <CardFooter>
            <Button type="submit" variant="primary" icon={Save} loading={busy} data-testid="cloud-save">{t("common.save")}</Button>
          </CardFooter>
        )}
      </Card>
    </form>
  );
}
