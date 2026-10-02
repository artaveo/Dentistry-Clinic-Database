import { useEffect, useState } from "react";
import { TriangleAlert } from "lucide-react";
import type { MedicalHistoryInfo } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";

/** Fixed warning banner at the top of the profile and treatment screens (3.2). */
export function MedicalAlertBanner({ patientId }: { patientId: string }) {
  const [history, setHistory] = useState<MedicalHistoryInfo | null>(null);

  useEffect(() => {
    let live = true;
    rpc("medical_history.get", { patient_id: patientId }).then((h) => live && setHistory(h)).catch(() => live && setHistory(null));
    return () => {
      live = false;
    };
  }, [patientId]);

  const parts = [history?.allergies, history?.chronic_conditions, history?.current_medications].filter((s): s is string => !!s?.trim());
  if (parts.length === 0) return null;

  return (
    <div className="notice danger medical-alert" role="alert" data-testid="medical-alert-banner">
      <TriangleAlert aria-hidden />
      <div className="notice-body">
        <span>{parts.join(" · ")}</span>
      </div>
    </div>
  );
}
