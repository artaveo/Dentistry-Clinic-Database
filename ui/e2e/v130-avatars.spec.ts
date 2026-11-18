import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { newClinicPage } from "./clinicClock";

// OF-049..OF-051 (pictures): default picture by sex and age, a real photo set and removed from the
// patient profile, the doctor's sex and own page, and the unit tiles with their status. The Core side
// (storage, limits, permissions, merge, backup) is tested in Rust (`core/src/tests_avatars.rs`).
// Runs after v120 in the shared clinic; the doctors and units were made in reception.spec.ts.
test.describe.configure({ mode: "serial" });

let page: Page;
const OWNER = { user: "owner", pass: "recovered-pass-1" };
const RPC = "http://127.0.0.1:8787/rpc";
// A tiny valid PNG: the browser crops and shrinks it, the Core shrinks it again and stores it encrypted.
const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==", "base64");

async function call(request: APIRequestContext, method: string, params: object, as: string | null) {
  const r = await request.post(RPC, { data: { method, params, token: as } });
  return r.json();
}

test.beforeAll(async ({ browser }) => {
  page = await newClinicPage(browser, { width: 1360, height: 900 });
  await page.goto("/");
  await page.getByTestId("login-username").fill(OWNER.user);
  await page.getByTestId("login-password").fill(OWNER.pass);
  await page.getByTestId("login-submit").click();
  await expect(page.getByTestId("tab-patients")).toBeVisible();
});

test("a girl gets the girl picture; a photo can be set from a file and removed again", async ({ request }) => {
  const token = (await call(request, "auth.login", { username: OWNER.user, password: OWNER.pass }, null)).result.token;
  const genders = (await call(request, "reference.list", { type_code: "gender", language: "fa" }, token)).result as { id: string; code: string }[];
  const female = genders.find((g) => g.code === "female")!.id;
  const created = await call(request, "patients.create", { full_name: "سارا تصویری", gender_id: female, approximate_age: 8, phone: "0700913001" }, token);
  const number = created.result.patient_number as string;

  await page.getByTestId("tab-patients").click();
  await page.getByTestId("patients-search").fill("سارا تصویری");
  const avatar = page.getByTestId(`patient-avatar-${number}`);
  await expect(avatar).toHaveAttribute("data-avatar", "art");
  await expect(avatar.locator("img")).toHaveAttribute("src", /patient-girl/);

  await page.getByTestId(`patient-row-${number}`).click();
  await expect(page.getByTestId("patient-header-avatar")).toHaveAttribute("data-avatar", "art");
  await page.getByTestId("patient-photo-edit").click();
  await expect(page.getByTestId("photo-dialog")).toBeVisible();
  await expect(page.getByTestId("photo-remove")).toHaveCount(0); // nothing to remove yet
  await page.getByTestId("photo-file").setInputFiles({ name: "sara.png", mimeType: "image/png", buffer: PNG });
  await expect(page.getByTestId("photo-pending")).toBeVisible();
  await page.getByTestId("photo-save").click();
  await expect(page.getByTestId("toast").last()).toHaveText("عکس ذخیره شد");
  await expect(page.getByTestId("patient-header-avatar")).toHaveAttribute("data-avatar", "photo");
  await expect(page.getByTestId("patient-overview-avatar")).toHaveAttribute("data-avatar", "photo");

  // The photo is stored by the Core (encrypted) and served back.
  const got = await call(request, "patients.get", { patient_id: created.result.id }, token);
  expect(got.result.photo_id).toBeTruthy();
  const photo = await call(request, "photos.get", { id: got.result.photo_id }, token);
  expect(photo.result.data_url).toMatch(/^data:image\/jpeg;base64,/);

  // Not an image: refused by the browser before anything is sent.
  await page.getByTestId("patient-photo-edit").click();
  await page.getByTestId("photo-file").setInputFiles({ name: "x.png", mimeType: "image/png", buffer: Buffer.from("not an image") });
  await expect(page.getByTestId("photo-error")).toBeVisible();
  // Remove: back to the default picture.
  await page.getByTestId("photo-remove").click();
  await expect(page.getByTestId("toast").last()).toContainText("عکس حذف شد");
  await expect(page.getByTestId("patient-header-avatar")).toHaveAttribute("data-avatar", "art");
  await page.getByTestId("patient-back").click();
});

test("doctors: the list opens the doctor's page; the sex picks the picture; units show their status", async () => {
  await page.getByTestId("tab-settings").click();
  await page.getByTestId("tab-doctors").click();
  await expect(page.getByTestId("doctor-list")).toBeVisible();
  // OF-050: no separate edit / schedule buttons in the list any more.
  await expect(page.getByTestId("doctor-list").getByRole("button")).toHaveCount(0);

  await page.getByTestId("doctor-open-داکتر احمد رحیمی").click();
  await expect(page.getByTestId("page-doctor")).toBeVisible();
  await expect(page.getByTestId("doctor-week")).toBeVisible();
  await page.getByTestId("doctor-edit-داکتر احمد رحیمی").click();
  await page.getByTestId("doctor-sex-male").click();
  await page.getByTestId("doctor-save").click();
  await expect(page.getByTestId("toast").last()).toHaveText("تغییرات ذخیره شد.");
  await expect(page.getByTestId("doctor-page-figure")).toBeVisible();

  await page.getByTestId("doctor-tab-units").click();
  await expect(page.getByTestId("doctor-units")).toContainText("یونیت ۱");
  await page.getByTestId("doctor-tab-stats").click();
  await expect(page.getByText("کارهای ۹۰ روز گذشته بر اساس دسته خدمات")).toBeVisible();

  await page.getByTestId("doctor-back").click();
  const row = page.getByTestId("doctor-list").locator("tr", { hasText: "داکتر احمد رحیمی" });
  await expect(row.locator("[data-avatar]")).toHaveAttribute("data-avatar", "art");
  // OF-051: each unit has a picture and a live status.
  await expect(page.getByTestId("unit-یونیت ۱")).toBeVisible();
  await expect(page.getByTestId("unit-status-یونیت ۱")).toBeVisible();
});
