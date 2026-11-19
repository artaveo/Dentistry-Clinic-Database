// «مالی» — one finance area (docs/design/finance-standard.md §ب۸): the overview that proves the books agree,
// income (receipts, invoices, installments, who owes what by age, deposits), expenses and suppliers, cash and
// bank, doctor shares, and the settings. What a person sees follows their permissions.
import { useEffect, useMemo, useState } from "react";
import { ArrowLeftRight, Banknote, BellRing, CalendarClock, CheckCircle2, Coins, Gauge, Landmark, LockKeyhole, Percent, PiggyBank, Printer, Receipt, ReceiptText, Search, Settings2, TriangleAlert, UsersRound, Wallet } from "lucide-react";
import type { AgingRow, CashDayInfo, ClinicProfile, FinanceOverview, InstallmentDue, InvoiceInfo, ReceiptInfo, VoucherInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { accountNameOf, afnInput, amountRule, invQ, invalidateAccounts, moneyAccounts, pvQ, rcQ, useAccounts } from "../../lib/billing";
import { addDays, monthRange, todayIso } from "../../lib/calendar";
import { digits, formatDate, formatDateTime } from "../../lib/dates";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader, Page, PageHeader, Stat } from "../../ui/Card";
import { Checkbox, Segmented } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { DocumentPreview } from "../../print/DocumentPreview";
import { CashDaySheet, StatementSheet } from "../../print/finance";
import { useLetterhead } from "../../print/letterhead";
import { AmountInput, MoneyAccountSelect, Money, PayBadge, ReceiptHost, StatusBadge, minorOf } from "./common";
import type { BillingPerms } from "./common";
import { ReceiptDialog } from "./dialogs";
import { BillingSettingsTab, CommissionTab, ExpensesTab, OtherReceiptDialog } from "./BillingExtras";
import { InvoiceDetail } from "./InvoiceDetail";
import { StaffTab } from "./Staff";

type TabId = "overview" | "income" | "expenses" | "cash" | "staff" | "commission" | "settings";

const TAB_ICONS = { overview: Gauge, income: ReceiptText, expenses: Wallet, cash: Banknote, staff: UsersRound, commission: Percent, settings: Settings2 };

export function BillingPage({ perms, clinic }: { perms: BillingPerms; clinic: ClinicProfile | null }) {
  const { t } = useI18n();
  const tabs = useMemo<TabId[]>(
    () => [
      ...(perms.reports ? (["overview"] as TabId[]) : []),
      ...(perms.view ? (["income"] as TabId[]) : []),
      ...(perms.reports ? (["expenses"] as TabId[]) : []),
      ...(perms.view ? (["cash"] as TabId[]) : []),
      ...(perms.payroll ? (["staff"] as TabId[]) : []),
      ...(perms.reports || perms.payroll ? (["commission"] as TabId[]) : []),
      ...(perms.settings ? (["settings"] as TabId[]) : []),
    ],
    [perms],
  );
  const [tab, setTab] = useState<TabId>(tabs[0] ?? "income");
  const calendar = clinic?.calendar_system ?? "shamsi";
  return (
    <Page testId="page-billing">
      <PageHeader title={t("nav.billing")} description={t("billing.subtitle")} />
      <div className="profile-tabs" role="tablist">
        {tabs.map((id) => {
          const Icon = TAB_ICONS[id];
          return (
            <button key={id} type="button" role="tab" aria-selected={tab === id} className="profile-tab" onClick={() => setTab(id)} data-testid={`billing-tab-${id}`}>
              <Icon aria-hidden />
              <span>{t(`billing.tab.${id}`)}</span>
            </button>
          );
        })}
      </div>
      {tab === "overview" && <OverviewTab calendar={calendar} />}
      {tab === "income" && <IncomeTab perms={perms} calendar={calendar} />}
      {tab === "expenses" && <ExpensesTab perms={perms} calendar={calendar} />}
      {tab === "cash" && <CashTab perms={perms} calendar={calendar} />}
      {tab === "staff" && <StaffTab perms={perms} calendar={calendar} />}
      {tab === "commission" && <CommissionTab perms={perms} calendar={calendar} />}
      {tab === "settings" && <BillingSettingsTab />}
    </Page>
  );
}

type Cal = ClinicProfile["calendar_system"];

/** A from–to pair of days in the clinic calendar (this month by default). */
function usePeriod(calendar: Cal) {
  const [from, setFrom] = useState(monthRange(todayIso(), calendar).from);
  const [to, setTo] = useState(todayIso());
  return { from, to, setFrom, setTo, ok: from <= to };
}

function PeriodFields({ p, calendar, testId }: { p: ReturnType<typeof usePeriod>; calendar: Cal; testId: string }) {
  const { t } = useI18n();
  return (
    <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
      <Field label={t("period.from")}><DateField value={p.from} onChange={p.setFrom} calendar={calendar} label={t("period.from")} testId={`${testId}-from`} yearsBefore={3} yearsAfter={0} /></Field>
      <Field label={t("period.to")}><DateField value={p.to} onChange={p.setTo} calendar={calendar} label={t("period.to")} testId={`${testId}-to`} yearsBefore={3} yearsAfter={0} /></Field>
      {!p.ok && <Notice tone="warning">{t("rule.period_range")}</Notice>}
    </div>
  );
}

// ───────────────────────────── overview and reconciliation ─────────────────────────────

