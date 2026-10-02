import { TriangleAlert, UserPlus } from "lucide-react";
import type { Language, PatientInfo } from "../../../../shared/ts/contract";
import { useI18n } from "../../i18n";
import { formatDate } from "../../lib/dates";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Overlay";

/** Shown when `patients.create` reports `possible_duplicate` (3.1): the user decides. */
export function DuplicateWarningDialog({ patients, lang, onCancel, onContinue }: { patients: PatientInfo[]; lang: Language; onCancel: () => void; onContinue: () => void }) {
  const { t } = useI18n();
  return (
    <Dialog
      title={t("patients.duplicate.title")}
      onClose={onCancel}
      testId="duplicate-dialog"
      footer={
        <>
          <Button onClick={onCancel}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={UserPlus} onClick={onContinue} data-testid="duplicate-continue">{t("patients.duplicate.continueAnyway")}</Button>
        </>
      }
    >
      <div className="stack">
        <div className="row"><TriangleAlert className="subtle" aria-hidden /><span>{t("patients.duplicate.body")}</span></div>
        <ul className="plain-list">
          {patients.map((p) => (
            <li key={p.id} className="row" style={{ justifyContent: "space-between" }}>
              <span className="cell-strong">{p.full_name}</span>
              <span className="subtle t-caption"><bdi className="ltr num">{p.patient_number}</bdi> · {formatDate(p.registration_date, lang)}</span>
            </li>
          ))}
        </ul>
      </div>
    </Dialog>
  );
}
