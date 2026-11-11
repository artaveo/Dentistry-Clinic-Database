import { useEffect, useState, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { CalendarDays, Printer, Save, ShieldCheck, Stethoscope, Timer } from "lucide-react";
import { isDesktop, listPrinters, loadProfile, saveProfile, type PrintProfile } from "../print/engine";
import { invalidateLetterhead } from "../print/letterhead";
import { Select } from "../ui/Field";
import { Switch } from "../ui/Controls";
import type { Settings, ToothNumbering } from "../../../shared/ts/contract";
import { invalidateClinicSettings } from "../lib/clinical";
import { SESSION_CHECK_EVENT, isSessionError, rpc } from "../lib/api";
import { digits, latinDigits } from "../lib/dates";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { Field, TextInput } from "../ui/Field";
import { Segmented } from "../ui/Controls";
import { ErrorState, Loading, Notice } from "../ui/Feedback";
import { useToast } from "../ui/Toast";

/** One entry of the Settings hub: an existing page, embedded unchanged. `group` is a translated heading. */
export type HubSection = { id: string; group: string; label: string; icon: LucideIcon; render: () => ReactNode };

/**
 * U3: everything that is not daily work (clinic, doctors, services, lists, users, backup, licence, audit,
 * network, about) lives here as a list of sections; the chosen section's own page is shown beside the list.
 * The caller already filtered `sections` by permission.
 */
export function SettingsHub({ sections, active, onSelect }: { sections: HubSection[]; active: string; onSelect: (id: string) => void }) {
  const { t } = useI18n();
  const current = sections.find((s) => s.id === active) ?? sections[0];
  return (
    <div className="settings-hub" data-testid="page-settings-hub">
      <nav className="hub-nav" aria-label={t("hub.nav")}>
        {sections.map((s, i) => {
          const Icon = s.icon;
          return (
            <div key={s.id} style={{ display: "contents" }}>
              {(i === 0 || sections[i - 1].group !== s.group) && <span className="t-overline">{s.group}</span>}
              <button type="button" className="hub-nav-item" aria-current={current.id === s.id ? "page" : undefined} onClick={() => onSelect(s.id)} data-testid={`tab-${s.id}`}>
                <Icon aria-hidden />
                <span>{s.label}</span>
              </button>
            </div>
          );
        })}
      </nav>
      <div className="hub-body" key={current.id}>{current.render()}</div>
    </div>
  );
}

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
    { session_timeout_minutes: digits(initial.session_timeout_minutes, lang) },
    { session_timeout_minutes: v.intRange(1, 240, "rule.session_timeout_range") },
  );
  const [snap, setSnap] = useState(initial.calendar_snap_minutes);
  const [brandFooter, setBrandFooter] = useState(initial.print_brand_footer);
  const [restrict, setRestrict] = useState(initial.restrict_service_specialty);
  const [numbering, setNumbering] = useState<ToothNumbering>(initial.tooth_numbering);
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
  const e = (k: "session_timeout_minutes") => form.error(k) && t(form.error(k)!);

  const save = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      const saved = await rpc("settings.update", {
        session_timeout_minutes: Number(latinDigits(form.values.session_timeout_minutes)),
        // The backup schedule is edited on the Backup page only; the saved values are sent back as they are.
        daily_backup_hour: initial.daily_backup_hour,
        backup_keep_daily: initial.backup_keep_daily,
        calendar_snap_minutes: snap,
        print_brand_footer: brandFooter,
        restrict_service_specialty: restrict,
        tooth_numbering: numbering,
      });
      onSaved(saved);
      invalidateLetterhead();
      invalidateClinicSettings();
      // A new auto-lock time applies at once, without signing in again (OF-012).
      window.dispatchEvent(new Event(SESSION_CHECK_EVENT));
      toast.success(t("common.saved"));
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
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
          {/* ADR-14: how tooth numbers are shown on the chart and on documents; stored numbers are always FDI. */}
          <Field label={t("settings.toothNumbering")} hint={t(`settings.toothNumbering.${numbering}Hint`)} className="setting-numbering">
            <Segmented<ToothNumbering>
              value={numbering}
              onChange={setNumbering}
              label={t("settings.toothNumbering")}
              options={(["quadrant", "fdi", "palmer", "universal"] as ToothNumbering[]).map((n) => ({ value: n, label: t(`settings.toothNumbering.${n}`), testId: `setting-numbering-${n}` }))}
            />
          </Field>
        </Card>
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <Button type="submit" variant="primary" icon={Save} loading={busy} data-testid="settings-save">{t("common.save")}</Button>
        </div>
      </form>
    </Page>
  );
}
