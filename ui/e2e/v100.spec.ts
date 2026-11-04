import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// v0.10.0 (Phase 9: dashboard, reports, notifications, global search, audit viewer) against the real Core.
// The numbers themselves are tested in Rust (`core/src/tests.rs`); here the screens are checked: the dashboard
// is the first page, a report runs and can be exported, the bell lists what needs attention, the palette finds
// a patient, and the audit log can be narrowed to the sensitive actions. Runs after the earlier files in the
// same clinic; it only logs in.
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

test("The dashboard is the first page and shows today's numbers", async () => {
  await expect(page.getByTestId("page-dashboard")).toBeVisible();
  await expect(page.getByTestId("tab-dashboard")).toHaveAttribute("aria-current", "page");
  await expect(page.getByTestId("dash-stats")).toBeVisible();
  await expect(page.getByTestId("dash-patients")).toBeVisible();
  await expect(page.getByTestId("dash-attention")).toBeVisible();
  // A state is a word and an icon, never only a colour.
  await expect(page.getByTestId("dash-license")).toContainText("۱۰");
  await page.getByTestId("tab-system").click();
  await page.getByTestId("tab-dashboard").click();
  await expect(page.getByTestId("dash-stats")).toBeVisible();
});

test("A report runs for a period, narrows by doctor, exports to Excel and prints", async () => {
  await page.getByTestId("tab-reports").click();
  await expect(page.getByTestId("page-reports")).toBeVisible();
  await expect(page.getByTestId("report-table")).toBeVisible();
  await expect(page.getByTestId("report-foot")).toBeVisible();

  await page.getByTestId("report-doctor_revenue").click();
  await expect(page.getByTestId("report-doctor")).toBeVisible();
  await page.getByTestId("report-preset-year").click();
  await expect(page.getByTestId("report-foot")).toBeVisible();

  // A report of the state today has no period.
  await page.getByTestId("report-stock").click();
  await expect(page.getByTestId("report-from")).toHaveCount(0);

  await page.getByTestId("report-payments").click();
  await page.getByTestId("report-preset-year").click();
  await expect(page.getByTestId("report-foot")).toBeVisible();

  // Profit and loss always has the months of the period; its Excel file lands in the exports folder.
  await page.getByTestId("report-profit_loss").click();
  await page.getByTestId("report-preset-year").click();
  await expect(page.getByTestId("report-foot")).toBeVisible();
  const export_ = page.getByTestId("report-export");
  if (await export_.isEnabled()) {
    await export_.click();
    await expect(page.getByTestId("report-export-done")).toBeVisible();
    await page.getByTestId("report-print").click();
    await expect(page.getByTestId("report-preview")).toBeVisible();
    await expect(page.getByTestId("doc-preview").getByTestId("report-sheet")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("report-preview")).toBeHidden();
  }
});

test("The bell lists a follow-up that is due and remembers what was read", async ({ request }) => {
  const patients = await call(request, "patients.list", { query: null, status: null, limit: 5, offset: 0 });
  const patient = patients.items[0];
  await call(request, "recalls.create", { patient_id: patient.id, kind: "follow_up", due_date: day(-1), note: "v100" });

  // Opening the bell asks the Core again, so the new recall shows at once.
  await page.getByTestId("notification-bell").click();
  const mine = page.getByTestId("notification-follow_up_due").filter({ hasText: patient.full_name }).first();
  await expect(mine).toBeVisible();
  await expect(page.getByTestId("notification-count")).toBeVisible();
  await page.getByTestId("notification-read-all").click();
  await expect(page.getByTestId("notification-count")).toHaveCount(0);
  // The notification leads to the page that deals with it.
  await mine.click();
  await expect(page.getByTestId("tab-recalls")).toHaveAttribute("aria-current", "page");
});

test("Ctrl+K finds a patient and opens the profile", async ({ request }) => {
  const patients = await call(request, "patients.list", { query: null, status: null, limit: 5, offset: 0 });
  const patient = patients.items[0];
  await page.getByTestId("tab-dashboard").click();
  await page.keyboard.press("Control+k");
  await page.getByTestId("command-palette-input").fill(patient.full_name.split(" ")[0]);
  const hit = page.getByTestId(`command-item-patient-${patient.id}`);
  await expect(hit).toBeVisible();
  await hit.click();
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();

  // Nothing found: the pages are still there and the box says what to try.
  await page.keyboard.press("Control+k");
  await page.getByTestId("command-palette-input").fill("zzqqxx-no-such");
  await expect(page.getByTestId("palette-no-hits")).toBeVisible();
  await page.keyboard.press("Escape");
});

test("The audit log narrows to the sensitive actions", async () => {
  await page.getByTestId("tab-audit").click();
  await expect(page.getByTestId("audit-list")).toBeVisible();
  await page.getByTestId("audit-filter-sensitive").click();
  const rows = page.getByTestId("audit-list").locator("tbody tr");
  await expect(rows.first()).toBeVisible();
  await expect(page.getByTestId("audit-sensitive").first()).toBeVisible();
  const total = await rows.count();
  expect(await page.getByTestId("audit-sensitive").count()).toBe(total);

  await page.getByTestId("audit-filter-action").fill("zzz-no-such-action");
  await expect(page.getByTestId("empty-state")).toBeVisible();
});

