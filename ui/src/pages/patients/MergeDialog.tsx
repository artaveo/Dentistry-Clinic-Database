import { useState } from "react";
import { Merge, Search } from "lucide-react";
import type { PatientInfo } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";

/** Folds `patient` into another record the user searches for and picks (3.1). */
export function MergeDialog({ patient, onClose, onMerged }: { patient: PatientInfo; onClose: () => void; onMerged: () => void }) {
  const { t, err } = useI18n();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<PatientInfo[]>([]);
  const [target, setTarget] = useState<PatientInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const search = async (q: string) => {
    setQuery(q);
    setTarget(null);
    if (!q.trim()) return setResults([]);
    const r = await rpc("patients.list", { query: q, status: null, limit: 10, offset: 0 });
    setResults(r.items.filter((p) => p.id !== patient.id));
  };

  const confirm = async () => {
    if (!target) return;
    setBusy(true);
    setError("");
    try {
      await rpc("patients.merge", { keep_id: target.id, merge_id: patient.id, merge_id_version: patient.version });
      onMerged();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("patients.merge.title")}
      description={t("patients.merge.hint")}
      onClose={onClose}
      testId="merge-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={Merge} disabled={!target} loading={busy} onClick={confirm} data-testid="merge-confirm">{t("patients.merge.confirm")}</Button>
        </>
      }
    >
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="control">
          <span className="control-icon"><Search aria-hidden /></span>
          <input value={query} onChange={(e) => search(e.target.value)} placeholder={t("patients.merge.search.placeholder")} data-testid="merge-search" />
        </div>
        <ul className="plain-list">
          {results.map((p) => (
            <li key={p.id}>
              <button type="button" className="merge-candidate" onClick={() => setTarget(p)} aria-pressed={target?.id === p.id} data-testid={`merge-candidate-${p.patient_number}`}>
                <span className="cell-strong">{p.full_name}</span>
                <bdi className="ltr num subtle">{p.patient_number}</bdi>
              </button>
            </li>
          ))}
        </ul>
        {target && <Notice tone="warning">{t("patients.merge.warning")}</Notice>}
      </div>
    </Dialog>
  );
}
