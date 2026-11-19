// The finance part of the in-browser mock Core (see mockCore.ts): enough accounts, documents and
// totals to look at and take screenshots of the finance screens without the Rust Core. The numbers
// are the end state of the worked example in docs/testing/manual/04b-money-ledger.md; the real
// posting rules live in core/src/fin and are covered by the Core's own tests.
/* eslint-disable @typescript-eslint/no-explicit-any */
import type {
  AccountInfo, AgingRow, CashCountInfo, CommissionRuleInfo, FinanceOverview, FinanceSettings, InstallmentDue, InvoiceInfo, ReceiptInfo, VoucherInfo,
} from "../../../shared/ts/contract";

const day = (n: number) => new Date(Date.now() + n * 86_400_000).toISOString().slice(0, 10);
const now = () => new Date().toISOString();
const AFN = 100;

const sys = (id: number, kind: AccountInfo["kind"], parent: number | null = null, money: AccountInfo["money"] = null, balance = 0): AccountInfo => ({
  id, parent_id: parent, kind, name: null, money, bank_name: null, bank_no: null, is_system: true, is_active: true, balance, version: 1,
});
const accounts: AccountInfo[] = [
  sys(1100, "asset"), sys(1110, "asset", 1100, "cash", 16_500 * AFN), { ...sys(1210, "asset", 1100, "bank", 14_350 * AFN), bank_name: "عزیزی بانک", bank_no: "0012345678" },
  { ...sys(1211, "asset", 1100, "mobile", 1_000 * AFN), name: "M-Paisa", is_system: false },
  sys(1310, "asset", null, null, 48_500 * AFN), sys(1320, "asset"), sys(1330, "asset"), sys(1340, "asset"), sys(1510, "asset"),
  sys(2110, "liability"), sys(2210, "liability"), sys(2310, "liability"), sys(2320, "liability"), sys(2410, "liability"), sys(2420, "liability"), sys(2430, "liability"), sys(2510, "liability"),
  sys(3110, "equity"), sys(3120, "equity"), sys(3210, "equity"), sys(3900, "equity"),
  sys(4110, "income"), sys(4190, "income"), sys(4910, "contra"), sys(4920, "contra"),
  sys(5000, "expense"), ...[5110, 5120, 5210, 5220, 5230, 5240, 5250, 5260, 5270, 5280].map((id) => sys(id, "expense", 5000)), sys(5810, "expense"), sys(5910, "expense"),
];

let settings: FinanceSettings = {
  share_enabled: true, share_basis: "treatment", receipt_footer: "با تشکر از مراجعه شما",
  legal: { legal_name: "کلینیک دندان آرتاویو", legal_form: "natural_person", tax_regime: "fixed", tin: "1000123456", business_license_no: null, health_license_no: "MoPH-4471", tax_office: "مستوفیت کابل", owner_name: "مالک نمونه", owner_father_name: null, official_address: "کابل، کارته سه" },
};
const rules: CommissionRuleInfo[] = [{ id: "cr1", doctor_id: null, service_id: null, kind: "percent", value: 500, version: 1 }];

const receipt = (id: number, serial: number, patient: [string, string, string], account: number, amount: number, kind: "rc" | "rf" = "rc", reason: string | null = null): ReceiptInfo => ({
  id, kind, number: `${kind === "rc" ? "RC" : "RF"}-1405-${String(serial).padStart(6, "0")}`, patient_id: patient[0], patient_name: patient[1], patient_number: patient[2],
  date: day(0), created_at: now(), account_id: account, amount: amount * AFN, allocations: [], to_deposit: 0, payer_name: null, note: null, reason, status: "posted",
  void_reason: null, owed_after: 0, deposit_after: 0, print_count: 1, created_by_name: "مالک نمونه",
});
const P1: [string, string, string] = ["p1", "مریم کریمی", "P-000001"];
const P3: [string, string, string] = ["p3", "حاجی غلام سخی نوری", "P-000003"];
const P4: [string, string, string] = ["p4", "نرگس حیدری", "P-000004"];
const P6: [string, string, string] = ["p6", "بیمار بدون تلفن", "P-000006"];
const receipts: ReceiptInfo[] = [
  receipt(1, 1, P1, 1110, 1_000), receipt(2, 2, P1, 1210, 350), receipt(3, 3, P1, 1110, 2_000), receipt(4, 4, P1, 1110, 8_000), receipt(5, 5, P1, 1110, 4_000),
  receipt(6, 6, P1, 1211, 1_000), receipt(7, 7, P3, 1110, 3_000), receipt(8, 8, P3, 1210, 4_000), receipt(9, 9, P4, 1110, 1_000), receipt(10, 10, P6, 1110, 3_000),
  receipt(11, 1, P4, 1110, 250, "rf", "خدمت کامل انجام نشد"),
];

