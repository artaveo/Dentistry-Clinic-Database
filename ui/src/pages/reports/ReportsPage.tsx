// «گزارش‌ها» — every report of the clinic in one place (Phase 9, 9.2–9.5): pick a report on the left, a
// period and (where it makes sense) a doctor or a service on top, read the table, and take it away as Excel
// or on paper. The Core builds the numbers; this screen only chooses and shows.
import { useEffect, useMemo, useRef, useState } from "react";
import { FileSpreadsheet, FileX, FolderOpen, Printer } from "lucide-react";
import type { ClinicProfile, DoctorInfo, ReportKind, ReportParams, ReportResult, ServiceInfo } from "../../../../shared/ts/contract";
import { isSessionError, revealExport, rpc } from "../../lib/api";
import { todayIso } from "../../lib/calendar";
import { digits } from "../../lib/dates";
import { GROUPS, PRESETS, REPORTS, TAG_VALUES, isNumeric, presetRange, renderCell, tagLabels } from "../../lib/reports";
import type { PeriodPreset, ReportGroup } from "../../lib/reports";
import { tr } from "../../lib/medical";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, Page, PageHeader } from "../../ui/Card";
import { DateField } from "../../ui/DateField";
import { EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, Select } from "../../ui/Field";
import { DocumentPreview } from "../../print/DocumentPreview";
import { ReportSheet } from "../../print/report";
import { useLetterhead } from "../../print/letterhead";

