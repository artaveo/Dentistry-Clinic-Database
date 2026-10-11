import { useEffect, useState } from "react";
import { Activity, CalendarCheck2, NotebookPen } from "lucide-react";
import type { CalendarSystem, ClinicProfile, PatientInfo, TreatmentItemInfo } from "../../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../../lib/api";
import { digits, formatDate } from "../../../lib/dates";
import { useCatalog, useClinicSettings } from "../../../lib/clinical";
import { formatAmount } from "../../../lib/money";
import { useI18n } from "../../../i18n";
import { Button } from "../../../ui/Button";
import { Card, CardHeader } from "../../../ui/Card";
import { Badge, EmptyState, ErrorState, SkeletonRows } from "../../../ui/Feedback";
import { teethLabel } from "../../../chart/numbering";
import { isSolo, useScheduling } from "../../reception/useScheduling";
import { ITEM_TONE, itemService, surfacesText } from "./labels";
import { SessionDots } from "./SessionDots";

/**
 * Patient profile → «درمان‌ها» (5.8): every treatment done or under way — date, tooth, surface, doctor,
 * service, price, result — each completed one ready for an invoice (Phase 6). New treatments are recorded
 * on the chart («انجام‌شده امروز») or by completing a visit booked for a session.
 */
export function TreatmentsTab({ patient, clinic, calendar, onOpenChart, onNote }: { patient: PatientInfo; clinic: ClinicProfile | null; calendar: CalendarSystem; onOpenChart: () => void; onNote: (item: TreatmentItemInfo) => void }) {
  const { t, err, lang } = useI18n();
  const catalog = useCatalog();
  const settings = useClinicSettings();
  const numbering = settings?.tooth_numbering ?? "fdi";
  const { doctors } = useScheduling();
  const solo = isSolo(clinic, doctors);
  const [items, setItems] = useState<TreatmentItemInfo[] | null>(null);
  const [error, setError] = useState("");
  const load = () =>
    rpc("treatments.list", { patient_id: patient.id })
      .then((v) => {
        setItems(v);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patient.id]);
  if (error && !items) return <ErrorState message={error} onRetry={load} />;
  const doctorName = (id: string | null | undefined) => doctors?.find((d) => d.id === id)?.full_name ?? "—";
  const total = (items ?? []).filter((i) => i.status === "completed").reduce((s, i) => s + i.unit_price * i.units, 0);
  return (
    <Card flush data-testid="treatments-tab">
      <CardHeader
        icon={Activity}
        title={t("patients.tab.treatments")}
        description={t("treatments.subtitle")}
        actions={<Button icon={CalendarCheck2} onClick={onOpenChart} data-testid="treatments-record">{t("treatments.record")}</Button>}
      />
      {items && items.length > 0 && (
        <div className="row treatments-summary">
          <span className="subtle">{t("treatments.doneTotal")}</span>
          <b className="num" data-testid="treatments-total">{formatAmount(total, lang)}</b>
          <span className="subtle t-caption">· {t("treatments.invoiceHint")}</span>
        </div>
      )}
      <div className="table-wrap">
        <table className="table" data-testid="treatments-table">
          <thead>
            <tr>
              <th>{t("treatments.date")}</th>
              <th>{t("plan.service")}</th>
              <th>{t("plan.teeth")}</th>
              {!solo && <th>{t("appt.doctor")}</th>}
              <th>{t("plan.sessionsCol")}</th>
              <th className="num">{t("plan.price")}</th>
              <th>{t("treatments.result")}</th>
              <th>{t("plan.statusCol")}</th>
            </tr>
          </thead>
          <tbody>
            {!items && <SkeletonRows cols={solo ? 7 : 8} />}
            {items?.map((i) => {
              const last = i.sessions[i.sessions.length - 1];
              const date = i.completed_at ?? (last ? `${last.performed_on}T06:30:00Z` : i.created_at);
              const result = [...i.sessions].reverse().find((s) => s.note)?.note ?? i.notes;
              return (
                <tr key={i.id} data-testid={`treatment-${i.id}`}>
                  <td>{formatDate(date, lang, calendar)}</td>
                  <td><span className="cell-strong">{itemService(catalog, i, lang)}</span>{i.details && <span className="subtle t-caption"> · {i.details}</span>}</td>
                  <td><bdi>{[teethLabel(i.teeth, numbering, lang), surfacesText(i.surfaces, i.teeth[0])].filter(Boolean).join(" ") || t("plan.wholeMouth")}</bdi></td>
                  {!solo && <td>{doctorName(i.doctor_id ?? last?.doctor_id)}</td>}
                  <td>
                    <span className="item-sessions">
                      {i.sessions_planned > 1 && <SessionDots item={i} size="sm" />}
                      <span className="t-caption">{digits(i.sessions_done, lang)}/{digits(i.sessions_planned, lang)}</span>
                    </span>
                  </td>
                  <td className="num">{i.unit_price ? formatAmount(i.unit_price * i.units, lang) : "—"}</td>
                  <td className="t-caption">{result ?? "—"}</td>
                  <td>
                    <span className="stack" style={{ gap: 2 }}>
                      <Badge tone={ITEM_TONE[i.status]} dot>{t(`item.status.${i.status}`)}</Badge>
                      {i.status === "completed" && !i.billed && <span className="t-caption subtle">{t("item.readyForInvoice")}</span>}
                      <Button size="sm" variant="link" icon={NotebookPen} onClick={() => onNote(i)} data-testid={`treatment-note-${i.id}`}>{t("treatments.writeNote")}</Button>
                    </span>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {items && !items.length && <EmptyState icon={Activity} title={t("treatments.none")} action={<Button variant="primary" icon={CalendarCheck2} onClick={onOpenChart}>{t("treatments.record")}</Button>}>{t("treatments.noneHint")}</EmptyState>}
    </Card>
  );
}
