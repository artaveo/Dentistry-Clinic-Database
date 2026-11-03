// Full A4 financial documents (Phase 6, 6.11): the invoice, the patient's statement, the day's cash report and the
// doctor commission settlement. Each has its own layout, on the clinic's letterhead like every printed
// document; the receipt is a separate, small document (print/receipt.tsx).
import type { CalendarSystem, CashClosingInfo, CashDay, CommissionReport, ExpenseInfo, InvoiceInfo, Language, PaymentInfo, StatementInfo, ToothNumbering } from "../../../shared/ts/contract";
import { surfacesText, teethLabel } from "../chart/numbering";
import { formatDate, formatDateTime } from "../lib/dates";
import { tr } from "../lib/medical";
import { formatAmount } from "../lib/money";
import { Letterhead as HeaderBlock, Sheet } from "./DocumentSheet";
import { docT, num } from "./docText";
import type { Letterhead } from "./letterhead";

type Common = { letterhead: Letterhead; lang: Language; calendar: CalendarSystem };

function Title({ title, number, date, lang, calendar }: { title: string; number?: string; date: string; lang: Language; calendar: CalendarSystem }) {
  const t = docT(lang);
  return (
    <>
      <h1 className="doc-title">{title}</h1>
      <div className="doc-meta">
        <span>{number ? <>{t("doc.number")}: <bdi className="ltr">{number}</bdi></> : null}</span>
        <span>{t("doc.date")}: {formatDate(date, lang, calendar)}</span>
      </div>
    </>
  );
}

function PatientBlock({ name, number, phone, lang }: { name: string; number: string; phone?: string | null; lang: Language }) {
  const t = docT(lang);
  return (
    <div className="doc-patient" data-testid="doc-patient">
      <span className="doc-patient-name">{t("doc.patient")}: <strong>{name}</strong></span>
      <span className="doc-patient-facts">
        <span>{t("doc.patientId")}: <bdi className="ltr">{number}</bdi></span>
        {phone && <span><bdi className="ltr">{phone}</bdi></span>}
      </span>
    </div>
  );
}

/** Label / amount pairs of a total block, the last one stressed. */
function Totals({ rows, lang }: { rows: { label: string; value: number; strong?: boolean; negative?: boolean }[]; lang: Language }) {
  return (
    <dl className="fin-totals" data-testid="fin-totals">
      {rows.map((r, i) => (
        <div key={i} className={r.strong ? "strong" : undefined}>
          <dt>{r.label}</dt>
          <dd className="num">{r.negative && r.value ? "− " : ""}{formatAmount(r.value, lang)}</dd>
        </div>
      ))}
    </dl>
  );
}

