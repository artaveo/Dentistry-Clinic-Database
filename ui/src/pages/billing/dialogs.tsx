// The money forms of a patient: take money (a receipt put towards invoices, the rest kept as deposit), put the
// deposit on an invoice, refund, set installments, make an invoice — and the "why?" dialog of everything that
// undoes or lowers something (void, price reduction, write-off), which always asks for its reason.
import { useEffect, useMemo, useState } from "react";
import { CalendarClock, Check, HandCoins, Plus, ReceiptText, Trash2, Undo2, Wallet } from "lucide-react";
import type { BillableItem, CalendarSystem, InvoiceInfo, ReceiptInfo, ServiceInfo, ToothNumbering } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { afnInput, amountRule, parseAfn } from "../../lib/billing";
import { addDays, shiftMonth, todayIso } from "../../lib/calendar";
import { serviceName, useCatalog, useClinicSettings } from "../../lib/clinical";
import { digits } from "../../lib/dates";
import { tr } from "../../lib/medical";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { surfacesText, teethLabel } from "../../chart/numbering";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Checkbox, Segmented } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Notice, Loading } from "../../ui/Feedback";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { ServicePicker } from "../patients/clinical/ServicePicker";
import { AmountInput, MoneyAccountSelect, minorOf } from "./common";

// ───────────────────────────── a receipt ─────────────────────────────

/**
 * Money from a patient: put on one open invoice, or on the oldest ones first, or kept as the patient's
 * deposit. The money goes into the chosen money account (cash drawer or bank).
 */
export function ReceiptDialog({ patientId, patientName, invoices, invoiceId, onClose, onDone }: { patientId: string; patientName: string; invoices: InvoiceInfo[]; invoiceId?: number | null; onClose: () => void; onDone: (r: ReceiptInfo) => void }) {
  const { t, lang, err } = useI18n();
  const open = invoices.filter((i) => i.status === "posted" && i.remaining > 0);
  const owed = open.reduce((s, i) => s + i.remaining, 0);
  const first = open.find((i) => i.id === invoiceId) ?? null;
  const [target, setTarget] = useState<string>(first ? String(first.id) : open.length ? "auto" : "deposit");
  const [account, setAccount] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm(
    { amount: first ? afnInput(first.remaining) : open.length ? afnInput(owed) : "", payer: "", note: "" },
    { amount: (s) => amountRule(s) },
  );
  const inv = open.find((i) => String(i.id) === target) ?? null;
  const amount = minorOf(form.values.amount);
  const toInvoices = target === "deposit" ? 0 : Math.min(amount, inv ? inv.remaining : owed);
  const toDeposit = Math.max(0, amount - toInvoices);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate() || account === null) return;
    setBusy(true);
    try {
      const r = await rpc("fin.receipts.create", {
        patient_id: patientId,
        account_id: account,
        amount,
        allocations: inv ? [{ invoice_id: inv.id, amount: Math.min(amount, inv.remaining) }] : [],
        auto_allocate: target === "auto",
        payer_name: form.values.payer.trim() || null,
        note: form.values.note.trim() || null,
      });
      onDone(r);
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("pay.add")}
      description={patientName}
      onClose={onClose}
      testId="payment-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={Wallet} loading={busy} onClick={submit} data-testid="payment-save">{t("pay.save")}</Button>
        </>
      }
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger" testId="payment-error">{error}</Notice>}
        <Field label={t("pay.for")} hint={t("pay.for.hint")}>
          <Select
            value={target}
            onChange={(e) => {
              const v = e.target.value;
              const next = open.find((i) => String(i.id) === v);
              setTarget(v);
              if (next) form.set("amount", afnInput(next.remaining));
              else if (v === "auto") form.set("amount", afnInput(owed));
            }}
            data-testid="payment-target"
          >
            {open.length > 1 && <option value="auto">{t("pay.oldestFirst").replace("{n}", formatAmount(owed, lang))}</option>}
            {open.map((i) => <option key={i.id} value={i.id}>{i.number} — {t("inv.remaining")}: {formatAmount(i.remaining, lang)}</option>)}
            <option value="deposit">{t("pay.advance")}</option>
          </Select>
        </Field>
        <Field label={t("pay.amount")} error={form.error("amount") && t(form.error("amount")!)}>
          <AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="payment-amount" autoFocus />
        </Field>
        <Field label={t("pay.account")} hint={t("pay.account.hint")}>
          <MoneyAccountSelect value={account} onChange={setAccount} testId="payment-account" />
        </Field>
        <div className="grid-2">
          <Field label={t("pay.payer")} optional>
            <TextInput value={form.values.payer} onChange={(e) => form.set("payer", e.target.value)} maxLength={100} data-testid="payment-payer" />
          </Field>
          <Field label={t("pay.note")} optional>
            <TextInput value={form.values.note} onChange={(e) => form.set("note", e.target.value)} maxLength={300} data-testid="payment-note" />
          </Field>
        </div>
        {amount > 0 && (
          <Notice tone={toDeposit > 0 ? "info" : "success"} testId="payment-after">
            {target === "deposit"
              ? t("pay.after.deposit").replace("{n}", formatAmount(amount, lang))
              : toDeposit > 0
                ? t("pay.after.split").replace("{a}", formatAmount(toInvoices, lang)).replace("{b}", formatAmount(toDeposit, lang))
                : t("pay.after.remaining").replace("{n}", formatAmount((inv ? inv.remaining : owed) - toInvoices, lang))}
          </Notice>
        )}
      </form>
    </Dialog>
  );
}