const invoice = (id: number, patient: [string, string, string], gross: number, discount: number, allocated: number, reductions = 0): InvoiceInfo => {
  const net = (gross - discount) * AFN;
  const remaining = net - reductions * AFN - allocated * AFN;
  return {
    id, number: `INV-1405-${String(id).padStart(6, "0")}`, patient_id: patient[0], patient_name: patient[1], patient_number: patient[2], status: "posted",
    pay_state: remaining === 0 ? "paid" : allocated ? "partial" : "unpaid", date: day(0), created_at: now(), lines: [], gross: gross * AFN, discount: discount * AFN, net,
    reductions: reductions * AFN, written_off: 0, allocated: allocated * AFN, remaining, note: null, void_reason: null, plan: null, adjustments: [], allocations: [], print_count: 0, created_by_name: "مالک نمونه",
  };
};
const invoices: InvoiceInfo[] = [
  invoice(1, P1, 1_500, 150, 1_350), invoice(2, P1, 5_500, 0, 2_000), invoice(3, P1, 30_000, 2_000, 13_000), invoice(4, ["p2", "کودک نمونه", "P-000002"], 800, 0, 800),
  invoice(5, P3, 12_800, 800, 7_000), invoice(6, P4, 1_000, 0, 750, 250), invoice(7, ["p5", "عبدالله نیازی", "P-000005"], 25_000, 0, 0), invoice(8, P6, 3_000, 0, 3_000),
];

const voucher = (id: number, account: number | null, line: number, amount: number, withheld: number, name: string | null, description: string): VoucherInfo => ({
  id, kind: "pv", number: `PV-1405-${String(id).padStart(6, "0")}`, date: day(0), created_at: now(), account_id: account, purpose: "expense", source: null, supplier_id: null, supplier_name: null,
  name, reference: null, lines: [{ no: 1, account_id: line, description, amount: amount * AFN, withheld: withheld * AFN, withholding: withheld ? "rent" : null }],
  gross: amount * AFN, withheld: withheld * AFN, net: (amount - withheld) * AFN, due_date: null, note: null, status: "posted", void_reason: null, print_count: 0, created_by_name: "مالک نمونه",
});
const vouchers: VoucherInfo[] = [
  voucher(901, 1210, 5210, 5_000, 500, "مالک ساختمان", "کرایه ماه"), voucher(902, 1110, 5220, 600, 0, null, "قبض برق"), voucher(903, 1110, 5230, 400, 0, null, "خرید دستکش"),
];

const counts: CashCountInfo[] = [
  { id: 950, number: "CC-1405-000001", account_id: 1110, date: day(0), created_at: now(), book: 16_550 * AFN, counted: 16_500 * AFN, difference: -50 * AFN, reason: "اشتباه شمارش تست", status: "posted", created_by_name: "مالک نمونه" },
];

const overview = (p: any): FinanceOverview => ({
  from: p.from, to: p.to, receivables: 48_500 * AFN, deposits: 0, payables: 0, tax_withheld: 0, overdue_installments: 4_000 * AFN,
  checks: ["ledger_balanced", "patients", "deposits", "suppliers", "bridge", "money"].map((key) => ({ key, ok: true, left: 0, right: 0 })),
  cash: {
    received: 28_150 * AFN, refunded: 250 * AFN, net_received: 27_900 * AFN,
    expenses: [{ account_id: 5210, amount: 5_000 * AFN }, { account_id: 5220, amount: 600 * AFN }, { account_id: 5230, amount: 400 * AFN }], expenses_total: 6_000 * AFN,
    suppliers_paid: 0, operating: 21_900 * AFN, withheld: 500 * AFN, owner_in: 10_000 * AFN, owner_out: 0, loans_in: 0, loans_out: 0, tax_paid: 500 * AFN, other_income: 0, count_differences: -50 * AFN,
    money: [
      { account_id: 1110, opening: 0, inflow: 22_800 * AFN, outflow: 6_300 * AFN, closing: 16_500 * AFN },
      { account_id: 1210, opening: 0, inflow: 19_350 * AFN, outflow: 5_000 * AFN, closing: 14_350 * AFN },
      { account_id: 1211, opening: 0, inflow: 1_000 * AFN, outflow: 0, closing: 1_000 * AFN },
    ],
    money_opening: 0, money_closing: 31_850 * AFN,
  },
  accrual: {
    gross: 79_600 * AFN, discounts: 2_950 * AFN, reductions: 250 * AFN, other_income: 0, revenue: 76_400 * AFN,
    expenses: [{ account_id: 5210, amount: 5_000 * AFN }, { account_id: 5220, amount: 600 * AFN }, { account_id: 5230, amount: 400 * AFN }, { account_id: 5910, amount: 50 * AFN }],
    expenses_total: 6_050 * AFN, write_offs: 0, result: 70_350 * AFN,
  },
});

