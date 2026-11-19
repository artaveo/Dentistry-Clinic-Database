//! Finance (docs/design/finance-standard.md): a double-entry ledger behind every money document —
//! invoices, receipts and their allocation to invoices, refunds, price reductions and write-offs,
//! installment plans, supplier bills, payment vouchers, other receipts, transfers and cash counts.
//! All amounts are AFN × 100 (ADR-06); dates are clinic-local `YYYY-MM-DD`; instants are UTC
//! ISO-8601 (ADR-07). Document ids are numbers; their printed number is `INV-1405-000001`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Translations;

fn yes() -> bool {
    true
}

// ───────────────────────────── codes ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Income,
    /// Takes income off (discounts, price reductions).
    Contra,
    Expense,
}

/// What kind of money an account holds; `None` for every other account.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MoneyKind {
    Cash,
    Bank,
    Mobile,
}

/// Every kind of finance document; the code is also the lower-case number prefix.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DocKind {
    /// Invoice for services (INV).
    Inv,
    /// Receipt: money from a patient (RC).
    Rc,
    /// Allocation of a patient's deposit to invoices, or back to the deposit (AL).
    Al,
    /// Refund of a patient's deposit (RF).
    Rf,
    /// Price reduction on an invoice (CN).
    Cn,
    /// Write-off of an uncollectable debt (WO).
    Wo,
    /// Installment plan (PP).
    Pp,
    /// Supplier or lab bill (BL).
    Bl,
    /// Payment voucher: money paid out (PV).
    Pv,
    /// Other receipt: money in that is not from a patient (OR).
    Or,
    /// Transfer between money accounts (TR).
    Tr,
    /// Cash count at the end of a day (CC).
    Cc,
    /// Monthly payroll: salaries earned, tax withheld and advances taken back (PR).
    Pr,
    /// A doctor's share statement, frozen when approved (DS).
    Ds,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocStatus {
    Posted,
    Void,
    /// An installment plan replaced by a newer version.
    Superseded,
}

/// How much of an invoice is settled (derived, never stored).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PayState {
    Unpaid,
    Partial,
    Paid,
    Void,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallmentState {
    Pending,
    Partial,
    Paid,
    Overdue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdjustKind {
    /// The price was lowered (a contra-income).
    Reduction,
    /// The debt will not be collected (an expense).
    WriteOff,
}

/// Why money is paid out.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoucherPurpose {
    /// A clinic expense paid at once (rent, electricity, materials bought for cash…).
    Expense,
    /// Paying what the clinic owes a supplier or lab.
    Supplier,
    /// Paying withheld tax to the government.
    Tax,
    /// The owner takes money out.
    Drawing,
    /// Paying back a loan.
    Loan,
    Other,
    /// Paying an employee the salary a payroll owes them.
    Salary,
    /// An advance on salary, taken back from a later payroll.
    Advance,
    /// Paying a doctor the share an approved statement owes them.
    Share,
}

/// Tax the clinic withheld from a payment (the amount is entered by hand; the app never computes it).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WithholdingKind {
    /// Income Tax Law art. 59: rent of a business building.
    Rent,
    /// Art. 72: a contractor.
    Contractor,
    /// Salary tax withheld by the payroll (only for paying it to the government).
    Wage,
}

