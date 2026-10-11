import { useState } from "react";
import { X } from "lucide-react";
import type { AppointmentInfo, CalendarSystem, Language } from "../../../../shared/ts/contract";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Loading } from "../../ui/Feedback";
import { DocumentPreview } from "../../print/DocumentPreview";
import { AppointmentCardSheet } from "../../print/DocumentSheet";
import { useLetterhead } from "../../print/letterhead";
import { LanguagePick } from "../patients/documents/shared";

/**
 * The appointment card the patient takes home (4.6), on the shared print engine (ADR-13): A6 at real
 * size, on this computer's printer (or compactly on A4) or as a PDF, in the language chosen for it.
 * It is not a clinical document, so it has no number and is not kept in the record.
 */
export function AppointmentCardDialog({ appointment: a, calendar, onClose }: { appointment: AppointmentInfo; clinicName?: string; calendar: CalendarSystem; onClose: () => void }) {
  const { t, lang: uiLang } = useI18n();
  const letterhead = useLetterhead();
  const [lang, setLang] = useState<Language>(uiLang);
  if (!letterhead) return <Loading />;
  return (
    <DocumentPreview
      title={t("appt.card.title")}
      paper="a6"
      fileStem={`CARD-${a.date}-${a.patient_number}`.replace(/[^A-Za-z0-9_-]/g, "")}
      testId="appointment-card-dialog"
      sheet={<AppointmentCardSheet appt={a} letterhead={letterhead} calendar={calendar} lang={lang} />}
      onClose={onClose}
      extra={
        <>
          <LanguagePick value={lang} onChange={setLang} testId="card-language" />
          <Button variant="subtle" icon={X} onClick={onClose} data-testid="appt-card-close">{t("common.close")}</Button>
        </>
      }
    />
  );
}
