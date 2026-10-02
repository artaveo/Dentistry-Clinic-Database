import { useState } from "react";
import { FileUp } from "lucide-react";
import type { ImportPatientsResult } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Badge, Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";

/** CSV patient import (3.7): pick a file, preview the per-row result, then commit. */
export function ImportDialog({ onClose, onImported }: { onClose: () => void; onImported: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [base64, setBase64] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImportPatientsResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const pick = async (file: File | undefined) => {
    if (!file) return;
    setError("");
    setPreview(null);
    const reader = new FileReader();
    reader.onload = async () => {
      const b64 = (reader.result as string).split(",")[1];
      setBase64(b64);
      try {
        setPreview(await rpc("patients.import", { csv_base64: b64, commit: false }));
      } catch (x) {
        setError(err(x));
      }
    };
    reader.readAsDataURL(file);
  };

  const commit = async () => {
    if (!base64) return;
    setBusy(true);
    setError("");
    try {
      const r = await rpc("patients.import", { csv_base64: base64, commit: true });
      toast.success(t("patients.import.done").replace("{imported}", String(r.imported)));
      onImported();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("patients.import.title")}
      description={t("patients.import.hint")}
      onClose={onClose}
      testId="import-dialog"
      wide
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={FileUp} disabled={!preview || preview.total === 0} loading={busy} onClick={commit} data-testid="import-commit">{t("patients.import.commit")}</Button>
        </>
      }
    >
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <input type="file" accept=".csv,text/csv" onChange={(e) => pick(e.target.files?.[0])} data-testid="import-file-input" />
        {preview && (
          <>
            <div className="row">
              <Badge>{t("patients.import.total")}: {preview.total}</Badge>
              <Badge tone="success">{t("patients.import.imported")}: {preview.total - preview.errors.length}</Badge>
              <Badge tone="danger">{t("patients.import.skipped")}: {preview.skipped}</Badge>
            </div>
            {preview.errors.length > 0 && (
              <div className="table-wrap">
                <table className="table" data-testid="import-errors">
                  <thead><tr><th>#</th><th>{t("patients.import.rowErrors")}</th></tr></thead>
                  <tbody>
                    {preview.errors.map((e, i) => (
                      <tr key={i}><td className="cell-num">{e.row_number}</td><td>{e.message}</td></tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        )}
      </div>
    </Dialog>
  );
}
