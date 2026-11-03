// Screenshots of the dental chart tab against the in-browser mock Core (no Rust needed), for design review:
//   VITE_MOCK=1 npx vite --port 8640      (in ui/)
//   node scripts/chart-review.mjs http://127.0.0.1:8640 <out dir> [width] [height] [light|dark] [fa|en|ps]
import { chromium } from "@playwright/test";

const [base = "http://127.0.0.1:8640", out = "chart-review", width = "1160", height = "740", theme = "light", lang = "fa"] = process.argv.slice(2);
const browser = await chromium.launch({ channel: "msedge" });
const page = await browser.newPage({ viewport: { width: Number(width), height: Number(height) }, deviceScaleFactor: 1.5, colorScheme: theme });
const shot = (name) => page.screenshot({ path: `${out}/${name}-${lang}-${theme}-${width}.png` });

await page.goto(`${base}/?mock=seeded`);
await page.getByTestId("login-username").fill("owner");
await page.getByTestId("login-password").fill("owner-pass-123");
await page.getByTestId("login-submit").click();
if (lang !== "fa") await page.getByTestId(`lang-${lang}`).first().click();
await page.getByTestId("tab-patients").click();
await page.locator("tr, [role=row]").filter({ hasText: "احمد خان" }).first().click();
await page.getByTestId("patient-tab-chart").click();
await page.getByTestId("odontogram").waitFor();
await page.getByTestId("chart-tab").evaluate((el) => el.scrollIntoView({ block: "start" }));
await page.waitForTimeout(400);
await shot("01-chart");

await page.getByTestId("tooth-36").locator(".odo-col").click();
await page.waitForTimeout(200);
await shot("02-tooth-selected");

await page.getByTestId("chart-mode-finding").click();
await page.getByTestId("condition-caries").click();
await page.waitForTimeout(200);
await shot("03-finding-dialog");
await page.getByTestId("surface-pick-O").click();
await page.getByTestId("surface-pick-D").click();
await page.getByTestId("finding-need-surface").or(page.getByTestId("finding-surfaces")).scrollIntoViewIfNeeded();
await shot("04-finding-surfaces");
await page.getByTestId("finding-save").click();
await page.waitForTimeout(400);
await shot("05-after-finding");

await page.getByTestId("chart-mode-plan").click();
await page.getByTestId("item-service-search").fill("END-01D");
await page.getByTestId("item-service-END-01D").click();
await page.getByTestId("item-add").scrollIntoViewIfNeeded();
await shot("06-plan-dialog");
await page.getByTestId("item-more").locator("summary").click();
await page.getByTestId("item-add").scrollIntoViewIfNeeded();
await shot("07-plan-more");
await page.getByTestId("item-add").click();
await page.waitForTimeout(400);
await shot("08-after-plan");

await page.getByTestId("tooth-details-36").scrollIntoViewIfNeeded();
await shot("09-tooth-details");
await page.getByTestId("chart-clear").click();
await page.getByTestId("chart-quick-open").click();
await shot("10-quick");
await browser.close();
console.log("done");
