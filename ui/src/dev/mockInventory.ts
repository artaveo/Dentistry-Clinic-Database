// The inventory part of the in-browser mock Core (see mockCore.ts): enough stock, purchases, suppliers and
// lab cases to work on and take screenshots of the inventory screens without the Rust Core. The rules
// (first-expiry-first-out, balances, statuses) follow the real Core in shared/src and core/src/inventory.rs
// but are simplified; the real behaviour is covered by the Core's own tests.
/* eslint-disable @typescript-eslint/no-explicit-any */
import type {
  BatchState, LabCaseInfo, LedgerEntry, PaymentMethodInfo, PurchaseInfo, StockBatchInfo, StockItemInfo, StockLookupInfo, StockMovementInfo, SupplierInfo, SupplierPaymentInfo, Translations,
} from "../../../shared/ts/contract";

const day = (n: number) => new Date(Date.now() + n * 86_400_000).toISOString().slice(0, 10);
const tri = (fa: string, ps: string, en: string): Translations => ({ fa, ps, en });
const uid = (() => {
  let n = 0;
  return (p: string) => `${p}-${++n}`;
})();
class Fail extends Error {
  constructor(public rule: string, public field: string | null = null) {
    super(rule);
  }
}

const lookup = (id: string, name: Translations, sort: number): StockLookupInfo => ({ id, name, is_system: true, is_active: true, sort_order: sort, version: 1 });
const categories = [
  lookup("sc-disposable", tri("مصرفی و حفاظتی", "مصرفي او ساتونکي", "Disposables and protection"), 10),
  lookup("sc-anesthetic", tri("بی‌حسی", "بې‌حسي", "Anesthetics"), 20),
  lookup("sc-restorative", tri("ترمیمی و پرکردگی", "ترمیمي او ډکونه", "Restorative"), 30),
  lookup("sc-endo", tri("عصب‌کشی", "عصب‌کشي", "Endodontic"), 40),
];
const units = [lookup("su-piece", tri("عدد", "دانه", "Piece"), 10), lookup("su-box", tri("بسته", "بکس", "Box"), 20), lookup("su-tube", tri("تیوب", "تیوب", "Tube"), 30), lookup("su-ml", tri("میلی‌لیتر", "ملي‌لیتر", "ml"), 40)];
const places = [lookup("sl-main", tri("گدام اصلی", "اصلي ګدام", "Main store"), 10), lookup("sl-room", tri("اتاق معاینه", "د معاینې خونه", "Treatment room"), 20)];

const methods: PaymentMethodInfo[] = [
  { id: "pm-cash", code: "cash", name: tri("نقد", "نغدي", "Cash"), is_cash: true, is_system: true, is_active: true, sort_order: 10, version: 1 },
  { id: "pm-bank", code: "bank_transfer", name: tri("حواله بانکی", "بانکي حواله", "Bank transfer"), is_cash: false, is_system: true, is_active: true, sort_order: 20, version: 1 },
];

type Item = { id: string; code: string | null; name: Translations; category_id: string; unit_id: string; min_qty: number; note: string | null; is_active: boolean; version: number };
type Batch = { id: string; item_id: string; location_id: string; lot_no: string | null; expiry_date: string | null; qty_left: number; unit_cost: number; received_on: string; supplier_id: string | null };

