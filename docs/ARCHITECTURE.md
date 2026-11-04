# Artaveo Dental — سند معماری (برای سشن Control Center)

نسخهٔ برنامه هنگام نوشتن: **0.11.1** (workspace). فقط از کد و تصمیم‌های ثبت‌شده در مخزن نوشته شده. جایی که مطمئن نیستم «نامشخص» آمده. هیچ رمز، کلید خصوصی، connection string یا داده بیمار در این سند نیست.

منابع اصلی: `Artaveo_Dental_Roadmap.md`، `docs/adr/*`، `core/src/*`، `core/migrations/*`.

---

## ۱. هدف و کاربران

* **Windows Desktop Application** برای مدیریت کامل کلینیک دندان‌پزشکی؛ آفلاین، دیتابیس محلی رمزشده، تک‌کامپیوتر (پیش‌فرض) یا چندکامپیوتر در LAN (اختیاری)، سه زبان: دری، پشتو، انگلیسی (RTL/LTR، Light/Dark).
* مشتری: **کلینیک** (یک License برای هر کلینیک). کاربران داخل کلینیک: مالک، مدیر، پذیرش، دکتر، حسابدار، دستیار.
* فقط Windows 10/11، سه معماری x64 / x86 / ARM64 (Installer جدا برای هر کدام). ارز فقط AFN. انتشار از وب‌سایت Artaveo، بدون Code Signing.
* **Artaveo هرگز داده بیمار را نگه نمی‌دارد.** سرور مرکزی فقط لایسنس/فعال‌سازی است.
* نصب فعلاً حضوری و رایگان توسط Artaveo.

## ۲. تصمیم‌های معماری قفل‌شده (ADR) و دلیل

| # | تصمیم | دلیل / گزینهٔ ردشده |
|---|---|---|
| ADR-01 | تمام منطق و دسترسی به دیتابیس در **Rust Core**؛ React فقط UI | دور زدن Permission/Audit ممکن نباشد؛ همان کد برای حالت شبکه |
| ADR-02 | یک API Contract (`Core::handle(RpcRequest)->RpcResponse`)؛ Tauri IPC در تک‌کامپیوتر، HTTPS/WebSocket در LAN | یک کد، دو حالت |
| ADR-03 | SQLite + **SQLCipher** (WAL، FK، STRICT)، کلید خام ۲۵۶ بیتی | امنیت داده پزشکی |
| ADR-04 | کلید با **DPAPI (Machine Scope)** + **Recovery Key** (چاپ در Setup)؛ نسخهٔ Recovery همراه هر Backup | بدون Recovery Key، Restore روی کامپیوتر جدید ممکن نیست (عمدی) |
| ADR-05 | UUIDv7 + شمارهٔ انسانی جدا (`P-000123`) | آمادهٔ Sync آینده |
| ADR-06 | پول INTEGER ×100 | بدون خطای Float |
| ADR-07/08/09 | زمان UTC، منطقهٔ `Asia/Kabul`، ساعت ۱۲ساعته؛ تقویم شمسی پیش‌فرض؛ ارقام لاتین در ذخیره | استاندارد افغانستان |
| ADR-10/11 | Soft Delete برای داده بالینی/مالی؛ فاکتور/رسید صادرشده غیرقابل ویرایش (فقط Void/Credit Note) | Audit و یکپارچگی مالی |
| ADR-12 | پیوست‌ها در پوشهٔ داده، نام بر پایهٔ SHA-256، رمزنگاری‌شده | Backup افزایشی |
| ADR-13 | PDF/چاپ با WebView2؛ حرارتی ESC/POS | RTL کامل |
| ADR-15/16 | **منسوخ برای دندان:** لایسنس با Supabase | جایگزین: ADR-25 |
| ADR-17 | Cloud Backup = پوشهٔ همگام‌سازی Drive/OneDrive خود کلینیک | بدون هزینه، داده به Artaveo نمی‌رسد |
| ADR-18 | LAN: mDNS، گواهی self-signed با Pin، Pairing با کد ۶ رقمی SPAKE2، token هر دستگاه | بدون MITM/brute-force |
| ADR-22 | هویت USB: Marker امضاشده HMAC + سریال سخت‌افزار (Trusted/Cloned/Unknown) | Drive Letter ناپایدار است |
| ADR-23 | داده در `%ProgramData%\ArtaveoDental`؛ `ARTAVEO_ENV`؛ Permissionها در کد | سرویس LAN و حساب‌های مشترک |
| **ADR-25** | **لایسنس از Control Center (Neon)** — جزئیات در بخش ۱۱ و `docs/adr/ADR-25-license-control-center.md` | سوپابیس کنار گذاشته شد |

