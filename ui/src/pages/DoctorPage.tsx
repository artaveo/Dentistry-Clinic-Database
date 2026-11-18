// The doctor's own page (OF-050): picture, details, this week, today's and next visits, leave,
// allowed units and what kind of work the doctor did in the last 90 days. Editing stays in the
// existing dialogs (opened from here), so there is one place for each change.
import { useEffect, useMemo, useState } from "react";
import { BadgeCheck, Camera, Clock, Pencil, UserRound } from "lucide-react";
import type { AppointmentInfo, CalendarSystem, ChairInfo, DoctorInfo } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { addDays, todayIso, weekDays, weekdayIndex } from "../lib/calendar";
import { specialtiesText, useCatalog, useSpecialties } from "../lib/clinical";
import { digits, formatDate, formatTime } from "../lib/dates";
import { tr } from "../lib/medical";
import { useI18n } from "../i18n";
import { BackButton, Button } from "../ui/Button";
import { Card, CardHeader, Page } from "../ui/Card";
import { Segmented } from "../ui/Controls";
import { Badge } from "../ui/Feedback";
import { PatientPic, PersonAvatar, illustrations } from "../ui/Avatar";
import { usePatientAvatars } from "../ui/usePatientAvatars";
import { STATUS_TONE } from "./reception/labels";

type Tab = "overview" | "schedule" | "units" | "stats";
const LIVE_OR_DONE = (a: AppointmentInfo) => a.status !== "cancelled" && a.status !== "rescheduled";

