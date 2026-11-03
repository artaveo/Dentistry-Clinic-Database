import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// v0.7.0 (Phase 6: invoices, payments, receipts, debts, installments, expenses, cash closing, commission)
// against the real Core. Runs after the earlier files in the same clinic; it only logs in.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };
const RPC = "http://127.0.0.1:8787/rpc";
let token = "";
let patientId = "";

async function call(request: APIRequestContext, method: string, params: object) {
  const r = await request.post(RPC, { data: { method, params, token } });
  const body = await r.json();
  if (body.status !== "ok") throw new Error(`${method}: ${JSON.stringify(body.error)}`);
  return body.result;
}

test.beforeAll(async ({ browser, request }) => {
  page = await newClinicPage(browser, { width: 1360, height: 900 });
  page.on("dialog", (d) => d.accept());
  await page.goto("/");
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
  token = (await (await request.post(RPC, { data: { method: "auth.login", params: { username: OWNER.user, password: OWNER.pass } } })).json()).result.token;
});

const toast = () => page.getByTestId("toast").last();

async function shot(name: string) {
  if (!process.env.SHOTS) return;
  await page.waitForTimeout(400);
  await page.screenshot({ path: `../docs/design/screenshots/v0.7.0/${name}.png`, style: ".toasts { display: none !important; }" });
}

test("treatment done → invoice with a discount → partial payment → receipt", async ({ request }) => {
  await page.getByTestId("tab-patients").click();
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-full-name").fill("بیمار فاکتور آزمایشی");
  await page.getByTestId("patient-phone").fill("0700900088");
  await page.getByTestId("patient-save").click();
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();
  const list = await call(request, "patients.list", { query: "بیمار فاکتور آزمایشی", status: null, limit: 5, offset: 0 });
  patientId = list.items[0].id;
  const catalog = await call(request, "catalog.get", { include_inactive: false });
  const svc = catalog.services.find((s: { code: string }) => s.code === "SRG-01");
  await call(request, "treatment_items.add", { patient_id: patientId, service_id: svc.id, teeth: [48], perform_now: true, unit_price: 350000 });

  await page.getByTestId("patient-tab-invoices").click();
  await expect(page.getByTestId("unbilled-notice")).toBeVisible();
  await page.getByTestId("invoice-new").click();
  await expect(page.getByTestId("invoice-billable")).toContainText("۸┐");
  await expect(page.getByTestId("invoice-net")).toContainText("۳٬۵۰۰");
  await page.getByTestId("invoice-discount").fill("500");
  await expect(page.getByTestId("invoice-net")).toContainText("۳٬۰۰۰");
  await shot("01-invoice-new");
  await page.getByTestId("invoice-save").click();
  await expect(page.getByTestId("invoice-detail")).toBeVisible();
  await expect(page.getByTestId("invoice-detail-remaining")).toContainText("۳٬۰۰۰");

  // 1 000 AFN now, by cash.
  await page.getByTestId("invoice-pay").click();
  await expect(page.getByTestId("payment-amount")).toHaveValue("3000");
  await page.getByTestId("payment-amount").fill("1000");
  await expect(page.getByTestId("payment-after")).toContainText("۲٬۰۰۰");
  await page.getByTestId("payment-save").click();
  await expect(page.getByTestId("receipt-preview")).toBeVisible();
  await expect(page.getByTestId("receipt-preview-paper")).toContainText("RC-");
  // First receipt on this computer: it asks how receipts are printed here (6.12).
  await expect(page.getByTestId("receipt-setup")).toBeVisible();
  await page.getByTestId("receipt-mode-thermal80").click();
  await page.getByTestId("receipt-setup-done").click();
  await expect(page.getByTestId("receipt-size")).toHaveText("حرارتی ۸۰ میلی‌متر");
  await shot("02-receipt");
  await expect(page.locator('[data-testid^="receipt-RC-"]').first()).toHaveAttribute("data-width", "80");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("invoice-detail-remaining")).toContainText("۲٬۰۰۰");
  await expect(page.getByTestId("invoice-detail-paid")).toContainText("۱٬۰۰۰");
});

