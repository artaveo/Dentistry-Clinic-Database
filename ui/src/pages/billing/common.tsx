// Pieces every finance screen shares: the permissions it works with, an amount input, the badges of documents,
// the account pickers, and the receipt/invoice previews that open from any list.
import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import type { AccountInfo, CalendarSystem, DocStatus, FinanceSettings, InvoiceInfo, PayState, ReceiptInfo, ToothNumbering } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { useClinicSettings } from "../../lib/clinical";
import { PAY_TONE, accountName, accountNameOf, moneyAccounts, parseAfn, useAccounts } from "../../lib/billing";
import { formatAmount } from "../../lib/money";
import { useI18n } from "../../i18n";
import { Badge } from "../../ui/Feedback";
import { Select, TextInput } from "../../ui/Field";
import { DocumentPreview } from "../../print/DocumentPreview";
import { InvoiceSheet } from "../../print/finance";
import { ReceiptPreview } from "../../print/receipt";
import { useLetterhead } from "../../print/letterhead";

export type BillingPerms = {
  view: boolean;
  /** Take money, issue invoices, pay expenses. */
  edit: boolean;
  /** Void, reduce, write off, refund. */
  void: boolean;
  /** Invoice a treatment at another price. */
  override: boolean;
  /** The overview, expenses, cash and bank, doctor shares, reports. */
  reports: boolean;
  /** Accounts, the legal profile, doctor share rules, the receipt footer. */
  settings: boolean;
  /** Employees, payroll, doctor-share statements and paying them. */
  payroll: boolean;
};

export const NO_BILLING: BillingPerms = { view: false, edit: false, void: false, override: false, reports: false, settings: false, payroll: false };

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

/** Posted documents say nothing; a void or replaced one says so. */
export function StatusBadge({ status }: { status: DocStatus }) {
  const { t } = useI18n();
  if (status === "posted") return null;
  return <Badge tone="neutral" dot testId={`doc-status-${status}`}>{t(`fin.status.${status}`)}</Badge>;
}

export function Money({ value, strong, tone }: { value: number; strong?: boolean; tone?: "danger" | "success" | "muted" }) {
  const { lang } = useI18n();
  return <span className={["num", strong && "cell-strong", tone && `money-${tone}`].filter(Boolean).join(" ")}>{value < 0 ? "− " : ""}{formatAmount(Math.abs(value), lang)}</span>;
}

/** A picker of the money accounts (cash drawer, banks); the cash drawer first. */
export function MoneyAccountSelect({ value, onChange, testId, exclude }: { value: number | null; onChange: (id: number) => void; testId?: string; exclude?: number | null }) {
  const { t } = useI18n();
  const accounts = useAccounts();
  const list = moneyAccounts(accounts).filter((a) => a.id !== exclude);
  useEffect(() => {
    if (value === null && list.length) onChange(list[0].id);
  }, [accounts]);
  return (
    <Select value={value ?? ""} onChange={(e) => onChange(Number(e.target.value))} data-testid={testId}>
      {list.map((a) => <option key={a.id} value={a.id}>{accountName(a, t)}</option>)}
    </Select>
  );
}

/** A picker of accounts from a list (expense accounts, …). */
export function AccountSelect({ value, onChange, accounts, testId, placeholder }: { value: number | null; onChange: (id: number) => void; accounts: AccountInfo[]; testId?: string; placeholder?: string }) {
  const { t } = useI18n();
  return (
    <Select value={value ?? ""} onChange={(e) => onChange(Number(e.target.value))} data-testid={testId}>
      {placeholder !== undefined && <option value="">{placeholder}</option>}
      {accounts.map((a) => <option key={a.id} value={a.id}>{accountName(a, t)}</option>)}
    </Select>
  );
}

let settingsCache: Promise<FinanceSettings> | null = null;
export function invalidateBillingSettings() {
  settingsCache = null;
}

/** The finance settings (receipt footer, doctor shares, legal profile); null until loaded or when this user cannot see them. */
export function useBillingSettings(enabled = true): FinanceSettings | null {
  const [v, setV] = useState<FinanceSettings | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let live = true;
    settingsCache ??= rpc("fin.settings.get", {}).catch((e) => {
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

/** Opens the receipt of a receipt, refund or allocation for printing; a reprint is counted, never new money. */
export function ReceiptHost({ receipt, calendar, onClose, onChanged }: { receipt: ReceiptInfo; calendar: CalendarSystem; onClose: () => void; onChanged?: (r: ReceiptInfo) => void }) {
  const { t } = useI18n();
  const settings = useBillingSettings();
  const accounts = useAccounts();
  return (
    <ReceiptPreview
      receipt={receipt}
      account={receipt.account_id !== null ? accountNameOf(receipt.account_id, accounts, t) : null}
      calendar={calendar}
      footer={settings?.receipt_footer ?? ""}
      onClose={onClose}
      onPrinted={async (pdf) => {
        const next = await rpc("fin.docs.printed", { id: receipt.id, pdf });
        onChanged?.({ ...receipt, print_count: next.print_count });
        return next.print_count;
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
  const [language, setLanguage] = useState(lang);
  if (!letterhead) return null;
  return (
    <DocumentPreview
      title={t("inv.preview")}
      paper="a4"
      fileStem={invoice.number}
      testId="invoice-preview"
      onClose={onClose}
      onPrinted={async (pdf) => {
        const next = await rpc("fin.docs.printed", { id: invoice.id, pdf });
        onChanged?.({ ...invoice, print_count: next.print_count });
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
      sheet={<InvoiceSheet invoice={invoice} letterhead={letterhead} lang={language} calendar={calendar} numbering={(settings?.tooth_numbering ?? "fdi") as ToothNumbering} footer={billing?.receipt_footer} />}
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
