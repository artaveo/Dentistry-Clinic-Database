# رفع باگ‌های هسته (Core): مدیریت، بکاپ، بیماران، نوبت‌ها، انبار

وضعیت: کد نوشته شده و تست‌ها در `core/src/tests_core_admin.rs` است. نتیجه‌ی اجرای تست‌ها پایین ثبت می‌شود (CI داور نهایی است).
این سشن چیزی commit نکرده است. `shared/` را دست نزده است.

## جدول یافته‌ها

| ID | وضعیت | فایل:خط | تست |
|---|---|---|---|
| R-11 مدیر رمز/نام مالک را عوض می‌کند | fixed | core/src/auth.rs `ensure_may_touch_user` (update_user، reset_password) | `administrator_cannot_touch_the_owner_account` |
| R-01 / PA-10 خروجی Excel بیماران برای همه | fixed (فقط `backup.create`: مالک و مدیر) | core/src/api.rs `PATIENTS_EXPORT` | `patient_export_and_purchase_data_are_limited_to_the_right_roles` |
| R-21 دکتر/منشی/دستیار خرید و بدهی را می‌بینند | fixed | api.rs: purchases.list/get، supplier_payments.list، supplier_ledger.list، lab_cases.report نیاز به `inventory.manage`؛ suppliers.list/get مبلغ‌ها را صفر می‌کند | همان تست بالا |
| R-12 رمز یکسان، نشست قبلی زنده | fixed | api.rs `AUTH_CHANGE_PASSWORD`، session.rs `remove_user_except` | `changing_the_password_needs_a_new_one_and_ends_other_sessions` |
| R-02 کد جنس تکراری | fixed | inventory.rs `save_item` | `stock_item_codes_are_unique_and_nearest_expiry_ignores_expired_lots` |
| R-04 «نزدیک‌ترین انقضا» lot منقضی | fixed | inventory.rs `ITEM_SELECT` | همان تست |
| R-13 clinic.update تلفن/آدرس | fixed (نام اصلاً با این RPC عوض نمی‌شود: باگ نبود) | clinic.rs `validate` | `clinic_phone_and_address_are_validated` |
| BR-1 بکاپ local بدون عکس/X-ray | fixed | backup.rs `create`: عکس‌ها کنار بکاپ در `backups\attachments` کپی می‌شوند؛ restore قبلاً از همان‌جا برمی‌گرداند. خطای باز کردن عکس گم‌شده حالا واضح است (attachment.rs `read`) و فهرست بندانگشتی‌ها خراب نمی‌شود (api.rs) | `local_backup_carries_the_images_and_a_damaged_file_is_called_damaged` |
| BR-2 بکاپ‌های قدیمی بعد از restore روی کامپیوتر جدید | fixed | backup.rs `reconcile` ردیف‌هایی که فایلشان نیست را حذف‌شده علامت می‌زند | همان تست |
| BR-4 / BR-8 فایل ناقص = «wrong_key» | fixed | restore.rs `inspect`: اندازه‌ی غیرمضرب ۴۰۹۶ = `damaged` | همان تست |
| BR-3 بکاپ prerestore با kind=manual | not-a-bug | UI از نام فایل برچسب را می‌سازد (`fileKindLabel`) | — |
| BR-5 scan پوشه معمولی source=cloud | deferred | نیاز به مقدار جدید در `BackupSource` (contract) | — |
| BR-6 retention فایل‌ها را حذف می‌کند | needs-decision | طبق طراحی (۳ تای آخر + روزانه/هفتگی/ماهانه نگه داشته می‌شود) | — |
| BR-7 هشدار عکس در restore | deferred (متن UI؛ حالا چون عکس‌ها داخل بکاپ هستند، هشدار کمتر لازم است) | ui | — |
| BR-9، BR-10 | UI | — | — |
| PA-01 / PA-30 جستجوی تلفن با فاصله/بدون صفر/+93 | fixed | patient.rs `fts_match_query`، `phone_variants` | `patient_input_is_validated_and_search_finds_phone_and_joined_names` |
| PA-02 نیم‌فاصله در نام | fixed (اندیس جستجو برای دیتابیس‌های قدیمی خودکار یک‌بار بازسازی می‌شود: `ensure_search_index` در core.rs `install`) | patient.rs | همان تست و `search_index_is_rebuilt_once_for_an_old_database` |
| PA-03 تاریخ تولد آینده/۱۸۰۰ | fixed | patient.rs `validate_birth_date` | همان تست |
| PA-07 تاریخ ثبت آینده | fixed | patient.rs `create_patient` | همان تست |
| PA-15 نام فقط عدد/HTML | fixed (باید حداقل یک حرف داشته باشد و `<` `>` نداشته باشد) | patient.rs `validate_full_name` | همان تست |
| PA-16 خطای FOREIGN KEY خام | fixed | patient.rs `list_value_error` | همان تست |
| PA-09 حذف بیمار دارای فاکتور/پرداخت/پرونده | fixed (حذف مسدود؛ باید «غیرفعال/بایگانی» شود) | patient.rs `delete_patient` | `a_patient_with_records_cannot_be_deleted_...` |
| PA-22 ادغام نسخه را چک نمی‌کند | not-a-bug (کد `version = ?` را چک می‌کند؛ نسخه‌ی بیمار تازه‌ساخته خودش ۱ بود) | patient.rs `merge_patients` | همان تست (نسخه‌ی قدیمی = Conflict) |
| PA-23 ادغام دوباره | fixed | patient.rs `merge_patients` | همان تست |
| PA-04 / PA-28 «نیامد» برای نوبت روز آینده | fixed در هسته (روز آینده رد می‌شود؛ همان روز آزاد است) | appointment.rs `set_status` | `a_future_appointment_cannot_be_a_no_show_...` |
| PA-29 / PA-41 یادآوری تکراری | fixed (ساخت دستی) | recall.rs `create_recall` | همان تست |
| PA-24 بیمار ادغام‌شده در جستجو | needs-decision (طراحی عمدی: برای راهنمایی کاربر به رکورد اصلی) | patient.rs | — |
| PA-05 نرمال‌سازی قالب تلفن | needs-decision (داده‌ی کاربر بازنویسی نمی‌شود) | — | — |
| PA-08 بایگانی بیمار در UI | UI (هسته `status=inactive` را دارد) | — | — |
| PA-19 / PA-21 / PA-34 / PA-25 / PA-38 | UI یا deferred (نوبت بدون یونیت و تاریخ ۲۰۹۹ تصمیم محصولی‌اند) | — | — |
| R-03 دریافت کالای منقضی بدون هشدار | needs-decision (هسته می‌پذیرد؛ هشدار در UI) | — | — |
| R-14 / R-15 ماژول انبار خاموش در API | needs-decision (لابراتوار به انبار گره خورده؛ `reports.rs` در حوزه‌ی دیگری است) | — | — |
| R-20 نام تکراری تأمین‌کننده/لابراتوار | needs-decision (دو شعبه ممکن است نام یکسان داشته باشند) | — | — |
| R-22…R-27 UI | UI | — | — |
| R-28 مقدار انبار ×۱۰۰ | not-a-bug (طراحی؛ مستند) | — | — |
| ورود: «account_locked» وجود نام کاربری را لو می‌دهد | needs-decision (پیام قفل برای کاربر مفید است) | auth.rs `login` | — |

