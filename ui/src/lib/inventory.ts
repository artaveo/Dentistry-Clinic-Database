// Phase 8 helpers shared by the inventory screens: quantities (hundredths of a unit, like money is
// hundredths of an afghani), the places / categories / units lists, tones of the status badges, and
// reading a list of tooth numbers a person typed.
import { useCallback, useEffect, useState } from "react";
import type { BatchState, Language, LabCaseListParams, LabStatus, PurchaseListParams, StockBatchListParams, StockItemListParams, StockLookupInfo, StockMovementListParams, SupplierListParams, SupplierPaymentListParams } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "./api";
import { afnInput, parseAfn } from "./billing";
import { digits, latinDigits } from "./dates";

/** A quantity typed as 2, 2.5 or ۲٫۵ in hundredths of a unit (200, 250), or null when it is not a number. */
export const parseQty = parseAfn;
/** A quantity as the plain number a person edits ("2", "2.5"). */
export const qtyInput = afnInput;

/** The rule key for a typed quantity: empty, not a number, zero (unless allowed). */
export function qtyRule(s: string, allowZero = false): string | null {
  if (!s.trim()) return "rule.required";
  const n = parseQty(s);
  if (n === null || (n === 0 && !allowZero)) return "rule.stock_quantity";
  return null;
}

/** A quantity in hundredths for display: `2.5` / `۲٫۵`, whole numbers without decimals. */
export function formatQty(q: number, lang: Language): string {
  const grouped = (q / 100).toLocaleString("en-US", { maximumFractionDigits: 2 });
  if (lang === "en") return grouped;
  return digits(grouped, lang).replace(/,/g, "٬").replace(".", "٫");
}

type Tone = "neutral" | "accent" | "success" | "warning" | "danger" | "info";

export const BATCH_TONE: Record<BatchState, Tone> = { valid: "success", expiring: "warning", expired: "danger" };
export const LAB_TONE: Record<LabStatus, Tone> = { planned: "neutral", sent: "info", received: "accent", delivered: "success", cancelled: "neutral" };

/** FDI tooth numbers typed as "36, 37 ۳۸"; null when one is not a tooth. An empty text is no teeth. */
export function parseTeeth(input: string): number[] | null {
  const parts = latinDigits(input).split(/[\s,،;]+/).filter(Boolean);
  const teeth: number[] = [];
  for (const p of parts) {
    if (!/^[1-8][1-8]$/.test(p)) return null;
    const n = Number(p);
    if (!teeth.includes(n)) teeth.push(n);
  }
  return teeth.length > 32 ? null : teeth;
}

export const teethText = (teeth: number[]) => teeth.join(", ");

export type LookupKind = "categories" | "units" | "locations";

/** One of the clinic's editable stock lists (categories, units, places). */
export function useStockLookup(kind: LookupKind, includeInactive = false) {
  const [items, setItems] = useState<StockLookupInfo[] | null>(null);
  const [error, setError] = useState<unknown>(null);
  const load = useCallback(() => {
    const p = { include_inactive: includeInactive };
    const call = kind === "categories" ? rpc("stock_categories.list", p) : kind === "units" ? rpc("stock_units.list", p) : rpc("stock_locations.list", p);
    call
      .then((r) => {
        setItems(r);
        setError(null);
      })
      .catch((e) => !isSessionError(e) && setError(e));
  }, [kind, includeInactive]);
  useEffect(load, [load]);
  return { items, error, reload: load };
}

/** Fill the unused filters of a list request (the contract has no optional fields). */
export const itemQ = (p: Partial<StockItemListParams> = {}): StockItemListParams => ({ search: null, category_id: null, only: null, include_inactive: false, expiring_days: null, ...p });
export const batchQ = (p: Partial<StockBatchListParams> = {}): StockBatchListParams => ({ item_id: null, include_empty: false, expiring_days: null, ...p });
export const moveQ = (p: Partial<StockMovementListParams> = {}): StockMovementListParams => ({ item_id: null, from: null, to: null, kind: null, ...p });
export const supplierQ = (p: Partial<SupplierListParams> = {}): SupplierListParams => ({ kind: null, include_inactive: false, owing_only: false, ...p });
export const purchaseQ = (p: Partial<PurchaseListParams> = {}): PurchaseListParams => ({ supplier_id: null, from: null, to: null, ...p });
export const spayQ = (p: Partial<SupplierPaymentListParams> = {}): SupplierPaymentListParams => ({ supplier_id: null, from: null, to: null, ...p });
export const labQ = (p: Partial<LabCaseListParams> = {}): LabCaseListParams => ({ status: null, lab_id: null, patient_id: null, open_only: false, delayed_only: false, ...p });
