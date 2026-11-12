//! Phase 8 — inventory, suppliers and the dental lab (roadmap 8.1–8.6). Quantities are hundredths of
//! the item's unit; money is AFN × 100 (ADR-06). Stock lives in batches (lot + expiry + place) and every
//! change is one signed row of the movement ledger, which is never edited (ADR-11): a mistake is a new
//! adjustment, or a void with its reason. A lab is a supplier of kind `lab`; a Lab Case is the work sent
//! to it, and its cost counts towards what the clinic owes that lab.

use artaveo_shared::*;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;

use crate::audit::{self, Actor};
use crate::billing::{
    check_patient, date_of, money, next_number, opt_date, optional, parse_teeth, required, scalar, trio,
};
use crate::clock::{now_iso, today_iso};
use crate::db::expect_one_row;
use crate::error::{CoreError, Result};
use crate::formulary::clean_translations;
use crate::ids::{new_id, seed_id};
use crate::scheduling::{format_date, parse_date};

/// Largest quantity of one line: 1 000 000 units.
const MAX_QTY: i64 = 100_000_000;
const MAX_AMOUNT: i128 = 100_000_000_000;
const EXPIRING_DEFAULT: i64 = 60;
const MAX_LIST: usize = 1000;

const STOCK_ITEMS_JSON: &str = include_str!("../seeds/stock_items.json");

// ───────────────────────────── seed ─────────────────────────────

/// One starter stock item of `core/seeds/stock_items.json` (audit 9.2): names, a category and a unit
/// that already exist in migration 0016. No price and no minimum: the clinic sets those itself.
#[derive(Deserialize)]
struct StockItemSeed {
    code: String,
    category: String,
    unit: String,
    name: Translations,
}

fn stock_item_seeds() -> Result<Vec<StockItemSeed>> {
    serde_json::from_str(STOCK_ITEMS_JSON)
        .map_err(|e| CoreError::validation(format!("stock_items.json: {e}")))
}

/// Writes the starter stock items once. Afterwards every row is the clinic's: a later start never
/// overwrites an edited name or minimum, never brings back an item the clinic switched off or deleted
/// (the row stays, so its deterministic id is still taken).
pub fn sync_seeds(conn: &Connection) -> Result<()> {
    let now = now_iso();
    for item in stock_item_seeds()? {
        conn.execute(
            "INSERT INTO stock_item(id, code, name_fa, name_ps, name_en, category_id, unit_id, min_qty, is_active,
                created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 1, ?8, ?8)
             ON CONFLICT(id) DO NOTHING",
            params![
                seed_id("stock_item", &item.code),
                item.code,
                item.name.fa,
                item.name.ps,
                item.name.en,
                item.category,
                item.unit,
                now
            ],
        )?;
    }
    Ok(())
}

// ───────────────────────────── helpers ─────────────────────────────

fn qty_of(field: &str, v: i64) -> Result<i64> {
    if !(1..=MAX_QTY).contains(&v) {
        return Err(CoreError::invalid(
            field,
            ValidationRule::StockQuantity,
            format!("{field} must be more than 0 and at most 1 000 000"),
        ));
    }
    Ok(v)
}

/// Quantity × cost per unit, both with their hundredths: the line total in AFN × 100.
fn line_total(qty: i64, unit_cost: i64) -> Result<i64> {
    let total = (i128::from(qty) * i128::from(unit_cost) + 50) / 100;
    if total > MAX_AMOUNT {
        return Err(CoreError::invalid(
            "unit_cost",
            ValidationRule::BillingAmount,
            "this total is too large",
        ));
    }
    Ok(total as i64)
}

fn plus_days(date: &str, days: i64) -> Result<String> {
    Ok(format_date(parse_date("date", date)? + time::Duration::days(days)))
}

/// The day an operation happened: today unless given, never in the future.
fn day_of(p: &Option<String>) -> Result<String> {
    let today = today_iso();
    match opt_date("date", p)? {
        None => Ok(today),
        Some(d) if d > today => {
            Err(CoreError::invalid("date", ValidationRule::DateFormat, "this cannot be dated in the future"))
        }
        Some(d) => Ok(d),
    }
}

/// `(today, last day that still counts as "expiring soon")`.
fn expiry_window(days: Option<i64>) -> Result<(String, String)> {
    let today = today_iso();
    let soon = plus_days(&today, days.unwrap_or(EXPIRING_DEFAULT).clamp(1, 730))?;
    Ok((today, soon))
}

fn batch_state(expiry: &Option<String>, today: &str, soon: &str) -> BatchState {
    match expiry.as_deref() {
        Some(e) if e < today => BatchState::Expired,
        Some(e) if e <= soon => BatchState::Expiring,
        _ => BatchState::Valid,
    }
}

fn not_found(entity: &str, id: &str, field: &str, rule: ValidationRule) -> CoreError {
    CoreError::api(ErrorCode::NotFound, format!("{entity} {id}")).on_field(field, rule)
}

fn kind_str(k: StockMovementKind) -> &'static str {
    match k {
        StockMovementKind::Purchase => "purchase",
        StockMovementKind::Receive => "receive",
        StockMovementKind::Consume => "consume",
        StockMovementKind::Adjust => "adjust",
        StockMovementKind::Return => "return",
        StockMovementKind::TransferOut => "transfer_out",
        StockMovementKind::TransferIn => "transfer_in",
        StockMovementKind::VoidPurchase => "void_purchase",
    }
}

fn kind_of(s: &str) -> StockMovementKind {
    match s {
        "purchase" => StockMovementKind::Purchase,
        "receive" => StockMovementKind::Receive,
        "consume" => StockMovementKind::Consume,
        "return" => StockMovementKind::Return,
        "transfer_out" => StockMovementKind::TransferOut,
        "transfer_in" => StockMovementKind::TransferIn,
        "void_purchase" => StockMovementKind::VoidPurchase,
        _ => StockMovementKind::Adjust,
    }
}

fn lab_status_str(s: LabStatus) -> &'static str {
    match s {
        LabStatus::Planned => "planned",
        LabStatus::Sent => "sent",
        LabStatus::Received => "received",
        LabStatus::Delivered => "delivered",
        LabStatus::Cancelled => "cancelled",
    }
}

fn lab_status_of(s: &str) -> LabStatus {
    match s {
        "sent" => LabStatus::Sent,
        "received" => LabStatus::Received,
        "delivered" => LabStatus::Delivered,
        "cancelled" => LabStatus::Cancelled,
        _ => LabStatus::Planned,
    }
}

// ───────────────────────────── categories, units, places ─────────────────────────────

#[derive(Clone, Copy)]
pub enum Lookup {
    Category,
    Unit,
    Location,
}

impl Lookup {
    fn table(self) -> &'static str {
        match self {
            Lookup::Category => "stock_category",
            Lookup::Unit => "stock_unit",
            Lookup::Location => "stock_location",
        }
    }
}

fn map_lookup(r: &rusqlite::Row<'_>) -> rusqlite::Result<StockLookupInfo> {
    Ok(StockLookupInfo {
        id: r.get(0)?,
        name: trio(r.get(1)?, r.get(2)?, r.get(3)?),
        sort_order: r.get(4)?,
        is_system: r.get::<_, i64>(5)? == 1,
        is_active: r.get::<_, i64>(6)? == 1,
        version: r.get(7)?,
    })
}

fn lookup_select(which: Lookup) -> String {
    format!(
        "SELECT id, name_fa, name_ps, name_en, sort_order, is_system, is_active, version
         FROM {} WHERE deleted_at IS NULL",
        which.table()
    )
}

pub fn list_lookups(
    conn: &Connection,
    which: Lookup,
    include_inactive: bool,
) -> Result<Vec<StockLookupInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{} {} ORDER BY sort_order, name_en",
        lookup_select(which),
        if include_inactive { "" } else { "AND is_active = 1" }
    ))?;
    let rows = stmt.query_map([], map_lookup)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn get_lookup(conn: &Connection, which: Lookup, id: &str, field: &str) -> Result<StockLookupInfo> {
    conn.query_row(&format!("{} AND id = ?1", lookup_select(which)), [id], map_lookup)
        .optional()?
        .ok_or_else(|| not_found(which.table(), id, field, ValidationRule::StockLookupNotFound))
}

pub fn save_lookup(
    conn: &Connection,
    actor: &Actor,
    which: Lookup,
    p: &SaveStockLookupParams,
) -> Result<StockLookupInfo> {
    let name = clean_translations("name", &p.name, 60)?;
    let now = now_iso();
    let table = which.table();
    match p.id.as_deref().filter(|s| !s.is_empty()) {
        None => {
            let id = new_id();
            let order =
                scalar(conn, &format!("SELECT MAX(sort_order) FROM {table} WHERE sort_order < 90"), &[])?
                    + 10;
            conn.execute(
                &format!(
                    "INSERT INTO {table}(id, name_fa, name_ps, name_en, sort_order, is_active, created_at, created_by, updated_at, updated_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?7, ?8)"
                ),
                params![id, name.fa, name.ps, name.en, order, p.is_active, now, actor.user_id],
            )?;
            let after = get_lookup(conn, which, &id, "id")?;
            audit::record(
                conn,
                actor,
                &format!("{table}.create"),
                Some(table),
                Some(&id),
                None,
                Some(&json!(after.name)),
            )?;
            Ok(after)
        }
        Some(id) => {
            let before = get_lookup(conn, which, id, "id")?;
            let changed = conn.execute(
                &format!(
                    "UPDATE {table} SET name_fa = ?1, name_ps = ?2, name_en = ?3, is_active = ?4, updated_at = ?5,
                        updated_by = ?6, version = version + 1 WHERE id = ?7 AND version = ?8 AND deleted_at IS NULL"
                ),
                params![
                    name.fa,
                    name.ps,
                    name.en,
                    p.is_active,
                    now,
                    actor.user_id,
                    id,
                    p.version.unwrap_or(before.version)
                ],
            )?;
            expect_one_row(changed, table)?;
            let after = get_lookup(conn, which, id, "id")?;
            audit::record(
                conn,
                actor,
                &format!("{table}.update"),
                Some(table),
                Some(id),
                Some(&json!(before)),
                Some(&json!(after)),
            )?;
            Ok(after)
        }
    }
}

// ───────────────────────────── items ─────────────────────────────

