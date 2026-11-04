// «خریدها» — goods bought from a supplier (8.2): each purchase receives its lines into stock as batches
// and adds to what the clinic owes the supplier (8.5). A purchase is never edited; a mistake is a void.
import { useEffect, useMemo, useState } from "react";
import { Ban, Plus, ShoppingCart, Trash2 } from "lucide-react";
import type { CalendarSystem, PurchaseInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { amountRule, parseAfn, usePaymentMethods } from "../../lib/billing";
import { addDays, todayIso } from "../../lib/calendar";
import { digits, formatDate } from "../../lib/dates";
import { formatQty, parseQty, purchaseQ, qtyRule, useStockLookup } from "../../lib/inventory";
import { useLive } from "../../lib/lan";
import { tr } from "../../lib/medical";
import { formatAmount } from "../../lib/money";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, Select, TextInput } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { AmountInput } from "../billing/common";
import { ReasonDialog } from "../billing/dialogs";
import { ExpiryInput, QtyInput, useAction, useItemOptions, useSupplierOptions } from "./common";
import type { InventoryPerms } from "./common";

export function PurchasesTab({ perms, calendar }: { perms: InventoryPerms; calendar: CalendarSystem }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const suppliers = useSupplierOptions("supplier");
  const [from, setFrom] = useState(addDays(todayIso(), -90));
  const [to, setTo] = useState(todayIso());
  const [supplier, setSupplier] = useState("");
  const [rows, setRows] = useState<PurchaseInfo[] | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const [adding, setAdding] = useState(false);
  const [open, setOpen] = useState<PurchaseInfo | null>(null);
  const reload = () => setTick((x) => x + 1);
  useLive(["purchases", "stock", "supplier_payments"], reload);
  useEffect(() => {
    if (from > to) return;
    rpc("purchases.list", purchaseQ({ from, to, supplier_id: supplier || null }))
      .then((r) => {
        setRows(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [from, to, supplier, tick]);
  if (error && !rows) return <ErrorState message={error} onRetry={reload} />;
  const money = (v: number) => formatAmount(v, lang);
  return (
    <div className="stack" data-testid="purchases-tab">
      <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
        <Field label={t("period.from")}><DateField value={from} onChange={setFrom} calendar={calendar} label={t("period.from")} testId="purchases-from" yearsBefore={3} yearsAfter={0} /></Field>
        <Field label={t("period.to")}><DateField value={to} onChange={setTo} calendar={calendar} label={t("period.to")} testId="purchases-to" yearsBefore={3} yearsAfter={0} /></Field>
        <Field label={t("stock.supplier")}>
          <Select value={supplier} onChange={(e) => setSupplier(e.target.value)} data-testid="purchases-supplier">
            <option value="">{t("purchase.allSuppliers")}</option>
            {suppliers.list?.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </Select>
        </Field>
        {from > to && <Notice tone="warning">{t("rule.period_range")}</Notice>}
      </div>
      <Card flush>
        <CardHeader
          icon={ShoppingCart}
          title={t("inventory.tab.purchases")}
          description={t("purchase.hint")}
          actions={perms.manage ? <Button variant="primary" icon={Plus} onClick={() => setAdding(true)} data-testid="purchase-add-open">{t("purchase.new")}</Button> : undefined}
        />
        <div className="table-wrap">
          <table className="table" data-testid="purchases-table">
            <thead>
              <tr>
                <th>{t("purchase.number")}</th>
                <th>{t("doc.date")}</th>
                <th>{t("stock.supplier")}</th>
                <th className="num">{t("purchase.lines")}</th>
                <th className="num">{t("purchase.total")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {!rows && <SkeletonRows cols={6} />}
              {rows?.map((p) => (
                <tr key={p.id} data-testid={`purchase-${p.id}`}>
                  <td>
                    <button type="button" className="link-btn plain" onClick={() => setOpen(p)} data-testid={`purchase-open-${p.id}`}><bdi className="ltr num">{p.number}</bdi></button>
                    {p.void && <> <Badge tone="neutral" icon={Ban}>{t("purchase.void")}</Badge></>}
                  </td>
                  <td>{formatDate(p.local_date, lang, calendar)}</td>
                  <td>{p.supplier_name}</td>
                  <td className="num">{digits(p.lines.length, lang)}</td>
                  <td className={`num cell-strong ${p.void ? "strike" : ""}`}>{money(p.total)}</td>
                  <td />
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {rows && !rows.length && <EmptyState icon={ShoppingCart} title={t("purchase.none")}>{t("purchase.noneHint")}</EmptyState>}
      </Card>
      {adding && <PurchaseDialog calendar={calendar} perms={perms} onClose={() => setAdding(false)} onDone={() => { setAdding(false); toast.success(t("purchase.saved")); reload(); }} />}
      {open && <PurchaseDetail purchase={open} perms={perms} calendar={calendar} onClose={() => setOpen(null)} onVoided={() => { setOpen(null); toast.success(t("purchase.voided")); reload(); }} />}
    </div>
  );
}

// ───────────────────────────── a purchase ─────────────────────────────

function PurchaseDetail({ purchase, perms, calendar, onClose, onVoided }: { purchase: PurchaseInfo; perms: InventoryPerms; calendar: CalendarSystem; onClose: () => void; onVoided: () => void }) {
  const { t, lang } = useI18n();
  const [voiding, setVoiding] = useState(false);
  const money = (v: number) => formatAmount(v, lang);
  return (
    <Dialog
      title={purchase.number}
      description={`${purchase.supplier_name} · ${formatDate(purchase.local_date, lang, calendar)}`}
      onClose={onClose}
      wide
      testId="purchase-detail"
      footer={perms.manage && !purchase.void ? <Button variant="danger" icon={Ban} onClick={() => setVoiding(true)} data-testid="purchase-void-open">{t("purchase.voidAction")}</Button> : undefined}
    >
      <div className="stack">
        {purchase.void && <Notice tone="warning" title={t("purchase.void")}>{purchase.void_reason}</Notice>}
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th>{t("stock.col.item")}</th>
                <th className="num">{t("stock.qty")}</th>
                <th className="num">{t("stock.unitCost")}</th>
                <th>{t("stock.lot")}</th>
                <th>{t("stock.expiry")}</th>
                <th>{t("stock.place")}</th>
                <th className="num">{t("purchase.lineTotal")}</th>
              </tr>
            </thead>
            <tbody>
              {purchase.lines.map((l) => (
                <tr key={l.id}>
                  <td>{tr(l.item_name, lang)}</td>
                  <td className="num">{formatQty(l.qty, lang)} <span className="t-caption subtle">{tr(l.unit, lang)}</span></td>
                  <td className="num">{money(l.unit_cost)}</td>
                  <td><bdi className="ltr">{l.lot_no ?? "—"}</bdi></td>
                  <td>{l.expiry_date ? formatDate(l.expiry_date, lang, calendar) : "—"}</td>
                  <td>{tr(l.location, lang)}</td>
                  <td className="num cell-strong">{money(l.line_total)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <dl className="sum">
          <div className="sum-row strong"><dt>{t("purchase.total")}</dt><dd className="num">{money(purchase.total)}</dd></div>
        </dl>
        {purchase.note && <p className="muted">{purchase.note}</p>}
      </div>
      {voiding && (
        <ReasonDialog
          title={t("purchase.voidAction")}
          description={t("purchase.voidHint")}
          confirm={t("purchase.voidAction")}
          danger
          onClose={() => setVoiding(false)}
          onConfirm={async (reason) => {
            await rpc("purchases.void", { id: purchase.id, version: purchase.version, reason });
            onVoided();
          }}
        />
      )}
    </Dialog>
  );
}

type Line = { item: string; qty: string; cost: string; lot: string; expiry: string | null; place: string };
const emptyLine = (place: string): Line => ({ item: "", qty: "", cost: "", lot: "", expiry: null, place });

function PurchaseDialog({ calendar, perms, onClose, onDone }: { calendar: CalendarSystem; perms: InventoryPerms; onClose: () => void; onDone: () => void }) {
  const { t, lang } = useI18n();
  const items = useItemOptions();
  const places = useStockLookup("locations");
  const suppliers = useSupplierOptions("supplier");
  const methods = usePaymentMethods();
  const [supplier, setSupplier] = useState("");
  const [date, setDate] = useState(todayIso());
  const [note, setNote] = useState("");
  const [lines, setLines] = useState<Line[]>([emptyLine("")]);
  const [paid, setPaid] = useState("");
  const [method, setMethod] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const { busy, error, run } = useAction();
  const [fieldError, setFieldError] = useState<{ field: string; message: string } | null>(null);

  useEffect(() => {
    if (!supplier && suppliers.list?.length) setSupplier(suppliers.list[0].id);
  }, [suppliers.list]);
  const firstPlace = places.items?.[0]?.id ?? "";
  useEffect(() => {
    if (firstPlace) setLines((ls) => ls.map((l) => (l.place ? l : { ...l, place: firstPlace })));
  }, [firstPlace]);
  useEffect(() => {
    if (!method && methods?.length) setMethod((methods.find((m) => m.is_active && m.is_cash) ?? methods.find((m) => m.is_active))?.id ?? "");
  }, [methods]);

  const patch = (i: number, p: Partial<Line>) => setLines((ls) => ls.map((l, k) => (k === i ? { ...l, ...p } : l)));
  const lineRule = (l: Line) => (!l.item ? "rule.required" : qtyRule(l.qty) ?? (l.cost.trim() ? amountRule(l.cost, true) : "rule.required"));
  const total = useMemo(() => lines.reduce((sum, l) => sum + Math.round(((parseQty(l.qty) ?? 0) * (parseAfn(l.cost) ?? 0)) / 100), 0), [lines]);
  const paidMinor = parseAfn(paid) ?? 0;
  const paidRule = paid.trim() ? amountRule(paid, true) ?? (paidMinor > total ? "rule.payment_exceeds" : null) : null;
  const invalid = !supplier || lines.some((l) => lineRule(l)) || !!paidRule;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitted(true);
    setFieldError(null);
    if (invalid) return;
    void run(
      async () => {
        await rpc("purchases.create", {
          supplier_id: supplier,
          date,
          note: note.trim() || null,
          lines: lines.map((l) => ({ item_id: l.item, qty: parseQty(l.qty) ?? 0, unit_cost: parseAfn(l.cost) ?? 0, lot_no: l.lot.trim() || null, expiry_date: l.expiry, location_id: l.place })),
          paid: paidMinor,
          method_id: paidMinor > 0 ? method || null : null,
        });
        onDone();
      },
      (x) => {
        const f = x as { field?: string | null; rule?: string | null };
        if (!f.field) return false;
        setFieldError({ field: f.field, message: t(f.rule ? `rule.${f.rule}` : "error.validation") });
        return true;
      },
    );
  };
  const showRule = (l: Line) => (submitted ? lineRule(l) : null);
  return (
    <Dialog
      title={t("purchase.new")}
      description={t("purchase.newHint")}
      onClose={onClose}
      wide
      testId="purchase-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={ShoppingCart} loading={busy} onClick={submit} data-testid="purchase-save">{t("purchase.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        {fieldError && <Notice tone="danger" testId="purchase-field-error">{fieldError.message}</Notice>}
        <div className="grid-2">
          <Field label={t("stock.supplier")} error={submitted && !supplier ? t("purchase.needSupplier") : null}>
            <Select value={supplier} onChange={(e) => setSupplier(e.target.value)} data-testid="purchase-supplier">
              <option value="">{t("purchase.pickSupplier")}</option>
              {suppliers.list?.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
            </Select>
          </Field>
          <Field label={t("doc.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="purchase-date" yearsBefore={1} yearsAfter={0} /></Field>
        </div>

        <div className="po-lines" data-testid="purchase-lines">
          {lines.map((l, i) => {
            const rule = showRule(l);
            const item = items?.find((x) => x.id === l.item);
            return (
              <fieldset className="po-line" key={i} data-testid={`purchase-line-${i}`}>
                <legend className="t-caption subtle">{t("purchase.line")} {digits(i + 1, lang)}</legend>
                <Field label={t("stock.col.item")} error={submitted && !l.item ? t("rule.required") : null}>
                  <Select value={l.item} onChange={(e) => patch(i, { item: e.target.value })} data-testid={`purchase-item-${i}`}>
                    <option value="">{t("stock.pick.item")}</option>
                    {items?.map((x) => <option key={x.id} value={x.id}>{tr(x.name, lang)}</option>)}
                  </Select>
                </Field>
                <Field label={t("stock.qty")} error={submitted && l.item && qtyRule(l.qty) ? t(qtyRule(l.qty)!) : null}>
                  <QtyInput value={l.qty} onChange={(v) => patch(i, { qty: v })} unit={item ? tr(item.unit, lang) : undefined} testId={`purchase-qty-${i}`} />
                </Field>
                <Field label={t("stock.unitCost")} error={submitted && l.item && !qtyRule(l.qty) && rule ? t(rule) : null}>
                  <AmountInput value={l.cost} onChange={(v) => patch(i, { cost: v })} testId={`purchase-cost-${i}`} />
                </Field>
                <Field label={t("stock.lot")} optional><TextInput value={l.lot} onChange={(e) => patch(i, { lot: e.target.value })} maxLength={60} dir="ltr" data-testid={`purchase-lot-${i}`} /></Field>
                <Field label={t("stock.place")}>
                  <Select value={l.place} onChange={(e) => patch(i, { place: e.target.value })} data-testid={`purchase-place-${i}`}>
                    {places.items?.map((p) => <option key={p.id} value={p.id}>{tr(p.name, lang)}</option>)}
                  </Select>
                </Field>
                <Field label={t("stock.expiry")} optional><ExpiryInput value={l.expiry} onChange={(v) => patch(i, { expiry: v })} calendar={calendar} label={t("stock.expiry")} testId={`purchase-expiry-${i}`} /></Field>
                {lines.length > 1 && <IconButton icon={Trash2} label={t("purchase.removeLine")} className="po-remove" onClick={() => setLines((ls) => ls.filter((_, k) => k !== i))} data-testid={`purchase-remove-${i}`} />}
              </fieldset>
            );
          })}
        </div>
        <div>
          <Button icon={Plus} onClick={() => setLines((ls) => [...ls, emptyLine(firstPlace)])} data-testid="purchase-add-line">{t("purchase.addLine")}</Button>
        </div>

        <Field label={t("purchase.note")} optional><TextInput value={note} onChange={(e) => setNote(e.target.value)} maxLength={300} data-testid="purchase-note" /></Field>
        <dl className="sum po-total"><div className="sum-row strong"><dt>{t("purchase.total")}</dt><dd className="num" data-testid="purchase-total">{formatAmount(total, lang)}</dd></div></dl>
        {perms.billingEdit && (
          <div className="grid-2">
            <Field label={t("purchase.paidNow")} hint={t("purchase.paidHint")} error={submitted && paidRule ? t(paidRule) : fieldError?.field === "paid" ? fieldError.message : null} optional>
              <AmountInput value={paid} onChange={setPaid} testId="purchase-paid" />
            </Field>
            {paidMinor > 0 && (
              <Field label={t("pay.method")}>
                <Select value={method} onChange={(e) => setMethod(e.target.value)} data-testid="purchase-method">
                  {methods?.filter((m) => m.is_active).map((m) => <option key={m.id} value={m.id}>{tr(m.name, lang)}</option>)}
                </Select>
              </Field>
            )}
          </div>
        )}
      </form>
    </Dialog>
  );
}
