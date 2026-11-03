// Phase 6 helpers shared by the billing screens: reading an amount a person typed, the payment methods
// (fetched once, refreshed after an edit), and the words for the states of money documents.
import { useEffect, useState } from "react";
import type { InvoiceInfo, InvoiceListParams, PaymentListParams, PayState, PaymentInfo, PaymentMethodInfo } from "../../../shared/ts/contract";
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

let methods: Promise<PaymentMethodInfo[]> | null = null;
const listeners = new Set<() => void>();

export function invalidatePaymentMethods() {
  methods = null;
  listeners.forEach((l) => l());
}

/** Every payment method, switched-off ones included (an old payment keeps its method). */
export function usePaymentMethods(): PaymentMethodInfo[] | null {
  const [value, setValue] = useState<PaymentMethodInfo[] | null>(null);
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
    methods ??= rpc("payment_methods.list", { include_inactive: true }).catch((e) => {
      methods = null;
      throw e;
    });
    methods.then((v) => live && setValue(v)).catch(() => {});
    return () => {
      live = false;
    };
  }, [tick]);
  return value;
}

export const PAY_TONE: Record<PayState, "danger" | "warning" | "success" | "neutral"> = {
  unpaid: "danger",
  partial: "warning",
  paid: "success",
  void: "neutral",
};

/** Whether the invoice still takes money. */
export const isOpen = (i: Pick<InvoiceInfo, "status" | "remaining">) => i.status === "issued" && i.remaining > 0;

/** How a money row is called on screen: the receipt number, or "credit used" / "refund". */
export const paymentTitleKey = (p: Pick<PaymentInfo, "kind" | "invoice_id">) =>
  p.kind === "refund" ? "pay.kind.refund" : p.kind === "credit" ? "pay.kind.credit" : p.invoice_id ? "pay.kind.payment" : "pay.kind.advance";

/** Fills the unused filters of a payment list request. */
export const payQ = (p: Partial<PaymentListParams>): PaymentListParams => ({ patient_id: null, invoice_id: null, from: null, to: null, limit: null, offset: null, ...p });
/** Fills the unused filters of an invoice list request. */
export const invQ = (p: Partial<InvoiceListParams>): InvoiceListParams => ({ patient_id: null, open_only: false, from: null, to: null, search: null, limit: null, offset: null, ...p });
