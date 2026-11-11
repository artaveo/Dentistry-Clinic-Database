// The forms of the stock screen: an item, receiving stock, taking it out, and the three corrections of
// one batch — adjust, move to another place, return to the supplier.
import { useEffect, useState } from "react";
import { ArrowLeftRight, ChevronDown, ChevronUp, Minus, PackagePlus, Plus, Save, Undo2 } from "lucide-react";
import type { CalendarSystem, StockBatchInfo, StockItemInfo, Translations } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { amountRule, parseAfn } from "../../lib/billing";
import { todayIso } from "../../lib/calendar";
import { formatQty, parseQty, qtyInput, qtyRule, useStockLookup } from "../../lib/inventory";
import { tr } from "../../lib/medical";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Segmented, Switch } from "../../ui/Controls";
import { Combobox } from "../../ui/Combobox";
import { DateField } from "../../ui/DateField";
import { Notice } from "../../ui/Feedback";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { TranslationsField } from "../../ui/TranslationsField";
import { AmountInput } from "../billing/common";
import { QtyInput, useAction, useItemOptions, useSupplierOptions } from "./common";

const required = (s: string) => (s.trim() ? null : "rule.required");

// ───────────────────────────── an item ─────────────────────────────

export function ItemFormDialog({ item, onClose, onDone }: { item?: StockItemInfo; onClose: () => void; onDone: (i: StockItemInfo) => void }) {
  const { t, lang } = useI18n();
  const categories = useStockLookup("categories", !!item);
  const units = useStockLookup("units", !!item);
  const [names, setNames] = useState<Translations>(item?.name ?? { fa: "", ps: "", en: "" });
  const [active, setActive] = useState(item?.is_active ?? true);
  const { busy, error, run } = useAction();
  // `name` carries what is typed in any language, so the same live check as every other field applies.
  const form = useForm(
    { name: item ? "x" : "", code: item?.code ?? "", category: item?.category_id ?? "", unit: item?.unit_id ?? "", min: item ? qtyInput(item.min_qty) : "", note: item?.note ?? "" },
    { name: required, category: required, unit: required, min: (s) => (s.trim() ? qtyRule(s, true) : null) },
  );
  useEffect(() => {
    if (!form.values.category && categories.items?.length) form.set("category", categories.items[0].id);
  }, [categories.items]);
  useEffect(() => {
    if (!form.values.unit && units.items?.length) form.set("unit", units.items[0].id);
  }, [units.items]);
  const unitName = tr(units.items?.find((u) => u.id === form.values.unit)?.name, lang);

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.validate()) return;
    void run(
      async () => {
        const saved = await rpc("stock_items.save", {
          id: item?.id ?? null,
          code: form.values.code.trim() || null,
          name: names,
          category_id: form.values.category,
          unit_id: form.values.unit,
          min_qty: parseQty(form.values.min) ?? 0,
          note: form.values.note.trim() || null,
          is_active: active,
          version: item?.version ?? null,
        });
        onDone(saved);
      },
      (x) => form.serverError(x, { category_id: "category", unit_id: "unit", min_qty: "min" }),
    );
  };
  return (
    <Dialog
      dismissOnScrim={false}
      title={t(item ? "stock.item.edit" : "stock.item.new")}
      onClose={onClose}
      wide
      testId="item-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Save} loading={busy} onClick={submit} data-testid="item-save">{t("common.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField
          label={t("stock.item.name")}
          value={names}
          onChange={(n) => {
            setNames(n);
            form.set("name", `${n.fa}${n.ps}${n.en}`);
          }}
          error={form.error("name") && t(form.error("name")!)}
          testId="item-name"
        />
        <div className="grid-2">
          <Field label={t("stock.item.category")} error={form.error("category") && t(form.error("category")!)}>
            <Select value={form.values.category} onChange={(e) => form.set("category", e.target.value)} data-testid="item-category">
              {categories.items?.map((c) => <option key={c.id} value={c.id}>{tr(c.name, lang)}</option>)}
            </Select>
          </Field>
          <Field label={t("stock.item.unit")} error={form.error("unit") && t(form.error("unit")!)}>
            <Select value={form.values.unit} onChange={(e) => form.set("unit", e.target.value)} data-testid="item-unit">
              {units.items?.map((u) => <option key={u.id} value={u.id}>{tr(u.name, lang)}</option>)}
            </Select>
          </Field>
        </div>
        <div className="grid-2">
          <Field label={t("stock.item.min")} hint={t("stock.item.minHint")} error={form.error("min") && t(form.error("min")!)} optional>
            <QtyInput value={form.values.min} onChange={(v) => form.set("min", v)} unit={unitName} testId="item-min" />
          </Field>
          <Field label={t("stock.item.code")} optional>
            <TextInput value={form.values.code} onChange={(e) => form.set("code", e.target.value)} maxLength={40} dir="ltr" data-testid="item-code" />
          </Field>
        </div>
        <Field label={t("stock.item.note")} optional>
          <Textarea rows={2} value={form.values.note} onChange={(e) => form.set("note", e.target.value)} maxLength={300} data-testid="item-note" />
        </Field>
        {item && <Switch checked={active} onChange={setActive} label={t("stock.item.active")} testId="item-active" />}
      </form>
    </Dialog>
  );
}

