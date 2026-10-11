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

// ───────────────────────────── Phase 5A ─────────────────────────────

/** With SHOTS=1 the run also saves the screens of the manual-test guide (real Core, not the simulator). */
async function shot(name: string) {
  if (!process.env.SHOTS) return;
  await page.waitForTimeout(400);
  // Earlier steps' confirmations would cover the page; the docs show the screen itself.
  await page.screenshot({ path: `../docs/testing/img/v0.5.0/${name}.png`, style: ".toasts { display: none !important; }" });
}

/** The PDF Chromium (WebView2's engine) makes of the page's print layer: its first page size in mm. */
async function pdfPageMm(): Promise<{ w: number; h: number }> {
  const pdf = (await page.pdf({ preferCSSPageSize: true, printBackground: true })).toString("latin1");
  const m = /\/MediaBox\s*\[\s*0\s+0\s+([\d.]+)\s+([\d.]+)\s*\]/.exec(pdf)!;
  return { w: Math.round((Number(m[1]) / 72) * 25.4), h: Math.round((Number(m[2]) / 72) * 25.4) };
}

test("M3: the service catalog is seeded from the spec, prices are set by the clinic", async () => {
  await page.getByTestId("tab-catalog").click();
  await expect(page.getByTestId("page-catalog")).toBeVisible();
  await expect(page.getByTestId("catalog-row-END-01D")).toContainText("دوکاناله");
  await expect(page.getByTestId("catalog-row-END-01D")).toContainText("تعیین نشده");
  await shot("01-catalog");
  await page.getByTestId("catalog-edit-END-01D").click();
  await page.getByTestId("catalog-service-price").fill("3500");
  await page.getByTestId("catalog-service-save").click();
  await expect(page.getByTestId("catalog-row-END-01D")).toContainText("۳٬۵۰۰ افغانی");
  // A new service: the code is checked live and by the Core (unique, Latin).
  await page.getByTestId("catalog-cat-SRG").click();
  await page.getByTestId("catalog-service-new").click();
  await page.getByTestId("catalog-service-code").fill("ریشه");
  await page.getByTestId("catalog-service-save").click();
  await expect(page.getByTestId("catalog-service-dialog").getByTestId("field-error").first()).toBeVisible();
  await page.getByTestId("catalog-service-code").fill("srg-01");
  await page.getByTestId("catalog-service-name-fa").fill("کشیدن دندان شیری");
  await page.getByTestId("catalog-service-save").click();
  await expect(page.getByTestId("catalog-service-dialog")).toContainText("خدمت دیگری همین کد را دارد");
  await page.getByTestId("catalog-service-code").fill("SRG-04");
  await page.getByTestId("catalog-service-price").fill("500");
  await page.getByTestId("catalog-service-save").click();
  await expect(page.getByTestId("catalog-row-SRG-04")).toContainText("کشیدن دندان شیری");
});

test("clinical lists: the clinic edits the checklist, medicines and the template texts", async () => {
  await page.getByTestId("tab-clinical").click();
  await expect(page.getByTestId("question-row-heart_disease")).toBeVisible();
  await shot("02-clinical-lists");
  await page.getByTestId("question-new").click();
  await page.getByTestId("question-label-fa").fill("سابقه کرونا");
  await page.getByTestId("question-alert").check({ force: true });
  await page.getByTestId("question-save").click();
  await expect(page.getByTestId("question-list")).toContainText("سابقه کرونا");

  await page.getByTestId("clinical-tab-drugs").click();
  await page.getByTestId("drug-new").click();
  await page.getByTestId("drug-name").fill("Benzydamine mouthwash");
  await page.getByTestId("drug-form").selectOption("mouthwash");
  await page.getByTestId("drug-times").fill("25");
  await page.getByTestId("drug-save").click();
  await expect(page.getByTestId("drug-dialog").getByTestId("field-error")).toHaveText("تعداد بار در روز ۱ تا ۱۲ و تعداد روز ۱ تا ۳۶۵ است.");
  await page.getByTestId("drug-times").fill("3");
  await page.getByTestId("drug-class-antiseptic").click();
  await page.getByTestId("drug-save").click();
  await expect(page.getByTestId("drug-list")).toContainText("Benzydamine mouthwash");

  // A built-in template rewritten by the clinic is marked, and goes back to the default on request.
  await page.getByTestId("clinical-tab-templates").click();
  await page.getByTestId("tpl-edit-post_op_extraction").click();
  await page.getByTestId("tpl-body-fa").fill("متن کلینیک برای بعد از کشیدن دندان");
  await page.getByTestId("tpl-save").click();
  await expect(page.getByTestId("tpl-row-post_op_extraction")).toContainText("ویرایش‌شده");
  await page.getByTestId("tpl-edit-post_op_extraction").click();
  page.once("dialog", (d) => d.accept());
  await page.getByTestId("tpl-reset").click();
  await expect(page.getByTestId("tpl-row-post_op_extraction")).not.toContainText("ویرایش‌شده");
});

