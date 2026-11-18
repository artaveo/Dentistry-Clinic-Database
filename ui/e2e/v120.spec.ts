import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// Regenerate the Recovery Key (Owner only) against the real Core. The key files and the old-key-stops-working
// rule are tested in Rust (`core/src/rekey.rs`, `core/src/tests_rekey.rs`); here the screen is checked: the
// Owner must type the password, a wrong one is refused under the field, the new key is shown once in the
// setup format and the dialog cannot be closed before "I stored it" is ticked. Runs after v110; it only logs in.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };
const RPC = "http://127.0.0.1:8787/rpc";

async function call(request: APIRequestContext, method: string, params: object, as: string | null) {
  const r = await request.post(RPC, { data: { method, params, token: as } });
  return r.json();
}

test.beforeAll(async ({ browser }) => {
  page = await newClinicPage(browser, { width: 1360, height: 900 });
  await page.goto("/");
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
});

test("The Owner makes a new Recovery Key with the password and must confirm it is stored", async ({ request }) => {
  await page.getByTestId("tab-settings").click();
  await page.getByTestId("tab-backup").click();
  await page.getByTestId("bk-tab-key").click();
  await expect(page.getByTestId("recovery-key-section")).toBeVisible();
  await page.getByTestId("rk-open").click();
  await expect(page.getByTestId("rk-dialog")).toBeVisible();

  // No password, then a wrong one: refused under the field, nothing is shown.
  await page.getByTestId("rk-confirm").click();
  await expect(page.getByTestId("rk-dialog").getByTestId("field-error")).toBeVisible();
  await page.getByTestId("rk-password").fill("not-the-password-1");
  await page.getByTestId("rk-confirm").click();
  await expect(page.getByTestId("rk-dialog").getByTestId("field-error")).toBeVisible();
  await expect(page.getByTestId("rk-key")).toHaveCount(0);

  await page.getByTestId("rk-password").fill(OWNER.pass);
  await page.getByTestId("rk-confirm").click();
  const key = (await page.getByTestId("rk-key").textContent()) ?? "";
  expect(key).toMatch(/^[A-Z2-7]{6}(-[A-Z2-7]{6}){5}$/);
  await expect(page.getByTestId("rk-result")).toBeVisible();
  await expect(page.getByTestId("rk-sheet")).toHaveCount(1);

  // Closing waits for the tick.
  await expect(page.getByTestId("rk-close")).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("rk-dialog")).toBeVisible();
  await page.getByTestId("rk-confirmed").check({ force: true });
  await page.getByTestId("rk-close").click();
  await expect(page.getByTestId("rk-dialog")).toHaveCount(0);

  // The shown key is the live one: owner-password recovery accepts it (same password, so later runs still sign in).
  const r = await call(request, "auth.recover_owner", { recovery_key: key, new_password: OWNER.pass }, null);
  expect(r.status).toBe("ok");
});
