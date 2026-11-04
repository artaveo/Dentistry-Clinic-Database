// «داشبورد» — the first thing a user sees (Phase 9, 9.1). What it shows follows who signed in: the Owner and
// reception see the clinic, a doctor sees only their own patients, visits and work. Numbers come from the
// Core; every figure that can be a warning says so in words and an icon, never by colour alone.
import { useEffect, useState } from "react";
import { AlertTriangle, CalendarCheck2, CalendarClock, CheckCircle2, ChevronRight, CircleHelp, DatabaseBackup, FlaskConical, Landmark, PackageMinus, ShieldCheck, Stethoscope, UsersRound, Wallet } from "lucide-react";
import type { CalendarSystem, Dashboard } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { digits, formatDate, formatTime } from "../lib/dates";
import { formatAmount } from "../lib/money";
import { useI18n } from "../i18n";
import { Card, CardHeader, Page, PageHeader, Stat } from "../ui/Card";
import { Button } from "../ui/Button";
import { Badge, EmptyState, ErrorState, Loading } from "../ui/Feedback";
import type { LucideIcon } from "lucide-react";

type Go = (page: string) => void;

export function DashboardPage({ calendar, go }: { calendar: CalendarSystem; go: Go }) {
  const { t, lang, err } = useI18n();
  const [data, setData] = useState<Dashboard | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  useEffect(() => {
    rpc("dashboard.get", {})
      .then((d) => {
        setData(d);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [tick]);

  if (error && !data) return <Page testId="page-dashboard"><ErrorState message={error} onRetry={() => setTick((x) => x + 1)} /></Page>;
  if (!data) return <Page testId="page-dashboard"><Loading /></Page>;

  const n = (v: number) => digits(v, lang);
  const own = data.scope === "doctor";
  const hours = data.backup?.age_hours;
  return (
    <Page testId="page-dashboard">
      <PageHeader
        title={t("dash.title")}
        description={own ? t("dash.subtitleDoctor").replace("{name}", data.doctor_name ?? "") : `${t("dash.subtitle")} · ${formatDate(data.date, lang, calendar)}`}
        actions={own ? <Badge tone="info" icon={Stethoscope} testId="dash-scope">{t("dash.scopeDoctor")}</Badge> : undefined}
      />
      <div className="stat-grid" data-testid="dash-stats">
        <Stat icon={UsersRound} label={t("dash.patientsToday")} value={n(data.patients_today)} testId="dash-patients" />
        <Stat
          icon={CalendarCheck2}
          label={t("dash.appointmentsToday")}
          value={n(data.appointments_today)}
          sub={t("dash.appointmentsDone").replace("{n}", n(data.appointments_done))}
          tone="info"
          testId="dash-appointments"
        />
        <Stat icon={Stethoscope} label={t("dash.treatmentsDone")} value={n(data.treatments_completed)} tone="success" testId="dash-treatments" />
        {data.revenue_today !== null && (
          <Stat
            icon={Wallet}
            label={data.revenue_basis === "invoiced" ? t("dash.revenueInvoiced") : t("dash.revenueToday")}
            value={formatAmount(data.revenue_today, lang)}
            tone="success"
            testId="dash-revenue"
          />
        )}
      </div>

      <div className="dash-columns">
        <Card flush>
          <CardHeader icon={CalendarClock} title={t("dash.upcoming")} description={t("dash.upcomingHint")} actions={<Button size="sm" variant="link" onClick={() => go("appointments")} data-testid="dash-open-calendar">{t("dash.openCalendar")}</Button>} />
          {data.upcoming.length === 0 ? (
            <EmptyState icon={CalendarCheck2} title={t("dash.noUpcoming")} />
          ) : (
            <ul className="dash-list" data-testid="dash-upcoming">
              {data.upcoming.map((v) => (
                <li key={v.appointment_id}>
                  <span className="dash-time num">{formatTime(v.time, lang)}</span>
                  <span className="dash-main">
                    <strong>{v.patient_name}</strong>
                    <span className="subtle">{formatDate(v.date, lang, calendar)}{!own && ` · ${v.doctor_name}`}</span>
                  </span>
                  <Badge tone={v.status === "scheduled" ? "neutral" : "info"}>{t(`appt.status.${v.status}`)}</Badge>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card flush>
          <CardHeader icon={ShieldCheck} title={t("dash.attention")} description={t("dash.attentionHint")} />
          <ul className="dash-status" data-testid="dash-attention">
            {data.outstanding !== null && (
              <Row icon={Landmark} label={t("dash.outstanding")} value={formatAmount(data.outstanding, lang)} bad={data.outstanding > 0} badText={t("dash.owed")} onClick={() => go("billing")} testId="dash-outstanding" />
            )}
            {data.low_stock !== null && (
              <Row icon={PackageMinus} label={t("dash.lowStock")} value={n(data.low_stock)} bad={data.low_stock > 0} badText={t("dash.needsOrder")} onClick={() => go("inventory")} testId="dash-lowstock" />
            )}
            {data.pending_lab !== null && (
              <Row icon={FlaskConical} label={t("dash.pendingLab")} value={n(data.pending_lab)} onClick={() => go("inventory")} testId="dash-lab" />
            )}
            {data.backup && (
              <Row
                icon={DatabaseBackup}
                label={t("dash.backup")}
                value={data.backup.last_at ? (data.backup.failed_since ? t("dash.backupFailed") : hours !== null && hours !== undefined ? t("dash.hoursAgo").replace("{n}", n(hours)) : "") : t("dash.noBackup")}
                bad={data.backup.failed_since || !data.backup.last_at || (hours ?? 0) >= 36}
                badText={t("dash.checkBackup")}
                onClick={() => go("backup")}
                testId="dash-backup"
              />
            )}
            <Row icon={CircleHelp} label={t("dash.license")} value={t("dash.licenseSoon")} testId="dash-license" />
          </ul>
        </Card>
      </div>
    </Page>
  );
}

/** One line of the "needs attention" list: a fine state is a quiet check, a bad one is a labelled warning. */
function Row({ icon: Icon, label, value, bad, badText, onClick, testId }: { icon: LucideIcon; label: string; value: string; bad?: boolean; badText?: string; onClick?: () => void; testId: string }) {
  const { t } = useI18n();
  const body = (
    <>
      <span className="dash-status-icon" aria-hidden><Icon /></span>
      <span className="dash-main">
        <strong>{label}</strong>
        {bad === undefined ? null : bad ? <Badge tone="warning" icon={AlertTriangle}>{badText}</Badge> : <Badge tone="success" icon={CheckCircle2}>{t("dash.fine")}</Badge>}
      </span>
      <span className="dash-value num">{value}</span>
      {onClick && <ChevronRight className="flip-rtl dash-go" aria-hidden />}
    </>
  );
  return (
    <li data-testid={testId}>
      {onClick ? <button type="button" className="dash-row" onClick={onClick}>{body}</button> : <div className="dash-row">{body}</div>}
    </li>
  );
}
