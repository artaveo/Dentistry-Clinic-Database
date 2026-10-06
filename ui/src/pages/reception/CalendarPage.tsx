import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Plus } from "lucide-react";
import type { AppointmentInfo, AppointmentStatus, CalendarSystem, ClinicProfile, DayCount } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { addDays, fromMinutes, inCalendar, isInMonth, monthRange, monthWeeks, nowMinutes, shiftMonth, todayIso, toMinutes, weekDays, weekdayIndex } from "../../lib/calendar";
import { digits, formatDate, formatTime, monthName } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Checkbox, Segmented } from "../../ui/Controls";
import { Card, Page, PageHeader } from "../../ui/Card";
import { Select } from "../../ui/Field";
import { ErrorState, Loading } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { AppointmentDialog, type Defaults } from "./AppointmentDialog";
import { layoutLanes } from "./layout";
import { MOVABLE, ruleText } from "./labels";
import { isSolo, useScheduling, type Perms } from "./useScheduling";

type View = "day" | "week" | "month";
type Group = "doctor" | "chair";
const PX_PER_MIN = 1.15;
const SNAP = 5;
const HIDDEN: AppointmentStatus[] = ["cancelled", "rescheduled"];
const VIEW_KEY = "artaveo.calendar.view";

/** One column of the time grid. */
type Column = { key: string; date: string; label: string; sub?: string; color?: string; doctorId?: string; chairId?: string | null };

function storedView(): View {
  try {
    const v = localStorage.getItem(VIEW_KEY);
    return v === "day" || v === "week" || v === "month" ? v : "day";
  } catch {
    return "day";
  }
}

/** OF-032: below this window width the week view shows three days (with the same arrows) instead of seven. */
const NARROW_QUERY = "(max-width: 860px)";