export function InvoiceSheet({ invoice, payments, numbering, footer, ...c }: Common & { invoice: InvoiceInfo; payments: PaymentInfo[]; numbering: ToothNumbering; footer?: string }) {
  const { lang, calendar, letterhead } = c;
  const t = docT(lang);
  const money = (v: number) => formatAmount(v, lang);
  const received = payments.filter((p) => p.kind !== "refund");
  return (
    <Sheet paper="a4" lang={lang} letterhead={letterhead} testId={`invoice-sheet-${invoice.number}`} void={invoice.status === "void"}>
      <HeaderBlock letterhead={letterhead} lang={lang} doctor={null} />
      <Title title={t("inv.title")} number={invoice.number} date={invoice.issued_at} lang={lang} calendar={calendar} />
      <PatientBlock name={invoice.patient_name} number={invoice.patient_number} lang={lang} />
      <div className="doc-body">
        <table className="doc-table fin-table" data-testid="invoice-lines">
          <thead>
            <tr>
              <th>#</th>
              <th>{t("inv.col.service")}</th>
              <th>{t("inv.col.teeth")}</th>
              <th className="num">{t("inv.col.qty")}</th>
              <th className="num">{t("inv.col.unit")}</th>
              <th className="num">{t("inv.col.total")}</th>
            </tr>
          </thead>
          <tbody>
            {invoice.lines.map((l, i) => (
              <tr key={l.id}>
                <td>{num(i + 1, lang)}</td>
                <td>{tr(l.names, lang)}</td>
                <td><bdi>{[teethLabel(l.teeth, numbering, lang), surfacesText(l.surfaces, l.teeth[0])].filter(Boolean).join(" ")}</bdi></td>
                <td className="num">{num(l.quantity, lang)}</td>
                <td className="num">{money(l.unit_price)}</td>
                <td className="num">{money(l.line_total)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <Totals
          lang={lang}
          rows={[
            { label: t("inv.subtotal"), value: invoice.subtotal },
            ...(invoice.discount ? [{ label: t("inv.discount"), value: invoice.discount, negative: true }] : []),
            ...(invoice.credit_notes_total ? [{ label: t("inv.creditNotes"), value: invoice.credit_notes_total, negative: true }] : []),
            { label: t("inv.net"), value: invoice.net, strong: true },
            { label: t("inv.paid"), value: invoice.paid - invoice.refunded },
            { label: t("inv.remaining"), value: invoice.remaining, strong: true },
          ]}
        />
        {invoice.note && <div className="doc-note">{invoice.note}</div>}
        {received.length > 0 && (
          <>
            <h2 className="doc-h2">{t("inv.payments")}</h2>
            <table className="doc-table fin-table">
              <tbody>
                {received.map((p) => (
                  <tr key={p.id}>
                    <td><bdi className="ltr">{p.number}</bdi></td>
                    <td>{formatDate(p.paid_at, lang, calendar)}</td>
                    <td>{p.kind === "credit" ? t("pay.kind.credit") : tr(p.method, lang)}</td>
                    <td className="num">{money(p.amount)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
        {invoice.installments.length > 0 && (
          <>
            <h2 className="doc-h2">{t("inv.installments")}</h2>
            <table className="doc-table fin-table" data-testid="invoice-installments">
              <thead>
                <tr><th>#</th><th>{t("inst.due")}</th><th className="num">{t("inst.amount")}</th><th>{t("inst.state")}</th></tr>
              </thead>
              <tbody>
                {invoice.installments.map((i) => (
                  <tr key={i.no}>
                    <td>{num(i.no, lang)}</td>
                    <td>{formatDate(i.due_date, lang, calendar)}</td>
                    <td className="num">{money(i.amount)}</td>
                    <td>{t(`inst.state.${i.state}`)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
        {invoice.status === "void" && invoice.void_reason && <div className="doc-note">{t("inv.voidReason")}: {invoice.void_reason}</div>}
        {footer && <div className="doc-closing">{footer}</div>}
      </div>
      <div className="doc-signatures">
        <div className="doc-sign"><span className="doc-sign-line" />{t("doc.sign.patient")}</div>
        <div className="doc-sign"><span className="doc-sign-line" />{t("inv.sign.cashier")}</div>
      </div>
    </Sheet>
  );
}

export function StatementSheet({ statement, ...c }: Common & { statement: StatementInfo }) {
  const { lang, calendar, letterhead } = c;
  const t = docT(lang);
  const money = (v: number) => formatAmount(v, lang);
  const a = statement.account;
  const balance = statement.lines.length ? statement.lines[statement.lines.length - 1].balance : statement.opening_balance;
  return (
    <Sheet paper="a4" lang={lang} letterhead={letterhead} testId="statement-sheet">
      <HeaderBlock letterhead={letterhead} lang={lang} doctor={null} />
      <Title title={t("stmt.title")} date={statement.generated_at} lang={lang} calendar={calendar} />
      <PatientBlock name={statement.patient_name} number={statement.patient_number} phone={statement.phone} lang={lang} />
      {(statement.from || statement.to) && (
        <div className="doc-meta" style={{ marginBottom: "2mm" }}>
          <span>{t("stmt.period")}: {statement.from ? formatDate(statement.from, lang, calendar) : "…"} — {statement.to ? formatDate(statement.to, lang, calendar) : "…"}</span>
        </div>
      )}
      <div className="doc-body">
        <table className="doc-table fin-table" data-testid="statement-lines">
          <thead>
            <tr>
              <th>{t("doc.date")}</th>
              <th>{t("stmt.col.doc")}</th>
              <th className="num">{t("stmt.col.debit")}</th>
              <th className="num">{t("stmt.col.credit")}</th>
              <th className="num">{t("stmt.col.balance")}</th>
            </tr>
          </thead>
          <tbody>
            {statement.opening_balance !== 0 && (
              <tr className="muted-row"><td colSpan={4}>{t("stmt.opening")}</td><td className="num">{money(statement.opening_balance)}</td></tr>
            )}
            {statement.lines.map((l, i) => (
              <tr key={i}>
                <td>{formatDate(l.at, lang, calendar)}</td>
                <td>{t(`stmt.kind.${l.kind}`)} <bdi className="ltr">{l.number}</bdi>{l.description && <span className="doc-muted"> · {l.description}</span>}</td>
                <td className="num">{l.debit ? money(l.debit) : ""}</td>
                <td className="num">{l.credit ? money(l.credit) : ""}</td>
                <td className="num">{money(l.balance)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <Totals
          lang={lang}
          rows={[
            { label: t("acct.total"), value: a.total },
            { label: t("acct.discount"), value: a.discount, negative: true },
            { label: t("acct.paid"), value: a.paid },
            { label: t("acct.refunded"), value: a.refunded },
            ...(a.credit ? [{ label: t("acct.credit"), value: a.credit }] : []),
            { label: balance < 0 ? t("stmt.inCredit") : t("acct.remaining"), value: Math.abs(balance), strong: true },
          ]}
        />
      </div>
    </Sheet>
  );
}

export function CashDaySheet({ day, payments, expenses, ...c }: Common & { day: CashDay; payments: PaymentInfo[]; expenses: ExpenseInfo[] }) {
  const { lang, calendar, letterhead } = c;
  const t = docT(lang);
  const money = (v: number) => formatAmount(v, lang);
  const closing: CashClosingInfo | null = day.closing;
  return (
    <Sheet paper="a4" lang={lang} letterhead={letterhead} testId="cash-sheet">
      <HeaderBlock letterhead={letterhead} lang={lang} doctor={null} />
      <Title title={t("cash.report")} date={day.date} lang={lang} calendar={calendar} />
      <div className="doc-body">
        <h2 className="doc-h2">{t("cash.byMethod")}</h2>
        <table className="doc-table fin-table">
          <thead><tr><th>{t("rcpt.method")}</th><th className="num">{t("cash.received")}</th><th className="num">{t("cash.refunded")}</th></tr></thead>
          <tbody>
            {day.by_method.map((m, i) => (
              <tr key={i}><td>{tr(m.method, lang)}</td><td className="num">{money(m.received)}</td><td className="num">{money(m.refunded)}</td></tr>
            ))}
            {!day.by_method.length && <tr><td colSpan={3} className="doc-muted">{t("cash.noPayments")}</td></tr>}
          </tbody>
        </table>
        <Totals
          lang={lang}
          rows={[
            { label: t("cash.cashIn"), value: day.cash_in },
            { label: t("cash.cashRefunds"), value: day.cash_refunds, negative: true },
            { label: t("cash.cashExpenses"), value: day.cash_expenses, negative: true },
            { label: t("cash.expected"), value: day.expected, strong: true },
            ...(closing ? [{ label: t("cash.counted"), value: closing.counted }, { label: t("cash.difference"), value: Math.abs(closing.difference), strong: closing.difference !== 0 }] : []),
          ]}
        />
        {closing?.reason && <div className="doc-note">{t("cash.reason")}: {closing.reason}</div>}
        {payments.length > 0 && (
          <>
            <h2 className="doc-h2">{t("cash.paymentsOfDay")}</h2>
            <table className="doc-table fin-table" data-testid="cash-payments">
              <tbody>
                {payments.map((p) => (
                  <tr key={p.id}>
                    <td><bdi className="ltr">{p.number}</bdi></td>
                    <td>{p.patient_name}</td>
                    <td>{p.kind === "credit" ? t("pay.kind.credit") : tr(p.method, lang)}</td>
                    <td className="num">{p.kind === "refund" ? "− " : ""}{money(p.amount)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
        {expenses.length > 0 && (
          <>
            <h2 className="doc-h2">{t("cash.expensesOfDay")}</h2>
            <table className="doc-table fin-table">
              <tbody>
                {expenses.map((e) => (
                  <tr key={e.id}><td>{tr(e.category, lang)}</td><td>{e.description}</td><td className="num">{money(e.amount)}</td></tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </div>
      <div className="doc-signatures">
        <div className="doc-sign"><span className="doc-sign-line" />{closing?.closed_by_name ?? t("inv.sign.cashier")}</div>
        <div className="doc-sign"><span className="doc-sign-line" />{t("cash.sign.manager")}</div>
      </div>
      <div className="doc-meta"><span>{closing ? `${t("cash.closedAt")}: ${formatDateTime(closing.closed_at, lang, calendar)}` : t("cash.notClosed")}</span></div>
    </Sheet>
  );
}

export function CommissionSheet({ report, ...c }: Common & { report: CommissionReport }) {
  const { lang, calendar, letterhead } = c;
  const t = docT(lang);
  const money = (v: number) => formatAmount(v, lang);
  return (
    <Sheet paper="a4" lang={lang} letterhead={letterhead} testId="commission-sheet">
      <HeaderBlock letterhead={letterhead} lang={lang} doctor={null} />
      <Title title={t("comm.title")} date={report.to} lang={lang} calendar={calendar} />
      <div className="doc-meta" style={{ marginBottom: "2mm" }}>
        <span>{t("stmt.period")}: {formatDate(report.from, lang, calendar)} — {formatDate(report.to, lang, calendar)}</span>
        <span>{t(`comm.basis.${report.basis}`)}</span>
      </div>
      <div className="doc-body">
        {report.doctors.map((d) => (
          <div key={d.doctor_id} className="fin-block">
            <h2 className="doc-h2">{d.doctor_name}</h2>
            <table className="doc-table fin-table">
              <thead><tr><th>{t("doc.date")}</th><th>{t("rcpt.invoice")}</th><th>{t("doc.patient")}</th><th>{t("inv.col.service")}</th><th className="num">{t("comm.base")}</th><th className="num">{t("comm.amount")}</th></tr></thead>
              <tbody>
                {d.lines.map((l, i) => (
                  <tr key={i}><td>{formatDate(l.date, lang, calendar)}</td><td><bdi className="ltr">{l.invoice_number}</bdi></td><td>{l.patient_name}</td><td>{tr(l.names, lang)}</td><td className="num">{money(l.base)}</td><td className="num">{money(l.commission)}</td></tr>
                ))}
              </tbody>
            </table>
            <Totals lang={lang} rows={[{ label: t("comm.base"), value: d.base }, { label: t("comm.owed"), value: d.commission, strong: true }]} />
          </div>
        ))}
        {!report.doctors.length && <p className="doc-muted">{t("comm.none")}</p>}
        {report.doctors.length > 1 && <Totals lang={lang} rows={[{ label: t("comm.owedTotal"), value: report.commission_total, strong: true }]} />}
      </div>
    </Sheet>
  );
}
