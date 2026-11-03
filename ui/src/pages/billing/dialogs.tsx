// The small money forms: take a payment, make an invoice, set installments, and the "why?" dialogs of
// everything that undoes something (void, credit note, refund, delete an expense) — each one asks for its reason.
import { useEffect, useMemo, useState } from "react";
import { CalendarClock, Check, Plus, ReceiptText, Trash2, Wallet } from "lucide-react";
import type { BillableItem, CalendarSystem, InvoiceInfo, PaymentInfo, ServiceInfo, ToothNumbering } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { afnInput, amountRule, parseAfn, usePaymentMethods } from "../../lib/billing";
import { addDays, todayIso } from "../../lib/calendar";
import { serviceName, useCatalog, useClinicSettings } from "../../lib/clinical";
import { digits } from "../../lib/dates";
import { tr } from "../../lib/medical";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { surfacesText, teethLabel } from "../../chart/numbering";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Checkbox } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Notice, Loading } from "../../ui/Feedback";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { ServicePicker } from "../patients/clinical/ServicePicker";
import { AmountInput, minorOf } from "./common";

// ───────────────────────────── a payment ─────────────────────────────

/**
 * Takes a payment (6.3): for an open invoice — all of it or part — or as an advance that becomes the patient's
 * credit; with money by any method, or from the credit the patient already has.
 */
export function PaymentDialog({ patientId, patientName, invoices, invoiceId, credit, onClose, onDone }: { patientId: string; patientName: string; invoices: InvoiceInfo[]; invoiceId?: string | null; credit: number; onClose: () => void; onDone: (p: PaymentInfo) => void }) {
  const { t, lang, err } = useI18n();
  const methods = usePaymentMethods();
  const open = invoices.filter((i) => i.status === "issued" && i.remaining > 0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const first = open.find((i) => i.id === invoiceId) ?? (invoiceId ? undefined : open[0]);
  const form = useForm(
    { target: first?.id ?? "advance", amount: first ? afnInput(first.remaining) : "", method: "", useCredit: "no", note: "" },
    {
      amount: (s, all) => {
        const rule = amountRule(s);
        if (rule) return rule;
        const n = minorOf(s);
        const inv = open.find((i) => i.id === all.target);
        if (inv && n > inv.remaining) return "rule.payment_exceeds";
        if (all.useCredit === "yes" && n > credit) return "rule.insufficient_credit";
        return null;
      },
      method: (s, all) => (all.useCredit === "yes" || s ? null : "rule.required"),
    },
  );
  const active = (methods ?? []).filter((m) => m.is_active);
  useEffect(() => {
    if (!form.values.method && active.length) form.set("method", (active.find((m) => m.code === "cash") ?? active[0]).id);
  }, [methods]);
  const inv = open.find((i) => i.id === form.values.target);
  const amount = minorOf(form.values.amount);
  const useCredit = form.values.useCredit === "yes" && !!inv;

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      const p = await rpc("payments.add", {
        patient_id: patientId,
        invoice_id: inv?.id ?? null,
        amount: parseAfn(form.values.amount) ?? 0,
        method_id: useCredit ? null : form.values.method,
        use_credit: useCredit,
        note: form.values.note.trim() || null,
      });
      onDone(p);
    } catch (x) {
      if (!form.serverError(x, { amount: "amount", method_id: "method" })) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  const after = inv ? Math.max(0, inv.remaining - amount) : null;
  const eligibleCredit = credit > 0 && !!inv;
  return (
    <Dialog
      title={t("pay.add")}
      description={patientName}
      onClose={onClose}
      testId="payment-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={Wallet} loading={busy} onClick={submit} data-testid="payment-save">{useCredit ? t("pay.useCredit") : t("pay.save")}</Button>
        </>
      }
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger" testId="payment-error">{error}</Notice>}
        <Field label={t("pay.for")} hint={t("pay.for.hint")}>
          <Select
            value={form.values.target}
            onChange={(e) => {
              const next = open.find((i) => i.id === e.target.value);
              form.set("target", e.target.value);
              form.set("amount", next ? afnInput(next.remaining) : "");
              if (!next) form.set("useCredit", "no");
            }}
            data-testid="payment-target"
          >
            {open.map((i) => <option key={i.id} value={i.id}>{i.number} — {t("inv.remaining")}: {formatAmount(i.remaining, lang)}</option>)}
            <option value="advance">{t("pay.advance")}</option>
          </Select>
        </Field>
        <Field label={t("pay.amount")} hint={inv ? t("pay.amount.hintInvoice").replace("{n}", formatAmount(inv.remaining, lang)) : t("pay.amount.hintAdvance")} error={form.error("amount") && t(form.error("amount")!)}>
          <AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="payment-amount" autoFocus />
        </Field>
        {inv && inv.remaining > 0 && minorOf(form.values.amount) !== inv.remaining && (
          <div className="row">
            <Button size="sm" variant="link" onClick={() => form.set("amount", afnInput(inv.remaining))} data-testid="payment-all">{t("pay.payAll")}</Button>
          </div>
        )}
        {eligibleCredit && (
          <Checkbox checked={form.values.useCredit === "yes"} onChange={(v) => form.set("useCredit", v ? "yes" : "no")} testId="payment-use-credit">
            {t("pay.fromCredit").replace("{n}", formatAmount(credit, lang))}
          </Checkbox>
        )}
        {!useCredit && (
          <Field label={t("pay.method")} error={form.error("method") && t(form.error("method")!)}>
            <Select value={form.values.method} onChange={(e) => form.set("method", e.target.value)} data-testid="payment-method">
              {active.map((m) => <option key={m.id} value={m.id}>{tr(m.name, lang)}</option>)}
            </Select>
          </Field>
        )}
        <Field label={t("pay.note")} optional>
          <TextInput value={form.values.note} onChange={(e) => form.set("note", e.target.value)} maxLength={300} data-testid="payment-note" />
        </Field>
        {after !== null && amount > 0 && (
          <Notice tone={after === 0 ? "success" : "info"} testId="payment-after">
            {after === 0 ? t("pay.after.settled") : t("pay.after.remaining").replace("{n}", formatAmount(after, lang))}
          </Notice>
        )}
      </form>
    </Dialog>
  );
}