export function ReportsPage({ clinic, canSeeStock }: { clinic: ClinicProfile | null; canSeeStock: boolean }) {
  const { t, lang, err } = useI18n();
  const calendar = clinic?.calendar_system ?? "shamsi";
  const letterhead = useLetterhead();
  const groups = GROUPS.filter((g) => g !== "inventory" || canSeeStock);
  const [kind, setKind] = useState<ReportKind>("daily_revenue");
  const meta = REPORTS.find((r) => r.kind === kind)!;
  const first = useMemo(() => presetRange("month", calendar), [calendar]);
  const [from, setFrom] = useState(first.from);
  const [to, setTo] = useState(first.to);
  const [doctor, setDoctor] = useState("");
  const [service, setService] = useState("");
  const [doctors, setDoctors] = useState<DoctorInfo[] | null>(null);
  const [services, setServices] = useState<ServiceInfo[] | null>(null);
  const [result, setResult] = useState<ReportResult | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [exported, setExported] = useState<{ name: string; path: string; rows: number } | null>(null);
  const [printing, setPrinting] = useState(false);
  const [tick, setTick] = useState(0);
  const run = useRef(0);

  // Doctors and services are only fetched once a report offers them as a filter.
  useEffect(() => {
    if (meta.filters.includes("doctor") && !doctors) rpc("doctors.list", { include_inactive: true }).then(setDoctors).catch(() => setDoctors([]));
    if (meta.filters.includes("service") && !services) rpc("catalog.get", { include_inactive: true }).then((c) => setServices(c.services)).catch(() => setServices([]));
  }, [meta, doctors, services]);

  const params = useMemo<ReportParams>(
    () => ({
      kind,
      // A report of the state today has no period of its own; the Core still wants valid dates.
      from: meta.period === "now" ? todayIso() : meta.period === "asOf" ? to : from,
      to: meta.period === "now" ? todayIso() : to,
      doctor_id: meta.filters.includes("doctor") && doctor ? doctor : null,
      service_id: meta.filters.includes("service") && service ? service : null,
      calendar,
    }),
    [kind, meta, from, to, doctor, service, calendar],
  );
  const invalid = meta.period === "range" && from > to;

  useEffect(() => {
    if (invalid) return;
    const mine = ++run.current;
    setBusy(true);
    setExported(null);
    rpc("reports.run", params)
      .then((r) => {
        if (mine !== run.current) return;
        setResult(r);
        setError("");
      })
      .catch((e) => {
        if (mine !== run.current || isSessionError(e)) return;
        setResult(null);
        setError(err(e));
      })
      .finally(() => mine === run.current && setBusy(false));
  }, [params, invalid, tick]);

  const title = t(`reports.kind.${kind}`);
  const pick = (g: ReportGroup) => {
    const next = REPORTS.find((r) => r.group === g)!;
    setKind(next.kind);
    setDoctor("");
    setService("");
  };
  const choose = (k: ReportKind) => {
    setKind(k);
    setDoctor("");
    setService("");
  };
  const applyPreset = (p: PeriodPreset) => {
    const r = presetRange(p, calendar);
    setFrom(r.from);
    setTo(r.to);
  };

  const exportExcel = async () => {
    if (!result) return;
    setError("");
    try {
      const done = await rpc("reports.export", {
        report: params,
        language: lang,
        title,
        headers: result.columns.map((c) => t(`reports.col.${c.key}`)),
        labels: tagLabels(t, TAG_VALUES),
      });
      setExported({ name: done.file_name, path: done.file_path, rows: done.rows });
    } catch (x) {
      if (!isSessionError(x)) setError(err(x));
    }
  };

  const rows = result?.rows ?? [];
  const ready = !!result && !busy;
  return (
    <Page testId="page-reports">
      <PageHeader
        title={t("reports.title")}
        description={t("reports.subtitle")}
        actions={
          <>
            <Button icon={FileSpreadsheet} onClick={exportExcel} disabled={!ready || !rows.length} data-testid="report-export">{t("reports.excel")}</Button>
            <Button icon={Printer} onClick={() => setPrinting(true)} disabled={!ready || !letterhead} data-testid="report-print">{t("reports.print")}</Button>
          </>
        }
      />
      <div className="reports-layout">
        <nav className="report-nav" aria-label={t("reports.list")} data-testid="report-nav">
          {groups.map((g) => (
            <div className="report-group" key={g}>
              <span className="t-overline">{t(`reports.group.${g}`)}</span>
              {REPORTS.filter((r) => r.group === g).map((r) => (
                <button key={r.kind} type="button" className="nav-item" aria-current={kind === r.kind ? "page" : undefined} onClick={() => choose(r.kind)} data-testid={`report-${r.kind}`}>
                  <r.icon aria-hidden />
                  <span>{t(`reports.kind.${r.kind}`)}</span>
                </button>
              ))}
            </div>
          ))}
        </nav>
        <div className="stack report-main">
          {/* On a narrow window the list becomes a menu. */}
          <div className="report-pick">
            <Field label={t("reports.group")}>
              <Select value={meta.group} onChange={(e) => pick(e.target.value as ReportGroup)} data-testid="report-group">
                {groups.map((g) => <option key={g} value={g}>{t(`reports.group.${g}`)}</option>)}
              </Select>
            </Field>
            <Field label={t("reports.report")}>
              <Select value={kind} onChange={(e) => choose(e.target.value as ReportKind)} data-testid="report-select">
                {REPORTS.filter((r) => r.group === meta.group).map((r) => <option key={r.kind} value={r.kind}>{t(`reports.kind.${r.kind}`)}</option>)}
              </Select>
            </Field>
          </div>
          <div className="report-filters" data-testid="report-filters">
            {meta.period !== "now" && (
              <>
                {meta.period === "range" && <div className="field"><span className="field-label">{t("reports.from")}</span><DateField value={from} onChange={setFrom} calendar={calendar} label={t("reports.from")} testId="report-from" yearsBefore={10} yearsAfter={0} /></div>}
                <div className="field"><span className="field-label">{meta.period === "range" ? t("reports.to") : t("reports.asOf")}</span><DateField value={to} onChange={setTo} calendar={calendar} label={meta.period === "range" ? t("reports.to") : t("reports.asOf")} testId="report-to" yearsBefore={10} yearsAfter={meta.period === "asOf" ? 3 : 0} /></div>
              </>
            )}
            {meta.filters.includes("doctor") && (
              <Field label={t("reports.doctor")}>
                <Select value={doctor} onChange={(e) => setDoctor(e.target.value)} data-testid="report-doctor">
                  <option value="">{t("reports.allDoctors")}</option>
                  {doctors?.map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
                </Select>
              </Field>
            )}
            {meta.filters.includes("service") && (
              <Field label={t("reports.service")}>
                <Select value={service} onChange={(e) => setService(e.target.value)} data-testid="report-service">
                  <option value="">{t("reports.allServices")}</option>
                  {services?.map((s) => <option key={s.id} value={s.id}>{tr(s.name, lang)}</option>)}
                </Select>
              </Field>
            )}
            {meta.period === "range" && (
              <div className="row report-presets" role="group" aria-label={t("reports.quick")}>
                {PRESETS.map((p) => <Button key={p} size="sm" variant="subtle" onClick={() => applyPreset(p)} data-testid={`report-preset-${p}`}>{t(`reports.preset.${p}`)}</Button>)}
              </div>
            )}
          </div>
          {meta.period === "now" && <Notice tone="info">{t("reports.nowNote")}</Notice>}
          {invalid && <Notice tone="warning" testId="report-range-error">{t("rule.period_range")}</Notice>}
          {exported && (
            <Notice tone="success" testId="report-export-done">
              {t("patients.export.done").replace("{name}", exported.name).replace("{n}", digits(exported.rows, lang))}{" "}
              <Button variant="link" icon={FolderOpen} onClick={() => revealExport(exported.path).catch((x) => setError(err(x)))}>{t("patients.export.openFolder")}</Button>
            </Notice>
          )}
          <Card flush>
            {error ? (
              <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />
            ) : (
              <div className="table-wrap" aria-busy={busy}>
                <table className="table report-table" data-testid="report-table" aria-label={title}>
                  <caption className="sr-only">{title}</caption>
                  <thead>
                    <tr>
                      {(result?.columns ?? []).map((c, i) => (
                        <th key={c.key} scope="col" className={rows[0] && isNumeric(rows[0][i]) ? "num" : undefined}>{t(`reports.col.${c.key}`)}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {!result && busy && <SkeletonRows rows={6} cols={5} />}
                    {rows.map((row, i) => (
                      <tr key={i}>
                        {row.map((cell, j) => (
                          <td key={j} className={isNumeric(cell) ? "cell-num num nowrap" : cell.type === "date" || cell.type === "month" || cell.type === "code" ? "nowrap" : undefined}>{renderCell(cell, lang, calendar, t)}</td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                  {result && result.totals.length > 0 && (
                    <tfoot>
                      <tr data-testid="report-totals">
                        {result.totals.map((cell, j) => (
                          <th key={j} scope="row" className={isNumeric(cell) ? "num" : undefined}>{j === 0 && cell.type === "empty" ? t("reports.total") : renderCell(cell, lang, calendar, t)}</th>
                        ))}
                      </tr>
                    </tfoot>
                  )}
                </table>
                {result && !rows.length && <EmptyState icon={FileX} title={t("reports.empty")}>{t("reports.emptyHint")}</EmptyState>}
              </div>
            )}
          </Card>
          {result && (
            <p className="muted report-foot" role="status" data-testid="report-foot">
              {t("reports.count").replace("{n}", digits(rows.length, lang)).replace("{ms}", digits(result.millis, lang))}
              {result.truncated && <> · <strong>{t("reports.truncated")}</strong></>}
            </p>
          )}
        </div>
      </div>
      {printing && result && letterhead && (
        <DocumentPreview
          title={title}
          paper="a4"
          fileStem={`REPORT-${kind}-${result.to}`}
          testId="report-preview"
          onClose={() => setPrinting(false)}
          sheet={<ReportSheet result={result} title={title} showPeriod={meta.period === "range"} letterhead={letterhead} lang={lang} calendar={calendar} />}
        />
      )}
    </Page>
  );
}