const aging: AgingRow[] = [
  { patient_id: "p5", patient_name: "عبدالله نیازی", patient_number: "P-000005", phone: "0700111222", invoice_count: 1, d0_30: 25_000 * AFN, d31_60: 0, d61_90: 0, d90_plus: 0, total: 25_000 * AFN, deposit: 0, oldest_date: day(0), last_receipt: null, next_due: day(30) },
  { patient_id: "p1", patient_name: "مریم کریمی", patient_number: "P-000001", phone: "0700333444", invoice_count: 2, d0_30: 18_500 * AFN, d31_60: 0, d61_90: 0, d90_plus: 0, total: 18_500 * AFN, deposit: 0, oldest_date: day(0), last_receipt: day(0), next_due: day(0) },
  { patient_id: "p3", patient_name: "حاجی غلام سخی نوری", patient_number: "P-000003", phone: null, invoice_count: 1, d0_30: 5_000 * AFN, d31_60: 0, d61_90: 0, d90_plus: 0, total: 5_000 * AFN, deposit: 0, oldest_date: day(0), last_receipt: day(0), next_due: null },
];
const due: InstallmentDue[] = [
  { patient_id: "p1", patient_name: "مریم کریمی", patient_number: "P-000001", phone: "0700333444", invoice_id: 3, invoice_number: "INV-1405-000003", no: 3, count: 10, due_date: day(0), amount: 2_000 * AFN, paid: 1_000 * AFN, state: "partial", days_late: 0 },
  { patient_id: "p1", patient_name: "مریم کریمی", patient_number: "P-000001", phone: "0700333444", invoice_id: 3, invoice_number: "INV-1405-000003", no: 4, count: 10, due_date: day(30), amount: 2_000 * AFN, paid: 0, state: "pending", days_late: 0 },
];

const inRange = (d: string, p: any) => (!p.from || d >= p.from) && (!p.to || d <= p.to);

/** Answers a finance method, or `undefined` when the method is not one of these (supplier payments stay in mockInventory). */
export function financeCall(method: string, p: any): unknown {
  switch (method) {
    case "fin.accounts.list":
      return accounts.filter((a) => p?.include_inactive || a.is_active);
    case "fin.accounts.save": {
      const a = accounts.find((x) => x.id === p.id);
      if (a) Object.assign(a, { name: p.name ?? a.name, bank_name: p.bank_name, bank_no: p.bank_no, is_active: p.is_active, version: a.version + 1 });
      else accounts.push({ id: p.money ? 1212 + accounts.length : 5300 + accounts.length, parent_id: p.money ? 1100 : 5000, kind: p.money ? "asset" : "expense", name: p.name, money: p.money ?? null, bank_name: p.bank_name ?? null, bank_no: p.bank_no ?? null, is_system: false, is_active: true, balance: 0, version: 1 });
      return a ?? accounts[accounts.length - 1];
    }
    case "fin.overview":
      return overview(p);
    case "fin.settings.get":
      return settings;
    case "fin.settings.save":
      settings = { ...settings, ...p };
      return settings;
    case "fin.shares.rules_list":
      return rules;
    case "fin.shares.preview":
      return { basis: settings.share_basis, from: p.from, to: p.to, doctors: [], base_total: 0, commission_total: 0 };
    case "fin.receipts.list":
      return receipts.filter((r) => inRange(r.date, p) && (!p.patient_id || r.patient_id === p.patient_id));
    case "fin.invoices.list":
      return invoices.filter((i) => inRange(i.date, p) && (!p.patient_id || i.patient_id === p.patient_id) && (!p.open_only || i.remaining > 0));
    case "fin.vouchers.list":
      if (p.supplier_id) return undefined;
      return vouchers.filter((v) => inRange(v.date, p) && (!p.kind || v.kind === p.kind));
    case "fin.receivables.aging":
      return aging;
    case "fin.receivables.due":
      return due;
    case "fin.cash.day": {
      const a = p.account_id ?? 1110;
      if (a !== 1110) return { account_id: a, date: p.date, opening: 0, received: 0, refunded: 0, paid_out: 0, other_in: 0, transfers: 0, adjustments: 0, closing: accounts.find((x) => x.id === a)?.balance ?? 0, receipt_count: 0, counts: [] };
      return { account_id: 1110, date: p.date, opening: 0, received: 22_800 * AFN, refunded: 250 * AFN, paid_out: 1_000 * AFN, other_in: 0, transfers: -5_000 * AFN, adjustments: -50 * AFN, closing: 16_500 * AFN, receipt_count: 8, counts };
    }
    case "fin.docs.void": {
      const v = vouchers.find((x) => x.id === p.id);
      if (!v) return undefined;
      v.status = "void";
      v.void_reason = p.reason;
      return null;
    }
    default:
      return undefined;
  }
}