// ───────────────────────────── receive ─────────────────────────────

export function ReceiveDialog({ item, calendar, onClose, onDone }: { item?: StockItemInfo; calendar: CalendarSystem; onClose: () => void; onDone: (i: StockItemInfo) => void }) {
  const { t, lang } = useI18n();
  const options = useItemOptions();
  const places = useStockLookup("locations");
  const suppliers = useSupplierOptions("supplier");
  const [date, setDate] = useState(todayIso());
  const [expiry, setExpiry] = useState<string | null>(null);
  const [dateBad, setDateBad] = useState(false);
  const [expiryBad, setExpiryBad] = useState(false);
  // Items added from inside this form (the list above was loaded before them).
  const [added, setAdded] = useState<StockItemInfo[]>([]);
  const [newItem, setNewItem] = useState(false);
  const [more, setMore] = useState(false);
  const { busy, error, run } = useAction();
  const form = useForm(
    { item: item?.id ?? "", qty: "", cost: "", lot: "", place: "", supplier: "", reason: "" },
    { item: required, qty: (s) => qtyRule(s), cost: (s) => (s.trim() ? amountRule(s, true) : null), place: required },
  );
  useEffect(() => {
    if (!form.values.place && places.items?.length) form.set("place", places.items[0].id);
  }, [places.items]);
  const all = item ? [item] : [...(options ?? []), ...added];
  const picked = item ?? all.find((i) => i.id === form.values.item);
  const unitName = picked ? tr(picked.unit, lang) : "";
  const noItems = !item && options !== null && all.length === 0;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    const ok = form.validate();
    if (!ok || dateBad || expiryBad) {
      if (expiryBad) setMore(true);
      return;
    }
    void run(
      async () => {
        onDone(
          await rpc("stock.receive", {
            item_id: form.values.item,
            qty: parseQty(form.values.qty) ?? 0,
            unit_cost: parseAfn(form.values.cost) ?? 0,
            lot_no: form.values.lot.trim() || null,
            expiry_date: expiry,
            location_id: form.values.place,
            supplier_id: form.values.supplier || null,
            date,
            reason: form.values.reason.trim() || null,
          }),
        );
      },
      (x) => {
        const field = (x as { field?: string }).field;
        if (field === "lot_no" || field === "supplier_id" || field === "expiry_date") setMore(true);
        return form.serverError(x, { item_id: "item", location_id: "place", unit_cost: "cost", supplier_id: "supplier", lot_no: "lot" });
      },
    );
  };
  return (
    <>
      <Dialog
        dismissOnScrim={false}
        title={t("stock.receive.title")}
        description={t("stock.receive.hint")}
        onClose={onClose}
        testId="receive-dialog"
        footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={PackagePlus} loading={busy} disabled={noItems} onClick={submit} data-testid="receive-save">{t("stock.receive.save")}</Button></>}
      >
        <form className="stack" onSubmit={submit} noValidate>
          {error && <Notice tone="danger">{error}</Notice>}
          {noItems ? (
            <div className="receive-empty" data-testid="receive-no-items">
              <strong>{t("stock.receive.noItems")}</strong>
              <p className="subtle">{t("stock.receive.noItemsHint")}</p>
              <Button variant="primary" icon={Plus} onClick={() => setNewItem(true)} data-testid="receive-add-item">{t("stock.receive.addItem")}</Button>
            </div>
          ) : (
            <Field label={t("stock.col.item")} error={form.error("item") && t(form.error("item")!)}>
              <Combobox
                options={all.map((i) => ({ value: i.id, label: tr(i.name, lang), hint: tr(i.unit, lang) }))}
                value={form.values.item}
                onChange={(v) => form.set("item", v)}
                placeholder={t("stock.pick.item")}
                emptyText={t("stock.receive.noMatch")}
                disabled={!!item}
                onCreate={item ? undefined : () => setNewItem(true)}
                createLabel={t("stock.receive.newItem")}
                testId="receive-item"
              />
            </Field>
          )}
          <div className="grid-2">
            <Field label={t("stock.qty")} error={form.error("qty") && t(form.error("qty")!)}>
              <QtyInput value={form.values.qty} onChange={(v) => form.set("qty", v)} unit={unitName} testId="receive-qty" />
            </Field>
            <Field label={t("stock.unitCost")} hint={t("stock.unitCostHint")} error={form.error("cost") && t(form.error("cost")!)} optional>
              <AmountInput value={form.values.cost} onChange={(v) => form.set("cost", v)} testId="receive-cost" />
            </Field>
          </div>
          <div className="grid-2">
            <Field label={t("stock.place")} error={form.error("place") && t(form.error("place")!)}>
              <Select value={form.values.place} onChange={(e) => form.set("place", e.target.value)} data-testid="receive-place">
                {places.items?.map((p) => <option key={p.id} value={p.id}>{tr(p.name, lang)}</option>)}
              </Select>
            </Field>
            <Field label={t("doc.date")}>
              <DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="receive-date" max={todayIso()} onInvalidChange={setDateBad} />
            </Field>
          </div>
          <Button variant="subtle" size="sm" className="receive-more" icon={more ? ChevronUp : ChevronDown} onClick={() => setMore((x) => !x)} aria-expanded={more} data-testid="receive-more">
            {t(more ? "stock.receive.lessOptions" : "stock.receive.moreOptions")}
          </Button>
          {more && (
            <>
              <div className="grid-2">
                <Field label={t("stock.expiry")} optional>
                  <DateField value={expiry ?? ""} onChange={(v) => setExpiry(v || null)} calendar={calendar} label={t("stock.expiry")} testId="receive-expiry" clearable onInvalidChange={setExpiryBad} />
                </Field>
                <Field label={t("stock.lot")} optional>
                  <TextInput value={form.values.lot} onChange={(e) => form.set("lot", e.target.value)} maxLength={60} dir="ltr" data-testid="receive-lot" />
                </Field>
              </div>
              <Field label={t("stock.supplier")} optional>
                <Select value={form.values.supplier} onChange={(e) => form.set("supplier", e.target.value)} data-testid="receive-supplier">
                  <option value="">{t("stock.noSupplier")}</option>
                  {suppliers.list?.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
                </Select>
              </Field>
              <Field label={t("stock.reason")} hint={t("stock.receive.reasonHint")} optional>
                <TextInput value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="receive-reason" />
              </Field>
            </>
          )}
        </form>
      </Dialog>
      {newItem && (
        <ItemFormDialog
          onClose={() => setNewItem(false)}
          onDone={(i) => {
            setAdded((a) => [...a, i]);
            form.set("item", i.id);
            setNewItem(false);
          }}
        />
      )}
    </>
  );
}

