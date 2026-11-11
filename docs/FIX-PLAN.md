# برنامه‌ی اجرای اصلاحات ممیزی (FIX-PLAN)

مبنا: `docs/AUDIT-REPORT.md` (نمره‌ی ۶۹). کار روی شاخه‌ی `main`، بدون commit، بدون build/test روی لپ‌تاپ.

## قاعده‌های جلوگیری از تداخل

* **هر فایل فقط یک مالک در هر زمان.** فایل‌های مشترک بزرگ این‌طور شکسته شدند (پایه را مادر ساخت):
  * `ui/src/i18n/parts/<area>.{fa,ps,en}.json` — کلید تازه‌ی هر سشن. هر سه زبان لازم است. کلید `parts` روی فایل‌های اصلی (`fa/ps/en.json`) برتری دارد، پس برای تغییر متن یک کلید قدیمی هم همین‌جا کلید را دوباره تعریف کنید. فایل‌های اصلی `fa/ps/en.json` را فقط سشن آخر (موج ۳) ویرایش می‌کند.
  * `ui/src/styles/<area>.css` — استایل تازه‌ی هر سشن؛ بعد از `styles.css` خودکار بارگذاری می‌شود (`main.tsx`). `styles.css` را فقط سشن آخر (موج ۳) ویرایش می‌کند.
* **`shared/ts/contract.ts` تولیدی است** (از `shared/src/lib.rs` با `cargo test -p artaveo-shared`؛ CI قدیمی‌بودنش را رد می‌کند). چون cargo روی لپ‌تاپ ممنوع است، **هیچ سشنی قرارداد API (`shared/`) را تغییر نمی‌دهد.** اگر کاری قرارداد جدید می‌خواهد، در «باقی‌مانده» نوشته شود.
* تغییر دیتابیس فقط با migration جدید (شماره‌ی بعدی بعد از `0018`)، فقط افزودنی.

## موج ۱ — هم‌زمان (فایل‌ها جدا)

| سشن | فایل‌های مجاز | کار | مدل / effort |
|---|---|---|---|
| **core** | `core/src/inventory.rs`، `core/src/seeds.rs`، `core/seeds/stock_items.json` (جدید)، `core/seeds/README.md`، `core/src/patient.rs`، `core/src/api.rs` (فقط unlock)، `core/src/auth.rs`، تست‌های Rust جدید داخل همان ماژول‌ها | seed اجناس انبار (بخش ۹.۲)؛ B1 ادغام بیمار؛ B3 سقف تلاش قفل صفحه | opus، `high` |
| **base-ui** | `ui/src/ui/*` (DateField، Overlay، Field، Controls، جدید: `DateRangeField.tsx`، `ConfirmDialog.tsx`، `Combobox.tsx`)، `ui/src/styles/base-ui.css`، `ui/src/i18n/parts/base-ui.*.json`، `ui/src/lib/calendar.ts`/`dates.ts` فقط افزودن تابع | U6 تاریخ تکی تایپی + تقویم؛ کنترل بازه با میان‌برها؛ B7 کلیک بیرون؛ B9/B12؛ ConfirmDialog مشترک؛ منوی بازشوی هماهنگ با تم (Combobox) | opus، `high` |
| **chart** | `ui/src/pages/patients/PatientProfile.tsx`، `ui/src/pages/patients/clinical/ChartTab.tsx`، `ChartPanel.tsx`، `ui/src/chart/*`، `ui/src/styles/chart.css`، `ui/src/i18n/parts/chart.*.json` | U1 چارت+پنل ثابت، سربرگ یک‌خطی، U2 زبانه‌ها ۱۳→۷، B22 راهنمای قدم بعدی | opus، `high` |
| **shell** | `ui/src/pages/Shell.tsx`، `ui/src/pages/Settings.tsx`، `ui/src/shell/*`، `ui/src/pages/Dashboard.tsx`، `ui/src/styles/shell.css`، `ui/src/i18n/parts/shell.*.json` | U3 نوار کناری ۷ آیتمی (تنظیمات تجمیعی)، حذف نشان «آنلاین»، حذف کارت Backup تکراری، B8، U5 | sonnet، `medium` |