## تغییر RPC/رفتار برای سشن‌های UI

1. `patients.export`: حالا نیاز به `backup.create` دارد (مالک و مدیر). دکمه‌ی «خروجی Excel» را برای بقیه پنهان کنید.
2. `purchases.list/get`، `supplier_payments.list`، `supplier_ledger.list`: فقط `inventory.manage` (مالک، مدیر، حسابدار). تب «خریدها» و دفتر تأمین‌کننده را برای دکتر/منشی/دستیار پنهان کنید.
3. `suppliers.list/get` برای کسی که `inventory.manage` ندارد، مبلغ‌ها (`purchases_total`, `lab_total`, `returns_total`, `paid_total`, `balance`) را صفر می‌دهد؛ فیلتر `owing_only` را نادیده می‌گیرد.
4. `lab_cases.report`: `inventory.manage` یا `reports.view`.
5. `users.update` / `users.reset_password` روی مالک توسط غیر مالک: خطای `forbidden`. UI باید دکمه‌ها را برای مدیر روی ردیف مالک پنهان/غیرفعال کند.
6. `auth.change_password`: اگر رمز جدید = رمز فعلی، خطای validation با فیلد `new_password` و rule `invalid_params`. نشست‌های دیگر همان کاربر بسته می‌شوند (نشست فعلی می‌ماند).
7. `stock_items.save` با کد تکراری: validation، فیلد `code`، rule `stock_code_taken`.
8. `patients.delete` برای بیمار دارای فاکتور/پرداخت/برنامه‌ی درمان/یادداشت/عکس: validation (rule `patient_has_history`). UI باید «غیرفعال کردن» را پیشنهاد کند (PA-08).
9. `patients.merge` روی رکورد ادغام‌شده: rule `patient_merged`.
10. `patients.create/update`: تاریخ تولد خارج از ۱۹۰۰..امروز → rule `age_range`؛ تاریخ ثبت آینده → `date_format`؛ نام بدون حرف یا با `<` `>` → `full_name_length`.
11. `appointments.set_status` به `no_show` برای روز آینده: validation، فیلد `status`، rule `appointment_not_started`. دکمه‌ی «نیامد» برای نوبت آینده را در UI پنهان کنید.
12. `recalls.create` تکراری (نوع + تاریخ، باز): validation، فیلد `due_date`، rule `recall_duplicate`.
13. `backup.restore_preview` / `restore_new`: فایل ناقص یا غیر دیتابیس: `problem = "damaged"` (قبلاً `wrong_key`). متن `bk.restore.problem.damaged` باید «فایل آسیب‌دیده یا ناقص است» بگوید.
14. بکاپ local حالا پوشه‌ی `backups\attachments` را هم می‌سازد (حجم دیسک بیشتر می‌شود، ولی فقط یک‌بار برای هر عکس). متن هشدار restore (BR-7) می‌تواند ساده‌تر شود.
15. `attachments.file` وقتی فایل عکس روی این کامپیوتر نیست: خطای `not_found` با پیام روشن (قبلاً os error 3).
16. اندیس جستجوی بیماران در اولین اجرای نسخه‌ی جدید خودکار بازسازی می‌شود (چند ثانیه برای دیتابیس‌های بزرگ).