// ───────────────────────────── consume ─────────────────────────────

export function ConsumeDialog({ item, onClose, onDone }: { item: StockItemInfo; onClose: () => void; onDone: (i: StockItemInfo) => void }) {
  const { t, lang } = useI18n();
  const places = useStockLookup("locations");
  const { busy, error, run } = useAction();
  const form = useForm(
    { qty: "", place: "", reason: "" },
    { qty: (s) => qtyRule(s) ?? ((parseQty(s) ?? 0) > item.usable_qty ? "rule.stock_insufficient" : null) },
  );
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.validate()) return;
    void run(
      async () => {
        onDone(await rpc("stock.consume", { item_id: item.id, qty: parseQty(form.values.qty) ?? 0, date: null, reason: form.values.reason.trim() || null, location_id: form.values.place || null, patient_id: null, treatment_item_id: null }));
      },
      (x) => form.serverError(x, { location_id: "place" }),
    );
  };
  return (
    <Dialog
      dismissOnScrim={false}
      title={t("stock.consume.title")}
      description={`${tr(item.name, lang)} · ${t("stock.consume.available")}: ${formatQty(item.usable_qty, lang)} ${tr(item.unit, lang)}`}
      onClose={onClose}
      testId="consume-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Minus} loading={busy} onClick={submit} data-testid="consume-save">{t("stock.consume.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Notice tone="info">{t("stock.consume.fefo")}</Notice>
        <Field label={t("stock.qty")} error={form.error("qty") && t(form.error("qty")!)}>
          <QtyInput value={form.values.qty} onChange={(v) => form.set("qty", v)} unit={tr(item.unit, lang)} testId="consume-qty" autoFocus />
        </Field>
        <Field label={t("stock.consume.from")} optional>
          <Select value={form.values.place} onChange={(e) => form.set("place", e.target.value)} data-testid="consume-place">
            <option value="">{t("stock.consume.anyPlace")}</option>
            {places.items?.map((p) => <option key={p.id} value={p.id}>{tr(p.name, lang)}</option>)}
          </Select>
        </Field>
        <Field label={t("stock.reason")} hint={t("stock.consume.reasonHint")} optional>
          <TextInput value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="consume-reason" />
        </Field>
      </form>
    </Dialog>
  );
}