اصول ثابت: یک Codebase برای همه کلینیک‌ها؛ Configuration به‌جای Fork؛ روز بدون اینترنت = روز کاری عادی؛ هیچ Feature نباید Security/Backup/License/Data Integrity را دور بزند.

## ۳. استک

| لایه | انتخاب |
|---|---|
| زبان Core | Rust (workspace: `core`, `shared`, `app`, `lan`) |
| دسکتاپ | Tauri 2 (پوسته فقط فرمان `rpc` + Installer NSIS؛ WebView2 آنلاین/آفلاین) |
| UI | React + TypeScript + Vite؛ فونت Vazirmatn + Inter؛ تست Playwright (E2E روی Core واقعی) |
| دیتابیس | `rusqlite` با `bundled-sqlcipher-vendored-openssl` (SQLCipher 4.5.7) |
| Contract | `shared` (Rust) → `shared/ts/contract.ts` با ts-rs؛ CI اگر کهنه باشد رد می‌کند |
| احراز هویت | Argon2id؛ session داخل Core (`session.rs`) |
| LAN | rustls (ring) + WebSocket، mDNS، SPAKE2 |
| لایسنس | JWS EdDSA (Ed25519)، اثرانگشت SHA-256 |
| بک‌اند | **ندارد.** همه چیز داخل Core روی کامپیوتر کلینیک (سرور LAN = همان برنامه در Server Mode). تنها سرویس بیرونی: تابع `validate` روی Neon |
| CI | GitHub Actions (Linux: fmt، clippy `-D warnings`، تست، contract، perf gate، UI، E2E؛ Windows x64/x86/arm64: تست + Installer + smoke)؛ Tag `vX.Y.Z` → GitHub Release |

قوانین کار: تست/Build فقط روی GitHub CI (نه لپ‌تاپ مالک)؛ `cargo fmt` از راه شاخهٔ موقت `fmt/*`.

## ۴. ساختار پوشه‌ها

```text
core/                 Rust Core: همهٔ منطق
  src/api.rs          جدول متدهای RPC (نام متد → Permission → تابع)
  src/core.rs         ساخت Core، کارهای زمان‌بندی‌شده (Backup روزانه، چک لایسنس)
  src/db.rs           اتصال SQLCipher، اجرای Migration
  src/auth.rs session.rs  کاربر، Role، Permission، Session
  src/audit.rs        Audit Log
  src/patient.rs medical.rs attachment.rs import.rs normalize.rs   بیمار، سوابق پزشکی، پیوست، CSV
  src/appointment.rs scheduling.rs recall.rs   نوبت، دکتر/صندلی/ساعت، فراخوان
  src/catalog.rs chart.rs perio.rs treatment.rs notes.rs timeline.rs documents.rs formulary.rs  کاتالوگ، چارت دندان، لثه، درمان، یادداشت، نسخه
  src/billing.rs      فاکتور، پرداخت، اقساط، کمیسیون، هزینه، بستن صندوق
  src/inventory.rs    انبار، تأمین‌کننده، لابراتوار
  src/reports.rs      داشبورد، ۲۶ گزارش، اعلان، جستجوی سراسری
  src/backup.rs usb.rs restore.rs   Backup Manager، USB مورد اعتماد، Restore
  src/license.rs fingerprint.rs     لایسنس (ADR-25)
  src/lan.rs          حالت سرور/کلاینت
  src/calendar* clock.rs keys/      تقویم شمسی، ساعت، کلید/DPAPI/Recovery
  migrations/         0001..0018 (.sql)
  seeds/              داده مرجع و جغرافیا (CSV سه‌زبانه)
  src/bin/            artaveo-dev-server (فقط 127.0.0.1)، artaveo-seed
shared/               Contract (نوع‌ها، ErrorCode، ValidationRule، PUBLIC_METHODS)
app/                  پوستهٔ Tauri
lan/                  Crate شبکه (سرور/کلاینت TLS+WebSocket) و تست‌های سوکت واقعی
ui/src/pages          صفحه‌ها (patients, reception, billing, inventory, reports, backup, License ...)
ui/src/shell setup    Shell، اعلان، بنر لایسنس، Setup Wizard
ui/src/i18n           fa / ps / en
ui/e2e                تست‌های Playwright
docs/                 adr، releases، specs، spikes، testing
scripts/              check-version.mjs
spikes/               نمونه‌های فاز ۰ (workspace جدا)
```

