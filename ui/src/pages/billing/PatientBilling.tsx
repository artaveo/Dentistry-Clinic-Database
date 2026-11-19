// The patient's «فاکتورها» and «پرداخت‌ها» tabs: the account at a glance (invoiced · reduced · paid · owed ·
// deposit), invoices from the treatments done, every receipt, refund and deposit move with its paper, and the
// statement with a running balance.
import { useEffect, useState } from "react";
import { FilePlus2, FileText, Receipt, ReceiptText, Undo2, Wallet } from "lucide-react";
import type { CalendarSystem, InvoiceInfo, PatientAccount, PatientInfo, ReceiptInfo, StatementInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { invQ, rcQ } from "../../lib/billing";
import { todayIso } from "../../lib/calendar";
import { formatDate, formatDateTime } from "../../lib/dates";
import { formatAmount } from "../../lib/money";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { useToast } from "../../ui/Toast";
import { DocumentPreview } from "../../print/DocumentPreview";
import { StatementSheet } from "../../print/finance";
import { useLetterhead } from "../../print/letterhead";
import { PayBadge, ReceiptHost, StatusBadge } from "./common";
import type { BillingPerms } from "./common";
import { InvoiceDialog, ReceiptDialog, RefundDialog } from "./dialogs";
import { InvoiceDetail } from "./InvoiceDetail";

/** Invoiced · reductions · paid · owed · deposit, and the balance. */
export function AccountStrip({ account }: { account: PatientAccount | null }) {
  const { t, lang } = useI18n();
  const cell = (k: string, v: number | undefined, tone?: "danger" | "success", testId?: string) => (
    <div className={`acct-cell ${tone ?? ""}`}>
      <span className="acct-label">{t(k)}</span>
      <span className="acct-value num" data-testid={testId}>{v === undefined ? "…" : formatAmount(v, lang)}</span>
    </div>
  );
  return (
    <div className="glass acct" data-testid="account-strip">
      {cell("acct.total", account?.invoiced, undefined, "acct-total")}
      {cell("acct.discount", account ? account.reductions + account.written_off : undefined, undefined, "acct-discount")}
      {cell("acct.paid", account?.allocated, "success", "acct-paid")}
      {cell("acct.remaining", account?.owed, account && account.owed > 0 ? "danger" : undefined, "acct-remaining")}
      {account && account.deposit > 0 && cell("acct.deposit", account.deposit, "success", "acct-credit")}
      {account && account.overdue > 0 && cell("acct.overdue", account.overdue, "danger", "acct-overdue")}
    </div>
  );
}

function useAccount(patientId: string, key: number) {
  const { err } = useI18n();
  const [account, setAccount] = useState<PatientAccount | null>(null);
  const [invoices, setInvoices] = useState<InvoiceInfo[] | null>(null);
  const [error, setError] = useState("");
  const load = () =>
    Promise.all([rpc("fin.patients.account", { patient_id: patientId }), rpc("fin.invoices.list", invQ({ patient_id: patientId, limit: 200 }))])
      .then(([a, i]) => {
        setAccount(a);
        setInvoices(i);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patientId, key]);
  return { account, invoices, error, load };
}

export function PatientInvoicesTab({ patient, perms, canOverride, clinic, onChanged }: { patient: PatientInfo; perms: BillingPerms; canOverride: boolean; clinic: { calendar_system: CalendarSystem } | null; onChanged?: () => void }) {
  const { t, lang } = useI18n();
  const toast = useToast();
  const calendar = clinic?.calendar_system ?? "shamsi";
  const [key, setKey] = useState(0);
  const { account, invoices, error, load } = useAccount(patient.id, key);
  const [creating, setCreating] = useState(false);
  const [paying, setPaying] = useState(false);
  const [openId, setOpenId] = useState<number | null>(null);
  const [receipt, setReceipt] = useState<ReceiptInfo | null>(null);
  const [statement, setStatement] = useState(false);
  const refresh = () => {
    setKey((k) => k + 1);
    onChanged?.();
  };
  if (error && !invoices) return <ErrorState message={error} onRetry={load} />;
  const open = (invoices ?? []).filter((i) => i.status === "posted" && i.remaining > 0);
  return (
    <div className="stack" data-testid="invoices-tab">
      <AccountStrip account={account} />
      {account && account.billable_count > 0 && perms.edit && (
        <Notice tone="info" testId="unbilled-notice">
          {t("inv.unbilled").replace("{n}", String(account.billable_count)).replace("{total}", formatAmount(account.billable_total, lang))}{" "}
          <Button size="sm" variant="link" onClick={() => setCreating(true)} data-testid="unbilled-create">{t("inv.new")}</Button>
        </Notice>
      )}
      <Card flush>
        <CardHeader
          icon={ReceiptText}
          title={t("patients.tab.invoices")}
          description={t("inv.subtitle")}
          actions={
            <>
              <Button icon={FileText} onClick={() => setStatement(true)} data-testid="statement-open">{t("stmt.title")}</Button>
              {perms.edit && <Button icon={Wallet} onClick={() => setPaying(true)} data-testid="pay-open">{t("pay.add")}</Button>}
              {perms.edit && <Button variant="primary" icon={FilePlus2} onClick={() => setCreating(true)} data-testid="invoice-new">{t("inv.new")}</Button>}
            </>
          }
        />
        <div className="table-wrap">
          <table className="table" data-testid="invoices-table">
            <thead>
              <tr><th>{t("rcpt.number")}</th><th>{t("doc.date")}</th><th className="num">{t("inv.net")}</th><th className="num">{t("inv.paid")}</th><th className="num">{t("inv.remaining")}</th><th>{t("inst.state")}</th></tr>
            </thead>
            <tbody>
              {!invoices && <SkeletonRows cols={6} />}
              {invoices?.map((i) => (
                <tr key={i.id} className="row-click" onClick={() => setOpenId(i.id)} data-testid={`invoice-row-${i.number}`}>
                  <td><button type="button" className="link-btn" onClick={(e) => { e.stopPropagation(); setOpenId(i.id); }}><bdi className="ltr num">{i.number}</bdi></button></td>
                  <td>{formatDate(i.date, lang, calendar)}</td>
                  <td className="num">{formatAmount(i.net, lang)}</td>
                  <td className="num">{formatAmount(i.allocated, lang)}</td>
                  <td className="num cell-strong">{formatAmount(i.remaining, lang)}</td>
                  <td><PayBadge state={i.pay_state} /></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {invoices && !invoices.length && (
          <EmptyState icon={ReceiptText} title={t("inv.none")} action={perms.edit ? <Button variant="primary" icon={FilePlus2} onClick={() => setCreating(true)}>{t("inv.new")}</Button> : undefined}>{t("inv.noneHint")}</EmptyState>
        )}
      </Card>

      {creating && (
        <InvoiceDialog
          patientId={patient.id}
          patientName={patient.full_name}
          perms={{ override: canOverride }}
          deposit={account?.deposit ?? 0}
          onClose={() => setCreating(false)}
          onCreated={(inv) => {
            setCreating(false);
            toast.success(t("inv.created").replace("{n}", inv.number));
            refresh();
            setOpenId(inv.id);
          }}
        />
      )}
      {paying && (
        <ReceiptDialog
          patientId={patient.id}
          patientName={patient.full_name}
          invoices={open}
          onClose={() => setPaying(false)}
          onDone={(r) => {
            setPaying(false);
            toast.success(t("pay.saved"));
            refresh();
            setReceipt(r);
          }}
        />
      )}
      {openId !== null && <InvoiceDetail invoiceId={openId} perms={perms} calendar={calendar} deposit={account?.deposit ?? 0} patientName={patient.full_name} onClose={() => setOpenId(null)} onChanged={refresh} />}
      {receipt && <ReceiptHost receipt={receipt} calendar={calendar} onClose={() => setReceipt(null)} />}
      {statement && <StatementPreview patient={patient} calendar={calendar} onClose={() => setStatement(false)} />}
    </div>
  );
}

export function PatientPaymentsTab({ patient, perms, clinic, onChanged }: { patient: PatientInfo; perms: BillingPerms; clinic: { calendar_system: CalendarSystem } | null; onChanged?: () => void }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const calendar = clinic?.calendar_system ?? "shamsi";
  const [key, setKey] = useState(0);
  const { account, invoices, load } = useAccount(patient.id, key);
  const [docs, setDocs] = useState<ReceiptInfo[] | null>(null);
  const [error, setError] = useState("");
  const [paying, setPaying] = useState(false);
  const [refunding, setRefunding] = useState(false);
  const [receipt, setReceipt] = useState<ReceiptInfo | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);
  useEffect(() => {
    rpc("fin.receipts.list", rcQ({ patient_id: patient.id, limit: 300 }))
      .then((p) => {
        setDocs(p);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [patient.id, key]);
  const refresh = () => {
    setKey((k) => k + 1);
    onChanged?.();
  };
  if (error && !docs) return <ErrorState message={error} onRetry={() => setKey((k) => k + 1)} />;
  const open = (invoices ?? []).filter((i) => i.status === "posted" && i.remaining > 0);
  const deposit = account?.deposit ?? 0;
  return (
    <div className="stack" data-testid="payments-tab">
      <AccountStrip account={account} />
      <Card flush>
        <CardHeader
          icon={Wallet}
          title={t("patients.tab.payments")}
          description={t("pay.subtitle")}
          actions={
            <>
              {perms.void && deposit > 0 && <Button icon={Undo2} onClick={() => setRefunding(true)} data-testid="refund-open">{t("pay.refund")}</Button>}
              {perms.edit && <Button variant="primary" icon={Wallet} onClick={() => setPaying(true)} data-testid="pay-open">{t("pay.add")}</Button>}
            </>
          }
        />
        <div className="table-wrap">
          <table className="table" data-testid="payments-table">
            <thead>
              <tr><th>{t("rcpt.number")}</th><th>{t("doc.date")}</th><th>{t("pay.kind")}</th><th>{t("rcpt.invoice")}</th><th className="num">{t("pay.amount")}</th><th className="num">{t("rcpt.toDeposit")}</th><th /></tr>
            </thead>
            <tbody>
              {!docs && <SkeletonRows cols={7} />}
              {docs?.map((p) => (
                <tr key={p.id} data-testid={`payment-row-${p.number}`}>
                  <td><bdi className="ltr num">{p.number}</bdi> <StatusBadge status={p.status} /></td>
                  <td>{formatDateTime(p.created_at, lang, calendar)}</td>
                  <td>{t(`fin.doc.${p.kind}`)}{p.reason && <span className="t-caption subtle"> · {p.reason}</span>}</td>
                  <td>
                    {p.allocations.length
                      ? p.allocations.map((a) => (
                          <button key={a.invoice_id} type="button" className="link-btn" onClick={() => setOpenId(a.invoice_id)}><bdi className="ltr num">{a.invoice_number}</bdi></button>
                        ))
                      : "—"}
                  </td>
                  <td className="num cell-strong">{p.kind === "rf" ? "− " : ""}{formatAmount(Math.abs(p.amount), lang)}</td>
                  <td className="num">{p.to_deposit ? `${p.to_deposit < 0 ? "− " : "+ "}${formatAmount(Math.abs(p.to_deposit), lang)}` : "—"}</td>
                  <td className="cell-actions"><Button size="sm" variant="link" icon={Receipt} onClick={() => setReceipt(p)} data-testid={`payment-receipt-${p.number}`}>{t("pay.receipt")}</Button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {docs && !docs.length && <EmptyState icon={Wallet} title={t("pay.none")}>{t("pay.noneHint")}</EmptyState>}
      </Card>
      {paying && (
        <ReceiptDialog
          patientId={patient.id}
          patientName={patient.full_name}
          invoices={open}
          onClose={() => setPaying(false)}
          onDone={(r) => {
            setPaying(false);
            toast.success(t("pay.saved"));
            refresh();
            setReceipt(r);
          }}
        />
      )}
      {refunding && (
        <RefundDialog
          patientId={patient.id}
          patientName={patient.full_name}
          deposit={deposit}
          onClose={() => setRefunding(false)}
          onDone={(r) => {
            setRefunding(false);
            toast.success(t("pay.refund.done"));
            refresh();
            setReceipt(r);
          }}
        />
      )}
      {openId !== null && <InvoiceDetail invoiceId={openId} perms={perms} calendar={calendar} deposit={deposit} patientName={patient.full_name} onClose={() => setOpenId(null)} onChanged={() => { refresh(); load(); }} />}
      {receipt && <ReceiptHost receipt={receipt} calendar={calendar} onClose={() => setReceipt(null)} onChanged={(r) => setDocs((list) => (list ?? []).map((x) => (x.id === r.id ? r : x)))} />}
    </div>
  );
}

/** The patient's statement: every invoice, receipt, refund and reduction with a running balance, as an A4 page. */
export function StatementPreview({ patient, calendar, onClose }: { patient: PatientInfo; calendar: CalendarSystem; onClose: () => void }) {
  const { t, lang, err } = useI18n();
  const letterhead = useLetterhead();
  const [statement, setStatement] = useState<StatementInfo | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    rpc("fin.patients.statement", { patient_id: patient.id, from: null, to: null }).then(setStatement).catch((e) => setError(err(e)));
  }, [patient.id]);
  if (error) return <Notice tone="danger">{error}</Notice>;
  if (!statement || !letterhead) return null;
  return (
    <DocumentPreview
      title={t("stmt.title")}
      paper="a4"
      fileStem={`ST-${patient.patient_number}-${todayIso()}`}
      testId="statement-preview"
      onClose={onClose}
      sheet={<StatementSheet statement={statement} letterhead={letterhead} lang={lang} calendar={calendar} />}
    />
  );
}