// ───────────────────────────── one batch: adjust, move, return ─────────────────────────────

const batchTitle = (b: StockBatchInfo, lang: "fa" | "ps" | "en") => `${tr(b.item_name, lang)}${b.lot_no ? ` · ${b.lot_no}` : ""} · ${formatQty(b.qty_left, lang)} ${tr(b.unit, lang)}`;

export function AdjustDialog({ batch, onClose, onDone }: { batch: StockBatchInfo; onClose: () => void; onDone: () => void }) {
  const { t, lang } = useI18n();
  const [direction, setDirection] = useState<"remove" | "add">("remove");
  const { busy, error, run } = useAction();
  const form = useForm({ qty: "", reason: "" }, { qty: (s) => qtyRule(s) ?? (direction === "remove" && (parseQty(s) ?? 0) > batch.qty_left ? "rule.stock_insufficient" : null), reason: required });
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.validate()) return;
    const qty = parseQty(form.values.qty) ?? 0;
    void run(
      async () => {
        await rpc("stock.adjust", { batch_id: batch.id, delta: direction === "add" ? qty : -qty, reason: form.values.reason.trim(), date: null });
        onDone();
      },
      (x) => form.serverError(x, { delta: "qty" }),
    );
  };
  return (
    <Dialog
      dismissOnScrim={false}
      title={t("stock.adjust.title")}
      description={batchTitle(batch, lang)}
      onClose={onClose}
      testId="adjust-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={submit} data-testid="adjust-save">{t("common.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Segmented value={direction} onChange={setDirection} label={t("stock.adjust.direction")} block options={[{ value: "remove", label: t("stock.adjust.remove"), testId: "adjust-remove" }, { value: "add", label: t("stock.adjust.add"), testId: "adjust-add" }]} />
        <Field label={t("stock.qty")} error={form.error("qty") && t(form.error("qty")!)}>
          <QtyInput value={form.values.qty} onChange={(v) => form.set("qty", v)} unit={tr(batch.unit, lang)} testId="adjust-qty" autoFocus />
        </Field>
        <Field label={t("stock.reason")} hint={t("stock.adjust.reasonHint")} error={form.error("reason") && t(form.error("reason")!)}>
          <TextInput value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="adjust-reason" />
        </Field>
      </form>
    </Dialog>
  );
}

