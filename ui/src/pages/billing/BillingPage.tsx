import { payQ, invQ } from "../../lib/billing";
// «مالی» — the money side of the clinic (Phase 6): the day at the cash desk and its closing (6.7), every
// invoice, who owes what (6.4), installments due (6.3), expenses (6.6), doctor commission (6.5), and the
// settings of all of it. What a person sees follows their permissions.
import { useEffect, useMemo, useState } from "react";
import { Banknote, BellRing, CalendarClock, Landmark, LockKeyhole, Printer, Receipt, ReceiptText, Search, Settings2, UsersRound, Wallet } from "lucide-react";
import type { CashDay, ClinicProfile, DebtorInfo, ExpenseInfo, InstallmentDue, InvoiceInfo, PaymentInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { afnInput, amountRule, parseAfn } from "../../lib/billing";
import { addDays, todayIso } from "../../lib/calendar";
import { digits, formatDate, formatDateTime } from "../../lib/dates";
import { tr } from "../../lib/medical";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader, Page, PageHeader, Stat } from "../../ui/Card";
import { Checkbox, Segmented } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, TextInput, Textarea } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { DocumentPreview } from "../../print/DocumentPreview";
import { CashDaySheet, StatementSheet } from "../../print/finance";
import { useLetterhead } from "../../print/letterhead";
import { AmountInput, PayBadge, ReceiptHost, minorOf } from "./common";
import type { BillingPerms } from "./common";
import { PaymentDialog } from "./dialogs";
import { CommissionTab, ExpensesTab, BillingSettingsTab } from "./BillingExtras";
import { InvoiceDetail } from "./InvoiceDetail";

type TabId = "cash" | "invoices" | "debtors" | "installments" | "expenses" | "commission" | "settings";

const TAB_ICONS = { cash: Banknote, invoices: ReceiptText, debtors: UsersRound, installments: CalendarClock, expenses: Wallet, commission: Landmark, settings: Settings2 };

export function BillingPage({ perms, clinic }: { perms: BillingPerms; clinic: ClinicProfile | null }) {
  const { t } = useI18n();
  const tabs = useMemo<TabId[]>(
    () => [
      ...(perms.view ? (["cash", "invoices", "debtors", "installments"] as TabId[]) : []),
      ...(perms.reports ? (["expenses", "commission"] as TabId[]) : []),
      ...(perms.settings ? (["settings"] as TabId[]) : []),
    ],
    [perms],
  );
  const [tab, setTab] = useState<TabId>(tabs[0] ?? "cash");
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
      {tab === "cash" && <CashTab perms={perms} calendar={calendar} />}
      {tab === "invoices" && <InvoicesTab perms={perms} calendar={calendar} />}
      {tab === "debtors" && <DebtorsTab perms={perms} calendar={calendar} />}
      {tab === "installments" && <InstallmentsTab perms={perms} calendar={calendar} />}
      {tab === "expenses" && <ExpensesTab perms={perms} calendar={calendar} />}
      {tab === "commission" && <CommissionTab calendar={calendar} />}
      {tab === "settings" && <BillingSettingsTab />}
    </Page>
  );
}

// ───────────────────────────── the day at the cash desk ─────────────────────────────