// ───────────────────────────── the deposit on an invoice ─────────────────────────────

/** Puts money the patient already left with the clinic (the deposit) on an open invoice. */
export function UseDepositDialog({ patientId, invoice, deposit, onClose, onDone }: { patientId: string; invoice: InvoiceInfo; deposit: number; onClose: () => void; onDone: (r: ReceiptInfo) => void }) {
  const { t, lang, err } = useI18n();
  const max = Math.min(deposit, invoice.remaining);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm({ amount: afnInput(max) }, { amount: (s) => amountRule(s) ?? (minorOf(s) > max ? "rule.amount_too_high" : null) });
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      onDone(await rpc("fin.receipts.allocate", { patient_id: patientId, allocations: [{ invoice_id: invoice.id, amount: minorOf(form.values.amount) }], reason: null }));
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("pay.useDeposit")} description={`${invoice.number} · ${t("acct.deposit")}: ${formatAmount(deposit, lang)}`} onClose={onClose} testId="deposit-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={HandCoins} loading={busy} onClick={submit} data-testid="deposit-save">{t("pay.useDeposit")}</Button></>}>
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("pay.amount")} hint={t("pay.amount.max").replace("{n}", formatAmount(max, lang))} error={form.error("amount") && t(form.error("amount")!)}>
          <AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="deposit-amount" autoFocus />
        </Field>
      </form>
    </Dialog>
  );
}

// ───────────────────────────── a refund ─────────────────────────────

/**
 * Money handed back to a patient: from the deposit, or (with an invoice) money taken off that invoice first.
 * Always with a reason; a refund cannot be more than what is there.
 */
export function RefundDialog({ patientId, patientName, deposit, invoice, onClose, onDone }: { patientId: string; patientName: string; deposit: number; invoice?: InvoiceInfo | null; onClose: () => void; onDone: (r: ReceiptInfo) => void }) {
  const { t, lang, err } = useI18n();
  const max = invoice ? invoice.allocated : deposit;
  const [account, setAccount] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm(
    { amount: afnInput(invoice ? invoice.allocated : deposit), reason: "" },
    { amount: (s) => amountRule(s) ?? (minorOf(s) > max ? "rule.amount_too_high" : null), reason: (s) => (s.trim() ? null : "rule.required") },
  );
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate() || account === null) return;
    setBusy(true);
    try {
      const amount = minorOf(form.values.amount);
      // From an invoice: that much is taken off the invoice back to the deposit, then handed out.
      const r = await rpc("fin.receipts.refund", { patient_id: patientId, account_id: account, amount, reason: form.values.reason.trim(), from_invoice_id: invoice?.id ?? null });
      onDone(r);
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t("pay.refund")}
      description={`${patientName}${invoice ? ` · ${invoice.number}` : ""}`}
      onClose={onClose}
      testId="refund-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="danger" icon={Undo2} loading={busy} onClick={submit} data-testid="reason-confirm">{t("pay.refund.confirm")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger" testId="reason-error">{error}</Notice>}
        <Notice tone="info">{invoice ? t("pay.refund.fromInvoiceHint") : t("pay.refund.hint")}</Notice>
        <Field label={t("pay.amount")} hint={t("pay.amount.max").replace("{n}", formatAmount(max, lang))} error={form.error("amount") && t(form.error("amount")!)}>
          <AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="reason-amount" />
        </Field>
        <Field label={t("pay.refund.from")}>
          <MoneyAccountSelect value={account} onChange={setAccount} testId="refund-account" />
        </Field>
        <Field label={t("billing.reason")} hint={t("billing.reason.hint")} error={form.error("reason") && t(form.error("reason")!)}>
          <Textarea rows={2} value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="reason-text" />
        </Field>
      </form>
    </Dialog>
  );
}

