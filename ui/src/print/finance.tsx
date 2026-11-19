// Full A4 financial documents (docs/design/finance-standard.md): the invoice, the patient's statement, a money
// account's day and the doctor share preview. Each has its own layout, on the clinic's letterhead like every
// printed document; the receipt is a separate, small document (print/receipt.tsx).
import type { CalendarSystem, CashDayInfo, CommissionReport, InvoiceInfo, Language, ReceiptInfo, StatementInfo, ToothNumbering, VoucherInfo } from "../../../shared/ts/contract";
import { teethLabel } from "../chart/numbering";
import { formatDate, formatDateTime } from "../lib/dates";
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

export function InvoiceSheet({ invoice, numbering, footer, ...c }: Common & { invoice: InvoiceInfo; numbering: ToothNumbering; footer?: string }) {
  const { lang, calendar, letterhead } = c;
  const t = docT(lang);
  const money = (v: number) => formatAmount(v, lang);
  const received = invoice.allocations.filter((a) => a.amount !== 0);
  const adjusted = invoice.adjustments.filter((a) => a.status === "posted");
  return (
    <Sheet paper="a4" lang={lang} letterhead={letterhead} testId={`invoice-sheet-${invoice.number}`} void={invoice.status === "void"}>
      <HeaderBlock letterhead={letterhead} lang={lang} doctor={null} />
      <Title title={t("inv.title")} number={invoice.number} date={invoice.date} lang={lang} calendar={calendar} />
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
              <th className="num">{t("inv.col.discount")}</th>
              <th className="num">{t("inv.col.total")}</th>
            </tr>
          </thead>
          <tbody>
            {invoice.lines.map((l) => (
              <tr key={l.no}>
                <td>{num(l.no, lang)}</td>
                <td>{l.name}</td>
                <td><bdi>{teethLabel(l.teeth, numbering, lang)}</bdi></td>
                <td className="num">{num(l.quantity, lang)}</td>
                <td className="num">{money(l.unit_price)}</td>
                <td className="num">{l.discount ? money(l.discount) : ""}</td>
                <td className="num">{money(l.net)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <Totals
          lang={lang}
          rows={[
            { label: t("inv.subtotal"), value: invoice.gross },
            ...(invoice.discount ? [{ label: t("inv.discount"), value: invoice.discount, negative: true }] : []),
            { label: t("inv.net"), value: invoice.net, strong: true },
            ...(invoice.reductions ? [{ label: t("inv.reductions"), value: invoice.reductions, negative: true }] : []),
            ...(invoice.written_off ? [{ label: t("inv.writtenOff"), value: invoice.written_off, negative: true }] : []),
            { label: t("inv.paid"), value: invoice.allocated },
            { label: t("inv.remaining"), value: invoice.remaining, strong: true },
          ]}
        />
        {invoice.note && <div className="doc-note">{invoice.note}</div>}
        {(received.length > 0 || adjusted.length > 0) && (
          <>
            <h2 className="doc-h2">{t("inv.payments")}</h2>
            <table className="doc-table fin-table">
              <tbody>
                {received.map((p) => (
                  <tr key={`a${p.doc_id}`}>
                    <td><bdi className="ltr">{p.number}</bdi></td>
                    <td>{formatDate(p.date, lang, calendar)}</td>
                    <td>{t(`fin.doc.${p.kind}`)}</td>
                    <td className="num">{p.amount < 0 ? "− " : ""}{money(Math.abs(p.amount))}</td>
                  </tr>
                ))}
                {adjusted.map((a) => (
                  <tr key={`j${a.id}`}>
                    <td><bdi className="ltr">{a.number}</bdi></td>
                    <td>{formatDate(a.date, lang, calendar)}</td>
                    <td>{t(a.kind === "write_off" ? "fin.doc.wo" : "fin.doc.cn")} · {a.reason}</td>
                    <td className="num">{money(a.amount)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
        {invoice.plan && invoice.plan.installments.length > 0 && (
          <>
            <h2 className="doc-h2">{t("inv.installments")}</h2>
            <table className="doc-table fin-table" data-testid="invoice-installments">
              <thead>
                <tr><th>#</th><th>{t("inst.due")}</th><th className="num">{t("inst.amount")}</th><th>{t("inst.state")}</th></tr>
              </thead>
              <tbody>
                {invoice.plan.installments.map((i) => (
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
  return (
    <Sheet paper="a4" lang={lang} letterhead={letterhead} testId="statement-sheet">
      <HeaderBlock letterhead={letterhead} lang={lang} doctor={null} />
      <Title title={t("stmt.title")} date={statement.to} lang={lang} calendar={calendar} />
      <PatientBlock name={statement.patient_name} number={statement.patient_number} phone={statement.phone} lang={lang} />
      {statement.from && (
        <div className="doc-meta" style={{ marginBottom: "2mm" }}>
          <span>{t("stmt.period")}: {formatDate(statement.from, lang, calendar)} — {formatDate(statement.to, lang, calendar)}</span>
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
            {statement.opening !== 0 && (
              <tr className="muted-row"><td colSpan={4}>{t("stmt.opening")}</td><td className="num">{money(statement.opening)}</td></tr>
            )}
            {statement.lines.map((l) => (
              <tr key={l.doc_id}>
                <td>{formatDate(l.date, lang, calendar)}</td>
                <td>{t(`fin.doc.${l.kind}`)} <bdi className="ltr">{l.number}</bdi>{l.description && <span className="doc-muted"> · {l.description}</span>}</td>
                <td className="num">{l.charge ? money(l.charge) : ""}</td>
                <td className="num">{l.credit ? `${l.credit < 0 ? "− " : ""}${money(Math.abs(l.credit))}` : ""}</td>
                <td className="num">{l.balance < 0 ? "− " : ""}{money(Math.abs(l.balance))}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <Totals
          lang={lang}
          rows={[
            { label: t("stmt.charges"), value: statement.charges },
            { label: t("stmt.credits"), value: statement.credits },
            { label: statement.closing < 0 ? t("stmt.inCredit") : t("acct.remaining"), value: Math.abs(statement.closing), strong: true },
          ]}
        />
      </div>
    </Sheet>
  );
}

/** One money account on one day: where the money came from and went, the receipts and payments, the counts. */
export function CashDaySheet({ day, account, receipts, vouchers, ...c }: Common & { day: CashDayInfo; account: string; receipts: ReceiptInfo[]; vouchers: VoucherInfo[] }) {
  const { lang, calendar, letterhead } = c;
  const t = docT(lang);
  const money = (v: number) => formatAmount(v, lang);
  const last = day.counts[day.counts.length - 1] ?? null;
  return (
    <Sheet paper="a4" lang={lang} letterhead={letterhead} testId="cash-sheet">
      <HeaderBlock letterhead={letterhead} lang={lang} doctor={null} />
      <Title title={`${t("cash.report")} — ${account}`} date={day.date} lang={lang} calendar={calendar} />
      <div className="doc-body">
        <Totals
          lang={lang}
          rows={[
            { label: t("cash.opening"), value: day.opening },
            { label: t("cash.received"), value: day.received },
            ...(day.other_in ? [{ label: t("cash.otherIn"), value: day.other_in }] : []),
            ...(day.refunded ? [{ label: t("cash.refunded"), value: day.refunded, negative: true }] : []),
            ...(day.paid_out ? [{ label: t("cash.paidOut"), value: day.paid_out, negative: true }] : []),
            ...(day.transfers ? [{ label: t("cash.transfers"), value: Math.abs(day.transfers), negative: day.transfers < 0 }] : []),
            ...(day.adjustments ? [{ label: t("cash.adjustments"), value: Math.abs(day.adjustments), negative: day.adjustments < 0 }] : []),
            { label: t("cash.closing"), value: day.closing, strong: true },
          ]}
        />
        {receipts.length > 0 && (
          <>
            <h2 className="doc-h2">{t("cash.paymentsOfDay")}</h2>
            <table className="doc-table fin-table" data-testid="cash-payments">
              <tbody>
                {receipts.map((p) => (
                  <tr key={p.id}>
                    <td><bdi className="ltr">{p.number}</bdi></td>
                    <td>{p.patient_name}</td>
                    <td>{t(`fin.doc.${p.kind}`)}</td>
                    <td className="num">{p.kind === "rf" ? "− " : ""}{money(p.amount)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
        {vouchers.length > 0 && (
          <>
            <h2 className="doc-h2">{t("cash.expensesOfDay")}</h2>
            <table className="doc-table fin-table">
              <tbody>
                {vouchers.map((v) => (
                  <tr key={v.id}><td><bdi className="ltr">{v.number}</bdi></td><td>{v.name ?? v.supplier_name ?? t(`fin.purpose.${v.purpose ?? "other"}`)}</td><td className="num">{money(v.net)}</td></tr>
                ))}
              </tbody>
            </table>
          </>
        )}
        {day.counts.length > 0 && (
          <>
            <h2 className="doc-h2">{t("cash.counts")}</h2>
            <table className="doc-table fin-table">
              <tbody>
                {day.counts.map((k) => (
                  <tr key={k.id}>
                    <td><bdi className="ltr">{k.number}</bdi></td>
                    <td>{t("cash.book")}: {money(k.book)}</td>
                    <td>{t("cash.counted")}: {money(k.counted)}</td>
                    <td className="num">{k.difference ? `${k.difference > 0 ? "+" : "−"}${money(Math.abs(k.difference))}` : "✓"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {last?.reason && <div className="doc-note">{t("cash.reason")}: {last.reason}</div>}
          </>
        )}
      </div>
      <div className="doc-signatures">
        <div className="doc-sign"><span className="doc-sign-line" />{last?.created_by_name ?? t("inv.sign.cashier")}</div>
        <div className="doc-sign"><span className="doc-sign-line" />{t("cash.sign.manager")}</div>
      </div>
      <div className="doc-meta"><span>{last ? `${t("cash.countedAt")}: ${formatDateTime(last.created_at, lang, calendar)}` : t("cash.notCounted")}</span></div>
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
                  <tr key={i}><td>{formatDate(l.date, lang, calendar)}</td><td><bdi className="ltr">{l.invoice_number}</bdi></td><td>{l.patient_name}</td><td>{l.name}</td><td className="num">{money(l.base)}</td><td className="num">{money(l.commission)}</td></tr>
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
