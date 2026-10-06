// Screenshots for a release's manual-test guide (docs/testing/img/<version>/): the owner-feedback
// screens that changed in v0.4.1, in the simulated Core (VITE_MOCK). They show the UI as built;
// they are not captures of the installed desktop app.
//
//   VITE_MOCK=1 npx vite --port 5199 &
//   node scripts/release-screens.mjs http://127.0.0.1:5199 ../docs/testing/img/v0.4.1
import { chromium } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

const [base = "http://127.0.0.1:5199", outDir = "../docs/testing/img/v0.4.1"] = process.argv.slice(2);
fs.mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: "msedge" });
const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
await ctx.addInitScript(() => {
  localStorage.setItem("artaveo.lang", "fa");
  localStorage.setItem("artaveo.theme", "light");
  localStorage.setItem("artaveo.perf", "full");
});
const page = await ctx.newPage();
const settle = () => page.waitForTimeout(500);
const shot = async (name) => {
  await settle();
  await page.screenshot({ path: path.join(outDir, `${name}.png`) });
  console.log(`${name}.png`);
};

await page.goto(`${base}/?mock=seeded`);
await page.getByTestId("login-username").waitFor();
await page.getByTestId("login-username").fill("owner");
await page.getByTestId("login-password").fill("owner-pass-123");
await page.getByTestId("login-submit").click();
await page.getByTestId("tab-patients").waitFor();

// OF-015: the lock screen over the patient list.
await page.getByTestId("tab-patients").click();
await page.getByTestId("current-user").click();
await page.getByTestId("lock").click();
await page.getByTestId("lock-screen").waitFor();
await shot("of015-lock-screen");
await page.getByTestId("unlock-password").fill("owner-pass-123");
await page.getByTestId("unlock").click();
await page.getByTestId("lock-screen").waitFor({ state: "detached" });

// OF-027: the smallest supported window.
await page.setViewportSize({ width: 800, height: 600 });
await shot("of027-window-800x600");

// OF-019: the age / exact birth date choice in the patient form.
await page.getByTestId("add-patient-open").click();
await page.getByTestId("patient-exact-birth").check();
await page.getByTestId("patient-dob").fill("1990-05-20");
await shot("of019-birth-date-or-age");
await page.getByTestId("patient-cancel").click();

// OF-032: the whole week in one view.
await page.setViewportSize({ width: 1280, height: 800 });
await page.getByTestId("tab-appointments").click();
await page.getByTestId("cal-view-week").click();
await shot("of032-week-view");

// OF-028: a built-in role opened for editing.
await page.getByTestId("tab-users").click();
await page.getByTestId("role-edit-receptionist").click();
await shot("of028-role-dialog");

await browser.close();