## ۵. دیتابیس (فایل محلی `clinic.db`، نه Postgres)

جداول (از Migrationها):

* **پایه/هویت:** `db_meta`, `setting`, `audit_log`, `permission`, `role`, `role_permission`, `app_user`, `counter`
* **مرجع/جغرافیا:** `reference_type`, `reference_item`, `reference_translation`, `province`, `district`, `geo_translation`
* **بیمار:** `patient` (+`patient_fts` جستجوی FTS5), `patient_medical_history`, `patient_attachment`, `medical_question(+translation)`, `patient_medical_answer`, `form_draft`
* **نوبت:** `doctor`, `chair`, `doctor_hours`, `doctor_break`, `doctor_chair`, `doctor_leave`, `appointment`, `recall`, `doctor_specialty`
* **بالینی:** `service_category`, `service`, `tooth_condition`, `dental_chart`, `tooth_finding`, `treatment_plan`, `treatment_item`, `treatment_session`, `plan_template`, `clinical_note`, `clinical_note_addendum`, `note_template`, `perio_exam`, `clinical_document`, `drug`, `rx_template`, `document_template(+text)`, `document_counter`
* **مالی:** `payment_method`, `invoice`, `invoice_line`, `credit_note`, `payment`, `installment`, `expense_category`, `expense`, `cash_closing`, `commission_rule`
* **انبار:** `stock_category`, `stock_unit`, `stock_location`, `supplier`, `stock_item`, `purchase(+line)`, `stock_batch`, `stock_movement`, `service_material`, `supplier_payment`, `lab_case`
* **LAN:** `lan_identity`, `lan_device`
* **Backup/اعلان:** `backup_log`, `backup_device`, `backup_copy`, `notification_read`

قواعد کلیدی: کلید اصلی UUIDv7؛ پول ×100 INTEGER؛ زمان UTC ISO-8601؛ Soft Delete (`deleted_at`) در داده بالینی/مالی؛ فاکتور/رسید پس از صدور تغییرناپذیر؛ مقدارهای مرجع با ID و جدول ترجمه (نه رشتهٔ متنی)؛ شماره‌گذاری دندان پیش‌فرض FDI (Universal/Palmer قابل انتخاب)؛ ID دادهٔ Seed قطعی (`ids::seed_id`). `schema_migrations` با Checksum SHA-256 و `user_version`: برنامهٔ قدیمی‌تر دیتابیس جدیدتر را باز نمی‌کند.

> **مهم برای لایسنس:** لایسنس عمداً داخل این دیتابیس **نیست**؛ در `license.json`, `license_key.txt`, `install.json` کنار دیتابیس است تا Restore لایسنس تمام‌شده را زنده نکند.

## ۶. احراز هویت و نقش‌ها