// ───────────────────────────── "why?" dialogs ─────────────────────────────

/**
 * Asks for the reason of an action that undoes or lowers something — and, for a price reduction or a
 * write-off, the amount. The Core refuses an empty reason; this shows it first (DoD rule 10).
 */
export function ReasonDialog({ title, description, confirm, danger, amount, kinds, onClose, onConfirm, testId = "reason-dialog" }: { title: string; description?: string; confirm: string; danger?: boolean; amount?: { max: number; initial: number }; kinds?: { value: string; label: string }[]; onClose: () => void; onConfirm: (reason: string, amount: number, kind: string) => Promise<void>; testId?: string }) {
  const { t, lang, err } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [kind, setKind] = useState(kinds?.[0]?.value ?? "");
  const form = useForm(
    { reason: "", amount: amount ? afnInput(amount.initial) : "" },
    {
      reason: (s) => (s.trim() ? null : "rule.required"),
      amount: (s) => {
        if (!amount) return null;
        const rule = amountRule(s);
        if (rule) return rule;
        return minorOf(s) > amount.max ? "rule.amount_too_high" : null;
      },
    },
  );
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await onConfirm(form.values.reason.trim(), parseAfn(form.values.amount) ?? 0, kind);
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={title}
      description={description}
      onClose={onClose}
      testId={testId}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant={danger ? "danger" : "primary"} loading={busy} onClick={submit} data-testid="reason-confirm">{confirm}</Button>
        </>
      }
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger" testId="reason-error">{error}</Notice>}
        {kinds && kinds.length > 1 && <Segmented<string> value={kind} onChange={setKind} label={title} block options={kinds.map((k) => ({ value: k.value, label: k.label, testId: `reason-kind-${k.value}` }))} />}
        {amount && (
          <Field label={t("pay.amount")} hint={t("pay.amount.max").replace("{n}", formatAmount(amount.max, lang))} error={form.error("amount") && t(form.error("amount")!)}>
            <AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="reason-amount" />
          </Field>
        )}
        <Field label={t("billing.reason")} hint={t("billing.reason.hint")} error={form.error("reason") && t(form.error("reason")!)}>
          <Textarea rows={3} value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="reason-text" />
        </Field>
      </form>
    </Dialog>
  );
}

// ───────────────────────────── installments ─────────────────────────────

type Row = { due: string; amount: string };

/**
 * An installment plan for what an invoice still owes: an equal split over N months on the same day of the
 * month, or hand-edited rows; they must add up exactly. Changing a plan keeps the old one (with a reason).
 */
