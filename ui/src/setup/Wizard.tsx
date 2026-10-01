import { useState } from "react";
import type {
  CalendarSystem,
  ClinicMode,
  DayHours,
  InstallMode,
  Language,
  ThemePreference,
} from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { formatDateTime } from "../lib/dates";
import { useI18n } from "../i18n";
import { useTheme } from "../theme";
import { LangSwitch } from "../pages/LangSwitch";
import { GeoPicker } from "./GeoPicker";

type Step =
  | "welcome"
  | "language"
  | "install_mode"
  | "clinic_info"
  | "hours"
  | "clinic_type"
  | "branding"
  | "trial"
  | "backup"
  | "owner"
  | "recovery";

const STEPS: Step[] = [
  "welcome",
  "language",
  "install_mode",
  "clinic_info",
  "hours",
  "clinic_type",
  "branding",
  "trial",
  "backup",
  "owner",
  "recovery",
];

/** Afghan week (ADR-21): 0 = Saturday … 6 = Friday. Friday is the default day off. */
function defaultHours(): DayHours[] {
  return [0, 1, 2, 3, 4, 5, 6].map((day) => ({
    day,
    closed: day === 6,
    open: day === 6 ? null : "08:00",
    close: day === 6 ? null : "16:00",
  }));
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/** First-run wizard (roadmap 2.5): clinic identity, geography, calendar,
 * branding and Owner + Recovery Key, in one atomic `app.setup` call. */
export function Setup({ onDone }: { onDone: () => void }) {
  const { t, err, lang } = useI18n();
  const { setTheme: applyGlobalTheme } = useTheme();
  const [stepIndex, setStepIndex] = useState(0);
  const step = STEPS[stepIndex];
  const go = (delta: number) => setStepIndex((i) => Math.max(0, Math.min(STEPS.length - 1, i + delta)));

  const [installMode, setInstallMode] = useState<InstallMode>("single");
  const [clinicName, setClinicName] = useState("");
  const [logoDataUrl, setLogoDataUrl] = useState<string | null>(null);
  const [logoFileName, setLogoFileName] = useState<string | null>(null);
  const [provinceId, setProvinceId] = useState<string | null>(null);
  const [districtId, setDistrictId] = useState<string | null>(null);
  const [address, setAddress] = useState("");
  const [phone, setPhone] = useState("");
  const [calendarSystem, setCalendarSystem] = useState<CalendarSystem>("shamsi");
  const [workingHours, setWorkingHours] = useState<DayHours[]>(defaultHours());
  const [clinicMode, setClinicMode] = useState<ClinicMode>("solo");
  const [theme, setTheme] = useState<ThemePreference>("system");
  const [colorPrimary, setColorPrimary] = useState("#0e7490");
  const [colorSecondary, setColorSecondary] = useState("#64748b");
  const [colorAccent, setColorAccent] = useState("#f59e0b");
  const [trialAcknowledged, setTrialAcknowledged] = useState(false);
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [repeat, setRepeat] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [confirmed, setConfirmed] = useState(false);

  const pickLogo = async (file: File | undefined) => {
    if (!file) return;
    setLogoDataUrl(await readFileAsDataUrl(file));
    setLogoFileName(file.name);
  };

  const create = async () => {
    setError("");
    if (password !== repeat) return setError(t("error.mismatch"));
    setBusy(true);
    try {
      const r = await rpc("app.setup", {
        clinic_name: clinicName,
        owner_username: username,
        owner_display_name: displayName || username,
        owner_password: password,
        language: lang as Language,
        install_mode: installMode,
        province_id: provinceId,
        district_id: districtId,
        address: address.trim() || null,
        phone: phone.trim() || null,
        logo_base64: logoDataUrl ? logoDataUrl.split(",")[1] : null,
        logo_file_name: logoFileName,
        calendar_system: calendarSystem,
        clinic_mode: clinicMode,
        theme,
        color_primary: colorPrimary,
        color_secondary: colorSecondary,
        color_accent: colorAccent,
        working_hours: workingHours,
        trial_acknowledged: trialAcknowledged,
      });
      setRecoveryKey(r.recovery_key);
      go(1);
    } catch (e) {
      setError(err(e));
    } finally {
      setBusy(false);
    }
  };

  const setDay = (day: number, patch: Partial<DayHours>) =>
    setWorkingHours((hs) => hs.map((h) => (h.day === day ? { ...h, ...patch } : h)));

  if (step === "recovery") {
    return (
      <div className="center">
        <div className="card wide">
          <h1>{t("recovery.title")}</h1>
          <p className="warn">{t("recovery.warning")}</p>
          <div className="key mono" data-testid="recovery-key">{recoveryKey}</div>
          <div className="actions">
            <button onClick={() => window.print()}>{t("recovery.print")}</button>
          </div>
          <label className="row">
            <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} data-testid="recovery-confirm" />
            {t("recovery.confirm")}
          </label>
          <div className="actions">
            <button className="primary" disabled={!confirmed} onClick={onDone} data-testid="recovery-continue">{t("recovery.continue")}</button>
          </div>
        </div>
        <div className="print-only">
          <h1>{t("recovery.sheetTitle")}</h1>
          <p>{t("recovery.sheetClinic")}: {clinicName}</p>
          <p>{t("recovery.sheetDate")}: {formatDateTime(new Date().toISOString(), lang, calendarSystem)}</p>
          <div className="key mono">{recoveryKey}</div>
          <p>{t("recovery.warning")}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="center">
      <div className="wizard">
        <nav className="stepper" aria-label={t("setup.title")}>
          {STEPS.filter((s) => s !== "recovery").map((s, i) => (
            <div key={s} className={`step${i === stepIndex ? " current" : i < stepIndex ? " done" : ""}`}>
              <span className="dot">{i < stepIndex ? "✓" : i + 1}</span>
              {t(`wizard.step.${s}`)}
            </div>
          ))}
        </nav>
        <div className="card" data-testid={`setup-step-${step}`}>
          {step === "welcome" && (
            <>
              <LangSwitch />
              <h1>{t("wizard.welcome.title")}</h1>
              <p className="muted">{t("wizard.welcome.body")}</p>
              <div className="actions">
                <button className="primary" onClick={() => go(1)} data-testid="wizard-start">{t("wizard.welcome.start")}</button>
              </div>
            </>
          )}

          {step === "language" && (
            <>
              <h2>{t("wizard.step.language")}</h2>
              <LangSwitch />
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" onClick={() => go(1)} data-testid="wizard-next">{t("setup.next")}</button>
              </div>
            </>
          )}

          {step === "install_mode" && (
            <>
              <h2>{t("wizard.installMode.title")}</h2>
              <div className="segmented" role="radiogroup" aria-label={t("wizard.installMode.title")}>
                <button type="button" aria-pressed={installMode === "single"} onClick={() => setInstallMode("single")} data-testid="install-single">
                  <span className="option-title">{t("wizard.installMode.single.title")}</span>
                  <span className="option-hint">{t("wizard.installMode.single.hint")}</span>
                </button>
                <button type="button" disabled title={t("wizard.installMode.comingSoon")} aria-pressed={false} data-testid="install-server">
                  <span className="option-title">{t("wizard.installMode.server.title")} · <span className="badge info">{t("wizard.installMode.comingSoon")}</span></span>
                  <span className="option-hint">{t("wizard.installMode.server.hint")}</span>
                </button>
                <button type="button" disabled title={t("wizard.installMode.comingSoon")} aria-pressed={false} data-testid="install-client">
                  <span className="option-title">{t("wizard.installMode.client.title")} · <span className="badge info">{t("wizard.installMode.comingSoon")}</span></span>
                  <span className="option-hint">{t("wizard.installMode.client.hint")}</span>
                </button>
              </div>
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" onClick={() => go(1)} data-testid="wizard-next">{t("setup.next")}</button>
              </div>
            </>
          )}

          {step === "clinic_info" && (
            <>
              <h2>{t("wizard.clinicInfo.title")}</h2>
              <label htmlFor="clinic">{t("setup.clinicName")}</label>
              <input id="clinic" value={clinicName} onChange={(e) => setClinicName(e.target.value)} data-testid="setup-clinic" />
              <label htmlFor="logo">{t("wizard.clinicInfo.logo")}</label>
              <input id="logo" type="file" accept="image/png,image/jpeg,image/webp" onChange={(e) => pickLogo(e.target.files?.[0])} data-testid="setup-logo" />
              {logoDataUrl && (
                <div className="row" style={{ marginTop: 8 }}>
                  <img src={logoDataUrl} alt="" style={{ width: 40, height: 40, borderRadius: 8, objectFit: "cover" }} />
                  <button type="button" className="ghost" onClick={() => { setLogoDataUrl(null); setLogoFileName(null); }}>{t("wizard.clinicInfo.logoRemove")}</button>
                </div>
              )}
              <GeoPicker
                provinceId={provinceId}
                districtId={districtId}
                onChange={(p, d) => { setProvinceId(p); setDistrictId(d); }}
              />
              <label htmlFor="address">{t("wizard.clinicInfo.address")}</label>
              <input id="address" value={address} onChange={(e) => setAddress(e.target.value)} data-testid="setup-address" />
              <label htmlFor="phone">{t("wizard.clinicInfo.phone")}</label>
              <input id="phone" className="ltr" value={phone} onChange={(e) => setPhone(e.target.value)} data-testid="setup-phone" />
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" disabled={!clinicName.trim()} onClick={() => go(1)} data-testid="wizard-next">{t("setup.next")}</button>
              </div>
            </>
          )}

          {step === "hours" && (
            <>
              <h2>{t("wizard.hours.title")}</h2>
              <label>{t("wizard.hours.calendar")}</label>
              <div className="segmented" role="radiogroup" aria-label={t("wizard.hours.calendar")}>
                <button type="button" aria-pressed={calendarSystem === "shamsi"} onClick={() => setCalendarSystem("shamsi")} data-testid="calendar-shamsi">
                  <span className="option-title">{t("wizard.hours.calendar.shamsi")}</span>
                </button>
                <button type="button" aria-pressed={calendarSystem === "gregorian"} onClick={() => setCalendarSystem("gregorian")} data-testid="calendar-gregorian">
                  <span className="option-title">{t("wizard.hours.calendar.gregorian")}</span>
                </button>
              </div>
              <label style={{ marginTop: 16 }}>{t("wizard.hours.workingHours")}</label>
              <table>
                <tbody>
                  {workingHours.map((h) => (
                    <tr key={h.day}>
                      <td>{t(`wizard.day.${h.day}`)}</td>
                      <td>
                        <label className="row">
                          <input type="checkbox" checked={!h.closed} onChange={(e) => setDay(h.day, e.target.checked ? { closed: false, open: "08:00", close: "16:00" } : { closed: true, open: null, close: null })} />
                          {t("wizard.hours.open")}
                        </label>
                      </td>
                      {!h.closed && (
                        <>
                          <td><input className="ltr" type="time" value={h.open ?? ""} onChange={(e) => setDay(h.day, { open: e.target.value })} /></td>
                          <td><input className="ltr" type="time" value={h.close ?? ""} onChange={(e) => setDay(h.day, { close: e.target.value })} /></td>
                        </>
                      )}
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" onClick={() => go(1)} data-testid="wizard-next">{t("setup.next")}</button>
              </div>
            </>
          )}

          {step === "clinic_type" && (
            <>
              <h2>{t("wizard.clinicType.title")}</h2>
              <div className="segmented" role="radiogroup" aria-label={t("wizard.clinicType.title")}>
                <button type="button" aria-pressed={clinicMode === "solo"} onClick={() => setClinicMode("solo")} data-testid="clinic-mode-solo">
                  <span className="option-title">{t("wizard.clinicType.solo.title")}</span>
                  <span className="option-hint">{t("wizard.clinicType.solo.hint")}</span>
                </button>
                <button type="button" aria-pressed={clinicMode === "multi"} onClick={() => setClinicMode("multi")} data-testid="clinic-mode-multi">
                  <span className="option-title">{t("wizard.clinicType.multi.title")}</span>
                  <span className="option-hint">{t("wizard.clinicType.multi.hint")}</span>
                </button>
              </div>
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" onClick={() => go(1)} data-testid="wizard-next">{t("setup.next")}</button>
              </div>
            </>
          )}

          {step === "branding" && (
            <>
              <h2>{t("wizard.branding.title")}</h2>
              <label>{t("wizard.branding.theme")}</label>
              <div className="segmented" role="radiogroup" aria-label={t("wizard.branding.theme")}>
                {(["light", "dark", "system"] as ThemePreference[]).map((th) => (
                  <button
                    key={th}
                    type="button"
                    aria-pressed={theme === th}
                    onClick={() => {
                      setTheme(th);
                      applyGlobalTheme(th);
                    }}
                    data-testid={`theme-${th}`}
                  >
                    <span className="option-title">{t(`theme.${th}`)}</span>
                  </button>
                ))}
              </div>
              <label style={{ marginTop: 16 }}>{t("wizard.branding.preview")}</label>
              <div className="swatches">
                <div className="swatch">
                  <input type="color" value={colorPrimary} onChange={(e) => setColorPrimary(e.target.value)} data-testid="color-primary" />
                  <span className="muted">{t("wizard.branding.colorPrimary")}</span>
                </div>
                <div className="swatch">
                  <input type="color" value={colorSecondary} onChange={(e) => setColorSecondary(e.target.value)} data-testid="color-secondary" />
                  <span className="muted">{t("wizard.branding.colorSecondary")}</span>
                </div>
                <div className="swatch">
                  <input type="color" value={colorAccent} onChange={(e) => setColorAccent(e.target.value)} data-testid="color-accent" />
                  <span className="muted">{t("wizard.branding.colorAccent")}</span>
                </div>
              </div>
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" onClick={() => go(1)} data-testid="wizard-next">{t("setup.next")}</button>
              </div>
            </>
          )}

          {step === "trial" && (
            <>
              <h2>{t("wizard.trial.title")}</h2>
              <p className="muted">{t("wizard.trial.body")}</p>
              <label className="row">
                <input type="checkbox" checked={trialAcknowledged} onChange={(e) => setTrialAcknowledged(e.target.checked)} data-testid="trial-ack" />
                {t("wizard.trial.ack")}
              </label>
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" onClick={() => go(1)} data-testid="wizard-next">{t("setup.next")}</button>
              </div>
            </>
          )}

          {step === "backup" && (
            <>
              <h2>{t("wizard.backup.title")}</h2>
              <p className="muted">{t("wizard.backup.body")}</p>
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" onClick={() => go(1)} data-testid="wizard-next">{t("setup.next")}</button>
              </div>
            </>
          )}

          {step === "owner" && (
            <>
              <h2>{t("setup.owner")}</h2>
              <label htmlFor="u">{t("login.username")}</label>
              <input id="u" className="ltr" autoComplete="off" value={username} onChange={(e) => setUsername(e.target.value)} data-testid="setup-username" />
              <label htmlFor="d">{t("login.displayName")}</label>
              <input id="d" value={displayName} onChange={(e) => setDisplayName(e.target.value)} data-testid="setup-display" />
              <label htmlFor="p">{t("login.password")}</label>
              <input id="p" type="password" value={password} onChange={(e) => setPassword(e.target.value)} data-testid="setup-password" />
              <label htmlFor="r">{t("login.passwordRepeat")}</label>
              <input id="r" type="password" value={repeat} onChange={(e) => setRepeat(e.target.value)} data-testid="setup-repeat" />
              {error && <div className="error" role="alert">{error}</div>}
              <div className="actions">
                <button onClick={() => go(-1)}>{t("setup.back")}</button>
                <button className="primary" disabled={busy || !username || !password} onClick={create} data-testid="setup-create">
                  {busy ? t("setup.creating") : t("setup.create")}
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
