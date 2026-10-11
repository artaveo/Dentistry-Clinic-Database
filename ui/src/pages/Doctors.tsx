import { useCallback, useEffect, useRef, useState } from "react";
import { CalendarOff, Clock, Pencil, Plus, Stethoscope, Trash2, Armchair } from "lucide-react";
import type { CalendarSystem, ChairInfo, ClinicProfile, DoctorInfo, ScheduleSlot, UserInfo } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { todayIso } from "../lib/calendar";
import { digits, formatDate } from "../lib/dates";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button, IconButton } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { ChipGroup, Switch } from "../ui/Controls";
import { specialtiesText, useSpecialties } from "../lib/clinical";
import { tr } from "../lib/medical";
import { DateField } from "../ui/DateField";
import { Field, Select, TextInput } from "../ui/Field";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { TimePicker12 } from "../ui/TimePicker";
import { useToast } from "../ui/Toast";
import { ruleText } from "./reception/labels";

const COLORS = ["#0e7490", "#f59e0b", "#7c3aed", "#16a34a", "#dc2626", "#db2777", "#2563eb", "#0d9488", "#ea580c", "#64748b"];

/** Doctors and chairs (4.2): profile, weekly schedule, breaks, usable chairs and leave. */
export function DoctorsPage({ clinic }: { clinic: ClinicProfile | null }) {
  const { t, err, lang } = useI18n();
  const specialties = useSpecialties();
  const calendar: CalendarSystem = clinic?.calendar_system ?? "shamsi";
  const [doctors, setDoctors] = useState<DoctorInfo[] | null>(null);
  const [chairs, setChairs] = useState<ChairInfo[] | null>(null);
  const [error, setError] = useState("");
  const [doctorDialog, setDoctorDialog] = useState<{ doctor?: DoctorInfo } | null>(null);
  const [schedule, setSchedule] = useState<string | null>(null);
  const [chairDialog, setChairDialog] = useState<{ chair?: ChairInfo } | null>(null);

  // Every reload and every saved doctor bumps this, so an older reload still in flight (started by,
  // say, removing a leave) can never put an outdated doctor back after a newer save.
  const seq = useRef(0);
  const load = useCallback(() => {
    const mine = ++seq.current;
    Promise.all([rpc("doctors.list", { include_inactive: true }), rpc("chairs.list", { include_inactive: true })])
      .then(([d, c]) => { if (mine !== seq.current) return; setDoctors(d); setChairs(c); setError(""); })
      .catch((e) => mine === seq.current && !isSessionError(e) && setError(err(e)));
  }, []);
  useEffect(load, [load]);
  /** A save's answer goes into the list at once: the schedule reopened right after shows it, with its new version. */
  const saved = (d?: DoctorInfo) => {
    if (d) {
      seq.current++;
      setDoctors((list) => list?.map((x) => (x.id === d.id ? d : x)) ?? null);
    }
    load();
  };

  const scheduled = doctors?.find((d) => d.id === schedule) ?? null;
  const summary = (d: DoctorInfo) => {
    const days = new Set(d.hours.map((h) => h.day));
    return days.size ? `${digits(days.size, lang)} ${t("doctors.daysPerWeek")}` : t("doctors.noHours");
  };

  return (
    <Page testId="page-doctors">
      <PageHeader title={t("nav.doctors")} description={t("doctors.subtitle")} />
      {error && <ErrorState message={error} onRetry={load} />}
      <Card flush>
        <CardHeader icon={Stethoscope} title={t("doctors.title")} description={t("doctors.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setDoctorDialog({})} data-testid="doctor-add-open">{t("doctors.add")}</Button>} />
        <div className="table-wrap">
          <table className="table" data-testid="doctor-list">
            <thead><tr><th>{t("doctors.name")}</th><th>{t("doctors.specialty")}</th><th>{t("doctors.schedule")}</th><th>{t("users.status")}</th><th className="cell-actions"><span className="visually-hidden">{t("users.edit")}</span></th></tr></thead>
            <tbody>
              {!doctors && <SkeletonRows cols={5} />}
              {doctors?.map((d) => (
                <tr key={d.id} data-testid={`doctor-row-${d.id}`}>
                  <td><span className="row"><span className="dot-swatch lg" style={{ background: d.color }} aria-hidden /><span className="cell-strong">{d.full_name}</span></span>{d.username && <span className="subtle t-caption"><br /><bdi className="ltr">{d.username}</bdi></span>}</td>
                  <td>{specialtiesText(d, specialties, lang) || "—"}{d.license_number && <span className="subtle t-caption"><br />{t("doctors.license")}: <bdi className="ltr">{d.license_number}</bdi></span>}</td>
                  <td>{summary(d)}{d.leaves.length > 0 && <span className="subtle t-caption"><br />{t("doctors.leaves")}: {digits(d.leaves.length, lang)}</span>}</td>
                  <td>{d.status === "active" ? <Badge tone="success" dot>{t("users.active")}</Badge> : <Badge dot>{t("users.inactive")}</Badge>}</td>
                  <td className="cell-actions">
                    <div className="row" style={{ justifyContent: "flex-end" }}>
                      <Button size="sm" icon={Clock} onClick={() => setSchedule(d.id)} data-testid={`doctor-schedule-${d.full_name}`}>{t("doctors.editSchedule")}</Button>
                      <IconButton icon={Pencil} label={t("users.edit")} size="sm" onClick={() => setDoctorDialog({ doctor: d })} data-testid={`doctor-edit-${d.full_name}`} />
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {doctors?.length === 0 && <EmptyState icon={Stethoscope} title={t("doctors.empty")} action={<Button variant="primary" icon={Plus} onClick={() => setDoctorDialog({})}>{t("doctors.add")}</Button>}>{t("doctors.emptyHint")}</EmptyState>}
        </div>
      </Card>

      <Card flush>
        <CardHeader icon={Armchair} title={t("chairs.title")} description={t("chairs.hint")} actions={<Button variant="primary" icon={Plus} onClick={() => setChairDialog({})} data-testid="chair-add-open">{t("chairs.add")}</Button>} />
        <div className="table-wrap">
          <table className="table" data-testid="chair-list">
            <thead><tr><th>{t("chairs.name")}</th><th>{t("users.status")}</th><th className="cell-actions"><span className="visually-hidden">{t("users.edit")}</span></th></tr></thead>
            <tbody>
              {!chairs && <SkeletonRows cols={3} />}
              {chairs?.map((c) => (
                <tr key={c.id}>
                  <td className="cell-strong">{c.name}</td>
                  <td>{c.status === "active" ? <Badge tone="success" dot>{t("users.active")}</Badge> : <Badge dot>{t("users.inactive")}</Badge>}</td>
                  <td className="cell-actions"><IconButton icon={Pencil} label={t("users.edit")} size="sm" onClick={() => setChairDialog({ chair: c })} data-testid={`chair-edit-${c.name}`} /></td>
                </tr>
              ))}
            </tbody>
          </table>
          {chairs?.length === 0 && <EmptyState icon={Armchair} title={t("chairs.empty")}>{t("chairs.emptyHint")}</EmptyState>}
        </div>
      </Card>

      {doctorDialog && <DoctorDialog doctor={doctorDialog.doctor} onClose={() => setDoctorDialog(null)} onSaved={saved} />}
      {chairDialog && <ChairDialog chair={chairDialog.chair} onClose={() => setChairDialog(null)} onSaved={load} />}
      {scheduled && chairs && <ScheduleDialog doctor={scheduled} chairs={chairs.filter((c) => c.status === "active")} calendar={calendar} onClose={() => setSchedule(null)} onSaved={saved} />}
    </Page>
  );
}

function DoctorDialog({ doctor, onClose, onSaved }: { doctor?: DoctorInfo; onClose: () => void; onSaved: (d?: DoctorInfo) => void }) {
  const { t, err, lang } = useI18n();
  const specialties = useSpecialties();
  const [specialtyIds, setSpecialtyIds] = useState<string[]>(doctor?.specialty_ids ?? []);
  const toast = useToast();
  const [users, setUsers] = useState<UserInfo[] | null>(null);
  const [active, setActive] = useState(doctor ? doctor.status === "active" : true);
  const [color, setColor] = useState(doctor?.color ?? COLORS[0]);
  const form = useForm(
    { full_name: doctor?.full_name ?? "", license_number: doctor?.license_number ?? "", user_id: doctor?.user_id ?? "" },
    { full_name: v.fullName, license_number: (s) => ([...s.trim()].length > 50 ? "rule.license_length" : null) },
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  // Linking a login needs permission to list users; without it the choice is simply not offered.
  useEffect(() => {
    rpc("users.list", {}).then(setUsers).catch(() => setUsers(null));
  }, []);

  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    // An old free-text specialty (before v0.5.0) is kept until the doctor is given specialties from the list.
    const base = {
      full_name: form.values.full_name.trim(),
      specialty: specialtyIds.length ? null : (doctor?.specialty ?? null),
      specialty_ids: specialtyIds,
      license_number: form.values.license_number.trim() || null,
      color,
      user_id: form.values.user_id || null,
    };
    try {
      const d = doctor
        ? await rpc("doctors.update", { ...base, id: doctor.id, version: doctor.version, status: active ? "active" : "inactive" })
        : await rpc("doctors.create", base);
      toast.success(t("common.saved"));
      onSaved(d);
      onClose();
    } catch (x) {
      if (!form.serverError(x)) setError(ruleText(t, x, err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={doctor ? t("doctors.edit") : t("doctors.add")}
      onClose={onClose}
      testId="doctor-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} type="submit" form="doctor-form" data-testid="doctor-save">{t("common.save")}</Button></>}
    >
      <form id="doctor-form" className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("doctors.name")} hint={t("hint.doctorName")} error={form.error("full_name") && t(form.error("full_name")!)}>
          <TextInput value={form.values.full_name} onChange={(e) => form.set("full_name", e.target.value)} onBlur={() => form.blur("full_name")} data-testid="doctor-name" />
        </Field>
        {/* M2: one or more specialties from the clinic's list (Settings → Clinical lists). */}
        <div className="field">
          <span className="field-label">{t("doctors.specialties")}<span className="optional">{t("common.optional")}</span></span>
          <ChipGroup
            label={t("doctors.specialties")}
            options={(specialties ?? []).filter((s) => s.is_active || specialtyIds.includes(s.id)).map((s) => ({ value: s.id, label: tr(s.label, lang), testId: `doctor-specialty-${s.code}` }))}
            selected={specialtyIds}
            onChange={setSpecialtyIds}
            testId="doctor-specialties"
          />
          <span className="field-hint">{doctor?.specialty && !specialtyIds.length ? t("doctors.oldSpecialty").replace("{text}", doctor.specialty) : t("doctors.specialtiesHint")}</span>
        </div>
        <Field label={t("doctors.license")} hint={t("doctors.licenseHint")} optional error={form.error("license_number") && t(form.error("license_number")!)}>
          <TextInput value={form.values.license_number} dir="ltr" onChange={(e) => form.set("license_number", e.target.value)} data-testid="doctor-license" />
        </Field>
        <div className="field">
          <span className="field-label">{t("doctors.color")}</span>
          <div className="color-choices" role="radiogroup" aria-label={t("doctors.color")}>
            {COLORS.map((c) => (
              <button key={c} type="button" role="radio" aria-checked={color === c} aria-label={c} className="color-choice" style={{ background: c }} onClick={() => setColor(c)} data-testid={`doctor-color-${c.slice(1)}`} />
            ))}
          </div>
          <span className="field-hint">{t("doctors.colorHint")}</span>
        </div>
        {users && (
          <Field label={t("doctors.user")} hint={t("doctors.userHint")} optional error={form.error("user_id") && t(form.error("user_id")!)}>
            <Select value={form.values.user_id} onChange={(e) => form.set("user_id", e.target.value)} data-testid="doctor-user">
              <option value="">—</option>
              {users.filter((u) => u.is_active).map((u) => <option key={u.id} value={u.id}>{u.display_name} ({u.username})</option>)}
            </Select>
          </Field>
        )}
        {doctor && (
          <div className="row" style={{ justifyContent: "space-between" }}>
            <span>{t("doctors.activeLabel")}</span>
            <Switch checked={active} onChange={setActive} label={<span className="visually-hidden">{t("doctors.activeLabel")}</span>} testId="doctor-active" />
          </div>
        )}
      </form>
    </Dialog>
  );
}

function ChairDialog({ chair, onClose, onSaved }: { chair?: ChairInfo; onClose: () => void; onSaved: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [active, setActive] = useState(chair ? chair.status === "active" : true);
  const form = useForm({ name: chair?.name ?? "" }, { name: (s) => (!s.trim() ? "rule.required" : [...s.trim()].length > 40 ? "rule.chair_name_length" : null) });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      if (chair) await rpc("chairs.update", { id: chair.id, version: chair.version, name: form.values.name.trim(), status: active ? "active" : "inactive" });
      else await rpc("chairs.create", { name: form.values.name.trim() });
      toast.success(t("common.saved"));
      onSaved();
      onClose();
    } catch (x) {
      if (!form.serverError(x)) setError(ruleText(t, x, err));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={chair ? t("chairs.edit") : t("chairs.add")}
      onClose={onClose}
      testId="chair-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} type="submit" form="chair-form" data-testid="chair-save">{t("common.save")}</Button></>}
    >
      <form id="chair-form" className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("chairs.name")} hint={t("hint.chairName")} error={form.error("name") && t(form.error("name")!)}>
          <TextInput value={form.values.name} onChange={(e) => form.set("name", e.target.value)} onBlur={() => form.blur("name")} data-testid="chair-name" />
        </Field>
        {chair && (
          <div className="row" style={{ justifyContent: "space-between" }}>
            <span>{t("chairs.activeLabel")}</span>
            <Switch checked={active} onChange={setActive} label={<span className="visually-hidden">{t("chairs.activeLabel")}</span>} testId="chair-active" />
          </div>
        )}
      </form>
    </Dialog>
  );
}

type Slots = ScheduleSlot[];

/** Weekly working hours, breaks, usable chairs and leave of one doctor. */
function ScheduleDialog({ doctor, chairs, calendar, onClose, onSaved }: { doctor: DoctorInfo; chairs: ChairInfo[]; calendar: CalendarSystem; onClose: () => void; onSaved: (d?: DoctorInfo) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [hours, setHours] = useState<Slots>(doctor.hours);
  const [breaks, setBreaks] = useState<Slots>(doctor.breaks);
  const [chairIds, setChairIds] = useState<string[]>(doctor.chair_ids);
  const [version, setVersion] = useState(doctor.version);
  const [leaves, setLeaves] = useState(doctor.leaves);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [leave, setLeave] = useState({ start: todayIso(), end: todayIso(), reason: "" });
  const [leaveError, setLeaveError] = useState("");
  const [affected, setAffected] = useState<number | null>(null);

  const bad = [...hours, ...breaks].some((s) => s.end <= s.start);

  const save = async () => {
    setError("");
    setBusy(true);
    try {
      const d = await rpc("doctors.set_schedule", { doctor_id: doctor.id, version, hours, breaks, chair_ids: chairIds });
      setVersion(d.version);
      toast.success(t("common.saved"));
      onSaved(d);
      onClose();
    } catch (x) {
      if (!isSessionError(x)) setError(ruleText(t, x, err));
    } finally {
      setBusy(false);
    }
  };

  const addLeave = async () => {
    setLeaveError("");
    try {
      const r = await rpc("doctors.add_leave", { doctor_id: doctor.id, start_date: leave.start, end_date: leave.end, reason: leave.reason.trim() || null });
      setLeaves((l) => [...l, r.leave].sort((a, b) => a.start_date.localeCompare(b.start_date)));
      setAffected(r.affected_appointments);
      setLeave({ start: todayIso(), end: todayIso(), reason: "" });
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) setLeaveError(ruleText(t, x, err));
    }
  };
  const removeLeave = async (id: string, ver: number) => {
    try {
      await rpc("doctors.delete_leave", { id, version: ver });
      setLeaves((l) => l.filter((x) => x.id !== id));
      setAffected(null);
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) toast.error(ruleText(t, x, err));
    }
  };

  const dayEditor = (list: Slots, set: (s: Slots) => void, kind: "hours" | "breaks") => (
    <div className="sched" data-testid={`sched-${kind}`}>
      {[0, 1, 2, 3, 4, 5, 6].map((day) => {
        const mine = list.map((s, i) => ({ s, i })).filter((x) => x.s.day === day);
        return (
          <div className="sched-row" key={day}>
            <span className="day">{t(`wizard.day.${day}`)}</span>
            <div className="sched-slots">
              {mine.length === 0 && <span className="subtle">{kind === "hours" ? t("wizard.hours.closed") : "—"}</span>}
              {mine.map(({ s, i }) => (
                <div className="sched-slot" key={i}>
                  <TimePicker12 value={s.start} onChange={(val) => set(list.map((x, j) => (j === i ? { ...x, start: val } : x)))} label={t("wizard.hours.from")} testId={`${kind}-${day}-${i}-from`} />
                  <span className="to">{t("wizard.hours.to")}</span>
                  <TimePicker12 value={s.end === "24:00" ? "23:55" : s.end} onChange={(val) => set(list.map((x, j) => (j === i ? { ...x, end: val } : x)))} label={t("wizard.hours.until")} testId={`${kind}-${day}-${i}-to`} />
                  {s.end <= s.start && <span className="field-error"><span>{t("rule.time_range")}</span></span>}
                  <IconButton icon={Trash2} label={t("sched.remove")} size="sm" onClick={() => set(list.filter((_, j) => j !== i))} />
                </div>
              ))}
              <Button size="sm" variant="subtle" icon={Plus} onClick={() => set([...list, { day, start: kind === "hours" ? "08:00" : "12:00", end: kind === "hours" ? "12:00" : "13:00" }])} data-testid={`${kind}-${day}-add`}>{t(kind === "hours" ? "sched.addHours" : "sched.addBreak")}</Button>
            </div>
          </div>
        );
      })}
    </div>
  );

  return (
    <Dialog
      title={`${t("doctors.schedule")} — ${doctor.full_name}`}
      description={t("sched.hint")}
      onClose={onClose}
      wide
      testId="schedule-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} disabled={bad} onClick={save} data-testid="schedule-save">{t("common.save")}</Button></>}
    >
      <div className="stack-lg">
        {error && <Notice tone="danger">{error}</Notice>}
        <section className="stack">
          <h3 className="t-subtitle">{t("sched.hours")}</h3>
          <span className="field-hint">{t("sched.hoursHint")}</span>
          {dayEditor(hours, setHours, "hours")}
        </section>
        <section className="stack">
          <h3 className="t-subtitle">{t("sched.breaks")}</h3>
          {dayEditor(breaks, setBreaks, "breaks")}
        </section>
        <section className="stack">
          <h3 className="t-subtitle">{t("sched.chairs")}</h3>
          <span className="field-hint">{t("sched.chairsHint")}</span>
          <div className="row">
            {chairs.map((c) => (
              <label className="chip-check" key={c.id}>
                <input type="checkbox" checked={chairIds.includes(c.id)} onChange={(e) => setChairIds((ids) => (e.target.checked ? [...ids, c.id] : ids.filter((x) => x !== c.id)))} data-testid={`sched-chair-${c.name}`} />
                <span>{c.name}</span>
              </label>
            ))}
            {!chairs.length && <span className="subtle">{t("chairs.empty")}</span>}
          </div>
        </section>
        <section className="stack" data-testid="sched-leaves">
          <h3 className="t-subtitle">{t("sched.leave")}</h3>
          <span className="field-hint">{t("sched.leaveHint")}</span>
          {leaves.length === 0 && <span className="subtle">{t("sched.noLeave")}</span>}
          <ul className="plain-list">
            {leaves.map((l) => (
              <li key={l.id} className="row" style={{ justifyContent: "space-between" }}>
                <span><CalendarOff size={15} aria-hidden /> {formatDate(l.start_date + "T06:00:00Z", lang, calendar)}{l.end_date !== l.start_date && ` – ${formatDate(l.end_date + "T06:00:00Z", lang, calendar)}`}{l.reason ? ` · ${l.reason}` : ""}</span>
                <IconButton icon={Trash2} label={t("sched.remove")} size="sm" onClick={() => removeLeave(l.id, l.version)} />
              </li>
            ))}
          </ul>
          {affected !== null && affected > 0 && <Notice tone="warning" testId="leave-affected">{digits(affected, lang)} {t("sched.affected")}</Notice>}
          {leaveError && <Notice tone="danger">{leaveError}</Notice>}
          <div className="grid-2">
            <Field label={t("sched.leaveFrom")}><DateField value={leave.start} onChange={(d) => setLeave((l) => ({ ...l, start: d, end: d > l.end ? d : l.end }))} calendar={calendar} label={t("sched.leaveFrom")} testId="leave-from" /></Field>
            <Field label={t("sched.leaveTo")}><DateField value={leave.end} onChange={(d) => setLeave((l) => ({ ...l, end: d }))} calendar={calendar} label={t("sched.leaveTo")} testId="leave-to" /></Field>
          </div>
          <Field label={t("sched.leaveReason")} optional><TextInput value={leave.reason} maxLength={200} onChange={(e) => setLeave((l) => ({ ...l, reason: e.target.value }))} data-testid="leave-reason" /></Field>
          <div><Button icon={Plus} onClick={addLeave} data-testid="leave-add">{t("sched.addLeave")}</Button></div>
        </section>
      </div>
    </Dialog>
  );
}
