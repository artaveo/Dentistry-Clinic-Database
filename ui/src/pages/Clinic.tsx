import { useEffect, useState } from "react";
import type { CalendarSystem, ClinicMode, ClinicProfile, ThemePreference } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { useI18n } from "../i18n";
import { GeoPicker } from "../setup/GeoPicker";
import { useTheme } from "../theme";

/** Edits the clinic profile the Setup Wizard collected (roadmap 2.5),
 * reusing the same `clinic.get`/`clinic.update` the wizard's atomic
 * `app.setup` call seeded. */
export function ClinicPage() {
  const { t, err } = useI18n();
  const { setTheme } = useTheme();
  const [c, setC] = useState<ClinicProfile | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    rpc("clinic.get", {}).then(setC).catch((e) => setMsg({ ok: false, text: err(e) }));
  }, []);

  if (!c) return <div className="muted">{t("common.loading")}</div>;

  const save = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      const saved = await rpc("clinic.update", c);
      setC(saved);
      setTheme(saved.theme);
      setMsg({ ok: true, text: t("clinic.saved") });
    } catch (x) {
      setMsg({ ok: false, text: err(x) });
    }
  };

  return (
    <form className="card wide" onSubmit={save}>
      <h2>{t("clinic.title")}</h2>
      <GeoPicker
        provinceId={c.province_id}
        districtId={c.district_id}
        onChange={(province_id, district_id) => setC({ ...c, province_id, district_id })}
      />
      <label>{t("clinic.address")}</label>
      <input value={c.address ?? ""} onChange={(e) => setC({ ...c, address: e.target.value })} data-testid="clinic-address" />
      <label>{t("clinic.phone")}</label>
      <input className="ltr" value={c.phone ?? ""} onChange={(e) => setC({ ...c, phone: e.target.value })} data-testid="clinic-phone" />

      <label>{t("clinic.calendarSystem")}</label>
      <select value={c.calendar_system} onChange={(e) => setC({ ...c, calendar_system: e.target.value as CalendarSystem })} data-testid="clinic-calendar">
        <option value="shamsi">{t("wizard.hours.calendar.shamsi")}</option>
        <option value="gregorian">{t("wizard.hours.calendar.gregorian")}</option>
      </select>

      <label>{t("clinic.clinicMode")}</label>
      <select value={c.clinic_mode} onChange={(e) => setC({ ...c, clinic_mode: e.target.value as ClinicMode })} data-testid="clinic-mode">
        <option value="solo">{t("wizard.clinicType.solo.title")}</option>
        <option value="multi">{t("wizard.clinicType.multi.title")}</option>
      </select>

      <label>{t("clinic.theme")}</label>
      <select value={c.theme} onChange={(e) => setC({ ...c, theme: e.target.value as ThemePreference })} data-testid="clinic-theme">
        <option value="light">{t("theme.light")}</option>
        <option value="dark">{t("theme.dark")}</option>
        <option value="system">{t("theme.system")}</option>
      </select>

      <label>{t("clinic.colors")}</label>
      <div className="swatches">
        <div className="swatch">
          <input type="color" value={c.color_primary} onChange={(e) => setC({ ...c, color_primary: e.target.value })} />
        </div>
        <div className="swatch">
          <input type="color" value={c.color_secondary} onChange={(e) => setC({ ...c, color_secondary: e.target.value })} />
        </div>
        <div className="swatch">
          <input type="color" value={c.color_accent} onChange={(e) => setC({ ...c, color_accent: e.target.value })} />
        </div>
      </div>

      <label>{t("clinic.workingHours")}</label>
      <table>
        <tbody>
          {c.working_hours.map((h) => (
            <tr key={h.day}>
              <td>{t(`wizard.day.${h.day}`)}</td>
              <td>
                <label className="row">
                  <input
                    type="checkbox"
                    checked={!h.closed}
                    onChange={(e) =>
                      setC({
                        ...c,
                        working_hours: c.working_hours.map((d) =>
                          d.day === h.day
                            ? e.target.checked
                              ? { ...d, closed: false, open: "08:00", close: "16:00" }
                              : { ...d, closed: true, open: null, close: null }
                            : d,
                        ),
                      })
                    }
                  />
                  {t("wizard.hours.open")}
                </label>
              </td>
              {!h.closed && (
                <>
                  <td>
                    <input
                      className="ltr"
                      type="time"
                      value={h.open ?? ""}
                      onChange={(e) =>
                        setC({ ...c, working_hours: c.working_hours.map((d) => (d.day === h.day ? { ...d, open: e.target.value } : d)) })
                      }
                    />
                  </td>
                  <td>
                    <input
                      className="ltr"
                      type="time"
                      value={h.close ?? ""}
                      onChange={(e) =>
                        setC({ ...c, working_hours: c.working_hours.map((d) => (d.day === h.day ? { ...d, close: e.target.value } : d)) })
                      }
                    />
                  </td>
                </>
              )}
            </tr>
          ))}
        </tbody>
      </table>

      {msg && <div className={msg.ok ? "success" : "error"} data-testid="clinic-message">{msg.text}</div>}
      <div className="actions">
        <button className="primary" type="submit" data-testid="clinic-save">{t("common.save")}</button>
      </div>
    </form>
  );
}