const items: Item[] = [
  { id: "it-gloves", code: "GLV-M", name: tri("دستکش لاتکس (M)", "لاتکس دستکشې (M)", "Latex gloves (M)"), category_id: "sc-disposable", unit_id: "su-box", min_qty: 500, note: null, is_active: true, version: 1 },
  { id: "it-lido", code: "ANS-LID", name: tri("لیدوکائین ۲٪", "لیدوکائین ۲٪", "Lidocaine 2%"), category_id: "sc-anesthetic", unit_id: "su-box", min_qty: 300, note: null, is_active: true, version: 1 },
  { id: "it-comp", code: "RST-A2", name: tri("کامپوزیت A2", "کامپوزیټ A2", "Composite A2"), category_id: "sc-restorative", unit_id: "su-tube", min_qty: 200, note: null, is_active: true, version: 1 },
  { id: "it-bond", code: "RST-BND", name: tri("بوند", "بوند", "Bonding agent"), category_id: "sc-restorative", unit_id: "su-ml", min_qty: 0, note: null, is_active: true, version: 1 },
  { id: "it-files", code: "END-F25", name: tri("فایل کانال ۲۵", "کانال فایل ۲۵", "Canal files 25"), category_id: "sc-endo", unit_id: "su-piece", min_qty: 600, note: null, is_active: true, version: 1 },
];
const batches: Batch[] = [
  { id: "b1", item_id: "it-gloves", location_id: "sl-main", lot_no: "G-2201", expiry_date: day(400), qty_left: 300, unit_cost: 45_000, received_on: day(-40), supplier_id: "su1" },
  { id: "b2", item_id: "it-lido", location_id: "sl-main", lot_no: "L-77", expiry_date: day(25), qty_left: 500, unit_cost: 60_000, received_on: day(-70), supplier_id: "su1" },
  { id: "b3", item_id: "it-lido", location_id: "sl-room", lot_no: "L-90", expiry_date: day(310), qty_left: 400, unit_cost: 62_000, received_on: day(-20), supplier_id: "su1" },
  { id: "b4", item_id: "it-comp", location_id: "sl-main", lot_no: "C-12", expiry_date: day(-12), qty_left: 150, unit_cost: 120_000, received_on: day(-300), supplier_id: "su1" },
  { id: "b5", item_id: "it-comp", location_id: "sl-main", lot_no: "C-31", expiry_date: day(200), qty_left: 500, unit_cost: 125_000, received_on: day(-15), supplier_id: "su1" },
  { id: "b6", item_id: "it-bond", location_id: "sl-main", lot_no: null, expiry_date: null, qty_left: 1500, unit_cost: 9_000, received_on: day(-60), supplier_id: null },
  { id: "b7", item_id: "it-files", location_id: "sl-main", lot_no: "F-5", expiry_date: day(500), qty_left: 1200, unit_cost: 5_000, received_on: day(-30), supplier_id: "su1" },
];
const moves: StockMovementInfo[] = [];
const suppliers: SupplierInfo[] = [];
const purchases: PurchaseInfo[] = [];
const payments: SupplierPaymentInfo[] = [];
const labs: LabCaseInfo[] = [];
const materials = new Map<string, { item_id: string; qty: number }[]>();

function supplier(id: string, kind: "supplier" | "lab", name: string, contact: string, phone: string): SupplierInfo {
  return { id, kind, name, contact_person: contact, phone, address: null, note: null, is_active: true, purchases_total: 0, lab_total: 0, returns_total: 0, paid_total: 0, balance: 0, version: 1 };
}
suppliers.push(supplier("su1", "supplier", "شرکت طبی کابل", "احمد رحیمی", "0700123456"), supplier("su2", "supplier", "Herat Dental Supply", "Nazia", "0799888777"), supplier("lab1", "lab", "لابراتوار نور", "استاد کریم", "0788456123"), supplier("lab2", "lab", "Smile Dental Lab", "Hamid", "0777111222"));

let purSeq = 0;
function addPurchase(supplierId: string, date: string, lines: { item: string; qty: number; cost: number; lot: string | null; expiry: string | null }[], paid: number) {
  const s = suppliers.find((x) => x.id === supplierId)!;
  const total = lines.reduce((a, l) => a + Math.round((l.qty * l.cost) / 100), 0);
  const id = uid("pu");
  purchases.push({
    id, number: `PU-1405-${String(++purSeq).padStart(6, "0")}`, supplier_id: supplierId, supplier_name: s.name, local_date: date, total, note: null, void: false, void_reason: null, created_by_name: "مالک", version: 1,
    lines: lines.map((l) => {
      const it = items.find((x) => x.id === l.item)!;
      return { id: uid("pl"), item_id: l.item, item_name: it.name, unit: units.find((u) => u.id === it.unit_id)!.name, qty: l.qty, unit_cost: l.cost, line_total: Math.round((l.qty * l.cost) / 100), lot_no: l.lot, expiry_date: l.expiry, location: places[0].name };
    }),
  });
  if (paid) payments.push({ id: uid("sp"), number: `SP-1405-${String(payments.length + 1).padStart(6, "0")}`, supplier_id: supplierId, supplier_name: s.name, amount: paid, method_id: "pm-cash", method: methods[0].name, local_date: date, note: null, void: false, void_reason: null, created_by_name: "مالک", version: 1 });
}
addPurchase("su1", day(-40), [{ item: "it-gloves", qty: 1000, cost: 45_000, lot: "G-2201", expiry: day(400) }, { item: "it-files", qty: 2000, cost: 5_000, lot: "F-5", expiry: day(500) }], 300_000);
addPurchase("su1", day(-15), [{ item: "it-comp", qty: 500, cost: 125_000, lot: "C-31", expiry: day(200) }], 0);

