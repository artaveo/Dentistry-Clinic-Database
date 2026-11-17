# رفع ایرادها — UI صفحه‌های بالینی (f-ui-clinical)

وضعیت‌ها: **fixed** = رفع شد · **not-a-bug** = ایراد نیست/قبلاً رفع شده · **needs-decision** = تصمیم مالک لازم · **deferred** = عمداً به بعد موکول (دلیل نوشته شده) · **core** = کار سمت `core/` است، UI تغییری نکرد.

## بررسی‌ها (واقعاً اجرا شد)
* `npx tsc --noEmit` ✔ بدون خطا · `node scripts/check-i18n.mjs` ✔ (۳۱۰۶ کلید × ۳ زبان) · `npx vitest run` ✔ (۷۵ تست).
* رندر واقعی: **فقط با Core شبیه‌ساز (mock، `?mock=seeded`)** در مرورگر، با چک DOM (لیست بیماران + فیلتر، پروفایل، ادغام، فرم ویرایش، تقویم، صف). Core واقعی (dev-server) بالا نیامد؛ **تست روی Core واقعی انجام نشد**. اسکرین‌شات به‌خاطر مشکل ابزار (تصویر ریز) قابل استفاده نبود، پس **بررسی بصری ۱۳۶۶×۷۶۸ تیره/روشن و RTL/LTR انجام نشد**؛ فقط DOM (اندازه، اسکرول، وجود عناصر) و کنسول (بدون خطا).
* Playwright (`ui/e2e`) اجرا نشد. دو spec به‌روز شد: ادغام (تیک «مطمئنم» اضافه شد، `merge-sure`) و ثبت هزینه (دسته‌ی پیش‌فرض حذف شد، حالا باید انتخاب شود). **خطر:** `reception.spec.ts:173` (دکمه‌ی «نیامد» روی نوبت امروز) ممکن است بسته به ساعت ساختگی فیکسچر دیگر دکمه را نبیند (حالا فقط بعد از شروع نوبت).

## فایل‌های تغییرکرده
`PatientsPage.tsx`, `PatientForm.tsx`, `PatientProfile.tsx`, `MergeDialog.tsx`, `DuplicateWarningDialog.tsx`, `MedicalAlertBanner.tsx`, `reception/{StatusActions,QueuePage,AppointmentDialog,CalendarPage}.tsx`, جدید: `reception/AlertLine.tsx`, `styles/clinical.css`, `i18n/parts/clinical.{fa,ps,en}.json`; `clinical/{ChartPanel,PlansTab}.tsx`, `clinical/labels.ts`, `lib/medical.ts` (تابع‌های `alertLabel/alertText`), `billing/{dialogs,BillingPage,BillingExtras}.tsx`.

