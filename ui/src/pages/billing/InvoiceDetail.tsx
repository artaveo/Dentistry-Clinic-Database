import { payQ } from "../../lib/billing";
import { useEffect, useState } from "react";
import { Ban, CalendarClock, FileMinus2, Printer, Receipt, Undo2, Wallet } from "lucide-react";
import type { CalendarSystem, InvoiceInfo, PaymentInfo, ToothNumbering } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { useClinicSettings } from "../../lib/clinical";
import { digits, formatDate, formatDateTime } from "../../lib/dates";
import { tr } from "../../lib/medical";
import { formatAmount } from "../../lib/money";
import { surfacesText, teethLabel } from "../../chart/numbering";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Badge, ErrorState, Loading } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { InvoicePrint, PayBadge, ReceiptHost } from "./common";
import type { BillingPerms } from "./common";
import { InstallmentsDialog, PaymentDialog, ReasonDialog } from "./dialogs";

const INST_TONE = { pending: "neutral", partial: "warning", paid: "success", overdue: "danger" } as const;

/**
 * One invoice with everything done on it (6.2–6.4): its lines, what was received, the installment plan, and
 * the actions — take a payment, print, set installments, credit note, void — each by permission.
 * A payment row opens its receipt, or is refunded with a reason.
 */
