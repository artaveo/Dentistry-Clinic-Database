import { expect, test, type Page } from "@playwright/test";

// Phase 4: staff, doctors, appointments and the reception workflow against the real Core.
// Runs after app.spec.ts and patients.spec.ts in the same shared clinic (single worker), so
// the owner already exists with the password recovered at the end of app.spec.ts.
// The scenario is "one full reception day": a doctor and a chair, bookings and a refused double
// booking, arrival → treatment → completion with a follow-up, a walk-in, a no-show, the call list.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };
const toast = () => page.getByTestId("toast").last();

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage({ viewport: { width: 1360, height: 900 } });
  await page.goto("/");
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-doctors")).toBeVisible();
});

async function pickNewPatient(prefix: string, name: string, phone: string) {
  await page.getByTestId(`${prefix}-new`).click();
  await page.getByTestId(`${prefix}-name-input`).fill(name);
  await page.getByTestId(`${prefix}-phone-input`).fill(phone);
  await page.getByTestId(`${prefix}-create`).click();
  await expect(page.getByTestId(`${prefix}-name`)).toHaveText(name);
}

test("doctors and chairs: add them, validate live, set a schedule and a leave", async () => {
  await page.getByTestId("tab-doctors").click();
  await expect(page.getByTestId("doctor-list")).toBeVisible();

  await page.getByTestId("doctor-add-open").click();
  await page.getByTestId("doctor-name").fill("   ");
  await page.getByTestId("doctor-name").blur();
  await expect(page.getByTestId("field-error")).toBeVisible(); // OF-001..003: red before pressing save
  await page.getByTestId("doctor-name").fill("داکتر احمد رحیمی");
  await page.getByTestId("doctor-specialty").fill("ارتودانسی");
  await page.getByTestId("doctor-color-7c3aed").click();
  await page.getByTestId("doctor-save").click();
  await expect(toast()).toHaveText("تغییرات ذخیره شد.");

  await page.getByTestId("doctor-add-open").click();
  await page.getByTestId("doctor-name").fill("داکتر فرید سلطانی");
  await page.getByTestId("doctor-save").click();
  await expect(page.getByTestId("doctor-list")).toContainText("داکتر فرید سلطانی");

  await page.getByTestId("chair-add-open").click();
  await page.getByTestId("chair-name").fill("چوکی ۱");
  await page.getByTestId("chair-save").click();
  await page.getByTestId("chair-add-open").click();
  await page.getByTestId("chair-name").fill("چوکی ۱");
  await page.getByTestId("chair-save").click();
  await expect(page.getByTestId("field-error")).toContainText("چوکی‌ای با این نام وجود دارد"); // Core error under the field
  await page.getByTestId("chair-name").fill("چوکی ۲");
  await page.getByTestId("chair-save").click();
  await expect(page.getByTestId("chair-list")).toContainText("چوکی ۲");

  // The second doctor works Saturday mornings only; the first has no hour limit.
  await page.getByTestId("doctor-schedule-داکتر فرید سلطانی").click();
  await page.getByTestId("hours-0-add").click();
  await page.getByTestId("leave-reason").fill("سفر");
  await page.getByTestId("leave-add").click();
  await expect(page.getByTestId("sched-leaves")).toContainText("سفر");
  await page.getByTestId("schedule-save").click();
  await expect(toast()).toHaveText("تغییرات ذخیره شد.");
  // Remove the leave again so it does not block the doctor later in this scenario.
  await page.getByTestId("doctor-schedule-داکتر فرید سلطانی").click();
  await page.getByTestId("sched-leaves").locator("li button").first().click();
  await expect(page.getByTestId("sched-leaves")).not.toContainText("سفر");
  await page.getByTestId("schedule-save").click();
});

