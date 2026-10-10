import type { Browser, Page } from "@playwright/test";

/** How far playwright.config.ts moved the clock (the Core's and every page's) for this run. */
export const CLOCK_SHIFT_MS = Number(process.env.ARTAVEO_E2E_CLOCK_SHIFT_MS ?? 0);

/** A new page whose clock runs on the suite's clinic time, the same as the Core's. */
export async function newClinicPage(browser: Browser, viewport: { width: number; height: number }): Promise<Page> {
  const page = await browser.newPage({ viewport });
  await page.clock.install({ time: new Date(Date.now() + CLOCK_SHIFT_MS) });
  return page;
}
