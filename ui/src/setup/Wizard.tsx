import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft, ArrowRight, Building2, CalendarClock, Check, DatabaseBackup, Globe, ImageUp, Laptop, LockKeyhole, MapPin, Moon,
  Monitor, Network, Phone, Printer, Server, ShieldCheck, Sparkles, Sun, Trash2, User, UserRound, Users, WifiOff, IdCard,
} from "lucide-react";
import type { CalendarSystem, ClinicMode, DayHours, InstallMode, Language, ThemePreference } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { applyBrand, rememberBrand } from "../lib/color";
import { digits, formatDateTime, formatHour } from "../lib/dates";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { useTheme } from "../theme";
import { LangSwitch } from "../pages/LangSwitch";
import { GeoPicker } from "./GeoPicker";
import { HoursEditor, badHours, defaultHours } from "./HoursEditor";
import { ColorSwatches } from "./ColorSwatches";
import { readLogo } from "./logo";
import { Button } from "../ui/Button";
import { Checkbox, OptionCards, Segmented } from "../ui/Controls";
import { Field, PasswordInput, TextInput } from "../ui/Field";
import { Badge, Notice } from "../ui/Feedback";
import { ArtaveoLockup, ClinicMark } from "../ui/Brand";

type Step = "welcome" | "language" | "install_mode" | "clinic_info" | "hours" | "clinic_type" | "branding" | "trial" | "backup" | "owner" | "recovery";
const STEPS: Step[] = ["welcome", "language", "install_mode", "clinic_info", "hours", "clinic_type", "branding", "trial", "backup", "owner", "recovery"];

/** First-run wizard (roadmap 2.5): clinic identity, geography, calendar,
 * branding and Owner + Recovery Key, in one atomic `app.setup` call. */
