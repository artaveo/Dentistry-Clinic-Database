// Design review screenshots (docs/design/screenshots/<version>/): every main
// page in Dari light, Dari dark and English light, at the app's window size.
//
//   VITE_MOCK=1 npx vite --port 5199 &          (simulated Core, src/dev/mockCore.ts)
//   node scripts/design-screens.mjs http://127.0.0.1:5199 ../docs/design/screenshots/v0.2.1
//
// Uses the installed Microsoft Edge (Playwright `msedge` channel) unless
// CHROME_PATH is set. Page selectors are the stable `data-testid`s.
import { chromium } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

const [base = "http://127.0.0.1:5199", outDir = "../docs/design/screenshots/current"] = process.argv.slice(2);
const variants = [
  { name: "fa-light", lang: "fa", theme: "light" },
  { name: "fa-dark", lang: "fa", theme: "dark" },
  { name: "en-light", lang: "en", theme: "light" },
];
const PAGES = [
  ["03-about", "system"],
  ["04-users", "users"],
  ["05-backup", "backup"],
  ["06-audit", "audit"],
  ["07-settings", "settings"],
  ["08-clinic", "clinic"],
];

const browser = await chromium.launch(process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : { channel: "msedge" });
for (const v of variants) {
  const dir = path.resolve(outDir, v.name);
  fs.mkdirSync(dir, { recursive: true });
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  await ctx.addInitScript(({ lang, theme }) => {
    localStorage.setItem("artaveo.lang", lang);
    localStorage.setItem("artaveo.theme", theme);
    localStorage.setItem("artaveo.perf", "full");
  }, v);
  const page = await ctx.newPage();
  // v0.2.1+ shows the Artaveo splash first; wait until it has gone.
  const noSplash = () => page.getByTestId("splash").waitFor({ state: "detached", timeout: 5000 }).catch(() => {});
  const shot = async (name) => {
    await page.waitForTimeout(450); // let fonts, transitions and blur settle
    await page.screenshot({ path: path.join(dir, `${name}.png`) });
    console.log(`${v.name}/${name}.png`);
  };

  // 1) First-run setup: welcome and the clinic information step.
  await page.goto(`${base}/?mock=fresh`);
  await page.getByTestId("wizard-start").waitFor();
  await noSplash();
  await shot("01-setup-welcome");
  await page.getByTestId("wizard-start").click();
  for (let i = 0; i < 2; i++) await page.getByTestId("wizard-next").click();
  await page.getByTestId("setup-clinic").fill(v.lang === "en" ? "Smile Dental Clinic" : "کلینیک دندان‌پزشکی لبخند");
  await shot("01-setup-clinic-info");

  // 2) Login, then every Shell page (seeded clinic).
  await page.goto(`${base}/?mock=seeded`);
  await page.getByTestId("login-username").waitFor();
  await noSplash();
  await shot("02-login");
  await page.getByTestId("login-username").fill("owner");
  await page.getByTestId("login-password").fill("owner-pass-123");
  await page.getByTestId("login-submit").click();
  await page.getByTestId("tab-system").waitFor();
  for (const [name, tab] of PAGES) {
    await page.getByTestId(`tab-${tab}`).click();
    // v0.2.1+ pages carry data-testid="page-<tab>"; v0.2.0 had none.
    await page.getByTestId(`page-${tab}`).waitFor({ timeout: 3000 }).catch(() => {});
    await shot(name);
  }
  await ctx.close();
}
await browser.close();