export function InvoiceDetail({ invoiceId, perms, calendar, credit, patientName, onClose, onChanged }: { invoiceId: string; perms: BillingPerms; calendar: CalendarSystem; credit: number; patientName?: string; onClose: () => void; onChanged: () => void }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const settings = useClinicSettings();
  const numbering = (settings?.tooth_numbering ?? "fdi") as ToothNumbering;
  const [invoice, setInvoice] = useState<InvoiceInfo | null>(null);
  const [payments, setPayments] = useState<PaymentInfo[]>([]);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<null | "pay" | "installments" | "credit_note" | "void" | "print">(null);
  const [receipt, setReceipt] = useState<PaymentInfo | null>(null);
  const [refunding, setRefunding] = useState<PaymentInfo | null>(null);

  const load = () =>
    Promise.all([rpc("invoices.get", { id: invoiceId }), rpc("payments.list", payQ({ invoice_id: invoiceId, limit: 200 }))])
      .then(([i, p]) => {
        setInvoice(i);
        setPayments(p);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [invoiceId]);
  const changed = () => {
    onChanged();
    return load();
  };
  const money = (v: number) => formatAmount(v, lang);

  if (error && !invoice) return <Dialog title={t("inv.title")} onClose={onClose}><ErrorState message={error} onRetry={load} /></Dialog>;
  if (!invoice) return <Dialog title={t("inv.title")} onClose={onClose}><Loading /></Dialog>;
  const netPaid = invoice.paid - invoice.refunded;
  const live = invoice.status === "issued";
  const refundable = (p: PaymentInfo) => perms.void && p.kind !== "refund" && p.amount - p.refunded > 0;

  return (
    <>
      <Dialog
        title={<span className="row" style={{ gap: "var(--space-3)" }}><bdi className="ltr">{invoice.number}</bdi><PayBadge state={invoice.pay_state} /></span>}
        description={`${patientName ?? invoice.patient_name} · ${formatDate(invoice.issued_at, lang, calendar)}`}
        onClose={onClose}
        wide
        testId="invoice-detail"
        footer={
          <>
            <Button icon={Printer} onClick={() => setDialog("print")} data-testid="invoice-print">{t("inv.print")}</Button>
            {live && perms.edit && invoice.remaining > 0 && <Button icon={CalendarClock} onClick={() => setDialog("installments")} data-testid="invoice-installments">{t("inst.title")}</Button>}
            {live && perms.void && invoice.remaining > 0 && <Button icon={FileMinus2} onClick={() => setDialog("credit_note")} data-testid="invoice-credit-note">{t("inv.creditNote")}</Button>}
            {live && perms.void && netPaid === 0 && <Button variant="danger" icon={Ban} onClick={() => setDialog("void")} data-testid="invoice-void">{t("inv.void")}</Button>}
            {live && perms.edit && invoice.remaining > 0 && <Button variant="primary" icon={Wallet} onClick={() => setDialog("pay")} data-testid="invoice-pay">{t("pay.add")}</Button>}
          </>
        }
      >
        <div className="stack" data-testid="invoice-detail-body">
          {invoice.status === "void" && <div className="notice danger" role="status" data-testid="invoice-void-note"><Ban aria-hidden /><div className="notice-body"><span className="notice-title">{t("inv.voided")}</span><span>{invoice.void_reason}</span></div></div>}
          <div className="table-wrap">
            <table className="table" data-testid="invoice-detail-lines">
              <thead>
                <tr><th>{t("inv.col.service")}</th><th>{t("inv.col.teeth")}</th><th className="num">{t("inv.col.qty")}</th><th className="num">{t("inv.col.unit")}</th><th className="num">{t("inv.col.total")}</th></tr>
              </thead>
              <tbody>
                {invoice.lines.map((l) => (
                  <tr key={l.id}>
                    <td><span className="cell-strong">{tr(l.names, lang)}</span>{l.listed_price > 0 && l.listed_price !== l.unit_price && <span className="t-caption subtle"> · {t("inv.priceWas").replace("{n}", money(l.listed_price))}</span>}</td>
                    <td><bdi>{[teethLabel(l.teeth, numbering, lang), surfacesText(l.surfaces, l.teeth[0])].filter(Boolean).join(" ") || "—"}</bdi></td>
                    <td className="num">{digits(l.quantity, lang)}</td>
                    <td className="num">{money(l.unit_price)}</td>
                    <td className="num">{money(l.line_total)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <dl className="fin-summary" data-testid="invoice-detail-totals">
            <div><dt>{t("inv.subtotal")}</dt><dd className="num">{money(invoice.subtotal)}</dd></div>
            {invoice.discount > 0 && <div><dt>{t("inv.discount")}</dt><dd className="num">− {money(invoice.discount)}</dd></div>}
            {invoice.credit_notes.map((c) => <div key={c.id}><dt>{t("inv.creditNote")} <bdi className="ltr">{c.number}</bdi> <span className="subtle">· {c.reason}</span></dt><dd className="num">− {money(c.amount)}</dd></div>)}
            <div className="strong"><dt>{t("inv.net")}</dt><dd className="num" data-testid="invoice-detail-net">{money(invoice.net)}</dd></div>
            <div><dt>{t("inv.paid")}</dt><dd className="num" data-testid="invoice-detail-paid">{money(netPaid)}</dd></div>
            <div className="strong"><dt>{t("inv.remaining")}</dt><dd className="num" data-testid="invoice-detail-remaining">{money(invoice.remaining)}</dd></div>
          </dl>
          {invoice.note && <p className="subtle">{invoice.note}</p>}

          {invoice.installments.length > 0 && (
            <section className="stack" style={{ gap: "var(--space-2)" }}>
              <h3 className="t-subtitle">{t("inv.installments")}</h3>
              <div className="table-wrap">
                <table className="table" data-testid="invoice-detail-installments">
                  <thead><tr><th>#</th><th>{t("inst.due")}</th><th className="num">{t("inst.amount")}</th><th className="num">{t("inst.paid")}</th><th>{t("inst.state")}</th></tr></thead>
                  <tbody>
                    {invoice.installments.map((i) => (
                      <tr key={i.no}>
                        <td>{digits(i.no, lang)}</td>
                        <td>{formatDate(i.due_date, lang, calendar)}</td>
                        <td className="num">{money(i.amount)}</td>
                        <td className="num">{money(i.paid)}</td>
                        <td><Badge tone={INST_TONE[i.state]} dot>{t(`inst.state.${i.state}`)}</Badge></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          )}

          <section className="stack" style={{ gap: "var(--space-2)" }}>
            <h3 className="t-subtitle">{t("inv.payments")}</h3>
            {payments.length === 0 ? (
              <p className="subtle" data-testid="invoice-no-payments">{t("inv.noPayments")}</p>
            ) : (
              <div className="table-wrap">
                <table className="table" data-testid="invoice-detail-payments">
                  <thead><tr><th>{t("rcpt.number")}</th><th>{t("doc.date")}</th><th>{t("pay.kind")}</th><th>{t("rcpt.method")}</th><th className="num">{t("pay.amount")}</th><th /></tr></thead>
                  <tbody>
                    {payments.map((p) => (
                      <tr key={p.id} data-testid={`payment-row-${p.number}`}>
                        <td><bdi className="ltr num">{p.number}</bdi></td>
                        <td>{formatDateTime(p.paid_at, lang, calendar)}</td>
                        <td>{t(p.kind === "refund" ? "pay.kind.refund" : p.kind === "credit" ? "pay.kind.credit" : "pay.kind.payment")}{p.reason && <span className="t-caption subtle"> · {p.reason}</span>}</td>
                        <td>{p.kind === "credit" ? t("pay.kind.credit") : tr(p.method, lang) || "—"}</td>
                        <td className="num">{p.kind === "refund" ? "− " : ""}{money(p.amount)}{p.refunded > 0 && <span className="t-caption subtle"> ({t("pay.refundedOf").replace("{n}", money(p.refunded))})</span>}</td>
                        <td className="cell-actions">
                          <span className="row">
                            <Button size="sm" variant="link" icon={Receipt} onClick={() => setReceipt(p)} data-testid={`payment-receipt-${p.number}`}>{t("pay.receipt")}</Button>
                            {refundable(p) && <Button size="sm" variant="link" icon={Undo2} onClick={() => setRefunding(p)} data-testid={`payment-refund-${p.number}`}>{t("pay.refund")}</Button>}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        </div>
      </Dialog>

      {dialog === "pay" && (
        <PaymentDialog patientId={invoice.patient_id} patientName={invoice.patient_name} invoices={[invoice]} invoiceId={invoice.id} credit={credit} onClose={() => setDialog(null)} onDone={(p) => { setDialog(null); setReceipt(p); toast.success(t("pay.saved")); changed(); }} />
      )}
      {dialog === "installments" && <InstallmentsDialog invoice={invoice} calendar={calendar} onClose={() => setDialog(null)} onDone={() => { setDialog(null); changed(); }} />}
      {dialog === "print" && <InvoicePrint invoice={invoice} calendar={calendar} onClose={() => setDialog(null)} onChanged={(i) => setInvoice(i)} />}
      {dialog === "void" && (
        <ReasonDialog
          title={t("inv.void")}
          description={t("inv.void.hint")}
          confirm={t("inv.void")}
          danger
          onClose={() => setDialog(null)}
          onConfirm={async (reason) => {
            await rpc("invoices.void", { id: invoice.id, version: invoice.version, reason });
            setDialog(null);
            toast.success(t("inv.voided"));
            changed();
          }}
        />
      )}
      {dialog === "credit_note" && (
        <ReasonDialog
          title={t("inv.creditNote")}
          description={t("inv.creditNote.hint")}
          confirm={t("inv.creditNote.issue")}
          amount={{ max: invoice.remaining, initial: 0 }}
          onClose={() => setDialog(null)}
          onConfirm={async (reason, amount) => {
            await rpc("invoices.credit_note", { invoice_id: invoice.id, amount, reason });
            setDialog(null);
            toast.success(t("inv.creditNote.done"));
            changed();
          }}
        />
      )}
      {refunding && (
        <ReasonDialog
          title={t("pay.refund")}
          description={`${refunding.number} · ${t("pay.refund.hint")}`}
          confirm={t("pay.refund.confirm")}
          danger
          amount={{ max: refunding.amount - refunding.refunded, initial: refunding.amount - refunding.refunded }}
          creditOption={refunding.kind === "payment" && !!refunding.invoice_id}
          testId="refund-dialog"
          onClose={() => setRefunding(null)}
          onConfirm={async (reason, amount, toCredit) => {
            const rv = await rpc("payments.refund", { payment_id: refunding.id, amount, reason, to_credit: toCredit });
            setRefunding(null);
            toast.success(t("pay.refund.done"));
            setReceipt(rv);
            changed();
          }}
        />
      )}
      {receipt && <ReceiptHost payment={receipt} calendar={calendar} onClose={() => setReceipt(null)} onChanged={(p) => setPayments((list) => list.map((x) => (x.id === p.id ? p : x)))} />}
    </>
  );
}
