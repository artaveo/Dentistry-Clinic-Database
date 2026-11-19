// Money out and suppliers (docs/design/finance-standard.md §ب۸): payment vouchers (expenses paid, suppliers paid,
// withheld tax paid, owner drawings), supplier bills, and other money in (capital, loans, sales).
import { useEffect, useState } from "react";
import { FileInput, Plus, Trash2, Wallet } from "lucide-react";
import type { CalendarSystem, OtherReceiptSource, SupplierInfo, VoucherInfo, VoucherPurpose, WithholdingKind } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { accountNameOf, amountRule, expenseAccounts, invalidateAccounts, pvQ, useAccounts } from "../../lib/billing";
import { monthRange, todayIso } from "../../lib/calendar";
import { formatDate } from "../../lib/dates";
import { formatAmount } from "../../lib/money";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, Stat } from "../../ui/Card";
import { Segmented } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, Select, TextInput } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { AccountSelect, AmountInput, MoneyAccountSelect, StatusBadge, minorOf } from "./common";
import type { BillingPerms } from "./common";
import { VoidDialog } from "./BillingPage";

export { BillingSettingsTab, CommissionTab } from "./FinanceSettings";

type Cal = CalendarSystem;

// ───────────────────────────── expenses and suppliers ─────────────────────────────