// ───────────────────────────── "why?" dialogs ─────────────────────────────

/**
 * Asks for the reason of an action that undoes something — and, for a credit note or a refund, the amount.
 * The Core refuses an empty reason; this shows it first (DoD rule 10).
 */
export function ReasonDialog({ title, description, confirm, danger, amount, creditOption, onClose, onConfirm, testId = "reason-dialog" }: { title: string; description?: string; confirm: string; danger?: boolean; amount?: { max: number; initial: number }; creditOption?: boolean; onClose: () => void; onConfirm: (reason: string, amount: number, toCredit: boolean) => Promise<void>; testId?: string }) {
  const { t, lang, err } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm(
    { reason: "", amount: amount ? afnInput(amount.initial) : "", toCredit: "no" },
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
      await onConfirm(form.values.reason.trim(), parseAfn(form.values.amount) ?? 0, form.values.toCredit === "yes");
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
        {amount && (
          <Field label={t("pay.amount")} hint={t("pay.amount.max").replace("{n}", formatAmount(amount.max, lang))} error={form.error("amount") && t(form.error("amount")!)}>
            <AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="reason-amount" />
          </Field>
        )}
        <Field label={t("billing.reason")} hint={t("billing.reason.hint")} error={form.error("reason") && t(form.error("reason")!)}>
          <Textarea rows={3} value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="reason-text" />
        </Field>
        {creditOption && <Checkbox checked={form.values.toCredit === "yes"} onChange={(v) => form.set("toCredit", v ? "yes" : "no")} testId="refund-to-credit">{t("refund.toCredit")}</Checkbox>}
      </form>
    </Dialog>
  );
}

// ───────────────────────────── installments ─────────────────────────────

type Row = { due: string; amount: string };