test("5.9: a prescription warns about the checklist, is numbered, prints at A5 and reprints with the same number", async () => {
  // A new patient (a woman, so the pregnancy question shows), allergic to penicillin and 4 months pregnant.
  await page.getByTestId("tab-patients").click();
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-full-name").fill("بیمار نسخه آزمایشی");
  await page.getByTestId("patient-phone").fill("0700900055");
  await page.getByTestId("patient-gender").selectOption({ label: "زن" });
  await page.getByTestId("patient-language").selectOption("ps");
  await page.getByTestId("patient-save").click();
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();
  await page.getByTestId("patient-tab-medical").click();
  await page.getByTestId("mh-allergy_penicillin-yes").click();
  await page.getByTestId("mh-pregnant-yes").click();
  await page.getByTestId("mh-pregnant-detail").fill("12");
  await page.getByTestId("mh-save").click();
  await expect(page.getByTestId("mh-q-pregnant").getByTestId("field-error")).toHaveText("تعداد ماه باید عددی بین ۱ تا ۱۰ باشد.");
  await page.getByTestId("mh-pregnant-detail").fill("4");
  await page.getByTestId("mh-rest-no").click();
  await page.getByTestId("mh-save").click();
  await expect(page.getByTestId("medical-alert-banner")).toContainText("حامله");
  await shot("03-medical-history");

  await page.getByTestId("patient-tab-prescriptions").click();
  await page.getByTestId("rx-new").click();
  await expect(page.getByTestId("rx-language-ps")).toHaveAttribute("aria-checked", "true");
  await page.getByTestId("rx-doctor").selectOption({ label: "داکتر احمد رحیمی" });
  await page.getByTestId("rx-template-tooth_infection").click();
  await expect(page.getByTestId("rx-lines").locator(".rx-line")).toHaveCount(3);
  await expect(page.getByTestId("rx-warning-penicillin_allergy:0")).toBeVisible();
  await expect(page.getByTestId("rx-warning-nsaid_pregnancy:2")).toBeVisible();
  // Instructions are written in the patient's language (Pashto here).
  await expect(page.getByTestId("rx-line-0-sentence")).toHaveText("ورځ کې ۳ ځله، هر ځل ۱ کپسول، له ډوډۍ وروسته، ۵ ورځې");
  await shot("04-prescription-editor");
  await page.getByTestId("rx-issue").click();
  await expect(page.getByTestId("rx-unacked")).toBeVisible();
  for (const key of ["penicillin_allergy:0", "pregnancy_caution:1", "nsaid_pregnancy:2"]) await page.getByTestId(`rx-ack-${key}`).check({ force: true });
  await page.getByTestId("rx-issue").click();
  await expect(toast()).toContainText(/نسخه RX-\d{4}-000001 صادر شد\./);

  // The preview opens: a real A5 sheet; the PDF of it is exactly 148 × 210 mm.
  const preview = page.getByTestId("doc-preview");
  await expect(preview.locator(".doc-sheet")).toHaveAttribute("data-paper", "a5");
  await expect(preview.getByTestId("doc-rx-items")).toContainText("Amoxicillin");
  expect(await pdfPageMm()).toEqual({ w: 148, h: 210 });
  await shot("05-prescription-preview");
  // On a printer without A5 paper the same sheet goes on A4 at real size; the PDF stays A5.
  await page.getByTestId("print-small-compact").click();
  expect(await pdfPageMm()).toEqual({ w: 210, h: 297 });
  await page.getByTestId("print-small-native").click();

  await page.getByTestId("doc-pdf").click();
  await page.keyboard.press("Escape");
  const row = page.locator('[data-testid^="doc-row-RX-"]').first();
  await expect(row).toContainText("۱ بار");
  // A reprint keeps the number and counts the copy.
  await row.locator('[data-testid^="doc-open-"]').click();
  await page.getByTestId("doc-pdf").click();
  await page.keyboard.press("Escape");
  await expect(row).toContainText("۲ بار");
  // Voiding keeps it in the record, marked, with the reason.
  await row.locator('[data-testid^="doc-void-open-"]').click();
  await page.getByTestId("doc-void-confirm").click();
  await expect(page.getByTestId("doc-void-dialog").getByTestId("field-error")).toBeVisible();
  await page.getByTestId("doc-void-reason").fill("دوز اشتباه نوشته شد");
  await page.getByTestId("doc-void-confirm").click();
  await expect(row).toContainText("باطل‌شده");
});