test("book an appointment with a patient registered on the spot; a double booking is refused", async () => {
  await page.getByTestId("tab-appointments").click();
  await expect(page.getByTestId("page-calendar")).toBeVisible();
  await page.getByTestId("appt-new").click();
  await pickNewPatient("patient-picker", "بیمار آزمایشی یکم", "0700900001");
  await page.getByTestId("appt-doctor").selectOption({ label: "داکتر احمد رحیمی — ارتودانسی" });
  await page.getByTestId("appt-reason").fill("کنترول");
  await page.getByTestId("appt-save").click();
  await expect(toast()).toHaveText("نوبت ثبت شد.");
  await expect(page.locator('[data-testid^="cal-appt-"]')).toHaveCount(1);
  await expect(page.locator('[data-testid^="cal-appt-"]').first()).toContainText("بیمار آزمایشی یکم");

  // Same doctor, same time, another patient → the exact reason under the doctor field.
  await page.getByTestId("appt-new").click();
  await pickNewPatient("patient-picker", "بیمار آزمایشی دوم", "0700900002");
  await page.getByTestId("appt-doctor").selectOption({ label: "داکتر احمد رحیمی — ارتودانسی" });
  await page.getByTestId("appt-save").click();
  await expect(page.getByTestId("field-error")).toHaveText("داکتر در این زمان نوبت دیگری دارد.");
  // Another doctor is free and works Saturdays only: at 9 PM the schedule objects, reception may override.
  await page.getByTestId("appt-doctor").selectOption({ label: "داکتر فرید سلطانی" });
  await page.getByTestId("appt-start-hour").selectOption("9");
  await page.getByTestId("appt-start-pm").click();
  await page.getByTestId("appt-save").click();
  await expect(page.getByTestId("appt-override")).toBeVisible();
  await page.getByTestId("appt-override-confirm").click();
  await expect(toast()).toHaveText("نوبت ثبت شد.");
  await expect(page.locator('[data-testid^="cal-appt-"]')).toHaveCount(2);
});

test("calendar views: week and month show the visits; the month counts them", async () => {
  await page.getByTestId("cal-view-week").click();
  await expect(page.getByTestId("cal-grid-week")).toBeVisible();
  await expect(page.locator('[data-testid^="cal-appt-"]')).toHaveCount(2);
  await page.getByTestId("cal-view-month").click();
  await expect(page.getByTestId("cal-grid-month")).toBeVisible();
  await expect(page.locator('[data-testid^="cal-count-"]').first()).toContainText("۲");
  await page.getByTestId("cal-view-day").click();
  await page.getByTestId("cal-by-chair").click();
  await expect(page.getByTestId("cal-grid-day")).toBeVisible();
  await page.getByTestId("cal-by-doctor").click();
});