function useNarrow(): boolean {
  const [narrow, setNarrow] = useState(() => typeof window !== "undefined" && window.matchMedia(NARROW_QUERY).matches);
  useEffect(() => {
    const mq = window.matchMedia(NARROW_QUERY);
    const onChange = () => setNarrow(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return narrow;
}

/** Day / week / month calendar (4.3) by doctor or chair, with drag & drop moving. */
export function CalendarPage({ clinic, perms, clinicName }: { clinic: ClinicProfile | null; perms: Perms; clinicName: string }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const calendar: CalendarSystem = clinic?.calendar_system ?? "shamsi";
  const { doctors, chairs, error: setupError, reload } = useScheduling();
  const [view, setViewState] = useState<View>(storedView());
  const [anchor, setAnchor] = useState(todayIso());
  const narrow = useNarrow();
  const [picking, setPicking] = useState(false);
  const weekCount = view === "week" && narrow ? 3 : 7;
  const [doctorId, setDoctorId] = useState("");
  const [group, setGroup] = useState<Group>("doctor");
  const [showClosed, setShowClosed] = useState(false);
  const [items, setItems] = useState<AppointmentInfo[] | null>(null);
  const [counts, setCounts] = useState<DayCount[]>([]);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<{ appt?: AppointmentInfo; defaults?: Defaults } | null>(null);
  const [tick, setTick] = useState(0);
  const drag = useRef<{ id: string; offsetMin: number; duration: number } | null>(null);
  const [dropHint, setDropHint] = useState<string | null>(null);

  const setView = (v: View) => {
    setViewState(v);
    try {
      localStorage.setItem(VIEW_KEY, v);
    } catch {
      /* a per-device convenience only */
    }
  };

  const range = useMemo(() => {
    if (view === "day") return { from: anchor, to: anchor };
    if (view === "week") {
      const d = weekDays(anchor);
      return { from: d[0], to: weekCount === 3 ? addDays(anchor, 2) : d[6] };
    }
    const m = monthRange(anchor, calendar);
    const w = monthWeeks(anchor, calendar);
    return { from: w[0][0] || m.from, to: w[w.length - 1][6] || m.to };
  }, [view, anchor, calendar, weekCount]);

  const load = useCallback(() => {
    const statuses = (showClosed ? [] : (["scheduled", "confirmed", "checked_in", "in_treatment", "completed", "no_show"] as AppointmentStatus[]));
    const base = { date_from: range.from, date_to: range.to, doctor_id: doctorId || null };
    const job = view === "month"
      ? rpc("appointments.counts", base).then((c) => { setCounts(c); setItems([]); })
      : rpc("appointments.list", { ...base, chair_id: null, patient_id: null, statuses, limit: 2000, offset: 0 }).then((a) => setItems(a));
    job.then(() => setError("")).catch((e) => !isSessionError(e) && setError(err(e)));
  }, [range.from, range.to, doctorId, view, showClosed]);

  useEffect(() => { load(); }, [load, tick]);
  // Other computers and other users change the calendar too: refresh quietly.
  useEffect(() => {
    const id = window.setInterval(() => !document.hidden && setTick((x) => x + 1), 30_000);
    return () => window.clearInterval(id);
  }, []);

  const shownDoctors = doctorId ? (doctors ?? []).filter((d) => d.id === doctorId) : (doctors ?? []);
  const solo = isSolo(clinic, doctors);

  // Visible hours: the clinic's working hours, widened to include every appointment.
  const [startMin, endMin] = useMemo(() => {
    const open = (clinic?.working_hours ?? []).filter((h) => !h.closed && h.open && h.close);
    let lo = open.length ? Math.min(...open.map((h) => toMinutes(h.open!))) : 8 * 60;
    let hi = open.length ? Math.max(...open.map((h) => toMinutes(h.close!))) : 18 * 60;
    for (const a of items ?? []) {
      lo = Math.min(lo, toMinutes(a.start_time));
      hi = Math.max(hi, toMinutes(a.end_time));
    }
    lo = Math.max(0, Math.floor(lo / 60) * 60 - 60);
    hi = Math.min(1440, Math.ceil(hi / 60) * 60 + 60);
    return [lo, hi] as const;
  }, [clinic, items]);

  const columns: Column[] = useMemo(() => {
    if (view === "week") {
      return weekDays(anchor).slice(0, weekCount).map((d) => ({ key: d, date: d, label: t(`wizard.day.${weekdayIndex(d)}`), sub: dayNumber(d, calendar, lang) }));
    }
    if (group === "chair") {
      return [...chairs.map((c) => ({ key: c.id, date: anchor, label: c.name, chairId: c.id as string | null })), { key: "none", date: anchor, label: t("cal.noChair"), chairId: null }];
    }
    // Solo clinic (2.6): one plain column for the day, no doctor header.
    if (solo) return [{ key: "day", date: anchor, label: t(`wizard.day.${weekdayIndex(anchor)}`), sub: dayNumber(anchor, calendar, lang) }];
    return shownDoctors.map((d) => ({ key: d.id, date: anchor, label: d.full_name, sub: d.specialty ?? undefined, color: d.color, doctorId: d.id }));
  }, [view, group, anchor, chairs, shownDoctors, calendar, lang, t, solo, weekCount]);

  const inColumn = (a: AppointmentInfo, c: Column) => {
    if (view === "week") return a.date === c.date;
    if (group === "chair") return a.date === anchor && (a.chair_id ?? null) === (c.chairId ?? null);
    return a.date === anchor && (!c.doctorId || a.doctor_id === c.doctorId);
  };

  const move = async (a: AppointmentInfo, col: Column, startAt: number) => {
    const duration = toMinutes(a.end_time) - toMinutes(a.start_time);
    const start = Math.max(0, Math.min(1440 - duration, startAt));
    try {
      await rpc("appointments.update", {
        id: a.id,
        version: a.version,
        doctor_id: view === "day" && group === "doctor" ? (col.doctorId ?? a.doctor_id) : a.doctor_id,
        chair_id: view === "day" && group === "chair" ? col.chairId ?? null : a.chair_id,
        date: col.date,
        start_time: fromMinutes(start),
        end_time: fromMinutes(start + duration),
        reason: a.reason,
        notes: a.notes,
        override_schedule: false,
      });
      toast.success(t("cal.moved"));
    } catch (x) {
      if (!isSessionError(x)) toast.error(ruleText(t, x, err));
    }
    load();
  };

  const minuteAt = (e: React.PointerEvent | React.DragEvent, el: HTMLElement) => {
    const y = e.clientY - el.getBoundingClientRect().top;
    return startMin + Math.round(y / PX_PER_MIN / SNAP) * SNAP;
  };

  const title = (() => {
    if (view === "day") return formatDate(anchor + "T06:00:00Z", lang, calendar);
    if (view === "week") {
      const d = weekDays(anchor);
      return `${formatDate(d[0] + "T06:00:00Z", lang, calendar)} – ${formatDate(d[6] + "T06:00:00Z", lang, calendar)}`;
    }
    const m = inCalendar(anchor, calendar);
    return `${monthName(calendar, m.month, lang)} ${digits(m.year, lang)}`;
  })();
  const step = (n: number) => setAnchor((a) => (view === "month" ? shiftMonth(a, calendar, n) : addDays(a, view === "week" ? weekCount * n : n)));
  const today = todayIso();
  const hours = Array.from({ length: Math.floor((endMin - startMin) / 60) }, (_, i) => startMin + i * 60);

  if (setupError) return <Page><ErrorState message={err(setupError)} onRetry={reload} /></Page>;

  return (
    <Page testId="page-calendar">
      <PageHeader
        title={t("nav.appointments")}
        description={t("cal.subtitle")}
        actions={perms.edit ? <Button variant="primary" icon={Plus} disabled={!doctors?.length} onClick={() => setDialog({ defaults: { date: anchor } })} data-testid="appt-new">{t("appt.new")}</Button> : undefined}
      />
      <div className="cal-toolbar">
        <div className="row">
          <IconButton icon={ChevronRight} label={t("cal.prev")} onClick={() => step(-1)} data-testid="cal-prev" className="flip-rtl-btn" />
          <Button onClick={() => setAnchor(today)} data-testid="cal-today">{t("cal.today")}</Button>
          <IconButton icon={ChevronLeft} label={t("cal.next")} onClick={() => step(1)} data-testid="cal-next" className="flip-rtl-btn" />
          {/* OF-038: the date opens a calendar to jump straight to any day or month. */}
          <button type="button" className="cal-title" onClick={() => setPicking(true)} aria-haspopup="dialog" data-testid="cal-title">
            <span className="t-title">{title}</span>
          </button>
        </div>
        <div className="row">
          {!solo && (
            <Select value={doctorId} onChange={(e) => setDoctorId(e.target.value)} aria-label={t("appt.doctor")} data-testid="cal-doctor">
              <option value="">{t("cal.allDoctors")}</option>
              {(doctors ?? []).map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
            </Select>
          )}
          {view === "day" && (!solo || chairs.length > 0) && (
            <Segmented<Group> value={group} onChange={setGroup} label={t("cal.groupBy")} options={[{ value: "doctor", label: t("cal.byDoctor"), testId: "cal-by-doctor" }, { value: "chair", label: t("cal.byChair"), testId: "cal-by-chair" }]} />
          )}
          <Segmented<View> value={view} onChange={setView} label={t("cal.view")} options={[{ value: "day", label: t("cal.day"), testId: "cal-view-day" }, { value: "week", label: t("cal.week"), testId: "cal-view-week" }, { value: "month", label: t("cal.month"), testId: "cal-view-month" }]} />
          <Checkbox checked={showClosed} onChange={setShowClosed} testId="cal-show-closed">{t("cal.showClosed")}</Checkbox>
        </div>
      </div>

      <Card flush>
        {error ? <ErrorState message={error} onRetry={load} /> : !doctors || !items ? <Loading /> : view === "month" ? (
          <MonthGrid anchor={anchor} calendar={calendar} counts={counts} today={today} onPick={(d) => { setAnchor(d); setView("day"); }} />
        ) : !doctors.length ? (
          <div className="empty"><span className="empty-title">{t("cal.noDoctors")}</span></div>
        ) : (
          <div className="timegrid" data-testid={`cal-grid-${view}`} data-view={view} style={{ ["--cols" as string]: columns.length }}>
            <div className="tg-head">
              <div className="tg-corner" />
              {columns.map((c) => (
                <div key={c.key} className={`tg-colhead ${c.date === today && view === "week" ? "today" : ""}`}>
                  {c.color && <span className="dot-swatch" style={{ background: c.color }} aria-hidden />}
                  <span className="cell-strong">{c.label}</span>
                  {c.sub && <span className="subtle t-caption">{c.sub}</span>}
                </div>
              ))}
            </div>
            <div className="tg-body" style={{ height: (endMin - startMin) * PX_PER_MIN }}>
              <div className="tg-hours">
                {hours.map((h) => <div key={h} className="tg-hour" style={{ top: (h - startMin) * PX_PER_MIN }}><span>{formatTime(fromMinutes(h), lang)}</span></div>)}
              </div>
              {columns.map((c) => {
                const mine = items.filter((a) => inColumn(a, c) && (showClosed || !HIDDEN.includes(a.status)));
                const placed = layoutLanes(mine.map((a) => ({ id: a.id, start: toMinutes(a.start_time), end: toMinutes(a.end_time) })));
                const nowLine = c.date === today ? nowMinutes() : null;
                return (
                  <div
                    key={c.key}
                    className={`tg-col ${dropHint === c.key ? "drop" : ""}`}
                    data-testid={`cal-col-${c.key}`}
                    onClick={(e) => {
                      if (!perms.edit || e.target !== e.currentTarget) return;
                      const m = Math.max(0, Math.round(minuteAt(e as unknown as React.PointerEvent, e.currentTarget) / 15) * 15);
                      setDialog({ defaults: { date: c.date, start: fromMinutes(Math.min(m, 1425)), doctorId: c.doctorId ?? (doctorId || doctors?.[0]?.id), chairId: c.chairId ?? undefined } });
                    }}
                    onDragOver={(e) => { if (drag.current) { e.preventDefault(); setDropHint(c.key); } }}
                    onDragLeave={() => setDropHint((k) => (k === c.key ? null : k))}
                    onDrop={(e) => {
                      e.preventDefault();
                      setDropHint(null);
                      const d = drag.current;
                      drag.current = null;
                      const a = items.find((x) => x.id === d?.id);
                      if (d && a) move(a, c, minuteAt(e, e.currentTarget) - d.offsetMin);
                    }}
                  >
                    {hours.map((h) => <div key={h} className="tg-line" style={{ top: (h - startMin) * PX_PER_MIN }} />)}
                    {nowLine !== null && nowLine >= startMin && nowLine <= endMin && <div className="tg-now" style={{ top: (nowLine - startMin) * PX_PER_MIN }} aria-hidden />}
                    {mine.map((a) => {
                      const p = placed.find((x) => x.id === a.id)!;
                      const movable = perms.edit && MOVABLE.includes(a.status);
                      const dur = toMinutes(a.end_time) - toMinutes(a.start_time);
                      return (
                        <button
                          key={a.id}
                          type="button"
                          className={`tg-appt st-${a.status} ${dur < 45 ? "short" : ""}`}
                          style={{ top: (toMinutes(a.start_time) - startMin) * PX_PER_MIN, height: Math.max(22, dur * PX_PER_MIN - 2), insetInlineStart: `${(p.lane / p.lanes) * 100}%`, width: `calc(${100 / p.lanes}% - 3px)`, ["--doc" as string]: a.doctor_color }}
                          draggable={movable}
                          onDragStart={(e) => {
                            const r = e.currentTarget.getBoundingClientRect();
                            drag.current = { id: a.id, offsetMin: Math.round((e.clientY - r.top) / PX_PER_MIN / SNAP) * SNAP, duration: dur };
                            e.dataTransfer.effectAllowed = "move";
                            e.dataTransfer.setData("text/plain", a.id);
                          }}
                          onDragEnd={() => { drag.current = null; setDropHint(null); }}
                          onClick={(e) => { e.stopPropagation(); setDialog({ appt: a }); }}
                          title={`${a.patient_name} · ${formatTime(a.start_time, lang)}–${formatTime(a.end_time, lang)}`}
                          data-testid={`cal-appt-${a.id}`}
                        >
                          <span className="tg-appt-time">{formatTime(a.start_time, lang)}</span>
                          <span className="tg-appt-name">{a.patient_name}</span>
                          {dur >= 40 && a.reason && <span className="tg-appt-reason">{a.reason}</span>}
                        </button>
                      );
                    })}
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </Card>

      {picking && (
        <Dialog title={title} onClose={() => setPicking(false)} testId="cal-date-picker">
          <div className="row" style={{ justifyContent: "space-between" }}>
            <IconButton icon={ChevronRight} label={t("cal.prev")} onClick={() => setAnchor((a) => shiftMonth(a, calendar, -1))} className="flip-rtl-btn" />
            <Button variant="subtle" onClick={() => { setAnchor(today); setPicking(false); }}>{t("cal.today")}</Button>
            <IconButton icon={ChevronLeft} label={t("cal.next")} onClick={() => setAnchor((a) => shiftMonth(a, calendar, 1))} className="flip-rtl-btn" />
          </div>
          <MonthGrid anchor={anchor} calendar={calendar} counts={[]} today={today} onPick={(d) => { setAnchor(d); setView("day"); setPicking(false); }} />
        </Dialog>
      )}

      {dialog && doctors && (
        <AppointmentDialog
          key={dialog.appt?.id ?? "new"}
          appointment={dialog.appt}
          defaults={dialog.defaults}
          doctors={doctors}
          chairs={chairs}
          calendar={calendar}
          perms={perms}
          clinicName={clinicName}
          solo={solo}
          onOpenAppointment={(other) => setDialog({ appt: other })}
          onClose={() => setDialog(null)}
          onSaved={() => { setTick((x) => x + 1); }}
        />
      )}
    </Page>
  );
}

function dayNumber(iso: string, calendar: CalendarSystem, lang: "fa" | "ps" | "en") {
  const d = inCalendar(iso, calendar);
  return `${digits(d.day, lang)} ${monthName(calendar, d.month, lang)}`;
}

function MonthGrid({ anchor, calendar, counts, today, onPick }: { anchor: string; calendar: CalendarSystem; counts: DayCount[]; today: string; onPick: (d: string) => void }) {
  const { t, lang } = useI18n();
  const byDate = new Map(counts.map((c) => [c.date, c]));
  return (
    <div className="monthgrid" data-testid="cal-grid-month">
      {[0, 1, 2, 3, 4, 5, 6].map((d) => <div key={d} className="mg-head">{t(`wizard.day.${d}`)}</div>)}
      {monthWeeks(anchor, calendar).flat().map((d) => {
        const c = byDate.get(d);
        const other = !isInMonth(d, anchor, calendar);
        return (
          <button key={d} type="button" className={`mg-cell ${other ? "other" : ""} ${d === today ? "today" : ""}`} onClick={() => onPick(d)} data-testid={`cal-day-${d}`}>
            <span className="mg-num">{digits(inCalendar(d, calendar).day, lang)}</span>
            {c && c.total > 0 && <span className="mg-count" data-testid={`cal-count-${d}`}>{digits(c.total, lang)} {t("cal.visits")}{c.open > 0 && c.open !== c.total ? ` · ${digits(c.open, lang)} ${t("cal.open")}` : ""}</span>}
          </button>
        );
      })}
    </div>
  );
}
