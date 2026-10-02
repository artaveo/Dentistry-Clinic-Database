import { useEffect, useState } from "react";
import { ScrollText } from "lucide-react";
import type { AuditEntry } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { formatDateTime } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Card, CardHeader } from "../../ui/Card";
import { EmptyState, ErrorState, SkeletonRows } from "../../ui/Feedback";

export function PatientAuditTab({ patientId }: { patientId: string }) {
  const { t, err, lang } = useI18n();
  const [rows, setRows] = useState<AuditEntry[] | null>(null);
  const [error, setError] = useState("");
  const label = (prefix: string, code: string) => {
    const k = `${prefix}.${code}`;
    const v = t(k);
    return v === k ? code : v;
  };

  const load = () =>
    rpc("audit.list", { limit: 200, offset: 0, entity_id: patientId })
      .then((r) => {
        setRows(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patientId]);

  return (
    <Card flush>
      <CardHeader title={t("patients.tab.audit")} />
      {error && !rows ? (
        <ErrorState message={error} onRetry={load} />
      ) : (
        <div className="table-wrap">
          <table className="table" data-testid="patient-audit-list">
            <thead><tr><th>{t("audit.when")}</th><th>{t("audit.who")}</th><th>{t("audit.action")}</th></tr></thead>
            <tbody>
              {!rows && <SkeletonRows rows={4} cols={3} />}
              {rows?.map((r) => (
                <tr key={r.id}>
                  <td className="cell-num">{formatDateTime(r.at, lang)}</td>
                  <td><bdi className="ltr">{r.username ?? t("audit.system")}</bdi></td>
                  <td className="cell-strong">{label("audit.act", r.action)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {rows?.length === 0 && <EmptyState icon={ScrollText} title={t("audit.empty")} />}
        </div>
      )}
    </Card>
  );
}
