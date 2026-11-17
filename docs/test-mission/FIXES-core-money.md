# رفع باگ‌های هستهٔ پول / پلان / چارت (f-core-money)

وضعیت: تمام. آزمون‌ها در `core/src/tests_core_money.rs` (۹ آزمون) و یک آزمون در `core/src/attachment.rs`؛ همه روی لپ‌تاپ سبز شدند. `shared/ts/contract.ts` دوباره ساخته شد. `Cargo.lock` به حالت اول برگشت.

اجرای کامل `cargo test -p artaveo-core` (release، محلی): همه سبز به جز سه آزمون که مال سشن دیگر است و به کار من ربط ندارد: `restore::tests::a_file_that_is_not_a_backup_is_reported_not_trusted`، `tests_phase10::restore_validates_...` (انتظار `wrong_key`، نتیجه `damaged`) و `tests_core_admin::a_patient_with_records_cannot_be_deleted_...` (`patients.get` بدون `patient_id`).

تغییر آزمون‌های موجود: `tests.rs` (جمع درآمد داکتر حالا خالص است: ۳۳۰٬۰۰۰)، بایت‌های جعلی آپلود در `tests.rs`، `tests_core_admin.rs`، `patient.rs` حالا با هدر PNG شروع می‌شوند.

## جدول یافته‌ها

| شناسه | وضعیت | محل اصلاح | آزمون |
|---|---|---|---|
| CT-02 یافتهٔ تکراری | fixed (همان یافته دوباره ثبت نمی‌شود، همان قبلی برمی‌گردد) | `chart.rs` add_findings | `ct02_ct03_ct04_ct16_chart_findings_are_checked` |
| CT-03 دندان غایب + پوسیدگی | fixed (روی دندان غایب/کشیده‌شده یافتهٔ دیگر رد می‌شود، مگر ایمپلنت/بریج/پروتز؛ ایمپلنت روی کشیده‌شده آزاد است) | `chart.rs` tooth_is_absent، add_findings | همان |
| CT-04 دندان شیری/دائمی | fixed (چارت دائمی شیری را و چارت شیری دائمی را رد می‌کند؛ مختلط هر دو) | `chart.rs` check_dentition (برای یافته و آیتم درمان) | همان + `ct10_...` |
| CT-05 تمام‌شده بدون جلسه | fixed (آیتم چندجلسه‌ای بدون حتی یک جلسه تمام نمی‌شود) | `treatment.rs` set_item_status | `ct05_...` |
| CT-06 لغو پلان بدون دلیل | fixed (دلیل اجباری؛ لغو آیتم شروع‌شده هم دلیل می‌خواهد؛ آیتم فاکتورشده لغو نمی‌شود) | `treatment.rs` set_plan_status / set_item_status | `ct06_...`، `ct08_...` |
| CT-07 پلان بسته‌شده | fixed (پلان «تمام‌شده» آیتم تازه می‌پذیرد و دوباره «در حال انجام» می‌شود؛ پلان لغوشده بسته می‌ماند) | `treatment.rs` add_items | `ct07_...` |
| CT-08 فاکتور ارتودنسی | fixed (آیتم چندجلسه‌ای که حداقل یک جلسه دارد قابل فاکتور است؛ یک‌بار کل مبلغ + قسط‌ها). ⚠ سیاست → «نیاز به تصمیم» | `billing.rs` BILLABLE_ITEM، billable، create_invoice؛ `shared/src/billing.rs` BillableItem | `ct08_...` |
| CT-09 قسط با تاریخ گذشته | fixed جزئی (قسط عقب‌افتاده طبق طراحی مجاز است؛ بیش از یک سال گذشته = غلط تایپی، رد می‌شود) | `billing.rs` write_installments | `ct09_fr04_...` |
| CT-10 تاریخ پایان قبل از شروع | fixed (تاریخ‌های گذشته مجازند) | `treatment.rs` check_est | `ct10_ct11_...` |
| CT-11 بریج | fixed (بریج = حداقل ۲ دندان در یک فک). پروتز کامل دو فک: not-a-bug (دو فک = دو واحد قیمت) | `treatment.rs` check_bridge | `ct10_ct11_...` |
| CT-12 کشیدن روی دندان غایب | fixed. ایمپلنت روی دندان غایب: not-a-bug (درست است). خدمت اطفال روی دندان دائمی: not-a-bug (سیلانت اطفال روی دائمی رایج است) | `treatment.rs` check_extraction | `ct10_ct11_...` |
| CT-13 تکرار خدمت روی همان دندان | fixed (در همان پلان، آیتم باز). قیمت صفر: needs-decision | `treatment.rs` add_items | `ct10_ct11_...` |
| CT-14، CT-29 پیام خام انگلیسی ورودی بد | deferred (در `api.rs` params؛ فایل سشن دیگر و UI باید پیام فارسی نشان دهد) | — | — |
| CT-16 حذف دوباره یافته | fixed (rule تازه `finding_not_found`) | `chart.rs` get_finding | `ct02_ct03_...` |
| CT-17 قالب با قیمت صفر | needs-decision (قالب‌ها خدمت والد را بدون انتخاب نوع می‌گذارند) | — | — |
| CT-18 قیمت‌های کاتالوگ صفر | needs-decision (قیمت را مالک می‌گذارد) | — | — |
| CT-23 چاپ سند باطل | not-a-bug (کد از قبل `document_void` برمی‌گرداند؛ آزمون ثابت می‌کند) | `documents.rs` check_issued | `ct23_ct26_...` |
| CT-24، CT-27 دسترسی دستیار/دکتر | needs-decision / سشن دیگر (`auth.rs`) | — | — |
| CT-25 پیوست | fixed (فقط تصویر/PDF، محتوا باید با پسوند بخواند، نام بدون پوشه). شمارهٔ دندان ۹۹: در `patient.rs` (سشن دیگر) | `attachment.rs` check_upload، `api.rs` ATTACHMENTS_UPLOAD | `uploads_must_be_what_they_claim_to_be` |
| CT-26 داروی تکراری در نسخه | fixed | `documents.rs` check_content | `ct23_ct26_...` |
| CT-28 بازکردن پلان لغوشده | fixed (آیتم‌هایی که همراه پلان لغو شده بودند برمی‌گردند) | `treatment.rs` set_plan_status | `ct06_...` |
| CT-30 | همان CT-08 | | |
| FR-02 صندوق با مورد انتظار منفی | needs-decision (فقط هشدار UI) | — | — |
| FR-03 مبنای کمیسیون | not-a-bug (جمع کل درست است؛ فقط عدد دوره‌های قبل با یادداشت اعتباری بعدی جابه‌جا می‌شود، طبق طراحی) | — | — |
| FR-04 اقساط بعد از یادداشت اعتباری | fixed (از آخرین قسط کم می‌شود؛ پرداخت‌شده‌ها دست‌نخورده) | `billing.rs` shrink_installments | `ct09_fr04_...` |
| FR-05 درآمد داکتر/خدمت ناخالص | fixed (خالص = بعد از تخفیف و یادداشت اعتباری) | `reports.rs` NET_SHARE | `fr05_fr08_fr09_...` |
| FR-06 برگشت به اعتبار در گزارش برگشت‌ها | needs-decision | — | — |
| FR-07 «طلب‌های باز» بدون بازه | needs-decision (برچسب «تا امروز» در UI) | — | — |
| FR-08 تخفیف‌ها بدون یادداشت اعتباری | fixed (ردیف یادداشت‌های اعتباری هم می‌آید) | `reports.rs` discounts | `fr05_fr08_fr09_...` |
| FR-09 Export روی هم می‌نویسد | fixed (پسوند -2، -3) | `reports.rs` export | `fr05_fr08_fr09_...` |
| FR-10 | سشن دیگر (`auth.rs`) | — | — |
| FR-11 فهرست فاکتور ۱۰۰ تایی | deferred (API صفحه‌بندی `offset` دارد؛ UI باید استفاده کند) | — | — |
| CT-01، 15، 19–22، 31–43، FR-13..22 | UI، به سشن UI سپرده شد | — | — |
| PA-04 (no-show نوبت آینده)، PA-09، PA-22 | سشن دیگر (`appointment.rs`/`patient.rs`) | — | — |

