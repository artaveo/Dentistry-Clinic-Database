//! Phase 6 — billing: invoices, payments and receipts, debts, installments, doctor commission,
//! clinic expenses and the daily cash closing. All amounts are AFN × 100 (ADR-06); all dates are
//! clinic-local `YYYY-MM-DD`; instants are UTC ISO-8601 (ADR-07).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Translations;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Issued,
    Void,
}

/// How much of an invoice is paid (derived, never stored).
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
pub enum PaymentKind {
    /// Money received: for an invoice, or an advance (no invoice) that becomes the patient's credit.
    Payment,
    /// The patient's credit put towards an invoice (no money moves).
    Credit,
    /// Money handed back (or credit given back) against an earlier payment.
    Refund,
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
pub enum CommissionBasis {
    /// Commission on money received.
    Payment,
    /// Commission on treatments invoiced.
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

// ───────────────────────────── payment methods ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PaymentMethodInfo {
    pub id: String,
    pub code: String,
    pub name: Translations,
    /// Counts in the cash drawer at closing.
    pub is_cash: bool,
    pub is_system: bool,
    pub is_active: bool,
    pub sort_order: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SavePaymentMethodParams {
    /// None = a new method.
    #[serde(default)]
    pub id: Option<String>,
    pub name: Translations,
    #[serde(default)]
    pub is_cash: bool,
    #[serde(default = "yes")]
    pub is_active: bool,
    #[serde(default)]
    pub version: Option<i64>,
}

fn yes() -> bool {
    true
}

// ───────────────────────────── invoices ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InvoiceLineInfo {
    pub id: String,
    pub treatment_item_id: Option<String>,
    pub service_id: Option<String>,
    pub names: Translations,
    pub teeth: Vec<u8>,
    pub surfaces: String,
    pub doctor_id: Option<String>,
    pub quantity: i64,
    pub unit_price: i64,
    /// The treatment's price before it was changed for this invoice (0 = none to compare with).
    pub listed_price: i64,
    pub line_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InstallmentInfo {
    pub no: i64,
    pub due_date: String,
    pub amount: i64,
    /// Covered so far by what the invoice has received (oldest installment first).
    pub paid: i64,
    pub state: InstallmentState,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreditNoteInfo {
    pub id: String,
    pub number: String,
    pub amount: i64,
    pub reason: String,
    pub issued_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InvoiceInfo {
    pub id: String,
    /// `INV-1405-000001`
    pub number: String,
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub status: InvoiceStatus,
    pub pay_state: PayState,
    pub issued_at: String,
    pub local_date: String,
    pub lines: Vec<InvoiceLineInfo>,
    pub subtotal: i64,
    pub discount: i64,
    pub credit_notes_total: i64,
    /// Subtotal − discount − credit notes: what the patient owes for this invoice.
    pub net: i64,
    /// Received for this invoice (payments and credit used), refunds not yet taken off.
    pub paid: i64,
    pub refunded: i64,
    /// `net − (paid − refunded)`, never below 0.
    pub remaining: i64,
    pub note: Option<String>,
    pub void_reason: Option<String>,
    pub installments: Vec<InstallmentInfo>,
    pub credit_notes: Vec<CreditNoteInfo>,
    pub print_count: i64,
    pub created_by_name: Option<String>,
    pub version: i64,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InvoiceItemPick {
    pub item_id: String,
    /// None = the treatment's price. A different price needs `billing.override`.
    #[serde(default)]
    pub unit_price: Option<i64>,
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
    #[serde(default)]
    pub discount: i64,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub installments: Vec<NewInstallment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct VoidInvoiceParams {
    pub id: String,
    pub version: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreditNoteParams {
    pub invoice_id: String,
    pub amount: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetInstallmentsParams {
    pub invoice_id: String,
    pub installments: Vec<NewInstallment>,
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

// ───────────────────────────── payments & receipts ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReceiptLine {
    pub names: Translations,
    pub amount: i64,
}

/// What a receipt says, frozen when it is issued, so a reprint is the same paper.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReceiptSnapshot {
    pub patient_name: String,
    pub patient_number: String,
    pub invoice_number: Option<String>,
    pub lines: Vec<ReceiptLine>,
    pub invoice_net: i64,
    /// Received for the invoice after this payment.
    pub paid_after: i64,
    pub remaining_after: i64,
    /// The patient's credit after this payment.
    pub credit_after: i64,
    pub method: Option<Translations>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PaymentInfo {
    pub id: String,
    /// `RC-1405-000001` (payment), `CU-…` (credit used), `RV-…` (refund).
    pub number: String,
    pub kind: PaymentKind,
    pub patient_id: String,
    pub patient_name: String,
    pub invoice_id: Option<String>,
    pub invoice_number: Option<String>,
    pub method_id: Option<String>,
    pub method: Option<Translations>,
    pub amount: i64,
    pub paid_at: String,
    pub local_date: String,
    pub note: Option<String>,
    /// A refund's original payment.
    pub refund_of: Option<String>,
    pub returns_to_credit: bool,
    pub reason: Option<String>,
    /// How much of this payment has been given back.
    pub refunded: i64,
    pub print_count: i64,
    pub last_printed_at: Option<String>,
    pub receipt: ReceiptSnapshot,
    pub created_by_name: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AddPaymentParams {
    pub patient_id: String,
    /// None = an advance payment (credit).
    #[serde(default)]
    pub invoice_id: Option<String>,
    pub amount: i64,
    /// Required unless `use_credit`.
    #[serde(default)]
    pub method_id: Option<String>,
    /// Pay the invoice from the patient's credit instead of with money.
    #[serde(default)]
    pub use_credit: bool,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RefundParams {
    pub payment_id: String,
    pub amount: i64,
    pub reason: String,
    /// A cash payment's refund hands money back unless this is set (the amount becomes credit instead).
    #[serde(default)]
    pub to_credit: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct PaymentListParams {
    #[serde(default)]
    pub patient_id: Option<String>,
    #[serde(default)]
    pub invoice_id: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

// ───────────────────────────── balance, debtors, statement ─────────────────────────────

/// Total · Paid · Discount · Refunded · Remaining (6.4), plus the patient's credit.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PatientAccount {
    pub patient_id: String,
    pub invoice_count: i64,
    /// Sum of invoice lines.
    pub total: i64,
    /// Discounts and credit notes.
    pub discount: i64,
    pub paid: i64,
    pub refunded: i64,
    pub remaining: i64,
    /// Advance money not yet put on an invoice.
    pub credit: i64,
    /// Completed treatments not invoiced yet.
    pub unbilled_count: i64,
    pub unbilled_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DebtorInfo {
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub phone: Option<String>,
    pub remaining: i64,
    pub invoice_count: i64,
    pub oldest_invoice_at: Option<String>,
    pub last_payment_at: Option<String>,
    /// The earliest installment date still unpaid.
    pub next_due: Option<String>,
    pub overdue: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct DebtorListParams {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StatementLine {
    pub at: String,
    /// `invoice`, `payment`, `credit`, `refund`, `credit_note`.
    pub kind: String,
    pub number: String,
    pub description: String,
    /// What the patient owes more (invoice).
    pub debit: i64,
    /// What the patient has paid or was credited.
    pub credit: i64,
    /// Running balance owed after this line (negative = in credit).
    pub balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StatementInfo {
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub phone: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub generated_at: String,
    pub opening_balance: i64,
    pub lines: Vec<StatementLine>,
    pub account: PatientAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StatementParams {
    pub patient_id: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InstallmentDue {
    pub invoice_id: String,
    pub invoice_number: String,
    pub patient_id: String,
    pub patient_name: String,
    pub phone: Option<String>,
    pub no: i64,
    pub due_date: String,
    pub amount: i64,
    pub unpaid: i64,
    pub overdue: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct InstallmentsDueParams {
    /// Installments due on or before this date (default: a week from today).
    #[serde(default)]
    pub through: Option<String>,
}

// ───────────────────────────── commission, settings ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BillingSettings {
    pub commission_enabled: bool,
    pub commission_basis: CommissionBasis,
    /// Printed at the foot of every receipt.
    pub receipt_footer: String,
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
    pub date: String,
    pub invoice_number: String,
    pub patient_name: String,
    pub names: Translations,
    /// What the commission is taken from.
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

// ───────────────────────────── expenses & cash closing ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ExpenseCategoryInfo {
    pub id: String,
    pub name: Translations,
    pub is_system: bool,
    pub is_active: bool,
    pub sort_order: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveExpenseCategoryParams {
    #[serde(default)]
    pub id: Option<String>,
    pub name: Translations,
    #[serde(default = "yes")]
    pub is_active: bool,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ExpenseInfo {
    pub id: String,
    pub category_id: String,
    pub category: Translations,
    pub amount: i64,
    pub local_date: String,
    pub description: String,
    pub payee: Option<String>,
    /// Paid out of the cash drawer (counts at the daily closing).
    pub from_cash: bool,
    pub created_by_name: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AddExpenseParams {
    pub category_id: String,
    pub amount: i64,
    pub date: String,
    pub description: String,
    #[serde(default)]
    pub payee: Option<String>,
    #[serde(default = "yes")]
    pub from_cash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DeleteExpenseParams {
    pub id: String,
    pub version: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PeriodParams {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DateParams {
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MethodTotal {
    pub method: Translations,
    pub is_cash: bool,
    pub received: i64,
    pub refunded: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CashClosingInfo {
    pub id: String,
    pub date: String,
    pub expected: i64,
    pub counted: i64,
    /// `counted − expected`.
    pub difference: i64,
    pub reason: Option<String>,
    pub closed_at: String,
    pub closed_by_name: Option<String>,
}

/// One day at the cash desk (6.7).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CashDay {
    pub date: String,
    pub by_method: Vec<MethodTotal>,
    pub cash_in: i64,
    pub cash_refunds: i64,
    pub cash_expenses: i64,
    pub expected: i64,
    pub payment_count: i64,
    pub closing: Option<CashClosingInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CloseCashParams {
    pub date: String,
    pub counted: i64,
    /// Required when the count differs from the expected amount.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CategoryTotal {
    pub category: Translations,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct FinanceSummary {
    pub from: String,
    pub to: String,
    pub invoiced: i64,
    pub received: i64,
    pub refunded: i64,
    pub expenses: i64,
    /// `received − refunded − expenses`.
    pub profit: i64,
    pub by_method: Vec<MethodTotal>,
    pub by_category: Vec<CategoryTotal>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct ExpenseListParams {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}