export function InstallmentsDialog({ invoice, calendar, onClose, onDone }: { invoice: InvoiceInfo; calendar: CalendarSystem; onClose: () => void; onDone: (i: InvoiceInfo) => void }) {
  const { t, lang, err } = useI18n();
  const existing = invoice.plan;
  const [count, setCount] = useState("3");
  const [rows, setRows] = useState<Row[]>(() => (existing?.installments ?? []).filter((i) => i.state !== "paid").map((i) => ({ due: i.due_date, amount: afnInput(i.amount - i.paid) })));
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const total = rows.reduce((s, r) => s + minorOf(r.amount), 0);
  const split = (n: number) => {
    // FR-14/15: the same day of each month (clinic calendar); the leftover afghanis spread over the first rows.
    const each = Math.floor(invoice.remaining / n / 100) * 100;
    let extra = invoice.remaining - each * n;
    setRows(Array.from({ length: n }, (_, i) => {
      const add = Math.min(extra, 100);
      const last = i === n - 1;
      const amount = last ? each + extra : each + add;
      extra -= last ? extra : add;
      return { due: shiftMonth(todayIso(), calendar, i + 1), amount: afnInput(amount) };
    }));
  };
  const needsReason = !!existing && !reason.trim();
  const valid = rows.length > 0 && total === invoice.remaining && rows.every((r) => parseAfn(r.amount) && r.due) && !needsReason;
  const save = async (clear = false) => {
    setBusy(true);
    setError("");
    try {
      const next = await rpc("fin.invoices.set_plan", { invoice_id: invoice.id, installments: clear ? [] : rows.map((r) => ({ due_date: r.due, amount: parseAfn(r.amount) ?? 0 })), reason: reason.trim() || null });
      onDone(next);
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t("inst.title")}
      description={`${invoice.number} — ${t("inv.remaining")}: ${formatAmount(invoice.remaining, lang)}`}
      onClose={onClose}
      wide
      testId="installments-dialog"
      footer={
        <>
          {existing && <Button variant="subtle" onClick={() => save(true)} disabled={!reason.trim()} loading={busy} data-testid="installments-clear">{t("inst.clear")}</Button>}
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={Check} disabled={!valid} loading={busy} onClick={() => save()} data-testid="installments-save">{t("common.save")}</Button>
        </>
      }
    >
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="row" style={{ alignItems: "flex-end" }}>
          <Field label={t("inst.count")}>
            <Select value={count} onChange={(e) => setCount(e.target.value)} data-testid="installments-count">
              {[2, 3, 4, 5, 6, 8, 9, 10, 12, 18, 24, 36].map((n) => <option key={n} value={n}>{digits(n, lang)}</option>)}
            </Select>
          </Field>
          <Button icon={CalendarClock} onClick={() => split(Number(count))} data-testid="installments-split">{t("inst.split")}</Button>
          <Button icon={Plus} onClick={() => setRows([...rows, { due: addDays(rows.length ? rows[rows.length - 1].due : todayIso(), 30), amount: "" }])} data-testid="installments-add">{t("inst.addRow")}</Button>
        </div>
        <div className="inst-rows" data-testid="installments-rows">
          {rows.map((r, i) => (
            <div key={i} className="inst-row">
              <span className="inst-no">{digits(i + 1, lang)}</span>
              <DateField value={r.due} calendar={calendar} label={t("inst.due")} testId={`installment-due-${i}`} yearsBefore={0} onChange={(due) => setRows(rows.map((x, j) => (j === i ? { ...x, due } : x)))} />
              <AmountInput value={r.amount} onChange={(amount) => setRows(rows.map((x, j) => (j === i ? { ...x, amount } : x)))} testId={`installment-amount-${i}`} />
              <IconButton icon={Trash2} label={t("common.delete")} onClick={() => setRows(rows.filter((_, j) => j !== i))} />
            </div>
          ))}
        </div>
        <Notice tone={total === invoice.remaining && rows.length ? "success" : "warning"} testId="installments-sum">
          {total === invoice.remaining && rows.length ? t("inst.sum.ok") : t("inst.sum.mismatch").replace("{sum}", formatAmount(total, lang)).replace("{n}", formatAmount(invoice.remaining, lang))}
        </Notice>
        {existing && (
          <Field label={t("inst.changeReason")} hint={t("inst.changeReason.hint")}>
            <TextInput value={reason} onChange={(e) => setReason(e.target.value)} maxLength={300} data-testid="installments-reason" />
          </Field>
        )}
      </div>
    </Dialog>
  );
}

// ───────────────────────────── a new invoice ─────────────────────────────

type Pick = { on: boolean; price: string };
type Extra = { service: ServiceInfo | null; description: string; qty: string; price: string };

/**
 * An invoice from the treatments done: tick them (all are ticked), change a price when allowed (audited),
 * add other lines, take a discount, and put the patient's deposit on it at once. Once issued it can only be
 * voided, reduced or written off, each with a reason.
 */
