import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// v0.9.0 (Phase 8: inventory, suppliers, dental lab) against the real Core. The rules themselves
// (first-expiry-first-out, balances, cash expense of a payment, lab statuses) are tested in Rust
// (`core/src/tests.rs`); here the screens are checked: stock and its alerts, taking stock out, a
// supplier's balance, and a lab case from "sent" to "back from the lab" with its late warning.
// Runs after the earlier files in the same clinic; it only logs in.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };
const RPC = "http://127.0.0.1:8787/rpc";
let token = "";

async function call(request: APIRequestContext, method: string, params: object, as: string | null = token) {
  const r = await request.post(RPC, { data: { method, params, token: as } });
  const body = await r.json();
  if (body.status !== "ok") throw new Error(`${method}: ${JSON.stringify(body.error)}`);
  return body.result;
}

const day = (n: number) => new Date(Date.now() + n * 86_400_000).toISOString().slice(0, 10);

test.beforeAll(async ({ browser, request }) => {
  page = await newClinicPage(browser, { width: 1360, height: 900 });
  page.on("dialog", (d) => d.accept());
  await page.goto("/");
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
  token = (await call(request, "auth.login", { username: OWNER.user, password: OWNER.pass }, null)).token;
});

test("Stock: a new item, receiving it, a low-stock flag, and taking some out", async ({ request }) => {
  await page.getByTestId("tab-inventory").click();
  await expect(page.getByTestId("page-inventory")).toBeVisible();
  await expect(page.getByTestId("stock-tab")).toBeVisible();

  // A new item through the form, with a minimum so it can run low.
  await page.getByTestId("item-add-open").click();
  await expect(page.getByTestId("item-dialog")).toBeVisible();
  await page.getByTestId("item-save").click();
  await expect(page.getByTestId("item-dialog").getByTestId("field-error").first()).toBeVisible();
  await page.getByTestId("item-name-fa").fill("سرنگ یکبارمصرف");
  await page.getByTestId("item-name-en").fill("Disposable syringe");
  await page.getByTestId("item-min").fill("20");
  await page.getByTestId("item-save").click();
  await expect(page.getByTestId("item-dialog")).toBeHidden();
  const row = page.getByTestId("stock-table").locator("tr", { hasText: "سرنگ یکبارمصرف" });
  await expect(row).toBeVisible();
  await expect(row).toContainText("تمام شده");

  // Receive 10 pieces: below the minimum of 20, so it is flagged as low.
  await page.getByTestId("receive-open").click();
  await page.getByTestId("receive-item").selectOption({ label: "سرنگ یکبارمصرف" });
  await page.getByTestId("receive-qty").fill("10");
  await page.getByTestId("receive-cost").fill("25");
  await page.getByTestId("receive-lot").fill("S-1");
  await page.getByTestId("receive-save").click();
  await expect(page.getByTestId("receive-dialog")).toBeHidden();
  await expect(row).toContainText("کمبود");
  await expect(page.getByTestId("stock-alert-low")).toContainText(/[۱-۹]/);

  // More than what is usable is refused under the field; a smaller amount works.
  await row.locator('[data-testid^="stock-consume-"]').click();
  await page.getByTestId("consume-qty").fill("30");
  await page.getByTestId("consume-save").click();
  await expect(page.getByTestId("consume-dialog").getByTestId("field-error")).toBeVisible();
  await page.getByTestId("consume-qty").fill("4");
  await page.getByTestId("consume-save").click();
  await expect(page.getByTestId("consume-dialog")).toBeHidden();
  await expect(row.locator(".qty-cell")).toContainText("۶");

  // The filters: only what is low.
  await page.getByTestId("stock-filter-low").click();
  await expect(row).toBeVisible();
  await page.getByTestId("stock-search").fill("zzz-nothing");
  await expect(page.getByTestId("empty-state")).toBeVisible();
  await page.getByTestId("stock-search").fill("");
  await page.getByTestId("stock-filter-all").click();

  // An item opens with its batch and history; the expired batch is flagged and never used.
  const items = await call(request, "stock_items.list", { search: "Disposable", category_id: null, only: null, include_inactive: false, expiring_days: null });
  const id = items[0].id as string;
  const places = await call(request, "stock_locations.list", { include_inactive: false });
  await call(request, "stock.receive", { item_id: id, qty: 500, unit_cost: 0, lot_no: "OLD", expiry_date: day(-3), location_id: places[0].id, supplier_id: null, date: null, reason: null });
  await page.getByTestId(`stock-open-${id}`).click();
  await expect(page.getByTestId("item-detail")).toBeVisible();
  await expect(page.getByTestId("item-batches")).toContainText("OLD");
  await expect(page.getByTestId("item-history")).toContainText("S-1");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId(`stock-expired-${id}`)).toBeVisible();
});

