import { expect, test, type Locator, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// v0.5.0 (Phase 5A and owner feedback OF-044, OF-045) against the real Core. Runs after the earlier files
// in the same clinic and session (single worker, one dev server), so it only logs in. The suite's clinic
// clock is 10:00 today (playwright.config.ts), so every afternoon time below is still to come.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };

test.beforeAll(async ({ browser }) => {
  page = await newClinicPage(browser, { width: 1360, height: 900 });
  await page.goto("/");
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
});

const toast = () => page.getByTestId("toast").last();

test("OF-045: the lock screen shows the app's shape through a light blur, never an opaque wall", async () => {
  await page.getByTestId("current-user").click();
  await page.getByTestId("lock").click();
  await expect(page.getByTestId("lock-screen")).toBeVisible();
  const blur = await page.locator(".app-main").evaluate((el) => Number(/blur\(([\d.]+)px\)/.exec(getComputedStyle(el).filter)?.[1] ?? 0));
  expect(blur).toBeGreaterThanOrEqual(14);
  expect(blur).toBeLessThanOrEqual(20);
  // The tint over it is translucent (about 40–55 %), and there is no second blur on top of the first.
  const tint = await page.getByTestId("lock-screen").evaluate((el) => {
    const s = getComputedStyle(el);
    const alpha = /rgba?\([^)]*?,\s*([\d.]+)\)|\/\s*([\d.]+)\)/.exec(s.backgroundColor);
    return { alpha: Number(alpha?.[1] ?? alpha?.[2] ?? 1), backdrop: s.backdropFilter };
  });
  expect(tint.alpha).toBeGreaterThanOrEqual(0.38);
  expect(tint.alpha).toBeLessThanOrEqual(0.56);
  expect(tint.backdrop === "none" || tint.backdrop === "").toBeTruthy();
  await expect(page.locator(".app-main")).toHaveAttribute("inert", "");
  await page.getByTestId("unlock-password").fill(OWNER.pass);
  await page.getByTestId("unlock").click();
  await expect(page.getByTestId("lock-screen")).toHaveCount(0);
});

test("OF-044: the calendar drag step is a setting (5 minutes by default)", async () => {
  await page.getByTestId("tab-settings").click();
  await expect(page.getByTestId("setting-calendar-snap-5")).toHaveAttribute("aria-checked", "true");
});

/** Screen position of minute `min` of the day in a calendar column (data attributes on the time grid). */
async function minuteY(col: Locator, min: number) {
  const body = page.locator(".tg-body");
  const start = Number(await body.getAttribute("data-start-min"));
  const px = Number(await body.getAttribute("data-px-per-min"));
  const box = (await col.boundingBox())!;
  return box.y + (min - start) * px;
}

/** Presses on the top edge of `appt` and moves it so its start lands on minute `min` of `col`. */
async function dragTo(appt: Locator, col: Locator, min: number) {
  await appt.scrollIntoViewIfNeeded();
  const box = (await appt.boundingBox())!;
  const grabAt = 3;
  await page.mouse.move(box.x + box.width / 2, box.y + grabAt);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + grabAt + 20, { steps: 3 });
  const colBox = (await col.boundingBox())!;
  await page.mouse.move(colBox.x + colBox.width / 2, (await minuteY(col, min)) + grabAt, { steps: 10 });
}

test("OF-044: dragging shows the exact landing time, snaps to 5 minutes, and saves 3:40 → 5:10 exactly", async () => {
  // A visit at 3:40 PM for a new patient.
  await page.getByTestId("tab-appointments").click();
  await expect(page.getByTestId("page-calendar")).toBeVisible();
  await page.getByTestId("cal-view-day").click();
  await page.getByTestId("cal-by-doctor").click();
  await page.getByTestId("appt-new").click();
  await page.getByTestId("patient-picker-new").click();
  await page.getByTestId("patient-picker-name-input").fill("بیمار کشیدن تقویم");
  await page.getByTestId("patient-picker-phone-input").fill("0700900044");
  await page.getByTestId("patient-picker-create").click();
  await expect(page.getByTestId("patient-picker-name")).toHaveText("بیمار کشیدن تقویم");
  await page.getByTestId("appt-doctor").selectOption({ label: "داکتر احمد رحیمی — ارتودنسی" });
  await page.getByTestId("appt-start-hour").selectOption("3");
  await page.getByTestId("appt-start-minute").selectOption("40");
  await page.getByTestId("appt-start-pm").click();
  await page.getByTestId("appt-save").click();
  await expect(toast()).toHaveText("نوبت ثبت شد.");

  const appt = page.locator('[data-testid^="cal-appt-"]').filter({ hasText: "بیمار کشیدن تقویم" });
  await expect(appt).toHaveAttribute("title", /۳:۴۰ ب\.ظ–۴:۱۰ ب\.ظ/);
  const col = appt.locator("xpath=ancestor::div[contains(@class,'tg-col')]");

  // Esc cancels a drag: nothing moves.
  await dragTo(appt, col, 17 * 60 + 10);
  await expect(page.getByTestId("cal-drag-label")).toHaveText("۵:۱۰ ب.ظ – ۵:۴۰ ب.ظ");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("cal-drag-ghost")).toHaveCount(0);
  await page.mouse.up();
  await expect(appt).toHaveAttribute("title", /۳:۴۰ ب\.ظ–۴:۱۰ ب\.ظ/);
  await expect(page.getByTestId("appointment-dialog")).toHaveCount(0);

  // A time already gone is red with its reason, and releasing there does not move the visit.
  await dragTo(appt, col, 9 * 60);
  await expect(page.getByTestId("cal-drag-ghost")).toHaveAttribute("data-valid", "false");
  await expect(page.getByTestId("cal-drag-issue")).toHaveText("زمان گذشته است");
  await page.mouse.up();
  await expect(appt).toHaveAttribute("title", /۳:۴۰ ب\.ظ–۴:۱۰ ب\.ظ/);

  // The live label follows the pointer on 5-minute steps; releasing on 5:10 saves exactly 5:10–5:40.
  await dragTo(appt, col, 17 * 60 + 12);
  await expect(page.getByTestId("cal-drag-label")).toHaveText("۵:۱۰ ب.ظ – ۵:۴۰ ب.ظ");
  await expect(page.getByTestId("cal-drag-ghost")).toHaveAttribute("data-start", "17:10");
  await expect(page.getByTestId("cal-drag-ghost")).toHaveAttribute("data-valid", "true");
  await page.mouse.up();
  await expect(toast()).toHaveText("نوبت جابه‌جا شد.");
  await expect(appt).toHaveAttribute("title", /۵:۱۰ ب\.ظ–۵:۴۰ ب\.ظ/);
  await appt.click();
  await expect(page.getByTestId("appt-start-hour")).toHaveValue("5");
  await expect(page.getByTestId("appt-start-minute")).toHaveValue("10");
  await expect(page.getByTestId("appt-start-pm")).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("Escape");
});
