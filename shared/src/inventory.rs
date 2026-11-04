//! Phase 8 — inventory, suppliers and the dental lab. Quantities are hundredths of the item's unit
//! (2.5 boxes = 250); money is AFN × 100 (ADR-06); dates are clinic-local `YYYY-MM-DD`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Translations;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StockMovementKind {
    /// Received with a purchase.
    Purchase,
    /// Received without a purchase (opening stock, a gift).
    Receive,
    Consume,
    Adjust,
    /// Given back to the supplier.
    Return,
    TransferOut,
    TransferIn,
    VoidPurchase,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchState {
    Valid,
    Expiring,
    Expired,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StockFilter {
    Low,
    Expired,
    Expiring,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupplierKind {
    Supplier,
    Lab,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LabStatus {
    Planned,
    Sent,
    Received,
    Delivered,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LedgerKind {
    Purchase,
    Lab,
    Return,
    Payment,
}

fn yes() -> bool {
    true
}

// ───────────────────────────── categories, units, places ─────────────────────────────

/// A stock category, unit or place: a named, translated list entry the clinic can extend.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StockLookupInfo {
    pub id: String,
    pub name: Translations,
    pub is_system: bool,
    pub is_active: bool,
    pub sort_order: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveStockLookupParams {
    /// None = a new entry.
    #[serde(default)]
    pub id: Option<String>,
    pub name: Translations,
    #[serde(default = "yes")]
    pub is_active: bool,
    #[serde(default)]
    pub version: Option<i64>,
}

// ───────────────────────────── items and batches ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StockItemInfo {
    pub id: String,
    pub code: Option<String>,
    pub name: Translations,
    pub category_id: String,
    pub category: Translations,
    pub unit_id: String,
    pub unit: Translations,
    pub min_qty: i64,
    pub note: Option<String>,
    pub is_active: bool,
    /// On the shelf, expired included.
    pub qty: i64,
    /// On the shelf and still valid.
    pub usable_qty: i64,
    pub expired_qty: i64,
    pub expiring_qty: i64,
    /// Value of the stock at the cost it was bought for (AFN × 100).
    pub value: i64,
    pub nearest_expiry: Option<String>,
    /// Usable stock is at or below the minimum (and a minimum is set).
    pub low: bool,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveStockItemParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    pub name: Translations,
    pub category_id: String,
    pub unit_id: String,
    #[serde(default)]
    pub min_qty: i64,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct StockItemListParams {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub only: Option<StockFilter>,
    #[serde(default)]
    pub include_inactive: bool,
    /// Days ahead that count as "expiring soon" (default 60).
    #[serde(default)]
    pub expiring_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StockBatchInfo {
    pub id: String,
    pub item_id: String,
    pub item_name: Translations,
    pub unit: Translations,
    pub location_id: String,
    pub location: Translations,
    pub lot_no: Option<String>,
    pub expiry_date: Option<String>,
    pub qty_left: i64,
    pub unit_cost: i64,
    pub received_on: String,
    pub supplier_name: Option<String>,
    pub state: BatchState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct StockBatchListParams {
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub include_empty: bool,
    #[serde(default)]
    pub expiring_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StockMovementInfo {
    pub id: String,
    pub item_id: String,
    pub item_name: Translations,
    pub unit: Translations,
    pub kind: StockMovementKind,
    pub qty: i64,
    pub unit_cost: i64,
    pub credit: i64,
    pub location: Translations,
    pub lot_no: Option<String>,
    pub supplier_name: Option<String>,
    pub patient_id: Option<String>,
    pub treatment_item_id: Option<String>,
    pub reason: Option<String>,
    pub local_date: String,
    pub created_by_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct StockMovementListParams {
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub kind: Option<StockMovementKind>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct StockAlertParams {
    #[serde(default)]
    pub expiring_days: Option<i64>,
}

/// Everything that needs the clinic's attention (8.3, 8.4, 8.6).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StockAlerts {
    pub low: Vec<StockItemInfo>,
    pub expired: Vec<StockBatchInfo>,
    pub expiring: Vec<StockBatchInfo>,
    pub lab_delayed: Vec<LabCaseInfo>,
}

// ───────────────────────────── stock operations ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReceiveStockParams {
    pub item_id: String,
    pub qty: i64,
    #[serde(default)]
    pub unit_cost: i64,
    #[serde(default)]
    pub lot_no: Option<String>,
    #[serde(default)]
    pub expiry_date: Option<String>,
    pub location_id: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ConsumeStockParams {
    pub item_id: String,
    pub qty: i64,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    /// Take only from this place.
    #[serde(default)]
    pub location_id: Option<String>,
    #[serde(default)]
    pub patient_id: Option<String>,
    #[serde(default)]
    pub treatment_item_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AdjustStockParams {
    pub batch_id: String,
    /// Positive adds, negative removes (a count correction, a breakage, an expired lot thrown away).
    pub delta: i64,
    pub reason: String,
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReturnStockParams {
    pub batch_id: String,
    pub qty: i64,
    pub supplier_id: String,
    /// What the supplier credits the clinic for (reduces what the clinic owes).
    #[serde(default)]
    pub credit: i64,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TransferStockParams {
    pub batch_id: String,
    pub qty: i64,
    pub to_location_id: String,
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ConsumeForTreatmentParams {
    pub treatment_item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ServiceMaterialInfo {
    pub item_id: String,
    pub item_name: Translations,
    pub unit: Translations,
    pub qty: i64,
    /// On the shelf and valid now.
    pub usable_qty: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ServiceIdParams {
    pub service_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MaterialLine {
    pub item_id: String,
    pub qty: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveServiceMaterialsParams {
    pub service_id: String,
    pub materials: Vec<MaterialLine>,
}

// ───────────────────────────── suppliers and labs ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SupplierInfo {
    pub id: String,
    pub kind: SupplierKind,
    pub name: String,
    pub contact_person: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub note: Option<String>,
    pub is_active: bool,
    /// Bought from this supplier.
    pub purchases_total: i64,
    /// Lab work received from this lab.
    pub lab_total: i64,
    /// Credited for goods returned.
    pub returns_total: i64,
    pub paid_total: i64,
    /// What the clinic still owes (negative = the supplier owes the clinic).
    pub balance: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SaveSupplierParams {
    #[serde(default)]
    pub id: Option<String>,
    pub kind: SupplierKind,
    pub name: String,
    #[serde(default)]
    pub contact_person: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
    #[serde(default)]
    pub version: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct SupplierListParams {
    #[serde(default)]
    pub kind: Option<SupplierKind>,
    #[serde(default)]
    pub include_inactive: bool,
    /// Only those the clinic owes money to.
    #[serde(default)]
    pub owing_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SupplierIdParams {
    pub supplier_id: String,
}

/// One line of a supplier's history; `amount` is its effect on what the clinic owes.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LedgerEntry {
    pub kind: LedgerKind,
    pub local_date: String,
    pub number: Option<String>,
    pub description: String,
    pub amount: i64,
    pub balance: i64,
}

// ───────────────────────────── purchases and payments ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PurchaseLineInfo {
    pub id: String,
    pub item_id: String,
    pub item_name: Translations,
    pub unit: Translations,
    pub qty: i64,
    pub unit_cost: i64,
    pub line_total: i64,
    pub lot_no: Option<String>,
    pub expiry_date: Option<String>,
    pub location: Translations,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PurchaseInfo {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub supplier_name: String,
    pub local_date: String,
    pub total: i64,
    pub note: Option<String>,
    pub void: bool,
    pub void_reason: Option<String>,
    pub lines: Vec<PurchaseLineInfo>,
    pub created_by_name: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PurchaseLineParams {
    pub item_id: String,
    pub qty: i64,
    pub unit_cost: i64,
    #[serde(default)]
    pub lot_no: Option<String>,
    #[serde(default)]
    pub expiry_date: Option<String>,
    pub location_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreatePurchaseParams {
    pub supplier_id: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    pub lines: Vec<PurchaseLineParams>,
    /// Paid on the spot (recorded as a payment to the supplier).
    #[serde(default)]
    pub paid: i64,
    #[serde(default)]
    pub method_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct PurchaseListParams {
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct VoidWithReasonParams {
    pub id: String,
    pub version: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SupplierPaymentInfo {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub supplier_name: String,
    pub amount: i64,
    pub method_id: Option<String>,
    pub method: Option<Translations>,
    pub local_date: String,
    pub note: Option<String>,
    pub void: bool,
    pub void_reason: Option<String>,
    pub created_by_name: Option<String>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AddSupplierPaymentParams {
    pub supplier_id: String,
    pub amount: i64,
    pub method_id: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct SupplierPaymentListParams {
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

// ───────────────────────────── dental lab ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LabCaseInfo {
    pub id: String,
    pub number: String,
    pub patient_id: String,
    pub patient_name: String,
    pub patient_number: String,
    pub doctor_id: Option<String>,
    pub doctor_name: Option<String>,
    pub treatment_item_id: Option<String>,
    pub lab_id: String,
    pub lab_name: String,
    pub teeth: Vec<u8>,
    pub work_type: String,
    pub shade: Option<String>,
    pub status: LabStatus,
    pub sent_date: Option<String>,
    pub expected_date: Option<String>,
    pub received_date: Option<String>,
    pub delivered_date: Option<String>,
    pub cost: i64,
    pub note: Option<String>,
    pub cancel_reason: Option<String>,
    /// Sent, and past its expected date.
    pub delayed: bool,
    pub days_late: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateLabCaseParams {
    pub patient_id: String,
    pub lab_id: String,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub treatment_item_id: Option<String>,
    #[serde(default)]
    pub teeth: Vec<u8>,
    pub work_type: String,
    #[serde(default)]
    pub shade: Option<String>,
    #[serde(default)]
    pub expected_date: Option<String>,
    #[serde(default)]
    pub cost: i64,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateLabCaseParams {
    pub id: String,
    pub version: i64,
    pub lab_id: String,
    #[serde(default)]
    pub doctor_id: Option<String>,
    #[serde(default)]
    pub teeth: Vec<u8>,
    pub work_type: String,
    #[serde(default)]
    pub shade: Option<String>,
    #[serde(default)]
    pub expected_date: Option<String>,
    #[serde(default)]
    pub cost: i64,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SetLabStatusParams {
    pub id: String,
    pub version: i64,
    pub status: LabStatus,
    /// The day it happened (default today).
    #[serde(default)]
    pub date: Option<String>,
    /// The final cost, when the work comes back.
    #[serde(default)]
    pub cost: Option<i64>,
    /// Required to cancel.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct LabCaseListParams {
    #[serde(default)]
    pub status: Option<LabStatus>,
    #[serde(default)]
    pub lab_id: Option<String>,
    #[serde(default)]
    pub patient_id: Option<String>,
    /// Open (planned or sent) cases only.
    #[serde(default)]
    pub open_only: bool,
    #[serde(default)]
    pub delayed_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LabProfitRow {
    pub case_id: String,
    pub number: String,
    pub patient_name: String,
    pub lab_name: String,
    pub work_type: String,
    pub teeth: Vec<u8>,
    pub received_date: Option<String>,
    pub cost: i64,
    /// Invoiced for the treatment this case belongs to (None = not linked to a treatment).
    pub billed: Option<i64>,
}

/// Lab cost of the cases received in a period, with what their treatments were invoiced for (8.6).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LabReport {
    pub rows: Vec<LabProfitRow>,
    pub total_cost: i64,
    pub total_billed: i64,
}