test("Suppliers: a purchase adds to stock and to what is owed", async ({ request }) => {
  const supplier = await call(request, "suppliers.save", { id: null, kind: "supplier", name: "شرکت طبی آزمایشی", contact_person: null, phone: null, address: null, note: null, is_active: true, version: null });
  const items = await call(request, "stock_items.list", { search: "Disposable", category_id: null, only: null, include_inactive: false, expiring_days: null });
  const places = await call(request, "stock_locations.list", { include_inactive: false });
  // 20.00 pieces at 30 afghani = 600 afghani owed.
  await call(request, "purchases.create", { supplier_id: supplier.id, date: null, note: null, paid: 0, method_id: null, lines: [{ item_id: items[0].id, qty: 2000, unit_cost: 3000, lot_no: "P-9", expiry_date: day(400), location_id: places[0].id }] });

  await page.getByTestId("inventory-tab-suppliers").click();
  await expect(page.getByTestId("suppliers-table")).toBeVisible();
  await expect(page.getByTestId(`supplier-balance-${supplier.id}`)).toContainText("۶۰۰");
  await page.getByTestId("inventory-tab-purchases").click();
  await expect(page.getByTestId("purchases-table")).toContainText("PU-");
  await page.getByTestId("inventory-tab-suppliers").click();
  await page.getByTestId(`supplier-open-${supplier.id}`).click();
  await expect(page.getByTestId("ledger-dialog")).toBeVisible();
  await expect(page.getByTestId("ledger-balance")).toContainText("۶۰۰");
  await expect(page.getByTestId("ledger-table")).toContainText("PU-");

  // Paying part of it lowers the balance; it is also an expense of the clinic.
  await page.getByTestId("supplier-pay-open").click();
  await page.getByTestId("supplier-pay-amount").fill("250");
  await page.getByTestId("supplier-pay-save").click();
  await expect(page.getByTestId("supplier-pay-dialog")).toBeHidden();
  await expect(page.getByTestId("ledger-balance")).toContainText("۳۵۰");
  await page.keyboard.press("Escape");
});

test("Dental lab: a late case is flagged, and coming back puts its cost on the lab's account", async ({ request }) => {
  const lab = await call(request, "suppliers.save", { id: null, kind: "lab", name: "لابراتوار آزمایشی", contact_person: null, phone: null, address: null, note: null, is_active: true, version: null });
  const patient = await call(request, "patients.create", { full_name: "بیمار لابراتوار" });
  const made = await call(request, "lab_cases.create", { patient_id: patient.id, lab_id: lab.id, doctor_id: null, treatment_item_id: null, teeth: [36], work_type: "Crown PFM", shade: "A2", expected_date: day(-2), cost: 40_000, note: null });
  await call(request, "lab_cases.set_status", { id: made.id, version: made.version, status: "sent", date: null, cost: null, reason: null });

  await page.getByTestId("inventory-tab-lab").click();
  await expect(page.getByTestId("lab-table")).toBeVisible();
  await expect(page.getByTestId(`lab-late-${made.id}`)).toBeVisible();
  await expect(page.getByTestId("lab-late-notice")).toBeVisible();
  await page.getByTestId("lab-view-late").click();
  await expect(page.getByTestId(`lab-row-${made.id}`)).toBeVisible();
  await page.getByTestId("lab-view-open").click();

  // The work comes back with its final cost.
  await page.getByTestId(`lab-next-${made.id}`).click();
  await expect(page.getByTestId("lab-status-dialog")).toBeVisible();
  await page.getByTestId("lab-status-cost").fill("450");
  await page.getByTestId("lab-status-save").click();
  await expect(page.getByTestId("lab-status-dialog")).toBeHidden();
  await expect(page.getByTestId(`lab-status-${made.id}`)).toContainText("برگشته");
  await expect(page.getByTestId(`lab-late-${made.id}`)).toBeHidden();
  const labs = await call(request, "suppliers.list", { kind: "lab", include_inactive: false, owing_only: true });
  expect(labs.find((l: { id: string }) => l.id === lab.id).balance).toBe(45_000);

  // A new case through the form needs a patient and a work type.
  await page.getByTestId("lab-add-open").click();
  await expect(page.getByTestId("lab-dialog")).toBeVisible();
  await page.getByTestId("lab-save").click();
  await expect(page.getByTestId("lab-dialog").getByTestId("field-error").first()).toBeVisible();
  await page.keyboard.press("Escape");
});

test("Stock settings: the lists and the materials of a service", async ({ request }) => {
  await page.getByTestId("inventory-tab-settings").click();
  await expect(page.getByTestId("inventory-settings")).toBeVisible();
  await expect(page.getByTestId("lookup-categories")).toBeVisible();
  await expect(page.getByTestId("lookup-units")).toBeVisible();
  await expect(page.getByTestId("lookup-locations")).toBeVisible();

  await page.getByTestId("lookup-add-locations").click();
  await page.getByTestId("lookup-name-fa").fill("کابینت دندان‌پزشکی");
  await page.getByTestId("lookup-save").click();
  await expect(page.getByTestId("lookup-locations")).toContainText("کابینت دندان‌پزشکی");

  const places = await call(request, "stock_locations.list", { include_inactive: false });
  expect(places.length).toBeGreaterThanOrEqual(3);
  await expect(page.getByTestId("materials-card")).toBeVisible();
});