/**
 * An installment plan for what an invoice still owes (6.3): an equal split over N months, or hand-edited
 * rows; they must add up exactly. A due or overdue installment shows in the reminder lists.
 */
export function InstallmentsDialog({ invoice, calendar, onClose, onDone }: { invoice: InvoiceInfo; calendar: CalendarSystem; onClose: () => void; onDone: (i: InvoiceInfo) => void }) {
  const { t, lang, err } = useI18n();
  const [count, setCount] = useState("3");
  const [rows, setRows] = useState<Row[]>(() => invoice.installments.filter((i) => i.state !== "paid").map((i) => ({ due: i.due_date, amount: afnInput(i.amount - i.paid) })));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const total = rows.reduce((s, r) => s + minorOf(r.amount), 0);
  const split = (n: number) => {
    const each = Math.floor(invoice.remaining / n / 100) * 100;
    setRows(Array.from({ length: n }, (_, i) => ({ due: addDays(todayIso(), 30 * (i + 1)), amount: afnInput(i === n - 1 ? invoice.remaining - each * (n - 1) : each) })));
  };
  const valid = rows.length > 0 && total === invoice.remaining && rows.every((r) => parseAfn(r.amount) && r.due);
  const save = async (clear = false) => {
    setBusy(true);
    setError("");
    try {
      const next = await rpc("invoices.set_installments", { invoice_id: invoice.id, installments: clear ? [] : rows.map((r) => ({ due_date: r.due, amount: parseAfn(r.amount) ?? 0 })) });
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
          {invoice.installments.length > 0 && <Button variant="subtle" onClick={() => save(true)} loading={busy} data-testid="installments-clear">{t("inst.clear")}</Button>}
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
              {[2, 3, 4, 5, 6, 9, 12].map((n) => <option key={n} value={n}>{digits(n, lang)}</option>)}
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
        <Notice tone={valid ? "success" : "warning"} testId="installments-sum">
          {valid ? t("inst.sum.ok") : t("inst.sum.mismatch").replace("{sum}", formatAmount(total, lang)).replace("{n}", formatAmount(invoice.remaining, lang))}
        </Notice>
      </div>
    </Dialog>
  );
}

// ───────────────────────────── a new invoice ─────────────────────────────

type Pick = { on: boolean; price: string };
type Extra = { service: ServiceInfo | null; description: string; qty: string; price: string };

/**
 * An invoice from the treatments done (6.2): tick them (all are ticked), change a price when allowed
 * (6.1, audited), add other lines, take a discount. Once issued it can only be voided or credited.
 */
export function InvoiceDialog({ patientId, patientName, perms, onClose, onCreated }: { patientId: string; patientName: string; perms: { override: boolean }; onClose: () => void; onCreated: (i: InvoiceInfo) => void }) {
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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    rpc("invoices.billable", { patient_id: patientId })
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

  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const inv = await rpc("invoices.create", {
        patient_id: patientId,
        items: (items ?? []).filter((i) => picks[i.item_id]?.on).map((i) => ({ item_id: i.item_id, unit_price: parseAfn(picks[i.item_id].price) })),
        extra_lines: extras.map((x) => ({ service_id: x.service?.id ?? null, description: x.service ? null : x.description.trim(), teeth: [], quantity: Math.max(1, Number(x.qty) || 1), unit_price: parseAfn(x.price) ?? 0, doctor_id: null })),
        discount: disc,
        note: note.trim() || null,
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
        <dl className="fin-summary" data-testid="invoice-summary">
          <div><dt>{t("inv.subtotal")}</dt><dd className="num">{formatAmount(subtotal, lang)}</dd></div>
          {disc > 0 && !discountError && <div><dt>{t("inv.discount")}</dt><dd className="num">− {formatAmount(disc, lang)}</dd></div>}
          <div className="strong"><dt>{t("inv.net")}</dt><dd className="num" data-testid="invoice-net">{formatAmount(Math.max(0, subtotal - (discountError ? 0 : disc)), lang)}</dd></div>
        </dl>
        <Notice tone="info">{t("inv.finalNote")}</Notice>
      </div>
    </Dialog>
  );
}

