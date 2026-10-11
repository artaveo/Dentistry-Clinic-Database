import { useEffect, useMemo, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { CalendarClock, ClipboardList, FileHeart, FileText, Images, NotebookPen, Pill, ScanLine, ScrollText, Stethoscope, UserPlus, BellRing, Activity } from "lucide-react";
import type { CalendarSystem, PatientInfo, TimelineEvent, TimelineKind, ToothNumbering } from "../../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../../lib/api";
import { digits, formatDate, formatTimeOf } from "../../../lib/dates";
import { useCatalog, useClinicSettings, useToothConditions } from "../../../lib/clinical";
import { tr } from "../../../lib/medical";
import { useI18n } from "../../../i18n";
import { Card, CardHeader } from "../../../ui/Card";
import { EmptyState, ErrorState, Loading } from "../../../ui/Feedback";
import { teethLabel } from "../../../chart/numbering";
import { itemService, surfacesText } from "./labels";

const KINDS: { kind: TimelineKind; icon: LucideIcon }[] = [
  { kind: "registered", icon: UserPlus },
  { kind: "appointment", icon: CalendarClock },
  { kind: "examination", icon: NotebookPen },
  { kind: "diagnosis", icon: Stethoscope },
  { kind: "plan", icon: ClipboardList },
  { kind: "treatment", icon: Activity },
  { kind: "prescription", icon: Pill },
  { kind: "document", icon: FileText },
  { kind: "follow_up", icon: BellRing },
  { kind: "attachment", icon: Images },
  { kind: "medical_history", icon: FileHeart },
  { kind: "perio", icon: ScanLine },
];
const ICON = new Map(KINDS.map((k) => [k.kind, k.icon]));

/** Patient profile → «Timeline» (5.11): the patient's whole story, newest first, by day, filterable by kind. */
export function TimelineTab({ patient, calendar }: { patient: PatientInfo; calendar: CalendarSystem }) {
  const { t, err } = useI18n();
  const settings = useClinicSettings();
  const [events, setEvents] = useState<TimelineEvent[] | null>(null);
  const [error, setError] = useState("");
  const [kinds, setKinds] = useState<TimelineKind[]>([]);
  const load = () =>
    rpc("timeline.list", { patient_id: patient.id, kinds: [], limit: 500 })
      .then((e) => {
        setEvents(e);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patient.id]);
  const shown = useMemo(() => (events ?? []).filter((e) => !kinds.length || kinds.includes(e.kind)), [events, kinds]);
  const present = new Set((events ?? []).map((e) => e.kind));
  return (
    <Card flush data-testid="timeline-tab">
      <CardHeader icon={ScrollText} title={t("patients.tab.timeline")} description={t("timeline.subtitle")} />
      <div className="chip-group timeline-filters" role="group" aria-label={t("timeline.filter")}>
        {KINDS.filter((k) => present.has(k.kind)).map(({ kind, icon: Icon }) => {
          const on = kinds.includes(kind);
          return (
            <button key={kind} type="button" className="chip-toggle" aria-pressed={on} onClick={() => setKinds((x) => (on ? x.filter((y) => y !== kind) : [...x, kind]))} data-testid={`timeline-filter-${kind}`}>
              <Icon aria-hidden />
              <span>{t(`timeline.kind.${kind}`)}</span>
            </button>
          );
        })}
      </div>
      <div className="timeline-body">
        {error ? <ErrorState message={error} onRetry={load} /> : !events ? <Loading /> : shown.length ? <TimelineList events={shown} numbering={settings?.tooth_numbering ?? "fdi"} calendar={calendar} /> : <EmptyState icon={ScrollText} title={t("timeline.empty")} />}
      </div>
    </Card>
  );
}

/** The events grouped by day, each with its icon, words and who did it (also a tooth's history on the chart). */
export function TimelineList({ events, numbering, calendar = "shamsi", compact }: { events: TimelineEvent[]; numbering: ToothNumbering; calendar?: CalendarSystem; compact?: boolean }) {
  const { t, lang } = useI18n();
  const catalog = useCatalog();
  const conditions = useToothConditions();
  const days: { day: string; list: TimelineEvent[] }[] = [];
  for (const e of events) {
    const day = formatDate(e.at, lang, calendar);
    const last = days[days.length - 1];
    if (last?.day === day) last.list.push(e);
    else days.push({ day, list: [e] });
  }
  const words = (e: TimelineEvent): { title: string; sub: string[] } => {
    const service = e.service_id ? itemService(catalog, { service_id: e.service_id }, lang) : "";
    const condition = e.condition_id ? tr(conditions?.find((c) => c.id === e.condition_id)?.name, lang) : "";
    const teeth = e.teeth.length ? teethLabel(e.teeth, numbering, lang) : "";
    const surfaces = e.surfaces ? surfacesText(e.surfaces, e.teeth[0]) : "";
    const session = e.session ? t("timeline.session").replace("{n}", digits(e.session.split("/")[0], lang)).replace("{of}", digits(e.session.split("/")[1], lang)) : "";
    const fill = (s: string) => s.replace("{title}", e.title ?? "").replace("{service}", service).replace("{condition}", condition).replace("{teeth}", teeth);
    const sub: string[] = [];
    const key = `timeline.action.${e.action}`;
    let title = t(key) === key ? e.action : fill(t(key));
    if (e.kind === "appointment") {
      title = fill(t(e.action.startsWith("appointment.") ? `timeline.action.appointment` : key)).replace("{status}", t(`appt.status.${e.action.split(".")[1]}`));
      if (service) sub.push([service, teeth, session].filter(Boolean).join(" · "));
    }
    if (e.kind === "treatment" || e.kind === "diagnosis" || e.action.startsWith("item.")) sub.push([teeth, surfaces, session].filter(Boolean).join(" · "));
    if (e.kind === "follow_up") {
      title = fill(t("timeline.action.recall")).replace("{kind}", t(`recall.kind.${e.title}`)).replace("{status}", t(`recall.status.${e.action.split(".")[1]}`));
      if (e.detail) sub.push(t("timeline.due").replace("{date}", formatDate(`${e.detail}T06:30:00Z`, lang, calendar)));
    } else if (e.kind === "document" || e.kind === "prescription") {
      if (e.detail) sub.push(t(`docs.kind.${e.detail}`));
    } else if (e.kind === "attachment") {
      if (e.detail) sub.push(t(`attachments.kind.${e.detail}`));
    } else if (e.kind === "perio") {
      if (e.detail) sub.push(t("timeline.perioMean").replace("{mm}", digits(e.detail, lang)));
    } else if (e.detail) {
      sub.push(e.detail);
    }
    return { title, sub: sub.filter(Boolean) };
  };
  return (
    <ol className={`timeline ${compact ? "compact" : ""}`} data-testid="timeline">
      {days.map(({ day, list }) => (
        <li key={day} className="timeline-day">
          <div className="timeline-date">{day}</div>
          <ol className="timeline-events">
            {list.map((e, i) => {
              const Icon = ICON.get(e.kind) ?? ScrollText;
              const w = words(e);
              return (
                <li key={`${e.at}-${e.action}-${e.ref_id ?? i}`} className={`timeline-event k-${e.kind} a-${e.action.replace(/\./g, "-")}`} data-testid={`timeline-event-${e.action}`}>
                  <span className="timeline-icon" aria-hidden><Icon /></span>
                  <div className="timeline-text">
                    <span className="timeline-title">{w.title}</span>
                    {w.sub.map((s, k) => <span key={k} className="timeline-sub">{s}</span>)}
                    <span className="timeline-meta">{formatTimeOf(e.at, lang)}{e.by_name ? ` · ${e.by_name}` : ""}</span>
                  </div>
                </li>
              );
            })}
          </ol>
        </li>
      ))}
    </ol>
  );
}
