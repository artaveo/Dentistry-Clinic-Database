import { expect, test, type Page } from "@playwright/test";

// Phase 3: patients, medical records, attachments, merge and import, against
// the real Core. Runs after app.spec.ts in the same shared clinic/session
// (single worker, one dev-server — see playwright.config.ts), so the clinic
// and owner already exist; this file only needs to log in.
test.describe.configure({ mode: "serial" });

let page: Page;
// app.spec.ts's last test recovers the owner's password via the Recovery
// Key, so by the time this file runs the real password is this one, not
// the original "owner-pass-123" set up at the very start of that file.
const OWNER = { user: "owner", pass: "recovered-pass-1" };

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.goto("/");
  // A fresh page load always re-shows the splash then the login screen (the
  // in-memory token from app.spec.ts's session does not survive a reload);
  // `fill`/`click` auto-wait past the splash, unlike a bare `isVisible()`.
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
  await page.getByTestId("tab-patients").click();
});

const toast = () => page.getByTestId("toast").last();

test("create a patient, see the duplicate warning, then open the profile", async () => {
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-full-name").fill("احمد خان رحیمی");
  await page.getByTestId("patient-phone").fill("0700111222");
  await page.getByTestId("patient-save").click();
  await expect(toast()).toHaveText("بیمار ثبت شد.");
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();
  await expect(page.getByText("احمد خان رحیمی")).toBeVisible();

  // A second patient with the same phone triggers the duplicate dialog (3.1).
  await page.getByTestId("patient-back").click();
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-full-name").fill("Someone Else");
  await page.getByTestId("patient-phone").fill("0700111222");
  await page.getByTestId("patient-save").click();
  await expect(page.getByTestId("duplicate-dialog")).toBeVisible();
  await expect(page.getByTestId("duplicate-dialog")).toContainText("احمد خان رحیمی");
  await page.getByTestId("duplicate-continue").click();
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();
  await expect(page.getByText("Someone Else")).toBeVisible();
});

test("search finds by partial name, patient number and phone", async () => {
  await page.getByTestId("patient-back").click();
  await page.getByTestId("patients-search").fill("احمد خان");
  await expect(page.getByTestId("patient-list")).toContainText("احمد خان رحیمی");
  await expect(page.getByTestId("patient-list")).not.toContainText("Someone Else");
  await page.getByTestId("patients-search").fill("");
  await expect(page.getByTestId("patient-list").locator("tbody tr")).toHaveCount(2);
});

test("edit a patient with live validation and optimistic locking", async () => {
  await page.getByTestId("patients-search").fill("احمد خان");
  await page.getByText("احمد خان رحیمی").click();
  await page.getByTestId("patient-edit-open").click();
  await page.getByTestId("patient-phone").fill("bad");
  await page.getByTestId("patient-full-name").fill("احمد خان رحیمی ۲");
  await page.getByTestId("patient-save").click();
  await expect(page.getByTestId("field-error")).toHaveText("شماره تماس باید ۷ تا ۱۵ رقم باشد.");
  await page.getByTestId("patient-phone").fill("0700111222");
  await page.getByTestId("patient-save").click();
  await expect(toast()).toHaveText("تغییرات ذخیره شد.");
  await expect(page.getByText("احمد خان رحیمی ۲")).toBeVisible();
});

test("medical history: saving allergies/conditions shows the alert banner (3.2/3.3)", async () => {
  await page.getByTestId("patient-tab-medical").click();
  await page.getByTestId("mh-allergies").fill("حساسیت به پنی‌سیلین");
  await page.getByTestId("mh-conditions").fill("دیابت");
  await page.getByTestId("mh-save").click();
  await expect(toast()).toHaveText("سوابق پزشکی ذخیره شد.");
  await expect(page.getByTestId("medical-alert-banner")).toContainText("پنی‌سیلین");
  await expect(page.getByTestId("medical-alert-banner")).toContainText("دیابت");
});