let labSeq = 0;
function addLab(p: Partial<LabCaseInfo> & { status: LabCaseInfo["status"] }): void {
  const lab = suppliers.find((s) => s.id === (p.lab_id ?? "lab1"))!;
  const expected = p.expected_date ?? null;
  const delayed = p.status === "sent" && !!expected && expected < day(0);
  labs.push({
    id: uid("lc"), number: `LC-1405-${String(++labSeq).padStart(6, "0")}`, patient_id: "p1", patient_name: "زرغونه احمدی", patient_number: "P-000001", doctor_id: "d1", doctor_name: "دکتر احمد", treatment_item_id: null,
    lab_id: lab.id, lab_name: lab.name, teeth: [36], work_type: "Crown PFM", shade: "A2", sent_date: null, expected_date: expected, received_date: null, delivered_date: null, cost: 300_000, note: null, cancel_reason: null,
    delayed, days_late: delayed ? Math.round((Date.parse(day(0)) - Date.parse(expected!)) / 86_400_000) : 0, version: 1, ...p,
  });
}
addLab({ status: "sent", sent_date: day(-12), expected_date: day(-3), patient_name: "زرغونه احمدی", teeth: [36], work_type: "Crown PFM" });
addLab({ status: "sent", sent_date: day(-2), expected_date: day(5), patient_name: "محمد نظری", patient_number: "P-000002", teeth: [14, 15, 16], work_type: "Bridge zirconia", shade: "B1", cost: 900_000, lab_id: "lab2", lab_name: "Smile Dental Lab" });
addLab({ status: "planned", expected_date: day(9), patient_name: "فاطمه رضایی", patient_number: "P-000003", teeth: [], work_type: "Full denture", shade: null, cost: 1_800_000 });
addLab({ status: "received", sent_date: day(-20), received_date: day(-1), expected_date: day(-4), patient_name: "علی کریمی", patient_number: "P-000004", teeth: [46], work_type: "Crown zirconia", cost: 450_000 });

function recompute(): void {
  for (const s of suppliers) {
    s.purchases_total = purchases.filter((p) => p.supplier_id === s.id && !p.void).reduce((a, p) => a + p.total, 0);
    s.lab_total = labs.filter((c) => c.lab_id === s.id && (c.status === "received" || c.status === "delivered")).reduce((a, c) => a + c.cost, 0);
    s.paid_total = payments.filter((p) => p.supplier_id === s.id && !p.void).reduce((a, p) => a + p.amount, 0);
    s.balance = s.purchases_total + s.lab_total - s.returns_total - s.paid_total;
  }
}
recompute();

