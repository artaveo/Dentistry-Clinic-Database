// OF-045: the lock screen over text-heavy pages (patient list, About), in both themes, at 1920×1080.
// Used to tune the blur so the app's shape and colours stay visible but no text or number can be read.
// Simulated Core (VITE_MOCK); not a capture of the installed desktop app.
//
//   VITE_MOCK=1 npx vite --port 5199 &
//   node scripts/lock-screens.mjs http://127.0.0.1:5199 ../docs/design/screenshots/of-045 after
import { chromium } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

const [base = "http://127.0.0.1:5199", outDir = "../docs/design/screenshots/of-045", label = "after"] = process.argv.slice(2);
fs.mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({ channel: "msedge" });
for (const theme of ["light", "dark"]) {
  const ctx = await browser.newContext({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  await ctx.addInitScript((th) => {
    localStorage.setItem("artaveo.lang", "fa");
    localStorage.setItem("artaveo.theme", th);
    localStorage.setItem("artaveo.perf", "full");
  }, theme);
  const page = await ctx.newPage();
  await page.goto(`${base}/?mock=seeded`);
  await page.getByTestId("login-username").fill("owner");
  await page.getByTestId("login-password").fill("owner-pass-123");
  await page.getByTestId("login-submit").click();
  for (const tab of ["patients", "system"]) {
    await page.getByTestId(`tab-${tab}`).click();
    await page.waitForTimeout(700);
    await page.getByTestId("current-user").click();
    await page.getByTestId("lock").click();
    await page.getByTestId("lock-screen").waitFor();
    await page.waitForTimeout(600);
    const file = path.join(outDir, `${label}-${theme}-${tab}.png`);
    await page.screenshot({ path: file });
    console.log(file);
    await page.getByTestId("unlock-password").fill("owner-pass-123");
    await page.getByTestId("unlock").click();
    await page.getByTestId("lock-screen").waitFor({ state: "hidden" });
  }
  await ctx.close();
}
await browser.close();
