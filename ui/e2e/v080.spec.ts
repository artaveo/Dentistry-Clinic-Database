import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// v0.8.0 (Phase 7: LAN mode) against the real Core. The network transport itself (TLS, pairing, live
// push between computers) is tested in Rust (`lan/tests/lan.rs`: one server, three clients); here the
// screens are checked: the Network page, instant refresh on a pushed change, "who has this open".
// Runs after the earlier files in the same clinic; it only logs in.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };
const RECEPTION = { user: "reception", pass: "reception-123" };
const RPC = "http://127.0.0.1:8787/rpc";
let token = "";

async function call(request: APIRequestContext, method: string, params: object, as: string | null = token) {
  const r = await request.post(RPC, { data: { method, params, token: as } });
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
  token = (await call(request, "auth.login", { username: OWNER.user, password: OWNER.pass }, null)).token;
});

const live = (topic: string, method: string) =>
  page.evaluate(([t, m]) => window.dispatchEvent(new CustomEvent("artaveo:live", { detail: { topic: t, method: m } })), [topic, method]);

test("Network page: server mode, pairing code, list of computers", async ({ request }) => {
  await page.getByTestId("tab-network").click();
  await expect(page.getByTestId("page-network")).toBeVisible();
  await expect(page.getByTestId("lan-switch")).not.toBeChecked();
  await expect(page.getByTestId("lan-add")).toBeDisabled();
  await expect(page.getByTestId("lan-devices")).toBeVisible();
  await expect(page.getByTestId("empty-state")).toBeVisible();

  // The switch is the clinic's choice "this computer is the clinic server" (roadmap 7.1).
  await page.locator("label.switch", { has: page.getByTestId("lan-switch") }).click();
  await expect(page.getByTestId("lan-switch")).toBeChecked();
  await expect(page.getByTestId("status-install-mode")).toContainText("سرور");
  // This E2E server has no network transport: it says "starting" instead of "running".
  await expect(page.getByTestId("lan-status")).toBeVisible();

  // A pairing code opened for a new computer is shown large, with its time and attempts.
  await call(request, "lan.pairing_open", {});
  await expect(page.getByTestId("lan-pairing")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByTestId("lan-code")).toHaveText(/^[۰-۹]{6}$/);
  await page.getByTestId("lan-pairing-cancel").click();
  await expect(page.getByTestId("lan-pairing")).toBeHidden();

  // Server mode can be turned off again; the plan's limit is shown.
  await expect(page.getByTestId("lan-count")).toContainText("۵");
  await page.locator("label.switch", { has: page.getByTestId("lan-switch") }).click();
  await expect(page.getByTestId("lan-switch")).not.toBeChecked();
  await expect(page.getByTestId("status-install-mode")).not.toContainText("سرور");
});

test("a change made on another computer refreshes the screen at once", async () => {
  let loads = 0;
  page.on("request", (r) => {
    if (r.url().endsWith("/rpc") && (r.postData() ?? "").includes('"appointments.list"')) loads++;
  });
  await page.getByTestId("tab-queue").click();
  await expect.poll(() => loads).toBeGreaterThan(0);

  const before = loads;
  await live("appointments", "appointments.set_status");
  await expect.poll(() => loads).toBeGreaterThan(before);

  // Something the queue does not show does not reload it.
  const quiet = loads;
  await live("billing", "invoices.create");
  await page.waitForTimeout(900);
  expect(loads).toBe(quiet);
});

test("a record shows who else has it open", async ({ request }) => {
  await page.getByTestId("tab-patients").click();
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-full-name").fill("بیمار همزمان آزمایشی");
  await page.getByTestId("patient-phone").fill("0700900099");
  await page.getByTestId("patient-save").click();
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();
  await expect(page.getByTestId("presence-banner")).toBeHidden();

  const list = await call(request, "patients.list", { query: "بیمار همزمان آزمایشی", status: null, limit: 5, offset: 0 });
  const patient = { entity: "patient", id: list.items[0].id };
  const other = (await call(request, "auth.login", { username: RECEPTION.user, password: RECEPTION.pass }, null)).token;

  await call(request, "presence.enter", patient, other);
  await live("presence", "enter");
  await expect(page.getByTestId("presence-banner")).toBeVisible();

  await call(request, "presence.leave", patient, other);
  await live("presence", "leave");
  await expect(page.getByTestId("presence-banner")).toBeHidden();
});