export function TransferDialog({ batch, onClose, onDone }: { batch: StockBatchInfo; onClose: () => void; onDone: () => void }) {
  const { t, lang } = useI18n();
  const places = useStockLookup("locations");
  const targets = (places.items ?? []).filter((p) => p.id !== batch.location_id);
  const { busy, error, run } = useAction();
  const form = useForm({ qty: "", to: "" }, { qty: (s) => qtyRule(s) ?? ((parseQty(s) ?? 0) > batch.qty_left ? "rule.stock_insufficient" : null), to: required });
  useEffect(() => {
    if (!form.values.to && targets.length) form.set("to", targets[0].id);
  }, [places.items]);
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.validate()) return;
    void run(
      async () => {
        await rpc("stock.transfer", { batch_id: batch.id, qty: parseQty(form.values.qty) ?? 0, to_location_id: form.values.to, date: null });
        onDone();
      },
      (x) => form.serverError(x, { to_location_id: "to" }),
    );
  };
  return (
    <Dialog
      dismissOnScrim={false}
      title={t("stock.transfer.title")}
      description={`${batchTitle(batch, lang)} · ${tr(batch.location, lang)}`}
      onClose={onClose}
      testId="transfer-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={ArrowLeftRight} loading={busy} onClick={submit} data-testid="transfer-save">{t("stock.transfer.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("stock.transfer.to")} error={form.error("to") && t(form.error("to")!)}>
          <Select value={form.values.to} onChange={(e) => form.set("to", e.target.value)} data-testid="transfer-to">
            {targets.map((p) => <option key={p.id} value={p.id}>{tr(p.name, lang)}</option>)}
          </Select>
        </Field>
        <Field label={t("stock.qty")} error={form.error("qty") && t(form.error("qty")!)}>
          <QtyInput value={form.values.qty} onChange={(v) => form.set("qty", v)} unit={tr(batch.unit, lang)} testId="transfer-qty" autoFocus />
        </Field>
      </form>
    </Dialog>
  );
}

export function ReturnDialog({ batch, onClose, onDone }: { batch: StockBatchInfo; onClose: () => void; onDone: () => void }) {
  const { t, lang } = useI18n();
  const suppliers = useSupplierOptions("supplier");
  const { busy, error, run } = useAction();
  const form = useForm(
    { qty: "", supplier: "", credit: "", reason: "" },
    { qty: (s) => qtyRule(s) ?? ((parseQty(s) ?? 0) > batch.qty_left ? "rule.stock_insufficient" : null), supplier: required, credit: (s) => (s.trim() ? amountRule(s, true) : null) },
  );
  useEffect(() => {
    if (!form.values.supplier && suppliers.list?.length) form.set("supplier", suppliers.list[0].id);
  }, [suppliers.list]);
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.validate()) return;
    void run(
      async () => {
        await rpc("stock.return_to_supplier", { batch_id: batch.id, qty: parseQty(form.values.qty) ?? 0, supplier_id: form.values.supplier, credit: parseAfn(form.values.credit) ?? 0, reason: form.values.reason.trim() || null, date: null });
        onDone();
      },
      (x) => form.serverError(x, { supplier_id: "supplier" }),
    );
  };
  return (
    <Dialog
      dismissOnScrim={false}
      title={t("stock.return.title")}
      description={batchTitle(batch, lang)}
      onClose={onClose}
      testId="return-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Undo2} loading={busy} onClick={submit} data-testid="return-save">{t("stock.return.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("stock.supplier")} error={form.error("supplier") && t(form.error("supplier")!)}>
          <Select value={form.values.supplier} onChange={(e) => form.set("supplier", e.target.value)} data-testid="return-supplier">
            {suppliers.list?.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
          </Select>
        </Field>
        <div className="grid-2">
          <Field label={t("stock.qty")} error={form.error("qty") && t(form.error("qty")!)}>
            <QtyInput value={form.values.qty} onChange={(v) => form.set("qty", v)} unit={tr(batch.unit, lang)} testId="return-qty" autoFocus />
          </Field>
          <Field label={t("stock.return.credit")} hint={t("stock.return.creditHint")} error={form.error("credit") && t(form.error("credit")!)} optional>
            <AmountInput value={form.values.credit} onChange={(v) => form.set("credit", v)} testId="return-credit" />
          </Field>
        </div>
        <Field label={t("stock.reason")} optional>
          <TextInput value={form.values.reason} onChange={(e) => form.set("reason", e.target.value)} maxLength={300} data-testid="return-reason" />
        </Field>
      </form>
    </Dialog>
  );
}