/// `?1` = today, `?2` = last day of "expiring soon".
const ITEM_SELECT: &str = "SELECT i.id, i.code, i.name_fa, i.name_ps, i.name_en, i.category_id, c.name_fa, c.name_ps, c.name_en,
    i.unit_id, u.name_fa, u.name_ps, u.name_en, i.min_qty, i.note, i.is_active, i.version,
    COALESCE((SELECT SUM(b.qty_left) FROM stock_batch b WHERE b.item_id = i.id), 0),
    COALESCE((SELECT SUM(b.qty_left) FROM stock_batch b WHERE b.item_id = i.id AND b.expiry_date < ?1), 0),
    COALESCE((SELECT SUM(b.qty_left) FROM stock_batch b WHERE b.item_id = i.id AND b.expiry_date >= ?1 AND b.expiry_date <= ?2), 0),
    COALESCE((SELECT SUM(b.qty_left * b.unit_cost) / 100 FROM stock_batch b WHERE b.item_id = i.id), 0),
    (SELECT MIN(b.expiry_date) FROM stock_batch b WHERE b.item_id = i.id AND b.qty_left > 0 AND b.expiry_date IS NOT NULL)
    FROM stock_item i JOIN stock_category c ON c.id = i.category_id JOIN stock_unit u ON u.id = i.unit_id
    WHERE i.deleted_at IS NULL";

fn map_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<StockItemInfo> {
    let min_qty: i64 = r.get(13)?;
    let qty: i64 = r.get(17)?;
    let expired_qty: i64 = r.get(18)?;
    let usable_qty = qty - expired_qty;
    Ok(StockItemInfo {
        id: r.get(0)?,
        code: r.get(1)?,
        name: trio(r.get(2)?, r.get(3)?, r.get(4)?),
        category_id: r.get(5)?,
        category: trio(r.get(6)?, r.get(7)?, r.get(8)?),
        unit_id: r.get(9)?,
        unit: trio(r.get(10)?, r.get(11)?, r.get(12)?),
        min_qty,
        note: r.get(14)?,
        is_active: r.get::<_, i64>(15)? == 1,
        version: r.get(16)?,
        qty,
        usable_qty,
        expired_qty,
        expiring_qty: r.get(19)?,
        value: r.get(20)?,
        nearest_expiry: r.get(21)?,
        low: min_qty > 0 && usable_qty <= min_qty,
    })
}

pub fn get_item(conn: &Connection, id: &str) -> Result<StockItemInfo> {
    let (today, soon) = expiry_window(None)?;
    conn.query_row(&format!("{ITEM_SELECT} AND i.id = ?3"), params![today, soon, id], map_item)
        .optional()?
        .ok_or_else(|| not_found("stock item", id, "item_id", ValidationRule::StockItemNotFound))
}

pub fn list_items(conn: &Connection, p: &StockItemListParams) -> Result<Vec<StockItemInfo>> {
    let (today, soon) = expiry_window(p.expiring_days)?;
    let mut stmt = conn.prepare(&format!(
        "{ITEM_SELECT} {} ORDER BY c.sort_order, i.name_en, i.name_fa",
        if p.include_inactive { "" } else { "AND i.is_active = 1" }
    ))?;
    let all = stmt.query_map(params![today, soon], map_item)?.collect::<rusqlite::Result<Vec<_>>>()?;
    let needle = p.search.as_deref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    let rows = all
        .into_iter()
        .filter(|i| p.category_id.as_deref().is_none_or(|c| c == i.category_id))
        .filter(|i| match p.only {
            None => true,
            Some(StockFilter::Low) => i.low,
            Some(StockFilter::Expired) => i.expired_qty > 0,
            Some(StockFilter::Expiring) => i.expiring_qty > 0,
        })
        .filter(|i| match &needle {
            None => true,
            Some(n) => {
                i.name.fa.to_lowercase().contains(n)
                    || i.name.ps.to_lowercase().contains(n)
                    || i.name.en.to_lowercase().contains(n)
                    || i.code.as_deref().is_some_and(|c| c.to_lowercase().contains(n))
            }
        })
        .take(MAX_LIST)
        .collect();
    Ok(rows)
}

pub fn save_item(conn: &Connection, actor: &Actor, p: &SaveStockItemParams) -> Result<StockItemInfo> {
    let name = clean_translations("name", &p.name, 100)?;
    let code = optional("code", &p.code, 40)?;
    let note = optional("note", &p.note, 300)?;
    get_lookup(conn, Lookup::Category, &p.category_id, "category_id")?;
    get_lookup(conn, Lookup::Unit, &p.unit_id, "unit_id")?;
    if !(0..=MAX_QTY).contains(&p.min_qty) {
        return Err(CoreError::invalid(
            "min_qty",
            ValidationRule::StockQuantity,
            "the minimum stock is not a valid quantity",
        ));
    }
    let now = now_iso();
    match p.id.as_deref().filter(|s| !s.is_empty()) {
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO stock_item(id, code, name_fa, name_ps, name_en, category_id, unit_id, min_qty, note, is_active,
                    created_at, created_by, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?11, ?12)",
                params![
                    id,
                    code,
                    name.fa,
                    name.ps,
                    name.en,
                    p.category_id,
                    p.unit_id,
                    p.min_qty,
                    note,
                    p.is_active,
                    now,
                    actor.user_id
                ],
            )?;
            audit::record(
                conn,
                actor,
                "stock_item.create",
                Some("stock_item"),
                Some(&id),
                None,
                Some(&json!({"name": name, "min_qty": p.min_qty})),
            )?;
            get_item(conn, &id)
        }
        Some(id) => {
            let before = get_item(conn, id)?;
            let changed = conn.execute(
                "UPDATE stock_item SET code = ?1, name_fa = ?2, name_ps = ?3, name_en = ?4, category_id = ?5, unit_id = ?6,
                    min_qty = ?7, note = ?8, is_active = ?9, updated_at = ?10, updated_by = ?11, version = version + 1
                 WHERE id = ?12 AND version = ?13 AND deleted_at IS NULL",
                params![
                    code,
                    name.fa,
                    name.ps,
                    name.en,
                    p.category_id,
                    p.unit_id,
                    p.min_qty,
                    note,
                    p.is_active,
                    now,
                    actor.user_id,
                    id,
                    p.version.unwrap_or(before.version)
                ],
            )?;
            expect_one_row(changed, "stock item")?;
            let after = get_item(conn, id)?;
            audit::record(
                conn,
                actor,
                "stock_item.update",
                Some("stock_item"),
                Some(id),
                Some(&json!({"name": before.name, "min_qty": before.min_qty, "is_active": before.is_active})),
                Some(&json!({"name": after.name, "min_qty": after.min_qty, "is_active": after.is_active})),
            )?;
            Ok(after)
        }
    }
}

/// Deletes a stock item. Seeded and user-made items are treated the same. An item that was ever received,
/// moved or bought has a history that must stay readable, so it is only archived (switched off); an item
/// without any history is removed (soft delete). `archived` in the result tells which one happened.
pub fn delete_item(conn: &Connection, actor: &Actor, id: &str) -> Result<DeleteStockItemResult> {
    let before = get_item(conn, id)?;
    let history: i64 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM stock_batch WHERE item_id = ?1)
              + (SELECT COUNT(*) FROM stock_movement WHERE item_id = ?1)
              + (SELECT COUNT(*) FROM purchase_line WHERE item_id = ?1)",
        [id],
        |r| r.get(0),
    )?;
    let now = now_iso();
    let archived = history > 0;
    let changed = if archived {
        conn.execute(
            "UPDATE stock_item SET is_active = 0, updated_at = ?1, updated_by = ?2, version = version + 1
             WHERE id = ?3 AND deleted_at IS NULL",
            params![now, actor.user_id, id],
        )?
    } else {
        // The materials a service lists are plain settings, not history: they go with the item.
        conn.execute("DELETE FROM service_material WHERE item_id = ?1", [id])?;
        conn.execute(
            "UPDATE stock_item SET deleted_at = ?1, updated_at = ?1, updated_by = ?2, version = version + 1
             WHERE id = ?3 AND deleted_at IS NULL",
            params![now, actor.user_id, id],
        )?
    };
    expect_one_row(changed, "stock item")?;
    audit::record(
        conn,
        actor,
        if archived { "stock_item.archive" } else { "stock_item.delete" },
        Some("stock_item"),
        Some(id),
        Some(&json!({"name": before.name, "is_active": before.is_active})),
        None,
    )?;
    Ok(DeleteStockItemResult { archived })
}

// ───────────────────────────── batches and the ledger ─────────────────────────────

const BATCH_SELECT: &str = "SELECT b.id, b.item_id, i.name_fa, i.name_ps, i.name_en, u.name_fa, u.name_ps, u.name_en,
    b.location_id, l.name_fa, l.name_ps, l.name_en, b.lot_no, b.expiry_date, b.qty_left, b.unit_cost, b.received_on, s.name
    FROM stock_batch b JOIN stock_item i ON i.id = b.item_id JOIN stock_unit u ON u.id = i.unit_id
    JOIN stock_location l ON l.id = b.location_id LEFT JOIN supplier s ON s.id = b.supplier_id
    WHERE 1 = 1";

fn map_batch(r: &rusqlite::Row<'_>, today: &str, soon: &str) -> rusqlite::Result<StockBatchInfo> {
    let expiry_date: Option<String> = r.get(13)?;
    Ok(StockBatchInfo {
        id: r.get(0)?,
        item_id: r.get(1)?,
        item_name: trio(r.get(2)?, r.get(3)?, r.get(4)?),
        unit: trio(r.get(5)?, r.get(6)?, r.get(7)?),
        location_id: r.get(8)?,
        location: trio(r.get(9)?, r.get(10)?, r.get(11)?),
        lot_no: r.get(12)?,
        state: batch_state(&expiry_date, today, soon),
        expiry_date,
        qty_left: r.get(14)?,
        unit_cost: r.get(15)?,
        received_on: r.get(16)?,
        supplier_name: r.get(17)?,
    })
}

pub fn list_batches(conn: &Connection, p: &StockBatchListParams) -> Result<Vec<StockBatchInfo>> {
    let (today, soon) = expiry_window(p.expiring_days)?;
    let mut sql = String::from(BATCH_SELECT);
    let mut args: Vec<Value> = Vec::new();
    if !p.include_empty {
        sql.push_str(" AND b.qty_left > 0");
    }
    if let Some(item) = p.item_id.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND b.item_id = ?");
        args.push(Value::Text(item.to_string()));
    }
    sql.push_str(&format!(
        " ORDER BY b.expiry_date IS NULL, b.expiry_date, b.received_on, b.created_at LIMIT {MAX_LIST}"
    ));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_from_iter(args.iter()), |r| map_batch(r, &today, &soon))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

struct BatchRow {
    item_id: String,
    location_id: String,
    lot_no: Option<String>,
    expiry_date: Option<String>,
    qty_left: i64,
    unit_cost: i64,
    received_on: String,
    supplier_id: Option<String>,
    purchase_id: Option<String>,
}