test("the appointment card shows the visit and can be printed", async () => {
  await page.locator('[data-testid^="cal-appt-"]').first().click();
  await expect(page.getByTestId("appointment-dialog")).toBeVisible();
  await page.getByTestId("appt-print").click();
  await expect(page.getByTestId("appointment-card")).toContainText("بیمار آزمایشی یکم");
  await page.getByTestId("appt-card-close").click();
  await expect(page.getByTestId("appointment-card-dialog")).toHaveCount(0);
  await expect(page.getByTestId("appointment-dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("appointment-dialog")).toHaveCount(0);
});

test("queue: arrival gets a number, treatment, completion with a follow-up", async () => {
  await page.getByTestId("tab-queue").click();
  await expect(page.getByTestId("queue-count-expected")).toHaveText("۲");
  const card = page.getByTestId("queue-expected").locator('[data-testid^="queue-card-"]').filter({ hasText: "بیمار آزمایشی یکم" });
  await card.getByTestId("appt-action-checked_in").click();
  await expect(page.getByTestId("queue-count-waiting")).toHaveText("۱");
  const waiting = page.getByTestId("queue-waiting").locator('[data-testid^="queue-card-"]');
  await expect(waiting.locator('[data-testid^="queue-ticket-"]')).toHaveText("۱");
  await waiting.getByTestId("appt-action-in_treatment").click();
  await expect(page.getByTestId("queue-count-treating")).toHaveText("۱");
  await page.getByTestId("queue-treating").getByTestId("appt-action-completed").click();
  await page.getByTestId("followup-toggle").check({ force: true });
  await page.getByTestId("followup-week2").click();
  await page.getByTestId("followup-repeat").fill("6");
  await page.getByTestId("appt-complete-confirm").click();
  await expect(toast()).toHaveText("ویزیت تکمیل شد.");
  await expect(page.getByTestId("queue-count-done")).toHaveText("۱");
});

test("walk-in joins the queue behind the others; a no-show leaves it", async () => {
  await page.getByTestId("walk-in-open").click();
  await pickNewPatient("walkin-patient", "بیمار بدون نوبت", "0700900003");
  await page.getByTestId("walk-in-doctor").selectOption({ label: "داکتر احمد رحیمی" });
  await page.getByTestId("walk-in-reason").fill("درد دندان");
  await page.getByTestId("walk-in-confirm").click();
  await expect(page.getByTestId("queue-count-waiting")).toHaveText("۱");
  await expect(page.getByTestId("queue-waiting").locator('[data-testid^="queue-ticket-"]')).toHaveText("۲");
  await expect(page.getByTestId("queue-waiting")).toContainText("بدون نوبت");

  const expected = page.getByTestId("queue-expected").locator('[data-testid^="queue-card-"]');
  await expect(expected).toHaveCount(1);
  await expected.getByTestId("appt-action-no_show").click();
  await page.getByTestId("appt-reason-confirm").click();
  await expect(toast()).toHaveText("به‌عنوان نیامده ثبت شد.");
  await expect(page.getByTestId("queue-closed")).toBeVisible();
});

test("call list: the follow-up and the no-show are there; record a call, then book from the list", async () => {
  await page.getByTestId("tab-recalls").click();
  const list = page.getByTestId("recall-list");
  await expect(list).toContainText("بیمار آزمایشی یکم");
  await expect(list).toContainText("بیمار آزمایشی دوم");
  await expect(list).toContainText("هر ۶ ماه");

  const noShowRow = list.locator("tbody tr").filter({ hasText: "بیمار آزمایشی دوم" });
  await noShowRow.locator('[data-testid^="recall-contact-"]').click();
  await page.getByTestId("recall-contact-note").fill("می‌آید");
  await page.getByTestId("recall-contact-confirm").click();
  await expect(toast()).toHaveText("تماس ثبت شد.");
  await expect(list.locator("tbody tr").filter({ hasText: "بیمار آزمایشی دوم" })).toContainText("می‌آید");

  await noShowRow.locator('[data-testid^="recall-book-"]').click();
  await expect(page.getByTestId("appointment-dialog")).toBeVisible();
  await expect(page.getByTestId("patient-picker-name")).toHaveText("بیمار آزمایشی دوم");
  await page.getByTestId("appt-doctor").selectOption({ label: "داکتر احمد رحیمی — ارتودانسی" });
  await page.getByTestId("appt-start-hour").selectOption("11");
  await page.getByTestId("appt-save").click();
  await expect(toast()).toHaveText("نوبت ثبت شد.");
  await expect(list.locator("tbody tr").filter({ hasText: "بیمار آزمایشی دوم" })).toHaveCount(0); // booked: off the call list
  await page.getByTestId("recall-tab-booked").click();
  await expect(page.getByTestId("recall-list")).toContainText("بیمار آزمایشی دوم");
});

test("patient profile lists the visits and recalls of the patient", async () => {
  await page.getByTestId("tab-patients").click();
  await page.getByTestId("patients-search").fill("بیمار آزمایشی یکم");
  await page.getByTestId("patient-list").getByText("بیمار آزمایشی یکم").click();
  await page.getByTestId("patient-tab-appointments").click();
  await expect(page.getByTestId("patient-appointments")).toContainText("داکتر احمد رحیمی");
  await expect(page.getByTestId("patient-appointments")).toContainText("تمام‌شده");
  await expect(page.getByTestId("patient-recalls")).toContainText("کنترول بعد از درمان");
  await page.getByTestId("patient-back").click();
});

test("roles: a custom role limits what its users see; reset password and unlock", async () => {
  await page.getByTestId("tab-users").click();
  await page.getByTestId("role-add-open").click();
  await page.getByTestId("role-name").fill("پذیرش شبانه");
  await page.getByTestId("role-save").click();
  await expect(page.getByTestId("field-error")).toBeVisible(); // no permission chosen yet
  for (const p of ["patients.view", "appointments.view", "appointments.edit"]) await page.getByTestId(`perm-${p}`).check({ force: true });
  await page.getByTestId("role-save").click();
  await expect(toast()).toHaveText("تغییرات ذخیره شد.");
  await expect(page.getByTestId("role-list")).toContainText("پذیرش شبانه");

  await page.getByTestId("add-user-open").click();
  await page.getByTestId("new-display").fill("پذیرش شب");
  await page.getByTestId("new-username").fill("night1");
  await page.getByTestId("new-password").fill("night-pass-12");
  await page.getByTestId("new-role").selectOption({ label: "پذیرش شبانه" });
  await page.getByTestId("create-user").click();
  await expect(page.getByTestId("user-list")).toContainText("پذیرش شبانه");

  await page.getByTestId("edit-user-night1").click();
  await page.getByTestId("user-reset-open").click();
  await page.getByTestId("reset-password-input").fill("short");
  await page.getByTestId("reset-password-input").blur();
  await expect(page.getByTestId("field-error")).toBeVisible();
  await page.getByTestId("reset-password-input").fill("brand-new-pass-1");
  await page.getByTestId("reset-password-confirm").click();
  await expect(toast()).toHaveText("رمز عبور بازنشانی شد.");
  await page.getByTestId("user-unlock").click();
  await expect(toast()).toHaveText("قفل حساب برداشته شد.");
  await page.getByTestId("edit-user-dialog").getByRole("button", { name: "لغو" }).click();

  // Sign in as that user: reception pages yes, doctors and users no.
  await page.getByTestId("current-user").click();
  await page.getByTestId("logout").click();
  await page.getByTestId("login-username").fill("night1");
  await page.getByTestId("login-password").fill("brand-new-pass-1");
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-appointments")).toBeVisible();
  await expect(page.getByTestId("tab-queue")).toBeVisible();
  for (const tab of ["tab-doctors", "tab-users", "tab-audit", "tab-clinic"]) await expect(page.getByTestId(tab)).toHaveCount(0);
  await page.getByTestId("tab-queue").click();
  await expect(page.getByTestId("walk-in-open")).toBeVisible();
  await expect(page.getByTestId("appt-action-in_treatment")).toHaveCount(0); // booking yes, treatment no
  await page.getByTestId("current-user").click();
  await page.getByTestId("logout").click();
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-doctors")).toBeVisible();
});

test("import: map the columns of a file with Dari headers in another order", async () => {
  await page.getByTestId("tab-patients").click();
  await page.getByTestId("patients-import-open").click();
  const csv = "شماره;نام و تخلص;نام پدر\n0700555111;وارد شده با نگاشت;کریم\n12;شماره بد;\n";
  await page.getByTestId("import-file-input").setInputFiles({ name: "bimaran.csv", mimeType: "text/csv", buffer: Buffer.from(csv, "utf-8") });
  await expect(page.getByTestId("import-mapping")).toBeVisible();
  await expect(page.getByTestId("import-map-0")).toHaveValue("phone"); // guessed from the Dari header
  await expect(page.getByTestId("import-map-1")).toHaveValue("full_name");
  await expect(page.getByTestId("import-map-2")).toHaveValue("father_name");
  await expect(page.getByTestId("import-errors")).toContainText("شماره تماس باید");
  await page.getByTestId("import-map-1").selectOption("");
  await expect(page.getByTestId("import-no-name")).toBeVisible();
  await expect(page.getByTestId("import-commit")).toBeDisabled();
  await page.getByTestId("import-map-1").selectOption("full_name");
  await expect(page.getByTestId("import-summary")).toContainText("1");
  await page.getByTestId("import-commit").click();
  await expect(toast()).toHaveText("1 بیمار با موفقیت وارد شد.");
});

test("attachment thumbnails come from the Core (OF-018)", async () => {
  await page.getByTestId("patients-search").fill("بیمار آزمایشی یکم");
  await page.getByTestId("patient-list").getByText("بیمار آزمایشی یکم").click();
  await page.getByTestId("patient-tab-documents").click();
  await page.getByTestId("attachment-add-open").click();
  const png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
  await page.getByTestId("attachment-file-input").setInputFiles({ name: "xray.png", mimeType: "image/png", buffer: Buffer.from(png, "base64") });
  await page.getByTestId("attachment-upload-submit").click();
  const thumb = page.locator(".attachment-thumb img").first();
  await expect(thumb).toBeVisible();
  await expect(thumb).toHaveAttribute("src", /^data:image\/jpeg;base64,/); // the Core's JPEG thumbnail, not the original PNG
});
