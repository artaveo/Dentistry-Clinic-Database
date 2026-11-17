import { useEffect, useState } from "react";
import { TriangleAlert } from "lucide-react";
import type { MedicalHistoryInfo } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { digits } from "../../lib/dates";
import { alertsOf, alertText, useMedicalQuestions } from "../../lib/medical";
import { useI18n } from "../../i18n";

// A short-lived cache: the queue shows many cards and refreshes every ten seconds.
const cache = new Map<string, { at: number; value: Promise<MedicalHistoryInfo | null> }>();
const TTL = 45_000;
function history(patientId: string): Promise<MedicalHistoryInfo | null> {
  const hit = cache.get(patientId);
  if (hit && Date.now() - hit.at < TTL) return hit.value;
  const value = rpc("medical_history.get", { patient_id: patientId }).catch(() => null);
  cache.set(patientId, { at: Date.now(), value });
  return value;
}

/**
 * PA-25: the patient's medical alerts (allergy, blood thinner, pregnancy…) where the patient is called in
 * or booked, not only inside the record. A user without clinical access sees nothing (the Core refuses).
 */
export function AlertLine({ patientId, testId }: { patientId: string; testId?: string }) {
  const { t, lang } = useI18n();
  const questions = useMedicalQuestions();
  const [h, setH] = useState<MedicalHistoryInfo | null>(null);
  useEffect(() => {
    let live = true;
    history(patientId).then((v) => live && setH(v));
    return () => {
      live = false;
    };
  }, [patientId]);
  if (!h || !questions) return null;
  const alerts = alertsOf(questions, h.answers);
  if (!alerts.length) return null;
  const texts = alerts.map((a) => alertText(a, lang, t, (s) => digits(s, lang)));
  return (
    <span className="alert-line" role="note" title={texts.join(" · ")} data-testid={testId}>
      <TriangleAlert aria-hidden />
      <span>{texts.join(" · ")}</span>
    </span>
  );
}