export function ExpensesTab({ perms, calendar }: { perms: BillingPerms; calendar: Cal }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const accounts = useAccounts();
  const [from, setFrom] = useState(monthRange(todayIso(), calendar).from);
  const [to, setTo] = useState(todayIso());
  const [kind, setKind] = useState<"pv" | "bl">("pv");
  const [list, setList] = useState<VoucherInfo[] | null>(null);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<null | "expense" | "supplier" | "bill" | "tax" | "drawing">(null);
  const [voiding, setVoiding] = useState<VoucherInfo | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (from > to) return;
    rpc("fin.vouchers.list", pvQ({ kind, from, to, limit: 500 }))
      .then((v) => {
        setList(v);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [from, to, kind, tick]);
  if (error && !list) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const money = (v: number) => formatAmount(v, lang);
  const posted = (list ?? []).filter((v) => v.status === "posted");
  const gross = posted.reduce((s, v) => s + v.gross, 0);
  const net = posted.reduce((s, v) => s + v.net, 0);
  const withheld = posted.reduce((s, v) => s + v.withheld, 0);
  const done = () => {
    setDialog(null);
    invalidateAccounts();
    toast.success(t("common.saved"));
    setTick((x) => x + 1);
  };
  const lineAccounts = (v: VoucherInfo) => [...new Set(v.lines.map((l) => accountNameOf(l.account_id, accounts, t)))].join("، ");
  return (
    <div className="stack" data-testid="billing-expenses">
      <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end", justifyContent: "space-between" }}>
        <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
          <Field label={t("period.from")}><DateField value={from} onChange={setFrom} calendar={calendar} label={t("period.from")} testId="expenses-from" yearsBefore={3} yearsAfter={0} /></Field>
          <Field label={t("period.to")}><DateField value={to} onChange={setTo} calendar={calendar} label={t("period.to")} testId="expenses-to" yearsBefore={3} yearsAfter={0} /></Field>
          <Segmented<"pv" | "bl"> value={kind} onChange={setKind} label={t("billing.tab.expenses")} options={[{ value: "pv", label: t("exp.paid"), testId: "expenses-kind-pv" }, { value: "bl", label: t("exp.bills"), testId: "expenses-kind-bl" }]} />
        </div>
        {perms.edit && (
          <div className="row" style={{ flexWrap: "wrap" }}>
            <Button icon={FileInput} onClick={() => setDialog("bill")} data-testid="bill-add-open">{t("exp.bill.new")}</Button>
            <Button onClick={() => setDialog("supplier")} data-testid="supplier-pay-open">{t("exp.paySupplier")}</Button>
            <Button onClick={() => setDialog("tax")} data-testid="tax-pay-open">{t("exp.payTax")}</Button>
            <Button onClick={() => setDialog("drawing")} data-testid="drawing-open">{t("exp.drawing")}</Button>
            <Button variant="primary" icon={Plus} onClick={() => setDialog("expense")} data-testid="expense-add-open">{t("expenses.add")}</Button>
          </div>
        )}
      </div>
      {from > to && <Notice tone="warning">{t("rule.period_range")}</Notice>}
      <div className="stat-grid">
        <Stat icon={Wallet} label={kind === "pv" ? t("exp.totalPaid") : t("exp.totalBilled")} value={list ? money(kind === "pv" ? net : gross) : "…"} tone="warning" testId="expenses-total" />
        {kind === "pv" && <Stat icon={Wallet} label={t("exp.withheld")} value={list ? money(withheld) : "…"} tone="info" sub={t("exp.withheld.hint")} testId="expenses-withheld" />}
      </div>
      <Card flush>
        <div className="table-wrap">
          <table className="table" data-testid="expenses-table">
            <thead><tr><th>{t("doc.date")}</th><th>{t("rcpt.number")}</th><th>{t("exp.purpose")}</th><th>{t("exp.accounts")}</th><th>{t("expenses.payee")}</th><th>{t("fin.account")}</th><th className="num">{t("exp.gross")}</th><th className="num">{t("exp.withheld")}</th><th className="num">{t("exp.net")}</th><th /></tr></thead>
            <tbody>
              {!list && <SkeletonRows cols={10} />}
              {list?.map((v) => (
                <tr key={v.id} className={v.status !== "posted" ? "row-void" : undefined} data-testid={`expense-${v.number}`}>
                  <td>{formatDate(v.date, lang, calendar)}</td>
                  <td><bdi className="ltr num">{v.number}</bdi> <StatusBadge status={v.status} /></td>
                  <td><Badge>{v.kind === "bl" ? t("exp.bill") : t(`fin.purpose.${v.purpose ?? "other"}`)}</Badge></td>
                  <td>{lineAccounts(v)}{v.lines[0]?.description && <span className="t-caption subtle"> · {v.lines[0].description}</span>}</td>
                  <td>{v.supplier_name ?? v.name ?? "—"}{v.reference && <span className="t-caption subtle"> · <bdi className="ltr">{v.reference}</bdi></span>}</td>
                  <td>{v.account_id !== null ? accountNameOf(v.account_id, accounts, t) : t("exp.onCredit")}</td>
                  <td className="num">{money(v.gross)}</td>
                  <td className="num">{v.withheld ? money(v.withheld) : "—"}</td>
                  <td className="num cell-strong">{money(v.net)}</td>
                  <td className="cell-actions">{perms.void && v.status === "posted" && <IconButton icon={Trash2} label={t("fin.void")} onClick={() => setVoiding(v)} data-testid={`expense-void-${v.number}`} />}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {list && !list.length && <EmptyState icon={Wallet} title={t("expenses.none")}>{t("expenses.noneHint")}</EmptyState>}
      </Card>
      {dialog === "expense" && <VoucherDialog purpose="expense" calendar={calendar} onClose={() => setDialog(null)} onDone={done} />}
      {dialog === "supplier" && <VoucherDialog purpose="supplier" calendar={calendar} onClose={() => setDialog(null)} onDone={done} />}
      {dialog === "tax" && <VoucherDialog purpose="tax" calendar={calendar} onClose={() => setDialog(null)} onDone={done} />}
      {dialog === "drawing" && <VoucherDialog purpose="drawing" calendar={calendar} onClose={() => setDialog(null)} onDone={done} />}
      {dialog === "bill" && <BillDialog calendar={calendar} onClose={() => setDialog(null)} onDone={done} />}
      {voiding && <VoidDialog id={voiding.id} number={voiding.number} onClose={() => setVoiding(null)} onDone={() => { setVoiding(null); toast.success(t("fin.voided")); setTick((x) => x + 1); }} />}
    </div>
  );
}

/** The suppliers and labs (for bills and payments). */
function useSuppliers() {
  const [list, setList] = useState<SupplierInfo[]>([]);
  useEffect(() => {
    rpc("suppliers.list", { kind: null, include_inactive: false, owing_only: false }).then(setList).catch(() => {});
  }, []);
  return list;
}

type Line = { account: number | null; amount: string; withheld: string; kind: WithholdingKind | ""; description: string };
const emptyLine = (): Line => ({ account: null, amount: "", withheld: "", kind: "", description: "" });

/** Expense lines: account, amount, description and (vouchers only) the tax kept back, entered by hand. */
function LinesEditor({ lines, setLines, withholding }: { lines: Line[]; setLines: (l: Line[]) => void; withholding: boolean }) {
  const { t } = useI18n();
  const accounts = useAccounts();
  const list = expenseAccounts(accounts);
  const set = (i: number, patch: Partial<Line>) => setLines(lines.map((l, j) => (j === i ? { ...l, ...patch } : l)));
  return (
    <div className="stack" style={{ gap: "var(--space-2)" }} data-testid="voucher-lines">
      {lines.map((l, i) => (
        <div key={i} className="stack voucher-line" style={{ gap: "var(--space-2)" }}>
          <div className="grid-2">
            <Field label={t("exp.account")}><AccountSelect value={l.account} onChange={(a) => set(i, { account: a })} accounts={list} placeholder={t("common.choose")} testId={`voucher-account-${i}`} /></Field>
            <Field label={t("pay.amount")}><AmountInput value={l.amount} onChange={(v) => set(i, { amount: v })} testId={`voucher-amount-${i}`} invalid={!!l.amount && !!amountRule(l.amount)} /></Field>
          </div>
          <div className="row" style={{ alignItems: "flex-end" }}>
            <Field label={t("expenses.description")} optional><TextInput value={l.description} onChange={(e) => set(i, { description: e.target.value })} maxLength={200} data-testid={`voucher-description-${i}`} /></Field>
            {lines.length > 1 && <IconButton icon={Trash2} label={t("common.delete")} onClick={() => setLines(lines.filter((_, j) => j !== i))} />}
          </div>
          {withholding && (
            <div className="grid-2">
              <Field label={t("exp.withheldKind")} hint={t("exp.withheldKind.hint")} optional>
                <Select value={l.kind} onChange={(e) => set(i, { kind: e.target.value as WithholdingKind | "" })} data-testid={`voucher-wh-kind-${i}`}>
                  <option value="">{t("exp.withheld.none")}</option>
                  <option value="rent">{t("exp.withheld.rent")}</option>
                  <option value="contractor">{t("exp.withheld.contractor")}</option>
                </Select>
              </Field>
              {l.kind && <Field label={t("exp.withheld")}><AmountInput value={l.withheld} onChange={(v) => set(i, { withheld: v })} testId={`voucher-withheld-${i}`} /></Field>}
            </div>
          )}
        </div>
      ))}
      <div className="row"><Button size="sm" icon={Plus} onClick={() => setLines([...lines, emptyLine()])} data-testid="voucher-add-line">{t("exp.addLine")}</Button></div>
    </div>
  );
}

const linesValid = (lines: Line[]) => lines.every((l) => l.account !== null && !amountRule(l.amount) && (!l.kind || (!amountRule(l.withheld, true) && minorOf(l.withheld) <= minorOf(l.amount))));
const toLines = (lines: Line[]) => lines.map((l) => ({ account_id: l.account ?? 0, amount: minorOf(l.amount), description: l.description.trim() || null, withheld: l.kind ? minorOf(l.withheld) : 0, withholding: l.kind || null }));

/**
 * Money paid out: an expense (with its lines and any tax withheld), a payment to a supplier, withheld tax paid
 * to the government, or the owner taking money out.
 */
function VoucherDialog({ purpose, calendar, onClose, onDone }: { purpose: VoucherPurpose; calendar: Cal; onClose: () => void; onDone: () => void }) {
  const { t, lang, err } = useI18n();
  const suppliers = useSuppliers();
  const [account, setAccount] = useState<number | null>(null);
  const [date, setDate] = useState(todayIso());
  const [lines, setLines] = useState<Line[]>([emptyLine()]);
  const [supplier, setSupplier] = useState("");
  const [amount, setAmount] = useState("");
  const [tax, setTax] = useState<WithholdingKind>("rent");
  const [name, setName] = useState("");
  const [reference, setReference] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const isLines = purpose === "expense" || purpose === "other";
  const owed = suppliers.find((s) => s.id === supplier)?.balance ?? 0;
  const valid = account !== null && (isLines ? linesValid(lines) : !amountRule(amount)) && (purpose !== "supplier" || !!supplier);
  const total = isLines ? lines.reduce((s, l) => s + minorOf(l.amount) - (l.kind ? minorOf(l.withheld) : 0), 0) : minorOf(amount);
  const save = async () => {
    if (!valid || account === null) return;
    setBusy(true);
    setError("");
    try {
      await rpc("fin.vouchers.create", {
        date,
        account_id: account,
        purpose,
        supplier_id: supplier || null,
        name: name.trim() || null,
        reference: reference.trim() || null,
        note: null,
        lines: isLines ? toLines(lines) : [],
        amount: isLines ? 0 : minorOf(amount),
        withholding: purpose === "tax" ? tax : null,
      });
      onDone();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t(`exp.dialog.${purpose}`)} description={t(`exp.dialog.${purpose}.hint`)} onClose={onClose} wide={isLines} testId="expense-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Wallet} disabled={!valid} loading={busy} onClick={save} data-testid="expense-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger" testId="expense-error">{error}</Notice>}
        <div className="grid-2">
          <Field label={t("exp.paidFrom")}><MoneyAccountSelect value={account} onChange={setAccount} testId="expense-account" /></Field>
          <Field label={t("doc.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="expense-date" yearsBefore={1} yearsAfter={0} /></Field>
        </div>
        {(purpose === "supplier" || purpose === "expense") && (
          <Field label={t("exp.supplier")} optional={purpose === "expense"} hint={purpose === "supplier" && supplier ? t("exp.supplierOwed").replace("{n}", formatAmount(owed, lang)) : undefined}>
            <Select value={supplier} onChange={(e) => setSupplier(e.target.value)} data-testid="expense-supplier">
              <option value="">{purpose === "supplier" ? t("common.choose") : "—"}</option>
              {suppliers.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
            </Select>
          </Field>
        )}
        {purpose === "tax" && (
          <Segmented<WithholdingKind> value={tax} onChange={setTax} label={t("exp.withheldKind")} block options={[{ value: "rent", label: t("exp.withheld.rent"), testId: "tax-kind-rent" }, { value: "contractor", label: t("exp.withheld.contractor"), testId: "tax-kind-contractor" }]} />
        )}
        {isLines ? (
          <LinesEditor lines={lines} setLines={setLines} withholding />
        ) : (
          <Field label={t("pay.amount")}><AmountInput value={amount} onChange={setAmount} testId="expense-amount" autoFocus /></Field>
        )}
        <div className="grid-2">
          {purpose !== "supplier" && <Field label={t("expenses.payee")} optional><TextInput value={name} onChange={(e) => setName(e.target.value)} maxLength={100} data-testid="expense-payee" /></Field>}
          <Field label={t("fin.reference")} optional hint={t("fin.reference.hint")}><TextInput value={reference} onChange={(e) => setReference(e.target.value)} maxLength={60} data-testid="expense-reference" /></Field>
        </div>
        <dl className="fin-summary"><div className="strong"><dt>{t("exp.moneyOut")}</dt><dd className="num" data-testid="expense-total">{formatAmount(total, lang)}</dd></div></dl>
      </div>
    </Dialog>
  );
}

/** A bill of a supplier or lab: what the clinic now owes, by expense account (paid later by a voucher). */
function BillDialog({ calendar, onClose, onDone }: { calendar: Cal; onClose: () => void; onDone: () => void }) {
  const { t, lang, err } = useI18n();
  const suppliers = useSuppliers();
  const [supplier, setSupplier] = useState("");
  const [date, setDate] = useState(todayIso());
  const [due, setDue] = useState("");
  const [lines, setLines] = useState<Line[]>([emptyLine()]);
  const [reference, setReference] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const valid = !!supplier && linesValid(lines);
  const save = async () => {
    if (!valid) return;
    setBusy(true);
    setError("");
    try {
      await rpc("fin.bills.create", { supplier_id: supplier, date, due_date: due || null, reference: reference.trim() || null, note: null, lines: toLines(lines).map((l) => ({ ...l, withheld: 0, withholding: null })) });
      onDone();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("exp.bill.new")} description={t("exp.bill.hint")} onClose={onClose} wide testId="bill-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={FileInput} disabled={!valid} loading={busy} onClick={save} data-testid="bill-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="grid-2">
          <Field label={t("exp.supplier")}>
            <Select value={supplier} onChange={(e) => setSupplier(e.target.value)} data-testid="bill-supplier">
              <option value="">{t("common.choose")}</option>
              {suppliers.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
            </Select>
          </Field>
          <Field label={t("fin.reference")} optional hint={t("exp.bill.reference")}><TextInput value={reference} onChange={(e) => setReference(e.target.value)} maxLength={60} data-testid="bill-reference" /></Field>
        </div>
        <div className="grid-2">
          <Field label={t("doc.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="bill-date" yearsBefore={1} yearsAfter={0} /></Field>
          <Field label={t("exp.bill.due")} optional><DateField value={due} onChange={setDue} calendar={calendar} label={t("exp.bill.due")} testId="bill-due" yearsBefore={0} yearsAfter={2} /></Field>
        </div>
        <LinesEditor lines={lines} setLines={setLines} withholding={false} />
        <dl className="fin-summary"><div className="strong"><dt>{t("exp.totalBilled")}</dt><dd className="num">{formatAmount(lines.reduce((s, l) => s + minorOf(l.amount), 0), lang)}</dd></div></dl>
      </div>
    </Dialog>
  );
}

/** Money in that is not from a patient: the owner puts money in, a loan, or something sold. */
export function OtherReceiptDialog({ account: initial, calendar, onClose, onDone }: { account: number | null; calendar: Cal; onClose: () => void; onDone: () => void }) {
  const { t, err } = useI18n();
  const [account, setAccount] = useState<number | null>(initial);
  const [source, setSource] = useState<OtherReceiptSource>("capital");
  const [date, setDate] = useState(todayIso());
  const [amount, setAmount] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const valid = account !== null && !amountRule(amount);
  const save = async () => {
    if (!valid || account === null) return;
    setBusy(true);
    setError("");
    try {
      await rpc("fin.other_receipts.create", { date, account_id: account, source, amount: minorOf(amount), name: name.trim() || null, note: null });
      onDone();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("cash.otherIn")} description={t("cash.otherIn.hint")} onClose={onClose} testId="other-receipt-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" disabled={!valid} loading={busy} onClick={save} data-testid="other-receipt-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <Segmented<OtherReceiptSource> value={source} onChange={setSource} label={t("cash.otherIn")} block options={(["capital", "loan", "sale"] as const).map((s) => ({ value: s, label: t(`fin.source.${s}`), testId: `other-source-${s}` }))} />
        <div className="grid-2">
          <Field label={t("pay.amount")}><AmountInput value={amount} onChange={setAmount} testId="other-receipt-amount" autoFocus /></Field>
          <Field label={t("pay.account")}><MoneyAccountSelect value={account} onChange={setAccount} testId="other-receipt-account" /></Field>
        </div>
        <div className="grid-2">
          <Field label={t("doc.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="other-receipt-date" yearsBefore={1} yearsAfter={0} /></Field>
          <Field label={t("pay.payer")} optional><TextInput value={name} onChange={(e) => setName(e.target.value)} maxLength={100} data-testid="other-receipt-name" /></Field>
        </div>
      </div>
    </Dialog>
  );
}