fn batch_row(conn: &Connection, id: &str) -> Result<BatchRow> {
    conn.query_row(
        "SELECT item_id, location_id, lot_no, expiry_date, qty_left, unit_cost, received_on, supplier_id, purchase_id
         FROM stock_batch WHERE id = ?1",
        [id],
        |r| {
            Ok(BatchRow {
                item_id: r.get(0)?,
                location_id: r.get(1)?,
                lot_no: r.get(2)?,
                expiry_date: r.get(3)?,
                qty_left: r.get(4)?,
                unit_cost: r.get(5)?,
                received_on: r.get(6)?,
                supplier_id: r.get(7)?,
                purchase_id: r.get(8)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| not_found("stock batch", id, "batch_id", ValidationRule::StockBatchNotFound))
}

struct NewBatch<'a> {
    item_id: &'a str,
    location_id: &'a str,
    lot_no: Option<&'a str>,
    expiry_date: Option<&'a str>,
    qty: i64,
    unit_cost: i64,
    received_on: &'a str,
    supplier_id: Option<&'a str>,
    purchase_id: Option<&'a str>,
}

fn insert_batch(conn: &Connection, actor: &Actor, b: &NewBatch<'_>) -> Result<String> {
    let id = new_id();
    conn.execute(
        "INSERT INTO stock_batch(id, item_id, location_id, lot_no, expiry_date, qty_left, unit_cost, received_on,
            supplier_id, purchase_id, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            b.item_id,
            b.location_id,
            b.lot_no,
            b.expiry_date,
            b.qty,
            b.unit_cost,
            b.received_on,
            b.supplier_id,
            b.purchase_id,
            now_iso(),
            actor.user_id
        ],
    )?;
    Ok(id)
}

/// Moves a batch's quantity; it can never go below zero.
fn bump_batch(conn: &Connection, batch_id: &str, delta: i64) -> Result<()> {
    let changed = conn.execute(
        "UPDATE stock_batch SET qty_left = qty_left + ?1 WHERE id = ?2 AND qty_left + ?1 >= 0",
        params![delta, batch_id],
    )?;
    if changed != 1 {
        return Err(CoreError::invalid(
            "qty",
            ValidationRule::StockInsufficient,
            "there is not enough of this stock",
        ));
    }
    Ok(())
}

struct Move<'a> {
    item_id: &'a str,
    batch_id: &'a str,
    location_id: &'a str,
    kind: StockMovementKind,
    qty: i64,
    unit_cost: i64,
    credit: i64,
    supplier_id: Option<&'a str>,
    purchase_id: Option<&'a str>,
    patient_id: Option<&'a str>,
    treatment_item_id: Option<&'a str>,
    reason: Option<&'a str>,
    date: &'a str,
}

impl<'a> Move<'a> {
    fn new(
        item_id: &'a str,
        batch_id: &'a str,
        location_id: &'a str,
        kind: StockMovementKind,
        qty: i64,
        date: &'a str,
    ) -> Self {
        Move {
            item_id,
            batch_id,
            location_id,
            kind,
            qty,
            unit_cost: 0,
            credit: 0,
            supplier_id: None,
            purchase_id: None,
            patient_id: None,
            treatment_item_id: None,
            reason: None,
            date,
        }
    }
}

fn record_move(conn: &Connection, actor: &Actor, m: &Move<'_>) -> Result<()> {
    conn.execute(
        "INSERT INTO stock_movement(id, item_id, batch_id, location_id, kind, qty, unit_cost, credit, supplier_id,
            purchase_id, patient_id, treatment_item_id, reason, local_date, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            new_id(),
            m.item_id,
            m.batch_id,
            m.location_id,
            kind_str(m.kind),
            m.qty,
            m.unit_cost,
            m.credit,
            m.supplier_id,
            m.purchase_id,
            m.patient_id,
            m.treatment_item_id,
            m.reason,
            m.date,
            now_iso(),
            actor.user_id
        ],
    )?;
    Ok(())
}

const MOVE_SELECT: &str = "SELECT m.id, m.item_id, i.name_fa, i.name_ps, i.name_en, u.name_fa, u.name_ps, u.name_en, m.kind, m.qty,
    m.unit_cost, m.credit, l.name_fa, l.name_ps, l.name_en, b.lot_no, s.name, m.patient_id, m.treatment_item_id, m.reason,
    m.local_date, au.display_name
    FROM stock_movement m JOIN stock_item i ON i.id = m.item_id JOIN stock_unit u ON u.id = i.unit_id
    JOIN stock_location l ON l.id = m.location_id JOIN stock_batch b ON b.id = m.batch_id
    LEFT JOIN supplier s ON s.id = m.supplier_id LEFT JOIN app_user au ON au.id = m.created_by
    WHERE 1 = 1";

fn map_move(r: &rusqlite::Row<'_>) -> rusqlite::Result<StockMovementInfo> {
    let kind: String = r.get(8)?;
    Ok(StockMovementInfo {
        id: r.get(0)?,
        item_id: r.get(1)?,
        item_name: trio(r.get(2)?, r.get(3)?, r.get(4)?),
        unit: trio(r.get(5)?, r.get(6)?, r.get(7)?),
        kind: kind_of(&kind),
        qty: r.get(9)?,
        unit_cost: r.get(10)?,
        credit: r.get(11)?,
        location: trio(r.get(12)?, r.get(13)?, r.get(14)?),
        lot_no: r.get(15)?,
        supplier_name: r.get(16)?,
        patient_id: r.get(17)?,
        treatment_item_id: r.get(18)?,
        reason: r.get(19)?,
        local_date: r.get(20)?,
        created_by_name: r.get(21)?,
    })
}

pub fn list_movements(conn: &Connection, p: &StockMovementListParams) -> Result<Vec<StockMovementInfo>> {
    let to = opt_date("to", &p.to)?.unwrap_or_else(today_iso);
    let from = match opt_date("from", &p.from)? {
        Some(f) => f,
        None => plus_days(&to, -90)?,
    };
    let mut sql = String::from(MOVE_SELECT);
    sql.push_str(" AND m.local_date BETWEEN ? AND ?");
    let mut args = vec![Value::Text(from), Value::Text(to)];
    if let Some(item) = p.item_id.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND m.item_id = ?");
        args.push(Value::Text(item.to_string()));
    }
    if let Some(kind) = p.kind {
        sql.push_str(" AND m.kind = ?");
        args.push(Value::Text(kind_str(kind).to_string()));
    }
    sql.push_str(&format!(" ORDER BY m.local_date DESC, m.created_at DESC, m.id DESC LIMIT {MAX_LIST}"));
    let mut stmt = conn.prepare(&sql)?;
    let rows =
        stmt.query_map(params_from_iter(args.iter()), map_move)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn movements_of_treatment(conn: &Connection, treatment_item_id: &str) -> Result<Vec<StockMovementInfo>> {
    let mut stmt =
        conn.prepare(&format!("{MOVE_SELECT} AND m.treatment_item_id = ?1 ORDER BY m.created_at, m.id"))?;
    let rows = stmt.query_map([treatment_item_id], map_move)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn alerts(conn: &Connection, p: &StockAlertParams) -> Result<StockAlerts> {
    let low = list_items(
        conn,
        &StockItemListParams {
            only: Some(StockFilter::Low),
            expiring_days: p.expiring_days,
            ..Default::default()
        },
    )?;
    let batches =
        list_batches(conn, &StockBatchListParams { expiring_days: p.expiring_days, ..Default::default() })?;
    let (expired, rest): (Vec<_>, Vec<_>) = batches.into_iter().partition(|b| b.state == BatchState::Expired);
    let expiring = rest.into_iter().filter(|b| b.state == BatchState::Expiring).collect();
    let lab_delayed = list_lab_cases(conn, &LabCaseListParams { delayed_only: true, ..Default::default() })?;
    Ok(StockAlerts { low, expired, expiring, lab_delayed })
}

// ───────────────────────────── stock operations ─────────────────────────────

fn check_supplier(conn: &Connection, id: &str, field: &str) -> Result<SupplierInfo> {
    get_supplier(conn, id).map_err(|_| not_found("supplier", id, field, ValidationRule::SupplierNotFound))
}

pub fn receive(conn: &Connection, actor: &Actor, p: &ReceiveStockParams) -> Result<StockItemInfo> {
    let item = get_item(conn, &p.item_id)?;
    let qty = qty_of("qty", p.qty)?;
    let unit_cost = money("unit_cost", p.unit_cost, true)?;
    let location = get_lookup(conn, Lookup::Location, &p.location_id, "location_id")?;
    if let Some(s) = p.supplier_id.as_deref().filter(|s| !s.is_empty()) {
        check_supplier(conn, s, "supplier_id")?;
    }
    let supplier = p.supplier_id.as_deref().filter(|s| !s.is_empty());
    let lot = optional("lot_no", &p.lot_no, 60)?;
    let expiry = opt_date("expiry_date", &p.expiry_date)?;
    let reason = optional("reason", &p.reason, 300)?;
    let date = day_of(&p.date)?;
    let batch = insert_batch(
        conn,
        actor,
        &NewBatch {
            item_id: &item.id,
            location_id: &location.id,
            lot_no: lot.as_deref(),
            expiry_date: expiry.as_deref(),
            qty,
            unit_cost,
            received_on: &date,
            supplier_id: supplier,
            purchase_id: None,
        },
    )?;
    let mut mv = Move::new(&item.id, &batch, &location.id, StockMovementKind::Receive, qty, &date);
    mv.unit_cost = unit_cost;
    mv.supplier_id = supplier;
    mv.reason = reason.as_deref();
    record_move(conn, actor, &mv)?;
    audit::record(
        conn,
        actor,
        "stock.receive",
        Some("stock_item"),
        Some(&item.id),
        None,
        Some(&json!({"qty": qty, "unit_cost": unit_cost, "lot_no": lot, "expiry_date": expiry})),
    )?;
    get_item(conn, &item.id)
}

struct Consumption<'a> {
    item_id: &'a str,
    qty: i64,
    date: &'a str,
    reason: Option<&'a str>,
    location_id: Option<&'a str>,
    patient_id: Option<&'a str>,
    treatment_item_id: Option<&'a str>,
}

