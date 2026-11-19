// Helpers shared by the finance screens (docs/design/finance-standard.md): reading an amount a person typed,
// the chart of accounts (fetched once, refreshed after an edit), account names, and the words for the states
// of money documents.
import { useEffect, useState } from "react";
import type { AccountInfo, DocKind, InvoiceInfo, InvoiceListParams, PayState, ReceiptListParams, VoucherListParams } from "../../../shared/ts/contract";
import { rpc } from "./api";
import { latinDigits } from "./dates";

/**
 * An amount typed in afghani — Persian or Latin digits, `٬` `,` or spaces between thousands, up to two
 * decimals — as AFN × 100 (ADR-06). null when it is not a number.
 */
export function parseAfn(input: string): number | null {
  const s = latinDigits(input)
    .replace(/[٬,\s]/g, "")
    .replace("٫", ".")
    .trim();
  if (!/^\d+(\.\d{1,2})?$/.test(s)) return null;
  return Math.round(Number(s) * 100);
}

/** An amount in AFN × 100 as the plain number a person edits ("2500", "2500.5"). */
export function afnInput(minor: number): string {
  const whole = minor / 100;
  return Number.isInteger(whole) ? String(whole) : whole.toFixed(2).replace(/0$/, "");
}

/** The rule key for a typed amount: empty, not a number, zero. */
export function amountRule(s: string, allowZero = false): string | null {
  if (!s.trim()) return "rule.required";
  const n = parseAfn(s);
  if (n === null) return "rule.billing_amount";
  if (n === 0 && !allowZero) return "rule.billing_amount";
  return null;
}

// ───────────────────────────── the chart of accounts ─────────────────────────────

let accounts: Promise<AccountInfo[]> | null = null;
const listeners = new Set<() => void>();

/** Call after an account was added or changed, or after money moved (balances). */
export function invalidateAccounts() {
  accounts = null;
  listeners.forEach((l) => l());
}

/** Every account, switched-off ones included (an old document keeps its account). */
export function useAccounts(): AccountInfo[] | null {
  const [value, setValue] = useState<AccountInfo[] | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const bump = () => setTick((x) => x + 1);
    listeners.add(bump);
    return () => {
      listeners.delete(bump);
    };
  }, []);
  useEffect(() => {
    let live = true;
    accounts ??= rpc("fin.accounts.list", { include_inactive: true }).catch((e) => {
      accounts = null;
      throw e;
    });
    accounts.then((v) => live && setValue(v)).catch(() => {});
    return () => {
      live = false;
    };
  }, [tick]);
  return value;
}

/** Money accounts (cash drawers, banks, mobile money) that are turned on. */
export const moneyAccounts = (list: AccountInfo[] | null) => (list ?? []).filter((a) => a.money && a.is_active);

/** Expense accounts a person books to by hand (not the parent 5000, bad debts or cash over/short). */
export const expenseAccounts = (list: AccountInfo[] | null) =>
  (list ?? []).filter((a) => a.kind === "expense" && a.is_active && ![5000, 5810, 5910].includes(a.id));

/** An account's name: the clinic's own name, or the built-in label for its code. */
export function accountName(a: Pick<AccountInfo, "id" | "name"> | undefined | null, t: (k: string) => string): string {
  if (!a) return "—";
  return a.name ?? t(`fin.account.${a.id}`);
}

/** The name of an account by its code, from a loaded list. */
export function accountNameOf(id: number | null | undefined, list: AccountInfo[] | null, t: (k: string) => string): string {
  if (id === null || id === undefined) return "—";
  const a = (list ?? []).find((x) => x.id === id);
  return a ? accountName(a, t) : t(`fin.account.${id}`);
}

// ───────────────────────────── documents ─────────────────────────────

export const PAY_TONE: Record<PayState, "danger" | "warning" | "success" | "neutral"> = {
  unpaid: "danger",
  partial: "warning",
  paid: "success",
  void: "neutral",
};

/** Whether the invoice still takes money. */
export const isOpen = (i: Pick<InvoiceInfo, "status" | "remaining">) => i.status === "posted" && i.remaining > 0;

/** The word for a kind of document ("receipt", "refund", …). */
export const docKindKey = (k: DocKind) => `fin.doc.${k}`;

/** Fills the unused filters of an invoice list request. */
export const invQ = (p: Partial<InvoiceListParams>): InvoiceListParams => ({ patient_id: null, open_only: false, from: null, to: null, search: null, limit: null, offset: null, ...p });
/** Fills the unused filters of a receipt list request. */
export const rcQ = (p: Partial<ReceiptListParams>): ReceiptListParams => ({ patient_id: null, kind: null, account_id: null, from: null, to: null, search: null, limit: null, offset: null, ...p });
/** Fills the unused filters of a voucher list request. */
export const pvQ = (p: Partial<VoucherListParams>): VoucherListParams => ({ kind: null, purpose: null, account_id: null, supplier_id: null, from: null, to: null, limit: null, offset: null, ...p });
