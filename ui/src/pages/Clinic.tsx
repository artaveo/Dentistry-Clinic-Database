import { useEffect, useRef, useState } from "react";
import { Building2, CalendarDays, Clock, ImageUp, MapPin, Moon, Monitor, Palette, Phone, Save, Sun, Trash2, User, Users } from "lucide-react";
import type { CalendarSystem, ClinicMode, ClinicProfile, ThemePreference } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { applyBrand } from "../lib/color";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { GeoPicker } from "../setup/GeoPicker";
import { HoursEditor, badHours } from "../setup/HoursEditor";
import { useTheme } from "../theme";
import { Button } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { OptionCards, Segmented } from "../ui/Controls";
import { Field, TextInput } from "../ui/Field";
import { ErrorState, Loading, Notice } from "../ui/Feedback";
import { ClinicMark } from "../ui/Brand";
import { useToast } from "../ui/Toast";
import { ColorSwatches } from "../setup/ColorSwatches";
import { readLogo } from "../setup/logo";

/** Edits the clinic profile the Setup Wizard collected (roadmap 2.5), and the logo (2.1b). */
export function ClinicPage({ logo, onLogoChange, onSaved }: { logo: string | null; onLogoChange: (l: string | null) => void; onSaved: () => void }) {
  const { err } = useI18n();
  const [c, setC] = useState<ClinicProfile | null>(null);
  const [error, setError] = useState("");
  const load = () => rpc("clinic.get", {}).then(setC).catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, []);
  if (error && !c) return <Page><ErrorState message={error} onRetry={load} /></Page>;
  if (!c) return <Page><Loading /></Page>;
  return <ClinicForm initial={c} logo={logo} onLogoChange={onLogoChange} onSaved={onSaved} />;
}

