import fs from "node:fs";
import path from "node:path";
import { expect, test, type Page } from "@playwright/test";

// v0.4.1 (owner feedback OF-015, OF-019 … OF-043) against the real Core. Runs after the earlier
// files in the same clinic and session (single worker, one dev server), so it only logs in.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };
// The Core's data folder is `.e2e-data` (playwright.config.ts); exports land in its `exports` folder.
const EXPORTS = path.resolve(".e2e-data/exports");
const FIXTURES = path.resolve("e2e/fixtures");

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto("/");
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
});

const toast = () => page.getByTestId("toast").last();

test("OF-015: the lock screen blurs what is behind it and hides it from screen readers", async () => {
  await page.getByTestId("current-user").click();
  await page.getByTestId("lock").click();
  await expect(page.getByTestId("lock-screen")).toBeVisible();
  await expect(page.locator(".app-main")).toHaveCSS("filter", /blur/);
  await expect(page.locator(".app-main")).toHaveAttribute("inert", "");
  await page.getByTestId("unlock-password").fill(OWNER.pass);
  await page.getByTestId("unlock").click();
  await expect(page.getByTestId("lock-screen")).toHaveCount(0);
  await expect(page.locator(".app-main")).toHaveCSS("filter", "none");
});

test("OF-027: at the smallest window (800x600) nothing sticks out sideways", async () => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.getByTestId("tab-patients").click();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  expect(overflow).toBeLessThanOrEqual(0);
  await page.setViewportSize({ width: 1280, height: 800 });
});

test("OF-021: the patient number column is called ID", async () => {
  await page.getByTestId("tab-patients").click();
  await expect(page.getByTestId("patient-list").locator("thead")).toContainText("ID بیمار");
});

test("OF-019: an age and a birth date are never typed twice", async () => {
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-full-name").fill("سن تقریبی");
  await expect(page.getByTestId("patient-dob")).toBeDisabled();
  await page.getByTestId("patient-age").fill("28");
  await page.getByTestId("patient-exact-birth").check();
  await expect(page.getByTestId("patient-age")).toBeDisabled();
  await page.getByTestId("patient-dob").fill("1990-05-20");
  const now = new Date();
  let age = now.getFullYear() - 1990;
  if (now < new Date(now.getFullYear(), 4, 20)) age -= 1;
  await expect(page.getByTestId("patient-age")).toHaveValue(String(age));
  await page.getByTestId("patient-save").click();
  await expect(toast()).toHaveText("بیمار ثبت شد.");
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();
});

test("OF-023: the back arrow and Alt+Left both return to the patient list", async () => {
  await page.getByTestId("patient-back").click();
  await expect(page.getByTestId("patient-list")).toBeVisible();
  await page.getByTestId("patients-search").fill("سن تقریبی");
  await page.getByTestId("patient-list").locator("tbody tr").first().click();
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();
  await page.keyboard.press("Alt+ArrowLeft");
  await expect(page.getByTestId("patient-list")).toBeVisible();
  await page.getByTestId("patients-search").fill("");
});

test("OF-029: a failed save moves the cursor to the first wrong box and counts the problems", async () => {
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-save").click();
  await expect(page.getByTestId("form-error-summary")).toBeVisible();
  await expect(page.getByTestId("patient-full-name")).toBeFocused();
  await page.getByTestId("patient-cancel").click();
});

test("OF-020: an unfinished patient form comes back after leaving the page", async () => {
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-full-name").fill("پیش نویس ناتمام");
  await page.getByTestId("tab-appointments").click();
  await page.getByTestId("tab-patients").click();
  await page.getByTestId("add-patient-open").click();
  await expect(page.getByTestId("draft-offer")).toBeVisible();
  await page.getByTestId("draft-restore").click();
  await expect(page.getByTestId("patient-full-name")).toHaveValue("پیش نویس ناتمام");
  // Saving the patient also clears the draft.
  await page.getByTestId("patient-save").click();
  await expect(toast()).toHaveText("بیمار ثبت شد.");
});

test("OF-022 / OF-043: the export is a real Excel file, reported, and reads back with Persian text", async () => {
  await page.getByTestId("tab-patients").click();
  await page.getByTestId("patients-export").click();
  await expect(page.getByTestId("export-done")).toBeVisible();
  await expect(page.getByTestId("export-done")).toContainText(".xlsx");
  const files = fs.readdirSync(EXPORTS).filter((f) => f.endsWith(".xlsx")).sort();
  const newest = files[files.length - 1];
  expect(newest).toBeTruthy();
  expect(fs.readFileSync(path.join(EXPORTS, newest)).subarray(0, 2).toString()).toBe("PK");

  await page.getByTestId("patients-import-open").click();
  await page.getByTestId("import-file-input").setInputFiles(path.join(EXPORTS, newest));
  await expect(page.getByTestId("import-mapping")).toContainText("نام و تخلص");
  await expect(page.getByTestId("import-mapping")).toContainText("احمد خان رحیمی");
  await page.keyboard.press("Escape");
});

test("OF-042: a sheet without a title row is read as data, and the import says why it cannot run", async () => {
  await page.getByTestId("patients-import-open").click();
  await page.getByTestId("import-file-input").setInputFiles(path.join(FIXTURES, "no-titles.xlsx"));
  await expect(page.getByTestId("import-has-header")).not.toBeChecked();
  await expect(page.getByTestId("import-mapping")).toContainText("ستون 1");
  await expect(page.getByTestId("import-commit-blocker")).toBeVisible();
  await expect(page.getByTestId("import-commit")).toBeDisabled();

  await page.getByTestId("import-map-0").selectOption("full_name");
  await expect(page.getByTestId("import-summary")).toContainText("2");
  await expect(page.getByTestId("import-commit")).toBeEnabled();

  // Saying the first row is a title row shifts the data down by one row.
  await page.getByTestId("import-has-header").check();
  await expect(page.getByTestId("import-mapping")).toContainText("ALI");
  await page.keyboard.press("Escape");
});

test("OF-042: a sheet with Dari titles maps itself and imports the valid rows", async () => {
  await page.getByTestId("patients-import-open").click();
  await page.getByTestId("import-file-input").setInputFiles(path.join(FIXTURES, "dari-titles.xlsx"));
  await expect(page.getByTestId("import-has-header")).toBeChecked();
  await expect(page.getByTestId("import-mapping")).toContainText("نام و تخلص");
  await expect(page.getByTestId("import-commit")).toBeEnabled();
  await page.getByTestId("import-commit").click();
  await expect(toast()).toContainText("بیمار با موفقیت وارد شد");
});

test("OF-032: the week shows all seven days without sideways scrolling", async () => {
  await page.getByTestId("tab-appointments").click();
  await page.getByTestId("cal-view-week").click();
  await expect(page.locator(".tg-colhead")).toHaveCount(7);
  const fits = await page.getByTestId("cal-grid-week").evaluate((el) => el.scrollWidth <= el.clientWidth + 1);
  expect(fits).toBe(true);
});

test("OF-038: the date in the calendar title opens a calendar that jumps to any day", async () => {
  await page.getByTestId("cal-view-day").click();
  await page.getByTestId("cal-title").click();
  await expect(page.getByTestId("cal-date-picker")).toBeVisible();
  await page.locator("[data-testid=cal-date-picker] .mg-cell").nth(10).click();
  await expect(page.getByTestId("cal-date-picker")).toHaveCount(0);
});
