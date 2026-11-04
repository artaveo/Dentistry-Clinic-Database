// The A4 print of a report (roadmap 9.5): the clinic's letterhead, the report's name and period, then the
// table exactly as the screen shows it. Words follow the language the user is working in.
import type { CalendarSystem, Language, ReportResult } from "../../../shared/ts/contract";
import { isNumeric, renderCell } from "../lib/reports";
import { formatDate } from "../lib/dates";
import { Letterhead as HeaderBlock, Sheet } from "./DocumentSheet";
import { docT } from "./docText";
import type { Letterhead } from "./letterhead";

export function ReportSheet({ result, title, showPeriod, letterhead, lang, calendar }: { result: ReportResult; title: string; showPeriod: boolean; letterhead: Letterhead; lang: Language; calendar: CalendarSystem }) {
  const t = docT(lang);
  return (
    <Sheet paper="a4" lang={lang} letterhead={letterhead} testId="report-sheet">
      <HeaderBlock letterhead={letterhead} lang={lang} doctor={null} />
      <h1 className="doc-title">{title}</h1>
      <div className="doc-meta">
        <span>{showPeriod ? `${t("reports.from")}: ${formatDate(result.from, lang, calendar)} — ${t("reports.to")}: ${formatDate(result.to, lang, calendar)}` : `${t("reports.asOf")}: ${formatDate(result.to, lang, calendar)}`}</span>
      </div>
      <div className="doc-body">
        <table className="doc-table fin-table report-table" data-testid="report-sheet-table">
          <thead>
            <tr>
              {result.columns.map((c) => (
                <th key={c.key}>{t(`reports.col.${c.key}`)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {result.rows.map((row, i) => (
              <tr key={i}>
                {row.map((cell, j) => (
                  <td key={j} className={isNumeric(cell) ? "num" : undefined}>{renderCell(cell, lang, calendar, t)}</td>
                ))}
              </tr>
            ))}
            {!result.rows.length && (
              <tr><td colSpan={result.columns.length} className="doc-muted">{t("reports.empty")}</td></tr>
            )}
          </tbody>
          {result.totals.length > 0 && (
            <tfoot>
              <tr>
                {result.totals.map((cell, j) => (
                  <th key={j} className={isNumeric(cell) ? "num" : undefined}>{j === 0 && cell.type === "empty" ? t("reports.total") : renderCell(cell, lang, calendar, t)}</th>
                ))}
              </tr>
            </tfoot>
          )}
        </table>
        {result.truncated && <div className="doc-note">{t("reports.truncated")}</div>}
      </div>
    </Sheet>
  );
}