export function InvoiceDialog({ patientId, patientName, perms, deposit = 0, onClose, onCreated }: { patientId: string; patientName: string; perms: { override: boolean }; deposit?: number; onClose: () => void; onCreated: (i: InvoiceInfo) => void }) {
  const { t, lang, err } = useI18n();
  const catalog = useCatalog();
  const settings = useClinicSettings();
  const numbering = (settings?.tooth_numbering ?? "fdi") as ToothNumbering;
  const [items, setItems] = useState<BillableItem[] | null>(null);
  const [picks, setPicks] = useState<Record<string, Pick>>({});
  const [extras, setExtras] = useState<Extra[]>([]);
  const [picking, setPicking] = useState(false);
  const [discount, setDiscount] = useState("");
  const [note, setNote] = useState("");
  const [useDeposit, setUseDeposit] = useState(deposit > 0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    rpc("fin.invoices.billable", { patient_id: patientId })
      .then((v) => {
        setItems(v);
        setPicks(Object.fromEntries(v.map((i) => [i.item_id, { on: true, price: afnInput(i.unit_price) }])));
      })
      .catch((e) => setError(err(e)));
  }, [patientId]);

  const lines = useMemo(() => {
    const fromItems = (items ?? []).filter((i) => picks[i.item_id]?.on).map((i) => minorOf(picks[i.item_id].price) * i.units);
    const fromExtras = extras.map((x) => minorOf(x.price) * Math.max(1, Number(x.qty) || 1));
    return [...fromItems, ...fromExtras];
  }, [items, picks, extras]);
  const subtotal = lines.reduce((s, v) => s + v, 0);
  const disc = minorOf(discount);
  const discountError = discount.trim() ? amountRule(discount, true) ?? (disc > subtotal ? "rule.discount_range" : null) : null;
  const extrasValid = extras.every((x) => parseAfn(x.price) !== null && (x.service || x.description.trim()) && Number(x.qty) >= 1);
  const canSave = lines.length > 0 && !discountError && extrasValid;
  const net = Math.max(0, subtotal - (discountError ? 0 : disc));

  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const inv = await rpc("fin.invoices.create", {
        patient_id: patientId,
        items: (items ?? []).filter((i) => picks[i.item_id]?.on).map((i) => ({ item_id: i.item_id, unit_price: parseAfn(picks[i.item_id].price), discount: 0 })),
        extra_lines: extras.map((x) => ({ service_id: x.service?.id ?? null, description: x.service ? null : x.description.trim(), teeth: [], quantity: Math.max(1, Number(x.qty) || 1), unit_price: parseAfn(x.price) ?? 0, discount: 0, doctor_id: null })),
        discount: disc,
        note: note.trim() || null,
        use_deposit: useDeposit,
        installments: [],
      });
      onCreated(inv);
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("inv.new")}
      description={patientName}
      onClose={onClose}
      wide
      testId="invoice-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={ReceiptText} disabled={!canSave} loading={busy} onClick={save} data-testid="invoice-save">{t("inv.issue")}</Button>
        </>
      }
    >
      <div className="stack">
        {error && <Notice tone="danger" testId="invoice-error">{error}</Notice>}
        {!items && !error && <Loading />}
        {items && (
          <div className="inv-pick" data-testid="invoice-billable">
            <div className="inv-pick-head">
              <span className="field-label">{t("inv.fromTreatments")}</span>
              {items.length > 0 && (
                <Button size="sm" variant="link" onClick={() => setPicks(Object.fromEntries(items.map((i) => [i.item_id, { ...picks[i.item_id], on: !items.every((x) => picks[x.item_id]?.on) }])))}>
                  {items.every((i) => picks[i.item_id]?.on) ? t("inv.untickAll") : t("inv.tickAll")}
                </Button>
              )}
            </div>
            {items.length === 0 && <p className="subtle" data-testid="invoice-none-ready">{t("inv.noneReady")}</p>}
            {items.map((i) => {
              const p = picks[i.item_id];
              if (!p) return null;
              const changed = minorOf(p.price) !== i.unit_price;
              return (
                <div key={i.item_id} className={`inv-pick-row ${p.on ? "" : "off"}`} data-testid={`invoice-item-${i.item_id}`}>
                  <Checkbox checked={p.on} onChange={(on) => setPicks({ ...picks, [i.item_id]: { ...p, on } })}>
                    <span className="cell-strong">{tr(i.names, lang)}</span>{" "}
                    <bdi className="subtle">{[teethLabel(i.teeth, numbering, lang), surfacesText(i.surfaces, i.teeth[0])].filter(Boolean).join(" ")}</bdi>
                    {i.units > 1 && <span className="subtle"> × {digits(i.units, lang)}</span>}
                  </Checkbox>
                  <div className="inv-pick-price">
                    <AmountInput value={p.price} onChange={(price) => setPicks({ ...picks, [i.item_id]: { ...p, price } })} testId={`invoice-price-${i.item_id}`} invalid={parseAfn(p.price) === null} />
                    {changed && <span className="t-caption subtle">{perms.override ? t("inv.priceChanged") : t("inv.priceNeedsPermission")}</span>}
                  </div>
                </div>
              );
            })}
          </div>
        )}
        {extras.map((x, i) => (
          <div key={i} className="inv-extra" data-testid={`invoice-extra-${i}`}>
            {x.service ? <span className="cell-strong">{serviceName(x.service, catalog, lang)}</span> : <TextInput placeholder={t("inv.extra.description")} value={x.description} onChange={(e) => setExtras(extras.map((y, j) => (j === i ? { ...y, description: e.target.value } : y)))} maxLength={120} data-testid={`invoice-extra-desc-${i}`} />}
            <TextInput inputMode="numeric" dir="ltr" value={x.qty} onChange={(e) => setExtras(extras.map((y, j) => (j === i ? { ...y, qty: e.target.value } : y)))} aria-label={t("inv.col.qty")} data-testid={`invoice-extra-qty-${i}`} />
            <AmountInput value={x.price} onChange={(price) => setExtras(extras.map((y, j) => (j === i ? { ...y, price } : y)))} testId={`invoice-extra-price-${i}`} invalid={x.price !== "" && parseAfn(x.price) === null} />
            <IconButton icon={Trash2} label={t("common.delete")} onClick={() => setExtras(extras.filter((_, j) => j !== i))} />
          </div>
        ))}
        {picking ? (
          <ServicePicker catalog={catalog} selectedTeeth={[]} value={null} testId="invoice-service-picker" onPick={(s) => { setExtras([...extras, { service: s, description: "", qty: "1", price: afnInput(s.price) }]); setPicking(false); }} />
        ) : (
          <div className="row">
            <Button size="sm" icon={Plus} onClick={() => setPicking(true)} data-testid="invoice-add-service">{t("inv.addService")}</Button>
            <Button size="sm" icon={Plus} onClick={() => setExtras([...extras, { service: null, description: "", qty: "1", price: "" }])} data-testid="invoice-add-free">{t("inv.addFree")}</Button>
          </div>
        )}
        <div className="grid-2">
          <Field label={t("inv.discount")} optional hint={t("inv.discount.hint")} error={discountError && t(discountError)}>
            <AmountInput value={discount} onChange={setDiscount} testId="invoice-discount" />
          </Field>
          <Field label={t("inv.note")} optional>
            <TextInput value={note} onChange={(e) => setNote(e.target.value)} maxLength={500} data-testid="invoice-note" />
          </Field>
        </div>
        {deposit > 0 && (
          <Checkbox checked={useDeposit} onChange={setUseDeposit} testId="invoice-use-deposit">
            {t("inv.useDeposit").replace("{n}", formatAmount(Math.min(deposit, net), lang))}
          </Checkbox>
        )}
        <dl className="fin-summary" data-testid="invoice-summary">
          <div><dt>{t("inv.subtotal")}</dt><dd className="num">{formatAmount(subtotal, lang)}</dd></div>
          {disc > 0 && !discountError && <div><dt>{t("inv.discount")}</dt><dd className="num">− {formatAmount(disc, lang)}</dd></div>}
          <div className="strong"><dt>{t("inv.net")}</dt><dd className="num" data-testid="invoice-net">{formatAmount(net, lang)}</dd></div>
        </dl>
        <Notice tone="info">{t("inv.finalNote")}</Notice>
      </div>
    </Dialog>
  );
}
