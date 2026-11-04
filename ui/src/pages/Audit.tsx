import { useEffect, useRef, useState } from "react";
import { ChevronDown, Monitor, ScrollText, ShieldAlert } from "lucide-react";
import type { AuditEntry, CalendarSystem } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { formatDateTime } from "../lib/dates";
import { presetRange } from "../lib/reports";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Card, Page, PageHeader } from "../ui/Card";
import { Switch } from "../ui/Controls";
import { Field, Select, TextInput } from "../ui/Field";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../ui/Feedback";
import { Avatar } from "../ui/Brand";

const PAGE = 50;

/** Failed/blocked security events stand out; everything else is neutral. */
function tone(action: string) {
  return /failed|blocked/.test(action) ? "danger" : /^(session|auth)\./.test(action) ? "info" : "accent";
}

export function AuditPage({ calendar }: { calendar?: CalendarSystem }) {
  const { t, err, lang } = useI18n();
  const [rows, setRows] = useState<AuditEntry[] | null>(null);
  const [done, setDone] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // 9.8: filters of the full log. Text filters wait for a pause in typing; the others apply at once.
  const [action, setAction] = useState("");
  const [user, setUser] = useState("");
  const [when, setWhen] = useState<"all" | "today" | "week" | "month">("all");
  const [sensitive, setSensitive] = useState(false);
  const [applied, setApplied] = useState({ action: "", user: "" });
  const typing = useRef<number | undefined>(undefined);
  useEffect(() => {
    window.clearTimeout(typing.current);
    typing.current = window.setTimeout(() => setApplied({ action: action.trim(), user: user.trim() }), 300);
    return () => window.clearTimeout(typing.current);
  }, [action, user]);
  const range = when === "all" ? null : presetRange(when, calendar ?? "shamsi");
  const label = (prefix: string, code: string) => {
    const k = `${prefix}.${code}`;
    const v = t(k);
    return v === k ? code : v;
  };

  const more = (offset: number) => {
    setBusy(true);
    return rpc("audit.list", { limit: PAGE, offset, entity_id: null, action: applied.action || null, username: applied.user || null, from: range?.from ?? null, to: range?.to ?? null, sensitive_only: sensitive })
      .then((r) => {
        setRows((prev) => (offset === 0 || !prev ? r : [...prev, ...r]));
        setDone(r.length < PAGE);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)))
      .finally(() => setBusy(false));
  };
  useEffect(() => {
    more(0);
  }, [applied, when, sensitive]);

  return (
    <Page testId="page-audit">
      <PageHeader title={t("audit.title")} description={t("audit.subtitle")} />
      <Notice tone="info">{t("audit.readOnly")}</Notice>
      <div className="report-filters" data-testid="audit-filters">
        <Field label={t("audit.filter.action")}><TextInput value={action} onChange={(e) => setAction(e.target.value)} placeholder={t("audit.filter.actionHint")} data-testid="audit-filter-action" /></Field>
        <Field label={t("audit.filter.user")}><TextInput value={user} onChange={(e) => setUser(e.target.value)} dir="ltr" data-testid="audit-filter-user" /></Field>
        <Field label={t("audit.filter.when")}>
          <Select value={when} onChange={(e) => setWhen(e.target.value as typeof when)} data-testid="audit-filter-when">
            <option value="all">{t("audit.when.all")}</option>
            <option value="today">{t("reports.preset.today")}</option>
            <option value="week">{t("reports.preset.week")}</option>
            <option value="month">{t("reports.preset.month")}</option>
          </Select>
        </Field>
        <Switch checked={sensitive} onChange={setSensitive} label={t("audit.filter.sensitive")} testId="audit-filter-sensitive" />
      </div>
      <Card flush>
        {error && !rows ? (
          <ErrorState message={error} onRetry={() => more(0)} />
        ) : (
          <div className="table-wrap">
            <table className="table" data-testid="audit-list">
              <thead>
                <tr><th>{t("audit.when")}</th><th>{t("audit.who")}</th><th>{t("audit.action")}</th><th>{t("audit.entity")}</th><th>{t("audit.computer")}</th></tr>
              </thead>
              <tbody>
                {!rows && <SkeletonRows rows={6} cols={5} />}
                {rows?.map((r) => (
                  <tr key={r.id}>
                    <td className="cell-num">{formatDateTime(r.at, lang, calendar)}</td>
                    <td>
                      <div className="person">
                        <Avatar name={r.username ?? "S"} size="sm" />
                        <bdi className="ltr">{r.username ?? t("audit.system")}</bdi>
                      </div>
                    </td>
                    <td>
                      <div className="person-text">
                        <span className="row" style={{ flexWrap: "nowrap" }}>
                          <span className={`audit-dot ${tone(r.action)}`} aria-hidden />
                          <span className="cell-strong">{label("audit.act", r.action)}</span>
                          {r.sensitive && <Badge tone="warning" icon={ShieldAlert} testId="audit-sensitive">{t("audit.sensitive")}</Badge>}
                        </span>
                        <span className="person-sub"><bdi className="ltr">{r.action}</bdi></span>
                      </div>
                    </td>
                    <td>{r.entity ? label("audit.ent", r.entity) : <span className="subtle">—</span>}</td>
                    <td className="subtle"><span className="row" style={{ flexWrap: "nowrap" }}><Monitor size={15} aria-hidden /><bdi className="ltr">{r.computer}</bdi></span></td>
                  </tr>
                ))}
              </tbody>
            </table>
            {rows?.length === 0 && <EmptyState icon={ScrollText} title={t("audit.empty")} />}
            {rows && !done && (
              <div className="table-footer">
                <Button variant="subtle" icon={ChevronDown} loading={busy} onClick={() => more(rows.length)} data-testid="audit-more">{t("audit.more")}</Button>
              </div>
            )}
          </div>
        )}
      </Card>
    </Page>
  );
}
