import { expect, test, type Page } from "@playwright/test";

// One clinic, walked through the Phase 2 exit criteria in order: the full
// Setup Wizard, then every Shell page in three languages and two themes.
// This also re-exercises every Phase 1 feature (login, lock, backup, users,
// audit, recovery), since the wizard and shell were rebuilt around them.
test.describe.configure({ mode: "serial" });

let page: Page;
let recoveryKey = "";
const OWNER = { user: "owner", pass: "owner-pass-123" };

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage();
});

// SCREENSHOTS=1 writes the images used by docs/testing/phase-2-manual-test.md.
async function shot(name: string) {
  if (process.env.SCREENSHOTS) await page.screenshot({ path: `../docs/testing/img/phase-2/${name}.png` });
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

test("setup wizard: welcome through clinic info", async () => {
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
  await page.getByTestId("wizard-start").click();
  await shot("01-wizard-welcome");

  // Language step.
  await expect(page.getByTestId("setup-step-language")).toBeVisible();
  await page.getByTestId("wizard-next").click();

  // Install mode: single is the only enabled option.
  await expect(page.getByTestId("install-server")).toBeDisabled();
  await expect(page.getByTestId("install-client")).toBeDisabled();
  await page.getByTestId("install-single").click();
  await expect(page.getByTestId("install-single")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("wizard-next").click();

  // Clinic info: name is required before moving on.
  await expect(page.getByTestId("wizard-next")).toBeDisabled();
  await page.getByTestId("setup-clinic").fill("کلینیک دندان‌پزشکی آزمایشی");
  await page.getByTestId("setup-province").selectOption({ label: "کابل" });
  await page.getByTestId("setup-address").fill("کابل، ناحیه ۵");
  await page.getByTestId("setup-phone").fill("0700000000");
  await shot("02-wizard-clinic-info");
  await page.getByTestId("wizard-next").click();
});

test("setup wizard: hours, clinic type, branding, trial, backup", async () => {
  await expect(page.getByTestId("setup-step-hours")).toBeVisible();
  await page.getByTestId("calendar-gregorian").click();
  await page.getByTestId("wizard-next").click();

  await page.getByTestId("clinic-mode-solo").click();
  await shot("03-wizard-clinic-type");
  await page.getByTestId("wizard-next").click();

  await expect(page.getByTestId("setup-step-branding")).toBeVisible();
  await page.getByTestId("theme-dark").click();
  await shot("04-wizard-branding");
  await page.getByTestId("wizard-next").click();

  await page.getByTestId("trial-ack").check();
  await page.getByTestId("wizard-next").click();

  await expect(page.getByTestId("setup-step-backup")).toBeVisible();
  await page.getByTestId("wizard-next").click();
});

test("setup wizard: owner account and the Recovery Key", async () => {
  await expect(page.getByTestId("setup-step-owner")).toBeVisible();
  await page.getByTestId("setup-username").fill(OWNER.user);
  await page.getByTestId("setup-display").fill("مالک کلینیک");
  await page.getByTestId("setup-password").fill(OWNER.pass);
  await page.getByTestId("setup-repeat").fill("different-123");
  await page.getByTestId("setup-create").click();
  await expect(page.getByRole("alert")).toHaveText("رمزها یکسان نیستند.");
  await page.getByTestId("setup-repeat").fill(OWNER.pass);
  await page.getByTestId("setup-create").click();

  const key = page.getByTestId("recovery-key");
  await expect(key).toHaveText(/^[A-Z2-7]{6}(-[A-Z2-7]{6}){5}$/);
  recoveryKey = (await key.textContent())!;
  await shot("05-recovery-key");
  await expect(page.getByTestId("recovery-continue")).toBeDisabled();
  await page.getByTestId("recovery-confirm").check();
  await page.getByTestId("recovery-continue").click();
  await expect(page.getByTestId("clinic-name")).toHaveText("کلینیک دندان‌پزشکی آزمایشی");
  await shot("06-login");
});

test("login rejects a wrong password and accepts the owner", async () => {
  await login(OWNER.user, "wrong-password");
  await expect(page.getByTestId("login-error")).toHaveText("نام کاربری یا رمز عبور اشتباه است.");
  await login(OWNER.user, OWNER.pass);
  await expect(page.getByTestId("current-user")).toContainText("مالک کلینیک");
  // The dark theme chosen in the wizard carried through to the Shell.
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await shot("07-shell-dark");
});

test("sidebar, status strip and command palette", async () => {
  await expect(page.getByTestId("status-online")).toHaveText("آنلاین");
  await expect(page.getByTestId("status-install-mode")).toHaveText("تک‌کامپیوتر");
  await page.getByTestId("command-palette-open").click();
  await page.getByTestId("command-palette-input").fill("کاربران");
  await expect(page.getByTestId("command-item-users")).toBeVisible();
  await page.getByTestId("command-item-users").click();
  await expect(page.getByTestId("tab-users")).toHaveAttribute("aria-current", "page");
  await shot("08-command-palette");
  await page.getByTestId("tab-system").click();
});

test("System Info shows version, processor build and an encrypted, healthy database", async () => {
  await expect(page.getByTestId("si-version")).toHaveText("0.2.0");
  await expect(page.getByTestId("si-build-arch")).toHaveText(/^(x64|x86|arm64)$/);
  await expect(page.getByTestId("si-encryption")).toContainText("رمزنگاری‌شده ✓");
  await expect(page.getByTestId("si-integrity")).toContainText("سالم ✓", { timeout: 15_000 });
  await expect(page.getByTestId("si-last-backup")).toHaveText("هنوز پشتیبانی گرفته نشده");
});

test("manual backup is created, verified and listed", async () => {
  await page.getByTestId("tab-backup").click();
  await page.getByTestId("backup-now").click();
  await expect(page.getByTestId("backup-message")).toHaveText("پشتیبان گرفته و بررسی شد.");
  await expect(page.getByTestId("backup-list").locator("tbody tr")).toHaveCount(1);
  await expect(page.getByTestId("backup-list")).toContainText("دستی");
  await page.getByTestId("tab-system").click();
  await expect(page.getByTestId("si-last-backup")).not.toHaveText("هنوز پشتیبانی گرفته نشده");
});

test("owner edits the clinic profile", async () => {
  await page.getByTestId("tab-clinic").click();
  await page.getByTestId("clinic-address").fill("کابل، ناحیه ۶ — آدرس تغییریافته");
  await page.getByTestId("clinic-save").click();
  await expect(page.getByTestId("clinic-message")).toHaveText("ذخیره شد.");
  await shot("09-clinic-page");
});

test("owner creates a receptionist; audit log records it", async () => {
  await page.getByTestId("tab-users").click();
  await page.getByTestId("new-username").fill("reception");
  await page.getByTestId("new-display").fill("پذیرش");
  await page.getByTestId("new-password").fill("reception-123");
  await page.getByTestId("new-role").selectOption("receptionist");
  await page.getByTestId("create-user").click();
  await expect(page.getByTestId("users-message")).toHaveText("کاربر ایجاد شد.");
  await expect(page.getByTestId("user-list")).toContainText("reception");
  await page.getByTestId("tab-audit").click();
  for (const action of ["app.setup", "auth.login_failed", "auth.login", "backup.create", "clinic.update", "user.create"]) {
    await expect(page.getByTestId("audit-list")).toContainText(action);
  }
});

test("user menu: theme toggle, lock screen", async () => {
  await openUserMenu();
  await page.getByTestId("theme-toggle-light").click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await shot("10-shell-light");
  await page.getByTestId("lock").click();
  await expect(page.getByTestId("lock-screen")).toBeVisible();
  await shot("11-lock-screen");
  await page.getByTestId("unlock-password").fill("nope-nope");
  await page.getByTestId("unlock").click();
  await expect(page.getByTestId("lock-screen").getByRole("alert")).toBeVisible();
  await page.getByTestId("unlock-password").fill(OWNER.pass);
  await page.getByTestId("unlock").click();
  await expect(page.getByTestId("lock-screen")).toBeHidden();
});

test("language switch: English is LTR, Pashto is RTL, no layout stays broken", async () => {
  await page.getByTestId("lang-en").first().click();
  await expect(page.locator("html")).toHaveAttribute("dir", "ltr");
  await expect(page.getByTestId("tab-system")).toHaveText("About");
  await shot("12-shell-english");
  await page.getByTestId("lang-ps").first().click();
  await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
  await expect(page.getByTestId("tab-backup")).toHaveText("بیک اپ");
  await shot("13-shell-pashto");
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
  await page.getByTestId("recover-key").fill(recoveryKey.toLowerCase());
  await page.getByTestId("login-password").fill("recovered-pass-1");
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("login-info")).toHaveText("رمز مالک تغییر کرد. اکنون وارد شوید.");
  await login(OWNER.user, OWNER.pass);
  await expect(page.getByTestId("login-error")).toBeVisible();
  await login(OWNER.user, "recovered-pass-1");
  await expect(page.getByTestId("current-user")).toBeVisible();
});
