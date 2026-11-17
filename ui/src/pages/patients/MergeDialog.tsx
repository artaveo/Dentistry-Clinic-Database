import { useState } from "react";
import { Merge, Search } from "lucide-react";
import type { PatientInfo } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { digits } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";

/** Folds `patient` into another record the user searches for and picks (3.1). */
export function MergeDialog({ patient, onClose, onMerged }: { patient: PatientInfo; onClose: () => void; onMerged: () => void }) {
  const { t, err, lang } = useI18n();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<PatientInfo[]>([]);
  const [target, setTarget] = useState<PatientInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // PA-39: a merge cannot be undone, so it takes a second, deliberate tick.
  const [sure, setSure] = useState(false);

  const search = async (q: string) => {
    setQuery(q);
    setTarget(null);
    setSure(false);
    if (!q.trim()) return setResults([]);
    const r = await rpc("patients.list", { query: q, status: "active", limit: 10, offset: 0 });
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
      dismissOnScrim={false}
      testId="merge-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={Merge} disabled={!target || !sure} loading={busy} onClick={confirm} data-testid="merge-confirm">{t("patients.merge.confirm")}</Button>
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
              <button type="button" className="merge-candidate" onClick={() => { setTarget(p); setSure(false); }} aria-pressed={target?.id === p.id} data-testid={`merge-candidate-${p.patient_number}`}>
                <span className="cell-strong">{p.full_name}</span>
                <bdi className="ltr num subtle">{p.patient_number}</bdi>
              </button>
            </li>
          ))}
        </ul>
        {target && (
          <div className="merge-compare" data-testid="merge-compare">
            <div className="merge-side merge-side-remove">
              <span className="t-caption subtle">{t("patients.merge.remove")}</span>
              <strong>{patient.full_name}</strong>
              <span className="subtle t-caption"><bdi className="ltr num">{patient.patient_number}</bdi>{patient.phone && <> · <bdi className="ltr">{patient.phone}</bdi></>}</span>
            </div>
            <div className="merge-side merge-side-keep">
              <span className="t-caption subtle">{t("patients.merge.keep")}</span>
              <strong>{target.full_name}</strong>
              <span className="subtle t-caption"><bdi className="ltr num">{target.patient_number}</bdi>{target.phone && <> · <bdi className="ltr">{target.phone}</bdi></>}{target.father_name && <> · {target.father_name}</>}</span>
            </div>
          </div>
        )}
        {target && <Notice tone="warning">{t("patients.merge.warningNamed").replace("{from}", patient.full_name).replace("{to}", target.full_name)}</Notice>}
        {target && (
          <label className="row" style={{ gap: "var(--space-2)", cursor: "pointer" }}>
            <input type="checkbox" checked={sure} onChange={(e) => setSure(e.target.checked)} data-testid="merge-sure" />
            <span>{t("patients.merge.sure").replace("{n}", digits(patient.patient_number, lang))}</span>
          </label>
        )}
      </div>
    </Dialog>
  );
}