export function DoctorPage({ doctor, chairs, calendar, canManage, onBack, onEdit, onSchedule, onPhoto }: {
  doctor: DoctorInfo;
  chairs: ChairInfo[];
  calendar: CalendarSystem;
  canManage: boolean;
  onBack: () => void;
  onEdit: () => void;
  onSchedule: () => void;
  onPhoto: () => void;
}) {
  const { t, lang } = useI18n();
  const specialties = useSpecialties();
  const [tab, setTab] = useState<Tab>("overview");
  const today = todayIso();
  const week = weekDays(today);
  const [weekAppts, setWeekAppts] = useState<AppointmentInfo[] | null>(null);
  const [upcoming, setUpcoming] = useState<AppointmentInfo[] | null>(null);

  useEffect(() => {
    let live = true;
    const base = { doctor_id: doctor.id, chair_id: null, patient_id: null, offset: 0 };
    rpc("appointments.list", { ...base, date_from: week[0], date_to: week[6], statuses: [], limit: 1000 })
      .then((a) => live && setWeekAppts(a.filter(LIVE_OR_DONE)))
      .catch(() => live && setWeekAppts([]));
    rpc("appointments.list", { ...base, date_from: addDays(today, 1), date_to: addDays(today, 60), statuses: ["scheduled", "confirmed"], limit: 5 })
      .then((a) => live && setUpcoming(a))
      .catch(() => live && setUpcoming([]));
    return () => {
      live = false;
    };
  }, [doctor.id, today]);

  const todays = (weekAppts ?? []).filter((a) => a.date === today).sort((a, b) => a.start_time.localeCompare(b.start_time));
  const avatars = usePatientAvatars([...todays, ...(upcoming ?? [])].map((a) => a.patient_id));
  const onLeave = (d: string) => doctor.leaves.some((l) => l.start_date <= d && d <= l.end_date);
  const hoursOf = (day: number) => doctor.hours.filter((h) => h.day === day).map((h) => `${formatTime(h.start, lang)}–${formatTime(h.end, lang)}`);
  const leaves = doctor.leaves.filter((l) => l.end_date >= today);
  const usable = doctor.chair_ids.length ? chairs.filter((c) => doctor.chair_ids.includes(c.id)) : chairs.filter((c) => c.status === "active");
  const figure = doctor.sex === "female" ? illustrations.doctorFemaleFigure : doctor.sex === "male" ? illustrations.doctorMaleFigure : null;

  const visitLine = (a: AppointmentInfo, withDate: boolean) => (
    <li key={a.id} className="row" style={{ justifyContent: "space-between", flexWrap: "nowrap" }} data-testid={`doctor-visit-${a.id}`}>
      <span className="person-line">
        <PatientPic id={a.patient_id} name={a.patient_name} avatars={avatars} />
        <span className="person-text">
          <span className="cell-strong">{a.patient_name}</span>
          <span className="subtle t-caption num">{withDate ? `${formatDate(a.date + "T06:00:00Z", lang, calendar)} · ` : ""}{formatTime(a.start_time, lang)}{a.chair_name ? ` · ${a.chair_name}` : ""}</span>
        </span>
      </span>
      <Badge tone={STATUS_TONE[a.status]}>{t(`appt.status.${a.status}`)}</Badge>
    </li>
  );

  return (
    <Page testId="page-doctor">
      <div><BackButton onBack={onBack} label={t("doctor.back")} testId="doctor-back" /></div>
      <Card>
        <div className="doctor-hero">
          {doctor.photo_id || !figure ? (
            <span className="doctor-figure-photo"><PersonAvatar kind="doctor" name={doctor.full_name} sex={doctor.sex} photoId={doctor.photo_id} ring={doctor.color} size="xl" testId="doctor-page-avatar" /></span>
          ) : (
            <img className="doctor-figure" src={figure} alt={t("doctor.photoAlt").replace("{name}", doctor.full_name)} data-testid="doctor-page-figure" />
          )}
          <div className="stack" style={{ minWidth: 0 }}>
            <div className="row" style={{ alignItems: "center" }}>
              <h1 className="t-title" style={{ margin: 0 }} data-testid="page-title">{doctor.full_name}</h1>
              {doctor.status === "active" ? <Badge tone="success" dot>{t("users.active")}</Badge> : <Badge dot>{t("users.inactive")}</Badge>}
            </div>
            <span className="muted">{specialtiesText(doctor, specialties, lang) || "—"}</span>
            <div className="doctor-facts">
              {doctor.license_number && <span><BadgeCheck aria-hidden />{t("doctors.license")}: <bdi className="ltr">{doctor.license_number}</bdi></span>}
              <span><UserRound aria-hidden />{doctor.username ? <>{t("doctor.login")}: <bdi className="ltr">{doctor.username}</bdi></> : t("doctor.noLogin")}</span>
              <span><span className="dot-swatch" style={{ background: doctor.color }} aria-hidden />{t("doctors.color")}</span>
            </div>
            {canManage && (
              <div className="row">
                <Button size="sm" icon={Pencil} onClick={onEdit} data-testid={`doctor-edit-${doctor.full_name}`}>{t("users.edit")}</Button>
                <Button size="sm" icon={Clock} onClick={onSchedule} data-testid={`doctor-schedule-${doctor.full_name}`}>{t("doctors.editSchedule")}</Button>
                <Button size="sm" variant="subtle" icon={Camera} onClick={onPhoto} data-testid="doctor-photo-open">{t(doctor.photo_id ? "avatar.photo.change" : "avatar.photo.add")}</Button>
              </div>
            )}
          </div>
        </div>
      </Card>

      <Segmented<Tab>
        value={tab}
        onChange={setTab}
        label={doctor.full_name}
        options={[
          { value: "overview", label: t("doctor.tab.overview"), testId: "doctor-tab-overview" },
          { value: "schedule", label: t("doctor.tab.schedule"), testId: "doctor-tab-schedule" },
          { value: "units", label: t("doctor.tab.units"), testId: "doctor-tab-units" },
          { value: "stats", label: t("doctor.tab.stats"), testId: "doctor-tab-stats" },
        ]}
      />

      {tab === "overview" && (
        <>
          <Card>
            <CardHeader title={t("doctor.week")} />
            <div className="doctor-week" data-testid="doctor-week">
              {week.map((d) => {
                const day = weekdayIndex(d);
                const hours = hoursOf(day);
                const count = (weekAppts ?? []).filter((a) => a.date === d).length;
                const off = onLeave(d) || (doctor.hours.length > 0 && hours.length === 0);
                return (
                  <div key={d} className={`day ${d === today ? "today" : ""} ${off ? "off" : ""}`}>
                    <span className="day-name">{t(`wizard.day.${day}`)}</span>
                    {onLeave(d) ? <span className="day-hours">{t("doctor.onLeave")}</span> : hours.length ? hours.map((h) => <span key={h} className="day-hours num">{h}</span>) : <span className="day-hours">{doctor.hours.length ? t("doctor.dayOff") : "—"}</span>}
                    {count > 0 && <span className="day-count">{t("doctor.appts").replace("{n}", digits(count, lang))}</span>}
                  </div>
                );
              })}
            </div>
          </Card>
          <div className="grid-2">
            <Card>
              <CardHeader title={t("doctor.today")} />
              {todays.length ? <ul className="plain-list stack" data-testid="doctor-today">{todays.map((a) => visitLine(a, false))}</ul> : <span className="subtle">{t("doctor.none")}</span>}
            </Card>
            <Card>
              <CardHeader title={t("doctor.upcoming")} />
              {upcoming?.length ? <ul className="plain-list stack">{upcoming.map((a) => visitLine(a, true))}</ul> : <span className="subtle">{t("doctor.none")}</span>}
            </Card>
          </div>
          <Card>
            <CardHeader title={t("doctor.leaves")} />
            {leaves.length ? (
              <ul className="plain-list stack">
                {leaves.map((l) => <li key={l.id}>{formatDate(l.start_date + "T06:00:00Z", lang, calendar)}{l.end_date !== l.start_date && ` – ${formatDate(l.end_date + "T06:00:00Z", lang, calendar)}`}{l.reason ? ` · ${l.reason}` : ""}</li>)}
              </ul>
            ) : <span className="subtle">{t("doctor.noLeave")}</span>}
          </Card>
        </>
      )}

      {tab === "schedule" && (
        <Card>
          <CardHeader title={t("doctor.tab.schedule")} description={t("sched.hint")} actions={canManage ? <Button size="sm" icon={Clock} onClick={onSchedule}>{t("doctors.editSchedule")}</Button> : undefined} />
          <dl className="kv" data-testid="doctor-schedule-summary">
            {[0, 1, 2, 3, 4, 5, 6].map((day) => {
              const hours = hoursOf(day);
              const breaks = doctor.breaks.filter((b) => b.day === day).map((b) => `${formatTime(b.start, lang)}–${formatTime(b.end, lang)}`);
              return (
                <div key={day}>
                  <dt>{t(`wizard.day.${day}`)}</dt>
                  <dd>{hours.length ? hours.join("، ") : doctor.hours.length ? t("doctor.dayOff") : t("doctors.noHours")}{breaks.length ? <span className="subtle t-caption"> · {t("sched.breaks")}: {breaks.join("، ")}</span> : null}</dd>
                </div>
              );
            })}
          </dl>
        </Card>
      )}

      {tab === "units" && (
        <Card flush>
          <CardHeader title={t("doctor.tab.units")} description={doctor.chair_ids.length ? t("doctor.units.hint") : t("doctor.units.any")} />
          <div className="unit-grid" data-testid="doctor-units">
            {usable.map((c) => (
              <div key={c.id} className={`unit-tile ${c.status === "active" ? "" : "inactive"}`}>
                <img src={illustrations.dentalUnit} alt="" />
                <span className="unit-name">{c.name}</span>
                <span />
              </div>
            ))}
            {!usable.length && <span className="subtle">{t("chairs.empty")}</span>}
          </div>
        </Card>
      )}

      {tab === "stats" && <DoctorStats doctorId={doctor.id} />}
    </Page>
  );
}

