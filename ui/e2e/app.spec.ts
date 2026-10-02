import { expect, test, type Page } from "@playwright/test";

// One clinic, walked through end to end against the real Core: the full Setup
// Wizard, every Shell page in three languages and two themes, and the owner
// feedback fixed in v0.2.1 (OF-001…OF-008: live field validation, LTR font and
// table alignment, 12-hour clock, the lock screen).
test.describe.configure({ mode: "serial" });

let page: Page;
let recoveryKey = "";
const OWNER = { user: "owner", pass: "owner-pass-123" };

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
});

// SCREENSHOTS=1 writes images for the manual test guide (docs/testing/img/v0.2.1).
async function shot(name: string) {
  if (process.env.SCREENSHOTS) await page.screenshot({ path: `../docs/testing/img/v0.2.1/${name}.png` });
}

async function login(user: string, pass: string) {
  await page.getByTestId("login-username").fill(user);
  await page.getByTestId("login-password").fill(pass);
  await page.getByTestId("login-submit").click();
}

async function openUserMenu() {
  if (await page.getByTestId("user-menu").isVisible()) return;
  await page.getByTestId("current-user").click();
  await expect(page.getByTestId("user-menu")).toBeVisible();
}

const toast = () => page.getByTestId("toast").last();

test("setup wizard: welcome through clinic info", async () => {
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
  await page.getByTestId("wizard-start").click();

  await expect(page.getByTestId("setup-step-language")).toBeVisible();
  await page.getByTestId("wizard-next").click();

  // Install mode: single is the only enabled option.
  await expect(page.getByTestId("install-server")).toBeDisabled();
  await expect(page.getByTestId("install-client")).toBeDisabled();
  await page.getByTestId("install-single").click();
  await expect(page.getByTestId("install-single")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("wizard-next").click();

  // Clinic info: name is required before moving on; a bad phone is flagged live.
  await expect(page.getByTestId("wizard-next")).toBeDisabled();
  await page.getByTestId("setup-clinic").fill("کلینیک دندان‌پزشکی آزمایشی");
  await page.getByTestId("setup-province").selectOption({ label: "کابل" });
  await page.getByTestId("setup-address").fill("کابل، ناحیه ۵");
  await page.getByTestId("setup-phone").fill("abc");
  await expect(page.getByTestId("field-error")).toHaveText("شماره تماس باید ۷ تا ۱۵ رقم باشد.");
  await page.getByTestId("setup-phone").fill("0700000000");
  await expect(page.getByTestId("field-error")).toHaveCount(0);
  await shot("01-wizard-clinic-info");
  await page.getByTestId("wizard-next").click();
});

test("setup wizard: hours (12-hour pickers), clinic type, branding, trial, backup", async () => {
  await expect(page.getByTestId("setup-step-hours")).toBeVisible();
  // Saturday 08:00 is shown as 8 + ق.ظ, never 0–23 (OF-007).
  await expect(page.getByTestId("day-0-from-hour")).toHaveValue("8");
  await expect(page.getByTestId("day-0-from-am")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("calendar-gregorian").click();
  await page.getByTestId("wizard-next").click();

  await page.getByTestId("clinic-mode-solo").click();
  await page.getByTestId("wizard-next").click();

  await expect(page.getByTestId("setup-step-branding")).toBeVisible();
  await page.getByTestId("theme-dark").click();
  await page.getByTestId("wizard-next").click();

  await page.getByTestId("trial-ack").check();
  await page.getByTestId("wizard-next").click();

  await expect(page.getByTestId("setup-step-backup")).toBeVisible();
  await page.getByTestId("wizard-next").click();
});

test("setup wizard: owner fields validate live (OF-001…003), then the Recovery Key", async () => {
  await expect(page.getByTestId("setup-step-owner")).toBeVisible();
  await page.getByTestId("setup-display").fill("مالک کلینیک");
  // A Dari username is flagged before pressing the button, under that field.
  await page.getByTestId("setup-username").fill("احمد");
  await expect(page.getByTestId("field-error")).toHaveText("نام کاربری فقط حروف انگلیسی، عدد و . _ - ؛ حروف فارسی/پشتو مجاز نیست.");
  await expect(page.getByTestId("setup-username")).toHaveAttribute("aria-invalid", "true");
  await shot("02-live-validation");
  await page.getByTestId("setup-username").fill(OWNER.user);
  await page.getByTestId("setup-password").fill("short");
  await expect(page.getByTestId("field-error")).toHaveText("رمز باید حداقل ۸ حرف باشد.");
  await page.getByTestId("setup-password").fill(OWNER.pass);
  await page.getByTestId("setup-repeat").fill("different-123");
  await expect(page.getByTestId("field-error")).toHaveText("تکرار رمز با رمز یکسان نیست.");
  await page.getByTestId("setup-repeat").fill(OWNER.pass);
  await expect(page.getByTestId("field-error")).toHaveCount(0);
  await page.getByTestId("setup-create").click();

  const key = page.getByTestId("recovery-key");
  await expect(key).toHaveText(/^[A-Z2-7]{6}(-[A-Z2-7]{6}){5}$/);
  recoveryKey = (await key.textContent())!;
  await expect(page.getByTestId("recovery-continue")).toBeDisabled();
  await page.getByTestId("recovery-confirm").check();
  await page.getByTestId("recovery-continue").click();
  await expect(page.getByTestId("clinic-name")).toHaveText("کلینیک دندان‌پزشکی آزمایشی");
});

test("login: LTR username keeps the UI font (OF-004); wrong password, then the owner", async () => {
  const font = await page.getByTestId("login-username").evaluate((el) => getComputedStyle(el).fontFamily);
  expect(font).not.toMatch(/mono|consolas|courier/i);
  await login(OWNER.user, "wrong-password");
  await expect(page.getByTestId("login-error")).toHaveText("نام کاربری یا رمز عبور اشتباه است.");
  await login(OWNER.user, OWNER.pass);
  await expect(page.getByTestId("current-user")).toContainText("مالک کلینیک");
  // The dark theme chosen in the wizard carried through to the Shell.
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  // Clinic identity (not Artaveo) in the header (OF-011).
  await expect(page.getByTestId("shell-clinic-name")).toHaveText("کلینیک دندان‌پزشکی آزمایشی");
  await expect(page.getByTestId("clinic-identity").getByTestId("clinic-mark")).toHaveText("ک");
});

test("header status and command palette", async () => {
  await expect(page.getByTestId("status-online")).toHaveText("آنلاین");
  await expect(page.getByTestId("status-install-mode")).toHaveText("تک‌کامپیوتر");
  await page.getByTestId("command-palette-open").click();
  await page.getByTestId("command-palette-input").fill("کاربران");
  await expect(page.getByTestId("command-item-users")).toBeVisible();
  await page.getByTestId("command-item-users").click();
  await expect(page.getByTestId("tab-users")).toHaveAttribute("aria-current", "page");
  await page.getByTestId("tab-system").click();
});

test("About shows version, processor build and an encrypted, healthy database", async () => {
  await expect(page.getByTestId("si-version")).toHaveText("0.2.1");
  await expect(page.getByTestId("si-build-arch")).toHaveText(/^(x64|x86|arm64)$/);
  await expect(page.getByTestId("si-encryption")).toHaveText("رمزنگاری‌شده");
  await expect(page.getByTestId("si-integrity")).toHaveText("سالم", { timeout: 15_000 });
  await expect(page.getByTestId("si-last-backup")).toHaveText("هنوز پشتیبانی گرفته نشده");
});

test("manual backup; dates read as one phrase with a 12-hour time (OF-006/007)", async () => {
  await page.getByTestId("tab-backup").click();
  await page.getByTestId("backup-now").click();
  await expect(toast()).toHaveText("پشتیبان گرفته و بررسی شد.");
  await expect(page.getByTestId("backup-list").locator("tbody tr")).toHaveCount(1);
  await expect(page.getByTestId("backup-list")).toContainText("دستی");
  // The wizard chose the Gregorian calendar: "2 October 2026، ساعت 1:14 ق.ظ" in Dari digits.
  await expect(page.getByTestId("backup-list").locator("tbody td").first()).toHaveText(/^[۰-۹]+ [^،]+ [۰-۹]{4}، ساعت [۰-۹]{1,2}:[۰-۹]{2} (ق|ب)\.ظ$/);
  await page.getByTestId("tab-system").click();
  await expect(page.getByTestId("si-last-backup")).toHaveText(/ساعت [۰-۹]{1,2}:[۰-۹]{2} (ق|ب)\.ظ$/);
});

test("settings: the daily backup time is picked as 12-hour (OF-007)", async () => {
  await page.getByTestId("tab-settings").click();
  // Default 19:00 → 7 ب.ظ.
  await expect(page.getByTestId("setting-daily_backup_hour-hour")).toHaveValue("7");
  await expect(page.getByTestId("setting-daily_backup_hour-pm")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("setting-daily_backup_hour-hour").selectOption("8");
  await page.getByTestId("setting-daily_backup_hour-am").click();
  // A bad value is flagged under its own field before saving.
  await page.getByTestId("setting-session_timeout_minutes").fill("0");
  await expect(page.getByTestId("field-error")).toHaveText("عددی بین ۱ تا ۲۴۰ وارد کنید.");
  await page.getByTestId("setting-session_timeout_minutes").fill("۱۰");
  await page.getByTestId("settings-save").click();
  await expect(toast()).toHaveText("تغییرات ذخیره شد.");
  await page.getByTestId("tab-backup").click();
  await expect(page.getByTestId("page-backup")).toContainText("هر روز، ساعت ۸:۰۰ ق.ظ");
});

test("owner edits the clinic profile", async () => {
  await page.getByTestId("tab-clinic").click();
  await page.getByTestId("clinic-address").fill("کابل، ناحیه ۶ — آدرس تغییریافته");
  await page.getByTestId("clinic-save").click();
  await expect(toast()).toHaveText("اطلاعات کلینیک ذخیره شد.");
});

test("users: live validation, Core field errors under the field (OF-001/002)", async () => {
  await page.getByTestId("tab-users").click();
  await page.getByTestId("add-user-open").click();
  const dialog = page.getByTestId("create-user-dialog");
  await dialog.getByTestId("new-display").fill("پذیرش");
  await dialog.getByTestId("new-username").fill("احمد");
  await expect(dialog.getByTestId("field-error")).toHaveText("نام کاربری فقط حروف انگلیسی، عدد و . _ - ؛ حروف فارسی/پشتو مجاز نیست.");
  // Taken usernames are only known to the Core: its error lands under the same field.
  await dialog.getByTestId("new-username").fill("Owner");
  await dialog.getByTestId("new-password").fill("reception-123");
  await dialog.getByTestId("create-user").click();
  await expect(dialog.getByTestId("field-error")).toHaveText("این نام کاربری قبلاً استفاده شده است؛ نام دیگری انتخاب کنید.");
  await dialog.getByTestId("new-username").fill("reception");
  await dialog.getByTestId("new-role").selectOption("receptionist");
  await dialog.getByTestId("create-user").click();
  await expect(toast()).toHaveText("کاربر ایجاد شد.");
  await expect(page.getByTestId("user-list")).toContainText("reception");
});

test("tables: LTR cells align with their column header in RTL (OF-005)", async () => {
  const offsets = await page.getByTestId("user-list").evaluate((table) =>
    [...table.querySelectorAll("tbody td")]
      .filter((td) => td.querySelector("bdi.ltr"))
      .map((td) => {
        const cell = td.getBoundingClientRect();
        const text = td.querySelector("bdi.ltr")!.getBoundingClientRect();
        return Math.abs(cell.right - parseFloat(getComputedStyle(td).paddingRight) - text.right);
      }),
  );
  expect(offsets.length).toBeGreaterThan(0);
  for (const o of offsets) expect(o).toBeLessThan(1.5);
});

test("audit log records everything, with 12-hour times", async () => {
  await page.getByTestId("tab-audit").click();
  for (const action of ["app.setup", "auth.login_failed", "auth.login", "backup.create", "clinic.update", "user.create", "settings.update"]) {
    await expect(page.getByTestId("audit-list")).toContainText(action);
  }
  await expect(page.getByTestId("audit-list").locator("tbody tr").first()).toContainText(/(ق|ب)\.ظ/);
});

test("lock screen: a stale «locked» answer never survives unlocking (OF-008)", async () => {
  await page.getByTestId("tab-system").click();
  await expect(page.getByTestId("si-version")).toBeVisible();
  await openUserMenu();
  await page.getByTestId("theme-toggle-light").click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.getByTestId("lock").click();
  await expect(page.getByTestId("lock-screen")).toBeVisible();
  // About polls every 5 s: while locked those polls get `session_locked`.
  await page.waitForTimeout(6_000);
  await shot("03-lock-screen");
  await page.getByTestId("unlock-password").fill("nope-nope");
  await page.getByTestId("unlock").click();
  await expect(page.getByTestId("lock-screen").getByRole("alert")).toHaveText("رمز درست نیست.");
  await page.getByTestId("unlock-password").fill(OWNER.pass);
  await page.getByTestId("unlock").click();
  await expect(page.getByTestId("lock-screen")).toBeHidden();
  // The page works at once and keeps working through further polls.
  await expect(page.getByTestId("si-version")).toBeVisible();
  await page.waitForTimeout(6_000);
  await expect(page.getByTestId("lock-screen")).toBeHidden();
  await expect(page.getByTestId("error-state")).toHaveCount(0);
  await expect(page.getByTestId("si-version")).toBeVisible();
});

test("change password validates live in its dialog", async () => {
  await openUserMenu();
  await page.getByTestId("change-password").click();
  await page.getByTestId("password-current").fill("wrong-current");
  await page.getByTestId("password-new").fill("brand-new-pass");
  await page.getByTestId("password-repeat").fill("brand-new-pass");
  await page.getByTestId("password-save").click();
  await expect(page.getByTestId("password-dialog").getByTestId("field-error")).toHaveText("رمز درست نیست.");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("password-dialog")).toHaveCount(0);
});

test("language switch: English is LTR, Pashto is RTL", async () => {
  await page.getByTestId("lang-en").first().click();
  await expect(page.locator("html")).toHaveAttribute("dir", "ltr");
  await expect(page.getByTestId("tab-system")).toHaveText("About");
  await expect(page.getByTestId("si-last-backup")).toHaveText(/, \d{1,2}:\d{2} (AM|PM)$/);
  await page.getByTestId("lang-ps").first().click();
  await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
  await expect(page.getByTestId("tab-backup")).toHaveText("بیک اپ");
  await expect(page.getByTestId("si-last-backup")).toHaveText(/غ\.(م|و)$/);
  await page.getByTestId("lang-fa").first().click();
});

test("receptionist sees only what the role allows", async () => {
  await openUserMenu();
  await page.getByTestId("logout").click();
  await login("reception", "reception-123");
  await expect(page.getByTestId("current-user")).toContainText("پذیرش");
  await expect(page.getByTestId("tab-system")).toBeVisible();
  for (const tab of ["tab-users", "tab-audit", "tab-backup", "tab-clinic", "tab-settings"]) {
    await expect(page.getByTestId(tab)).toHaveCount(0);
  }
  await openUserMenu();
  await page.getByTestId("logout").click();
});

test("owner password is recovered offline with the Recovery Key", async () => {
  await page.getByTestId("login-mode").click();
  await page.getByTestId("recover-key").fill("AAAAAA-AAAAAA-AAAAAA-AAAAAA-AAAAAA-AAAAAA");
  await page.getByTestId("login-password").fill("recovered-pass-1");
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("field-error")).toHaveText("کلید بازیابی با این کلینیک مطابقت ندارد. حروف را دوباره بررسی کنید.");
  await page.getByTestId("recover-key").fill(recoveryKey.toLowerCase());
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("login-info")).toHaveText("رمز مالک تغییر کرد. اکنون وارد شوید.");
  await login(OWNER.user, OWNER.pass);
  await expect(page.getByTestId("login-error")).toBeVisible();
  await login(OWNER.user, "recovered-pass-1");
  await expect(page.getByTestId("current-user")).toBeVisible();
});