Roleها مجموعه‌ای از Permission هستند که در کد (`core/src/auth.rs`) تعریف و در هر Startup با DB همگام می‌شوند. Roleهای سیستمی: `owner`, `administrator`, `receptionist`, `doctor`, `accountant`, `assistant`؛ کلینیک می‌تواند Role سفارشی بسازد.

Permissionها: `patients.view/edit`, `appointments.view/edit/treat`, `doctors.manage`, `clinical.view/edit`, `billing.view/edit/void/override`, `reports.view`, `inventory.view/manage`, `users.manage`, `settings.manage`, `backup.view/create/restore`, `audit.view`.

| نقش | دسترسی کلی |
|---|---|
| owner | همه (فقط او `backup.restore` دارد؛ `OWNER_ONLY`) |
| administrator | همه بجز restore |
| receptionist | بیمار، نوبت، فاکتور/پرداخت، دیدن انبار |
| doctor | بیمار، نوبت (درمان)، بالینی، دیدن مالی/انبار؛ داشبورد فقط کار خودش |
| accountant | مالی کامل، گزارش، انبار |
| assistant | دیدن بیمار/نوبت/بالینی، شروع درمان، دیدن انبار |

هر عملیات حساس در `audit_log`. لایسنس: متدهای `license.*` فقط برای مدیران (تست `the_plan_sets_the_lan_client_limit_and_only_managers_use_the_license_pages`). متدهای عمومی بدون ورود: `backup.scan`, `backup.restore_new`.

## ۷. Migrationها

* ابزار: خود Core (بدون ابزار خارجی). فایل‌ها `core/migrations/NNNN_name.sql`، ثبت‌شده در `core/src/db.rs`.
* هر Migration در یک Transaction `IMMEDIATE`، قبلش Backup خودکار، Checksum SHA-256 در `schema_migrations`.
* اجرا هنگام باز کردن دیتابیس (Startup). جدیدترین: `0018_backup_manager.sql`.
* **Control Center (Neon)** Migration جدا دارد و در این مخزن نیست (فقط SQL پیشنهادی در ADR-25). نامشخص: فایل Migration نهایی Neon در کدام مخزن است.

## ۸. محیط‌ها و متغیرها

* `ARTAVEO_ENV` = `development | test | production` (پیش‌فرض: debug=development، release=production).
* در production: لایسنس الزامی (بدون آن فقط‌خواندنی)، فقط کلید عمومی production پذیرفته می‌شود. در development/test: کلید توسعه هم پذیرفته می‌شود و بدون لایسنس باز است.
* متغیرها (فقط نام): `ARTAVEO_ENV`, `ARTAVEO_DATA_DIR`, `ARTAVEO_LOG`, `ARTAVEO_LICENSE_URL` (زمان اجرا یا Build)، `ARTAVEO_DEV_SERVER`, `ARTAVEO_GIT_COMMIT`.
* آدرس سرور لایسنس production داخل کد (`PRODUCTION_URL` در `license.rs`) است؛ هیچ Secret در برنامه نیست.
* مسیر داده production: `%ProgramData%\ArtaveoDental` (`clinic.db`, `clinic.key`, `clinic.rkey`, `backups/`, `logs/`).

## ۹. فازها

