import { useEffect, useState } from "react";
import { Archive, CalendarDays, DatabaseBackup, Printer, Save, ShieldCheck, Stethoscope, Timer } from "lucide-react";
import { isDesktop, listPrinters, loadProfile, saveProfile, type PrintProfile } from "../print/engine";
import { invalidateLetterhead } from "../print/letterhead";
import { Select } from "../ui/Field";
import { Switch } from "../ui/Controls";
import type { Settings } from "../../../shared/ts/contract";
import { SESSION_CHECK_EVENT, isSessionError, rpc } from "../lib/api";
import { digits, latinDigits } from "../lib/dates";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { Field, TextInput } from "../ui/Field";
import { Segmented } from "../ui/Controls";
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
  const [snap, setSnap] = useState(initial.calendar_snap_minutes);
  const [brandFooter, setBrandFooter] = useState(initial.print_brand_footer);
  const [restrict, setRestrict] = useState(initial.restrict_service_specialty);
  // This computer's printer (ADR-13: printers belong to a computer, not to the clinic).
  const [profile, setProfile] = useState<PrintProfile>(loadProfile());
  const [printers, setPrinters] = useState<string[]>([]);
  useEffect(() => {
    listPrinters().then((l) => setPrinters(l.printers)).catch(() => {});
  }, []);
  const updateProfile = (p: PrintProfile) => {
    setProfile(p);
    saveProfile(p);
  };
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
        calendar_snap_minutes: snap,
        print_brand_footer: brandFooter,
        restrict_service_specialty: restrict,
      });
      onSaved(saved);
      invalidateLetterhead();
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
        <Card>
          <CardHeader icon={CalendarDays} title={t("settings.calendar")} description={t("settings.calendarHint")} />
          {/* OF-044: an appointment dragged in the calendar lands on steps of this size. */}
          <Field label={t("settings.snap")} hint={t("hint.snap")}>
            <Segmented<string>
              value={String(snap)}
              onChange={(x) => setSnap(Number(x))}
              label={t("settings.snap")}
              options={[1, 5, 10, 15].map((n) => ({ value: String(n), label: `${digits(n, lang)} ${t("common.minutes")}`, testId: `setting-calendar-snap-${n}` }))}
            />
          </Field>
        </Card>
        <Card>
          <CardHeader icon={Printer} title={t("settings.print")} description={t("settings.printHint")} />
          <div className="grid-2">
            <Field label={t("print.printer")} hint={isDesktop() ? t("settings.printerHint") : t("print.browserHint")}>
              <Select value={profile.printer ?? ""} disabled={!isDesktop()} onChange={(e) => updateProfile({ ...profile, printer: e.target.value || null })} data-testid="setting-printer">
                <option value="">{t("print.noPrinter")}</option>
                {printers.map((p) => <option key={p} value={p}>{p}</option>)}
              </Select>
            </Field>
            <Field label={t("print.smallPaper")} hint={profile.smallPaper === "compact_a4" ? t("print.compactHint") : t("print.nativeHint")}>
              <Segmented<PrintProfile["smallPaper"]>
                value={profile.smallPaper}
                onChange={(v) => updateProfile({ ...profile, smallPaper: v })}
                label={t("print.smallPaper")}
                options={[{ value: "native", label: t("settings.smallNative"), testId: "setting-small-native" }, { value: "compact_a4", label: t("print.compactA4"), testId: "setting-small-compact" }]}
              />
            </Field>
          </div>
          <div style={{ marginTop: "var(--space-5)" }}>
            <Switch checked={brandFooter} onChange={setBrandFooter} label={t("settings.brandFooter")} testId="setting-brand-footer" />
          </div>
        </Card>
        <Card>
          <CardHeader icon={Stethoscope} title={t("settings.clinicalRules")} description={t("settings.clinicalRulesHint")} />
          <Switch checked={restrict} onChange={setRestrict} label={t("settings.restrictSpecialty")} testId="setting-restrict-specialty" />
        </Card>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <Button type="submit" variant="primary" icon={Save} loading={busy} data-testid="settings-save">{t("common.save")}</Button>
        </div>
      </form>
    </Page>
  );
}
