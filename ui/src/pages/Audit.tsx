import { useEffect, useState } from "react";
import type { AuditEntry } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { formatDateTime } from "../lib/dates";
import { useI18n } from "../i18n";

const PAGE = 50;

export function AuditPage() {
  const { t, err, lang } = useI18n();
  const [rows, setRows] = useState<AuditEntry[]>([]);
  const [done, setDone] = useState(false);
  const [error, setError] = useState("");
  const more = (offset: number) =>
    rpc("audit.list", { limit: PAGE, offset })
      .then((r) => {
        setRows((prev) => (offset === 0 ? r : [...prev, ...r]));
        setDone(r.length < PAGE);
      })
      .catch((e) => setError(err(e)));
  useEffect(() => {
    more(0);
  }, []);

  return (
    <div className="card wide">
      <h2>{t("audit.title")}</h2>
      {error && <div className="error">{error}</div>}
      <table data-testid="audit-list">
        <thead><tr><th>{t("audit.when")}</th><th>{t("audit.who")}</th><th>{t("audit.action")}</th><th>{t("audit.entity")}</th><th>{t("audit.computer")}</th></tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>{formatDateTime(r.at, lang)}</td>
              <td className="ltr">{r.username ?? "system"}</td>
              <td className="ltr">{r.action}</td>
              <td className="ltr">{r.entity ?? ""}</td>
              <td className="ltr">{r.computer}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {!done && <div className="actions"><button onClick={() => more(rows.length)}>{t("audit.more")}</button></div>}
    </div>
  );
}