| فاز | وضعیت | ساخته شد / باز |
|---|---|---|
| 0 | ✅ | Spikeها، ADRهای 03/04/12/13/18–22، جدول ریسک |
| 1 (v0.1.0) | ✅ | Tauri+Core+SQLCipher، Migration، Auth، Audit، Backup پایه، CI سه‌معماری |
| 2 (v0.2.1) | ✅ | Design System، Shell، چندزبانه، Setup Wizard، حالت تک‌دکتر |
| 3 (v0.3.0) | ✅ | بیمار، سوابق پزشکی، جستجوی FTS5، پیوست، Import CSV |
| 4 (v0.4.0/0.4.1) | ✅ | کارکنان، دکتر/صندلی، نوبت، صف پذیرش، فراخوان |
| 5 (v0.5.0/0.6.0) | ✅ | کاتالوگ، چارت دندان، لثه، پلن درمان، یادداشت، نسخه، رضایت‌نامه، Timeline |
| 6 (v0.7.0) | ✅ | فاکتور، پرداخت، اقساط، کمیسیون، هزینه، صندوق، رسید/چاپ. **باز:** ESC/POS Raster و سؤال چاپگر در Wizard (فاز ۱۱) |
| 7 (v0.8.0) | ✅ | حالت LAN. **باز:** سرور فعلاً برنامهٔ پس‌زمینه است نه Windows Service؛ تست روی روتر واقعی (فاز ۱۱) |
| 8 (v0.9.0) | ✅ | انبار، تأمین‌کننده، لابراتوار. **باز:** فرمایش چاپی لابراتوار به Lab Case وصل نیست |
| 9 (v0.10.0) | ✅ | داشبورد، ۲۶ گزارش + Excel، اعلان، جستجوی سراسری، Audit Viewer |
| 10 (v0.11.0) | ✅ | Backup Manager (محلی/USB مورد اعتماد/پوشهٔ ابر، نگهداری، Restore اعتبارسنجی‌شده)، لایسنس، فقط‌خواندنی |
| 10b (v0.11.1) | در جریان | لایسنس مطابق ADR-25 (Neon، توکن JWS، اثرانگشت، تأیید دستی). CI آخرین Push هنوز سبز تأیید نشده؛ Tag نزده‌ام |
| 11 | شروع نشده | Installer نهایی، آپدیت خودکار، Crash/Recovery، QA کامل، Pilot، **تنها تست دستی نهایی مالک** |

## ۱۰. محدودیت‌ها و بدهی‌ها

* `validate` روی Neon هنوز باید به `cc.validate_license_v2` وصل شود و `fingerprint` و `computer_name` را بپذیرد؛ تا آن موقع فعال‌سازی آنلاین عملاً کار نمی‌کند.
* پنل ادمین (تأیید دستگاه، باطل، تمدید، ترایل) ساخته نشده؛ فعلاً توابع `cc.admin_*` در کنسول Neon.
* اثرانگشت: فقط `MachineGuid` ویندوز (تصمیم نهایی؛ ADR-25 با کد یکی شد).
* USB و اثرانگشت روی ویندوز واقعی در تست دستی نهایی دیده می‌شود (نه قبلش).
* به‌روزرسانی خودکار و اعلان نسخه (`LicenseVerify`/آپدیت) کامل نیست (فاز ۱۱).
* ویژگی‌های عمداً خارج از V1: پورتال بیمار، رزرو آنلاین، SMS/WhatsApp، Sync چندشعبه، چندارزی، حسابداری پیشرفته، بیمه، AI، موبایل.
* یک خطای E2E گذرا (۱۳۱) در دو CI دیده شد و علتش اثبات نشد (در اجراهای بعدی پاس).

## ۱۱. نقاط اتصال به لایسنس

**وضع فعلی در کد (واقعی):**

* مدل: لایسنس فقط روی کامپیوتر **سرور**؛ کلاینت‌ها زیر همان لایسنس.
* Core: `LicenseManager` (`core/src/license.rs`) توکن JWS را با کلید عمومی (`kid` انتخاب‌کننده؛ production فقط کلید production) و `nonce`/`installation_id`/`fingerprint` تطبیق می‌دهد. درخواست به `validate`: `{license_key, installation_id, app_version, fingerprint, computer_name, nonce}`.
* اعداد مهلت (`check_every_days`, `offline_grace_days`, `post_expiry_grace_days`, `valid_until`) از توکن می‌آیند، نه کد.
* حالت‌ها: unactivated, trial, active, expired_warning, grace, expired, revoked, pending, blocked, clock_tampered, invalid, needs_verification.
* **گیت یکتا:** `blocked_when_read_only(method)` در `call()` (`core/src/api.rs`) هر متد «تغییر» (`lan::is_change`) را در حالت فقط‌خواندنی رد می‌کند؛ استثنا: موضوع‌های `backup, license, reports, search, notifications, dashboard` و `*mark_printed`. دیدن، چاپ، Export، Backup و Restore هرگز بسته نمی‌شود.
* **سقف کامپیوتر:** `apply_license_limits` (`core/src/core.rs`) مقدار `lan_client_limit = max_computers - 1` را در `setting` می‌نویسد؛ `lan.rs` آن را هنگام Pairing اعمال می‌کند.
* بنر/صفحه: `ui/src/shell/LicenseBanner.tsx`, `ui/src/pages/License.tsx`.

