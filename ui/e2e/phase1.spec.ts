import { expect, test, type Page } from "@playwright/test";

// One clinic, walked through the Phase 1 exit criteria in order.
test.describe.configure({ mode: "serial" });

let page: Page;
let recoveryKey = "";
const OWNER = { user: "owner", pass: "owner-pass-123" };

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage();
});

// SCREENSHOTS=1 writes the images used by docs/testing/phase-1-manual-test.md.
async function shot(name: string) {
  if (process.env.SCREENSHOTS) await page.screenshot({ path: `../docs/testing/img/phase-1/${name}.png` });
}

async function login(user: string, pass: string) {
  await page.getByTestId("login-username").fill(user);
  await page.getByTestId("login-password").fill(pass);
  await page.getByTestId("login-submit").click();
}

test("first run: setup wizard creates the clinic and shows the Recovery Key once", async () => {
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
  await page.getByTestId("setup-clinic").fill("کلینیک دندان‌پزشکی آزمایشی");
  await shot("01-setup-clinic");
  await page.getByTestId("setup-next").click();
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
  await shot("02-recovery-key");
  await expect(page.getByTestId("recovery-continue")).toBeDisabled();
  await page.getByTestId("recovery-confirm").check();
  await page.getByTestId("recovery-continue").click();
  await expect(page.getByTestId("clinic-name")).toHaveText("کلینیک دندان‌پزشکی آزمایشی");
  await shot("03-login");
});

test("login rejects a wrong password and accepts the owner", async () => {
  await login(OWNER.user, "wrong-password");
  await expect(page.getByTestId("login-error")).toHaveText("نام کاربری یا رمز عبور اشتباه است.");
  await login(OWNER.user, OWNER.pass);
  await expect(page.getByTestId("current-user")).toContainText("مالک کلینیک");
});

test("System Info shows version, processor build and an encrypted, healthy database", async () => {
  await expect(page.getByTestId("si-version")).toHaveText("0.1.0");
  await expect(page.getByTestId("si-build-arch")).toHaveText(/^(x64|x86|arm64)$/);
  await expect(page.getByTestId("si-encryption")).toContainText("رمزنگاری‌شده ✓");
  await expect(page.getByTestId("si-integrity")).toContainText("سالم ✓", { timeout: 15_000 });
  await expect(page.getByTestId("si-last-backup")).toHaveText("هنوز پشتیبانی گرفته نشده");
  await shot("04-system-info");
});

test("manual backup is created, verified and listed", async () => {
  await page.getByTestId("tab-backup").click();
  await page.getByTestId("backup-now").click();
  await expect(page.getByTestId("backup-message")).toHaveText("پشتیبان گرفته و بررسی شد.");
  await expect(page.getByTestId("backup-list").locator("tbody tr")).toHaveCount(1);
  await expect(page.getByTestId("backup-list")).toContainText("دستی");
  await shot("05-backup");
  await page.getByTestId("tab-system").click();
  await expect(page.getByTestId("si-last-backup")).not.toHaveText("هنوز پشتیبانی گرفته نشده");
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
  for (const action of ["app.setup", "auth.login_failed", "auth.login", "backup.create", "user.create"]) {
    await expect(page.getByTestId("audit-list")).toContainText(action);
  }
});

test("lock screen blocks the app until the password is entered", async () => {
  await page.getByTestId("lock").click();
  await expect(page.getByTestId("lock-screen")).toBeVisible();
  await shot("06-lock-screen");
  await page.getByTestId("unlock-password").fill("nope-nope");
  await page.getByTestId("unlock").click();
  await expect(page.getByTestId("lock-screen").getByRole("alert")).toBeVisible();
  await page.getByTestId("unlock-password").fill(OWNER.pass);
  await page.getByTestId("unlock").click();
  await expect(page.getByTestId("lock-screen")).toBeHidden();
});

test("language switch: English is LTR, Pashto is RTL", async () => {
  await page.getByTestId("lang-en").first().click();
  await expect(page.locator("html")).toHaveAttribute("dir", "ltr");
  await expect(page.getByTestId("tab-system")).toHaveText("About");
  await page.getByTestId("lang-ps").first().click();
  await expect(page.locator("html")).toHaveAttribute("dir", "rtl");
  await expect(page.getByTestId("tab-backup")).toHaveText("بیک اپ");
  await page.getByTestId("lang-fa").first().click();
});

test("receptionist sees only what the role allows", async () => {
  await page.getByTestId("logout").click();
  await login("reception", "reception-123");
  await expect(page.getByTestId("current-user")).toContainText("پذیرش");
  await expect(page.getByTestId("tab-system")).toBeVisible();
  for (const tab of ["tab-users", "tab-audit", "tab-backup", "tab-settings"]) {
    await expect(page.getByTestId(tab)).toHaveCount(0);
  }
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