/// Money in that is not from a patient.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OtherReceiptSource {
    /// The owner puts money in.
    Capital,
    Loan,
    /// Other income (selling something).
    Sale,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommissionBasis {
    /// Doctor share on money received.
    Payment,
    /// Doctor share on treatments invoiced.
    Treatment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommissionKind {
    /// `value` is hundredths of a percent (2500 = 25 %).
    Percent,
    /// `value` is AFN × 100 per invoiced unit.
    Fixed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LegalForm {
    NaturalPerson,
    Partnership,
    Llc,
    Corporation,
}

/// Only a label on official reports; the app never computes tax.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaxRegime {
    Unknown,
    Fixed,
    IncomeBrt,
}

// ───────────────────────────── common ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocIdParams {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct VoidDocParams {
    pub id: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocPrintedParams {
    pub id: i64,
    /// Saved as a PDF instead of printed on paper.
    #[serde(default)]
    pub pdf: bool,
}

/// The head of any finance document.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DocSummary {
    pub id: i64,
    pub kind: DocKind,
    /// `RC-1405-000012`
    pub number: String,
    pub date: String,
    pub status: DocStatus,
    pub amount: i64,
    pub print_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct PeriodParams {
    pub from: String,
    pub to: String,
}

// ───────────────────────────── accounts ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AccountInfo {
    /// The account code (1110).
    pub id: i64,
    pub parent_id: Option<i64>,
    pub kind: AccountType,
    /// None for a built-in account: the UI shows its own label for the code.
    pub name: Option<String>,
    pub money: Option<MoneyKind>,
    pub bank_name: Option<String>,
    pub bank_no: Option<String>,
    pub is_system: bool,
    pub is_active: bool,
    /// Debits − credits so far (today included).
    pub balance: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct AccountListParams {
    #[serde(default)]
    pub include_inactive: bool,
}

/// A clinic's own expense account (under 5000) or money account (cash or bank, under 1100).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveAccountParams {
    /// None = a new account; Core picks the next free code under the parent.
    #[serde(default)]
    pub id: Option<i64>,
    /// 5000 for an expense account, 1100 for a money account (new accounts only).
    #[serde(default)]
    pub parent_id: Option<i64>,
    /// Required for a clinic's own account; ignored for a built-in one.
    #[serde(default)]
    pub name: Option<String>,
    /// For a new money account: cash, bank or mobile.
    #[serde(default)]
    pub money: Option<MoneyKind>,
    #[serde(default)]
    pub bank_name: Option<String>,
    #[serde(default)]
    pub bank_no: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
    #[serde(default)]
    pub version: Option<i64>,
}