**قابلیت‌هایی که فعلاً پشت لایسنس *نیستند* (طبق roadmap و کد):** همهٔ امکانات در همهٔ پلن‌ها یکسان است؛ تفاوت پلن فقط تعداد کامپیوتر. نمودار دندان، گزارش‌ها، چندکاربره (تعداد کاربر نرم‌افزاری) سقف لایسنسی ندارند. توکن فیلد `features` دارد (مثلاً `"lan"`) ولی **Core امروز آن را نمی‌خواند** (نامشخص/پیاده‌نشده).

اگر قرار شود قابلیتی پشت لایسنس برود، نقطهٔ گیت درست: یا افزودن به `blocked_when_read_only`-مانند در `api.rs::call()` برای متد/Permission خاص، یا خواندن `features` از `Payload` در `license.rs` و نمایش در `LicenseStatus` (`shared/src/license.rs`).

## ۱۲. سؤال‌های باز برای ثبت محصول `dental` در Control Center

۱. **پلن‌ها:** ثبت‌شده در roadmap: basic ۱ کامپیوتر (۳٬۰۰۰ AFN/سال)، standard ۳ (۵٬۰۰۰)، pro ۶ (۷٬۵۰۰)؛ Trial ۳۰ روز. آیا قیمت‌ها در Control Center هم نگه داشته شود یا فقط سقف؟
۲. **قابلیت‌ها (`features`):** امروز همه پلن‌ها یکسان. آیا `features` خالی بماند یا `lan` فقط برای standard/pro باشد؟ (الان basic که ۱ کامپیوتر است عملاً LAN ندارد چون سقف کلاینت ۰ می‌شود.)
۳. **سقف کاربر نرم‌افزاری:** وجود دارد؟ فعلاً نه.
۴. **مدل استقرار:** لایسنس فقط روی سرور؛ آیا Control Center `max_activations` را «همهٔ کامپیوترها» (سرور + کلاینت) می‌شمارد؟ الان Core `max_computers - 1` را کلاینت می‌گیرد، یعنی سرور یکی حساب می‌شود.
۵. **اثرانگشت:** حل شد: فقط `MachineGuid`.
۶. **تغییر سخت‌افزار / نصب ویندوز دوباره:** سیاست انتقال فعال‌سازی چیست (ردیف pending جدید + تأیید دستی؟).
۷. **پیش‌فرض‌های مهلت:** چک ماهانه، ۱۲۰ روز بدون اینترنت، ۱۲۰ روز پس از پایان؛ برای ترایل چک ۷ روز و مهلت ۴۵ روز در تست‌ها استفاده شده. مقدار نهایی ترایل را مالک تعیین کند.
۸. **`version_blocked`:** توکن این نتیجه را ذکر می‌کند ولی Core آن را جدا مدیریت نمی‌کند؛ سیاست نسخه (حداقل نسخه، نسخهٔ مسدود) باید تعریف شود.
۹. **مالکیت Migration Neon:** کدام مخزن، و آیا کلید توسعه (`cc-development-2026-10`) باید پس از آزمایش حذف شود؟
۱۰. **کانال آفلاین:** پاسخ آفلاین همان توکن امضاشده است (`offline_apply`)؛ برای صدور آن پنل ادمین چه خروجی‌ای باید بدهد؟