function CashTab({ perms, calendar }: { perms: BillingPerms; calendar: ClinicProfile["calendar_system"] }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const [date, setDate] = useState(todayIso());
  const [day, setDay] = useState<CashDay | null>(null);
  const [payments, setPayments] = useState<PaymentInfo[] | null>(null);
  const [expenses, setExpenses] = useState<ExpenseInfo[]>([]);
  const [error, setError] = useState("");
  const [closing, setClosing] = useState(false);
  const [printing, setPrinting] = useState(false);
  const [receipt, setReceipt] = useState<PaymentInfo | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    setDay(null);
    Promise.all([
      rpc("cash.day", { date }),
      rpc("payments.list", payQ({ from: date, to: date, limit: 300 })),
      perms.reports ? rpc("expenses.list", { from: date, to: date }) : Promise.resolve([] as ExpenseInfo[]),
    ])
      .then(([d, p, e]) => {
        setDay(d);
        setPayments(p);
        setExpenses(e);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [date, tick]);
  if (error && !day) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const closed = day?.closing;
  const stale = !!closed && !!day && closed.expected !== day.expected;
  const isToday = date === todayIso();
  return (
    <div className="stack" data-testid="cash-tab">
      <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap" }}>
        <DateField value={date} onChange={setDate} calendar={calendar} label={t("cash.day")} testId="cash-date" yearsBefore={2} yearsAfter={0} />
        <div className="row">
          {!isToday && <Button size="sm" variant="link" onClick={() => setDate(todayIso())}>{t("cash.today")}</Button>}
          <Button icon={Printer} onClick={() => setPrinting(true)} disabled={!day || !letterhead} data-testid="cash-print">{t("cash.report")}</Button>
          {perms.edit && !closed && <Button variant="primary" icon={LockKeyhole} onClick={() => setClosing(true)} disabled={!day} data-testid="cash-close-open">{t("cash.close")}</Button>}
        </div>
      </div>
      <div className="stat-grid">
        <Stat icon={Banknote} label={t("cash.cashIn")} value={day ? formatAmount(day.cash_in, lang) : "…"} testId="cash-in" />
        <Stat icon={Wallet} label={t("cash.cashOut")} value={day ? formatAmount(day.cash_refunds + day.cash_expenses, lang) : "…"} tone="warning" sub={day ? `${t("cash.cashRefunds")}: ${formatAmount(day.cash_refunds, lang)} · ${t("cash.cashExpenses")}: ${formatAmount(day.cash_expenses, lang)}` : undefined} testId="cash-out" />
        <Stat icon={Landmark} label={t("cash.expected")} value={day ? formatAmount(day.expected, lang) : "…"} tone="success" testId="cash-expected" />
        <Stat icon={Receipt} label={t("cash.receipts")} value={day ? digits(day.payment_count, lang) : "…"} tone="info" testId="cash-count" />
      </div>
      {closed && (
        <Notice tone={closed.difference === 0 ? "success" : "warning"} title={t("cash.closedTitle")} testId="cash-closed">
          {t("cash.closedBody").replace("{counted}", formatAmount(closed.counted, lang)).replace("{expected}", formatAmount(closed.expected, lang)).replace("{at}", formatDateTime(closed.closed_at, lang, calendar))}
          {closed.difference !== 0 && <> {t("cash.differenceIs").replace("{n}", `${closed.difference > 0 ? "+" : "−"}${formatAmount(Math.abs(closed.difference), lang)}`)} {closed.reason}</>}
          {stale && <> {t("cash.staleAfterClose")}</>}
        </Notice>
      )}
      <Card flush>
        <CardHeader icon={Receipt} title={t("cash.paymentsOfDay")} />
        <div className="table-wrap">
          <table className="table" data-testid="cash-payments">
            <thead><tr><th>{t("rcpt.number")}</th><th>{t("doc.patient")}</th><th>{t("rcpt.method")}</th><th className="num">{t("pay.amount")}</th><th /></tr></thead>
            <tbody>
              {!payments && <SkeletonRows cols={5} />}
              {payments?.map((p) => (
                <tr key={p.id} data-testid={`cash-row-${p.number}`}>
                  <td><bdi className="ltr num">{p.number}</bdi></td>
                  <td>{p.patient_name}</td>
                  <td>{p.kind === "credit" ? t("pay.kind.credit") : tr(p.method, lang) || "—"}</td>
                  <td className="num cell-strong">{p.kind === "refund" ? "− " : ""}{formatAmount(p.amount, lang)}</td>
                  <td className="cell-actions"><Button size="sm" variant="link" icon={Receipt} onClick={() => setReceipt(p)}>{t("pay.receipt")}</Button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {payments && !payments.length && <EmptyState icon={Banknote} title={t("cash.noPayments")}>{t("cash.noPaymentsHint")}</EmptyState>}
      </Card>
      {closing && day && <CloseCashDialog day={day} onClose={() => setClosing(false)} onDone={() => { setClosing(false); toast.success(t("cash.closed")); setTick((x) => x + 1); }} />}
      {receipt && <ReceiptHost payment={receipt} calendar={calendar} onClose={() => setReceipt(null)} />}
      {printing && day && letterhead && (
        <DocumentPreview
          title={t("cash.report")}
          paper="a4"
          fileStem={`CASH-${date}`}
          testId="cash-preview"
          onClose={() => setPrinting(false)}
          sheet={<CashDaySheet day={day} payments={payments ?? []} expenses={expenses} letterhead={letterhead} lang={lang} calendar={calendar} />}
        />
      )}
    </div>
  );
}

/** Counted against expected (6.7): any difference needs its reason. */
function CloseCashDialog({ day, onClose, onDone }: { day: CashDay; onClose: () => void; onDone: () => void }) {
  const { t, lang, err } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm({ counted: afnInput(day.expected), reason: "" }, {
    counted: (s) => amountRule(s, true),
    reason: (s, all) => (minorOf(all.counted) !== day.expected && !s.trim() ? "rule.cash_reason_required" : null),
  });
  const counted = minorOf(form.values.counted);
  const diff = counted - day.expected;
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await rpc("cash.close", { date: day.date, counted: parseAfn(form.values.counted) ?? 0, reason: form.values.reason.trim() || null });
      onDone();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t("cash.close")}
      description={t("cash.close.hint")}
      onClose={onClose}
      testId="cash-close-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={LockKeyhole} loading={busy} onClick={submit} data-testid="cash-close-confirm">{t("cash.close")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <dl className="fin-summary">
          <div><dt>{t("cash.expected")}</dt><dd className="num" data-testid="close-expected">{formatAmount(day.expected, lang)}</dd></div>
        </dl>
        <Field label={t("cash.counted")} hint={t("cash.counted.hint")} error={form.error("counted") && t(form.error("counted")!)}>
          <AmountInput value={form.values.counted} onChange={(v) => form.set("counted", v)} testId="cash-counted" autoFocus />
        </Field>
        <Notice tone={diff === 0 ? "success" : "warning"} testId="cash-diff">{diff === 0 ? t("cash.diff.none") : t(diff > 0 ? "cash.diff.over" : "cash.diff.short").replace("{n}", formatAmount(Math.abs(diff), lang))}</Notice>
        <Field label={t("cash.reason")} hint={t("cash.reason.hint")} optional={diff === 0} error={form.error("reason") && t(form.error("reason")!)}>
          <Textarea rows={2} value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="cash-reason" />
        </Field>
      </form>
    </Dialog>
  );
}

// ───────────────────────────── invoices ─────────────────────────────

function InvoicesTab({ perms, calendar }: { perms: BillingPerms; calendar: ClinicProfile["calendar_system"] }) {
  const { t, lang, err } = useI18n();
  const [search, setSearch] = useState("");
  const [openOnly, setOpenOnly] = useState(false);
  const [list, setList] = useState<InvoiceInfo[] | null>(null);
  const [error, setError] = useState("");
  const [openId, setOpenId] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const id = window.setTimeout(() => {
      rpc("invoices.list", invQ({ search: search.trim() || null, open_only: openOnly, limit: 150 }))
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
      <CardHeader icon={ReceiptText} title={t("billing.tab.invoices")} description={t("billing.invoices.hint")} />
      <div className="row toolbar">
        <TextInput icon={Search} value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("billing.invoices.search")} data-testid="billing-invoice-search" />
        <Checkbox checked={openOnly} onChange={setOpenOnly} testId="billing-open-only">{t("billing.invoices.openOnly")}</Checkbox>
      </div>
      <div className="table-wrap">
        <table className="table" data-testid="billing-invoices-table">
          <thead><tr><th>{t("rcpt.number")}</th><th>{t("doc.patient")}</th><th>{t("doc.date")}</th><th className="num">{t("inv.net")}</th><th className="num">{t("inv.remaining")}</th><th>{t("inst.state")}</th></tr></thead>
          <tbody>
            {!list && <SkeletonRows cols={6} />}
            {list?.map((i) => (
              <tr key={i.id} className="row-click" onClick={() => setOpenId(i.id)} data-testid={`billing-invoice-${i.number}`}>
                <td><button type="button" className="link-btn" onClick={(e) => { e.stopPropagation(); setOpenId(i.id); }}><bdi className="ltr num">{i.number}</bdi></button></td>
                <td>{i.patient_name}</td>
                <td>{formatDate(i.issued_at, lang, calendar)}</td>
                <td className="num">{formatAmount(i.net, lang)}</td>
                <td className="num cell-strong">{formatAmount(i.remaining, lang)}</td>
                <td><PayBadge state={i.pay_state} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {list && !list.length && <EmptyState icon={ReceiptText} title={t("inv.none")}>{t("billing.invoices.noneHint")}</EmptyState>}
      {openId && <InvoiceDetail invoiceId={openId} perms={perms} calendar={calendar} credit={0} onClose={() => setOpenId(null)} onChanged={() => setTick((x) => x + 1)} />}
    </Card>
  );
}

// ───────────────────────────── debtors ─────────────────────────────

function DebtorsTab({ perms, calendar }: { perms: BillingPerms; calendar: ClinicProfile["calendar_system"] }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const [search, setSearch] = useState("");
  const [list, setList] = useState<DebtorInfo[] | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const [paying, setPaying] = useState<{ debtor: DebtorInfo; invoices: InvoiceInfo[]; credit: number } | null>(null);
  const [statement, setStatement] = useState<DebtorInfo | null>(null);
  const [receipt, setReceipt] = useState<PaymentInfo | null>(null);
  useEffect(() => {
    const id = window.setTimeout(() => {
      rpc("billing.debtors", { search: search.trim() || null, limit: 300 })
        .then((v) => {
          setList(v);
          setError("");
        })
        .catch((e) => !isSessionError(e) && setError(err(e)));
    }, 200);
    return () => window.clearTimeout(id);
  }, [search, tick]);
  if (error && !list) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const total = (list ?? []).reduce((s, d) => s + d.remaining, 0);
  const pay = async (d: DebtorInfo) => {
    try {
      const [invoices, account] = await Promise.all([rpc("invoices.list", invQ({ patient_id: d.patient_id, open_only: true, limit: 100 })), rpc("billing.account", { patient_id: d.patient_id })]);
      setPaying({ debtor: d, invoices, credit: account.credit });
    } catch (x) {
      if (!isSessionError(x)) toast.error(err(x));
    }
  };
  return (
    <>
      <Card flush data-testid="billing-debtors">
        <CardHeader icon={UsersRound} title={t("billing.tab.debtors")} description={list ? t("debtors.total").replace("{n}", formatAmount(total, lang)).replace("{count}", digits(list.length, lang)) : undefined} />
        <div className="row toolbar">
          <TextInput icon={Search} value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("debtors.search")} data-testid="debtors-search" />
        </div>
        <div className="table-wrap">
          <table className="table" data-testid="debtors-table">
            <thead><tr><th>{t("doc.patient")}</th><th>{t("patient.field.phone")}</th><th className="num">{t("inv.remaining")}</th><th>{t("debtors.since")}</th><th>{t("debtors.lastPayment")}</th><th>{t("debtors.nextDue")}</th><th /></tr></thead>
            <tbody>
              {!list && <SkeletonRows cols={7} />}
              {list?.map((d) => (
                <tr key={d.patient_id} data-testid={`debtor-${d.patient_number}`}>
                  <td><span className="cell-strong">{d.patient_name}</span> <bdi className="ltr num subtle">{d.patient_number}</bdi></td>
                  <td><bdi className="ltr">{d.phone ?? "—"}</bdi></td>
                  <td className="num cell-strong">{formatAmount(d.remaining, lang)}</td>
                  <td>{d.oldest_invoice_at ? formatDate(d.oldest_invoice_at, lang, calendar) : "—"}</td>
                  <td>{d.last_payment_at ? formatDate(d.last_payment_at, lang, calendar) : "—"}</td>
                  <td>{d.next_due ? <>{formatDate(d.next_due, lang, calendar)} {d.overdue && <Badge tone="danger" dot>{t("inst.state.overdue")}</Badge>}</> : "—"}</td>
                  <td className="cell-actions">
                    <span className="row">
                      <Button size="sm" variant="link" onClick={() => setStatement(d)} data-testid={`debtor-statement-${d.patient_number}`}>{t("stmt.title")}</Button>
                      {perms.edit && <Button size="sm" variant="link" icon={Wallet} onClick={() => pay(d)} data-testid={`debtor-pay-${d.patient_number}`}>{t("pay.add")}</Button>}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {list && !list.length && <EmptyState icon={UsersRound} title={t("debtors.none")}>{t("debtors.noneHint")}</EmptyState>}
      </Card>
      {paying && (
        <PaymentDialog patientId={paying.debtor.patient_id} patientName={paying.debtor.patient_name} invoices={paying.invoices} credit={paying.credit} onClose={() => setPaying(null)} onDone={(p) => { setPaying(null); toast.success(t("pay.saved")); setTick((x) => x + 1); setReceipt(p); }} />
      )}
      {receipt && <ReceiptHost payment={receipt} calendar={calendar} onClose={() => setReceipt(null)} />}
      {statement && letterhead && <DebtorStatement debtor={statement} calendar={calendar} onClose={() => setStatement(null)} />}
    </>
  );
}

function DebtorStatement({ debtor, calendar, onClose }: { debtor: DebtorInfo; calendar: ClinicProfile["calendar_system"]; onClose: () => void }) {
  const { t, lang, err } = useI18n();
  const letterhead = useLetterhead();
  const [statement, setStatement] = useState<Awaited<ReturnType<typeof loadStatement>> | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    loadStatement(debtor.patient_id).then(setStatement).catch((e) => setError(err(e)));
  }, [debtor.patient_id]);
  if (error) return <Notice tone="danger">{error}</Notice>;
  if (!statement || !letterhead) return null;
  return <DocumentPreview title={t("stmt.title")} paper="a4" fileStem={`ST-${debtor.patient_number}-${todayIso()}`} testId="statement-preview" onClose={onClose} sheet={<StatementSheet statement={statement} letterhead={letterhead} lang={lang} calendar={calendar} />} />;
}

const loadStatement = (patientId: string) => rpc("billing.statement", { patient_id: patientId, from: null, to: null });

// ───────────────────────────── installments due ─────────────────────────────

function InstallmentsTab({ perms, calendar }: { perms: BillingPerms; calendar: ClinicProfile["calendar_system"] }) {
  const { t, lang, err } = useI18n();
  const [window_, setWindow] = useState<"7" | "30" | "overdue">("7");
  const [list, setList] = useState<InstallmentDue[] | null>(null);
  const [error, setError] = useState("");
  const [openId, setOpenId] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    setList(null);
    const through = window_ === "overdue" ? addDays(todayIso(), -1) : addDays(todayIso(), Number(window_));
    rpc("billing.installments_due", { through })
      .then((v) => {
        setList(v);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [window_, tick]);
  if (error && !list) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const sum = (list ?? []).reduce((s, i) => s + i.unpaid, 0);
  return (
    <Card flush data-testid="billing-installments">
      <CardHeader
        icon={BellRing}
        title={t("billing.tab.installments")}
        description={list ? t("inst.dueTotal").replace("{n}", formatAmount(sum, lang)).replace("{count}", digits(list.length, lang)) : undefined}
        actions={<Segmented<"7" | "30" | "overdue"> value={window_} onChange={setWindow} label={t("billing.tab.installments")} options={[{ value: "overdue", label: t("inst.window.overdue"), testId: "inst-window-overdue" }, { value: "7", label: t("inst.window.7"), testId: "inst-window-7" }, { value: "30", label: t("inst.window.30"), testId: "inst-window-30" }]} />}
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
                <td><bdi className="ltr num">{i.invoice_number}</bdi> <span className="subtle">#{digits(i.no, lang)}</span></td>
                <td>{formatDate(i.due_date, lang, calendar)} {i.overdue && <Badge tone="danger" dot>{t("inst.state.overdue")}</Badge>}</td>
                <td className="num cell-strong">{formatAmount(i.unpaid, lang)}</td>
                <td className="cell-actions"><Button size="sm" variant="link" onClick={() => setOpenId(i.invoice_id)}>{t("inst.open")}</Button></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {list && !list.length && <EmptyState icon={CalendarClock} title={t("inst.none")}>{t("inst.noneHint")}</EmptyState>}
      {openId && <InvoiceDetail invoiceId={openId} perms={perms} calendar={calendar} credit={0} onClose={() => setOpenId(null)} onChanged={() => setTick((x) => x + 1)} />}
    </Card>
  );
}
