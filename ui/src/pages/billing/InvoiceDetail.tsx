import { useEffect, useState } from "react";
import { Ban, CalendarClock, FileMinus2, HandCoins, Printer, Receipt, Undo2, Wallet } from "lucide-react";
import type { CalendarSystem, InvoiceInfo, ReceiptInfo, ToothNumbering } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { useClinicSettings } from "../../lib/clinical";
import { digits, formatDate } from "../../lib/dates";
import { formatAmount } from "../../lib/money";
import { teethLabel } from "../../chart/numbering";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Badge, ErrorState, Loading } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { InvoicePrint, PayBadge, ReceiptHost, StatusBadge } from "./common";
import type { BillingPerms } from "./common";
import { InstallmentsDialog, ReasonDialog, ReceiptDialog, RefundDialog, UseDepositDialog } from "./dialogs";

const INST_TONE = { pending: "neutral", partial: "warning", paid: "success", overdue: "danger" } as const;

/**
 * One invoice with everything done on it: its lines, the money put towards it, price reductions and
 * write-offs, the installment plan, and the actions — take money, use the deposit, print, plan, reduce or
 * write off, refund, void — each by permission and each with its reason where it undoes something.
 */
export function InvoiceDetail({ invoiceId, perms, calendar, deposit = 0, patientName, onClose, onChanged }: { invoiceId: number; perms: BillingPerms; calendar: CalendarSystem; deposit?: number; patientName?: string; onClose: () => void; onChanged: () => void }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const settings = useClinicSettings();
  const numbering = (settings?.tooth_numbering ?? "fdi") as ToothNumbering;
  const [invoice, setInvoice] = useState<InvoiceInfo | null>(null);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<null | "pay" | "deposit" | "installments" | "adjust" | "void" | "refund" | "print">(null);
  const [receipt, setReceipt] = useState<ReceiptInfo | null>(null);

  const load = () =>
    rpc("fin.invoices.get", { id: invoiceId })
      .then((i) => {
        setInvoice(i);
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
  const openReceipt = (id: number) =>
    rpc("fin.receipts.get", { id })
      .then(setReceipt)
      .catch((e) => toast.error(err(e)));

  if (error && !invoice) return <Dialog title={t("inv.title")} onClose={onClose}><ErrorState message={error} onRetry={load} /></Dialog>;
  if (!invoice) return <Dialog title={t("inv.title")} onClose={onClose}><Loading /></Dialog>;
  const live = invoice.status === "posted";
  const plan = invoice.plan?.installments ?? [];

  return (
    <>
      <Dialog
        title={<span className="row" style={{ gap: "var(--space-3)" }}><bdi className="ltr">{invoice.number}</bdi><PayBadge state={invoice.pay_state} /></span>}
        description={`${patientName ?? invoice.patient_name} · ${formatDate(invoice.date, lang, calendar)}`}
        onClose={onClose}
        wide
        testId="invoice-detail"
        footer={
          <>
            <Button icon={Printer} onClick={() => setDialog("print")} data-testid="invoice-print">{t("inv.print")}</Button>
            {live && perms.edit && invoice.remaining > 0 && <Button icon={CalendarClock} onClick={() => setDialog("installments")} data-testid="invoice-installments">{t("inst.title")}</Button>}
            {live && perms.void && invoice.remaining > 0 && <Button icon={FileMinus2} onClick={() => setDialog("adjust")} data-testid="invoice-credit-note">{t("inv.adjust")}</Button>}
            {live && perms.void && invoice.allocated > 0 && <Button icon={Undo2} onClick={() => setDialog("refund")} data-testid="invoice-refund">{t("pay.refund")}</Button>}
            {live && perms.void && invoice.allocated === 0 && invoice.reductions === 0 && invoice.written_off === 0 && <Button variant="danger" icon={Ban} onClick={() => setDialog("void")} data-testid="invoice-void">{t("inv.void")}</Button>}
            {live && perms.edit && invoice.remaining > 0 && deposit > 0 && <Button icon={HandCoins} onClick={() => setDialog("deposit")} data-testid="invoice-use-deposit">{t("pay.useDeposit")}</Button>}
            {live && perms.edit && invoice.remaining > 0 && <Button variant="primary" icon={Wallet} onClick={() => setDialog("pay")} data-testid="invoice-pay">{t("pay.add")}</Button>}
          </>
        }
      >
        <div className="stack" data-testid="invoice-detail-body">
          {invoice.status === "void" && <div className="notice danger" role="status" data-testid="invoice-void-note"><Ban aria-hidden /><div className="notice-body"><span className="notice-title">{t("inv.voided")}</span><span>{invoice.void_reason}</span></div></div>}
          <div className="table-wrap">
            <table className="table" data-testid="invoice-detail-lines">
              <thead>
                <tr><th>{t("inv.col.service")}</th><th>{t("inv.col.teeth")}</th><th>{t("appt.doctor")}</th><th className="num">{t("inv.col.qty")}</th><th className="num">{t("inv.col.unit")}</th><th className="num">{t("inv.col.discount")}</th><th className="num">{t("inv.col.total")}</th></tr>
              </thead>
              <tbody>
                {invoice.lines.map((l) => (
                  <tr key={l.no}>
                    <td><span className="cell-strong">{l.name}</span>{l.listed_price > 0 && l.listed_price !== l.unit_price && <span className="t-caption subtle"> · {t("inv.priceWas").replace("{n}", money(l.listed_price))}</span>}</td>
                    <td><bdi>{teethLabel(l.teeth, numbering, lang) || "—"}</bdi></td>
                    <td>{l.doctor_name ?? "—"}</td>
                    <td className="num">{digits(l.quantity, lang)}</td>
                    <td className="num">{money(l.unit_price)}</td>
                    <td className="num">{l.discount ? money(l.discount) : "—"}</td>
                    <td className="num">{money(l.net)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <dl className="fin-summary" data-testid="invoice-detail-totals">
            <div><dt>{t("inv.subtotal")}</dt><dd className="num">{money(invoice.gross)}</dd></div>
            {invoice.discount > 0 && <div><dt>{t("inv.discount")}</dt><dd className="num">− {money(invoice.discount)}</dd></div>}
            <div className="strong"><dt>{t("inv.net")}</dt><dd className="num" data-testid="invoice-detail-net">{money(invoice.net)}</dd></div>
            {invoice.adjustments.filter((a) => a.status === "posted").map((a) => (
              <div key={a.id}><dt>{t(a.kind === "write_off" ? "inv.writeOff" : "inv.reduction")} <bdi className="ltr">{a.number}</bdi> <span className="subtle">· {a.reason}</span></dt><dd className="num">− {money(a.amount)}</dd></div>
            ))}
            <div><dt>{t("inv.paid")}</dt><dd className="num" data-testid="invoice-detail-paid">{money(invoice.allocated)}</dd></div>
            <div className="strong"><dt>{t("inv.remaining")}</dt><dd className="num" data-testid="invoice-detail-remaining">{money(invoice.remaining)}</dd></div>
          </dl>
          {invoice.note && <p className="subtle">{invoice.note}</p>}

          {plan.length > 0 && (
            <section className="stack" style={{ gap: "var(--space-2)" }}>
              <h3 className="t-subtitle">{t("inv.installments")} {invoice.plan && invoice.plan.version > 1 && <span className="subtle t-caption">· {t("inst.version").replace("{n}", digits(invoice.plan.version, lang))}</span>}</h3>
              <div className="table-wrap">
                <table className="table" data-testid="invoice-detail-installments">
                  <thead><tr><th>#</th><th>{t("inst.due")}</th><th className="num">{t("inst.amount")}</th><th className="num">{t("inst.paid")}</th><th>{t("inst.state")}</th></tr></thead>
                  <tbody>
                    {plan.map((i) => (
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
            {invoice.allocations.length === 0 ? (
              <p className="subtle" data-testid="invoice-no-payments">{t("inv.noPayments")}</p>
            ) : (
              <div className="table-wrap">
                <table className="table" data-testid="invoice-detail-payments">
                  <thead><tr><th>{t("rcpt.number")}</th><th>{t("doc.date")}</th><th>{t("pay.kind")}</th><th className="num">{t("pay.amount")}</th><th /></tr></thead>
                  <tbody>
                    {invoice.allocations.map((p) => (
                      <tr key={`${p.doc_id}-${p.amount}`} data-testid={`payment-row-${p.number}`}>
                        <td><bdi className="ltr num">{p.number}</bdi></td>
                        <td>{formatDate(p.date, lang, calendar)}</td>
                        <td>{t(p.amount < 0 ? "fin.doc.back" : `fin.doc.${p.kind}`)}</td>
                        <td className="num">{p.amount < 0 ? "− " : ""}{money(Math.abs(p.amount))}</td>
                        <td className="cell-actions"><Button size="sm" variant="link" icon={Receipt} onClick={() => openReceipt(p.doc_id)} data-testid={`payment-receipt-${p.number}`}>{t("pay.receipt")}</Button></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
          {invoice.adjustments.some((a) => a.status !== "posted") && (
            <p className="subtle t-caption">{invoice.adjustments.filter((a) => a.status !== "posted").map((a) => <span key={a.id}><bdi className="ltr">{a.number}</bdi> <StatusBadge status={a.status} /> </span>)}</p>
          )}
        </div>
      </Dialog>

      {dialog === "pay" && (
        <ReceiptDialog patientId={invoice.patient_id} patientName={invoice.patient_name} invoices={[invoice]} invoiceId={invoice.id} onClose={() => setDialog(null)} onDone={(r) => { setDialog(null); setReceipt(r); toast.success(t("pay.saved")); changed(); }} />
      )}
      {dialog === "deposit" && (
        <UseDepositDialog patientId={invoice.patient_id} invoice={invoice} deposit={deposit} onClose={() => setDialog(null)} onDone={() => { setDialog(null); toast.success(t("pay.saved")); changed(); }} />
      )}
      {dialog === "refund" && (
        <RefundDialog patientId={invoice.patient_id} patientName={invoice.patient_name} deposit={deposit} invoice={invoice} onClose={() => setDialog(null)} onDone={(r) => { setDialog(null); setReceipt(r); toast.success(t("pay.refund.done")); changed(); }} />
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
            await rpc("fin.docs.void", { id: invoice.id, reason });
            setDialog(null);
            toast.success(t("inv.voided"));
            changed();
          }}
        />
      )}
      {dialog === "adjust" && (
        <ReasonDialog
          title={t("inv.adjust")}
          description={t("inv.adjust.hint")}
          confirm={t("inv.adjust.issue")}
          amount={{ max: invoice.remaining, initial: 0 }}
          kinds={[{ value: "reduction", label: t("inv.reduction") }, { value: "write_off", label: t("inv.writeOff") }]}
          onClose={() => setDialog(null)}
          onConfirm={async (reason, amount, kind) => {
            await rpc("fin.invoices.adjust", { invoice_id: invoice.id, kind: kind === "write_off" ? "write_off" : "reduction", amount, reason });
            setDialog(null);
            toast.success(t("inv.adjust.done"));
            changed();
          }}
        />
      )}
      {receipt && <ReceiptHost receipt={receipt} calendar={calendar} onClose={() => setReceipt(null)} />}
    </>
  );
}