## بیماران و نوبت‌ها (patients-appts)
| ID | وضعیت | یادداشت / فایل |
|---|---|---|
| PA-01, PA-02, PA-30 | core | جستجوی تلفن/نیم‌فاصله در `core/` (normalize) |
| PA-03, PA-07, PA-15, PA-16 | core | اعتبارسنجی تاریخ/نام در core؛ فرم UI تاریخ آینده را خودش رد می‌کند (`max=todayIso`) |
| PA-04 | fixed (UI) | «نیامد» فقط بعد از شروع زمان نوبت: `StatusActions.tsx:~36,66`. بررسی سمت core جدا لازم است |
| PA-28 | fixed | همان بالا |
| PA-05, PA-06 | deferred | قالب تلفن در core؛ یکسان‌سازی ارقام (شماره‌ی بیمار/تلفن لاتین) روی `lib/dates.ts` است (مال سشن دیگر) |
| PA-08, PA-44 | fixed | سوئیچ «پرونده فعال» در فرم ویرایش (بایگانی) `PatientForm.tsx` + فیلتر همه/فعال/غیرفعال در فهرست `PatientsPage.tsx` |
| PA-09, PA-10, PA-22, PA-23, PA-24, PA-29 | core | قفل نسخه‌ی ادغام، حذف با سابقه‌ی مالی، دسترسی export، تکرار recall — core. (UI فقط خطای core را نشان می‌دهد؛ جستجوی ادغام حالا فقط پرونده‌ی **فعال** را می‌آورد = PA-24 بخش UI) |
| PA-11 | fixed | عنوان زبانه‌ی غیرفعال «برای نقش شما فعال نیست» `PatientProfile.tsx` (کلید `patients.tab.noAccess`) |
| PA-12, U-05 | fixed (بخشی) | نوار ذخیره/لغو چسبنده + دکمه‌ی «بازگشت» در فرم `PatientForm.tsx`. فرم دو مرحله‌ای = needs-decision |
| PA-13 | fixed | دکمه‌ی «افزودن «نام» به‌عنوان بیمار تازه» وقتی جستجو خالی است؛ نام در فرم پر می‌شود |
| PA-14, U-14 | fixed | «حدود ۳۴ ساله» و برچسب «سن تقریبی» (پروفایل، هدر، فهرست) |
| PA-17 | fixed | برچسب وضعیت کارت صف حذف (ستون خودش وضعیت است) و شکستن نام بلند `styles/clinical.css` |
| PA-18, CT-24, CT-27 | needs-decision | دسترسی داکتر/دستیار = سیاست نقش‌ها در core |
| PA-19 | deferred | پیام «همه‌ی یونیت‌ها پر است» در متن خطای core |
| PA-20, CT-20 | not-a-bug | رفرش = قفل/ورود دوباره، رفتار طراحی‌شده در حالت مرورگر |
| PA-21 | deferred | جستجوی «3» = core |
| PA-25 | fixed | هشدار پزشکی (آلرژی…) در کارت صف و دیالوگ نوبت: `reception/AlertLine.tsx`. در تقویم (روی خود نوبت‌ها) **انجام نشد** — deferred |
| PA-26 | fixed | چیپ هشدار فقط داخل چارت؛ بیرون از چارت فقط بنر |
| PA-27 | deferred | چک‌لیست ۳۴ سؤال گروه‌بندی‌شونده — بازطراحی بزرگ‌تر (`MedicalHistoryTab.tsx`) |
| PA-31, PA-35, PA-36, PA-37, PA-43 | deferred | Ctrl+K، تقویم بازشو (DateField)، واژگان «چوکی»، تقویم لاتین، تم — فایل‌های سشن دیگر (`CommandPalette`, `ui/*`, i18n اصلی) |
| PA-32 | deferred | Ctrl+K = shell |
| PA-33 | deferred | پیشنهاد ساعت خالی نیاز به API ندارد ولی طراحی جدا می‌خواهد |
| PA-34 | not-a-bug | قبلاً رفع شده: `AppointmentDialog.tsx:~44` ساعت پیش‌فرض را بعد از «اکنون» می‌گذارد |
| PA-38 | fixed | متن هشدار کوتاه («حامله — ۵ ماه») `lib/medical.ts alertText` در بنر/چیپ/صف |
| PA-39 | fixed | دیالوگ ادغام: دو پرونده کنار هم، متن هشدار با نام‌ها، تیک تأیید دوم `MergeDialog.tsx` |
| PA-40 | fixed | دکمه‌ی «باز کردن پرونده» در هشدار بیمار تکراری |
| PA-41 | deferred | گروه‌بندی recallهای تکراری؛ ریشه در core (PA-29) |
| PA-42 | deferred | جستجو در منوی خدمت: نیاز به Combobox مشترک (`ui/*`) |

## چارت، پلان، نسخه (chart-treatment)
| ID | وضعیت | یادداشت |
|---|---|---|
| CT-01 | fixed | ثبت بدون سطح حالا پیام می‌دهد `ChartPanel.tsx` (`item-error`) |
| CT-02..13, 16, 17, 18, 23, 25, 26, 28 | core | قواعد بالینی/داده در core |
| CT-08, CT-30, CT-38 | core + needs-decision | فاکتور ارتودنسی مرحله‌ای: تغییر مدل در core؛ UI بعد از آن |
| CT-14, CT-29 | core | پیام خام انگلیسی از core؛ UI فیلد قیمت را از قبل اعتبارسنجی می‌کند |
| CT-15 | deferred | شمارنده‌ی نویسه |
| CT-19 | deferred | بازطراحی دیالوگ ثبت (۳ مرحله) و پرش چیپ — پیشنهاد «نوار ابزار ۸ وضعیت پرتکرار» محصولی است → needs-decision |
| CT-21 | not-a-bug | تم از تنظیم کاربر (localStorage) می‌آید، `theme.ts` سشن دیگر |
| CT-22, CT-35 | deferred | دسترسی‌پذیری دندان‌ها/پریو (`Odontogram.tsx`, `PerioChart.tsx`) — کار بزرگ، جدا |
| CT-31 | deferred | پنجره‌ی چاپ نسخه: ریشه احتمالاً در `ui/Overlay.tsx` (سشن دیگر) |
| CT-32 | not-a-bug | منوی «تاریخچه» الان Popover است و با کلیک بسته می‌شود (کد فعلی `PatientProfile.tsx`) |
| CT-33, CT-34, CT-41 | deferred | اسکرول افقی پریو/پلان در ۱۰۲۴px — پنجره‌ی هدف ۱۳۶۶ است؛ بررسی بصری انجام نشد |
| CT-36 | fixed | «۱ از ۲۴ جلسه انجام شد» `clinical/labels.ts sessionText` |
| CT-37 | fixed | دکمه‌ی «افزودن خدمت از چارت» در پلان خالی `PlansTab.tsx` |
| CT-39, CT-43, CT-42 | deferred | پیش‌فرض داکتر نسخه، دیالوگ یادداشت، توضیح نقش دستیار |
| CT-40 | not-a-bug | مثبت |

