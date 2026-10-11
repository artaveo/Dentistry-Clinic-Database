import { useEffect, useState } from "react";
import { ArrowLeft, Route } from "lucide-react";
import type { CalendarSystem, PatientInfo, TreatmentItemInfo } from "../../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../../lib/api";
import { formatDate, formatTime } from "../../../lib/dates";
import { useCatalog, useClinicSettings } from "../../../lib/clinical";
import { useI18n } from "../../../i18n";
import { Button } from "../../../ui/Button";
import { Card, CardHeader } from "../../../ui/Card";
import { Badge } from "../../../ui/Feedback";
import { ITEM_TONE, itemTitle } from "./labels";
import { SessionDots } from "./SessionDots";

/**
 * The patient's courses of treatment on the profile overview (M4): what is under way or planned, the
 * progress of each (✓✓○) and the next booked session.
 */
export function CourseCard({ patient, calendar, onOpen }: { patient: PatientInfo; calendar: CalendarSystem; onOpen: () => void }) {
  const { t, lang } = useI18n();
  const catalog = useCatalog();
  const settings = useClinicSettings();
  const [items, setItems] = useState<TreatmentItemInfo[] | null>(null);
  useEffect(() => {
    // The plans the patient took (an alternative still on the table is not under way).
    rpc("plans.list", { patient_id: patient.id })
      .then((plans) => setItems(plans.filter((p) => p.status === "accepted" || p.status === "in_progress").flatMap((p) => p.items).filter((i) => i.status === "planned" || i.status === "in_progress")))
      .catch((e) => !isSessionError(e) && setItems([]));
  }, [patient.id]);
  if (!items?.length) return null;
  const shown = [...items].sort((a, b) => Number(b.status === "in_progress") - Number(a.status === "in_progress")).slice(0, 6);
  return (
    <Card data-testid="course-card">
      <CardHeader icon={Route} title={t("course.title")} description={t("course.hint")} actions={<Button size="sm" variant="subtle" iconEnd={ArrowLeft} flipIcon onClick={onOpen} data-testid="course-open">{t("course.open")}</Button>} />
      <ul className="plain-list course-list">
        {shown.map((i) => {
          const next = i.upcoming[0];
          return (
            <li key={i.id} className="course-row" data-testid={`course-${i.id}`}>
              <div className="course-main">
                <span className="cell-strong">{itemTitle(t, catalog, i, settings?.tooth_numbering ?? "fdi", lang)}</span>
                <span className="subtle t-caption">
                  {next ? t("course.next").replace("{date}", formatDate(`${next.date}T06:30:00Z`, lang, calendar)).replace("{time}", formatTime(next.start_time, lang)) : t("course.noNext")}
                </span>
              </div>
              {i.sessions_planned > 1 && <SessionDots item={i} />}
              <Badge tone={ITEM_TONE[i.status]} dot>{t(`item.status.${i.status}`)}</Badge>
            </li>
          );
        })}
      </ul>
    </Card>
  );
}
