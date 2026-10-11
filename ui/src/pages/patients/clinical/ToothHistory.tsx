import { useEffect, useState } from "react";
import { History } from "lucide-react";
import type { PatientInfo, TimelineEvent, ToothNumbering } from "../../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../../lib/api";
import { useI18n } from "../../../i18n";
import { Dialog } from "../../../ui/Overlay";
import { EmptyState, ErrorState, Loading } from "../../../ui/Feedback";
import { toothLabel } from "../../../chart/numbering";
import { TimelineList } from "./TimelineTab";

/** Everything that happened to one tooth (5.3): findings added and removed, work planned, sessions, completion. */
export function ToothHistory({ patient, tooth, numbering, onClose }: { patient: PatientInfo; tooth: number; numbering: ToothNumbering; onClose: () => void }) {
  const { t, err, lang } = useI18n();
  const [events, setEvents] = useState<TimelineEvent[] | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    rpc("chart.tooth_history", { patient_id: patient.id, tooth })
      .then(setEvents)
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [patient.id, tooth]);
  return (
    <Dialog title={`${t("chart.history")} — ${t("chart.tooth")} ${toothLabel(tooth, numbering, lang)}`} description={patient.full_name} onClose={onClose} testId="tooth-history">
      {error ? <ErrorState message={error} /> : !events ? <Loading /> : events.length ? <TimelineList events={events} numbering={numbering} compact /> : <EmptyState icon={History} title={t("chart.historyEmpty")} />}
    </Dialog>
  );
}