const state = (e: string | null, today = day(0), soon = day(60)): BatchState => (e && e < today ? "expired" : e && e <= soon ? "expiring" : "valid");
function itemInfo(i: Item, days = 60): StockItemInfo {
  const soon = day(days);
  const bs = batches.filter((b) => b.item_id === i.id);
  const qty = bs.reduce((a, b) => a + b.qty_left, 0);
  const expired = bs.filter((b) => state(b.expiry_date) === "expired").reduce((a, b) => a + b.qty_left, 0);
  const expiring = bs.filter((b) => state(b.expiry_date, day(0), soon) === "expiring").reduce((a, b) => a + b.qty_left, 0);
  const usable = qty - expired;
  const dates = bs.filter((b) => b.qty_left > 0 && b.expiry_date).map((b) => b.expiry_date!).sort();
  return {
    id: i.id, code: i.code, name: i.name, category_id: i.category_id, category: categories.find((c) => c.id === i.category_id)!.name, unit_id: i.unit_id, unit: units.find((u) => u.id === i.unit_id)!.name,
    min_qty: i.min_qty, note: i.note, is_active: i.is_active, qty, usable_qty: usable, expired_qty: expired, expiring_qty: expiring, value: bs.reduce((a, b) => a + Math.round((b.qty_left * b.unit_cost) / 100), 0),
    nearest_expiry: dates[0] ?? null, low: i.min_qty > 0 && usable <= i.min_qty, version: i.version,
  };
}
function batchInfo(b: Batch, days = 60): StockBatchInfo {
  const it = items.find((i) => i.id === b.item_id)!;
  return {
    id: b.id, item_id: b.item_id, item_name: it.name, unit: units.find((u) => u.id === it.unit_id)!.name, location_id: b.location_id, location: places.find((p) => p.id === b.location_id)!.name, lot_no: b.lot_no, expiry_date: b.expiry_date,
    qty_left: b.qty_left, unit_cost: b.unit_cost, received_on: b.received_on, supplier_name: suppliers.find((s) => s.id === b.supplier_id)?.name ?? null, state: state(b.expiry_date, day(0), day(days)),
  };
}
function move(b: Batch, kind: StockMovementInfo["kind"], qty: number, reason: string | null): void {
  const it = items.find((i) => i.id === b.item_id)!;
  moves.unshift({
    id: uid("mv"), item_id: b.item_id, item_name: it.name, unit: units.find((u) => u.id === it.unit_id)!.name, kind, qty, unit_cost: b.unit_cost, credit: 0, location: places.find((p) => p.id === b.location_id)!.name, lot_no: b.lot_no,
    supplier_name: null, patient_id: null, treatment_item_id: null, reason, local_date: day(0), created_by_name: "مالک",
  });
}

const simpleList = (list: StockLookupInfo[], p: any) => list.filter((x) => p?.include_inactive || x.is_active);
function saveLookup(list: StockLookupInfo[], p: any): StockLookupInfo {
  if (!p.name.fa && !p.name.ps && !p.name.en) throw new Fail("required", "name");
  const name = { fa: p.name.fa || p.name.ps || p.name.en, ps: p.name.ps || p.name.fa || p.name.en, en: p.name.en || p.name.fa || p.name.ps };
  if (p.id) {
    const x = list.find((i) => i.id === p.id)!;
    Object.assign(x, { name, is_active: p.is_active, version: x.version + 1 });
    return x;
  }
  const x = { id: uid("lk"), name, is_system: false, is_active: p.is_active, sort_order: 50, version: 1 };
  list.push(x);
  return x;
}

/** Answers an inventory method, or `undefined` when the method is not one of these. */
export function inventoryCall(method: string, p: any): unknown {
  try {
    return answer(method, p);
  } catch (e) {
    if (e instanceof Fail) throw Object.assign(new Error(e.rule), { mockRule: e.rule, mockField: e.field });
    throw e;
  }
}