function OverviewTab({ calendar }: { calendar: Cal }) {
  const { t, lang, err } = useI18n();
  const accounts = useAccounts();
  const p = usePeriod(calendar);
  const [o, setO] = useState<FinanceOverview | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (!p.ok) return;
    rpc("fin.overview", { from: p.from, to: p.to })
      .then((v) => {
        setO(v);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [p.from, p.to, tick]);
  if (error && !o) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const money = (v: number) => formatAmount(v, lang);
  const allOk = o?.checks.every((c) => c.ok) ?? true;
  const row = (label: string, value: number | undefined, opts: { sign?: "-" | "+"; strong?: boolean; testId?: string; muted?: boolean } = {}) => (
    <tr className={opts.strong ? "recon-total" : opts.muted ? "recon-muted" : undefined}>
      <td>{opts.sign === "-" ? "− " : opts.sign === "+" ? "+ " : ""}{label}</td>
      <td className="num" data-testid={opts.testId}>{value === undefined ? "…" : money(value)}</td>
    </tr>
  );
  return (
    <div className="stack" data-testid="billing-overview">
      <PeriodFields p={p} calendar={calendar} testId="overview" />
      <div className="stat-grid">
        <Stat icon={Banknote} label={t("ov.netReceived")} value={o ? money(o.cash.net_received) : "…"} tone="success" sub={t("ov.cashBasis")} testId="ov-net-received" />
        <Stat icon={ReceiptText} label={t("ov.revenue")} value={o ? money(o.accrual.revenue) : "…"} tone="info" sub={t("ov.accrualBasis")} testId="ov-revenue" />
        <Stat icon={UsersRound} label={t("ov.receivables")} value={o ? money(o.receivables) : "…"} tone={o && o.overdue_installments > 0 ? "warning" : "accent"} sub={o && o.overdue_installments > 0 ? `${t("ov.overdue")}: ${money(o.overdue_installments)}` : undefined} testId="ov-receivables" />
        <Stat icon={Landmark} label={t("ov.payables")} value={o ? money(o.payables) : "…"} tone="warning" sub={o && o.tax_withheld ? `${t("ov.taxWithheld")}: ${money(o.tax_withheld)}` : undefined} testId="ov-payables" />
      </div>
      <Notice tone={allOk ? "success" : "danger"} title={allOk ? t("ov.checks.ok") : t("ov.checks.bad")} testId="ov-checks">
        {o?.checks.map((c) => (
          <span key={c.key} className="row" style={{ gap: "var(--space-2)", display: "inline-flex", marginInlineEnd: "var(--space-4)" }}>
            {c.ok ? <CheckCircle2 aria-hidden size={14} /> : <TriangleAlert aria-hidden size={14} />}
            {t(`ov.check.${c.key}`)}{!c.ok && ` (${money(c.left)} ≠ ${money(c.right)})`}
          </span>
        ))}
      </Notice>
      <div className="grid-2">
        <Card flush>
          <CardHeader icon={Coins} title={t("ov.cashBasis")} description={t("ov.cashBasis.hint")} />
          <table className="table recon" data-testid="recon-cash">
            <tbody>
              {row(t("ov.received"), o?.cash.received)}
              {row(t("ov.refunded"), o?.cash.refunded, { sign: "-" })}
              {row(t("ov.netReceived"), o?.cash.net_received, { strong: true })}
              {o?.cash.expenses.map((e) => <tr key={e.account_id}><td>− {accountNameOf(e.account_id, accounts, t)}</td><td className="num">{money(e.amount)}</td></tr>)}
              {o && o.cash.suppliers_paid !== 0 && row(t("ov.suppliersPaid"), o.cash.suppliers_paid, { sign: "-" })}
              {o && o.cash.staff_paid !== 0 && row(t("ov.staffPaid"), o.cash.staff_paid, { sign: "-" })}
              {row(t("ov.operating"), o?.cash.operating, { strong: true, testId: "recon-operating" })}
              {o && o.cash.withheld !== 0 && row(t("ov.withheld"), o.cash.withheld, { sign: "+", muted: true })}
              {o && o.cash.tax_paid !== 0 && row(t("ov.taxPaid"), o.cash.tax_paid, { sign: "-", muted: true })}
              {o && o.cash.owner_in !== 0 && row(t("ov.ownerIn"), o.cash.owner_in, { sign: "+", muted: true })}
              {o && o.cash.owner_out !== 0 && row(t("ov.ownerOut"), o.cash.owner_out, { sign: "-", muted: true })}
              {o && o.cash.loans_in !== 0 && row(t("ov.loansIn"), o.cash.loans_in, { sign: "+", muted: true })}
              {o && o.cash.loans_out !== 0 && row(t("ov.loansOut"), o.cash.loans_out, { sign: "-", muted: true })}
              {o && o.cash.other_income !== 0 && row(t("ov.otherIncome"), o.cash.other_income, { sign: "+", muted: true })}
              {o && o.cash.count_differences !== 0 && row(t("ov.countDifferences"), o.cash.count_differences, { muted: true })}
              {row(t("ov.moneyChange"), o ? o.cash.money_closing - o.cash.money_opening : undefined, { strong: true, testId: "recon-money-change" })}
            </tbody>
          </table>
        </Card>
        <Card flush>
          <CardHeader icon={ReceiptText} title={t("ov.accrualBasis")} description={t("ov.accrualBasis.hint")} />
          <table className="table recon" data-testid="recon-accrual">
            <tbody>
              {row(t("ov.gross"), o?.accrual.gross)}
              {row(t("ov.discounts"), o?.accrual.discounts, { sign: "-" })}
              {row(t("ov.reductions"), o?.accrual.reductions, { sign: "-" })}
              {o && o.accrual.other_income !== 0 && row(t("ov.otherIncome"), o.accrual.other_income, { sign: "+" })}
              {row(t("ov.revenue"), o?.accrual.revenue, { strong: true })}
              {o?.accrual.expenses.map((e) => <tr key={e.account_id}><td>− {accountNameOf(e.account_id, accounts, t)}</td><td className="num">{money(e.amount)}</td></tr>)}
              {o && o.accrual.write_offs !== 0 && row(t("ov.writeOffs"), o.accrual.write_offs, { sign: "-" })}
              {row(t("ov.result"), o?.accrual.result, { strong: true, testId: "recon-result" })}
            </tbody>
          </table>
        </Card>
      </div>
      <Card flush>
        <CardHeader icon={PiggyBank} title={t("ov.money")} />
        <div className="table-wrap">
          <table className="table" data-testid="recon-money">
            <thead><tr><th>{t("fin.account")}</th><th className="num">{t("cash.opening")}</th><th className="num">{t("ov.in")}</th><th className="num">{t("ov.out")}</th><th className="num">{t("cash.closing")}</th></tr></thead>
            <tbody>
              {o?.cash.money.map((m) => (
                <tr key={m.account_id}><td>{accountNameOf(m.account_id, accounts, t)}</td><td className="num">{money(m.opening)}</td><td className="num">{money(m.inflow)}</td><td className="num">{money(m.outflow)}</td><td className="num cell-strong">{money(m.closing)}</td></tr>
              ))}
              {o && (
                <tr className="recon-total"><td>{t("ov.total")}</td><td className="num">{money(o.cash.money_opening)}</td><td /><td /><td className="num">{money(o.cash.money_closing)}</td></tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>
    </div>
  );
}

// ───────────────────────────── income ─────────────────────────────

type IncomeSub = "receipts" | "invoices" | "installments" | "aging";

function IncomeTab({ perms, calendar }: { perms: BillingPerms; calendar: Cal }) {
  const { t } = useI18n();
  const [sub, setSub] = useState<IncomeSub>("receipts");
  return (
    <div className="stack">
      <Segmented<IncomeSub>
        value={sub}
        onChange={setSub}
        label={t("billing.tab.income")}
        options={[
          { value: "receipts", label: t("income.receipts"), testId: "income-receipts" },
          { value: "invoices", label: t("income.invoices"), testId: "billing-tab-invoices" },
          { value: "installments", label: t("income.installments"), testId: "billing-tab-installments" },
          { value: "aging", label: t("income.aging"), testId: "billing-tab-debtors" },
        ]}
      />
      {sub === "receipts" && <ReceiptsList perms={perms} calendar={calendar} />}
      {sub === "invoices" && <InvoicesList perms={perms} calendar={calendar} />}
      {sub === "installments" && <InstallmentsList perms={perms} calendar={calendar} />}
      {sub === "aging" && <AgingList perms={perms} calendar={calendar} />}
    </div>
  );
}

/** The main income table: every receipt, refund and deposit move of the period, with its totals. */
function ReceiptsList({ perms, calendar }: { perms: BillingPerms; calendar: Cal }) {
  const { t, lang, err } = useI18n();
  const accounts = useAccounts();
  const p = usePeriod(calendar);
  const [search, setSearch] = useState("");
  const [list, setList] = useState<ReceiptInfo[] | null>(null);
  const [error, setError] = useState("");
  const [receipt, setReceipt] = useState<ReceiptInfo | null>(null);
  const [voiding, setVoiding] = useState<ReceiptInfo | null>(null);
  const [tick, setTick] = useState(0);
  const toast = useToast();
  useEffect(() => {
    if (!p.ok) return;
    const id = window.setTimeout(() => {
      rpc("fin.receipts.list", rcQ({ from: p.from, to: p.to, search: search.trim() || null, limit: 500 }))
        .then((v) => {
          setList(v);
          setError("");
        })
        .catch((e) => !isSessionError(e) && setError(err(e)));
    }, 200);
    return () => window.clearTimeout(id);
  }, [p.from, p.to, search, tick]);
  if (error && !list) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const posted = (list ?? []).filter((r) => r.status === "posted");
  const received = posted.filter((r) => r.kind === "rc").reduce((s, r) => s + r.amount, 0);
  const refunded = posted.filter((r) => r.kind === "rf").reduce((s, r) => s + r.amount, 0);
  return (
    <Card flush data-testid="billing-receipts">
      <CardHeader icon={Receipt} title={t("income.receipts")} description={t("income.receipts.hint")} />
      <div className="row toolbar" style={{ flexWrap: "wrap" }}>
        <PeriodFields p={p} calendar={calendar} testId="receipts" />
        <TextInput icon={Search} value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("billing.invoices.search")} data-testid="receipts-search" />
      </div>
      <div className="table-wrap">
        <table className="table" data-testid="receipts-table">
          <thead><tr><th>{t("doc.date")}</th><th>{t("rcpt.number")}</th><th>{t("doc.patient")}</th><th>{t("pay.kind")}</th><th>{t("fin.account")}</th><th>{t("rcpt.invoice")}</th><th className="num">{t("income.in")}</th><th className="num">{t("income.out")}</th><th /></tr></thead>
          <tbody>
            {!list && <SkeletonRows cols={9} />}
            {list?.map((r) => (
              <tr key={r.id} className={r.status !== "posted" ? "row-void" : undefined} data-testid={`receipt-row-${r.number}`}>
                <td>{formatDate(r.date, lang, calendar)}</td>
                <td><bdi className="ltr num">{r.number}</bdi> <StatusBadge status={r.status} /></td>
                <td><span className="cell-strong">{r.patient_name}</span> <bdi className="ltr num subtle">{r.patient_number}</bdi></td>
                <td>{t(`fin.doc.${r.kind}`)}</td>
                <td>{r.account_id !== null ? accountNameOf(r.account_id, accounts, t) : "—"}</td>
                <td>{r.allocations.map((a) => <bdi key={a.invoice_id} className="ltr num subtle">{a.invoice_number} </bdi>)}{r.to_deposit > 0 && <Badge tone="info">{t("acct.deposit")}</Badge>}</td>
                <td className="num">{r.kind === "rc" ? formatAmount(r.amount, lang) : ""}</td>
                <td className="num">{r.kind === "rf" ? formatAmount(r.amount, lang) : ""}</td>
                <td className="cell-actions">
                  <span className="row">
                    <Button size="sm" variant="link" icon={Receipt} onClick={() => setReceipt(r)}>{t("pay.receipt")}</Button>
                    {perms.void && r.status === "posted" && <Button size="sm" variant="link" onClick={() => setVoiding(r)} data-testid={`receipt-void-${r.number}`}>{t("fin.void")}</Button>}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
          {list && list.length > 0 && (
            <tfoot>
              <tr className="table-total">
                <td colSpan={6}>{t("income.totals").replace("{n}", digits(posted.length, lang))}</td>
                <td className="num" data-testid="receipts-total-in">{formatAmount(received, lang)}</td>
                <td className="num" data-testid="receipts-total-out">{formatAmount(refunded, lang)}</td>
                <td className="num cell-strong" data-testid="receipts-total-net">{formatAmount(received - refunded, lang)}</td>
              </tr>
            </tfoot>
          )}
        </table>
      </div>
      {list && !list.length && <EmptyState icon={Receipt} title={t("cash.noPayments")}>{t("cash.noPaymentsHint")}</EmptyState>}
      {receipt && <ReceiptHost receipt={receipt} calendar={calendar} onClose={() => setReceipt(null)} />}
      {voiding && <VoidDialog number={voiding.number} id={voiding.id} onClose={() => setVoiding(null)} onDone={() => { setVoiding(null); toast.success(t("fin.voided")); setTick((x) => x + 1); }} />}
    </Card>
  );
}

/** Voids any finance document with its reason; the Core refuses when later documents depend on it. */
export function VoidDialog({ id, number, onClose, onDone }: { id: number; number: string; onClose: () => void; onDone: () => void }) {
  const { t, err } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm({ reason: "" }, { reason: (s) => (s.trim() ? null : "rule.required") });
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await rpc("fin.docs.void", { id, reason: form.values.reason.trim() });
      invalidateAccounts();
      onDone();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={`${t("fin.void")} · ${number}`} description={t("fin.void.hint")} onClose={onClose} testId="void-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="danger" loading={busy} onClick={submit} data-testid="void-confirm">{t("fin.void")}</Button></>}>
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger" testId="void-error">{error}</Notice>}
        <Field label={t("billing.reason")} error={form.error("reason") && t(form.error("reason")!)}>
          <Textarea rows={2} value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="void-reason" />
        </Field>
      </form>
    </Dialog>
  );
}

function InvoicesList({ perms, calendar }: { perms: BillingPerms; calendar: Cal }) {
  const { t, lang, err } = useI18n();
  const [search, setSearch] = useState("");
  const [openOnly, setOpenOnly] = useState(false);
  const [list, setList] = useState<InvoiceInfo[] | null>(null);
  const [error, setError] = useState("");
  const [openId, setOpenId] = useState<number | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const id = window.setTimeout(() => {
      rpc("fin.invoices.list", invQ({ search: search.trim() || null, open_only: openOnly, limit: 200 }))
        .then((v) => {
          setList(v);
          setError("");
        })
        .catch((e) => !isSessionError(e) && setError(err(e)));
    }, 200);
    return () => window.clearTimeout(id);
  }, [search, openOnly, tick]);
  if (error && !list) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  return (
    <Card flush data-testid="billing-invoices">
      <CardHeader icon={ReceiptText} title={t("income.invoices")} description={t("billing.invoices.hint")} />
      <div className="row toolbar">
        <TextInput icon={Search} value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("billing.invoices.search")} data-testid="billing-invoice-search" />
        <Checkbox checked={openOnly} onChange={setOpenOnly} testId="billing-open-only">{t("billing.invoices.openOnly")}</Checkbox>
      </div>
      <div className="table-wrap">
        <table className="table" data-testid="billing-invoices-table">
          <thead><tr><th>{t("rcpt.number")}</th><th>{t("doc.patient")}</th><th>{t("doc.date")}</th><th className="num">{t("inv.net")}</th><th className="num">{t("inv.reductions")}</th><th className="num">{t("inv.paid")}</th><th className="num">{t("inv.remaining")}</th><th>{t("inst.state")}</th></tr></thead>
          <tbody>
            {!list && <SkeletonRows cols={8} />}
            {list?.map((i) => (
              <tr key={i.id} className="row-click" onClick={() => setOpenId(i.id)} data-testid={`billing-invoice-${i.number}`}>
                <td><button type="button" className="link-btn" onClick={(e) => { e.stopPropagation(); setOpenId(i.id); }}><bdi className="ltr num">{i.number}</bdi></button></td>
                <td>{i.patient_name}</td>
                <td>{formatDate(i.date, lang, calendar)}</td>
                <td className="num">{formatAmount(i.net, lang)}</td>
                <td className="num">{i.reductions + i.written_off ? formatAmount(i.reductions + i.written_off, lang) : "—"}</td>
                <td className="num">{formatAmount(i.allocated, lang)}</td>
                <td className="num cell-strong">{formatAmount(i.remaining, lang)}</td>
                <td><PayBadge state={i.pay_state} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {list && !list.length && <EmptyState icon={ReceiptText} title={t("inv.none")}>{t("billing.invoices.noneHint")}</EmptyState>}
      {openId !== null && <InvoiceDetail invoiceId={openId} perms={perms} calendar={calendar} onClose={() => setOpenId(null)} onChanged={() => setTick((x) => x + 1)} />}
    </Card>
  );
}

function InstallmentsList({ perms, calendar }: { perms: BillingPerms; calendar: Cal }) {
  const { t, lang, err } = useI18n();
  const [window_, setWindow] = useState<"7" | "30" | "overdue">("7");
  const [list, setList] = useState<InstallmentDue[] | null>(null);
  const [error, setError] = useState("");
  const [openId, setOpenId] = useState<number | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    setList(null);
    const until = window_ === "overdue" ? todayIso() : addDays(todayIso(), Number(window_));
    rpc("fin.receivables.due", { until, overdue_only: window_ === "overdue", patient_id: null })
      .then((v) => {
        setList(v);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [window_, tick]);
  if (error && !list) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const sum = (list ?? []).reduce((s, i) => s + i.amount - i.paid, 0);
  return (
    <Card flush data-testid="billing-installments">
      <CardHeader
        icon={BellRing}
        title={t("income.installments")}
        description={list ? t("inst.dueTotal").replace("{n}", formatAmount(sum, lang)).replace("{count}", digits(list.length, lang)) : undefined}
        actions={<Segmented<"7" | "30" | "overdue"> value={window_} onChange={setWindow} label={t("income.installments")} options={[{ value: "overdue", label: t("inst.window.overdue"), testId: "inst-window-overdue" }, { value: "7", label: t("inst.window.7"), testId: "inst-window-7" }, { value: "30", label: t("inst.window.30"), testId: "inst-window-30" }]} />}
      />
      <div className="table-wrap">
        <table className="table" data-testid="installments-due-table">
          <thead><tr><th>{t("doc.patient")}</th><th>{t("patient.field.phone")}</th><th>{t("rcpt.invoice")}</th><th>{t("inst.due")}</th><th className="num">{t("inst.unpaid")}</th><th /></tr></thead>
          <tbody>
            {!list && <SkeletonRows cols={6} />}
            {list?.map((i) => (
              <tr key={`${i.invoice_id}-${i.no}`} data-testid={`inst-due-${i.invoice_number}-${i.no}`}>
                <td className="cell-strong">{i.patient_name}</td>
                <td><bdi className="ltr">{i.phone ?? "—"}</bdi></td>
                <td><bdi className="ltr num">{i.invoice_number}</bdi> <span className="subtle">{digits(i.no, lang)}/{digits(i.count, lang)}</span></td>
                <td>{formatDate(i.due_date, lang, calendar)} {i.state === "overdue" && <Badge tone="danger" dot>{t("inst.state.overdue")} · {t("inst.daysLate").replace("{n}", digits(i.days_late, lang))}</Badge>}</td>
                <td className="num cell-strong">{formatAmount(i.amount - i.paid, lang)}</td>
                <td className="cell-actions"><Button size="sm" variant="link" onClick={() => setOpenId(i.invoice_id)}>{t("inst.open")}</Button></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {list && !list.length && <EmptyState icon={CalendarClock} title={t("inst.none")}>{t("inst.noneHint")}</EmptyState>}
      {openId !== null && <InvoiceDetail invoiceId={openId} perms={perms} calendar={calendar} onClose={() => setOpenId(null)} onChanged={() => setTick((x) => x + 1)} />}
    </Card>
  );
}

/** Who owes what, by how long (0–30, 31–60, 61–90, 90+ days), as of a day; with the deposits held. */
function AgingList({ perms, calendar }: { perms: BillingPerms; calendar: Cal }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const [asOf, setAsOf] = useState(todayIso());
  const [search, setSearch] = useState("");
  const [list, setList] = useState<AgingRow[] | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const [paying, setPaying] = useState<{ row: AgingRow; invoices: InvoiceInfo[] } | null>(null);
  const [statement, setStatement] = useState<AgingRow | null>(null);
  const [receipt, setReceipt] = useState<ReceiptInfo | null>(null);
  useEffect(() => {
    const id = window.setTimeout(() => {
      rpc("fin.receivables.aging", { as_of: asOf, search: search.trim() || null, limit: 1000 })
        .then((v) => {
          setList(v);
          setError("");
        })
        .catch((e) => !isSessionError(e) && setError(err(e)));
    }, 200);
    return () => window.clearTimeout(id);
  }, [asOf, search, tick]);
  if (error && !list) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const sum = (k: keyof Pick<AgingRow, "d0_30" | "d31_60" | "d61_90" | "d90_plus" | "total">) => (list ?? []).reduce((s, r) => s + r[k], 0);
  const pay = async (r: AgingRow) => {
    try {
      setPaying({ row: r, invoices: await rpc("fin.invoices.list", invQ({ patient_id: r.patient_id, open_only: true, limit: 100 })) });
    } catch (x) {
      if (!isSessionError(x)) toast.error(err(x));
    }
  };
  return (
    <Card flush data-testid="billing-debtors">
      <CardHeader icon={UsersRound} title={t("income.aging")} description={list ? t("debtors.total").replace("{n}", formatAmount(sum("total"), lang)).replace("{count}", digits(list.length, lang)) : undefined} />
      <div className="row toolbar" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
        <Field label={t("aging.asOf")}><DateField value={asOf} onChange={setAsOf} calendar={calendar} label={t("aging.asOf")} testId="aging-as-of" yearsBefore={3} yearsAfter={0} /></Field>
        <TextInput icon={Search} value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("debtors.search")} data-testid="debtors-search" />
      </div>
      <div className="table-wrap">
        <table className="table" data-testid="debtors-table">
          <thead><tr><th>{t("doc.patient")}</th><th>{t("patient.field.phone")}</th><th className="num">{t("aging.d0_30")}</th><th className="num">{t("aging.d31_60")}</th><th className="num">{t("aging.d61_90")}</th><th className="num">{t("aging.d90_plus")}</th><th className="num">{t("aging.total")}</th><th>{t("debtors.nextDue")}</th><th /></tr></thead>
          <tbody>
            {!list && <SkeletonRows cols={9} />}
            {list?.map((d) => (
              <tr key={d.patient_id} data-testid={`debtor-${d.patient_number}`}>
                <td><span className="cell-strong">{d.patient_name}</span> <bdi className="ltr num subtle">{d.patient_number}</bdi>{d.deposit > 0 && <span className="t-caption subtle"> · {t("acct.deposit")}: {formatAmount(d.deposit, lang)}</span>}</td>
                <td><bdi className="ltr">{d.phone ?? "—"}</bdi></td>
                <td className="num">{d.d0_30 ? formatAmount(d.d0_30, lang) : "—"}</td>
                <td className="num">{d.d31_60 ? formatAmount(d.d31_60, lang) : "—"}</td>
                <td className="num">{d.d61_90 ? formatAmount(d.d61_90, lang) : "—"}</td>
                <td className={`num ${d.d90_plus ? "money-danger" : ""}`}>{d.d90_plus ? formatAmount(d.d90_plus, lang) : "—"}</td>
                <td className="num cell-strong">{formatAmount(d.total, lang)}</td>
                <td>{d.next_due ? formatDate(d.next_due, lang, calendar) : "—"}</td>
                <td className="cell-actions">
                  <span className="row">
                    <Button size="sm" variant="link" onClick={() => setStatement(d)} data-testid={`debtor-statement-${d.patient_number}`}>{t("stmt.title")}</Button>
                    {perms.edit && <Button size="sm" variant="link" icon={Wallet} onClick={() => pay(d)} data-testid={`debtor-pay-${d.patient_number}`}>{t("pay.add")}</Button>}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
          {list && list.length > 0 && (
            <tfoot>
              <tr className="table-total">
                <td colSpan={2}>{t("ov.total")}</td>
                <td className="num">{formatAmount(sum("d0_30"), lang)}</td>
                <td className="num">{formatAmount(sum("d31_60"), lang)}</td>
                <td className="num">{formatAmount(sum("d61_90"), lang)}</td>
                <td className="num">{formatAmount(sum("d90_plus"), lang)}</td>
                <td className="num cell-strong" data-testid="aging-total">{formatAmount(sum("total"), lang)}</td>
                <td colSpan={2} />
              </tr>
            </tfoot>
          )}
        </table>
      </div>
      {list && !list.length && <EmptyState icon={UsersRound} title={t("debtors.none")}>{t("debtors.noneHint")}</EmptyState>}
      {paying && (
        <ReceiptDialog patientId={paying.row.patient_id} patientName={paying.row.patient_name} invoices={paying.invoices} onClose={() => setPaying(null)} onDone={(r) => { setPaying(null); toast.success(t("pay.saved")); setTick((x) => x + 1); setReceipt(r); }} />
      )}
      {receipt && <ReceiptHost receipt={receipt} calendar={calendar} onClose={() => setReceipt(null)} />}
      {statement && letterhead && <AgingStatement row={statement} calendar={calendar} onClose={() => setStatement(null)} />}
    </Card>
  );
}

function AgingStatement({ row, calendar, onClose }: { row: AgingRow; calendar: Cal; onClose: () => void }) {
  const { t, lang, err } = useI18n();
  const letterhead = useLetterhead();
  const [statement, setStatement] = useState<Awaited<ReturnType<typeof loadStatement>> | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    loadStatement(row.patient_id).then(setStatement).catch((e) => setError(err(e)));
  }, [row.patient_id]);
  if (error) return <Notice tone="danger">{error}</Notice>;
  if (!statement || !letterhead) return null;
  return <DocumentPreview title={t("stmt.title")} paper="a4" fileStem={`ST-${row.patient_number}-${todayIso()}`} testId="statement-preview" onClose={onClose} sheet={<StatementSheet statement={statement} letterhead={letterhead} lang={lang} calendar={calendar} />} />;
}

const loadStatement = (patientId: string) => rpc("fin.patients.statement", { patient_id: patientId, from: null, to: null });

// ───────────────────────────── cash and bank ─────────────────────────────

function CashTab({ perms, calendar }: { perms: BillingPerms; calendar: Cal }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const accounts = useAccounts();
  const money = moneyAccounts(accounts);
  const [account, setAccount] = useState<number | null>(null);
  const [date, setDate] = useState(todayIso());
  const [day, setDay] = useState<CashDayInfo | null>(null);
  const [receipts, setReceipts] = useState<ReceiptInfo[] | null>(null);
  const [vouchers, setVouchers] = useState<VoucherInfo[]>([]);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<null | "count" | "transfer" | "other" | "print">(null);
  const [receipt, setReceipt] = useState<ReceiptInfo | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (account === null && money.length) setAccount(money[0].id);
  }, [accounts]);
  useEffect(() => {
    if (account === null) return;
    setDay(null);
    Promise.all([
      rpc("fin.cash.day", { account_id: account, date }),
      rpc("fin.receipts.list", rcQ({ account_id: account, from: date, to: date, limit: 500 })),
      perms.reports ? rpc("fin.vouchers.list", pvQ({ kind: "pv", account_id: account, from: date, to: date, limit: 500 })) : Promise.resolve([] as VoucherInfo[]),
    ])
      .then(([d, r, v]) => {
        setDay(d);
        setReceipts(r);
        setVouchers(v.filter((x) => x.account_id === account));
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [account, date, tick]);
  if (error && !day) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const name = accountNameOf(account, accounts, t);
  const done = () => {
    setDialog(null);
    invalidateAccounts();
    setTick((x) => x + 1);
  };
  const isToday = date === todayIso();
  const lastCount = day?.counts[day.counts.length - 1];
  return (
    <div className="stack" data-testid="cash-tab">
      <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap", alignItems: "flex-end" }}>
        <div className="row" style={{ alignItems: "flex-end", flexWrap: "wrap" }}>
          <Field label={t("fin.account")}>
            <Select value={account ?? ""} onChange={(e) => setAccount(Number(e.target.value))} data-testid="cash-account">
              {money.map((a) => <option key={a.id} value={a.id}>{accountNameOf(a.id, accounts, t)}</option>)}
            </Select>
          </Field>
          <DateField value={date} onChange={setDate} calendar={calendar} label={t("cash.day")} testId="cash-date" yearsBefore={2} yearsAfter={0} />
          {!isToday && <Button size="sm" variant="link" onClick={() => setDate(todayIso())}>{t("cash.today")}</Button>}
        </div>
        <div className="row">
          <Button icon={Printer} onClick={() => setDialog("print")} disabled={!day || !letterhead} data-testid="cash-print">{t("cash.report")}</Button>
          {perms.edit && <Button icon={ArrowLeftRight} onClick={() => setDialog("transfer")} data-testid="cash-transfer-open">{t("cash.transfer")}</Button>}
          {perms.edit && <Button icon={PiggyBank} onClick={() => setDialog("other")} data-testid="cash-other-open">{t("cash.otherIn")}</Button>}
          {perms.edit && <Button variant="primary" icon={LockKeyhole} onClick={() => setDialog("count")} disabled={!day} data-testid="cash-close-open">{t("cash.count")}</Button>}
        </div>
      </div>
      <div className="stat-grid">
        <Stat icon={Landmark} label={t("cash.opening")} value={day ? formatAmount(day.opening, lang) : "…"} testId="cash-opening" />
        <Stat icon={Banknote} label={t("cash.received")} value={day ? formatAmount(day.received + day.other_in, lang) : "…"} tone="success" sub={day && day.other_in ? `${t("cash.otherIn")}: ${formatAmount(day.other_in, lang)}` : undefined} testId="cash-in" />
        <Stat icon={Wallet} label={t("cash.out")} value={day ? formatAmount(day.refunded + day.paid_out, lang) : "…"} tone="warning" sub={day ? `${t("cash.refunded")}: ${formatAmount(day.refunded, lang)} · ${t("cash.paidOut")}: ${formatAmount(day.paid_out, lang)}` : undefined} testId="cash-out" />
        <Stat icon={PiggyBank} label={t("cash.closing")} value={day ? formatAmount(day.closing, lang) : "…"} tone="accent" sub={day && day.transfers ? `${t("cash.transfers")}: ${day.transfers > 0 ? "+" : "−"}${formatAmount(Math.abs(day.transfers), lang)}` : undefined} testId="cash-expected" />
      </div>
      {lastCount && (
        <Notice tone={lastCount.difference === 0 ? "success" : "warning"} title={t("cash.countedTitle")} testId="cash-closed">
          {t("cash.countedBody").replace("{counted}", formatAmount(lastCount.counted, lang)).replace("{book}", formatAmount(lastCount.book, lang)).replace("{at}", formatDateTime(lastCount.created_at, lang, calendar))}
          {lastCount.difference !== 0 && <> {t("cash.differenceIs").replace("{n}", `${lastCount.difference > 0 ? "+" : "−"}${formatAmount(Math.abs(lastCount.difference), lang)}`)} {lastCount.reason}</>}
        </Notice>
      )}
      <Card flush>
        <CardHeader icon={Receipt} title={t("cash.paymentsOfDay")} description={day ? `${t("cash.receipts")}: ${digits(day.receipt_count, lang)}` : undefined} />
        <div className="table-wrap">
          <table className="table" data-testid="cash-payments">
            <thead><tr><th>{t("rcpt.number")}</th><th>{t("doc.patient")}</th><th>{t("pay.kind")}</th><th className="num">{t("pay.amount")}</th><th /></tr></thead>
            <tbody>
              {!receipts && <SkeletonRows cols={5} />}
              {receipts?.filter((r) => r.kind !== "al").map((r) => (
                <tr key={r.id} className={r.status !== "posted" ? "row-void" : undefined} data-testid={`cash-row-${r.number}`}>
                  <td><bdi className="ltr num">{r.number}</bdi> <StatusBadge status={r.status} /></td>
                  <td>{r.patient_name}</td>
                  <td>{t(`fin.doc.${r.kind}`)}</td>
                  <td className="num cell-strong"><Money value={r.kind === "rf" ? -r.amount : r.amount} /></td>
                  <td className="cell-actions"><Button size="sm" variant="link" icon={Receipt} onClick={() => setReceipt(r)}>{t("pay.receipt")}</Button></td>
                </tr>
              ))}
              {vouchers.map((v) => (
                <tr key={`v${v.id}`} className={v.status !== "posted" ? "row-void" : undefined}>
                  <td><bdi className="ltr num">{v.number}</bdi> <StatusBadge status={v.status} /></td>
                  <td>{v.name ?? v.supplier_name ?? "—"}</td>
                  <td>{t(`fin.purpose.${v.purpose ?? "other"}`)}</td>
                  <td className="num cell-strong"><Money value={-v.net} /></td>
                  <td />
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {receipts && !receipts.length && !vouchers.length && <EmptyState icon={Banknote} title={t("cash.noPayments")}>{t("cash.noPaymentsHint")}</EmptyState>}
      </Card>
      {dialog === "count" && day && <CountDialog day={day} account={name} onClose={() => setDialog(null)} onDone={() => { done(); toast.success(t("cash.counted.saved")); }} />}
      {dialog === "transfer" && account !== null && <TransferDialog from={account} calendar={calendar} onClose={() => setDialog(null)} onDone={() => { done(); toast.success(t("common.saved")); }} />}
      {dialog === "other" && <OtherReceiptDialog account={account} calendar={calendar} onClose={() => setDialog(null)} onDone={() => { done(); toast.success(t("common.saved")); }} />}
      {receipt && <ReceiptHost receipt={receipt} calendar={calendar} onClose={() => setReceipt(null)} />}
      {dialog === "print" && day && letterhead && (
        <DocumentPreview
          title={t("cash.report")}
          paper="a4"
          fileStem={`CASH-${account}-${date}`}
          testId="cash-preview"
          onClose={() => setDialog(null)}
          sheet={<CashDaySheet day={day} account={name} receipts={(receipts ?? []).filter((r) => r.kind !== "al" && r.status === "posted")} vouchers={vouchers.filter((v) => v.status === "posted")} letterhead={letterhead} lang={lang} calendar={calendar} />}
        />
      )}
    </div>
  );
}

/** Counts the money in an account against the books; any difference needs its reason and is booked. */
function CountDialog({ day, account, onClose, onDone }: { day: CashDayInfo; account: string; onClose: () => void; onDone: () => void }) {
  const { t, lang, err } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm({ counted: afnInput(day.closing), reason: "" }, {
    counted: (s) => amountRule(s, true),
    reason: (s, all) => (minorOf(all.counted) !== day.closing && !s.trim() ? "rule.cash_reason_required" : null),
  });
  const counted = minorOf(form.values.counted);
  const diff = counted - day.closing;
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await rpc("fin.cash.count", { account_id: day.account_id, date: day.date, counted, reason: form.values.reason.trim() || null });
      onDone();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t("cash.count")}
      description={`${account} · ${t("cash.count.hint")}`}
      onClose={onClose}
      testId="cash-close-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={LockKeyhole} loading={busy} onClick={submit} data-testid="cash-close-confirm">{t("cash.count")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <dl className="fin-summary">
          <div><dt>{t("cash.book")}</dt><dd className="num" data-testid="close-expected">{formatAmount(day.closing, lang)}</dd></div>
        </dl>
        <Field label={t("cash.counted")} hint={t("cash.counted.hint")} error={form.error("counted") && t(form.error("counted")!)}>
          <AmountInput value={form.values.counted} onChange={(v) => form.set("counted", v)} testId="cash-counted" autoFocus />
        </Field>
        <Notice tone={form.values.counted.trim() === "" ? "warning" : diff === 0 ? "success" : "warning"} testId="cash-diff">{form.values.counted.trim() === "" ? t("cash.diff.enter") : diff === 0 ? t("cash.diff.none") : t(diff > 0 ? "cash.diff.over" : "cash.diff.short").replace("{n}", formatAmount(Math.abs(diff), lang))}</Notice>
        <Field label={t("cash.reason")} hint={t("cash.reason.hint")} optional={diff === 0} error={form.error("reason") && t(form.error("reason")!)}>
          <Textarea rows={2} value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="cash-reason" />
        </Field>
      </form>
    </Dialog>
  );
}

/** Money moved between two money accounts (cash to the bank, …): not income, not an expense. */
function TransferDialog({ from, calendar, onClose, onDone }: { from: number; calendar: Cal; onClose: () => void; onDone: () => void }) {
  const { t, err } = useI18n();
  const [source, setSource] = useState<number | null>(from);
  const [target, setTarget] = useState<number | null>(null);
  const [date, setDate] = useState(todayIso());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm({ amount: "", reference: "" }, { amount: (s) => amountRule(s) });
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate() || source === null || target === null) return;
    setBusy(true);
    try {
      await rpc("fin.transfers.create", { date, from_account_id: source, to_account_id: target, amount: minorOf(form.values.amount), reference: form.values.reference.trim() || null, note: null });
      onDone();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("cash.transfer")} description={t("cash.transfer.hint")} onClose={onClose} testId="transfer-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={ArrowLeftRight} loading={busy} onClick={submit} data-testid="transfer-save">{t("common.save")}</Button></>}>
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="grid-2">
          <Field label={t("cash.transfer.from")}><MoneyAccountSelect value={source} onChange={setSource} testId="transfer-from" /></Field>
          <Field label={t("cash.transfer.to")}><MoneyAccountSelect value={target} onChange={setTarget} exclude={source} testId="transfer-to" /></Field>
        </div>
        <Field label={t("pay.amount")} error={form.error("amount") && t(form.error("amount")!)}><AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="transfer-amount" autoFocus /></Field>
        <div className="grid-2">
          <Field label={t("doc.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="transfer-date" yearsBefore={1} yearsAfter={0} /></Field>
          <Field label={t("fin.reference")} optional><TextInput value={form.values.reference} onChange={(e) => form.set("reference", e.target.value)} maxLength={60} data-testid="transfer-reference" /></Field>
        </div>
      </form>
    </Dialog>
  );
}
