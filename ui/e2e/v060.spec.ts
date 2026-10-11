import { expect, test, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// v0.6.0 (Phase 5B: dental chart, treatment plans, multi-session treatments, clinical notes, perio chart,
// timeline) against the real Core. Runs after the earlier files in the same clinic and session (single
// worker, one dev server), so it only logs in. The suite's clinic clock is 10:00 today.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };

test.beforeAll(async ({ browser }) => {
  page = await newClinicPage(browser, { width: 1360, height: 900 });
  // Signing a note, completing an item and the like ask "are you sure?" first.
  page.on("dialog", (d) => d.accept());
  await page.goto("/");
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
});

const toast = () => page.getByTestId("toast").last();

async function shot(name: string) {
  if (!process.env.SHOTS) return;
  await page.waitForTimeout(400);
  await page.screenshot({ path: `../docs/testing/img/v0.6.0/${name}.png`, style: ".toasts { display: none !important; }" });
}

test("M5: tooth first — a finding on 36 with its surfaces, then a root canal added to a new plan", async () => {
  await page.getByTestId("tab-patients").click();
  await page.getByTestId("add-patient-open").click();
  await page.getByTestId("patient-full-name").fill("بیمار چارت آزمایشی");
  await page.getByTestId("patient-phone").fill("0700900077");
  await page.getByTestId("patient-save").click();
  await expect(page.getByTestId("page-patient-profile")).toBeVisible();
  await page.getByTestId("patient-tab-chart").click();
  await expect(page.getByTestId("odontogram")).toBeVisible();
  // The adult chart: 32 teeth, FDI numbers in the interface's digits.
  await expect(page.getByTestId("tooth-num-36")).toHaveText("۳۶");

  await page.getByTestId("tooth-36").click();
  await expect(page.getByTestId("chart-selection")).toContainText("۳۶");
  await expect(page.getByTestId("tooth-details-36")).toBeVisible();
  // An existing condition: caries needs surfaces, picked on the tooth's biting surface.
  await page.getByTestId("chart-mode-finding").click();
  await page.getByTestId("condition-caries").click();
  await expect(page.getByTestId("finding-need-surface")).toBeVisible();
  await expect(page.getByTestId("finding-save")).toBeDisabled();
  await page.getByTestId("surface-36-O").click();
  await page.getByTestId("surface-36-D").click();
  await expect(page.getByTestId("chart-selection")).toContainText("OD");
  await page.getByTestId("finding-save").click();
  await expect(toast()).toHaveText("وضعیت دندان ثبت شد.");
  await expect(page.getByTestId("tooth-details-36")).toContainText("پوسیدگی");
  await expect(page.getByTestId("surface-36-O")).toHaveAttribute("data-state", "existing");

  // Then the work: a two-canal root canal from the catalog, priced from it, into a new plan.
  await page.getByTestId("chart-mode-plan").click();
  await page.getByTestId("item-service-search").fill("END-01D");
  await page.getByTestId("item-service-END-01D").click();
  await expect(page.getByTestId("picked-service")).toContainText("۳۶");
  await expect(page.getByTestId("item-price")).toHaveValue("3500");
  await expect(page.getByTestId("item-sessions")).toHaveValue("2");
  await page.getByTestId("item-add").click();
  await expect(toast()).toHaveText("به پلان درمان اضافه شد.");
  // A label from the tooth: number — work — sessions — status.
  await expect(page.getByTestId("odontogram")).toContainText("دوکاناله");
  await shot("01-chart-tooth-first");
});

test("M5: service first — a whole-mouth service needs no teeth; quadrant buttons select teeth", async () => {
  await page.getByTestId("chart-clear").click();
  await page.getByTestId("chart-quick-q3").click();
  await expect(page.getByTestId("chart-selection")).toContainText("۳۱–۳۸");
  await page.getByTestId("chart-clear").click();
  await page.getByTestId("item-service-search").fill("PER-01");
  await page.getByTestId("item-service-PER-01").click();
  await expect(page.getByTestId("item-teeth-problem")).toHaveCount(0);
  await page.getByTestId("item-add").click();
  await expect(toast()).toHaveText("به پلان درمان اضافه شد.");
  await expect(page.getByTestId("chart-whole-mouth")).toContainText("جرم‌گیری");
  // A per-tooth service picked first asks for its teeth.
  await page.getByTestId("item-service-search").fill("RST-01C");
  await page.getByTestId("item-service-RST-01C").click();
  await expect(page.getByTestId("item-teeth-problem")).toBeVisible();
  // The specialty views keep only their own work.
  await page.getByTestId("chart-view-ortho").click();
  await expect(page.getByTestId("odontogram")).not.toContainText("دوکاناله");
  await page.getByTestId("chart-view-general").click();
  await expect(page.getByTestId("odontogram")).toContainText("دوکاناله");
});

test("5.7: plans — totals from the catalog, an alternative from a ready plan, accepting one, the printed estimate", async () => {
  await page.getByTestId("patient-tab-plans").click();
  await expect(page.getByTestId("plans-tab")).toBeVisible();
  const first = page.locator(".plan-card").filter({ hasText: "دوکاناله" });
  await expect(first.locator('[data-testid^="plan-total-"]')).toContainText("۳٬۵۰۰");
  // An alternative from the ready plan "root canal and crown" for 36.
  await page.getByTestId("plan-new").click();
  await page.getByTestId("plan-title").fill("گزینه ب — روکش");
  await page.getByTestId("plan-template").selectOption({ label: "عصب‌کشی و روکش" });
  await expect(page.getByTestId("plan-template-preview")).toBeVisible();
  await page.getByTestId("plan-create").click();
  await expect(page.getByTestId("plan-new-dialog").getByTestId("field-error")).toBeVisible();
  await page.getByTestId("plan-teeth").fill("36");
  await page.getByTestId("plan-create").click();
  await expect(toast()).toHaveText("پلان درمان ساخته شد.");
  const second = page.locator(".plan-card").filter({ hasText: "گزینه ب — روکش" });
  await expect(second).toContainText("روکش");

  // Accepting the first sets the other aside (kept, not deleted).
  await first.locator('[data-testid^="plan-accept-"]').click();
  await page.getByTestId("plan-accept-dialog-confirm").click();
  await expect(toast()).toHaveText("پلان پذیرفته شد.");
  await expect(first.locator('[data-testid^="plan-status-"]')).toHaveText("پذیرفته‌شده");
  await expect(second).toHaveCount(0);
  await page.getByTestId("plan-show-closed").click();
  await expect(page.locator(".plan-card").filter({ hasText: "گزینه ب — روکش" })).toBeVisible();
  await shot("02-plans");

  // The estimate: a numbered A4 document with the plan frozen in it.
  await first.locator('[data-testid^="plan-print-"]').click();
  await expect(page.getByTestId("estimate-dialog")).toBeVisible();
  await page.getByTestId("estimate-issue").click();
  const preview = page.getByTestId("estimate-preview");
  await expect(preview).toContainText(/TP-\d{4}-000001/);
  await expect(preview.getByTestId("doc-plan")).toContainText("دوکاناله");
  await shot("03-estimate");
  await page.keyboard.press("Escape");
});

test("M4: a session recorded, the next one suggested and booked — the visit shows «جلسه ۲ از ۲»", async () => {
  const row = page.locator(".item-row").filter({ hasText: "دوکاناله" });
  await row.locator('[data-testid^="item-record-"]').click();
  await expect(page.getByTestId("session-dialog")).toBeVisible();
  await page.getByTestId("session-note").fill("باز کردن حفره و پاکسازی کانال‌ها");
  await page.getByTestId("session-save").click();
  await expect(page.getByTestId("next-suggestion")).toContainText("جلسه ۲ از ۲");
  await page.getByTestId("next-book").click();
  // The booking form comes filled: the treatment, its session number and the suggested day.
  await expect(page.getByTestId("appt-session")).toContainText("۲");
  await expect(page.getByTestId("appt-treatment")).not.toHaveValue("");
  await page.getByTestId("appt-start-hour").selectOption("10");
  await page.getByTestId("appt-start-minute").selectOption("0");
  await page.getByTestId("appt-start-am").click();
  await page.getByTestId("appt-save").click();
  if (await page.getByTestId("appt-override").isVisible().catch(() => false)) await page.getByTestId("appt-override-confirm").click();
  await expect(toast()).toHaveText("نوبت ثبت شد.");
  await expect(row.locator('[data-testid^="item-next-"]')).toBeVisible();
  await expect(row).toContainText("در حال انجام");
  await expect(row).toContainText("جلسه ۲ از ۲");
  await shot("04-sessions");
});

test("5.6: a visit note from a template, signed and locked, then an addendum", async () => {
  await page.getByTestId("patient-tab-notes").click();
  await expect(page.getByTestId("notes-tab")).toBeVisible();
  await page.getByTestId("note-new").click();
  await page.getByTestId("note-save").click();
  await expect(page.getByTestId("note-empty")).toHaveText("حداقل یک بخش یادداشت را بنویسید.");
  await page.getByTestId("note-template-rct_first").click();
  await expect(page.getByTestId("note-procedure")).not.toHaveValue("");
  await page.getByTestId("note-chief_complaint").fill("درد دندان ۳۶ هنگام جویدن");
  await page.getByTestId("note-save-sign").click();
  await expect(toast()).toHaveText("یادداشت امضا و قفل شد.");
  const card = page.locator('[data-testid^="note-status-"]').first();
  await expect(card).toContainText("امضاشده");
  // Signed: no editing or deleting any more, only an addendum.
  await expect(page.locator('[data-testid^="note-edit-"]')).toHaveCount(0);
  await expect(page.locator('[data-testid^="note-delete-"]')).toHaveCount(0);
  await page.locator('[data-testid^="note-addendum-"]').first().click();
  await page.getByTestId("note-addendum-text").fill("بیمار تلفنی گفت درد کمتر شده است.");
  await page.getByTestId("note-addendum-save").click();
  await expect(toast()).toHaveText("افزودنی ثبت شد.");
  await expect(page.locator('[data-testid^="note-addenda-"]').first()).toContainText("درد کمتر شده");
  await shot("05-note-signed");
});

test("5.5: a perio exam — depths, bleeding, the summary", async () => {
  await page.getByTestId("patient-tab-chart").click();
  await page.getByTestId("chart-view-perio").click();
  await expect(page.getByTestId("perio-chart")).toBeVisible();
  await page.getByTestId("perio-depth-16-0").fill("5");
  await page.getByTestId("perio-depth-16-1").fill("4");
  await page.getByTestId("perio-depth-16-2").fill("3");
  await page.getByTestId("perio-bleeding-16-0").click();
  await page.getByTestId("perio-mobility-16").click();
  await expect(page.getByTestId("perio-mobility-16")).toHaveText("۰");
  await page.getByTestId("perio-save").click();
  await expect(toast()).toHaveText("معاینه لثه ذخیره شد.");
  await expect(page.getByTestId("perio-summary")).toBeVisible();
  await shot("06-perio");
  await page.getByTestId("chart-view-general").click();
});

test("5.11: the timeline tells the story, newest first, and filters by kind", async () => {
  await page.getByTestId("patient-tab-timeline").click();
  const timeline = page.getByTestId("timeline");
  await expect(timeline).toBeVisible();
  for (const action of ["finding.added", "plan.created", "plan.accepted", "session.recorded", "note.signed", "perio.recorded", "document.issued"]) {
    await expect(page.getByTestId(`timeline-event-${action}`).first()).toBeVisible();
  }
  await page.getByTestId("timeline-filter-plan").click();
  await expect(page.getByTestId("timeline-event-note.signed")).toHaveCount(0);
  await expect(page.getByTestId("timeline-event-plan.accepted").first()).toBeVisible();
  await page.getByTestId("timeline-filter-plan").click();
  await shot("07-timeline");
});

test("Phase 5 exit: the printed record carries the visit — the chart, the treatment, the signed note", async () => {
  await page.getByTestId("patient-tab-documents").click();
  await page.getByTestId("doc-new").click();
  await page.getByTestId("doc-new-record_summary").click();
  await page.getByTestId("doc-issue").click();
  await expect(toast()).toContainText(/MR-\d{4}-\d{6}/);
  const sheet = page.getByTestId("doc-preview");
  await expect(sheet.getByTestId("doc-summary-findings")).toContainText("پوسیدگی (OD)");
  await expect(sheet.getByTestId("doc-summary-treatments")).toContainText("دوکاناله");
  await expect(sheet.getByTestId("doc-summary-notes")).toContainText("اقدام درمانی");
  await shot("09-record-summary");
  await page.keyboard.press("Escape");
});

test("clinical lists: the doctor edits tooth conditions, note templates and ready plans", async () => {
  await page.getByTestId("tab-clinical").click();
  await page.getByTestId("clinical-tab-conditions").click();
  await expect(page.getByTestId("condition-row-caries")).toContainText("پوسیدگی");
  await page.getByTestId("clinical-tab-notes").click();
  await page.getByTestId("note-template-edit-toothache").click();
  await page.getByTestId("note-template-fa-recommendations").fill("مسکن در صورت نیاز؛ برگشت در صورت تورم.");
  await page.getByTestId("note-template-save").click();
  await expect(page.getByTestId("note-template-row-toothache")).toBeVisible();
  await page.getByTestId("clinical-tab-plans").click();
  await expect(page.getByTestId("plan-template-row-rct_crown")).toContainText("عصب‌کشی و روکش");
  await shot("08-clinical-lists");
});