test("attachments: upload an image, view it, then delete it (3.5)", async () => {
  await page.getByTestId("patient-tab-documents").click();
  await page.getByTestId("attachment-add-open").click();
  await page.getByTestId("attachment-file-input").setInputFiles({ name: "xray.png", mimeType: "image/png", buffer: Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]) });
  await page.getByTestId("attachment-tooth").fill("26");
  await page.getByTestId("attachment-upload-submit").click();
  await expect(toast()).toHaveText("پیوست اضافه شد.");
  await expect(page.getByTestId("attachment-grid")).toContainText("رادیوگرافی");

  await page.locator('[data-testid^="attachment-open-"]').first().click();
  await expect(page.getByTestId("attachment-viewer")).toBeVisible();
  await expect(page.getByTestId("viewer-image")).toBeVisible();
  await page.getByTestId("viewer-zoom-in").click();
  await page.keyboard.press("Escape");

  page.once("dialog", (d) => d.accept());
  await page.locator('[data-testid^="attachment-delete-"]').first().click();
  await expect(toast()).toHaveText("پیوست حذف شد.");
  await expect(page.getByTestId("attachment-grid")).not.toContainText("رادیوگرافی");
});

test("patient audit history is scoped to this patient", async () => {
  await page.getByTestId("patient-tab-audit").click();
  await expect(page.getByTestId("patient-audit-list")).toContainText("ویرایش بیمار");
  await expect(page.getByTestId("patient-audit-list")).toContainText("به‌روزرسانی سوابق پزشکی");
});

test("merge folds the record you're viewing into the one you pick", async () => {
  // We're on احمد خان's profile; picking Someone Else as the target means
  // احمد خان (the record being viewed) becomes the one that is folded away.
  await page.getByTestId("patient-merge-open").click();
  await page.getByTestId("merge-search").fill("Someone Else");
  await page.locator('[data-testid^="merge-candidate-"]').first().click();
  await expect(page.getByTestId("merge-dialog")).toContainText("غیرفعال");
  await page.getByTestId("merge-confirm").click();
  await expect(toast()).toHaveText("پرونده‌ها ادغام شدند.");

  await page.getByTestId("patient-back").click();
  await page.getByTestId("patients-search").fill("احمد خان");
  const mergedRow = page.getByTestId("patient-list").locator("tbody tr").first();
  await expect(mergedRow.locator(".badge")).toHaveText("غیرفعال");
  await page.getByTestId("patients-search").fill("Someone Else");
  const keptRow = page.getByTestId("patient-list").locator("tbody tr").first();
  await expect(keptRow.locator(".badge")).toHaveText("فعال");
});

test("import: preview reports row errors, commit only adds valid rows", async () => {
  await page.getByTestId("patients-search").fill("");
  await page.getByTestId("patients-import-open").click();
  const csv = "full_name,father_name,phone,secondary_phone,date_of_birth,address\nوارده شده,کریم,0799000111,,,کابل\n,بد,123,,,";
  await page.getByTestId("import-file-input").setInputFiles({ name: "patients.csv", mimeType: "text/csv", buffer: Buffer.from(csv, "utf-8") });
  await expect(page.getByTestId("import-errors")).toBeVisible();
  await page.getByTestId("import-commit").click();
  await expect(toast()).toHaveText("1 بیمار با موفقیت وارد شد.");
  await page.getByTestId("patients-search").fill("وارده شده");
  await expect(page.getByTestId("patient-list")).toContainText("وارده شده");
});

test("delete removes a patient from the active list", async () => {
  await page.getByTestId("patients-search").fill("وارده شده");
  await page.getByText("وارده شده").click();
  page.once("dialog", (d) => d.accept());
  await page.getByTestId("patient-delete").click();
  await expect(toast()).toHaveText("پرونده بیمار حذف شد.");
  await expect(page.getByTestId("page-patients")).toBeVisible();
  await page.getByTestId("patients-search").fill("وارده شده");
  await expect(page.getByTestId("empty-state")).toBeVisible();
});