## نیاز به تصمیم مالک
* دریافت کالای منقضی (R-03): ممنوع شود یا فقط هشدار؟
* نام تکراری تأمین‌کننده/لابراتوار (R-20): ممنوع یا فقط هشدار؟
* ماژول انبار خاموش (R-14/R-15): لابراتوار هم خاموش شود؟ API هم قفل شود؟
* پیام «حساب قفل شد» نام کاربری موجود را لو می‌دهد (امنیت در برابر راحتی کاربر).
* رکورد ادغام‌شده در جستجو دیده شود یا نه (PA-24).
* نوبت بدون یونیت و نوبت خیلی دور (۲۰۹۹) مجاز باشد؟
* حداکثر حجم بکاپ local با عکس‌ها (حالا همه‌ی عکس‌ها در `backups\attachments` کپی می‌شوند).

## نتیجه‌ی ساخت و تست
* `cargo test -p artaveo-core --lib` روی همین لپ‌تاپ: **۲۰۶ تست موفق، ۰ ناموفق** (شامل ۹ تست تازه در `tests_core_admin.rs`).
* دو تست قدیمی عمداً عوض شدند: فایل متنی/ناقص حالا `damaged` است نه `wrong_key` (`restore.rs` و `tests_phase10.rs`).
* rustfmt اجرا نشده؛ مادر فرمت کند. CI داور نهایی است.
* قاعده‌های `rule.patient_has_history`، `rule.stock_code_taken`، `rule.appointment_not_started`، `rule.recall_duplicate` در UI (fa/ps/en) باید متن داشته باشند (سشن UI).

## فایل‌های تغییرکرده
auth.rs, api.rs, session.rs, clinic.rs, inventory.rs, patient.rs, appointment.rs, recall.rs, backup.rs, restore.rs, attachment.rs, core.rs (فقط یک خط: بازسازی اندیس جستجو)، lib.rs (ثبت ماژول تست)، tests_core_admin.rs (جدید)، tests_phase10.rs (یک انتظار).