test("5.10 / 5.10b: a consent form from its template and a medical certificate with rest", async () => {
  await page.getByTestId("patient-tab-documents").click();
  await page.getByTestId("doc-new").click();
  await page.getByTestId("doc-new-consent").click();
  await page.getByTestId("doc-template").selectOption({ label: "رضایت‌نامه کشیدن دندان" });
  await page.getByTestId("doc-teeth").fill("38");
  // Pashto, the patient's language; the template's placeholders are filled in.
  await expect(page.getByTestId("doc-body")).toHaveValue(/زه بیمار نسخه آزمایشی/);
  await expect(page.getByTestId("doc-body")).toHaveValue(/38/);
  await page.getByTestId("doc-issue").click();
  await expect(toast()).toContainText(/سند CF-\d{4}-000001 صادر شد\./);
  await expect(page.getByTestId("doc-preview").locator(".doc-sheet")).toHaveAttribute("data-paper", "a4");
  expect(await pdfPageMm()).toEqual({ w: 210, h: 297 });
  await shot("06-consent");
  await page.keyboard.press("Escape");

  await page.getByTestId("doc-new").click();
  await page.getByTestId("doc-new-certificate").click();
  await page.getByTestId("doc-language-fa").click();
  await page.getByTestId("doc-rest-days").fill("90");
  await page.getByTestId("doc-issue").click();
  await expect(page.getByTestId("doc-editor").getByTestId("field-error")).toHaveText("تعداد روزهای استراحت باید بین ۱ تا ۶۰ باشد.");
  await page.getByTestId("doc-rest-days").fill("3");
  await page.getByTestId("doc-addressee").fill("اداره");
  await page.getByTestId("doc-issue").click();
  await expect(toast()).toContainText(/سند MC-\d{4}-000001 صادر شد\./);
  await expect(page.getByTestId("doc-preview").getByTestId("doc-rest")).toContainText("۳ روز");
  await page.keyboard.press("Escape");
  await expect(page.locator('[data-testid^="doc-row-MC-"]')).toBeVisible();
});

test("M2 / 5.10: booking a service lists its specialty's doctors first and suggests its consent form", async () => {
  await page.getByTestId("tab-appointments").click();
  await page.getByTestId("appt-new").click();
  await page.getByTestId("patient-picker-new").click();
  await page.getByTestId("patient-picker-name-input").fill("بیمار کشیدن دندان");
  await page.getByTestId("patient-picker-phone-input").fill("0700900066");
  await page.getByTestId("patient-picker-create").click();
  await page.getByTestId("appt-service").selectOption({ label: "کشیدن ساده" });
  await expect(page.getByTestId("appt-consent-suggestion")).toBeVisible();
  // No doctor here is an oral surgeon: the chosen one only gets a gentle warning.
  await expect(page.getByTestId("appt-specialty-warning")).toBeVisible();
  await expect(page.getByTestId("appt-reason")).toHaveValue("کشیدن ساده");
  await page.getByTestId("appt-start-hour").selectOption("6");
  await page.getByTestId("appt-start-minute").selectOption("0");
  await page.getByTestId("appt-start-pm").click();
  await page.getByTestId("appt-save").click();
  await expect(toast()).toHaveText("نوبت ثبت شد.");
  // Today's queue reminds reception to ask a new patient's medical history.
  await page.getByTestId("tab-queue").click();
  const card = page.locator('[data-testid^="queue-card-"]').filter({ hasText: "بیمار کشیدن دندان" });
  await expect(card.locator('[data-testid^="queue-review-"]')).toBeVisible();
  await shot("07-queue-review");
});

test("settings: print and clinical rules", async () => {
  await page.getByTestId("tab-settings").click();
  await expect(page.getByTestId("setting-brand-footer")).toBeChecked();
  await expect(page.getByTestId("setting-restrict-specialty")).not.toBeChecked();
  await page.getByTestId("setting-brand-footer").scrollIntoViewIfNeeded();
  await shot("08-settings-print");
});
