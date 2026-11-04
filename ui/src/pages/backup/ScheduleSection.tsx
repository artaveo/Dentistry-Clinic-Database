import { useState } from "react";
import { Archive, CalendarClock, Save, Usb } from "lucide-react";
import type { BackupSettings } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { digits, latinDigits } from "../../lib/dates";
import { useForm, v } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardFooter, CardHeader } from "../../ui/Card";
import { Notice } from "../../ui/Feedback";
import { Field, TextInput } from "../../ui/Field";
import { TimePicker12 } from "../../ui/TimePicker";
import { useToast } from "../../ui/Toast";

const hhmm = (h: number, m: number) => `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;

/** When the daily backup runs, how long copies are kept (roadmap 10.2, 10.4, 10.5). */
export function ScheduleSection({ settings, canEdit, onSaved }: { settings: BackupSettings; canEdit: boolean; onSaved: (s: BackupSettings) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [time, setTime] = useState(hhmm(settings.daily_hour, settings.daily_minute));
  const [busy, setBusy] = useState(false);
  const form = useForm(
    {
      usb_warn_minutes: digits(settings.usb_warn_minutes, lang),
      keep_daily: digits(settings.keep_daily, lang),
      keep_weekly: digits(settings.keep_weekly, lang),
      keep_monthly: digits(settings.keep_monthly, lang),
    },
    {
      usb_warn_minutes: v.intRange(5, 720, "rule.usb_interval_range"),
      keep_daily: v.intRange(1, 365, "rule.backup_keep_range"),
      keep_weekly: v.intRange(0, 104, "rule.backup_retention_range"),
      keep_monthly: v.intRange(0, 60, "rule.backup_retention_range"),
    },
  );
  const num = (k: keyof typeof form.values) => Number(latinDigits(form.values[k]));
  const e = (k: keyof typeof form.values) => form.error(k) && t(form.error(k)!);

  const save = async (ev: React.FormEvent) => {
    ev.preventDefault();
    if (!form.validate()) return;
    const [hour, minute] = time.split(":").map(Number);
    setBusy(true);
    try {
      const saved = await rpc("backup.settings_save", {
        ...settings,
        daily_hour: hour,
        daily_minute: minute,
        usb_warn_minutes: num("usb_warn_minutes"),
        keep_daily: num("keep_daily"),
        keep_weekly: num("keep_weekly"),
        keep_monthly: num("keep_monthly"),
      });
      onSaved(saved);
      toast.success(t("common.saved"));
    } catch (x) {
      if (!form.serverError(x)) toast.error(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="stack-lg" onSubmit={save} noValidate data-testid="schedule-section">
      <Card>
        <CardHeader icon={CalendarClock} title={t("bk.schedule.title")} description={t("bk.schedule.hint")} />
        <Field label={t("bk.schedule.time")} hint={t("bk.schedule.timeHint")}>
          <TimePicker12 value={time} onChange={setTime} label={t("bk.schedule.time")} testId="backup-time" />
        </Field>
      </Card>
      <Card>
        <CardHeader icon={Archive} title={t("bk.keep.title")} description={t("bk.keep.hint")} />
        <div className="grid-3">
          <Field label={t("bk.keep.daily")} hint={t("bk.keep.dailyHint")} error={e("keep_daily")}>
            <TextInput inputMode="numeric" suffix={t("bk.keep.days")} value={form.values.keep_daily} onChange={(x) => form.set("keep_daily", x.target.value)} onBlur={() => form.blur("keep_daily")} data-testid="keep-daily" />
          </Field>
          <Field label={t("bk.keep.weekly")} hint={t("bk.keep.weeklyHint")} error={e("keep_weekly")}>
            <TextInput inputMode="numeric" suffix={t("bk.keep.weeks")} value={form.values.keep_weekly} onChange={(x) => form.set("keep_weekly", x.target.value)} onBlur={() => form.blur("keep_weekly")} data-testid="keep-weekly" />
          </Field>
          <Field label={t("bk.keep.monthly")} hint={t("bk.keep.monthlyHint")} error={e("keep_monthly")}>
            <TextInput inputMode="numeric" suffix={t("bk.keep.months")} value={form.values.keep_monthly} onChange={(x) => form.set("keep_monthly", x.target.value)} onBlur={() => form.blur("keep_monthly")} data-testid="keep-monthly" />
          </Field>
        </div>
        <Notice tone="info">{t("bk.keep.newest")}</Notice>
      </Card>
      <Card>
        <CardHeader icon={Usb} title={t("bk.usbWarn.title")} description={t("bk.usbWarn.hint")} />
        <Field label={t("bk.usbWarn.every")} hint={t("bk.usbWarn.everyHint")} error={e("usb_warn_minutes")}>
          <TextInput inputMode="numeric" suffix={t("bk.usbWarn.minutes")} value={form.values.usb_warn_minutes} onChange={(x) => form.set("usb_warn_minutes", x.target.value)} onBlur={() => form.blur("usb_warn_minutes")} data-testid="usb-warn" />
        </Field>
        {canEdit && (
          <CardFooter>
            <Button type="submit" variant="primary" icon={Save} loading={busy} data-testid="schedule-save">{t("common.save")}</Button>
          </CardFooter>
        )}
      </Card>
    </form>
  );
}
