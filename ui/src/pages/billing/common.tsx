import { payQ } from "../../lib/billing";
// Pieces every billing screen shares: the permissions it works with, an amount input, the badges of
// invoices, and the receipt/invoice previews that open from any list.
import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import type { BillingSettings, CalendarSystem, InvoiceInfo, PayState, PaymentInfo, ToothNumbering } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { useClinicSettings } from "../../lib/clinical";
import { PAY_TONE, parseAfn } from "../../lib/billing";
import { formatAmount } from "../../lib/money";
import { useI18n } from "../../i18n";
import { Badge } from "../../ui/Feedback";
import { TextInput } from "../../ui/Field";
import { DocumentPreview } from "../../print/DocumentPreview";
import { InvoiceSheet } from "../../print/finance";
import { ReceiptPreview } from "../../print/receipt";
import { useLetterhead } from "../../print/letterhead";

export type BillingPerms = {
  view: boolean;
  edit: boolean;
  /** Void invoices, credit notes, refunds. */
  void: boolean;
  /** Invoice a treatment at another price. */
  override: boolean;
  /** Expenses, profit and commission reports. */
  reports: boolean;
  /** Payment methods, commission rules, billing settings. */
  settings: boolean;
};

export const NO_BILLING: BillingPerms = { view: false, edit: false, void: false, override: false, reports: false, settings: false };

/** An input for an amount in afghani (typed as 2500 or ۲۵۰۰), with the currency after it. */
export function AmountInput({ value, onChange, invalid, testId, autoFocus, ...rest }: { value: string; onChange: (v: string) => void; invalid?: boolean; testId?: string; autoFocus?: boolean; id?: string; "aria-describedby"?: string }) {
  const { lang } = useI18n();
  return (
    <TextInput
      {...rest}
      inputMode="decimal"
      autoComplete="off"
      dir="ltr"
      autoFocus={autoFocus}
      invalid={invalid}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      suffix={lang === "en" ? "AFN" : "افغانی"}
      data-testid={testId}
    />
  );
}

/** What a typed amount is worth in AFN × 100, or 0 while it is not a number yet. */
export const minorOf = (s: string) => parseAfn(s) ?? 0;

export function PayBadge({ state }: { state: PayState }) {
  const { t } = useI18n();
  return <Badge tone={PAY_TONE[state]} dot testId={`pay-state-${state}`}>{t(`pay.state.${state}`)}</Badge>;
}

export function Money({ value, strong, tone }: { value: number; strong?: boolean; tone?: "danger" | "success" | "muted" }) {
  const { lang } = useI18n();
  return <span className={["num", strong && "cell-strong", tone && `money-${tone}`].filter(Boolean).join(" ")}>{formatAmount(value, lang)}</span>;
}

let settingsCache: Promise<BillingSettings> | null = null;
export function invalidateBillingSettings() {
  settingsCache = null;
}

/** The billing settings (receipt footer, commission); null until loaded or when this user cannot see them. */
export function useBillingSettings(enabled = true): BillingSettings | null {
  const [v, setV] = useState<BillingSettings | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let live = true;
    settingsCache ??= rpc("billing.settings_get", {}).catch((e) => {
      settingsCache = null;
      throw e;
    });
    settingsCache.then((x) => live && setV(x)).catch(() => {});
    return () => {
      live = false;
    };
  }, [enabled]);
  return v;
}

/** Opens the receipt of a payment for printing; a reprint is counted by the Core, never a new payment. */
export function ReceiptHost({ payment, calendar, onClose, onChanged }: { payment: PaymentInfo; calendar: CalendarSystem; onClose: () => void; onChanged?: (p: PaymentInfo) => void }) {
  const settings = useBillingSettings();
  return (
    <ReceiptPreview
      payment={payment}
      calendar={calendar}
      footer={settings?.receipt_footer ?? ""}
      onClose={onClose}
      onPrinted={async (pdf) => {
        const next = await rpc("payments.mark_printed", { id: payment.id, pdf });
        onChanged?.(next);
        return next;
      }}
    />
  );
}

/** The full A4 invoice: preview, print and PDF; each print is counted. */
export function InvoicePrint({ invoice, calendar, onClose, onChanged }: { invoice: InvoiceInfo; calendar: CalendarSystem; onClose: () => void; onChanged?: (i: InvoiceInfo) => void }) {
  const { t, lang } = useI18n();
  const letterhead = useLetterhead();
  const settings = useClinicSettings();
  const billing = useBillingSettings();
  const [payments, setPayments] = useState<PaymentInfo[] | null>(null);
  const [language, setLanguage] = useState(lang);
  useEffect(() => {
    rpc("payments.list", payQ({ invoice_id: invoice.id, limit: 100 })).then(setPayments).catch(() => setPayments([]));
  }, [invoice.id]);
  if (!letterhead || !payments) return null;
  return (
    <DocumentPreview
      title={t("inv.preview")}
      paper="a4"
      fileStem={invoice.number}
      testId="invoice-preview"
      onClose={onClose}
      onPrinted={async (pdf) => {
        onChanged?.(await rpc("invoices.mark_printed", { id: invoice.id, pdf }));
      }}
      extra={
        <div className="field">
          <span className="field-label">{t("rcpt.language")}</span>
          <div className="segmented block" role="radiogroup" aria-label={t("rcpt.language")}>
            {(["fa", "ps", "en"] as const).map((l) => (
              <button key={l} type="button" role="radio" aria-checked={language === l} onClick={() => setLanguage(l)}>{l === "fa" ? "دری" : l === "ps" ? "پښتو" : "EN"}</button>
            ))}
          </div>
        </div>
      }
      sheet={<InvoiceSheet invoice={invoice} payments={payments} letterhead={letterhead} lang={language} calendar={calendar} numbering={(settings?.tooth_numbering ?? "fdi") as ToothNumbering} footer={billing?.receipt_footer} />}
    />
  );
}

/** A labelled number row of an account summary. */
export function SumRow({ label, children, strong }: { label: ReactNode; children: ReactNode; strong?: boolean }) {
  return (
    <div className={strong ? "sum-row strong" : "sum-row"}>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}