## موج ۲ — بعد از base-ui (هم‌زمان، فایل‌ها جدا)

| سشن | فایل‌های مجاز | کار | مدل / effort |
|---|---|---|---|
| **reports** | `ui/src/pages/reports/*`، `ui/src/lib/reports.tsx`، `ui/src/styles/reports.css`، `ui/src/i18n/parts/reports.*.json` | U7/U10: کادر ثابت، لیست گروه‌بندی‌شده با «همه‌ی گزارش‌ها»، کنترل بازه‌ی واحد، حذف «ms»، Excel با توضیح، حالت خالی با میان‌بر | opus، `high` |
| **inventory-forms** | `ui/src/pages/inventory/*`، `ui/src/lib/inventory.ts`، `ui/src/pages/patients/PatientForm.tsx`، `ui/src/pages/reception/AppointmentDialog.tsx`، `ui/src/styles/forms.css`، `ui/src/i18n/parts/forms.*.json` | فرم دریافت موجودی: حالت «جنس خالی» + دکمه، Combobox جنس، تاریخ تکی؛ B6 تاریخ تولد شمسی؛ نوبت با کنترل تاریخ/ساعت جدید | sonnet، `medium` |
| **patients-list** | `ui/src/pages/patients/PatientsPage.tsx`، `MergeDialog.tsx`، `ui/src/pages/reception/RecallsPage.tsx`، `ui/src/styles/patients-list.css`، `ui/src/i18n/parts/patients-list.*.json` | B2 «۱۰۰ از N» + صفحه‌بندی، U13 ستون‌های مفید، B11 کیبورد روی ردیف | sonnet، `medium` |

## موج ۳ — آخر، تک‌سشن (مالک فایل‌های مشترک)

| سشن | فایل‌های مجاز | کار | مدل / effort |
|---|---|---|---|
| **cleanup** | `ui/src/styles.css`، `ui/src/i18n/{fa,ps,en}.json`، فایل‌های دارای `window.confirm` (۱۰ فایل: PatientProfile بعد از chart، ChartPanel، PlansTab، NotesTab، PerioChart، ClinicalListsPage، ClinicalLists5B، PrescriptionEditor، Users، AttachmentsTab) | B15 جایگزینی با `ConfirmDialog`؛ B21 `transition: all`؛ U11 واژه‌ها (چوکی→یونیت، ID بیمار)؛ ادغام فایل‌های `parts` اگر لازم | haiku/sonnet، `low` |

## خارج از این دور (باقی‌مانده / نیاز به تصمیم)

* B4 (Export با permission جدا) — قرارداد API را عوض می‌کند → ممنوع تا CI قرارداد را بسازد.
* B5 (Backup با Connection جدا)، تقسیم `styles.css`، ناوبری با URL، U9 (Wizard ماژولی)، حالت Demo، داکتر/یونیت در Wizard، قیمت خدمات.
* B1: انتقال پاسخ‌های سوابق پزشکی — نیاز به تصمیم دکتر. (پیوست/لابراتوار/جابه‌جایی انبار انجام می‌شود.)
* حذف/پنهان‌کردن «Perio»، ماژول انبار اختیاری — تصمیم مالک.

## معیار «تمام شد» برای همه‌ی سشن‌ها

* فقط فایل‌های مجاز تغییر کرده؛ commit نشده.
* هر کلید i18n در سه زبان؛ هر `t("…")` ثابت کلید موجود دارد.
* TypeScript/Rust را با دقت بخوانید (نمی‌توان build کرد). گزارش صریح: چه چیزی دیده شد، چه چیزی فقط از روی کد.
