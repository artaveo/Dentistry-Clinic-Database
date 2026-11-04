import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// v0.11.0 (Phase 10: backup manager, trusted USB, cloud folder, restore, license) against the real Core.
// The rules themselves (retention, USB identity, pending transfer, restore, read-only) are tested in Rust
// (`core/src/tests_phase10.rs`); here the screens are checked: the backup page answers "is my data safe,
// where", the plan can be changed, a restore asks for the password and signs everybody out, and the license
// page tells the state in words. Runs last: the restore replaces the data with a backup made a moment before.
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

async function signIn(request: APIRequestContext) {
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
  token = (await call(request, "auth.login", { username: OWNER.user, password: OWNER.pass }, null)).token;
}

test.beforeAll(async ({ browser, request }) => {
  page = await newClinicPage(browser, { width: 1360, height: 900 });
  page.on("dialog", (d) => d.accept());
  await page.goto("/");
  await signIn(request);
});

test("The dashboard license row leads to the License page, which says the state in words", async () => {
  await expect(page.getByTestId("dash-license")).toContainText("فعال نشده");
  await page.getByTestId("dash-license").getByRole("button").click();
  await expect(page.getByTestId("page-license")).toBeVisible();
  await expect(page.getByTestId("lic-headline")).toContainText("فعال نشده");
  await expect(page.getByTestId("lic-state")).toBeVisible();
  await expect(page.getByTestId("lic-installation")).not.toBeEmpty();
  // This build has no license server address: online activation is off, offline activation is offered.
  await expect(page.getByTestId("lic-offline-build")).toBeVisible();
  await expect(page.getByTestId("lic-activate")).toBeDisabled();
  await expect(page.getByTestId("lic-trial")).toBeDisabled();
});

test("Offline activation shows a request code and refuses a wrong response", async () => {
  await page.getByTestId("lic-offline-open").click();
  const dialog = page.getByTestId("lic-offline-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.getByTestId("lic-offline-request")).not.toHaveValue("");
  await dialog.getByTestId("lic-offline-response").fill("this is not a license");
  await dialog.getByTestId("lic-offline-apply").click();
  await expect(dialog.getByTestId("field-error")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

test("The backup page shows where the data is and makes a backup", async ({ request }) => {
  await page.getByTestId("tab-backup").click();
  await expect(page.getByTestId("page-backup")).toBeVisible();
  await expect(page.getByTestId("backup-destinations")).toBeVisible();
  await expect(page.getByTestId("dest-usb")).toContainText("تنظیم نشده");
  await expect(page.getByTestId("dest-cloud")).toContainText("تنظیم نشده");
  await expect(page.getByTestId("backup-next")).toBeVisible();

  const before = await page.getByTestId("backup-list").locator("tbody tr").count();
  await page.getByTestId("backup-now").click();
  await expect(page.getByTestId("backup-list").locator("tbody tr")).toHaveCount(before + 1);
  await expect(page.getByTestId("dest-local")).toContainText("سالم");
  // No drive in the CI machine: the USB tab says so plainly.
  await page.getByTestId("bk-tab-usb").click();
  await expect(page.getByTestId("usb-section")).toBeVisible();
  await expect(page.getByTestId("usb-section")).toContainText("فلشی وصل نیست");
  void request;
});

test("The plan changes live: a wrong cloud folder is refused under the field, retention is saved", async () => {
  await page.getByTestId("bk-tab-cloud").click();
  await page.getByTestId("cloud-folder").fill("relative/folder");
  await page.getByTestId("cloud-save").click();
  await expect(page.getByTestId("cloud-section").getByTestId("field-error")).toBeVisible();
  await page.getByTestId("cloud-folder").fill("");

  await page.getByTestId("bk-tab-schedule").click();
  await page.getByTestId("keep-daily").fill("20");
  await page.getByTestId("schedule-save").click();
  await page.getByTestId("tab-system").click();
  await page.getByTestId("tab-backup").click();
  await page.getByTestId("bk-tab-schedule").click();
  await expect(page.getByTestId("keep-daily")).toHaveValue("۲۰");
  // A value out of range is refused before it is sent.
  await page.getByTestId("keep-daily").fill("0");
  await page.getByTestId("schedule-save").click();
  await expect(page.getByTestId("schedule-section").getByTestId("field-error")).toBeVisible();
  await page.getByTestId("keep-daily").fill("14");
  await page.getByTestId("schedule-save").click();
});

test("Restore compares, asks for the password and signs everybody out", async ({ request }) => {
  await call(request, "backup.create", {});
  await call(request, "patients.create", { full_name: "E2E Restore Later" });

  await page.getByTestId("tab-backup").click();
  await page.getByTestId("bk-tab-restore").click();
  await expect(page.getByTestId("restore-list")).toBeVisible();
  await page.getByTestId("restore-item").first().click();
  await expect(page.getByTestId("restore-preview")).toBeVisible();
  await expect(page.getByTestId("restore-warning")).toBeVisible();
  // Not possible without the password and the tick.
  await expect(page.getByTestId("restore-go")).toBeDisabled();
  await page.getByTestId("restore-password").fill("wrong-password-1");
  await page.getByTestId("restore-sure").check({ force: true });
  await page.getByTestId("restore-go").click();
  await expect(page.getByTestId("restore-preview").getByTestId("field-error")).toBeVisible();

  await page.getByTestId("restore-password").fill(OWNER.pass);
  await page.getByTestId("restore-go").click();
  await expect(page.getByTestId("login-username")).toBeVisible();

  await signIn(request);
  const found = await call(request, "patients.list", { query: "E2E Restore Later", limit: 10, offset: 0 });
  expect(found.items.length).toBe(0);
  // The safety copy of what was replaced is in the list.
  await page.getByTestId("tab-backup").click();
  await expect(page.getByTestId("backup-list")).toContainText("پیش از بازیابی");
});