## نیاز به تصمیم مالک

* **CT-08:** الان هر درمان چندجلسه‌ای (ارتودنسی، عصب‌کشی دو جلسه‌ای…) بعد از اولین جلسه می‌تواند کامل فاکتور شود. آیا فقط ارتودنسی؟ نیاز به نشانهٔ «قابل قسط» روی خدمت.
* **قیمت صفر (CT-13/CT-18):** کاتالوگ بعد از نصب قیمت ندارد؛ بستن ثبت با قیمت صفر تا قیمت‌گذاری مالک ممکن نیست.
* **CT-17:** قالب «عصب‌کشی و روکش» خدمت والد (بدون نوع) را می‌گذارد. باید نوع پیش‌فرض (مثلاً یک کانال) انتخاب شود.
* **FR-06/07/02:** تعریف گزارش برگشت (شامل برگشت به اعتبار؟)، برچسب «تا امروز»، هشدار صندوق منفی.
* **CT-24/27، FR-10:** دسترسی دستیار/داکتر/حسابدار (در `auth.rs`).

## تغییرات قرارداد (`shared`) برای سشن‌های UI

* `ValidationRule` سه مقدار تازه: `tooth_conflict`، `finding_not_found`، `item_duplicate` (+ چهار مقدار درخواست‌شدهٔ f-core-admin: `patient_has_history`، `stock_code_taken`، `appointment_not_started`، `recall_duplicate`). برای همه باید متن `rule.*` در `en/fa/ps.json` اضافه شود.
* `BillableItem`: سه فیلد تازه `completed`، `sessions_done`، `sessions_planned`.
* رفتار جدید: `plans.set_status` به `cancelled` حتماً `reason` می‌خواهد (field=`reason`، rule=`required`)؛ `treatment_items.set_status` به `cancelled` برای آیتم شروع‌شده هم `reason` می‌خواهد؛ `treatment_items.set_status completed` برای آیتم چندجلسه‌ای بدون جلسه `item_sessions` می‌دهد؛ `attachments.upload` فقط png/jpg/webp/gif/pdf با محتوای درست.
