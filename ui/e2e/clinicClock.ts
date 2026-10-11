import type { Browser, Page } from "@playwright/test";

/** How far playwright.config.ts moved the clock (the Core's and every page's) for this run. */
export const CLOCK_SHIFT_MS = Number(process.env.ARTAVEO_E2E_CLOCK_SHIFT_MS ?? 0);

/** A new page whose clock runs on the suite's clinic time, the same as the Core's. */
export async function newClinicPage(browser: Browser, viewport: { width: number; height: number }): Promise<Page> {
  const page = await browser.newPage({ viewport });
  await page.clock.install({ time: new Date(Date.now() + CLOCK_SHIFT_MS) });
  // In a browser "Print" / "Save PDF" open the browser's print window, which a test cannot close (the
  // desktop app prints through WebView2 instead). Count the calls so a test can check them.
  await page.addInitScript(() => {
    (window as unknown as { __prints: number }).__prints = 0;
    window.print = () => {
      (window as unknown as { __prints: number }).__prints++;
    };
  });
  return page;
}
