// Pieces every inventory screen shares: the permissions it works with, a quantity input, the optional
// expiry date, a stock-level meter, and the small hooks that load items and suppliers for the forms.
import { useCallback, useEffect, useState } from "react";
import { X } from "lucide-react";
import type { CalendarSystem, StockItemInfo, SupplierInfo, SupplierKind } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { addDays, todayIso } from "../../lib/calendar";
import { itemQ, supplierQ } from "../../lib/inventory";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { DateField } from "../../ui/DateField";
import { TextInput } from "../../ui/Field";

export type InventoryPerms = {
  view: boolean;
  /** Items, receiving, adjusting, purchases, suppliers, lab. */
  manage: boolean;
  /** Pay a supplier (it is also an expense). */
  billingEdit: boolean;
  /** Void a payment to a supplier. */
  billingVoid: boolean;
  clinicalView: boolean;
  clinicalEdit: boolean;
  treat: boolean;
  reports: boolean;
  settings: boolean;
  patientsEdit: boolean;
};

export const NO_INVENTORY: InventoryPerms = { view: false, manage: false, billingEdit: false, billingVoid: false, clinicalView: false, clinicalEdit: false, treat: false, reports: false, settings: false, patientsEdit: false };

/** Staff who may take materials out for a treatment. */
export const canConsume = (p: InventoryPerms) => p.manage || p.clinicalEdit || p.treat;

/** Staff who may open, edit and move lab cases. */
export const canLab = (p: InventoryPerms) => p.manage || p.clinicalEdit;

/** A quantity input with the item's unit after it. */
export function QtyInput({ value, onChange, unit, invalid, testId, autoFocus, ...rest }: { value: string; onChange: (v: string) => void; unit?: string; invalid?: boolean; testId?: string; autoFocus?: boolean; id?: string; "aria-describedby"?: string }) {
  return <TextInput {...rest} inputMode="decimal" autoComplete="off" dir="ltr" autoFocus={autoFocus} invalid={invalid} value={value} onChange={(e) => onChange(e.target.value)} suffix={unit} data-testid={testId} />;
}

/** An optional expiry date: a button to set one, then the date with a way to clear it. */
export function ExpiryInput({ value, onChange, calendar, label, testId }: { value: string | null; onChange: (v: string | null) => void; calendar: CalendarSystem; label: string; testId: string }) {
  const { t } = useI18n();
  if (value === null)
    return <Button onClick={() => onChange(addDays(todayIso(), 365))} data-testid={`${testId}-set`}>{t("stock.expiry.set")}</Button>;
  return (
    <div className="row">
      <DateField value={value} onChange={onChange} calendar={calendar} label={label} testId={testId} yearsBefore={1} yearsAfter={8} />
      <IconButton icon={X} label={t("stock.expiry.clear")} onClick={() => onChange(null)} data-testid={`${testId}-clear`} />
    </div>
  );
}

/** How full a stock is next to its minimum. Decorative: the numbers and the badge say the same. */
export function Level({ qty, min }: { qty: number; min: number }) {
  if (min <= 0) return null;
  const pct = Math.max(0, Math.min(100, (qty / (min * 2)) * 100));
  const state = qty <= 0 ? "empty" : qty <= min ? "low" : "ok";
  return (
    <span className="level" data-state={state} aria-hidden>
      <i style={{ width: `${pct}%` }} />
    </span>
  );
}

/** Run an action with a busy flag and an error line; a field error goes to the form via `onField`. */
export function useAction() {
  const { err } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const run = useCallback(
    async (fn: () => Promise<void>, onField?: (e: unknown) => boolean) => {
      setError("");
      setBusy(true);
      try {
        await fn();
      } catch (x) {
        if (!onField?.(x) && !isSessionError(x)) setError(err(x));
      } finally {
        setBusy(false);
      }
    },
    [err],
  );
  return { busy, error, run, setError };
}

/** Every active item, for the pickers of the forms. */
export function useItemOptions() {
  const [items, setItems] = useState<StockItemInfo[] | null>(null);
  useEffect(() => {
    rpc("stock_items.list", itemQ()).then(setItems).catch(() => setItems([]));
  }, []);
  return items;
}

/** The clinic's active suppliers (or labs), for the pickers of the forms. */
export function useSupplierOptions(kind: SupplierKind | null) {
  const [list, setList] = useState<SupplierInfo[] | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    rpc("suppliers.list", supplierQ({ kind })).then(setList).catch(() => setList([]));
  }, [kind, tick]);
  return { list, reload: () => setTick((x) => x + 1) };
}