export function Setup({ onDone }: { onDone: () => void }) {
  const { t, err, lang, setLang } = useI18n();
  const { setTheme: applyGlobalTheme } = useTheme();
  const [stepIndex, setStepIndex] = useState(0);
  const step = STEPS[stepIndex];
  const go = (delta: number) => setStepIndex((i) => Math.max(0, Math.min(STEPS.length - 1, i + delta)));

  const [installMode, setInstallMode] = useState<InstallMode>("single");
  const info = useForm({ clinic_name: "", address: "", phone: "" }, { clinic_name: v.clinicName, phone: v.phone });
  const [logo, setLogo] = useState<{ base64: string; name: string; dataUrl: string } | null>(null);
  const [logoError, setLogoError] = useState("");
  const [provinceId, setProvinceId] = useState<string | null>(null);
  const [districtId, setDistrictId] = useState<string | null>(null);
  const [calendarSystem, setCalendarSystem] = useState<CalendarSystem>("shamsi");
  const [workingHours, setWorkingHours] = useState<DayHours[]>(defaultHours());
  const [hoursError, setHoursError] = useState("");
  const [clinicMode, setClinicMode] = useState<ClinicMode>("solo");
  const [theme, setTheme] = useState<ThemePreference>("system");
  const [colors, setColors] = useState({ color_primary: "#0e7490", color_secondary: "#64748b", color_accent: "#f59e0b" });
  const [trialAcknowledged, setTrialAcknowledged] = useState(false);
  const owner = useForm(
    { username: "", display_name: "", password: "", repeat: "" },
    { username: v.username, display_name: v.displayName, password: v.password, repeat: (r, all) => (!r ? "rule.required" : r !== all.password ? "rule.mismatch" : null) },
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => applyBrand(colors.color_primary, colors.color_accent), [colors.color_primary, colors.color_accent]);

  const pickLogo = async (file: File | undefined) => {
    if (!file) return;
    setLogoError("");
    try {
      setLogo(await readLogo(file));
    } catch (e) {
      setLogoError(t(e instanceof Error && e.message.startsWith("rule.") ? e.message : "rule.logo_type"));
    }
  };

  const next = () => {
    if (step === "clinic_info" && !info.validate()) return;
    if (step === "hours" && badHours(workingHours)) return setHoursError(t("rule.working_hours"));
    go(1);
  };

  const create = async () => {
    setError("");
    if (!owner.validate()) return;
    setBusy(true);
    try {
      const r = await rpc("app.setup", {
        clinic_name: info.values.clinic_name.trim(),
        owner_username: owner.values.username,
        owner_display_name: owner.values.display_name.trim(),
        owner_password: owner.values.password,
        language: lang as Language,
        install_mode: installMode,
        province_id: provinceId,
        district_id: districtId,
        address: info.values.address.trim() || null,
        phone: info.values.phone.trim() || null,
        logo_base64: logo?.base64 ?? null,
        logo_file_name: logo?.name ?? null,
        calendar_system: calendarSystem,
        clinic_mode: clinicMode,
        theme,
        ...colors,
        working_hours: workingHours,
        trial_acknowledged: trialAcknowledged,
      });
      rememberBrand(colors.color_primary, colors.color_accent);
      setRecoveryKey(r.recovery_key);
      go(1);
    } catch (e) {
      const map = { owner_username: "username", owner_display_name: "display_name", owner_password: "password" } as const;
      if (!owner.serverError(e, map)) setError(err(e));
    } finally {
      setBusy(false);
    }
  };

  const visible = STEPS.filter((s) => s !== "recovery");
  const total = visible.length;
  const nav = (nextButton = true, disabled = false) => (
    <div className="setup-actions">
      {stepIndex > 0 && <Button variant="subtle" icon={ArrowLeft} flipIcon onClick={() => go(-1)} data-testid="wizard-back">{t("setup.back")}</Button>}
      <span className="spacer" />
      {nextButton && <Button variant="primary" iconEnd={ArrowRight} flipIcon disabled={disabled} onClick={next} data-testid="wizard-next">{t("setup.next")}</Button>}
    </div>
  );
  const head = (title: string, desc?: string) => (
    <div className="setup-head">
      <span className="step-count">{t("wizard.stepOf").replace("{n}", digits(stepIndex + 1, lang)).replace("{total}", digits(total, lang))}</span>
      <h1 className="t-title-lg">{title}</h1>
      {desc && <p className="muted">{desc}</p>}
    </div>
  );
  const e = (form: typeof owner | typeof info, k: string) => {
    const x = (form.error as (k: string) => string | null)(k);
    return x && t(x);
  };

  return (
    <div className="setup">
      <aside className="setup-aside">
        <div className="row" style={{ justifyContent: "space-between" }}>
          <ArtaveoLockup height={46} />
        </div>
        <nav className="stepper" aria-label={t("setup.title")}>
          {visible.map((s, i) => {
            const done = i < stepIndex || step === "recovery";
            return (
              <div key={s} className={`step${i === stepIndex ? " current" : done ? " done" : ""}`} aria-current={i === stepIndex ? "step" : undefined}>
                <span className="step-dot">{done ? <Check aria-hidden /> : digits(i + 1, lang)}</span>
                {t(`wizard.step.${s}`)}
              </div>
            );
          })}
        </nav>
        <div style={{ marginTop: "auto" }} className="stack">
          <LangSwitch />
          <span className="subtle t-caption">Artaveo Dental</span>
        </div>
      </aside>

      <main className="setup-main">
        <div className="glass setup-card" data-testid={`setup-step-${step}`} key={step}>
          {step === "welcome" && (
            <div className="welcome">
              <div className="welcome-hero">
                <span className="card-header" style={{ margin: 0 }}><span className="card-icon"><Sparkles aria-hidden /></span></span>
                <h1 className="t-display">{t("wizard.welcome.title")}</h1>
                <p className="muted" style={{ fontSize: 15, lineHeight: "26px" }}>{t("wizard.welcome.body")}</p>
              </div>
              <div className="welcome-points">
                <div className="welcome-point"><ShieldCheck aria-hidden /><strong>{t("wizard.welcome.p1")}</strong><span>{t("wizard.welcome.p1Hint")}</span></div>
                <div className="welcome-point"><WifiOff aria-hidden /><strong>{t("wizard.welcome.p2")}</strong><span>{t("wizard.welcome.p2Hint")}</span></div>
                <div className="welcome-point"><Globe aria-hidden /><strong>{t("wizard.welcome.p3")}</strong><span>{t("wizard.welcome.p3Hint")}</span></div>
              </div>
              <div className="setup-actions">
                <span className="subtle t-caption">{t("wizard.welcome.time")}</span>
                <span className="spacer" />
                <Button variant="primary" size="lg" iconEnd={ArrowRight} flipIcon onClick={() => go(1)} data-testid="wizard-start">{t("wizard.welcome.start")}</Button>
              </div>
            </div>
          )}

          {step === "language" && (
            <>
              {head(t("wizard.step.language"), t("wizard.language.hint"))}
              <div className="options lang-cards" role="radiogroup" aria-label={t("wizard.step.language")}>
                {(["fa", "ps", "en"] as Language[]).map((l) => (
                  <button key={l} type="button" className="option" role="radio" aria-checked={lang === l} aria-pressed={lang === l} onClick={() => setLang(l)} data-testid={`wizard-lang-${l}`} lang={l}>
                    <span className="lang-native">{t(`lang.${l}`)}</span>
                    <span className="lang-sub">{t(`wizard.language.${l}`)}</span>
                  </button>
                ))}
              </div>
              {nav()}
            </>
          )}

          {step === "install_mode" && (
            <>
              {head(t("wizard.installMode.title"), t("wizard.installMode.hint"))}
              <OptionCards<InstallMode>
                value={installMode}
                onChange={setInstallMode}
                label={t("wizard.installMode.title")}
                options={[
                  { value: "single", title: t("wizard.installMode.single.title"), hint: t("wizard.installMode.single.hint"), icon: Laptop, testId: "install-single" },
                  { value: "server", title: t("wizard.installMode.server.title"), hint: t("wizard.installMode.server.hint"), icon: Server, disabled: true, badge: <Badge tone="info">{t("wizard.installMode.comingSoon")}</Badge>, testId: "install-server" },
                  { value: "client", title: t("wizard.installMode.client.title"), hint: t("wizard.installMode.client.hint"), icon: Network, disabled: true, badge: <Badge tone="info">{t("wizard.installMode.comingSoon")}</Badge>, testId: "install-client" },
                ]}
              />
              {nav()}
            </>
          )}

          {step === "clinic_info" && (
            <>
              {head(t("wizard.clinicInfo.title"), t("wizard.clinicInfo.hint"))}
              <Field label={t("setup.clinicName")} hint={t("hint.clinicName")} error={e(info, "clinic_name")}>
                <TextInput icon={Building2} large autoFocus value={info.values.clinic_name} onChange={(x) => info.set("clinic_name", x.target.value)} onBlur={() => info.blur("clinic_name")} data-testid="setup-clinic" />
              </Field>
              <div className="field">
                <span className="field-label">{t("wizard.clinicInfo.logo")} <span className="optional">{t("common.optional")}</span></span>
                <div className="logo-picker">
                  <ClinicMark name={info.values.clinic_name} logo={logo?.dataUrl} size="lg" />
                  <div className="logo-text">
                    <span>{logo ? logo.name : t("wizard.clinicInfo.logoNone")}</span>
                    <span className="subtle t-caption">{t("clinic.logoHint")}</span>
                  </div>
                  <input ref={fileRef} type="file" accept="image/png,image/jpeg,image/webp" hidden onChange={(x) => pickLogo(x.target.files?.[0])} data-testid="setup-logo" />
                  <Button icon={ImageUp} onClick={() => fileRef.current?.click()}>{logo ? t("clinic.logoChange") : t("clinic.logoUpload")}</Button>
                  {logo && <Button variant="subtle" icon={Trash2} onClick={() => setLogo(null)}>{t("wizard.clinicInfo.logoRemove")}</Button>}
                </div>
                {logoError && <div className="field-error" role="alert">{logoError}</div>}
              </div>
              <div className="grid-2">
                <GeoPicker provinceId={provinceId} districtId={districtId} onChange={(p, d) => { setProvinceId(p); setDistrictId(d); }} />
                <Field label={t("wizard.clinicInfo.address")} optional className="span-2">
                  <TextInput icon={MapPin} value={info.values.address} onChange={(x) => info.set("address", x.target.value)} placeholder={t("wizard.clinicInfo.addressPlaceholder")} data-testid="setup-address" />
                </Field>
                <Field label={t("wizard.clinicInfo.phone")} optional hint={t("hint.phone")} error={e(info, "phone")}>
                  <TextInput icon={Phone} dir="ltr" inputMode="tel" value={info.values.phone} onChange={(x) => info.set("phone", x.target.value)} data-testid="setup-phone" />
                </Field>
              </div>
              {nav(true, !info.values.clinic_name.trim())}
            </>
          )}

          {step === "hours" && (
            <>
              {head(t("wizard.hours.title"), t("wizard.hours.hint"))}
              <Field label={t("wizard.hours.calendar")} hint={t("hint.calendar")}>
                <Segmented<CalendarSystem>
                  value={calendarSystem}
                  onChange={setCalendarSystem}
                  label={t("wizard.hours.calendar")}
                  options={[
                    { value: "shamsi", label: t("wizard.hours.calendar.shamsi"), testId: "calendar-shamsi" },
                    { value: "gregorian", label: t("wizard.hours.calendar.gregorian"), testId: "calendar-gregorian" },
                  ]}
                />
              </Field>
              <div className="field">
                <span className="field-label">{t("wizard.hours.workingHours")}</span>
                {hoursError && <Notice tone="danger">{hoursError}</Notice>}
                <HoursEditor hours={workingHours} onChange={(h) => { setWorkingHours(h); setHoursError(""); }} />
              </div>
              {nav()}
            </>
          )}

          {step === "clinic_type" && (
            <>
              {head(t("wizard.clinicType.title"), t("wizard.clinicType.hint"))}
              <OptionCards<ClinicMode>
                value={clinicMode}
                onChange={setClinicMode}
                label={t("wizard.clinicType.title")}
                options={[
                  { value: "solo", title: t("wizard.clinicType.solo.title"), hint: t("wizard.clinicType.solo.hint"), icon: User, testId: "clinic-mode-solo" },
                  { value: "multi", title: t("wizard.clinicType.multi.title"), hint: t("wizard.clinicType.multi.hint"), icon: Users, testId: "clinic-mode-multi" },
                ]}
              />
              {nav()}
            </>
          )}

          {step === "branding" && (
            <>
              {head(t("wizard.branding.title"), t("wizard.branding.hint"))}
              <Field label={t("wizard.branding.theme")} hint={t("hint.theme")}>
                <Segmented<ThemePreference>
                  value={theme}
                  onChange={(th) => {
                    setTheme(th);
                    applyGlobalTheme(th);
                  }}
                  label={t("wizard.branding.theme")}
                  options={[
                    { value: "light", label: t("theme.light"), icon: Sun, testId: "theme-light" },
                    { value: "dark", label: t("theme.dark"), icon: Moon, testId: "theme-dark" },
                    { value: "system", label: t("theme.system"), icon: Monitor, testId: "theme-system" },
                  ]}
                />
              </Field>
              <ColorSwatches primary={colors.color_primary} secondary={colors.color_secondary} accent={colors.color_accent} onChange={(k, val) => setColors((c) => ({ ...c, [k]: val }))} />
              <div className="brand-preview" aria-hidden>
                <span className="t-overline">{t("wizard.branding.livePreview")}</span>
                <div className="row" style={{ gap: 12 }}>
                  <ClinicMark name={info.values.clinic_name} logo={logo?.dataUrl} size="sm" />
                  <span className="nav-item" aria-current="page" style={{ flex: 1 }}><Building2 aria-hidden /><span>{t("nav.clinic")}</span></span>
                  <Button variant="primary" tabIndex={-1}>{t("common.save")}</Button>
                </div>
              </div>
              {nav()}
            </>
          )}

          {step === "trial" && (
            <>
              {head(t("wizard.trial.title"))}
              <Notice tone="info" title={t("wizard.trial.noticeTitle")}>{t("wizard.trial.body")}</Notice>
              <Checkbox checked={trialAcknowledged} onChange={setTrialAcknowledged} testId="trial-ack">{t("wizard.trial.ack")}</Checkbox>
              {nav()}
            </>
          )}

          {step === "backup" && (
            <>
              {head(t("wizard.backup.title"), t("wizard.backup.body"))}
              <div className="welcome-points">
                <div className="welcome-point"><CalendarClock aria-hidden /><strong>{t("backup.daily")}</strong><span>{t("backup.dailyAt").replace("{time}", formatHour(19, lang))}</span></div>
                <div className="welcome-point"><ShieldCheck aria-hidden /><strong>{t("wizard.backup.encrypted")}</strong><span>{t("wizard.backup.encryptedHint")}</span></div>
                <div className="welcome-point"><DatabaseBackup aria-hidden /><strong>{t("backup.kept")}</strong><span>{t("backup.keptValue").replace("{n}", digits(14, lang))}</span></div>
              </div>
              {nav()}
            </>
          )}

          {step === "owner" && (
            <>
              {head(t("setup.owner"), t("wizard.owner.hint"))}
              {error && <Notice tone="danger">{error}</Notice>}
              <Field label={t("login.displayName")} hint={t("hint.displayName")} error={e(owner, "display_name")}>
                <TextInput icon={IdCard} autoFocus value={owner.values.display_name} onChange={(x) => owner.set("display_name", x.target.value)} onBlur={() => owner.blur("display_name")} data-testid="setup-display" />
              </Field>
              <Field label={t("login.username")} hint={t("hint.username")} error={e(owner, "username")}>
                <TextInput icon={UserRound} dir="ltr" autoComplete="off" value={owner.values.username} onChange={(x) => owner.set("username", x.target.value)} onBlur={() => owner.blur("username")} data-testid="setup-username" />
              </Field>
              <div className="grid-2">
                <Field label={t("login.password")} hint={t("hint.password")} error={e(owner, "password")}>
                  <PasswordInput icon={LockKeyhole} autoComplete="new-password" value={owner.values.password} onChange={(x) => owner.set("password", x.target.value)} onBlur={() => owner.blur("password")} data-testid="setup-password" />
                </Field>
                <Field label={t("login.passwordRepeat")} error={e(owner, "repeat")}>
                  <PasswordInput icon={LockKeyhole} autoComplete="new-password" value={owner.values.repeat} onChange={(x) => owner.set("repeat", x.target.value)} onBlur={() => owner.blur("repeat")} data-testid="setup-repeat" />
                </Field>
              </div>
              <div className="setup-actions">
                <Button variant="subtle" icon={ArrowLeft} flipIcon onClick={() => go(-1)}>{t("setup.back")}</Button>
                <span className="spacer" />
                <Button variant="primary" icon={ShieldCheck} loading={busy} onClick={create} data-testid="setup-create">
                  {busy ? t("setup.creating") : t("setup.create")}
                </Button>
              </div>
            </>
          )}

          {step === "recovery" && (
            <>
              <div className="setup-head">
                <span className="step-count">{t("wizard.recovery.done")}</span>
                <h1 className="t-title-lg">{t("recovery.title")}</h1>
              </div>
              <Notice tone="warning" title={t("recovery.warningTitle")}>{t("recovery.warning")}</Notice>
              <div className="recovery-key" data-testid="recovery-key">{recoveryKey}</div>
              <div className="row">
                <Button icon={Printer} onClick={() => window.print()}>{t("recovery.print")}</Button>
              </div>
              <Checkbox checked={confirmed} onChange={setConfirmed} testId="recovery-confirm">{t("recovery.confirm")}</Checkbox>
              <div className="setup-actions">
                <span className="spacer" />
                <Button variant="primary" size="lg" iconEnd={ArrowRight} flipIcon disabled={!confirmed} onClick={onDone} data-testid="recovery-continue">{t("recovery.continue")}</Button>
              </div>
              <div className="print-only">
                <h1>{t("recovery.sheetTitle")}</h1>
                <p>{t("recovery.sheetClinic")}: {info.values.clinic_name}</p>
                <p>{t("recovery.sheetDate")}: {formatDateTime(new Date().toISOString(), lang, calendarSystem)}</p>
                <div className="recovery-key">{recoveryKey}</div>
                <p>{t("recovery.warning")}</p>
              </div>
            </>
          )}
        </div>
      </main>
    </div>
  );
}