// ───────────────────────────── invoices ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InvoiceLineInfo {
    pub no: i64,
    pub treatment_item_id: Option<String>,
    pub service_id: Option<String>,
    /// The service name as invoiced.
    pub name: String,
    pub teeth: Vec<u8>,
    pub doctor_id: Option<String>,
    pub doctor_name: Option<String>,
    pub quantity: i64,
    pub unit_price: i64,
    /// The treatment's price before it was changed for this invoice (0 = none to compare with).
    pub listed_price: i64,
    /// `quantity × unit_price`.
    pub gross: i64,
    /// This line's discount, its share of the invoice discount included.
    pub discount: i64,
    /// `gross − discount`.
    pub net: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InstallmentInfo {
    pub no: i64,
    pub due_date: String,
    pub amount: i64,
    /// Covered so far by what was allocated to the invoice (oldest installment first).
    pub paid: i64,
    pub state: InstallmentState,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PlanInfo {
    pub id: i64,
    pub number: String,
    /// 1, 2, … each time the plan is replaced.
    pub version: i64,
    pub date: String,
    pub reason: Option<String>,
    pub installments: Vec<InstallmentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AdjustmentInfo {
    pub id: i64,
    pub number: String,
    pub kind: AdjustKind,
    pub date: String,
    pub amount: i64,
    pub reason: String,
    pub status: DocStatus,
}

/// Money put towards an invoice: by a receipt, or from the deposit (negative = taken back).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AllocationInfo {
    pub doc_id: i64,
    pub kind: DocKind,
    pub number: String,
    pub date: String,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InvoiceInfo {
    pub id: i64,
    pub number: String,
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub status: DocStatus,
    pub pay_state: PayState,
    pub date: String,
    pub created_at: String,
    pub lines: Vec<InvoiceLineInfo>,
    pub gross: i64,
    pub discount: i64,
    /// `gross − discount`: what was invoiced.
    pub net: i64,
    /// Price reductions after the invoice (posted ones).
    pub reductions: i64,
    pub written_off: i64,
    /// Money put towards the invoice (receipts and deposit).
    pub allocated: i64,
    /// `net − reductions − written_off − allocated`.
    pub remaining: i64,
    pub note: Option<String>,
    pub void_reason: Option<String>,
    pub plan: Option<PlanInfo>,
    pub adjustments: Vec<AdjustmentInfo>,
    pub allocations: Vec<AllocationInfo>,
    pub print_count: i64,
    pub created_by_name: Option<String>,
}

/// A completed treatment not on any invoice yet.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BillableItem {
    pub item_id: String,
    pub service_id: String,
    pub names: Translations,
    pub teeth: Vec<u8>,
    pub surfaces: String,
    pub units: i64,
    pub unit_price: i64,
    pub doctor_id: Option<String>,
    pub completed_at: Option<String>,
    pub details: Option<String>,
    /// CT-08: false for a multi-session course (orthodontics) invoiced while still in progress.
    pub completed: bool,
    pub sessions_done: i64,
    pub sessions_planned: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InvoiceItemPick {
    pub item_id: String,
    /// None = the treatment's price. A different price needs `billing.override`.
    #[serde(default)]
    pub unit_price: Option<i64>,
    /// Discount on this line.
    #[serde(default)]
    pub discount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NewInvoiceLine {
    #[serde(default)]
    pub service_id: Option<String>,
    /// Required when there is no service.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub teeth: Vec<u8>,
    pub quantity: i64,
    pub unit_price: i64,
    #[serde(default)]
    pub discount: i64,
    #[serde(default)]
    pub doctor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NewInstallment {
    pub due_date: String,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateInvoiceParams {
    pub patient_id: String,
    #[serde(default)]
    pub items: Vec<InvoiceItemPick>,
    #[serde(default)]
    pub extra_lines: Vec<NewInvoiceLine>,
    /// Discount on the whole invoice, shared out over the lines.
    #[serde(default)]
    pub discount: i64,
    #[serde(default)]
    pub note: Option<String>,
    /// Put the patient's deposit towards the new invoice at once.
    #[serde(default)]
    pub use_deposit: bool,
    #[serde(default)]
    pub installments: Vec<NewInstallment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AdjustInvoiceParams {
    pub invoice_id: i64,
    pub kind: AdjustKind,
    pub amount: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetPlanParams {
    pub invoice_id: i64,
    /// Empty = remove the plan.
    pub installments: Vec<NewInstallment>,
    /// Why a plan is replaced (required when one exists).
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct InvoiceListParams {
    #[serde(default)]
    pub patient_id: Option<String>,
    /// Only invoices still owing money.
    #[serde(default)]
    pub open_only: bool,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    /// Invoice number or patient name / number.
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

// ───────────────────────────── receipts, allocations, refunds ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AllocationLine {
    pub invoice_id: i64,
    pub invoice_number: String,
    pub amount: i64,
}

/// A receipt (RC), a deposit allocation (AL) or a refund (RF): money and a patient.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReceiptInfo {
    pub id: i64,
    pub kind: DocKind,
    pub number: String,
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub date: String,
    pub created_at: String,
    /// The money account (none for an allocation).
    pub account_id: Option<i64>,
    pub amount: i64,
    pub allocations: Vec<AllocationLine>,
    /// The part of a receipt left as the patient's deposit.
    pub to_deposit: i64,
    pub payer_name: Option<String>,
    pub note: Option<String>,
    pub reason: Option<String>,
    pub status: DocStatus,
    pub void_reason: Option<String>,
    /// What the patient still owed on invoices right after this document.
    pub owed_after: i64,
    /// The patient's deposit right after this document.
    pub deposit_after: i64,
    pub print_count: i64,
    pub created_by_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NewAllocation {
    pub invoice_id: i64,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateReceiptParams {
    pub patient_id: String,
    pub account_id: i64,
    pub amount: i64,
    /// Where the money goes; empty with `auto_allocate` = oldest invoices first; the rest is deposit.
    #[serde(default)]
    pub allocations: Vec<NewAllocation>,
    #[serde(default)]
    pub auto_allocate: bool,
    #[serde(default)]
    pub payer_name: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Put deposit towards invoices (positive) or take money off an invoice back to the deposit
/// (negative amounts; a reason is required then).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AllocateParams {
    pub patient_id: String,
    pub allocations: Vec<NewAllocation>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Money handed back from the patient's deposit. With `from_invoice_id`, that much is first taken
/// off the invoice back to the deposit (one step for "refund this payment").
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RefundParams {
    pub patient_id: String,
    pub account_id: i64,
    pub amount: i64,
    pub reason: String,
    #[serde(default)]
    pub from_invoice_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct ReceiptListParams {
    #[serde(default)]
    pub patient_id: Option<String>,
    /// rc, al or rf; None = all three.
    #[serde(default)]
    pub kind: Option<DocKind>,
    #[serde(default)]
    pub account_id: Option<i64>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

// ───────────────────────────── patient account, receivables ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InstallmentDue {
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub phone: Option<String>,
    pub invoice_id: i64,
    pub invoice_number: String,
    pub no: i64,
    pub count: i64,
    pub due_date: String,
    pub amount: i64,
    pub paid: i64,
    pub state: InstallmentState,
    /// Days past the due date (0 when not due yet).
    pub days_late: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct InstallmentsDueParams {
    /// Due on or before this day (default today); unpaid ones only.
    #[serde(default)]
    pub until: Option<String>,
    #[serde(default)]
    pub overdue_only: bool,
    #[serde(default)]
    pub patient_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PatientAccount {
    pub patient_id: String,
    /// Net of all posted invoices.
    pub invoiced: i64,
    pub reductions: i64,
    pub written_off: i64,
    pub allocated: i64,
    /// Still owed on invoices.
    pub owed: i64,
    /// Money held for the patient (not yet put on an invoice).
    pub deposit: i64,
    /// `owed − deposit`: positive = the patient owes the clinic.
    pub balance: i64,
    pub billable_count: i64,
    pub billable_total: i64,
    pub overdue: i64,
    pub next_due: Option<InstallmentDue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StatementLine {
    pub date: String,
    pub doc_id: i64,
    pub kind: DocKind,
    pub number: String,
    /// What it is for (invoice lines, reason, …).
    pub description: String,
    /// Raises what the patient owes (invoice).
    pub charge: i64,
    /// Lowers it (money received, reductions, write-offs) — refunds count negative.
    pub credit: i64,
    /// `owed − deposit` after this line.
    pub balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StatementInfo {
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub phone: Option<String>,
    pub from: Option<String>,
    pub to: String,
    pub opening: i64,
    pub lines: Vec<StatementLine>,
    pub charges: i64,
    pub credits: i64,
    pub closing: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StatementParams {
    pub patient_id: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

/// One patient's open invoices by how long they have been owed.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AgingRow {
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub phone: Option<String>,
    pub invoice_count: i64,
    pub d0_30: i64,
    pub d31_60: i64,
    pub d61_90: i64,
    pub d90_plus: i64,
    pub total: i64,
    pub deposit: i64,
    pub oldest_date: Option<String>,
    pub last_receipt: Option<String>,
    pub next_due: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct AgingParams {
    /// Owed as of this day (default today).
    #[serde(default)]
    pub as_of: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

// ───────────────────────────── money out, bills, money in, transfers ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct VoucherLineInfo {
    pub no: i64,
    pub account_id: i64,
    pub description: Option<String>,
    /// Gross amount of the line.
    pub amount: i64,
    pub withheld: i64,
    pub withholding: Option<WithholdingKind>,
}

/// A payment voucher (PV), a supplier bill (BL) or an other receipt (OR).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct VoucherInfo {
    pub id: i64,
    pub kind: DocKind,
    pub number: String,
    pub date: String,
    pub created_at: String,
    /// The money account (none for a bill).
    pub account_id: Option<i64>,
    pub purpose: Option<VoucherPurpose>,
    pub source: Option<OtherReceiptSource>,
    pub supplier_id: Option<String>,
    pub supplier_name: Option<String>,
    /// The employee paid (salary, advance).
    pub employee_id: Option<String>,
    /// The doctor paid (share).
    pub doctor_id: Option<String>,
    /// The supplier, employee or doctor paid, by name.
    pub party_name: Option<String>,
    /// Who was paid / who paid (free text).
    pub name: Option<String>,
    /// Bank slip, vendor's bill number, …
    pub reference: Option<String>,
    pub lines: Vec<VoucherLineInfo>,
    pub gross: i64,
    pub withheld: i64,
    /// Money that actually moved: `gross − withheld`.
    pub net: i64,
    pub due_date: Option<String>,
    pub note: Option<String>,
    pub status: DocStatus,
    pub void_reason: Option<String>,
    pub print_count: i64,
    pub created_by_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NewVoucherLine {
    pub account_id: i64,
    pub amount: i64,
    #[serde(default)]
    pub description: Option<String>,
    /// Tax withheld from this line, entered by hand (vouchers only).
    #[serde(default)]
    pub withheld: i64,
    #[serde(default)]
    pub withholding: Option<WithholdingKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateVoucherParams {
    pub date: String,
    pub account_id: i64,
    pub purpose: VoucherPurpose,
    /// Required for `supplier`; optional for an expense bought from a known supplier.
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// Required for `salary` and `advance`.
    #[serde(default)]
    pub employee_id: Option<String>,
    /// Required for `share`.
    #[serde(default)]
    pub doctor_id: Option<String>,
    /// Expense lines; for `supplier` one line is made from `amount`.
    #[serde(default)]
    pub lines: Vec<NewVoucherLine>,
    /// For supplier / tax / drawing / loan / other: the amount paid.
    #[serde(default)]
    pub amount: i64,
    /// For `tax`: which withheld tax is paid.
    #[serde(default)]
    pub withholding: Option<WithholdingKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateBillParams {
    pub supplier_id: String,
    pub date: String,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    pub lines: Vec<NewVoucherLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateOtherReceiptParams {
    pub date: String,
    pub account_id: i64,
    pub source: OtherReceiptSource,
    pub amount: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct VoucherListParams {
    /// pv, bl or or; None = all three.
    #[serde(default)]
    pub kind: Option<DocKind>,
    #[serde(default)]
    pub purpose: Option<VoucherPurpose>,
    /// Vouchers with a line on this (expense) account, or paid from this money account.
    #[serde(default)]
    pub account_id: Option<i64>,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateTransferParams {
    pub date: String,
    pub from_account_id: i64,
    pub to_account_id: i64,
    pub amount: i64,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

// ───────────────────────────── cash and bank ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CashCountInfo {
    pub id: i64,
    pub number: String,
    pub account_id: i64,
    pub date: String,
    pub created_at: String,
    /// What the books say was in the account at the count.
    pub book: i64,
    pub counted: i64,
    /// `counted − book`.
    pub difference: i64,
    pub reason: Option<String>,
    pub status: DocStatus,
    pub created_by_name: Option<String>,
}

/// One money account on one day.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CashDayInfo {
    pub account_id: i64,
    pub date: String,
    pub opening: i64,
    /// From patients (receipts).
    pub received: i64,
    /// Back to patients (refunds).
    pub refunded: i64,
    /// Paid out (vouchers).
    pub paid_out: i64,
    /// Other money in (capital, loans, sales).
    pub other_in: i64,
    /// Transfers in − transfers out.
    pub transfers: i64,
    /// Over/short booked by earlier counts that day.
    pub adjustments: i64,
    pub closing: i64,
    pub receipt_count: i64,
    pub counts: Vec<CashCountInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CashDayParams {
    pub account_id: i64,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateCashCountParams {
    pub account_id: i64,
    pub date: String,
    pub counted: i64,
    /// Required when the count differs from the books.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct CashCountListParams {
    #[serde(default)]
    pub account_id: Option<i64>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LedgerLine {
    pub date: String,
    pub doc_id: i64,
    pub kind: DocKind,
    pub number: String,
    pub description: String,
    /// + debit, − credit.
    pub amount: i64,
    pub balance: i64,
    /// A reversing row written when the document was voided.
    pub reversal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AccountLedger {
    pub account_id: i64,
    pub from: String,
    pub to: String,
    pub opening: i64,
    pub lines: Vec<LedgerLine>,
    pub closing: i64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AccountLedgerParams {
    pub account_id: i64,
    /// Only this supplier's or patient's rows (sub-ledger).
    #[serde(default)]
    pub party_id: Option<String>,
    pub from: String,
    pub to: String,
}

// ───────────────────────────── overview & reconciliation ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AccountTotal {
    pub account_id: i64,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MoneyAccountTotal {
    pub account_id: i64,
    pub opening: i64,
    pub inflow: i64,
    pub outflow: i64,
    pub closing: i64,
}

/// Money that moved (Income Tax Law art. 38: the cash method).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CashBasis {
    /// From patients.
    pub received: i64,
    /// Back to patients.
    pub refunded: i64,
    /// `received − refunded`.
    pub net_received: i64,
    /// Paid for expenses, by expense account.
    pub expenses: Vec<AccountTotal>,
    pub expenses_total: i64,
    /// Paid to suppliers and labs.
    pub suppliers_paid: i64,
    /// Salaries, advances and doctor shares paid.
    pub staff_paid: i64,
    /// `net_received − expenses_total − suppliers_paid`.
    pub operating: i64,
    /// Tax kept back from payments in the period (owed to the government, not money out yet).
    pub withheld: i64,
    pub owner_in: i64,
    pub owner_out: i64,
    pub loans_in: i64,
    pub loans_out: i64,
    pub tax_paid: i64,
    pub other_income: i64,
    /// Over (+) / short (−) at cash counts.
    pub count_differences: i64,
    pub money: Vec<MoneyAccountTotal>,
    pub money_opening: i64,
    pub money_closing: i64,
}

/// Work done and invoiced (art. 37: the accrual method).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AccrualBasis {
    pub gross: i64,
    pub discounts: i64,
    pub reductions: i64,
    pub other_income: i64,
    /// `gross − discounts − reductions + other_income`.
    pub revenue: i64,
    pub expenses: Vec<AccountTotal>,
    pub expenses_total: i64,
    pub write_offs: i64,
    /// `revenue − expenses_total − write_offs`.
    pub result: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CheckResult {
    /// `ledger_balanced`, `patients`, `deposits`, `suppliers`, `bridge`, `money`.
    pub key: String,
    pub ok: bool,
    /// The two numbers compared.
    pub left: i64,
    pub right: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct FinanceOverview {
    pub from: String,
    pub to: String,
    pub cash: CashBasis,
    pub accrual: AccrualBasis,
    /// Owed by patients on invoices (end of period).
    pub receivables: i64,
    /// Held for patients (end of period).
    pub deposits: i64,
    /// Owed to suppliers and labs (end of period).
    pub payables: i64,
    /// Withheld tax not yet paid to the government (end of period).
    pub tax_withheld: i64,
    pub overdue_installments: i64,
    pub checks: Vec<CheckResult>,
}

// ───────────────────────────── settings, doctor shares ─────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct LegalProfile {
    #[serde(default)]
    pub legal_name: Option<String>,
    #[serde(default)]
    pub legal_form: Option<LegalForm>,
    #[serde(default)]
    pub tax_regime: Option<TaxRegime>,
    /// Taxpayer identification number.
    #[serde(default)]
    pub tin: Option<String>,
    #[serde(default)]
    pub business_license_no: Option<String>,
    #[serde(default)]
    pub health_license_no: Option<String>,
    #[serde(default)]
    pub tax_office: Option<String>,
    #[serde(default)]
    pub owner_name: Option<String>,
    #[serde(default)]
    pub owner_father_name: Option<String>,
    #[serde(default)]
    pub official_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct FinanceSettings {
    pub share_enabled: bool,
    pub share_basis: CommissionBasis,
    /// Printed at the foot of every receipt.
    pub receipt_footer: String,
    pub legal: LegalProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CommissionRuleInfo {
    pub id: String,
    /// None = every doctor.
    pub doctor_id: Option<String>,
    /// None = every service.
    pub service_id: Option<String>,
    pub kind: CommissionKind,
    pub value: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveCommissionRuleParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub service_id: Option<String>,
    pub kind: CommissionKind,
    pub value: i64,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CommissionReportParams {
    #[serde(default)]
    pub doctor_id: Option<String>,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CommissionLine {
    pub invoice_id: i64,
    pub service_id: Option<String>,
    pub date: String,
    pub invoice_number: String,
    pub patient_name: String,
    pub name: String,
    /// What the share is taken from.
    pub base: i64,
    pub commission: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DoctorCommission {
    pub doctor_id: String,
    pub doctor_name: String,
    pub base: i64,
    pub commission: i64,
    pub lines: Vec<CommissionLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CommissionReport {
    pub basis: CommissionBasis,
    pub from: String,
    pub to: String,
    pub doctors: Vec<DoctorCommission>,
    pub base_total: i64,
    pub commission_total: i64,
}

// ───────────────────────────── staff and payroll ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct EmployeeInfo {
    pub id: String,
    pub full_name: String,
    pub father_name: Option<String>,
    /// Taxpayer number (on the payroll register).
    pub tin: Option<String>,
    pub national_id: Option<String>,
    pub phone: Option<String>,
    pub position: String,
    /// When the employee is also one of the clinic's doctors.
    pub doctor_id: Option<String>,
    pub hire_date: String,
    pub end_date: Option<String>,
    /// Monthly base salary, the payroll's starting point.
    pub base_salary: i64,
    pub note: Option<String>,
    pub is_active: bool,
    /// Salary approved and not paid yet.
    pub owed: i64,
    /// Advances paid and not yet taken back.
    pub advances: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveEmployeeParams {
    #[serde(default)]
    pub id: Option<String>,
    pub full_name: String,
    #[serde(default)]
    pub father_name: Option<String>,
    #[serde(default)]
    pub tin: Option<String>,
    #[serde(default)]
    pub national_id: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    pub position: String,
    #[serde(default)]
    pub doctor_id: Option<String>,
    pub hire_date: String,
    #[serde(default)]
    pub end_date: Option<String>,
    pub base_salary: i64,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct EmployeeListParams {
    #[serde(default)]
    pub include_inactive: bool,
}

/// One employee's line of a payroll. Gross = base + extra + commission; net = gross − advance − tax.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PayrollLineInfo {
    pub employee_id: String,
    pub full_name: String,
    pub father_name: Option<String>,
    pub tin: Option<String>,
    pub position: String,
    pub base: i64,
    /// Allowances and overtime.
    pub extra: i64,
    pub commission: i64,
    pub gross: i64,
    /// Advance taken back.
    pub advance: i64,
    /// Salary tax withheld (entered by hand).
    pub tax: i64,
    pub net: i64,
    /// Paid so far on this employee's salary (all payrolls, oldest first).
    pub paid: i64,
    /// Advances outstanding before this payroll (draft only).
    pub advances_open: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PayrollInfo {
    /// None for a draft that is not approved yet.
    pub id: Option<i64>,
    pub number: Option<String>,
    pub from: String,
    pub to: String,
    pub date: Option<String>,
    pub status: Option<DocStatus>,
    pub lines: Vec<PayrollLineInfo>,
    pub gross: i64,
    pub advance: i64,
    pub tax: i64,
    pub net: i64,
    pub void_reason: Option<String>,
    pub created_by_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PayrollDraftParams {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NewPayrollLine {
    pub employee_id: String,
    pub base: i64,
    #[serde(default)]
    pub extra: i64,
    #[serde(default)]
    pub commission: i64,
    #[serde(default)]
    pub advance: i64,
    #[serde(default)]
    pub tax: i64,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreatePayrollParams {
    pub from: String,
    pub to: String,
    /// The day the payroll is approved (booked).
    pub date: String,
    pub lines: Vec<NewPayrollLine>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PayrollListParams {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

/// An approved doctor's share statement: the lines are frozen; later price reductions do not change it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ShareStatementInfo {
    pub id: i64,
    pub number: String,
    pub doctor_id: String,
    pub doctor_name: String,
    pub from: String,
    pub to: String,
    pub date: String,
    pub basis: CommissionBasis,
    pub lines: Vec<CommissionLine>,
    pub base: i64,
    pub share: i64,
    pub status: DocStatus,
    pub void_reason: Option<String>,
    pub print_count: i64,
    pub created_by_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateShareStatementParams {
    pub doctor_id: String,
    pub from: String,
    pub to: String,
    /// The day the statement is approved (booked).
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ShareStatementListParams {
    #[serde(default)]
    pub doctor_id: Option<String>,
}

/// What a doctor is owed from approved statements and what was paid.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DoctorShareBalance {
    pub doctor_id: String,
    pub doctor_name: String,
    pub approved: i64,
    pub paid: i64,
    pub owed: i64,
}
