import { useEffect, useState } from "react";
import { Archive, DatabaseBackup, Save, ShieldCheck, Timer } from "lucide-react";
import type { Settings } from "../../../shared/ts/contract";
import { SESSION_CHECK_EVENT, isSessionError, rpc } from "../lib/api";
import { digits, latinDigits } from "../lib/dates";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { Field, TextInput } from "../ui/Field";
import { ErrorState, Loading, Notice } from "../ui/Feedback";
import { HourPicker12 } from "../ui/TimePicker";
import { useToast } from "../ui/Toast";

export function SettingsPage() {
  const { err } = useI18n();
  const [s, setS] = useState<Settings | null>(null);
  const [error, setError] = useState("");
  const load = () => rpc("settings.get", {}).then(setS).catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, []);
  if (error && !s) return <Page><ErrorState message={error} onRetry={load} /></Page>;
  if (!s) return <Page><Loading /></Page>;
  return <SettingsForm initial={s} onSaved={setS} />;
}

function SettingsForm({ initial, onSaved }: { initial: Settings; onSaved: (s: Settings) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const form = useForm(
    { session_timeout_minutes: digits(initial.session_timeout_minutes, lang), backup_keep_daily: digits(initial.backup_keep_daily, lang) },
    { session_timeout_minutes: v.intRange(1, 240, "rule.session_timeout_range"), backup_keep_daily: v.intRange(1, 365, "rule.backup_keep_range") },
  );
  const [hour, setHour] = useState(initial.daily_backup_hour);
  const [hourError, setHourError] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const e = (k: "session_timeout_minutes" | "backup_keep_daily") => form.error(k) && t(form.error(k)!);

  const save = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      const saved = await rpc("settings.update", {
        session_timeout_minutes: Number(latinDigits(form.values.session_timeout_minutes)),
        daily_backup_hour: hour,
        backup_keep_daily: Number(latinDigits(form.values.backup_keep_daily)),
      });
      onSaved(saved);
      // A new auto-lock time applies at once, without signing in again (OF-012).
      window.dispatchEvent(new Event(SESSION_CHECK_EVENT));
      toast.success(t("common.saved"));
    } catch (x) {
      if (x && (x as { field?: string }).field === "daily_backup_hour") setHourError(t("rule.backup_hour_range"));
      else if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Page testId="page-settings">
      <PageHeader title={t("settings.title")} description={t("settings.subtitle")} />
      <form className="stack-lg" onSubmit={save} noValidate>
        {error && <Notice tone="danger" testId="settings-message">{error}</Notice>}
        <Card>
          <CardHeader icon={ShieldCheck} title={t("settings.security")} description={t("settings.securityHint")} />
          <div className="grid-2">
            <Field label={t("settings.timeout")} hint={t("hint.timeout")} error={e("session_timeout_minutes")}>
              <TextInput icon={Timer} inputMode="numeric" suffix={t("common.minutes")} value={form.values.session_timeout_minutes} onChange={(x) => form.set("session_timeout_minutes", x.target.value)} data-testid="setting-session_timeout_minutes" />
            </Field>
          </div>
        </Card>
        <Card>
          <CardHeader icon={DatabaseBackup} title={t("settings.backup")} description={t("settings.backupHint")} />
          <div className="grid-2">
            <Field label={t("settings.backupHour")} hint={t("hint.backupHour")} error={hourError || null}>
              <HourPicker12 value={hour} onChange={(h) => { setHour(h); setHourError(""); }} label={t("settings.backupHour")} testId="setting-daily_backup_hour" />
            </Field>
            <Field label={t("settings.keep")} hint={t("hint.keep")} error={e("backup_keep_daily")}>
              <TextInput icon={Archive} inputMode="numeric" suffix={t("common.copies")} value={form.values.backup_keep_daily} onChange={(x) => form.set("backup_keep_daily", x.target.value)} data-testid="setting-backup_keep_daily" />
            </Field>
          </div>
        </Card>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <Button type="submit" variant="primary" icon={Save} loading={busy} data-testid="settings-save">{t("common.save")}</Button>
        </div>
      </form>
    </Page>
  );
}
