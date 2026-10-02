import { useEffect, useState } from "react";
import { Save } from "lucide-react";
import type { MedicalHistoryInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Field, Textarea } from "../../ui/Field";
import { ErrorState, Loading, Notice } from "../../ui/Feedback";
import { useToast } from "../../ui/Toast";

export function MedicalHistoryTab({ patientId, canEdit, onSaved }: { patientId: string; canEdit: boolean; onSaved?: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [history, setHistory] = useState<MedicalHistoryInfo | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const load = () =>
    rpc("medical_history.get", { patient_id: patientId })
      .then((h) => {
        setHistory(h);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patientId]);

  if (error && !history) return <ErrorState message={error} onRetry={load} />;
  if (!history) return <Loading />;

  const field = (k: keyof MedicalHistoryInfo) => ({
    value: (history[k] as string | null) ?? "",
    onChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => setHistory({ ...history, [k]: e.target.value }),
  });

  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const saved = await rpc("medical_history.update", {
        patient_id: patientId,
        version: history.version,
        allergies: history.allergies,
        current_medications: history.current_medications,
        chronic_conditions: history.chronic_conditions,
        dental_history: history.dental_history,
        previous_surgeries: history.previous_surgeries,
        notes: history.notes,
      });
      setHistory(saved);
      toast.success(t("medicalHistory.saved"));
      onSaved?.();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card>
      <CardHeader title={t("medicalHistory.title")} description={t("medicalHistory.subtitle")} />
      {error && <Notice tone="danger">{error}</Notice>}
      <div className="stack" data-testid="medical-history-form">
        <Field label={t("medicalHistory.allergies")} optional>
          <Textarea {...field("allergies")} disabled={!canEdit} data-testid="mh-allergies" />
        </Field>
        <Field label={t("medicalHistory.currentMedications")} optional>
          <Textarea {...field("current_medications")} disabled={!canEdit} data-testid="mh-medications" />
        </Field>
        <Field label={t("medicalHistory.chronicConditions")} optional>
          <Textarea {...field("chronic_conditions")} disabled={!canEdit} data-testid="mh-conditions" />
        </Field>
        <Field label={t("medicalHistory.dentalHistory")} optional>
          <Textarea {...field("dental_history")} disabled={!canEdit} data-testid="mh-dental-history" />
        </Field>
        <Field label={t("medicalHistory.previousSurgeries")} optional>
          <Textarea {...field("previous_surgeries")} disabled={!canEdit} data-testid="mh-surgeries" />
        </Field>
        <Field label={t("medicalHistory.notes")} optional>
          <Textarea {...field("notes")} disabled={!canEdit} data-testid="mh-notes" />
        </Field>
        {canEdit && (
          <div className="row" style={{ justifyContent: "flex-end" }}>
            <Button variant="primary" icon={Save} loading={busy} onClick={save} data-testid="mh-save">{t("common.save")}</Button>
          </div>
        )}
      </div>
    </Card>
  );
}