test("installments, advance credit, refund with a reason, credit note", async () => {
  // The invoice detail is still open behind the receipt.
  await page.getByTestId("invoice-installments").click();
  await page.getByTestId("installments-count").selectOption("2");
  await page.getByTestId("installments-split").click();
  await expect(page.getByTestId("installments-sum")).toContainText("برابر");
  await page.getByTestId("installments-save").click();
  await expect(page.getByTestId("invoice-detail-installments")).toBeVisible();

  // A refund needs its reason.
  const row = page.locator('[data-testid^="payment-row-RC-"]').first();
  await row.getByRole("button", { name: "بازپرداخت" }).click();
  await page.getByTestId("reason-amount").fill("200");
  await page.getByTestId("reason-confirm").click();
  await expect(page.getByTestId("field-error").first()).toBeVisible();
  await page.getByTestId("reason-text").fill("اشتباه در مبلغ");
  await page.getByTestId("reason-confirm").click();
  await expect(page.getByTestId("receipt-preview")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("invoice-detail-remaining")).toContainText("۲٬۲۰۰");

  // A credit note lowers the invoice after issue.
  await page.getByTestId("invoice-credit-note").click();
  await page.getByTestId("reason-amount").fill("200");
  await page.getByTestId("reason-text").fill("تخفیف بعدی");
  await page.getByTestId("reason-confirm").click();
  await expect(page.getByTestId("invoice-detail-net")).toContainText("۲٬۸۰۰");
  await shot("03-invoice-detail");
  await page.keyboard.press("Escape");

  // The account strip: total · paid · discount · refunded · remaining.
  await expect(page.getByTestId("acct-total")).toContainText("۳٬۵۰۰");
  await expect(page.getByTestId("acct-refunded")).toContainText("۲۰۰");
  await expect(page.getByTestId("acct-remaining")).toContainText("۲٬۰۰۰");
  await shot("04-account");
});

test("debtors, installments due, cash desk closing with a reason, expenses", async () => {
  await page.getByTestId("tab-billing").click();
  await page.getByTestId("billing-tab-debtors").click();
  await expect(page.getByTestId("debtors-table")).toContainText("بیمار فاکتور آزمایشی");
  await shot("05-debtors");
  await page.getByTestId("billing-tab-installments").click();
  await page.getByTestId("inst-window-30").click();
  await expect(page.getByTestId("installments-due-table")).toContainText("بیمار فاکتور آزمایشی");

  await page.getByTestId("billing-tab-expenses").click();
  await page.getByTestId("expense-add-open").click();
  await page.getByTestId("expense-amount").fill("100");
  await page.getByTestId("expense-description").fill("ماسک و دستکش");
  await page.getByTestId("expense-save").click();
  await expect(toast()).toHaveText("هزینه ثبت شد.");
  await expect(page.getByTestId("profit-expenses")).toContainText("۱۰۰");
  await shot("06-expenses");

  await page.getByTestId("billing-tab-cash").click();
  await expect(page.getByTestId("cash-expected")).toContainText("۷۰۰");
  await page.getByTestId("cash-close-open").click();
  await page.getByTestId("cash-counted").fill("650");
  await expect(page.getByTestId("cash-diff")).toContainText("۵۰");
  await page.getByTestId("cash-close-confirm").click();
  await expect(page.getByTestId("field-error").first()).toBeVisible();
  await page.getByTestId("cash-reason").fill("خرد نشد");
  await page.getByTestId("cash-close-confirm").click();
  await expect(page.getByTestId("cash-closed")).toBeVisible();
  await shot("07-cash");
});

test("commission settings and settlement", async () => {
  await page.getByTestId("billing-tab-settings").click();
  await page.getByTestId("commission-enabled").click({ force: true });
  await expect(page.getByTestId("commission-enabled")).toBeChecked();
  await page.getByTestId("commission-rule-add").click();
  await page.getByTestId("rule-value").fill("20");
  await page.getByTestId("rule-save").click();
  await expect(page.getByTestId("commission-rules")).toContainText("۲۰");
  await shot("08-settings");
  await page.getByTestId("billing-tab-commission").click();
  await expect(page.getByTestId("billing-commission")).toBeVisible();
});