/// Takes `qty` from the valid batches that expire first (first-expiry-first-out); an expired batch is
/// never used.
fn consume_fefo(conn: &Connection, actor: &Actor, c: &Consumption<'_>) -> Result<()> {
    let today = today_iso();
    let mut stmt = conn.prepare(
        "SELECT id, location_id, qty_left, unit_cost FROM stock_batch
         WHERE item_id = ?1 AND qty_left > 0 AND (expiry_date IS NULL OR expiry_date >= ?2)
           AND (?3 IS NULL OR location_id = ?3)
         ORDER BY expiry_date IS NULL, expiry_date, received_on, created_at",
    )?;
    let batches = stmt
        .query_map(params![c.item_id, today, c.location_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    let mut remaining = c.qty;
    for (batch_id, location_id, left, unit_cost) in batches {
        if remaining == 0 {
            break;
        }
        let take = remaining.min(left);
        bump_batch(conn, &batch_id, -take)?;
        let mut mv = Move::new(c.item_id, &batch_id, &location_id, StockMovementKind::Consume, -take, c.date);
        mv.unit_cost = unit_cost;
        mv.patient_id = c.patient_id;
        mv.treatment_item_id = c.treatment_item_id;
        mv.reason = c.reason;
        record_move(conn, actor, &mv)?;
        remaining -= take;
    }
    if remaining > 0 {
        return Err(CoreError::invalid(
            "qty",
            ValidationRule::StockInsufficient,
            "there is not enough valid stock of this item",
        ));
    }
    Ok(())
}

pub fn consume(conn: &Connection, actor: &Actor, p: &ConsumeStockParams) -> Result<StockItemInfo> {
    let item = get_item(conn, &p.item_id)?;
    let qty = qty_of("qty", p.qty)?;
    let date = day_of(&p.date)?;
    let reason = optional("reason", &p.reason, 300)?;
    let location = p.location_id.as_deref().filter(|s| !s.is_empty());
    if let Some(l) = location {
        get_lookup(conn, Lookup::Location, l, "location_id")?;
    }
    let patient = p.patient_id.as_deref().filter(|s| !s.is_empty());
    if let Some(pt) = patient {
        check_patient(conn, pt)?;
    }
    consume_fefo(
        conn,
        actor,
        &Consumption {
            item_id: &item.id,
            qty,
            date: &date,
            reason: reason.as_deref(),
            location_id: location,
            patient_id: patient,
            treatment_item_id: p.treatment_item_id.as_deref().filter(|s| !s.is_empty()),
        },
    )?;
    audit::record(
        conn,
        actor,
        "stock.consume",
        Some("stock_item"),
        Some(&item.id),
        None,
        Some(&json!({"qty": qty, "reason": reason})),
    )?;
    get_item(conn, &item.id)
}

pub fn consume_for_treatment(
    conn: &Connection,
    actor: &Actor,
    p: &ConsumeForTreatmentParams,
) -> Result<Vec<StockMovementInfo>> {
    let row: Option<(String, String, i64, String)> = conn
        .query_row(
            "SELECT patient_id, service_id, units, status FROM treatment_item WHERE id = ?1 AND deleted_at IS NULL",
            [&p.treatment_item_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((patient_id, service_id, units, status)) = row else {
        return Err(CoreError::api(ErrorCode::NotFound, format!("treatment {}", p.treatment_item_id))
            .on_field("treatment_item_id", ValidationRule::ItemNotBillable));
    };
    if status == "cancelled" {
        return Err(CoreError::invalid(
            "treatment_item_id",
            ValidationRule::ItemNotBillable,
            "this treatment was cancelled",
        ));
    }
    let materials = list_materials(conn, &service_id)?;
    if materials.is_empty() {
        return Err(CoreError::invalid(
            "treatment_item_id",
            ValidationRule::MaterialsNone,
            "this service has no materials set",
        ));
    }
    if scalar(
        conn,
        "SELECT COUNT(*) FROM stock_movement WHERE treatment_item_id = ?1 AND kind = 'consume'",
        &[&p.treatment_item_id],
    )? > 0
    {
        return Err(CoreError::invalid(
            "treatment_item_id",
            ValidationRule::MaterialsAlreadyConsumed,
            "the materials of this treatment were already taken from stock",
        ));
    }
    let date = today_iso();
    for m in &materials {
        let qty = (i128::from(m.qty) * i128::from(units)).min(i128::from(MAX_QTY)) as i64;
        consume_fefo(
            conn,
            actor,
            &Consumption {
                item_id: &m.item_id,
                qty,
                date: &date,
                reason: None,
                location_id: None,
                patient_id: Some(&patient_id),
                treatment_item_id: Some(&p.treatment_item_id),
            },
        )?;
    }
    audit::record(
        conn,
        actor,
        "stock.consume_treatment",
        Some("treatment_item"),
        Some(&p.treatment_item_id),
        None,
        Some(&json!({"materials": materials.len(), "units": units})),
    )?;
    movements_of_treatment(conn, &p.treatment_item_id)
}

pub fn adjust(conn: &Connection, actor: &Actor, p: &AdjustStockParams) -> Result<StockItemInfo> {
    let b = batch_row(conn, &p.batch_id)?;
    if p.delta == 0 || p.delta.abs() > MAX_QTY {
        return Err(CoreError::invalid(
            "delta",
            ValidationRule::StockQuantity,
            "the change must not be zero",
        ));
    }
    let reason = required("reason", &p.reason, 300)?;
    let date = day_of(&p.date)?;
    bump_batch(conn, &p.batch_id, p.delta)?;
    let mut mv =
        Move::new(&b.item_id, &p.batch_id, &b.location_id, StockMovementKind::Adjust, p.delta, &date);
    mv.unit_cost = b.unit_cost;
    mv.reason = Some(&reason);
    record_move(conn, actor, &mv)?;
    audit::record(
        conn,
        actor,
        "stock.adjust",
        Some("stock_item"),
        Some(&b.item_id),
        Some(&json!({"qty_left": b.qty_left})),
        Some(&json!({"delta": p.delta, "reason": reason})),
    )?;
    get_item(conn, &b.item_id)
}

pub fn return_to_supplier(conn: &Connection, actor: &Actor, p: &ReturnStockParams) -> Result<StockItemInfo> {
    let b = batch_row(conn, &p.batch_id)?;
    let qty = qty_of("qty", p.qty)?;
    let supplier = check_supplier(conn, &p.supplier_id, "supplier_id")?;
    let credit = money("credit", p.credit, true)?;
    let reason = optional("reason", &p.reason, 300)?;
    let date = day_of(&p.date)?;
    bump_batch(conn, &p.batch_id, -qty)?;
    let mut mv = Move::new(&b.item_id, &p.batch_id, &b.location_id, StockMovementKind::Return, -qty, &date);
    mv.unit_cost = b.unit_cost;
    mv.credit = credit;
    mv.supplier_id = Some(&supplier.id);
    mv.reason = reason.as_deref();
    record_move(conn, actor, &mv)?;
    audit::record(
        conn,
        actor,
        "stock.return_to_supplier",
        Some("stock_item"),
        Some(&b.item_id),
        None,
        Some(&json!({"qty": qty, "credit": credit, "supplier": supplier.name, "reason": reason})),
    )?;
    get_item(conn, &b.item_id)
}

pub fn transfer(conn: &Connection, actor: &Actor, p: &TransferStockParams) -> Result<StockItemInfo> {
    let b = batch_row(conn, &p.batch_id)?;
    let qty = qty_of("qty", p.qty)?;
    let to = get_lookup(conn, Lookup::Location, &p.to_location_id, "to_location_id")?;
    if to.id == b.location_id {
        return Err(CoreError::invalid(
            "to_location_id",
            ValidationRule::StockSamePlace,
            "the stock is already in this place",
        ));
    }
    let date = day_of(&p.date)?;
    bump_batch(conn, &p.batch_id, -qty)?;
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM stock_batch WHERE item_id = ?1 AND location_id = ?2 AND lot_no IS ?3 AND expiry_date IS ?4
               AND unit_cost = ?5 AND received_on = ?6 LIMIT 1",
            params![b.item_id, to.id, b.lot_no, b.expiry_date, b.unit_cost, b.received_on],
            |r| r.get(0),
        )
        .optional()?;
    let target = match existing {
        Some(id) => {
            bump_batch(conn, &id, qty)?;
            id
        }
        None => insert_batch(
            conn,
            actor,
            &NewBatch {
                item_id: &b.item_id,
                location_id: &to.id,
                lot_no: b.lot_no.as_deref(),
                expiry_date: b.expiry_date.as_deref(),
                qty,
                unit_cost: b.unit_cost,
                received_on: &b.received_on,
                supplier_id: b.supplier_id.as_deref(),
                purchase_id: b.purchase_id.as_deref(),
            },
        )?,
    };
    let mut out =
        Move::new(&b.item_id, &p.batch_id, &b.location_id, StockMovementKind::TransferOut, -qty, &date);
    out.unit_cost = b.unit_cost;
    record_move(conn, actor, &out)?;
    let mut inn = Move::new(&b.item_id, &target, &to.id, StockMovementKind::TransferIn, qty, &date);
    inn.unit_cost = b.unit_cost;
    record_move(conn, actor, &inn)?;
    audit::record(
        conn,
        actor,
        "stock.transfer",
        Some("stock_item"),
        Some(&b.item_id),
        None,
        Some(&json!({"qty": qty, "from": b.location_id, "to": to.id})),
    )?;
    get_item(conn, &b.item_id)
}

// ───────────────────────────── service materials ─────────────────────────────

pub fn list_materials(conn: &Connection, service_id: &str) -> Result<Vec<ServiceMaterialInfo>> {
    let today = today_iso();
    let mut stmt = conn.prepare(
        "SELECT m.item_id, i.name_fa, i.name_ps, i.name_en, u.name_fa, u.name_ps, u.name_en, m.qty,
            COALESCE((SELECT SUM(b.qty_left) FROM stock_batch b WHERE b.item_id = m.item_id
                      AND (b.expiry_date IS NULL OR b.expiry_date >= ?2)), 0)
         FROM service_material m JOIN stock_item i ON i.id = m.item_id JOIN stock_unit u ON u.id = i.unit_id
         WHERE m.service_id = ?1 ORDER BY i.name_en, i.name_fa",
    )?;
    let rows = stmt
        .query_map(params![service_id, today], |r| {
            Ok(ServiceMaterialInfo {
                item_id: r.get(0)?,
                item_name: trio(r.get(1)?, r.get(2)?, r.get(3)?),
                unit: trio(r.get(4)?, r.get(5)?, r.get(6)?),
                qty: r.get(7)?,
                usable_qty: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn save_materials(
    conn: &Connection,
    actor: &Actor,
    p: &SaveServiceMaterialsParams,
) -> Result<Vec<ServiceMaterialInfo>> {
    let exists = scalar(conn, "SELECT COUNT(*) FROM service WHERE id = ?1", &[&p.service_id])?;
    if exists == 0 {
        return Err(CoreError::api(ErrorCode::NotFound, format!("service {}", p.service_id))
            .on_field("service_id", ValidationRule::StockLookupNotFound));
    }
    if p.materials.len() > 40 {
        return Err(CoreError::invalid("materials", ValidationRule::StockQuantity, "at most 40 materials"));
    }
    let mut seen = std::collections::HashSet::new();
    for m in &p.materials {
        qty_of("qty", m.qty)?;
        get_item(conn, &m.item_id)?;
        if !seen.insert(m.item_id.as_str()) {
            return Err(CoreError::invalid(
                "item_id",
                ValidationRule::StockQuantity,
                "an item is listed twice",
            ));
        }
    }
    let before = list_materials(conn, &p.service_id)?;
    conn.execute("DELETE FROM service_material WHERE service_id = ?1", [&p.service_id])?;
    for m in &p.materials {
        conn.execute(
            "INSERT INTO service_material(service_id, item_id, qty) VALUES (?1, ?2, ?3)",
            params![p.service_id, m.item_id, m.qty],
        )?;
    }
    let after = list_materials(conn, &p.service_id)?;
    audit::record(
        conn,
        actor,
        "service_materials.save",
        Some("service"),
        Some(&p.service_id),
        Some(&json!(before
            .iter()
            .map(|m| json!({"item": m.item_name.en, "qty": m.qty}))
            .collect::<Vec<_>>())),
        Some(&json!(after.iter().map(|m| json!({"item": m.item_name.en, "qty": m.qty})).collect::<Vec<_>>())),
    )?;
    Ok(after)
}

// ───────────────────────────── suppliers and labs ─────────────────────────────

fn kind_name(k: SupplierKind) -> &'static str {
    match k {
        SupplierKind::Supplier => "supplier",
        SupplierKind::Lab => "lab",
    }
}

const SUPPLIER_SELECT: &str = "SELECT s.id, s.kind, s.name, s.contact_person, s.phone, s.address, s.note, s.is_active, s.version,
    COALESCE((SELECT SUM(p.total) FROM purchase p WHERE p.supplier_id = s.id AND p.status = 'received'), 0),
    COALESCE((SELECT SUM(c.cost) FROM lab_case c WHERE c.lab_id = s.id AND c.status IN ('received', 'delivered')), 0),
    COALESCE((SELECT SUM(m.credit) FROM stock_movement m WHERE m.supplier_id = s.id AND m.kind = 'return'), 0),
    COALESCE((SELECT SUM(sp.amount) FROM supplier_payment sp WHERE sp.supplier_id = s.id AND sp.status = 'paid'), 0)
    FROM supplier s WHERE s.deleted_at IS NULL";

fn map_supplier(r: &rusqlite::Row<'_>) -> rusqlite::Result<SupplierInfo> {
    let kind: String = r.get(1)?;
    let purchases_total: i64 = r.get(9)?;
    let lab_total: i64 = r.get(10)?;
    let returns_total: i64 = r.get(11)?;
    let paid_total: i64 = r.get(12)?;
    Ok(SupplierInfo {
        id: r.get(0)?,
        kind: if kind == "lab" { SupplierKind::Lab } else { SupplierKind::Supplier },
        name: r.get(2)?,
        contact_person: r.get(3)?,
        phone: r.get(4)?,
        address: r.get(5)?,
        note: r.get(6)?,
        is_active: r.get::<_, i64>(7)? == 1,
        version: r.get(8)?,
        purchases_total,
        lab_total,
        returns_total,
        paid_total,
        balance: purchases_total + lab_total - returns_total - paid_total,
    })
}

pub fn get_supplier(conn: &Connection, id: &str) -> Result<SupplierInfo> {
    conn.query_row(&format!("{SUPPLIER_SELECT} AND s.id = ?1"), [id], map_supplier)
        .optional()?
        .ok_or_else(|| not_found("supplier", id, "id", ValidationRule::SupplierNotFound))
}

pub fn list_suppliers(conn: &Connection, p: &SupplierListParams) -> Result<Vec<SupplierInfo>> {
    let mut stmt = conn.prepare(&format!(
        "{SUPPLIER_SELECT} {} ORDER BY s.kind, s.name",
        if p.include_inactive { "" } else { "AND s.is_active = 1" }
    ))?;
    let rows = stmt
        .query_map([], map_supplier)?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|s| p.kind.is_none_or(|k| k == s.kind))
        .filter(|s| !p.owing_only || s.balance > 0)
        .collect();
    Ok(rows)
}

pub fn save_supplier(conn: &Connection, actor: &Actor, p: &SaveSupplierParams) -> Result<SupplierInfo> {
    let name = required("name", &p.name, 100)?;
    let contact = optional("contact_person", &p.contact_person, 100)?;
    let phone = optional("phone", &p.phone, 30)?;
    let address = optional("address", &p.address, 200)?;
    let note = optional("note", &p.note, 300)?;
    let now = now_iso();
    match p.id.as_deref().filter(|s| !s.is_empty()) {
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO supplier(id, kind, name, contact_person, phone, address, note, is_active, created_at, created_by,
                    updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?9, ?10)",
                params![id, kind_name(p.kind), name, contact, phone, address, note, p.is_active, now, actor.user_id],
            )?;
            audit::record(
                conn,
                actor,
                "supplier.create",
                Some("supplier"),
                Some(&id),
                None,
                Some(&json!({"name": name, "kind": kind_name(p.kind)})),
            )?;
            get_supplier(conn, &id)
        }
        Some(id) => {
            // The kind never changes after creation: a lab stays a lab in the history.
            let before = get_supplier(conn, id)?;
            let changed = conn.execute(
                "UPDATE supplier SET name = ?1, contact_person = ?2, phone = ?3, address = ?4, note = ?5, is_active = ?6,
                    updated_at = ?7, updated_by = ?8, version = version + 1
                 WHERE id = ?9 AND version = ?10 AND deleted_at IS NULL",
                params![
                    name,
                    contact,
                    phone,
                    address,
                    note,
                    p.is_active,
                    now,
                    actor.user_id,
                    id,
                    p.version.unwrap_or(before.version)
                ],
            )?;
            expect_one_row(changed, "supplier")?;
            let after = get_supplier(conn, id)?;
            audit::record(
                conn,
                actor,
                "supplier.update",
                Some("supplier"),
                Some(id),
                Some(&json!({"name": before.name, "phone": before.phone, "is_active": before.is_active})),
                Some(&json!({"name": after.name, "phone": after.phone, "is_active": after.is_active})),
            )?;
            Ok(after)
        }
    }
}

/// Everything that moved what the clinic owes one supplier, oldest first, with the running balance.
pub fn supplier_ledger(conn: &Connection, supplier_id: &str) -> Result<Vec<LedgerEntry>> {
    get_supplier(conn, supplier_id)?;
    // (date, order key, kind, number, description, amount)
    let mut rows: Vec<(String, String, LedgerKind, Option<String>, String, i64)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT local_date, created_at, number, COALESCE(note, ''), total FROM purchase
             WHERE supplier_id = ?1 AND status = 'received'",
        )?;
        let it = stmt.query_map([supplier_id], |r| {
            Ok((r.get(0)?, r.get(1)?, LedgerKind::Purchase, Some(r.get(2)?), r.get(3)?, r.get::<_, i64>(4)?))
        })?;
        for row in it {
            rows.push(row?);
        }
    }
    {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(received_date, sent_date, substr(created_at, 1, 10)), created_at, number, work_type, cost
             FROM lab_case WHERE lab_id = ?1 AND status IN ('received', 'delivered')",
        )?;
        let it = stmt.query_map([supplier_id], |r| {
            Ok((r.get(0)?, r.get(1)?, LedgerKind::Lab, Some(r.get(2)?), r.get(3)?, r.get::<_, i64>(4)?))
        })?;
        for row in it {
            rows.push(row?);
        }
    }
    {
        let mut stmt = conn.prepare(
            "SELECT local_date, created_at, COALESCE(reason, ''), credit FROM stock_movement
             WHERE supplier_id = ?1 AND kind = 'return' AND credit > 0",
        )?;
        let it = stmt.query_map([supplier_id], |r| {
            Ok((r.get(0)?, r.get(1)?, LedgerKind::Return, None, r.get(2)?, -r.get::<_, i64>(3)?))
        })?;
        for row in it {
            rows.push(row?);
        }
    }
    {
        let mut stmt = conn.prepare(
            "SELECT local_date, created_at, number, COALESCE(note, ''), amount FROM supplier_payment
             WHERE supplier_id = ?1 AND status = 'paid'",
        )?;
        let it = stmt.query_map([supplier_id], |r| {
            Ok((r.get(0)?, r.get(1)?, LedgerKind::Payment, Some(r.get(2)?), r.get(3)?, -r.get::<_, i64>(4)?))
        })?;
        for row in it {
            rows.push(row?);
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut balance = 0;
    Ok(rows
        .into_iter()
        .map(|(local_date, _, kind, number, description, amount)| {
            balance += amount;
            LedgerEntry { kind, local_date, number, description, amount, balance }
        })
        .collect())
}

// ───────────────────────────── purchases ─────────────────────────────

fn purchase_lines(conn: &Connection, purchase_id: &str) -> Result<Vec<PurchaseLineInfo>> {
    let mut stmt = conn.prepare(
        "SELECT l.id, l.item_id, i.name_fa, i.name_ps, i.name_en, u.name_fa, u.name_ps, u.name_en, l.qty, l.unit_cost,
            l.line_total, l.lot_no, l.expiry_date, pl.name_fa, pl.name_ps, pl.name_en
         FROM purchase_line l JOIN stock_item i ON i.id = l.item_id JOIN stock_unit u ON u.id = i.unit_id
         JOIN stock_location pl ON pl.id = l.location_id WHERE l.purchase_id = ?1 ORDER BY l.sort_order",
    )?;
    let rows = stmt
        .query_map([purchase_id], |r| {
            Ok(PurchaseLineInfo {
                id: r.get(0)?,
                item_id: r.get(1)?,
                item_name: trio(r.get(2)?, r.get(3)?, r.get(4)?),
                unit: trio(r.get(5)?, r.get(6)?, r.get(7)?),
                qty: r.get(8)?,
                unit_cost: r.get(9)?,
                line_total: r.get(10)?,
                lot_no: r.get(11)?,
                expiry_date: r.get(12)?,
                location: trio(r.get(13)?, r.get(14)?, r.get(15)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

const PURCHASE_SELECT: &str =
    "SELECT p.id, p.number, p.supplier_id, s.name, p.local_date, p.total, p.note, p.status, p.void_reason,
    au.display_name, p.version
    FROM purchase p JOIN supplier s ON s.id = p.supplier_id LEFT JOIN app_user au ON au.id = p.created_by
    WHERE 1 = 1";

fn map_purchase(r: &rusqlite::Row<'_>) -> rusqlite::Result<PurchaseInfo> {
    let status: String = r.get(7)?;
    Ok(PurchaseInfo {
        id: r.get(0)?,
        number: r.get(1)?,
        supplier_id: r.get(2)?,
        supplier_name: r.get(3)?,
        local_date: r.get(4)?,
        total: r.get(5)?,
        note: r.get(6)?,
        void: status == "void",
        void_reason: r.get(8)?,
        lines: Vec::new(),
        created_by_name: r.get(9)?,
        version: r.get(10)?,
    })
}

pub fn get_purchase(conn: &Connection, id: &str) -> Result<PurchaseInfo> {
    let mut p = conn
        .query_row(&format!("{PURCHASE_SELECT} AND p.id = ?1"), [id], map_purchase)
        .optional()?
        .ok_or_else(|| not_found("purchase", id, "id", ValidationRule::PurchaseNotFound))?;
    p.lines = purchase_lines(conn, id)?;
    Ok(p)
}

pub fn list_purchases(conn: &Connection, p: &PurchaseListParams) -> Result<Vec<PurchaseInfo>> {
    let to = opt_date("to", &p.to)?.unwrap_or_else(today_iso);
    let from = match opt_date("from", &p.from)? {
        Some(f) => f,
        None => plus_days(&to, -90)?,
    };
    let mut sql = format!("{PURCHASE_SELECT} AND p.local_date BETWEEN ? AND ?");
    let mut args = vec![Value::Text(from), Value::Text(to)];
    if let Some(s) = p.supplier_id.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND p.supplier_id = ?");
        args.push(Value::Text(s.to_string()));
    }
    sql.push_str(" ORDER BY p.local_date DESC, p.created_at DESC LIMIT 300");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows =
        stmt.query_map(params_from_iter(args.iter()), map_purchase)?.collect::<rusqlite::Result<Vec<_>>>()?;
    for row in &mut rows {
        row.lines = purchase_lines(conn, &row.id)?;
    }
    Ok(rows)
}

pub fn create_purchase(conn: &Connection, actor: &Actor, p: &CreatePurchaseParams) -> Result<PurchaseInfo> {
    let supplier = check_supplier(conn, &p.supplier_id, "supplier_id")?;
    if supplier.kind == SupplierKind::Lab {
        return Err(CoreError::invalid(
            "supplier_id",
            ValidationRule::SupplierNotFound,
            "a lab supplies work, not goods",
        ));
    }
    if p.lines.is_empty() || p.lines.len() > 100 {
        return Err(CoreError::invalid(
            "lines",
            ValidationRule::PurchaseEmpty,
            "a purchase needs 1 to 100 lines",
        ));
    }
    let date = day_of(&p.date)?;
    let note = optional("note", &p.note, 300)?;
    let paid = money("paid", p.paid, true)?;
    // Validate every line before anything is written.
    struct Line {
        item_id: String,
        qty: i64,
        unit_cost: i64,
        total: i64,
        lot: Option<String>,
        expiry: Option<String>,
        location_id: String,
    }
    let mut lines = Vec::with_capacity(p.lines.len());
    let mut total = 0i64;
    for l in &p.lines {
        let item = get_item(conn, &l.item_id)?;
        let qty = qty_of("qty", l.qty)?;
        let unit_cost = money("unit_cost", l.unit_cost, true)?;
        let location = get_lookup(conn, Lookup::Location, &l.location_id, "location_id")?;
        let line = Line {
            item_id: item.id,
            qty,
            unit_cost,
            total: line_total(qty, unit_cost)?,
            lot: optional("lot_no", &l.lot_no, 60)?,
            expiry: opt_date("expiry_date", &l.expiry_date)?,
            location_id: location.id,
        };
        total += line.total;
        lines.push(line);
    }
    if paid > total {
        return Err(CoreError::invalid(
            "paid",
            ValidationRule::PaymentExceeds,
            "more than the purchase total was paid",
        ));
    }
    let id = new_id();
    let number = next_number(conn, "PU")?;
    conn.execute(
        "INSERT INTO purchase(id, number, supplier_id, local_date, total, note, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, number, supplier.id, date, total, note, now_iso(), actor.user_id],
    )?;
    for (order, l) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO purchase_line(id, purchase_id, item_id, qty, unit_cost, line_total, lot_no, expiry_date, location_id, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                new_id(),
                id,
                l.item_id,
                l.qty,
                l.unit_cost,
                l.total,
                l.lot,
                l.expiry,
                l.location_id,
                order as i64
            ],
        )?;
        let batch = insert_batch(
            conn,
            actor,
            &NewBatch {
                item_id: &l.item_id,
                location_id: &l.location_id,
                lot_no: l.lot.as_deref(),
                expiry_date: l.expiry.as_deref(),
                qty: l.qty,
                unit_cost: l.unit_cost,
                received_on: &date,
                supplier_id: Some(&supplier.id),
                purchase_id: Some(&id),
            },
        )?;
        let mut mv = Move::new(&l.item_id, &batch, &l.location_id, StockMovementKind::Purchase, l.qty, &date);
        mv.unit_cost = l.unit_cost;
        mv.supplier_id = Some(&supplier.id);
        mv.purchase_id = Some(&id);
        record_move(conn, actor, &mv)?;
    }
    audit::record(
        conn,
        actor,
        "purchase.create",
        Some("purchase"),
        Some(&id),
        None,
        Some(&json!({"number": number, "supplier": supplier.name, "total": total, "lines": lines.len()})),
    )?;
    if paid > 0 {
        let method_id = p.method_id.clone().unwrap_or_default();
        add_supplier_payment(
            conn,
            actor,
            &AddSupplierPaymentParams {
                supplier_id: supplier.id.clone(),
                amount: paid,
                method_id,
                date: Some(date.clone()),
                note: Some(number.clone()),
            },
        )?;
    }
    get_purchase(conn, &id)
}

pub fn void_purchase(conn: &Connection, actor: &Actor, p: &VoidWithReasonParams) -> Result<PurchaseInfo> {
    let before = get_purchase(conn, &p.id)?;
    if before.void {
        return Err(CoreError::invalid("id", ValidationRule::PurchaseVoid, "this purchase is already void"));
    }
    let reason = required("reason", &p.reason, 300)?;
    let date = today_iso();
    let mut stmt = conn.prepare(
        "SELECT b.id, b.item_id, b.location_id, b.qty_left FROM stock_batch b WHERE b.purchase_id = ?1",
    )?;
    let batches = stmt
        .query_map([&p.id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (batch_id, item_id, location_id, left) in batches {
        let original: i64 = conn.query_row(
            "SELECT COALESCE(SUM(qty), 0) FROM stock_movement WHERE batch_id = ?1 AND kind = 'purchase'",
            [&batch_id],
            |r| r.get(0),
        )?;
        let touched: i64 = conn.query_row(
            "SELECT COUNT(*) FROM stock_movement WHERE batch_id = ?1 AND kind NOT IN ('purchase')",
            [&batch_id],
            |r| r.get(0),
        )?;
        if left != original || touched > 0 {
            return Err(CoreError::invalid(
                "id",
                ValidationRule::PurchaseUsed,
                "part of this purchase was already used or moved; adjust the stock instead",
            ));
        }
        bump_batch(conn, &batch_id, -left)?;
        let mut mv =
            Move::new(&item_id, &batch_id, &location_id, StockMovementKind::VoidPurchase, -left, &date);
        mv.purchase_id = Some(&p.id);
        mv.reason = Some(&reason);
        record_move(conn, actor, &mv)?;
    }
    let changed = conn.execute(
        "UPDATE purchase SET status = 'void', void_reason = ?1, voided_at = ?2, version = version + 1
         WHERE id = ?3 AND version = ?4 AND status = 'received'",
        params![reason, now_iso(), p.id, p.version],
    )?;
    expect_one_row(changed, "purchase")?;
    audit::record(
        conn,
        actor,
        "purchase.void",
        Some("purchase"),
        Some(&p.id),
        Some(&json!({"number": before.number, "total": before.total})),
        Some(&json!({"reason": reason})),
    )?;
    get_purchase(conn, &p.id)
}

// ───────────────────────────── payments to suppliers ─────────────────────────────

const PAYMENT_SELECT: &str = "SELECT sp.id, sp.number, sp.supplier_id, s.name, sp.amount, sp.method_id, m.name_fa, m.name_ps, m.name_en,
    sp.local_date, sp.note, sp.status, sp.void_reason, au.display_name, sp.version
    FROM supplier_payment sp JOIN supplier s ON s.id = sp.supplier_id LEFT JOIN payment_method m ON m.id = sp.method_id
    LEFT JOIN app_user au ON au.id = sp.created_by WHERE 1 = 1";

fn map_payment(r: &rusqlite::Row<'_>) -> rusqlite::Result<SupplierPaymentInfo> {
    let status: String = r.get(11)?;
    let method_fa: Option<String> = r.get(6)?;
    let method_ps: Option<String> = r.get(7)?;
    let method_en: Option<String> = r.get(8)?;
    let method = method_fa.map(|fa| trio(fa, method_ps.unwrap_or_default(), method_en.unwrap_or_default()));
    Ok(SupplierPaymentInfo {
        id: r.get(0)?,
        number: r.get(1)?,
        supplier_id: r.get(2)?,
        supplier_name: r.get(3)?,
        amount: r.get(4)?,
        method_id: r.get(5)?,
        method,
        local_date: r.get(9)?,
        note: r.get(10)?,
        void: status == "void",
        void_reason: r.get(12)?,
        created_by_name: r.get(13)?,
        version: r.get(14)?,
    })
}

fn get_payment(conn: &Connection, id: &str) -> Result<SupplierPaymentInfo> {
    conn.query_row(&format!("{PAYMENT_SELECT} AND sp.id = ?1"), [id], map_payment)
        .optional()?
        .ok_or_else(|| not_found("supplier payment", id, "id", ValidationRule::SupplierPaymentNotFound))
}

pub fn list_payments(conn: &Connection, p: &SupplierPaymentListParams) -> Result<Vec<SupplierPaymentInfo>> {
    let to = opt_date("to", &p.to)?.unwrap_or_else(today_iso);
    let from = match opt_date("from", &p.from)? {
        Some(f) => f,
        None => plus_days(&to, -90)?,
    };
    let mut sql = format!("{PAYMENT_SELECT} AND sp.local_date BETWEEN ? AND ?");
    let mut args = vec![Value::Text(from), Value::Text(to)];
    if let Some(s) = p.supplier_id.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND sp.supplier_id = ?");
        args.push(Value::Text(s.to_string()));
    }
    sql.push_str(" ORDER BY sp.local_date DESC, sp.created_at DESC LIMIT 300");
    let mut stmt = conn.prepare(&sql)?;
    let rows =
        stmt.query_map(params_from_iter(args.iter()), map_payment)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn add_supplier_payment(
    conn: &Connection,
    actor: &Actor,
    p: &AddSupplierPaymentParams,
) -> Result<SupplierPaymentInfo> {
    let supplier = check_supplier(conn, &p.supplier_id, "supplier_id")?;
    let amount = money("amount", p.amount, false)?;
    let date = day_of(&p.date)?;
    let note = optional("note", &p.note, 300)?;
    let method: Option<(String, i64)> = conn
        .query_row(
            "SELECT id, is_cash FROM payment_method WHERE id = ?1 AND deleted_at IS NULL AND is_active = 1",
            [&p.method_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((method_id, is_cash)) = method else {
        return Err(not_found(
            "payment method",
            &p.method_id,
            "method_id",
            ValidationRule::PaymentMethodNotFound,
        ));
    };
    let id = new_id();
    let number = next_number(conn, "SP")?;
    // The money leaves the clinic: it is an expense too, so the cash closing and the profit stay right.
    let expense = crate::billing::add_expense(
        conn,
        actor,
        &AddExpenseParams {
            category_id: if supplier.kind == SupplierKind::Lab {
                "ec-lab".into()
            } else {
                "ec-materials".into()
            },
            amount,
            date: date.clone(),
            description: format!("{number} - {}", supplier.name),
            payee: Some(supplier.name.clone()),
            from_cash: is_cash == 1,
        },
    )?;
    conn.execute(
        "INSERT INTO supplier_payment(id, number, supplier_id, amount, method_id, local_date, note, expense_id, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, number, supplier.id, amount, method_id, date, note, expense.id, now_iso(), actor.user_id],
    )?;
    audit::record(
        conn,
        actor,
        "supplier_payment.add",
        Some("supplier_payment"),
        Some(&id),
        None,
        Some(&json!({"number": number, "supplier": supplier.name, "amount": amount, "date": date})),
    )?;
    get_payment(conn, &id)
}

pub fn void_supplier_payment(
    conn: &Connection,
    actor: &Actor,
    p: &VoidWithReasonParams,
) -> Result<SupplierPaymentInfo> {
    let before = get_payment(conn, &p.id)?;
    if before.void {
        return Err(CoreError::invalid(
            "id",
            ValidationRule::SupplierPaymentVoid,
            "this payment is already void",
        ));
    }
    let reason = required("reason", &p.reason, 300)?;
    let expense: Option<(String, i64)> = conn
        .query_row(
            "SELECT e.id, e.version FROM supplier_payment sp JOIN expense e ON e.id = sp.expense_id
             WHERE sp.id = ?1 AND e.deleted_at IS NULL",
            [&p.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((expense_id, version)) = expense {
        crate::billing::delete_expense(
            conn,
            actor,
            &DeleteExpenseParams { id: expense_id, version, reason: reason.clone() },
        )?;
    }
    let changed = conn.execute(
        "UPDATE supplier_payment SET status = 'void', void_reason = ?1, version = version + 1
         WHERE id = ?2 AND version = ?3 AND status = 'paid'",
        params![reason, p.id, p.version],
    )?;
    expect_one_row(changed, "supplier payment")?;
    audit::record(
        conn,
        actor,
        "supplier_payment.void",
        Some("supplier_payment"),
        Some(&p.id),
        Some(&json!({"number": before.number, "amount": before.amount})),
        Some(&json!({"reason": reason})),
    )?;
    get_payment(conn, &p.id)
}

// ───────────────────────────── dental lab ─────────────────────────────

const LAB_SELECT: &str = "SELECT c.id, c.number, c.patient_id, p.full_name, p.patient_number, c.doctor_id, d.full_name,
    c.treatment_item_id, c.lab_id, s.name, c.teeth, c.work_type, c.shade, c.status, c.sent_date, c.expected_date,
    c.received_date, c.delivered_date, c.cost, c.note, c.cancel_reason, c.version
    FROM lab_case c JOIN patient p ON p.id = c.patient_id LEFT JOIN doctor d ON d.id = c.doctor_id
    JOIN supplier s ON s.id = c.lab_id WHERE 1 = 1";

fn map_lab(r: &rusqlite::Row<'_>, today: &str) -> rusqlite::Result<LabCaseInfo> {
    let status = lab_status_of(&r.get::<_, String>(13)?);
    let expected: Option<String> = r.get(15)?;
    let late = matches!(status, LabStatus::Sent) && expected.as_deref().is_some_and(|e| e < today);
    let days_late = match (&expected, late) {
        (Some(e), true) => match (parse_date("date", today), parse_date("date", e)) {
            (Ok(t), Ok(e)) => (t - e).whole_days(),
            _ => 0,
        },
        _ => 0,
    };
    Ok(LabCaseInfo {
        id: r.get(0)?,
        number: r.get(1)?,
        patient_id: r.get(2)?,
        patient_name: r.get(3)?,
        patient_number: r.get(4)?,
        doctor_id: r.get(5)?,
        doctor_name: r.get(6)?,
        treatment_item_id: r.get(7)?,
        lab_id: r.get(8)?,
        lab_name: r.get(9)?,
        teeth: parse_teeth(&r.get::<_, String>(10)?),
        work_type: r.get(11)?,
        shade: r.get(12)?,
        status,
        sent_date: r.get(14)?,
        expected_date: expected,
        received_date: r.get(16)?,
        delivered_date: r.get(17)?,
        cost: r.get(18)?,
        note: r.get(19)?,
        cancel_reason: r.get(20)?,
        delayed: late,
        days_late,
        version: r.get(21)?,
    })
}

pub fn get_lab_case(conn: &Connection, id: &str) -> Result<LabCaseInfo> {
    let today = today_iso();
    conn.query_row(&format!("{LAB_SELECT} AND c.id = ?1"), [id], |r| map_lab(r, &today))
        .optional()?
        .ok_or_else(|| not_found("lab case", id, "id", ValidationRule::LabCaseNotFound))
}

pub fn list_lab_cases(conn: &Connection, p: &LabCaseListParams) -> Result<Vec<LabCaseInfo>> {
    let today = today_iso();
    let mut sql = String::from(LAB_SELECT);
    let mut args: Vec<Value> = Vec::new();
    if let Some(s) = p.status {
        sql.push_str(" AND c.status = ?");
        args.push(Value::Text(lab_status_str(s).to_string()));
    }
    if let Some(l) = p.lab_id.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND c.lab_id = ?");
        args.push(Value::Text(l.to_string()));
    }
    if let Some(pt) = p.patient_id.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND c.patient_id = ?");
        args.push(Value::Text(pt.to_string()));
    }
    if p.open_only {
        sql.push_str(" AND c.status IN ('planned', 'sent')");
    }
    if p.delayed_only {
        sql.push_str(" AND c.status = 'sent' AND c.expected_date < ?");
        args.push(Value::Text(today.clone()));
    }
    sql.push_str(&format!(
        " ORDER BY CASE c.status WHEN 'sent' THEN 0 WHEN 'planned' THEN 1 WHEN 'received' THEN 2 ELSE 3 END,
          c.expected_date IS NULL, c.expected_date, c.created_at DESC LIMIT {MAX_LIST}"
    ));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_from_iter(args.iter()), |r| map_lab(r, &today))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn teeth_text(teeth: &[u8]) -> Result<String> {
    if teeth.len() > 32 || teeth.iter().any(|t| !(1..=85).contains(t)) {
        return Err(CoreError::invalid(
            "teeth",
            ValidationRule::StockQuantity,
            "a tooth number is not valid",
        ));
    }
    Ok(teeth.iter().map(u8::to_string).collect::<Vec<_>>().join(","))
}

fn check_lab(conn: &Connection, lab_id: &str) -> Result<SupplierInfo> {
    let s = check_supplier(conn, lab_id, "lab_id")?;
    if s.kind != SupplierKind::Lab {
        return Err(not_found("lab", lab_id, "lab_id", ValidationRule::SupplierNotFound));
    }
    Ok(s)
}

fn check_doctor(conn: &Connection, doctor_id: &Option<String>) -> Result<Option<String>> {
    let Some(id) = doctor_id.as_deref().filter(|s| !s.is_empty()) else { return Ok(None) };
    if scalar(conn, "SELECT COUNT(*) FROM doctor WHERE id = ?1 AND deleted_at IS NULL", &[&id])? == 0 {
        return Err(not_found("doctor", id, "doctor_id", ValidationRule::LabCaseNotFound));
    }
    Ok(Some(id.to_string()))
}

pub fn create_lab_case(conn: &Connection, actor: &Actor, p: &CreateLabCaseParams) -> Result<LabCaseInfo> {
    check_patient(conn, &p.patient_id)?;
    let lab = check_lab(conn, &p.lab_id)?;
    let doctor = check_doctor(conn, &p.doctor_id)?;
    let work = required("work_type", &p.work_type, 100)?;
    let shade = optional("shade", &p.shade, 30)?;
    let note = optional("note", &p.note, 500)?;
    let teeth = teeth_text(&p.teeth)?;
    let expected = opt_date("expected_date", &p.expected_date)?;
    let cost = money("cost", p.cost, true)?;
    let treatment = p.treatment_item_id.as_deref().filter(|s| !s.is_empty());
    if let Some(t) = treatment {
        let ok = scalar(
            conn,
            "SELECT COUNT(*) FROM treatment_item WHERE id = ?1 AND patient_id = ?2 AND deleted_at IS NULL",
            &[&t, &p.patient_id],
        )?;
        if ok == 0 {
            return Err(not_found("treatment", t, "treatment_item_id", ValidationRule::ItemNotBillable));
        }
    }
    let id = new_id();
    let number = next_number(conn, "LC")?;
    let now = now_iso();
    conn.execute(
        "INSERT INTO lab_case(id, number, patient_id, doctor_id, treatment_item_id, lab_id, teeth, work_type, shade,
            expected_date, cost, note, created_at, created_by, updated_at, updated_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?13, ?14)",
        params![
            id,
            number,
            p.patient_id,
            doctor,
            treatment,
            lab.id,
            teeth,
            work,
            shade,
            expected,
            cost,
            note,
            now,
            actor.user_id
        ],
    )?;
    audit::record(
        conn,
        actor,
        "lab_case.create",
        Some("lab_case"),
        Some(&id),
        None,
        Some(&json!({"number": number, "lab": lab.name, "work_type": work, "teeth": teeth, "cost": cost})),
    )?;
    get_lab_case(conn, &id)
}

pub fn update_lab_case(conn: &Connection, actor: &Actor, p: &UpdateLabCaseParams) -> Result<LabCaseInfo> {
    let before = get_lab_case(conn, &p.id)?;
    if matches!(before.status, LabStatus::Delivered | LabStatus::Cancelled) {
        return Err(CoreError::invalid("id", ValidationRule::LabCaseClosed, "this lab case is closed"));
    }
    let lab = check_lab(conn, &p.lab_id)?;
    let doctor = check_doctor(conn, &p.doctor_id)?;
    let work = required("work_type", &p.work_type, 100)?;
    let shade = optional("shade", &p.shade, 30)?;
    let note = optional("note", &p.note, 500)?;
    let teeth = teeth_text(&p.teeth)?;
    let expected = opt_date("expected_date", &p.expected_date)?;
    let cost = money("cost", p.cost, true)?;
    let changed = conn.execute(
        "UPDATE lab_case SET lab_id = ?1, doctor_id = ?2, teeth = ?3, work_type = ?4, shade = ?5, expected_date = ?6, cost = ?7,
            note = ?8, updated_at = ?9, updated_by = ?10, version = version + 1
         WHERE id = ?11 AND version = ?12",
        params![lab.id, doctor, teeth, work, shade, expected, cost, note, now_iso(), actor.user_id, p.id, p.version],
    )?;
    expect_one_row(changed, "lab case")?;
    audit::record(
        conn,
        actor,
        "lab_case.update",
        Some("lab_case"),
        Some(&p.id),
        Some(&json!({"lab": before.lab_name, "work_type": before.work_type, "cost": before.cost})),
        Some(&json!({"lab": lab.name, "work_type": work, "cost": cost})),
    )?;
    get_lab_case(conn, &p.id)
}

pub fn set_lab_status(conn: &Connection, actor: &Actor, p: &SetLabStatusParams) -> Result<LabCaseInfo> {
    let before = get_lab_case(conn, &p.id)?;
    let allowed = matches!(
        (before.status, p.status),
        (LabStatus::Planned, LabStatus::Sent)
            | (LabStatus::Sent, LabStatus::Received)
            | (LabStatus::Received, LabStatus::Delivered)
            | (LabStatus::Planned | LabStatus::Sent | LabStatus::Received, LabStatus::Cancelled)
    );
    if !allowed {
        return Err(CoreError::invalid(
            "status",
            ValidationRule::LabStatusInvalid,
            "this lab case cannot change to that status",
        ));
    }
    let date = day_of(&p.date)?;
    let now = now_iso();
    let (cost, reason) = match p.status {
        LabStatus::Received => (
            match p.cost {
                Some(c) => money("cost", c, true)?,
                None => before.cost,
            },
            None,
        ),
        LabStatus::Cancelled => {
            (before.cost, Some(required("reason", p.reason.as_deref().unwrap_or(""), 300)?))
        }
        _ => (before.cost, None),
    };
    if let (Some(sent), LabStatus::Received) = (before.sent_date.as_deref(), p.status) {
        if date.as_str() < sent {
            return Err(CoreError::invalid(
                "date",
                ValidationRule::DateFormat,
                "the work cannot come back before it was sent",
            ));
        }
    }
    let date_set = match p.status {
        LabStatus::Sent => "sent_date = ?2,",
        LabStatus::Received => "received_date = ?2,",
        LabStatus::Delivered => "delivered_date = ?2,",
        _ => "",
    };
    let changed = conn.execute(
        &format!(
            "UPDATE lab_case SET status = ?1, {date_set} cost = ?3, cancel_reason = COALESCE(?4, cancel_reason),
                updated_at = ?5, updated_by = ?6, version = version + 1 WHERE id = ?7 AND version = ?8"
        ),
        params![
            lab_status_str(p.status),
            date,
            cost,
            reason,
            now,
            actor.user_id,
            p.id,
            p.version
        ],
    )?;
    expect_one_row(changed, "lab case")?;
    audit::record(
        conn,
        actor,
        "lab_case.status",
        Some("lab_case"),
        Some(&p.id),
        Some(&json!({"status": lab_status_str(before.status), "cost": before.cost})),
        Some(&json!({"status": lab_status_str(p.status), "cost": cost, "date": date, "reason": reason})),
    )?;
    get_lab_case(conn, &p.id)
}

/// Lab cost of the cases that came back in a period, next to what their treatments were invoiced for.
pub fn lab_report(conn: &Connection, p: &PeriodParams) -> Result<LabReport> {
    let from = date_of("from", &p.from)?;
    let to = date_of("to", &p.to)?;
    if from > to {
        return Err(CoreError::invalid("from", ValidationRule::PeriodRange, "the period is backwards"));
    }
    let mut stmt = conn.prepare(
        "SELECT c.id, c.number, pt.full_name, s.name, c.work_type, c.teeth, c.received_date, c.cost,
            (SELECT SUM(l.line_total) FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id
             WHERE l.treatment_item_id = c.treatment_item_id AND i.status = 'issued')
         FROM lab_case c JOIN patient pt ON pt.id = c.patient_id JOIN supplier s ON s.id = c.lab_id
         WHERE c.status IN ('received', 'delivered') AND c.received_date BETWEEN ?1 AND ?2
         ORDER BY c.received_date, c.number",
    )?;
    let rows = stmt
        .query_map([&from, &to], |r| {
            Ok(LabProfitRow {
                case_id: r.get(0)?,
                number: r.get(1)?,
                patient_name: r.get(2)?,
                lab_name: r.get(3)?,
                work_type: r.get(4)?,
                teeth: parse_teeth(&r.get::<_, String>(5)?),
                received_date: r.get(6)?,
                cost: r.get(7)?,
                billed: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let total_cost: i64 = rows.iter().map(|r| r.cost).sum();
    let total_billed: i64 = rows.iter().filter_map(|r| r.billed).sum();
    Ok(LabReport { rows, total_cost, total_billed })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn stock_item_seed_file_is_valid() {
        let items = stock_item_seeds().unwrap();
        assert!(items.len() >= 30);
        let mut codes = HashSet::new();
        for i in &items {
            assert!(codes.insert(i.code.clone()), "duplicate code {}", i.code);
            for n in [&i.name.fa, &i.name.ps, &i.name.en] {
                assert!(!n.trim().is_empty(), "{}: empty name", i.code);
            }
            // Dari labels use Persian ی/ک, never Arabic ي/ك (search and sorting depend on it).
            assert!(!i.name.fa.contains(['ي', 'ك']), "{}: {}", i.code, i.name.fa);
        }
    }

    #[test]
    fn stock_item_seeds_point_at_existing_categories_and_units_and_are_idempotent() {
        let t = crate::tests::T::new();
        t.setup();
        let items = stock_item_seeds().unwrap();
        t.core
            .with_db(|o| {
                for i in &items {
                    let known = |table: &str, id: &str| -> Result<i64> {
                        Ok(o.conn.query_row(
                            &format!("SELECT COUNT(*) FROM {table} WHERE id = ?1"),
                            [id],
                            |r| r.get(0),
                        )?)
                    };
                    assert_eq!(known("stock_category", &i.category)?, 1, "{}: category", i.code);
                    assert_eq!(known("stock_unit", &i.unit)?, 1, "{}: unit", i.code);
                }
                let count = |c: &Connection| -> Result<i64> {
                    Ok(c.query_row("SELECT COUNT(*) FROM stock_item", [], |r| r.get(0))?)
                };
                assert_eq!(count(&o.conn)?, items.len() as i64, "seeded on the first start");
                // The clinic renames one item and deletes another; a second sync touches neither.
                let renamed = seed_id("stock_item", &items[0].code);
                let deleted = seed_id("stock_item", &items[1].code);
                o.conn.execute(
                    "UPDATE stock_item SET name_en = 'Our gloves', min_qty = 5 WHERE id = ?1",
                    [&renamed],
                )?;
                o.conn.execute(
                    "UPDATE stock_item SET deleted_at = ?2 WHERE id = ?1",
                    params![deleted, now_iso()],
                )?;
                sync_seeds(&o.conn)?;
                assert_eq!(count(&o.conn)?, items.len() as i64, "no duplicates, nothing brought back");
                let (name, min): (String, i64) = o.conn.query_row(
                    "SELECT name_en, min_qty FROM stock_item WHERE id = ?1",
                    [&renamed],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                assert_eq!((name.as_str(), min), ("Our gloves", 5));
                let still_deleted: Option<String> =
                    o.conn.query_row("SELECT deleted_at FROM stock_item WHERE id = ?1", [&deleted], |r| {
                        r.get(0)
                    })?;
                assert!(still_deleted.is_some());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn deleting_an_item_archives_it_when_it_has_history_and_removes_it_otherwise() {
        use serde_json::{json, Value};

        let t = crate::tests::T::new();
        t.setup();
        let owner = t.login("owner", "owner-pass-1");
        let places = t.ok("stock_locations.list", json!({}), Some(&owner));
        let place = places[0]["id"].as_str().unwrap().to_string();
        let list = |inactive: bool| -> Vec<Value> {
            t.ok("stock_items.list", json!({"include_inactive": inactive}), Some(&owner))
                .as_array()
                .unwrap()
                .clone()
        };
        // Starter (seeded) items are deleted exactly like the ones the clinic made.
        let items = list(false);
        let used = items[0]["id"].as_str().unwrap().to_string();
        let unused = items[1]["id"].as_str().unwrap().to_string();
        let before = items.len();
        t.ok(
            "stock.receive",
            json!({"item_id": used, "qty": 100, "unit_cost": 0, "location_id": place}),
            Some(&owner),
        );

        let r = t.ok("stock_items.delete", json!({"id": used}), Some(&owner));
        assert_eq!(r["archived"], true, "an item with history is archived, not removed");
        assert_eq!(list(false).len(), before - 1, "an archived item is hidden from the normal list");
        let archived = t.ok("stock_items.get", json!({"id": used}), Some(&owner));
        assert_eq!(archived["is_active"], false);
        assert_eq!(archived["qty"], 100, "its stock and history stay readable");
        let moves = t.ok("stock_movements.list", json!({"item_id": used}), Some(&owner));
        assert_eq!(moves.as_array().unwrap().len(), 1);
        assert!(list(true).iter().any(|i| i["id"] == used.as_str()), "still there to switch on");

        let r = t.ok("stock_items.delete", json!({"id": unused}), Some(&owner));
        assert_eq!(r["archived"], false, "an item without history is removed");
        assert!(!list(true).iter().any(|i| i["id"] == unused.as_str()));
        assert!(t.call("stock_items.get", json!({"id": unused}), Some(&owner)).is_err());
        let again = t.call("stock_items.delete", json!({"id": unused}), Some(&owner));
        assert!(again.is_err(), "no second delete");

        // The next start brings neither of them back (and does not switch the archived one on).
        t.core.with_db(|o| sync_seeds(&o.conn)).unwrap();
        assert_eq!(list(false).len(), before - 1);
        assert!(!list(true).iter().any(|i| i["id"] == unused.as_str()));
    }
}