/** Share of completed visits by service group, last 90 days (only visits that name a service). */
function DoctorStats({ doctorId }: { doctorId: string }) {
  const { t, lang } = useI18n();
  const catalog = useCatalog();
  const [visits, setVisits] = useState<AppointmentInfo[] | null>(null);
  useEffect(() => {
    const today = todayIso();
    rpc("appointments.list", { date_from: addDays(today, -90), date_to: today, doctor_id: doctorId, chair_id: null, patient_id: null, statuses: ["completed"], limit: 1000, offset: 0 })
      .then(setVisits)
      .catch(() => setVisits([]));
  }, [doctorId]);
  const rows = useMemo(() => {
    if (!visits) return null;
    const counts = new Map<string, number>();
    for (const v of visits) {
      const serviceId = v.service_id ?? v.treatment?.service_id;
      if (!serviceId) continue;
      const cat = catalog?.services.find((s) => s.id === serviceId)?.category_id ?? "";
      counts.set(cat, (counts.get(cat) ?? 0) + 1);
    }
    const total = [...counts.values()].reduce((a, b) => a + b, 0);
    return [...counts.entries()]
      .map(([cat, n]) => ({ cat, n, pct: total ? Math.round((n * 100) / total) : 0, label: (cat && tr(catalog?.categories.find((c) => c.id === cat)?.name ?? { fa: "", ps: "", en: "" }, lang)) || t("doctor.stats.uncategorized") }))
      .sort((a, b) => b.n - a.n);
  }, [visits, catalog, lang]);
  return (
    <Card>
      <CardHeader title={t("doctor.stats.title")} description={t("doctor.stats.hint")} />
      {rows && rows.length === 0 && <span className="subtle" data-testid="doctor-stats-empty">{t("doctor.stats.empty")}</span>}
      {rows && rows.length > 0 && (
        <ul className="plain-list stack" data-testid="doctor-stats">
          {rows.map((r) => (
            <li key={r.cat} className="stack" style={{ gap: 6 }}>
              <span className="row" style={{ justifyContent: "space-between" }}>
                <span className="cell-strong">{r.label}</span>
                <span className="subtle num">{digits(r.pct, lang)}{lang === "en" ? "%" : "٪"} ·{t("doctor.stats.visits").replace("{n}", digits(r.n, lang))}</span>
              </span>
              <span className="stat-bar" aria-hidden><span style={{ width: `${r.pct}%` }} /></span>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}
