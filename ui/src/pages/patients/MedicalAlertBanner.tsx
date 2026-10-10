import { useEffect, useState } from "react";
import { ClipboardClock, TriangleAlert } from "lucide-react";
import type { MedicalHistoryInfo } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { digits } from "../../lib/dates";
import { alertsOf, tr, useMedicalQuestions } from "../../lib/medical";
import { useI18n } from "../../i18n";

/**
 * Fixed warning banner at the top of the profile and treatment screens (3.2): every "yes" to an alert
 * question of the checklist (M1) with its detail, e.g. «رقیق‌کننده خون: وارفارین · حامله (۵ ماه)»,
 * and a reminder when the history should be asked again (never recorded / older than six months).
 */
export function MedicalAlertBanner({ patientId, onOpenHistory, hideReview }: { patientId: string; onOpenHistory?: () => void; hideReview?: boolean }) {
  const { t, lang } = useI18n();
  const questions = useMedicalQuestions();
  const [history, setHistory] = useState<MedicalHistoryInfo | null>(null);

  useEffect(() => {
    let live = true;
    rpc("medical_history.get", { patient_id: patientId }).then((h) => live && setHistory(h)).catch(() => live && setHistory(null));
    return () => {
      live = false;
    };
  }, [patientId]);

  if (!history || !questions) return null;
  const alerts = alertsOf(questions, history.answers);
  const text = (a: (typeof alerts)[number]) => {
    const parts = [a.a.detail_text && (a.q.detail_kind === "months" ? `${digits(a.a.detail_text, lang)} ${t("medicalHistory.months")}` : a.a.detail_text), a.a.detail_choice && t(`mh.choice.${a.a.detail_choice}`)].filter(Boolean);
    return parts.length ? `${tr(a.q.label, lang)} (${parts.join("، ")})` : tr(a.q.label, lang);
  };

  return (
    <>
      {alerts.length > 0 && (
        <div className="notice danger medical-alert" role="alert" data-testid="medical-alert-banner">
          <TriangleAlert aria-hidden />
          <div className="notice-body">
            <ul className="medical-alert-list">
              {alerts.map((a) => (
                <li key={a.q.id} data-testid={`medical-alert-${a.q.code}`}>
                  <strong>{text(a)}</strong>
                  {a.q.alert_note && <span className="medical-alert-note"> — {tr(a.q.alert_note, lang)}</span>}
                </li>
              ))}
            </ul>
          </div>
        </div>
      )}
      {history.review_due && !hideReview && (
        <div className="notice warning medical-review" data-testid="medical-review-reminder">
          <ClipboardClock aria-hidden />
          <div className="notice-body row" style={{ justifyContent: "space-between", flexWrap: "wrap" }}>
            <span>{history.version === 0 ? t("medicalHistory.neverHint") : t("medicalHistory.reviewDueHint")}</span>
            {onOpenHistory && <button type="button" className="link-btn" onClick={onOpenHistory} data-testid="medical-review-open">{t("medicalHistory.open")}</button>}
          </div>
        </div>
      )}
    </>
  );
}