## مالی (finance-reports + ui-polish)
| ID | وضعیت | یادداشت |
|---|---|---|
| FR-03/04/05/06/07/08/09/11 | core | محاسبات و export |
| FR-13 | fixed | تعداد اقساط تا ۳۶ (۱۸، ۲۴ و…) `billing/dialogs.tsx` |
| FR-14 | fixed | «ماهانه» = همان روز هر ماه در تقویم کلینیک (`shiftMonth`) |
| FR-15 | fixed | باقی‌مانده بین ردیف‌های اول پخش می‌شود |
| FR-16 | fixed | شمارش خالی → «مبلغ شمارش‌شده را بنویسید» `BillingPage.tsx` |
| FR-17 | fixed | دسته‌ی هزینه پیش‌فرض ندارد؛ باید انتخاب شود `BillingExtras.tsx` |
| U-23 | needs-decision | شمارش کور صندوق (خالی گذاشتن مبلغ) = تصمیم محصول |
| FR-18, FR-20, FR-21, FR-22 | deferred | فیلتر/صفحه‌بندی فاکتور، تخفیف درصدی، سن بدهی، تکرار کمیسیون |
| FR-19 | deferred | نماد دندان در فاکتور `InvoiceDetail.tsx` |
| U-01 | deferred | خطای `<tr>/<div>` در تب اسناد؛ محل دقیق پیدا نشد |
| U-03, U-04 | deferred | ویرایشگر نسخه (قاب/اسکرول تودرتو/۱۰ کلیک) — بازطراحی جدا |
| U-06 | fixed | یادآور «سوابق پزشکی پرسیده نشده» فقط در نمای کلی، چارت/درمان، یادداشت‌ها و سوابق؛ نه در مالی/نوبت/اسناد |
| U-07 | deferred | میان‌بر «نوبت/پرداخت/نسخه» در هدر پروفایل (باید دیالوگ‌ها بالا بیایند) |
| U-08 | needs-decision | ادغام زیرتب «درمان‌ها»+«پلان‌ها» |
| U-09 | fixed | تقویم هنگام باز شدن به ساعت اکنون (یا اولین نوبت) اسکرول می‌شود `CalendarPage.tsx`؛ ارتفاع کل ثابت ماند |
| U-12 | deferred | «دکتر/داکتر» در فایل‌های i18n اصلی (سشن دیگر) |
| U-13, U-15, U-20, U-25..U-32 | deferred | مال `ui/*`, Shell, Settings, i18n/dates (سشن دیگر)؛ U-17 (دیالوگ نوبت کم‌کنترل‌تر) deferred |
| U-19 | not-a-bug | «تاریخچه» الان در ردیف زبانه‌ها و منو است |
| U-22 | deferred | ستون بدهی/نوبت بعدی در فهرست بیماران نیاز به API |

## درخواست برای سشن primitiveها
* `ui/Overlay.tsx`: Esc روی دیالوگ با داده‌ی تایپ‌شده تأیید بخواهد (U-27)، برگشت فوکوس (U-28)، و بررسی قاب/ارتفاع چاپ نسخه (CT-31).
* `ui/Controls.tsx` Segmented: `role=radio` همراه `aria-pressed` (U-25).

## کلمات پشتوی مشکوک (`clinical.ps.json`)
`patients.duplicate.open` («پرونده خلاصه کړئ»)، `patients.age.about` («شاوخوا»)، `item.sessionsDoneOf`، `patients.merge.warningNamed` — لطفاً یک پشتوزبان بازبینی کند.
