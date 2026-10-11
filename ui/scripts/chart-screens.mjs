// Screenshots of the Phase 5B clinical screens for docs/design/screenshots/v0.6.0 (the manual-test guide's
// pictures come from the E2E run: SHOTS=1 npx playwright test). Against the real Core (dev server) filled by scripts/seed-5b.mjs.
//
//   [SHOT_H=1300] node scripts/chart-screens.mjs http://127.0.0.1:5199 <out dir> [fa|ps|en] [light|dark] [width]
import { chromium } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

const [base = "http://127.0.0.1:5199", outDir = "../docs/design/screenshots/v0.6.0", lang = "fa", theme = "light", width = "1440"] = process.argv.slice(2);
fs.mkdirSync(outDir, { recursive: true });
const browser = await chromium.launch({ channel: "msedge" });
const ctx = await browser.newContext({ viewport: { width: Number(width), height: Number(process.env.SHOT_H ?? 1700) }, deviceScaleFactor: 1 });
await ctx.addInitScript(([l, th]) => {
  localStorage.setItem("artaveo.lang", l);
  localStorage.setItem("artaveo.theme", th);
  localStorage.setItem("artaveo.perf", "full");
}, [lang, theme]);
const page = await ctx.newPage();
page.on("pageerror", (e) => console.error("pageerror", e.message));
const suffix = `${lang}-${theme}${width === "1440" ? "" : `-${width}`}`;
const shot = async (name, opts = {}) => {
  await page.waitForTimeout(600);
  await page.screenshot({ path: path.join(outDir, `${name}-${suffix}.png`), ...opts });
  console.log(`${name}-${suffix}.png`);
};

await page.goto(base);
await page.getByTestId("login-username").fill("owner");
await page.getByTestId("login-password").fill("owner-pass-123");
await page.getByTestId("login-submit").click();
await page.getByTestId("tab-patients").click();
await page.getByText("فریده احمدی").first().click();
await page.getByTestId("page-patient-profile").waitFor();
await shot("profile-overview");
await page.getByTestId("patient-tab-chart").click();
await page.getByTestId("odontogram").waitFor();
await shot("chart-general");
await page.getByTestId("tooth-36").click();
await shot("chart-tooth-36");
await page.getByTestId("chart-view-implant").click();
await shot("chart-implant");
await page.getByTestId("chart-view-prosth").click();
await shot("chart-prosth");
await page.getByTestId("chart-view-perio").click();
await page.getByTestId("perio-chart").waitFor();
await shot("chart-perio");
await page.getByTestId("chart-view-general").click();
await page.getByTestId("patient-tab-plans").click();
await page.getByTestId("plans-tab").waitFor();
await shot("plans");
await page.getByTestId("patient-tab-notes").click();
await page.getByTestId("notes-tab").waitFor();
await shot("notes");
await page.getByTestId("patient-tab-timeline").click();
await page.getByTestId("timeline").waitFor();
await shot("timeline");
await browser.close();