function ClinicForm({ initial, logo, onLogoChange, onSaved }: { initial: ClinicProfile; logo: string | null; onLogoChange: (l: string | null) => void; onSaved: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const { setTheme } = useTheme();
  const [c, setC] = useState(initial);
  const form = useForm({ address: initial.address ?? "", phone: initial.phone ?? "" }, { phone: v.phone });
  const [busy, setBusy] = useState(false);
  const [logoBusy, setLogoBusy] = useState(false);
  const [error, setError] = useState("");
  const [hoursError, setHoursError] = useState("");
  const fileRef = useRef<HTMLInputElement>(null);
  const savedColors = useRef([initial.color_primary, initial.color_accent] as const);

  // Live preview of the clinic colours; restored if the page is left unsaved.
  useEffect(() => applyBrand(c.color_primary, c.color_accent), [c.color_primary, c.color_accent]);
  useEffect(() => () => applyBrand(...savedColors.current), []);

  const save = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setHoursError("");
    const okHours = !badHours(c.working_hours);
    if (!okHours) setHoursError(t("rule.working_hours"));
    if (!form.validate() || !okHours) return;
    setBusy(true);
    try {
      const saved = await rpc("clinic.update", { ...c, address: form.values.address.trim() || null, phone: form.values.phone.trim() || null });
      setC(saved);
      setTheme(saved.theme);
      savedColors.current = [saved.color_primary, saved.color_accent];
      onSaved();
      toast.success(t("clinic.saved"));
    } catch (x) {
      const f = (x as { field?: string }).field;
      if (f === "working_hours") setHoursError(t("rule.working_hours"));
      else if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  const pickLogo = async (file: File | undefined) => {
    if (!file) return;
    setLogoBusy(true);
    try {
      const l = await readLogo(file);
      const r = await rpc("clinic.set_logo", { logo_base64: l.base64, logo_file_name: l.name });
      onLogoChange(r.data_url);
      toast.success(t("clinic.logoSaved"));
    } catch (x) {
      toast.error(x instanceof Error && x.message.startsWith("rule.") ? t(x.message) : err(x));
    } finally {
      setLogoBusy(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  };
  const removeLogo = async () => {
    setLogoBusy(true);
    try {
      await rpc("clinic.set_logo", { logo_base64: null, logo_file_name: null });
      onLogoChange(null);
    } catch (x) {
      toast.error(err(x));
    } finally {
      setLogoBusy(false);
    }
  };

  return (
    <Page testId="page-clinic">
      <PageHeader title={t("clinic.title")} description={t("clinic.subtitle")} />
      <form className="stack-lg" onSubmit={save} noValidate>
        {error && <Notice tone="danger" testId="clinic-message">{error}</Notice>}

        <Card>
          <CardHeader icon={Building2} title={t("clinic.identity")} description={t("clinic.identityHint")} />
          <div className="logo-picker">
            <ClinicMark name={c.name} logo={logo} size="lg" />
            <div className="logo-text">
              <span className="t-title">{c.name}</span>
              <span className="subtle t-caption">{t("clinic.logoHint")}</span>
            </div>
            <input ref={fileRef} type="file" accept="image/png,image/jpeg,image/webp" hidden onChange={(e) => pickLogo(e.target.files?.[0])} data-testid="clinic-logo-file" />
            <Button icon={ImageUp} loading={logoBusy} onClick={() => fileRef.current?.click()}>{logo ? t("clinic.logoChange") : t("clinic.logoUpload")}</Button>
            {logo && <Button variant="subtle" icon={Trash2} onClick={removeLogo} disabled={logoBusy}>{t("wizard.clinicInfo.logoRemove")}</Button>}
          </div>
        </Card>

        <Card>
          <CardHeader icon={MapPin} title={t("clinic.location")} description={t("clinic.locationHint")} />
          <div className="grid-2">
            <GeoPicker provinceId={c.province_id} districtId={c.district_id} onChange={(province_id, district_id) => setC({ ...c, province_id, district_id })} />
            <Field label={t("clinic.address")} optional className="span-2">
              <TextInput icon={MapPin} value={form.values.address} onChange={(e) => form.set("address", e.target.value)} data-testid="clinic-address" />
            </Field>
            <Field label={t("clinic.phone")} optional hint={t("hint.phone")} error={form.error("phone") && t(form.error("phone")!)}>
              <TextInput icon={Phone} dir="ltr" inputMode="tel" value={form.values.phone} onChange={(e) => form.set("phone", e.target.value)} data-testid="clinic-phone" />
            </Field>
          </div>
        </Card>

        <Card>
          <CardHeader icon={CalendarDays} title={t("clinic.practice")} description={t("clinic.practiceHint")} />
          <div className="stack">
            <Field label={t("clinic.calendarSystem")} hint={t("hint.calendar")}>
              <Segmented<CalendarSystem>
                value={c.calendar_system}
                onChange={(calendar_system) => setC({ ...c, calendar_system })}
                label={t("clinic.calendarSystem")}
                options={[
                  { value: "shamsi", label: t("wizard.hours.calendar.shamsi"), testId: "clinic-calendar-shamsi" },
                  { value: "gregorian", label: t("wizard.hours.calendar.gregorian"), testId: "clinic-calendar-gregorian" },
                ]}
              />
            </Field>
            <OptionCards<ClinicMode>
              value={c.clinic_mode}
              onChange={(clinic_mode) => setC({ ...c, clinic_mode })}
              label={t("clinic.clinicMode")}
              columns={2}
              options={[
                { value: "solo", title: t("wizard.clinicType.solo.title"), hint: t("wizard.clinicType.solo.hint"), icon: User, testId: "clinic-mode-solo" },
                { value: "multi", title: t("wizard.clinicType.multi.title"), hint: t("wizard.clinicType.multi.hint"), icon: Users, testId: "clinic-mode-multi" },
              ]}
            />
          </div>
        </Card>

        <Card>
          <CardHeader icon={Palette} title={t("clinic.appearance")} description={t("clinic.appearanceHint")} />
          <div className="stack">
            <Field label={t("clinic.theme")} hint={t("hint.theme")}>
              <Segmented<ThemePreference>
                value={c.theme}
                onChange={(theme) => setC({ ...c, theme })}
                label={t("clinic.theme")}
                options={[
                  { value: "light", label: t("theme.light"), icon: Sun },
                  { value: "dark", label: t("theme.dark"), icon: Moon },
                  { value: "system", label: t("theme.system"), icon: Monitor },
                ]}
              />
            </Field>
            <ColorSwatches
              primary={c.color_primary}
              secondary={c.color_secondary}
              accent={c.color_accent}
              onChange={(k, val) => setC({ ...c, [k]: val })}
            />
          </div>
        </Card>

        <Card>
          <CardHeader icon={Clock} title={t("clinic.workingHours")} description={t("clinic.workingHoursHint")} />
          {hoursError && <Notice tone="danger">{hoursError}</Notice>}
          <HoursEditor hours={c.working_hours} onChange={(working_hours) => { setC({ ...c, working_hours }); setHoursError(""); }} />
        </Card>

        <div className="row" style={{ justifyContent: "flex-end" }}>
          <Button type="submit" variant="primary" icon={Save} loading={busy} data-testid="clinic-save">{t("common.save")}</Button>
        </div>
      </form>
    </Page>
  );
}