function answer(method: string, p: any): unknown {
  switch (method) {
    case "payment_methods.list":
      return methods;
    case "treatments.list":
      return [];
    case "stock_categories.list":
      return simpleList(categories, p);
    case "stock_categories.save":
      return saveLookup(categories, p);
    case "stock_units.list":
      return simpleList(units, p);
    case "stock_units.save":
      return saveLookup(units, p);
    case "stock_locations.list":
      return simpleList(places, p);
    case "stock_locations.save":
      return saveLookup(places, p);
    case "stock_items.list": {
      const needle = (p.search ?? "").toLowerCase();
      return items
        .filter((i) => p.include_inactive || i.is_active)
        .map((i) => itemInfo(i, p.expiring_days ?? 60))
        .filter((i) => !p.category_id || i.category_id === p.category_id)
        .filter((i) => (p.only === "low" ? i.low : p.only === "expired" ? i.expired_qty > 0 : p.only === "expiring" ? i.expiring_qty > 0 : true))
        .filter((i) => !needle || [i.name.fa, i.name.ps, i.name.en, i.code ?? ""].some((x) => x.toLowerCase().includes(needle)));
    }
    case "stock_items.get":
      return itemInfo(items.find((i) => i.id === p.id)!);
    case "stock_items.save": {
      if (!p.name.fa && !p.name.ps && !p.name.en) throw new Fail("required", "name");
      const name = { fa: p.name.fa || p.name.ps || p.name.en, ps: p.name.ps || p.name.fa || p.name.en, en: p.name.en || p.name.fa || p.name.ps };
      if (p.id) {
        const i = items.find((x) => x.id === p.id)!;
        Object.assign(i, { name, code: p.code, category_id: p.category_id, unit_id: p.unit_id, min_qty: p.min_qty, note: p.note, is_active: p.is_active, version: i.version + 1 });
        return itemInfo(i);
      }
      const i: Item = { id: uid("it"), code: p.code, name, category_id: p.category_id, unit_id: p.unit_id, min_qty: p.min_qty, note: p.note, is_active: p.is_active, version: 1 };
      items.push(i);
      return itemInfo(i);
    }
    case "stock_items.delete": {
      const at = items.findIndex((x) => x.id === p.id);
      if (at < 0) throw new Fail("not_found", "item_id");
      // History (a batch or a movement) keeps the item: it is only switched off.
      if (batches.some((b) => b.item_id === p.id) || moves.some((m) => m.item_id === p.id)) {
        items[at].is_active = false;
        items[at].version += 1;
        return { archived: true };
      }
      items.splice(at, 1);
      return { archived: false };
    }
    case "stock_batches.list":
      return batches.filter((b) => (p.include_empty || b.qty_left > 0) && (!p.item_id || b.item_id === p.item_id)).sort((a, b) => (a.expiry_date ?? "9999").localeCompare(b.expiry_date ?? "9999")).map((b) => batchInfo(b, p.expiring_days ?? 60));
    case "stock_movements.list":
      return moves.filter((m) => !p.item_id || m.item_id === p.item_id);
    case "stock_alerts.list": {
      const bs = batches.filter((b) => b.qty_left > 0).map((b) => batchInfo(b, p.expiring_days ?? 60));
      return {
        low: items.map((i) => itemInfo(i)).filter((i) => i.low),
        expired: bs.filter((b) => b.state === "expired"),
        expiring: bs.filter((b) => b.state === "expiring"),
        lab_delayed: labs.filter((c) => c.delayed),
      };
    }
    case "stock.receive": {
      const b: Batch = { id: uid("b"), item_id: p.item_id, location_id: p.location_id, lot_no: p.lot_no, expiry_date: p.expiry_date, qty_left: p.qty, unit_cost: p.unit_cost, received_on: p.date ?? day(0), supplier_id: p.supplier_id };
      batches.push(b);
      move(b, "receive", p.qty, p.reason);
      return itemInfo(items.find((i) => i.id === p.item_id)!);
    }
    case "stock.consume": {
      let left = p.qty;
      for (const b of batches.filter((x) => x.item_id === p.item_id && x.qty_left > 0 && state(x.expiry_date) !== "expired" && (!p.location_id || x.location_id === p.location_id)).sort((a, c) => (a.expiry_date ?? "9999").localeCompare(c.expiry_date ?? "9999"))) {
        const take = Math.min(left, b.qty_left);
        if (take <= 0) continue;
        b.qty_left -= take;
        left -= take;
        move(b, "consume", -take, p.reason);
      }
      if (left > 0) throw new Fail("stock_insufficient", "qty");
      return itemInfo(items.find((i) => i.id === p.item_id)!);
    }
    case "stock.adjust": {
      const b = batches.find((x) => x.id === p.batch_id)!;
      if (b.qty_left + p.delta < 0) throw new Fail("stock_insufficient", "qty");
      b.qty_left += p.delta;
      move(b, "adjust", p.delta, p.reason);
      return itemInfo(items.find((i) => i.id === b.item_id)!);
    }
    case "stock.transfer": {
      const b = batches.find((x) => x.id === p.batch_id)!;
      if (b.qty_left < p.qty) throw new Fail("stock_insufficient", "qty");
      b.qty_left -= p.qty;
      move(b, "transfer_out", -p.qty, null);
      const t: Batch = { ...b, id: uid("b"), location_id: p.to_location_id, qty_left: p.qty };
      batches.push(t);
      move(t, "transfer_in", p.qty, null);
      return itemInfo(items.find((i) => i.id === b.item_id)!);
    }
    case "stock.return_to_supplier": {
      const b = batches.find((x) => x.id === p.batch_id)!;
      if (b.qty_left < p.qty) throw new Fail("stock_insufficient", "qty");
      b.qty_left -= p.qty;
      move(b, "return", -p.qty, p.reason);
      const s = suppliers.find((x) => x.id === p.supplier_id)!;
      s.returns_total += p.credit;
      recompute();
      s.balance -= p.credit;
      return itemInfo(items.find((i) => i.id === b.item_id)!);
    }
    case "service_materials.list":
      return (materials.get(p.service_id) ?? []).map((m) => {
        const it = items.find((i) => i.id === m.item_id)!;
        return { item_id: m.item_id, item_name: it.name, unit: units.find((u) => u.id === it.unit_id)!.name, qty: m.qty, usable_qty: itemInfo(it).usable_qty };
      });
    case "service_materials.save":
      materials.set(p.service_id, p.materials);
      return answer("service_materials.list", { service_id: p.service_id });
    case "suppliers.list":
      recompute();
      return suppliers.filter((s) => (!p.kind || s.kind === p.kind) && (p.include_inactive || s.is_active) && (!p.owing_only || s.balance > 0));
    case "suppliers.get":
      recompute();
      return suppliers.find((s) => s.id === p.id)!;
    case "suppliers.save": {
      if (!String(p.name).trim()) throw new Fail("required", "name");
      if (p.id) {
        const s = suppliers.find((x) => x.id === p.id)!;
        Object.assign(s, { name: p.name, contact_person: p.contact_person, phone: p.phone, address: p.address, note: p.note, is_active: p.is_active, version: s.version + 1 });
        return s;
      }
      const s = supplier(uid("su"), p.kind, p.name, p.contact_person, p.phone);
      suppliers.push(s);
      return s;
    }
    case "supplier_ledger.list": {
      const rows: (LedgerEntry & { key: string })[] = [];
      for (const x of purchases.filter((q) => q.supplier_id === p.supplier_id && !q.void)) rows.push({ kind: "purchase", local_date: x.local_date, number: x.number, description: "", amount: x.total, balance: 0, key: x.local_date });
      for (const x of labs.filter((q) => q.lab_id === p.supplier_id && (q.status === "received" || q.status === "delivered"))) rows.push({ kind: "lab", local_date: x.received_date ?? day(0), number: x.number, description: x.work_type, amount: x.cost, balance: 0, key: x.received_date ?? "" });
      for (const x of payments.filter((q) => q.supplier_id === p.supplier_id && !q.void)) rows.push({ kind: "payment", local_date: x.local_date, number: x.number, description: "", amount: -x.amount, balance: 0, key: x.local_date });
      rows.sort((a, b) => a.key.localeCompare(b.key));
      let bal = 0;
      return rows.map(({ key: _k, ...r }) => ({ ...r, balance: (bal += r.amount) }));
    }
    case "purchases.list":
      return purchases.filter((q) => !p.supplier_id || q.supplier_id === p.supplier_id).sort((a, b) => b.local_date.localeCompare(a.local_date));
    case "purchases.get":
      return purchases.find((q) => q.id === p.id)!;
    case "purchases.create": {
      if (!p.lines.length) throw new Fail("purchase_empty", "lines");
      addPurchase(p.supplier_id, p.date ?? day(0), p.lines.map((l: any) => ({ item: l.item_id, qty: l.qty, cost: l.unit_cost, lot: l.lot_no, expiry: l.expiry_date })), p.paid ?? 0);
      for (const l of p.lines) batches.push({ id: uid("b"), item_id: l.item_id, location_id: l.location_id, lot_no: l.lot_no, expiry_date: l.expiry_date, qty_left: l.qty, unit_cost: l.unit_cost, received_on: p.date ?? day(0), supplier_id: p.supplier_id });
      recompute();
      return purchases[purchases.length - 1];
    }
    case "purchases.void": {
      const q = purchases.find((x) => x.id === p.id)!;
      q.void = true;
      q.void_reason = p.reason;
      q.version++;
      recompute();
      return q;
    }
    case "supplier_payments.list":
      return payments.filter((q) => !p.supplier_id || q.supplier_id === p.supplier_id).sort((a, b) => b.local_date.localeCompare(a.local_date));
    case "supplier_payments.add": {
      const s = suppliers.find((x) => x.id === p.supplier_id)!;
      const m = methods.find((x) => x.id === p.method_id)!;
      const pay: SupplierPaymentInfo = { id: uid("sp"), number: `SP-1405-${String(payments.length + 1).padStart(6, "0")}`, supplier_id: s.id, supplier_name: s.name, amount: p.amount, method_id: m.id, method: m.name, local_date: p.date ?? day(0), note: p.note, void: false, void_reason: null, created_by_name: "مالک", version: 1 };
      payments.push(pay);
      recompute();
      return pay;
    }
    case "supplier_payments.void": {
      const q = payments.find((x) => x.id === p.id)!;
      q.void = true;
      q.void_reason = p.reason;
      q.version++;
      recompute();
      return q;
    }
    case "lab_cases.list":
      return labs.filter((c) => (!p.status || c.status === p.status) && (!p.lab_id || c.lab_id === p.lab_id) && (!p.patient_id || c.patient_id === p.patient_id) && (!p.delayed_only || c.delayed));
    case "lab_cases.get":
      return labs.find((c) => c.id === p.id)!;
    case "lab_cases.create": {
      const lab = suppliers.find((s) => s.id === p.lab_id)!;
      addLab({ status: "planned", lab_id: lab.id, lab_name: lab.name, patient_id: p.patient_id, patient_name: "بیمار آزمایشی", patient_number: "P-000009", doctor_name: null, teeth: p.teeth, work_type: p.work_type, shade: p.shade, expected_date: p.expected_date, cost: p.cost, note: p.note });
      return labs[labs.length - 1];
    }
    case "lab_cases.update": {
      const c = labs.find((x) => x.id === p.id)!;
      const lab = suppliers.find((s) => s.id === p.lab_id)!;
      Object.assign(c, { lab_id: lab.id, lab_name: lab.name, teeth: p.teeth, work_type: p.work_type, shade: p.shade, expected_date: p.expected_date, cost: p.cost, note: p.note, version: c.version + 1 });
      return c;
    }
    case "lab_cases.set_status": {
      const c = labs.find((x) => x.id === p.id)!;
      if (p.status === "cancelled" && !String(p.reason ?? "").trim()) throw new Fail("required", "reason");
      c.status = p.status;
      if (p.status === "sent") c.sent_date = p.date;
      if (p.status === "received") {
        c.received_date = p.date;
        if (p.cost !== null) c.cost = p.cost;
      }
      if (p.status === "delivered") c.delivered_date = p.date;
      if (p.status === "cancelled") c.cancel_reason = p.reason;
      c.delayed = c.status === "sent" && !!c.expected_date && c.expected_date < day(0);
      c.version++;
      recompute();
      return c;
    }
    case "lab_cases.report": {
      const rows = labs.filter((c) => (c.status === "received" || c.status === "delivered") && c.received_date && c.received_date >= p.from && c.received_date <= p.to);
      return {
        rows: rows.map((c) => ({ case_id: c.id, number: c.number, patient_name: c.patient_name, lab_name: c.lab_name, work_type: c.work_type, teeth: c.teeth, received_date: c.received_date, cost: c.cost, billed: c.cost * 3 })),
        total_cost: rows.reduce((a, c) => a + c.cost, 0),
        total_billed: rows.reduce((a, c) => a + c.cost * 3, 0),
      };
    }
  }
  return undefined;
}
