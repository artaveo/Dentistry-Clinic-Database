# Artaveo Dental

## Master Development Roadmap — V1 (Revision 2)

### تعریف محصول

Artaveo Dental یک **Windows Desktop Application** برای مدیریت کامل کلینیک دندان‌پزشکی است؛ با قابلیت کارکرد آفلاین، دیتابیس محلی، کارکرد روی یک کامپیوتر یا چند کامپیوتر در شبکه داخلی کلینیک، License مرکزی، Backup چندلایه، رابط مدرن و چندزبانه، و معماری Single-Codebase که بتواند برای کلینیک‌های متعدد توزیع شود.

در پایان این رودمپ، سیستم باید **۱۰۰٪ کامل و قابل استفاده و فروش** باشد. ترتیب فازها طوری چیده شده که هسته اصلی زودتر ساخته و تست شود و هر فاز روی فاز قبلی بنا شود؛ هیچ امکانی از V1 حذف نشده است.

---

# بخش صفر — تصمیم‌های قطعی پروژه

### تصمیم‌های کسب‌وکار (توسط مالک محصول)

| موضوع | تصمیم |
|---|---|
| سیستم‌عامل | فقط **Windows 10 و Windows 11**. Windows 7/8 پشتیبانی نمی‌شود. |
| نوع پردازنده | **همه انواع رایج**: x64 (Intel/AMD 64-bit — اصلی)، x86 (Windows 10 نسخه 32-bit روی کامپیوترهای قدیمی)، ARM64 (لپ‌تاپ‌های Snapdragon). برای هر کدام Installer جدا ساخته می‌شود و وب‌سایت نسخه مناسب را پیشنهاد می‌دهد. |
| ارز | فقط **افغانی (AFN)**. ساختار داده برای افزودن ارز در آینده باز می‌ماند، ولی UI و منطق V1 تک‌ارزی است. |
| مدل License | **هر کلینیک یک License**؛ تعداد کامپیوترهای مجاز بر اساس **پلن** تعیین می‌شود. |
| حالت پیش‌فرض | **تک‌کامپیوتر**. حالت چندکامپیوتری (LAN) اختیاری است و بدون نیاز به IT راه‌اندازی می‌شود. |
| محدوده V1 | تمام امکانات این سند جزو V1 هستند. |
| حریم داده بیماران | **Artaveo هیچ‌وقت داده بیماران را نگهداری نمی‌کند.** Cloud Backup هر کلینیک به فضای ابری **متعلق به خود کلینیک** می‌رود، نه سرور Artaveo. |
| پلن‌ها | همه امکانات در همه پلن‌ها یکسان است؛ تفاوت فقط در تعداد کامپیوتر. **پایه**: ۱ کامپیوتر، ۳٬۰۰۰ AFN/سال · **استاندارد**: تا ۳ کامپیوتر، ۵٬۰۰۰ AFN/سال · **حرفه‌ای**: تا ۶ کامپیوتر، ۷٬۵۰۰ AFN/سال. Trial رایگان ۳۰ روزه. |
| نصب و راه‌اندازی | فعلاً به‌صورت حضوری و رایگان توسط Artaveo انجام می‌شود. |
| انتشار برنامه | از وب‌سایت Artaveo، بدون Code Signing Certificate (هشدار SmartScreen پذیرفته شده است). |

### تصمیم‌های فنی (Architecture Decision Records)

| # | موضوع | تصمیم | دلیل |
|---|---|---|---|
| ADR-01 | محل Business Logic | تمام منطق برنامه و دسترسی به دیتابیس در **Rust Core** (داخل Tauri). React فقط UI است و از طریق Command/API با Core حرف می‌زند. | امنیت (دور زدن Permission/Audit ممکن نیست) + امکان حالت شبکه با همان کد. |
| ADR-02 | لایه انتقال | یک **API Contract** واحد برای Core. در حالت تک‌کامپیوتر از Tauri IPC، در حالت شبکه از HTTPS/WebSocket روی LAN استفاده می‌شود. | یک کد، دو حالت اجرا. |
| ADR-03 | دیتابیس | SQLite با **SQLCipher** (رمزنگاری کامل فایل)، WAL، Foreign Keys، جداول STRICT. کلید خام ۲۵۶ بیتی؛ Integrity Check کامل در پس‌زمینه، نه در Startup ([جزئیات فاز ۰](docs/adr/ADR-03-database.md)). | امنیت داده پزشکی در صورت سرقت کامپیوتر. |
| ADR-04 | نگهداری کلید رمز | کلید دیتابیس با **Windows DPAPI** (Machine Scope) محافظت می‌شود + یک **Recovery Key** که هنگام نصب برای Owner چاپ/ذخیره می‌شود؛ نسخه Wrap‌شده با Recovery Key همراه هر Backup ([جزئیات فاز ۰](docs/adr/ADR-04-key-storage.md)). | بدون Recovery Key، Restore روی کامپیوتر جدید غیرممکن است. |
| ADR-05 | شناسه‌ها | **UUIDv7** به‌عنوان Primary Key + شماره‌های انسانی جدا (مثل `P-000123`). | سازگار با Sync و Cloud در آینده، قابل مرتب‌سازی زمانی. |
| ADR-06 | پول | مبالغ به‌صورت **INTEGER** (کوچک‌ترین واحد، ×100) ذخیره می‌شوند؛ هرگز REAL/Float. | جلوگیری از خطای گرد کردن. |
| ADR-07 | زمان | ذخیره به **UTC** (ISO-8601)؛ نمایش با Timezone کلینیک (پیش‌فرض `Asia/Kabul`, +04:30). **نمایش و ورود ساعت در تمام سیستم ۱۲ ساعته** با ق.ظ / ب.ظ (پشتو: غ.م / غ.و، انگلیسی: AM / PM) — تصمیم مالک محصول. | |
| ADR-08 | تقویم | نمایش پیش‌فرض **هجری شمسی**؛ میلادی قابل انتخاب. ذخیره همیشه میلادی/UTC. | استاندارد رسمی افغانستان. |
| ADR-09 | اعداد | ذخیره و جستجو با ارقام لاتین؛ نمایش با ارقام فارسی یا لاتین طبق تنظیمات. ورودی هر دو نوع رقم را می‌پذیرد. | جستجوی شماره تماس و شماره بیمار نباید به نوع رقم وابسته باشد. |
| ADR-10 | حذف داده | **Soft Delete** برای تمام داده‌های بالینی و مالی. Hard Delete فقط برای داده‌های موقت. | Audit و الزامات پزشکی. |
| ADR-11 | اسناد مالی | فاکتور و رسید پس از صدور **غیرقابل ویرایش** هستند. اصلاح فقط از طریق **Void** یا **Credit Note** با ثبت دلیل. | یکپارچگی مالی. |
| ADR-12 | فایل‌های پیوست | فایل‌ها (X-Ray، عکس، اسکن) در **پوشه داده برنامه** با نام مبتنی بر Hash (SHA-256)؛ Metadata در دیتابیس؛ Thumbnail خودکار؛ فایل‌ها نیز رمزنگاری می‌شوند. | دیتابیس سبک می‌ماند، Backup افزایشی ممکن می‌شود. |
| ADR-13 | PDF و چاپ | تولید PDF و چاپ از طریق **موتور WebView2 (Chromium)** با Page Size سفارشی و یک قالب HTML مشترک؛ PDF همیشه در اندازه واقعی سند. پرینتر حرارتی: **ESC/POS Raster** (روش اصلی) و WebView2 Silent Print (پشتیبان). A5/A6 با Page Size واقعی (سینی توسط درایور)، N-up با چیدمان خودمان و خط برش — قطعی‌شده در فاز ۰ ([جزئیات](docs/adr/ADR-13-pdf-printing.md)). | پشتیبانی کامل RTL و اتصال حروف فارسی/پشتو. |
| ADR-14 | شماره‌گذاری دندان | پیش‌فرض **FDI** (ISO 3950)؛ Universal و Palmer در تنظیمات. | |
| ADR-15 | License | فایل License امضاشده با **Ed25519**؛ کلید خصوصی فقط روی سرور مرکزی. | جعل License ممکن نیست. |
| ADR-16 | سرویس‌های مرکزی | Supabase (Postgres + Edge Functions) فقط برای License، Activation، Installation Tracking، Security Events و Update Metadata. **پلن رایگان**. هیچ داده بیمار روی Supabase نیست. | داده هر کلینیک کمتر از چند ده KB؛ پلن رایگان برای ده‌ها هزار کلینیک کافی است. |
| ADR-17 | Cloud Backup | Backup رمزنگاری‌شده در **پوشه همگام‌سازی فضای ابری خود کلینیک** (Google Drive / OneDrive Desktop) نوشته می‌شود و برنامه همگام‌سازی آن را آپلود می‌کند. | بدون هزینه برای Artaveo، بدون نیاز به API و حساب ابری اختصاصی، صف آفلاین را خود Google Drive/OneDrive مدیریت می‌کند، داده بیماران هرگز به Artaveo نمی‌رسد. |
| ADR-18 | امنیت حالت شبکه | Pairing با کد ۶ رقمی از طریق **SPAKE2** + TLS با گواهی Pin‌شده + token برای هر دستگاه ([جزئیات](docs/adr/ADR-18-lan-security.md)). | کد کوتاه بدون امکان Brute-force آفلاین یا MITM. |
| ADR-19 | فونت‌ها | **Vazirmatn** برای حروف دری/پشتو + **Inter** برای حروف لاتین (از v0.2.1) + Noto Sans Arabic فقط برای ؋؛ همه فونت‌ها همراه برنامه ([جزئیات](docs/adr/ADR-19-fonts.md)). | خروجی یکسان روی همه کامپیوترها. |
| ADR-20 | توزیع WebView2 | Installer استاندارد (`embedBootstrapper`) + Installer آفلاین (`offlineInstaller`)، هر دو از وب‌سایت Artaveo ([جزئیات](docs/adr/ADR-20-webview2.md)). | کلینیک‌های بدون اینترنت. |
| ADR-21 | تقویم شمسی | Core: ماژول Rust خودمان؛ UI: `@internationalized/date`؛ نام ماه‌های افغانی از جدول خودمان ([جزئیات](docs/adr/ADR-21-calendar.md)). | UI و Core هیچ‌وقت اختلاف تاریخ ندارند. |
| ADR-22 | هویت USB | Marker امضاشده (HMAC) روی دستگاه + Serial سخت‌افزاری معتبر؛ تشخیص با BusType ([جزئیات](docs/adr/ADR-22-usb-identity.md)). | Drive Letter و Volume Serial پایدار نیستند. |

### نیازمندی‌های غیرعملکردی (NFR)

| معیار | هدف |
|---|---|
| حداقل سخت‌افزار | Windows 10 (x64، x86 یا ARM64)، 4GB RAM، 2GB فضای خالی (بدون احتساب تصاویر) |
| زمان اجرای برنامه | کمتر از ۳ ثانیه تا صفحه Login |
| جستجوی بیمار | کمتر از ۲۰۰ms با ۱۰۰٬۰۰۰ بیمار |
| باز شدن پروفایل بیمار | کمتر از ۵۰۰ms |
| تأخیر در حالت شبکه (LAN) | عملیات معمول کمتر از ۳۰۰ms |
| حداکثر از دست رفتن داده | صفر برای تراکنش‌های تأییدشده (ACID + WAL) |
| Backup روزانه | کمتر از ۲ دقیقه برای دیتابیس ۱GB |

---

# معماری پایه

### حالت ۱ — تک‌کامپیوتر (پیش‌فرض)

```text
┌──────────────── Windows PC ────────────────┐
│  React + TypeScript (UI)                   │
│          │  Tauri IPC                      │
│  Rust Core (Business Logic, Auth, Audit)   │
│          │                                 │
│  SQLite + SQLCipher   +   Attachments      │
└────────────────────────────────────────────┘
```

### حالت ۲ — چندکامپیوتری (LAN، اختیاری)

```text
            ┌──────── Clinic Server PC ────────┐
            │  Artaveo Dental (Server Mode)    │
            │  Rust Core + SQLite + Files      │
            │  LAN API (HTTPS + WebSocket)     │
            │  Backup Manager                  │
            └───────────────┬──────────────────┘
                            │  Wi-Fi / LAN / Router معمولی
        ┌───────────────────┼───────────────────┐
  Reception PC          Doctor PC 1          Doctor PC 2
  (Client Mode)         (Client Mode)        (Client Mode)
```

اصول حالت شبکه:

* همان برنامه، همان Installer؛ فقط در Setup انتخاب می‌شود: **«این کامپیوتر سرور کلینیک است»** یا **«اتصال به سرور کلینیک»**.
* داده **فقط روی کامپیوتر سرور** است. Clientها دیتابیس محلی ندارند؛ بنابراین هیچ تداخل یا Sync پیچیده‌ای وجود ندارد.
* **کشف خودکار سرور** در شبکه (mDNS)؛ کاربر IP وارد نمی‌کند.
* **اتصال با کد ۶ رقمی** که روی صفحه سرور نمایش داده می‌شود (Pairing). پس از Pairing، ارتباط با گواهی TLS مخصوص همان سرور رمزنگاری و Pin می‌شود.
* Installer قانون **Windows Firewall** لازم را خودکار اضافه می‌کند. هیچ تنظیم دستی شبکه لازم نیست.
* سرور می‌تواند به‌صورت **Background Service** اجرا شود تا با بسته‌شدن پنجره برنامه، Clientها قطع نشوند.
* اگر سرور خاموش باشد، Client پیام واضح نشان می‌دهد: «کامپیوتر سرور کلینیک روشن نیست».
* تغییرات به‌صورت **Real-time** (WebSocket) به همه Clientها اطلاع داده می‌شود؛ مثلاً ورود بیمار در پذیرش فوراً در صفحه دکتر دیده می‌شود.
* تعداد Clientهای مجاز طبق **پلن License** کنترل می‌شود.
* تبدیل تک‌کامپیوتر به سرور در هر زمان بدون از دست رفتن داده ممکن است.

### Central Services (اینترنت — اختیاری برای عملیات روزانه)

```text
Central Services (Supabase)
    ├── License & Activation
    ├── Installation Tracking
    ├── Security Events
    ├── (بدون داده بیمار — Cloud Backup در فضای ابری خود کلینیک)
    ├── App Update Metadata
    └── Vendor Admin Panel (مدیریت مشتریان و License)
```

**Cloud نباید Single Point of Failure برای عملیات روزمره کلینیک باشد.**

قطع اینترنت نباید مانع مشاهده بیمار، ثبت درمان، نوبت، پرداخت یا صدور رسید شود.

---

# سیاست تست دستی مالک محصول

CI در هر نسخه برنامه را روی هر سه معماری (x64، x86، ARM64) نصب و اجرا می‌کند. بنابراین تست دستی مالک محصول:

| نوع تغییر | تست دستی روی |
|---|---|
| ظاهر، متن‌ها، فرم‌ها، منطق برنامه (بیشتر فازها) | **فقط یک کامپیوتر** (لپ‌تاپ مالک) |
| نصب، آپدیت، رمزنگاری/کلید، شناسه سخت‌افزار و License | هر سه نوع پردازنده (x64 الزامی، ARM64 الزامی، x86 در صورت دسترسی) |
| چاپ (فاز ۶) | انواع پرینتر (حرارتی، لیزری، جوهرافشان) — نوع پردازنده مهم نیست |
| شبکه چندکامپیوتری (فاز ۷) | حداقل ۲ تا ۳ کامپیوتر واقعی در یک شبکه |
| USB و Backup (فاز ۱۰) | چند فلش و هارد اکسترنال واقعی |
| انتشار نهایی (فاز ۱۱) | **همه موارد بالا** |

---

# قوانین مشترک تمام فازها (Definition of Done هر Feature)

هیچ Feature‌ای «تمام» حساب نمی‌شود مگر اینکه:

1. منطق آن در Rust Core باشد و از طریق API Contract در دسترس باشد.
2. **Permission** مربوطه تعریف و اعمال شده باشد.
3. عملیات حساس آن در **Audit Log** ثبت شود.
4. تمام متن‌های UI در فایل‌های ترجمه (فارسی، پشتو، انگلیسی) باشند.
5. در **RTL و LTR** و در **Light و Dark** درست نمایش داده شود.
6. تغییرات Schema از طریق **Migration** باشد و Migration تست شده باشد.
7. **Unit Test** برای منطق و **Integration Test** برای API نوشته شده باشد و در CI پاس شود.
8. در حالت **آفلاین** کار کند.
9. با **داده حجیم آزمایشی** (Seed بزرگ) کارایی آن بررسی شده باشد.
10. هر فرم **اعتبارسنجی زنده** داشته باشد: راهنمای فرمت زیر هر کادر، قرمز شدن همان کادر و پیام دقیق زیر آن قبل از زدن دکمه؛ خطاهای Core با نام فیلد برگردند (OF-001 تا OF-003).
11. مواردی از [بازخورد مالک محصول](docs/feedback/owner-feedback.md) که فاز هدفشان این فاز یا قبل از آن است، رفع شده باشند.
12. **خطاهای CI بدون نیاز به مالک قابل خواندن باشند:** خروجی clippy، cargo build/test، lint و تست‌های UI (پیام خطا با نام فایل و شماره خط) به‌صورت **GitHub Annotation** در خلاصه همان اجرا ثبت شود، طوری که بدون ورود به GitHub از API عمومی `check-runs/{job_id}/annotations` خوانده شود. وقتی CI شکست خورد، سشن **اول** خطا را از همین راه می‌خواند و فقط اگر ممکن نبود از مالک محصول کمک می‌خواهد.
13. **ابزارهای سنگین روی لپ‌تاپ مالک فقط موقت:** سشن می‌تواند هر ابزاری که برای کارش لازم است نصب کند (Visual Studio Build Tools، Windows SDK، شبیه‌ساز و…)، ولی **وقتی کارش با آن تمام شد باید آن را کامل حذف (Uninstall) کند** و فایل‌های موقت، کش‌ها و خروجی‌های بزرگ خودش را هم پاک کند. در گزارش پایان کار بنویسد چه نصب کرد و چه حذف کرد. (لپ‌تاپ ARM64 مالک Rust Core را نمی‌سازد؛ ساخت و تست Rust در CI.)
14. **هیچ ورودی کاربر گم نشود:** هر فرم طولانی پیش‌نویس خودکار دارد؛ با قفل صفحه، کوچک کردن پنجره، رفتن به صفحه یا برنامه دیگر و حتی بستن برنامه، اطلاعات نیمه‌تمام حفظ می‌شود و فقط با «لغو» یا ذخیره پاک می‌شود (OF-020).

---

# PHASE 0 — Technical Spikes & Risk Validation

هدف: قبل از ساخت محصول، ریسک‌های فنی بزرگ با نمونه‌های کوچک (Proof of Concept) حل شوند.

### 0.1 Spikeها

| Spike | سؤالی که باید جواب داده شود |
|---|---|
| Tauri 2 + Rust Core + SQLCipher | Build، رمزنگاری، Migration و DPAPI روی Windows 10/11 کار می‌کند؟ |
| RTL PDF | PDF فارسی/پشتو با متن مختلط انگلیسی و اعداد، با Page Size دلخواه (A6، 80mm) درست تولید می‌شود؟ |
| Thermal Printing | چاپ مستقیم روی پرینتر حرارتی 58/80mm بدون Dialog ممکن است؟ (WebView2 Silent Print در برابر ESC/POS) |
| A4 Compact Print | رسید کوچک روی کاغذ A4 در محل قابل‌تنظیم چاپ می‌شود؟ |
| Small Paper on Normal Printer | چاپ مستقیم روی کاغذ A5/A6 با پرینتر معمولی (Manual Feed)، و چاپ ۲ یا ۴ رسید روی یک A4 با خط برش |
| Shamsi Calendar | کتابخانه تقویم شمسی برای Date Picker، نمایش و گزارش‌ها |
| LAN Mode | کشف سرور با mDNS، Pairing، TLS، WebSocket روی یک Router معمولی |
| WebView2 | نصب خودکار WebView2 در Installer برای سیستم‌هایی که ندارند |
| USB Identity | خواندن Serial/Volume ID واقعی USB در Windows |

### 0.2 خروجی

* گزارش هر Spike با نتیجه و تصمیم نهایی
* به‌روزرسانی ADRها بر اساس نتایج
* Repository اولیه با ساختار پوشه‌ها

**Exit Criteria:** همه Spikeها نتیجه قطعی دارند و هیچ ریسک فنی «نامعلوم» در جدول ریسک باقی نمانده.

> **وضعیت فاز ۰: ✅ انجام شد.** گزارش‌ها: [docs/spikes](docs/spikes/README.md) · ADRهای به‌روز: [docs/adr](docs/adr/README.md) · جدول ریسک فنی: [docs/risk-register.md](docs/risk-register.md) · تأیید روی سخت‌افزار فیزیکی: [چک‌لیست](docs/spikes/hardware-checklist.md)

---

# PHASE 1 — Engineering Foundation

هدف: ساخت «اسکلت واقعی محصول»، نه UI نهایی.

### 1.1 Desktop Foundation

* Tauri 2، React، TypeScript، Vite
* ساختار Workspace: `core` (Rust)، `app` (Tauri shell)، `ui` (React)، `shared` (API Contract و Typeها)
* تولید خودکار Typeهای TypeScript از Contract در Rust
* Application Versioning (SemVer)
* Dev / Test / Production configuration
* Structured Logging با چرخش فایل (Log Rotation)
* Windows Build و Installer اولیه برای هر سه معماری: x64، x86 (32-bit) و ARM64 (CI هر سه را می‌سازد و تست می‌کند)

### 1.2 CI و کیفیت کد

* CI (GitHub Actions): Build، Lint، Format، Unit Test، Integration Test
* Test Database و Seed آزمایشی بزرگ (مثلاً ۱۰۰٬۰۰۰ بیمار)
* E2E Test Framework برای UI (Playwright/WebDriver)

### 1.3 Local Data Architecture

* SQLCipher + WAL + Foreign Keys + STRICT
* Transactions و Prepared Queries
* Migration Engine با Schema Versioning
* Backup خودکار دیتابیس **قبل از هر Migration**
* Integrity Check هنگام Startup
* Index Strategy

### 1.4 Data Conventions

هر جدول عملیاتی:

```text
id            UUIDv7 (TEXT, PK)
created_at    UTC
created_by    user_id
updated_at    UTC
updated_by    user_id
deleted_at    UTC | NULL      ← Soft Delete
version       INTEGER         ← Optimistic Locking (برای حالت چندکامپیوتری)
```

* `version` برای جلوگیری از بازنویسی همزمان: اگر دو کاربر همزمان یک رکورد را ویرایش کنند، دومی پیام «این رکورد توسط کاربر دیگری تغییر کرده» می‌گیرد.
* هر کلینیک دیتابیس جداگانه خودش را دارد؛ بنابراین ستون `clinic_id` در جداول عملیاتی لازم نیست. Clinic ID در Metadata دیتابیس و License نگهداری می‌شود.

### 1.5 Reference Data Architecture

```text
reference_type          (gender, blood_group, marital_status, payment_method, ...)
reference_item          id, type_id, code, sort_order, is_active, is_system
reference_translation   reference_item_id (FK), language_code, label
```

برای جغرافیا جداول مستقل با سلسله‌مراتب واقعی:

```text
province               id, code
district               id, province_id (FK)
geo_translation        entity_type, entity_id, language_code, label
```

* داده بیمار فقط ID ذخیره می‌کند: `patient.gender_id`، `patient.province_id`، `patient.district_id`.
* هر ID در هر زبان نام خودش را دارد.
* Statusهای منطقی سیستم (Appointment Status، Invoice Status و...) که رفتار کد به آن‌ها وابسته است، به‌صورت **Enum در کد** + ترجمه در فایل‌های i18n هستند؛ Reference Table برای داده‌های قابل توسعه توسط کلینیک است.
* Seed کامل ولایات و ولسوالی‌های افغانستان در سه زبان.

### 1.6 Identity & Security Foundation

* Local Authentication با Argon2id
* Session Management با Timeout قابل تنظیم و **Lock Screen** (قفل سریع هنگام ترک میز)
* Permission Model مبتنی بر Permission‌های Granular + Roleها به‌عنوان مجموعه Permission
* Roleهای پیش‌فرض: Owner، Administrator، Receptionist، Doctor، Accountant، Assistant
* **Recovery Key** برای Owner (بازیابی رمز و دیتابیس بدون اینترنت)
* Audit Log Infrastructure (از همین فاز فعال است):

```text
who · what · when · entity · entity_id · old_value · new_value · computer
```

* Audit Log فقط-افزودنی (Append-only) است و از UI قابل حذف نیست.

### 1.7 Basic Local Backup

از همین فاز، Backup روزانه محلی و دستی فعال است تا هیچ داده آزمایشی/واقعی بدون Backup نماند. (نسخه کامل در فاز ۱۰)

### 1.8 تحویل برای تست دستی مالک محصول

در پایان فاز ۱ (و هر فاز بعدی که خروجی قابل نصب دارد):

* انتشار Installerها در **GitHub Releases** (نه فقط Artifact) با نام‌های واضح:
  `ArtaveoDental-Setup-x64.exe` · `ArtaveoDental-Setup-x86.exe` · `ArtaveoDental-Setup-arm64.exe` و نسخه‌های `-offline` آن‌ها
* فایل `docs/testing/phase-1-manual-test.md` به **فارسی ساده و غیرفنی**: کدام فایل را روی کدام کامپیوتر نصب کن، دقیقاً چه دکمه‌ای بزن، چه چیزی باید ببینی، و جدول ثبت نتیجه (قبول/رد + عکس صفحه)
* صفحه **«درباره برنامه / System Info»** که نسخه، نوع پردازنده (x64/x86/ARM64)، وضعیت رمزنگاری دیتابیس و آخرین Backup را نشان دهد تا مالک محصول بدون ابزار فنی بتواند نتیجه را ببیند

**Exit Criteria:** برنامه نصب می‌شود، کاربر Owner ساخته می‌شود، Login کار می‌کند، Migration و Backup محلی کار می‌کند، CI سبز است، Installerهای سه معماری در GitHub Releases منتشر شده‌اند و راهنمای تست دستی آماده است.

> **وضعیت فاز ۱: ✅ تحویل شد — [v0.1.0](https://github.com/artaveo/Dentistry-Clinic-Database/releases/tag/v0.1.0)** (۶ Installer برای x64/x86/ARM64، آنلاین و آفلاین؛ CI روی هر سه معماری نصب و اجرا را تست می‌کند). راهنمای تست دستی: [docs/testing/phase-1-manual-test.md](docs/testing/phase-1-manual-test.md).
> ولسوالی‌ها: ۴۰۴ ولسوالی (همراه با ۳۴ مرکز ولایت) از فهرست مالک محصول Seed شد ([core/seeds/README.md](core/seeds/README.md)) — بعد از v0.1.0 اضافه شده و با نسخه بعدی منتشر می‌شود.
> باز (مانع شروع فاز ۲ نیست):
> * تست دستی مالک محصول با راهنمای بالا و ثبت نتیجه؛ موارد «رد» قبل از فاز ۲ اصلاح می‌شوند.
> * بازبینی متن‌های پشتو (رابط برنامه، ولایات، ولسوالی‌ها) توسط گوینده بومی.
> * تأیید چند نام ولسوالی: بدخشان «حامی» و «یمگان»، پکتیا «سمکنی»، «زرمت» و «لجه منگل»، تخار «درقد»، غور «چارسده»، هلمند «نوزاد».

---

# PHASE 2 — Design System, Application Shell, Localization & Clinic Setup

### 2.1 Design System

* Typography (فونت فارسی/پشتو + لاتین هماهنگ)
* Spacing، Radius، Shadows
* Glass Surfaces (به‌صورت ملایم و خوانا) + **حالت کارایی** که افکت‌های سنگین را روی کامپیوترهای ضعیف غیرفعال می‌کند
* Cards، Tables، Inputs، Buttons، Dialogs، Toasts، Modals
* Empty / Loading / Error States
* Light و Dark Theme
* Keyboard Navigation و Focus States

### 2.1b Branding (هویت کلینیک و Artaveo)

قاعده: برنامه ابزار **کلینیک** است؛ بیمار و کارمند باید هویت کلینیک را ببینند، نه سازنده نرم‌افزار.

**معماری برند (Endorsed Brand):** سه سطح هویت، هر کدام جای خودش:

| سطح | لوگو | فایل‌ها |
|---|---|---|
| ۱. کلینیک | لوگوی خود کلینیک (توسط کلینیک وارد می‌شود) | — |
| ۲. محصول | **Artaveo Dental** — نماد دندان با روبان تاخورده، الهام‌گرفته از Artaveo (تصمیم مالک محصول، ۱۰ میزان ۱۴۰۵) | `branding/artaveo-dental/` |
| ۳. شرکت سازنده | **Artaveo** (Digital Development) — فقط به‌صورت «by Artaveo» کوچک | `branding/artaveo/` |

| محل | چه چیزی |
|---|---|
| هدر اصلی برنامه (بالای صفحه بعد از ورود) | **لوگو و نام کلینیک** (برجسته) |
| رسید، نسخه، فاکتور و همه اسناد چاپی | **لوگو، نام، آدرس و تلفن کلینیک**؛ در پایین سند با فونت خیلی کوچک و اختیاری: نماد تک‌رنگ Artaveo Dental + «Artaveo Dental» |
| آیکن برنامه (دسکتاپ، منوی Start، نوار وظیفه، نوار عنوان پنجره)، Installer | **نماد فلت Artaveo Dental** |
| پایین منوی کناری (نسخه برنامه) | **نماد کوچک Artaveo Dental** + «Artaveo Dental 0.x.y» |
| صفحه شروع (Splash) | **نسخه پریمیوم تیره Artaveo Dental** |
| صفحه ورود | **لوگوی افقی Artaveo Dental** (روشن/تیره طبق تم) + نام و لوگوی کلینیک |
| «درباره برنامه» | **Artaveo Dental** (نسخه پریمیوم یا عمودی)، نسخه، پشتیبانی؛ زیر آن یک خط کوچک «by Artaveo» با لوگوی شرکت |

برای محصولات آینده Artaveo در حوزه‌های دیگر همین الگو تکرار می‌شود: نماد اختصاصی همان کسب‌وکار، با همان زبان طراحی روبان تاخورده، همان رنگ‌های ذغالی و طلایی و همان فونت.

* **لوگوی کلینیک** توسط خود کلینیک وارد می‌شود: در Setup Wizard (اختیاری) و هر زمان بعد از آن در «تنظیمات ← اطلاعات کلینیک» (PNG/JPG/SVG، برش و پیش‌نمایش؛ نسخه مناسب چاپ سیاه‌وسفید هم ساخته شود).
* اگر کلینیک لوگو ندارد، حرف اول نام کلینیک در یک نشان رنگی (با رنگ اصلی کلینیک) نمایش داده می‌شود.
* فایل‌های لوگوی Artaveo در `branding/artaveo/` (SVG + PNG 1024px + نسخه تک‌رنگ) نگهداری می‌شوند و همه آیکن‌ها از آن‌ها تولید می‌شوند.

### 2.2 Application Shell

* Sidebar، Top Bar، User Menu، Clinic Identity
* Breadcrumbs، Page Layout
* Modal و Dialog System
* Notification Bell (زیرساخت؛ محتوا در فاز ۹)
* Command Palette (`Ctrl+K`) برای ناوبری سریع (جستجوی سراسری کامل در فاز ۹)
* نشانگر وضعیت: Online/Offline، اتصال به سرور (در حالت LAN)، وضعیت Backup

### 2.3 Multilingual Engine

سه زبان از ابتدا:

```text
فارسی ← Default
پښتو
English
```

مدیریت:

* RTL/LTR واقعی
* متن مختلط فارسی/پشتو + انگلیسی
* ارقام فارسی و لاتین (ADR-09)
* تاریخ شمسی و میلادی (ADR-08)
* نمایش مبلغ (`2,500 AFN` / `۲٬۵۰۰ افغانی`)
* Print و PDF rendering

```text
بیمار: John Smith
درمان: Root Canal Treatment
مبلغ: 2,500 AFN
```

نباید Layout خراب شود.

### 2.4 Data Language ≠ Interface Language

داده‌ای که کاربر وارد می‌کند (`Ahmad Khan`، `احمد خان`، `Ahmad خان`) همه معتبر است و **ترجمه یا Normalize اجباری نمی‌شود**.

برای موارد تحت کنترل سیستم (Service Names، Treatment Names، Statuses، Reference Data) از Localization استفاده می‌شود.

جستجو باید تفاوت‌های نوشتاری رایج را نادیده بگیرد: «ی/ي»، «ک/ك»، ارقام فارسی/لاتین، فاصله و نیم‌فاصله.

### 2.5 Clinic Setup Wizard (اولین اجرا)

```text
Welcome
   ↓
Language
   ↓
حالت نصب: تک‌کامپیوتر / سرور کلینیک / اتصال به سرور
   ↓
Clinic Info: Name, Logo, Province, District, Address, Phone
   ↓
Working Hours, Calendar (شمسی/میلادی)
   ↓
نوع کلینیک: تک‌دکتر / چند دکتر
   ↓
Theme & Branding: Primary/Secondary/Accent Color
   ↓
Owner User + Recovery Key
   ↓
Trial / Activation
   ↓
Backup Setup
   ↓
Finished
```

### 2.6 Solo Clinic Mode (کلینیک تک‌دکتر)

بخش بزرگی از کلینیک‌های هدف فقط یک دکتر دارند. معماری و داده برای هر دو حالت یکی است (دکتر همیشه یک رکورد واقعی است)، ولی در حالت تک‌دکتر UI ساده‌تر می‌شود:

* انتخاب دکتر در فرم‌ها (نوبت، درمان، Plan، نسخه) خودکار و پنهان
* تقویم و صف بدون ستون/فیلتر دکتر
* Doctor Commission و گزارش‌های مقایسه دکترها پنهان
* یک کاربر می‌تواند هم‌زمان Owner و Doctor باشد (و در صورت نبود منشی، کار پذیرش و صندوق را هم انجام دهد)
* داشبورد یکپارچه به‌جای داشبوردهای جدا برای هر Role
* افزودن دکتر دوم در هر زمان، کلینیک را بدون Migration یا از دست رفتن داده به حالت چند دکتر تبدیل می‌کند

**Exit Criteria:** Wizard کامل اجرا می‌شود؛ تمام صفحات Shell در سه زبان و دو Theme بدون خرابی Layout نمایش داده می‌شوند.

> **وضعیت فاز ۲: ✅ تحویل شد — [v0.2.1](https://github.com/artaveo/Dentistry-Clinic-Database/releases/tag/v0.2.1)** (نسخه قابل تست مالک محصول؛ v0.2.0 با بازخوردهای OF-001 تا OF-011 جایگزین شد).
> **v0.2.1:** بازطراحی کامل ظاهر از پایه (تم شیشه‌ای به سبک Windows 11 با حالت کارایی، Vazirmatn + Inter، آیکن‌های Lucide، کامپوننت‌های یکدست، روشن/تیره/RTL/LTR) بر اساس بررسی نرم‌افزارهای مرجع — [جهت طراحی](docs/design/design-direction.md)، [اسکرین‌شات قبل/بعد](docs/design/screenshots/README.md)؛ هویت کلینیک در هدر و لوگوی قابل تغییر؛ اعتبارسنجی زنده در همه فرم‌ها و خطاهای Core با نام فیلد؛ ساعت ۱۲ ساعته در کل سیستم؛ رفع باگ قفل صفحه؛ تست CI ارتقا از v0.1.0 روی هر سه پردازنده؛ قفل خودکار دقیق (OF-012)؛ اصلاحات دور اول بازبینی پشتو (OF-013). همه موارد [بازخورد](docs/feedback/owner-feedback.md) ✅. راهنمای تست دستی: [docs/testing/v0.2.1-manual-test.md](docs/testing/v0.2.1-manual-test.md).
> **v0.2.0 (پیشین):** سیستم طراحی (نشانه‌های رنگ/فاصله/شعاع/سایه، حالت روشن/تیره با انتخاب دستی + حالت کارایی، ADR-24)، پوسته برنامه (Sidebar، TopBar، نشانگر Online/Offline و نوع نصب، Notification Bell زیرساخت، Command Palette با `Ctrl+K`)، موتور چندزبانه (نمایش تاریخ شمسی/میلادی طبق انتخاب کلینیک، نمایش مبلغ آماده برای فازهای بعدی، نرمال‌سازی جستجو در `core::normalize` آماده برای فاز ۳)، و ویزارد کامل راه‌اندازی کلینیک (زبان، حالت نصب، اطلاعات کلینیک + لوگو + جغرافیا، ساعات کاری و تقویم، نوع کلینیک، ظاهر و رنگ، نسخه آزمایشی، پشتیبان‌گیری، مالک + کلید بازیابی) تحویل شدند. راهنمای تست دستی: [docs/testing/phase-2-manual-test.md](docs/testing/phase-2-manual-test.md).
> باز (مانع شروع فاز ۳ نیست):
> * تست دستی مالک محصول با [راهنمای v0.2.1](docs/testing/v0.2.1-manual-test.md)؛ موارد «رد» قبل از فاز ۳ اصلاح می‌شوند.
> * اطلاعات تماس پشتیبانی Artaveo (تلفن +93 790 685 832، artaveo.dev@gmail.com، artaveo-seven.vercel.app) بعد از انتشار v0.2.1 به «درباره برنامه» اضافه شد و با نسخه بعدی منتشر می‌شود (در v0.2.1 هنوز متن عمومی است).
> * بازبینی متن‌های پشتوی این فاز (ویزارد، پوسته برنامه) توسط گوینده بومی: [docs/i18n/pashto-review.md](docs/i18n/pashto-review.md).
> * «حالت نصب: سرور کلینیک / اتصال به سرور» فقط در ویزارد ذخیره می‌شود؛ پیاده‌سازی واقعی LAN همچنان فاز ۷ است.
> * «نسخه آزمایشی» در ویزارد فقط اطلاع‌رسانی است؛ سامانه کامل License/Activation همچنان فاز ۱۰ است.
> * سمت UI تقویم شمسی هنوز از `@internationalized/date`/React Aria استفاده نمی‌کند (نمایش با `Intl` کافی بود)؛ پذیرش آن کتابخانه تا اولین Date Picker واقعی (فاز ۴، نوبت‌دهی) به تعویق افتاد — جزئیات در [ADR-21](docs/adr/ADR-21-calendar.md).
> * «Solo Clinic Mode» فقط به‌صورت یک تنظیم ذخیره‌شده (`clinic.clinic_mode`) آماده است؛ منطق واقعی پنهان‌کردن ستون/فیلتر دکتر از فاز ۴ که موجودیت Doctor ساخته می‌شود فعال خواهد شد.

---

# PHASE 3 — Patients & Medical Records

### 3.0 شروع فاز: برند محصول (OF-014)

قبل از کارهای بیمار: جایگزینی لوگوها با برند **Artaveo Dental** طبق 2.1b و `branding/artaveo-dental/README.md`.

### 3.1 Patient Management

* Patient ID (UUID) + Patient Number (`P-000123`)
* Full Name، Father's Name (رایج در افغانستان برای تفکیک بیماران هم‌نام)
* Preferred Language
* Gender ID، Date of Birth (یا سن تقریبی)
* Phone، Secondary Phone
* Province ID، District ID، Address
* Emergency Contact
* Referral Source (چطور با کلینیک آشنا شد)
* Notes، Registration Date، Status
* **تشخیص بیمار تکراری** هنگام ثبت (نام + شماره تماس مشابه)
* **Merge** دو پرونده تکراری (با Audit)

### 3.2 Medical Alert

بنر هشدار ثابت در بالای پروفایل و در صفحات درمان:

```text
⚠ حساسیت به پنی‌سیلین · دیابت · مصرف داروی رقیق‌کننده خون
```

### 3.3 Medical History

* Allergies
* Current Medications
* Chronic Conditions (دیابت، فشار خون، بیماری قلبی، بارداری، ...)
* Dental History
* Previous Surgeries
* Relevant Notes
* تاریخچه تغییرات (نسخه‌بندی شده)

### 3.4 Patient Search

جستجوی سریع بر اساس نام، بخشی از نام، نام پدر، شماره بیمار، شماره تماس؛ با Index مناسب و FTS5 (Full-Text Search در SQLite).

### 3.5 Attachments

* X-Ray، Dental Photos، Documents، Scans، Consent Forms
* Drag & Drop و Import از پوشه
* Metadata: نوع، تاریخ، دندان مرتبط، توضیح
* Viewer داخلی با Zoom/Pan/Brightness/Contrast
* ذخیره طبق ADR-12

### 3.6 Patient Profile

ساختار تب‌ها (هر تب در فاز مربوط به خودش فعال می‌شود):

```text
Overview · Medical History · Appointments · Dental Chart · Clinical Notes
Treatment Plans · Treatments · Prescriptions · Invoices · Payments
Documents & Images · Timeline · Audit History
```

### 3.7 Data Import

* Import بیماران از **Excel/CSV** با نگاشت ستون‌ها، پیش‌نمایش و گزارش خطا
* Export پرونده بیمار (PDF) و Export داده (Excel/CSV)

**Exit Criteria:** ثبت، جستجو، ویرایش، Merge، Import و پیوست فایل برای بیمار با ۱۰۰٬۰۰۰ رکورد طبق NFR کار می‌کند.

> **وضعیت فاز ۳: ✅ تحویل شد — v0.3.0** (نسخه قابل تست مالک محصول).
> برند محصول Artaveo Dental جایگزین لوگوهای شرکت شد (OF-014)؛ حریم خصوصی صفحه قفل تقویت شد (OF-015). ثبت/ویرایش/حذف بیمار با اعتبارسنجی زنده، تشخیص بیمار تکراری و ادغام دو پرونده (3.1)؛ جستجوی سریع با FTS5 و نادیده‌گرفتن تفاوت رسم‌الخط/نوع رقم (3.4)؛ بنر هشدار پزشکی و صفحه سوابق پزشکی (3.2، 3.3)؛ پیوست فایل رمزنگاری‌شده با نمایشگر Zoom/Pan/Brightness/Contrast و ADR-12 قطعی‌شده (3.5)؛ پروفایل بیمار با تب‌های فعال این فاز و پیش‌نمایش کم‌رنگ تب‌های فازهای بعدی (3.6)؛ ورود/خروج CSV با پیش‌نمایش و گزارش خطا (3.7). همه موارد [بازخورد](docs/feedback/owner-feedback.md) ✅. راهنمای تست دستی: [docs/testing/phase-3-manual-test.md](docs/testing/phase-3-manual-test.md).
> باز (مانع شروع فاز ۴ نیست):
> * تست دستی مالک محصول با راهنمای بالا؛ موارد «رد» قبل از فاز ۴ اصلاح می‌شوند.
> * بازبینی متن‌های پشتوی این فاز توسط گوینده بومی: [docs/i18n/pashto-review.md](docs/i18n/pashto-review.md).
> * Thumbnail پیوست‌ها در مرورگر ساخته می‌شود، نه در Core؛ ساخت Thumbnail واقعی در Core به فاز بعد موکول شد ([ADR-12](docs/adr/ADR-12-attachments.md)).
> * ورود بیماران از CSV سرستون ثابت دارد؛ نگاشت آزاد ستون‌های دلخواه به فاز بعد موکول شد.
> * خروجی PDF پرونده بیمار با موتور PDF فاز ۶ می‌آید؛ فعلاً فقط CSV.

---

# PHASE 4 — Staff, Doctors, Appointments & Reception Workflow

### 4.1 Staff & Users

* مدیریت کاربران و Roleها
* Permissionهای Granular؛ مثلاً Receptionist نوبت می‌سازد ولی به گزارش مالی و تنظیمات دسترسی ندارد
* غیرفعال‌سازی کاربر (بدون حذف، برای حفظ Audit)

### 4.2 Doctors & Chairs

* پروفایل دکتر: تخصص، رنگ در تقویم، وضعیت
* Chair/Room
* Doctor Schedule: Working Days، Working Hours، Breaks، Available Chairs، Leave/Unavailable

### 4.3 Appointment Engine

تقویم Day / Week / Month (شمسی یا میلادی)، با نمای هر دکتر و هر Chair.

Appointment شامل: Patient، Doctor، Chair، Date، Start، End، Reason، Status، Notes.

* جلوگیری از تداخل (Double Booking) برای دکتر و Chair
* Drag & Drop برای جابه‌جایی
* ثبت سریع بیمار جدید از داخل فرم نوبت

### 4.4 Appointment Status

```text
Scheduled → Confirmed → Checked-in → In Treatment → Completed
                    ↘ Cancelled · No-show · Rescheduled
```

### 4.5 Queue / Reception Workflow

```text
Appointment → Patient Arrived → Waiting → Doctor → Treatment → Checkout → Payment
```

* صفحه «صف امروز» برای پذیرش و دکتر
* بیمار بدون نوبت (Walk-in)
* در حالت LAN، تغییر وضعیت در یک کامپیوتر فوراً در کامپیوترهای دیگر دیده می‌شود

### 4.6 Follow-up & Recall

* Next Visit، Follow-up Date
* Recall (مثلاً جرم‌گیری هر ۶ ماه)
* لیست «بیمارانی که باید تماس گرفته شوند»
* چاپ کارت نوبت

**Exit Criteria:** یک روز کامل پذیرش (نوبت، Walk-in، صف، لغو، No-show) بدون خطا اجرا می‌شود.

> **وضعیت فاز ۴: ✅ تحویل شد — v0.4.0** (نسخه قابل تست مالک محصول).
> * **کاربران و نقش‌ها (4.1):** نقش سفارشی با دسترسی دلخواه (به‌جز دسترسی‌های ویژه مالک)، تغییر دسترسی نقش برای کاربران واردشده بلافاصله اعمال می‌شود؛ بازنشانی رمز دیگران (نشست‌های قبلی بسته می‌شوند)، رفع قفل حساب، غیرفعال‌سازی بدون حذف؛ جلوگیری از غیرفعال کردن حساب خود. دسترسی‌های تازه `appointments.treat` و `doctors.manage`.
> * **داکتران و چوکی‌ها (4.2):** پروفایل (تخصص، رنگ در تقویم، وضعیت، اتصال اختیاری به حساب کاربری)، چوکی/اتاق، برنامه هفتگی با چند بازه در روز، استراحت‌ها، چوکی‌های مجاز و رخصتی (با شمار نوبت‌های متأثر).
> * **موتور نوبت (4.3، 4.4):** تقویم روز/هفته/ماه (شمسی یا میلادی) به‌تفکیک داکتر یا چوکی، Drag & Drop، جلوگیری از رزرو همزمان داکتر/چوکی/بیمار، رعایت ساعت کاری/استراحت/رخصتی (با امکان ثبت آگاهانه خارج از برنامه)، ثبت سریع بیمار جدید از داخل فرم نوبت، انتقال نوبت با پیوند دوطرفه، گردش وضعیت‌ها.
> * **صف و پذیرش (4.5):** صفحه «صف امروز» با ستون‌های منتظر، در انتظار، در درمان، تمام‌شده؛ شماره صف به ترتیب رسیدن؛ بیمار بدون نوبت (Walk-in).
> * **پیگیری و تماس (4.6):** پیگیری بعدی هنگام تکمیل ویزیت، فراخوان تکرارشونده (مثلاً جرم‌گیری هر ۶ ماه)، فراخوان خودکار پس از غیبت، «لیست تماس» با ثبت نتیجه تماس و نوبت‌گیری از همان‌جا، تب نوبت‌ها در پروفایل بیمار، چاپ کارت نوبت (چاپ ساده مرورگر؛ موتور چاپ حرفه‌ای فاز ۶).
> * **بازخورد مالک:** OF-016 (دروازه کارایی ۱۰۰ هزار بیمار در CI)، OF-017 (ورود از Excel/CSV با نگاشت ستون) و OF-018 (Thumbnail در Core) رفع شدند.
> * راهنمای تست دستی: [docs/testing/phase-4-manual-test.md](docs/testing/phase-4-manual-test.md).
> باز (مانع شروع فاز ۵ نیست):
> * تست دستی مالک محصول با راهنمای بالا؛ موارد «رد» قبل از فاز ۵ اصلاح می‌شوند.
> * بازبینی متن‌های پشتوی تازه این فاز (حدود ۳۲۰ متن) توسط گوینده بومی: [docs/i18n/pashto-review.md](docs/i18n/pashto-review.md).
> * به‌روزرسانی لحظه‌ای صف روی چند کامپیوتر فعلاً با تازه‌سازی هر ۱۰ ثانیه است؛ ارسال فوری (WebSocket) در فاز ۷ می‌آید.
> * ارسال پیامک/واتس‌اپ یادآوری نوبت در رودمپ نیست و پیاده نشده است؛ لیست تماس برای تماس دستی است.

---

# PHASE 5 — Service Catalog & Dental Clinical Core

> **مرجع اصلی این فاز:** [docs/specs/clinical-workflow-v1.md](docs/specs/clinical-workflow-v1.md) (سوابق پزشکی ساختاریافته M1، تخصص داکترها M2، کاتالوگ چندسطحی M3، درمان چندجلسه‌ای M4، چارت دندان پیشرفته M5، اسناد چاپی M6) — در هر تضاد، این سند بر متن زیر مقدم است.
>
> **این فاز به دو بخش و دو نسخه تقسیم می‌شود** (هر بخش یک سشن جدا):
> * **v0.4.1 (نسخه اصلاحی، قبل از 5A، سشن جدا):** فقط رفع بازخوردهای OF-015 و OF-019 تا OF-043 (اشکالات نوبت‌دهی، فرم‌ها، ورود/خروج Excel، صفحه قفل، پنجره، نقش‌ها) — تا 5A روی پایه سالم ساخته شود.
> * **5A (v0.5.0):** هر بازخورد باقی‌مانده از v0.4.1 ← موتور مشترک سند و چاپ (ADR-13) ← سوابق پزشکی ساختاریافته (M1) ← تخصص داکترها (M2) ← کاتالوگ خدمات چندسطحی (5.1 + M3) ← نسخه (5.9)، رضایت‌نامه (5.10)، اسناد بالینی (5.10b)، کارت نوبت.
> * **5B (v0.6.0):** چارت دندان پیشرفته (5.2 تا 5.4 + M5) ← پلن درمان و درمان چندجلسه‌ای (5.7، 5.8 + M4، اتصال به نوبت‌های فاز ۴) ← یادداشت بالینی (5.6) ← چارت لثه (5.5) ← Timeline (5.11).

این فاز محصول را از یک Clinic Manager ساده به **Dental Management System واقعی** تبدیل می‌کند.


این فاز محصول را از یک Clinic Manager ساده به **Dental Management System واقعی** تبدیل می‌کند.

### 5.1 Service Catalog

```text
Consultation · Filling · Extraction · Root Canal · Crown · Implant · Cleaning · ...
```

برای هر Service: Code، Localized Name، Category، Default Price، وابسته به دندان/سطح بودن، Lab Required، Active/Inactive.

### 5.2 Dental Chart / Odontogram

* Permanent و Primary (Deciduous) Teeth
* Upper/Lower، Left/Right، Individual Tooth، Tooth Surface
* شماره‌گذاری FDI (پیش‌فرض)، Universal، Palmer (ADR-14)
* نمای بصری تعاملی

### 5.3 Tooth States

Healthy، Caries، Filled، Missing، Extracted، Crown، Root Canal، Implant، Bridge، و وضعیت‌های قابل تعریف توسط کلینیک.

* تفکیک **وضعیت موجود (Existing)** از **درمان برنامه‌ریزی‌شده (Planned)** و **درمان انجام‌شده (Completed)** با رنگ‌های متفاوت
* تاریخچه تغییر وضعیت هر دندان

### 5.4 Tooth Surface

Mesial، Distal، Occlusal/Incisal، Buccal/Labial، Lingual/Palatal.

### 5.5 Periodontal Chart

* Probing Depth (۶ نقطه برای هر دندان)، Recession، Bleeding on Probing، Mobility، Furcation
* مقایسه دو معاینه

### 5.6 Clinical Notes (هر Visit)

Chief Complaint، Examination، Diagnosis، Findings، Procedure، Notes، Follow-up، Recommendations.

* **قالب‌های آماده (Templates)** برای یادداشت‌های رایج
* یادداشت پس از Sign-off قفل می‌شود؛ تغییر بعدی فقط به‌صورت Addendum

### 5.7 Treatment Plan

```text
Treatment Plan
   ├── Procedure (از Service Catalog)
   ├── Tooth / Surface
   ├── Doctor
   ├── Estimated Cost (از قیمت Catalog، قابل Override)
   ├── Priority / Phase
   ├── Status
   └── Notes
```

وضعیت‌ها: Proposed → Accepted → In Progress → Completed / Cancelled.

* چاپ Treatment Plan برای بیمار (A4) با برآورد هزینه
* چند Plan جایگزین برای انتخاب بیمار

### 5.8 Treatments (درمان انجام‌شده)

* ثبت درمان با تاریخ، دندان، سطح، دکتر، Service، قیمت، نتیجه
* درمان انجام‌شده به‌صورت خودکار **آماده افزودن به فاکتور** می‌شود (اتصال به فاز ۶)

### 5.9 Prescriptions (نسخه)

* لیست داروهای کلینیک (Formulary) قابل تعریف؛ Seed اولیه از داروهای پرکاربرد دندان‌پزشکی در **فهرست داروهای اساسی وزارت صحت عامه** (آنتی‌بیوتیک، مسکن، دهان‌شویه، ...)
* نام دارو با **نام علمی/تجاری لاتین** (آنچه دواخانه می‌خواند)؛ شکل دارو، قوت (mg)، تعداد؛ **دستور مصرف به زبان بیمار** (دری/پشتو/انگلیسی) — مثلاً «روزانه ۳ بار، بعد از غذا، ۵ روز»
* **نسخه‌های آماده (Templates)**: مثلاً «بعد از کشیدن دندان» یا «عفونت دندان» با یک کلیک، قابل ویرایش
* هشدار در صورت تداخل با Allergy و بیماری‌های ثبت‌شده بیمار (مثلاً بارداری)
* سربرگ: نام و لوگوی کلینیک، **نام دکتر، تخصص و شماره جواز/ثبت**؛ جای امضا و مهر (امضای دستی روی کاغذ؛ تصویر مهر اختیاری)
* پیش‌فرض A5؛ تمام Output Profileهای بخش 6.9 (A4 فشرده، فقط PDF، ...)
* شماره یکتا (`RX-1405-000001`)، ذخیره در پرونده و Timeline بیمار، چاپ مجدد بدون ساخت نسخه جدید

### 5.10 Consent Forms

* قالب‌های رضایت‌نامه (کشیدن دندان، ایمپلنت، عصب‌کشی و...) در سه زبان
* چاپ، و اسکن نسخه امضاشده به‌عنوان پیوست

### 5.10b Clinical Documents (سایر اسناد بالینی)

همه اسناد زیر روی یک **موتور قالب سند مشترک** ساخته می‌شوند (همان موتور رسید و نسخه): سربرگ کلینیک، پر شدن خودکار اطلاعات بیمار/دکتر/تاریخ شمسی، سه زبان، شماره یکتا، ذخیره در پرونده بیمار، چاپ مجدد و PDF در اندازه واقعی. کلینیک می‌تواند متن قالب‌ها را ویرایش کند (با Audit).

| سند | کاربرد | اندازه پیش‌فرض |
|---|---|---|
| دستورات بعد از درمان (Post-op Instructions) | برگه آماده برای بیمار بعد از کشیدن دندان، عصب‌کشی، ایمپلنت، جرم‌گیری و... | A5 |
| معرفی‌نامه (Referral Letter) | ارجاع به متخصص یا داکتر دیگر، با خلاصه پرونده و دلیل ارجاع | A4 |
| درخواست عکس/آزمایش | درخواست OPG، CBCT، عکس پری‌اپیکال یا آزمایش خون به مرکز رادیولوژی/لابراتوار طبی | A5 |
| تصدیق طبی / رخصتی | تأیید مراجعه یا استراحت برای محل کار یا مکتب | A5 |
| فرمایش لابراتوار دندان (Lab Work Order) | جزئیات کار برای لابراتوار: دندان، نوع کار، رنگ (Shade)، تاریخ تحویل — متصل به Lab Case فاز 8.6 | A5 |
| برآورد هزینه درمان | همان Treatment Plan چاپی (5.7) | A4 |
| گزارش پرونده بیمار | خلاصه سوابق برای خود بیمار یا داکتر دیگر | A4 |

### 5.11 Patient Timeline

تمام رویدادهای بیمار به ترتیب زمان:

```text
Registered · Appointment · Examination · Diagnosis · Treatment
Prescription · Payment · Document · Follow-up
```

**Exit Criteria:** یک Visit کامل بالینی (معاینه، چارت، Plan، درمان، نسخه، یادداشت) ثبت و چاپ می‌شود.

---

# PHASE 6 — Billing, Payments, Receipts & Professional Printing

### 6.1 Treatment Pricing

قیمت واقعی در زمان Treatment یا Invoice قابل Override است (با Permission و Audit).

### 6.2 Invoice

* Patient، Items (Service، دندان، Quantity، Unit Price)، Discount، Total، Paid، Balance، Payment Status
* ساخت فاکتور از درمان‌های انجام‌شده با یک کلیک
* شماره یکتا: `INV-1405-000001`
* پس از صدور غیرقابل ویرایش (ADR-11)؛ Void و Credit Note با دلیل و Audit

### 6.3 Payments

* Cash، Bank Transfer، روش‌های قابل تعریف
* Partial Payment
* Installment Plan (اقساط با تاریخ سررسید و یادآوری)
* Advance Payment / Credit (پیش‌پرداخت بیمار)
* Refund (با Permission و Audit)

### 6.4 Debt / Balance

سیستم همیشه نشان دهد:

```text
Total · Paid · Discount · Refunded · Remaining
```

* لیست بدهکاران
* Patient Statement (صورت‌حساب کامل بیمار، A4)

### 6.5 Doctor Commission

* درصد یا مبلغ ثابت برای هر دکتر، قابل تعیین برای هر Service
* محاسبه بر اساس پرداخت دریافت‌شده یا درمان انجام‌شده (قابل انتخاب)
* گزارش تسویه دکتر
* قابل غیرفعال‌سازی برای کلینیک‌هایی که لازم ندارند

### 6.6 Clinic Expenses

* ثبت هزینه‌ها: اجاره، معاش، برق، خرید مواد، ...
* دسته‌بندی قابل تعریف
* لازم برای گزارش سود

### 6.7 Daily Cash Closing (بستن صندوق)

* پایان روز: مبلغ مورد انتظار در برابر مبلغ شمارش‌شده
* ثبت اختلاف با دلیل
* گزارش روزانه صندوق قابل چاپ

### 6.8 Receipt Numbering

```text
RC-1405-000001
RC-1405-000002
```

* سال بر اساس تقویم انتخاب‌شده کلینیک
* تخصیص شماره داخل همان Transaction پرداخت (بدون تکرار یا شماره گم‌شده، حتی در حالت LAN)
* **Reprint** هیچ‌وقت Payment جدید ثبت نمی‌کند و روی نسخه دوم «کپی / Reprint» درج می‌شود

### 6.9 Compact Receipt Engine

**Receipt یک Document Page Size واقعی دارد:**

```text
Thermal 58mm
Thermal 80mm
Compact A6
Compact A5
Custom Compact
```

اندازه سند همیشه کوچک و استاندارد است، **مستقل از اینکه کلینیک چه پرینتری دارد یا اصلاً پرینتر دارد یا نه**. فرض اصلی: بسیاری از کلینیک‌ها در ابتدا پرینتر حرارتی ندارند.

#### حالت‌های خروجی (Output Profiles)

| وضعیت کلینیک | خروجی |
|---|---|
| پرینتر حرارتی 58/80mm | چاپ مستقیم روی رول حرارتی |
| پرینتر معمولی با کاغذ A4 | رسید در اندازه واقعی خودش روی بخشی از A4 (محل قابل تنظیم) |
| پرینتر معمولی با کاغذ کوچک A5/A6 | چاپ مستقیم روی کاغذ کوچک از سینی دستی (Manual Feed)؛ اکثر پرینترهای لیزری و جوهرافشان معمولی A5 و بسیاری A6 را پشتیبانی می‌کنند |
| پرینتر معمولی، چند رسید در یک A4 | چاپ ۲ یا ۴ رسید روی یک برگ A4 با **خط برش (Cut Marks)** برای صرفه‌جویی کاغذ |
| بدون پرینتر | فقط **PDF** در اندازه واقعی رسید |

* پیش‌فرض برای A4: رسید در **نیمه بالایی** برگ چاپ می‌شود تا نیمه پایینی برای رسید بعدی (با برگرداندن کاغذ) یا برش قابل استفاده باشد.
* پیش‌نمایش (Print Preview) دقیقاً همان چیزی را نشان می‌دهد که روی کاغذ می‌آید.
* تست چاپ و تنظیم حاشیه (Calibration) برای پرینترهایی که کمی جابه‌جا چاپ می‌کنند.

#### حالت بدون پرینتر

* ذخیره خودکار PDF هر رسید/سند در پوشه سازمان‌یافته (`Receipts/1405/07/RC-1405-000184.pdf`)
* دکمه «باز کردن محل فایل» و «کپی فایل» برای ارسال دستی به بیمار (مثلاً از طریق WhatsApp)
* نمایش رسید روی صفحه برای عکس گرفتن توسط بیمار
* هر زمان پرینتر اضافه شود، فقط Output Profile تغییر می‌کند

در چاپ با پرینتر A4، رسید روی بخشی کوچک از کاغذ قرار می‌گیرد و فضای خالی بزرگ ایجاد نمی‌کند:

```text
┌──────────────────────────── A4 ────────────────────────────┐
│                                                            │
│    ┌─────────────── COMPACT RECEIPT ───────────────┐       │
│    │        KABUL SMILE DENTAL                     │       │
│    │        Receipt RC-1405-000184                 │       │
│    │        Patient: Ahmad Khan                    │       │
│    │        Root Canal        2,500 AFN            │       │
│    │        Paid              1,500 AFN            │       │
│    │        Balance           1,000 AFN            │       │
│    └───────────────────────────────────────────────┘       │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

محل قرارگیری رسید روی صفحه قابل تنظیم است.

### 6.10 PDF

PDF رسید **A4 خالی با رسید کوچک وسط آن نیست**؛ خودش اندازه واقعی دارد:

```text
Page Size = A6
Page Size = 80mm × dynamic height
```

### 6.11 Full Documents (A4)

Full Invoice، Patient Statement، Treatment Plan، Financial Report، Patient Report.

```text
Receipt ≠ A4 Report
```

هر کدام Template مستقل دارند.

### 6.12 Print Settings

* انتخاب Output Profile و پرینتر پیش‌فرض برای هر نوع سند (رسید → حرارتی / A4 فشرده / A5 / فقط PDF، گزارش → A4 / فقط PDF)
* نسخه دکتر (Prescription) نیز همین Profileها را دارد (پیش‌فرض A5)
* در Setup Wizard پرسیده می‌شود: «پرینتر حرارتی / پرینتر معمولی / بدون پرینتر»
* در حالت LAN، هر کامپیوتر پرینتر خودش را دارد
* Branding رسید: لوگو، نام، آدرس، تلفن، متن پایین رسید

**Exit Criteria:** چرخه کامل درمان → فاکتور → پرداخت جزئی → رسید → پرداخت قسط بعدی → بستن صندوق، روی پرینتر حرارتی، A4 و PDF درست اجرا می‌شود.

---

# PHASE 7 — Multi-Computer LAN Mode

زیرساخت این فاز از فاز ۱ آماده است (ADR-01, ADR-02)؛ در این فاز حالت شبکه کامل و قابل استفاده می‌شود.

### 7.1 Server Mode

* فعال‌سازی «این کامپیوتر سرور کلینیک است» (در Setup یا بعداً از تنظیمات)
* LAN API روی HTTPS + WebSocket
* اجرا به‌صورت Windows Service/Background
* شروع خودکار با روشن شدن کامپیوتر
* قانون Firewall خودکار

### 7.2 Client Mode

* کشف خودکار سرور در شبکه (mDNS)
* در صورت عدم کشف خودکار، ورود دستی آدرس به‌عنوان راه دوم
* Pairing با کد ۶ رقمی نمایش‌داده‌شده روی سرور
* TLS Pinning پس از Pairing
* اتصال مجدد خودکار پس از قطع شبکه

### 7.3 Multi-user Consistency

* Optimistic Locking (`version`) برای جلوگیری از بازنویسی همزمان
* Real-time به‌روزرسانی صفحه‌ها (صف، تقویم، وضعیت بیمار)
* نمایش «چه کسی الان این پرونده را باز کرده»

### 7.4 مدیریت Clientها

* لیست کامپیوترهای متصل در سرور (نام، کاربر، آخرین اتصال)
* لغو دسترسی یک کامپیوتر
* کنترل تعداد Client بر اساس پلن License

### 7.5 فایل‌ها و چاپ در حالت شبکه

* آپلود/نمایش پیوست‌ها از Client به سرور
* چاپ روی پرینتر محلی هر Client

### 7.6 تبدیل حالت

* تک‌کامپیوتر → سرور بدون جابه‌جایی داده
* انتقال سرور به کامپیوتر جدید از طریق Backup/Restore

**Exit Criteria:** یک سرور + ۳ Client روی یک Router خانگی معمولی، بدون هیچ تنظیم دستی شبکه، یک روز کاری شبیه‌سازی‌شده را بدون تداخل داده اجرا می‌کنند.

---

# PHASE 8 — Inventory, Suppliers & Dental Lab

### 8.1 Inventory

Items، Categories، Units، Stock، Minimum Stock، Expiry Date، Batch/Lot.

### 8.2 Stock Operations

Purchase، Receive، Consume، Adjustment، Return، Transfer.

* اتصال اختیاری مصرف مواد به Service (مثلاً هر Filling مقدار مشخصی کامپوزیت مصرف کند)

### 8.3 Low Stock

```text
Latex Gloves   ⚠ Low Stock
```

### 8.4 Expiry Monitoring

Expired · Expiring Soon · Valid

### 8.5 Suppliers

Supplier، Contact، Purchases، Outstanding Balance، Payments to Supplier، History.

### 8.6 Dental Lab

برای Crown، Bridge، Denture و سایر کارهای لابراتوار:

```text
Lab Case · Patient · Doctor · Tooth · Lab · Work Type · Shade
Sent Date · Expected Date · Received Date · Status · Cost · Notes
```

* هشدار تأخیر لابراتوار
* هزینه لابراتوار در گزارش سود هر درمان

**Exit Criteria:** خرید، مصرف، هشدار موجودی/انقضا و چرخه کامل یک Lab Case کار می‌کند.

---

# PHASE 9 — Dashboard, Reports, Notifications & Global Search

### 9.1 Dashboard (بر اساس Role)

```text
Today's Patients · Today's Appointments · Completed Treatments
Today's Revenue · Outstanding Balance · Upcoming Appointments
Low Stock · Pending Lab Cases · Backup Status · License Status
```

دکتر فقط اطلاعات خودش را می‌بیند؛ Owner همه چیز را.

### 9.2 Financial Reports

Daily/Monthly Revenue، Payments، Outstanding Balances، Discounts، Refunds، Doctor Revenue، Doctor Commission، Service Revenue، Expenses، **Profit & Loss**، Cash Closing History.

### 9.3 Clinical Reports

Patients، Treatments، Procedures، Tooth Conditions، Doctor Activity، Follow-ups، Recall List، Referral Sources.

### 9.4 Inventory Reports

Stock، Low Stock، Purchase، Consumption، Expiry، Supplier Balance.

### 9.5 Report Engine

* فیلتر بازه تاریخ (شمسی/میلادی)، دکتر، Service
* Export به PDF (A4) و Excel
* چاپ

### 9.6 Notification Center

داخل برنامه + Windows Notification:

```text
Appointment Reminder · Follow-up Due · Installment Due
Low Stock · Expiring Item · Lab Case Delayed
Backup Failed · USB Missing · License Warning · Update Available
Server Disconnected (حالت LAN)
```

### 9.7 Global Search

جستجوی سریع در بیماران، نوبت‌ها، فاکتورها، رسیدها، Lab Caseها؛ با FTS5 و Index، بدون Full-table Scan.

### 9.8 Audit Log Viewer

* مشاهده و فیلتر Audit Log (فقط Owner/Administrator)
* موارد حساس: Delete، Edit Patient، Price Override، Void، Refund، Discount، Permission Change، License Event، Restore

**Exit Criteria:** تمام گزارش‌ها با داده یک سال کلینیک پرکار در کمتر از ۳ ثانیه تولید می‌شوند.

---

# PHASE 10 — Backup, License, Security & Vendor Admin

### 10.1 Offline Guarantee

بدون اینترنت باید کار کنند:

```text
Login · Patients · Appointments · Clinical Data · Dental Chart · Treatments
Invoices · Payments · Reports · Printing · Backup · LAN Mode
```

### 10.2 Backup Manager

Schedule قابل تنظیم (پیش‌فرض `19:00`)، برای هر کلینیک مستقل. در حالت LAN فقط روی سرور اجرا می‌شود.

```text
Create Backup (SQLite Online Backup API، بدون توقف کار)
      ↓
Verify (Integrity Check روی نسخه Backup)
      ↓
Encrypt
      ↓
Local Backup
      ↓
Trusted USB
      ↓
Cloud Queue
```

* Backup پیوست‌ها به‌صورت **افزایشی** (فقط فایل‌های جدید)
* Backup دستی در هر زمان
* Backup خودکار قبل از Update و قبل از Restore

### 10.3 USB Backup

تشخیص Device با Identity واقعی، نه Drive Letter.

```text
New Backup Device Detected
[Use for Backup]   [Ignore]
```

بعد از تأیید: `Trusted`. اتصال‌های بعدی: `Detect → Verify Identity → Automatic Backup`.

### 10.4 Missing USB

```text
19:00 → Warning
19:30 → Warning
20:00 → Warning
...
```

Interval قابل تنظیم. با وصل شدن USB: `USB Detected → Pending Backup → Automatic Transfer`.

### 10.5 Backup Retention

```text
Daily:   14 days
Weekly:  4 copies
Monthly: 3 copies
```

قابل تنظیم، با Cleanup خودکار.

### 10.6 Cloud Backup

Cloud Backup به **حساب ابری خود کلینیک** می‌رود، نه سرور Artaveo (ADR-17).

* کلینیک یک بار پوشه همگام‌سازی Google Drive یا OneDrive را انتخاب می‌کند (فضای رایگان Google Drive: 15GB)
* Backup رمزنگاری‌شده با کلید کلینیک در آن پوشه نوشته می‌شود؛ حتی صاحب حساب ابری بدون Recovery Key نمی‌تواند آن را باز کند
* اگر اینترنت نبود: `Local ✅ · USB ✅/Pending · Cloud: در پوشه همگام‌سازی، منتظر اینترنت`
* با بازگشت اینترنت، برنامه Google Drive/OneDrive خودش آپلود را انجام و ادامه می‌دهد
* برنامه وضعیت همگام‌سازی را تا حد ممکن نمایش می‌دهد و اگر پوشه یا برنامه همگام‌سازی پیدا نشود هشدار می‌دهد
* دیتابیس همیشه؛ تصاویر اختیاری (برای صرفه‌جویی فضا و اینترنت)
* Retention جداگانه برای پوشه ابری (پیش‌فرض: ۷ نسخه روزانه + ۴ هفتگی)
* اتصال مستقیم API به Google Drive/OneDrive در صورت نیاز در نسخه‌های بعدی

### 10.7 Restore

```text
Select Backup → Validate → Preview → Backup فعلی → Restore → Verify
```

* Restore بدون Validation ممکن نیست
* Restore روی کامپیوتر جدید با **Recovery Key**
* فقط Owner، با Audit

### 10.8 License

Flow:

```text
Install → First Activation (Online) → Trial / Paid License
       → Signed Local License File (Ed25519) → Periodic Verification
```

قواعد:

* Trial فقط یک بار برای هر کلینیک/دستگاه
* Activation اولیه آنلاین
* Verification دوره‌ای هر ۷ روز در صورت وجود اینترنت
* Grace Period آفلاین: ۳۰ روز (قابل تنظیم از سرور مرکزی)
* Expiration، Manual Revocation
* Installation ID، Installation Limit، Unknown Installation Detection
* تعداد Clientهای LAN طبق پلن
* Local Clock Tamper Detection
* تمدید از تاریخ انقضای قبلی، نه از تاریخ خرید:

```text
Previous Expiry:   2027-01-01
Customer renews:   2027-04-01
New Effective Start: 2027-01-01
```

* **انتقال License** هنگام تعویض کامپیوتر یا نصب مجدد Windows (Self-service با محدودیت تعداد + دستی از Vendor Panel)

#### نحوه تحویل License به مشتری

License توسط **سرور مرکزی (Supabase)** ساخته و امضا می‌شود، نه توسط کامپیوتر فروشنده. فروشنده فقط از طریق **Vendor Admin Panel** (فاز 10.11) دستور می‌دهد.

```text
Trial (بدون کد):
  نصب → «شروع Trial ۳۰ روزه» → (یک بار اینترنت) → سرور Trial امضاشده برای این Installation ID صادر می‌کند

خرید / تبدیل Trial به Paid (بدون کد):
  مشتری پرداخت می‌کند → فروشنده در Vendor Panel کلینیک را پیدا و پلن/مدت را ثبت می‌کند
  → برنامه کلینیک در اولین اتصال اینترنت License جدید را خودکار دریافت می‌کند

نصب جدید / کامپیوتر جدید (با کد):
  فروشنده در Vendor Panel → «ساخت کد فعال‌سازی» → کد کوتاه مثل ARTA-7K2M-9QXP-4HTD
  → کد را حضوری، با پیامک یا WhatsApp به مشتری می‌دهد
  → مشتری کد را در صفحه Activation وارد می‌کند → سرور کد + Installation ID را بررسی و License امضاشده را برمی‌گرداند

تمدید:
  فروشنده در Vendor Panel تمدید را ثبت می‌کند → برنامه در Verification بعدی خودکار به‌روز می‌شود
```

* کد فعال‌سازی خودش License نیست؛ فقط «اجازه دریافت License» است. هر کد به یک کلینیک و حداکثر تعداد کامپیوترهای پلن آن محدود است، تاریخ انقضا دارد و از Vendor Panel قابل ابطال است.
* کد فعال‌سازی بدون حروف مبهم (O/0، I/1) تولید می‌شود تا تلفنی یا پیامکی راحت منتقل شود.
* **Activation آفلاین** (کلینیک بدون اینترنت): برنامه یک «کد درخواست» کوتاه یا QR نمایش می‌دهد → مشتری آن را برای فروشنده می‌فرستد → فروشنده در Vendor Panel «کد پاسخ» می‌سازد → مشتری کد پاسخ را وارد می‌کند. (روش اصلی Activation آنلاین است؛ این روش جایگزین است.)

### 10.9 Read-only State

پس از انقضا یا ابطال، برنامه فقط-خواندنی می‌شود، ولی همیشه این موارد **کار می‌کنند**:

* مشاهده تمام پرونده‌ها
* چاپ و Export داده
* Backup و Restore

داده پزشکی بیماران هیچ‌وقت در گرو License قرار نمی‌گیرد.

### 10.10 Security

* SQLCipher + DPAPI + Recovery Key (ADR-03, ADR-04)
* Encrypted Backup و Encrypted Attachments
* Argon2id Password Hashing، قفل حساب پس از تلاش‌های ناموفق
* Protected Application Configuration
* TLS برای License API و LAN
* **هیچ Secret یا Service-Role Key داخل EXE نیست**؛ EXE فقط Public Key و Publishable Key دارد
* امضای Update با کلید رایگان Tauri Updater (اجباری)
* Code Signing Certificate ویندوز (اختیاری؛ در صورت توجیه مالی بعداً)
* Secure Migration Process

### 10.11 Vendor Admin Panel

پنل وب داخلی برای فروشنده (Artaveo):

* مدیریت مشتریان/کلینیک‌ها
* صدور، تمدید، ابطال و انتقال License
* تعیین پلن و تعداد کامپیوتر
* مشاهده Installationها و Security Events
* زمان آخرین Backup موفق هر کلینیک (فقط تاریخ و وضعیت، برای پشتیبانی؛ بدون هیچ داده‌ای از محتوا)
* Export دوره‌ای دیتابیس License خود Artaveo (پلن رایگان Supabase Backup خودکار ندارد)
* انتشار نسخه جدید و Update Metadata

**Exit Criteria:** سناریوهای License (Trial، Activation، انقضا، Grace، ابطال، انتقال، دست‌کاری ساعت) و Backup/Restore (Local، USB، Cloud، کامپیوتر جدید) همه تست شده و پاس شده‌اند.

---

# PHASE 11 — Production Hardening, Installer, Update & Release

### 11.1 Installer

```text
Artaveo Dental Setup.exe
```

* Install، Uninstall، Repair، Upgrade
* نصب خودکار WebView2 در صورت نبود
* Data Preservation (Uninstall داده را پاک نمی‌کند مگر با تأیید صریح)
* Migration خودکار هنگام Upgrade
* انتشار از وب‌سایت خود Artaveo
* بدون Code Signing Certificate، ویندوز هنگام اولین اجرای Installer هشدار SmartScreen نشان می‌دهد (`More info → Run anyway`)؛ راهنمای تصویری این مرحله در وب‌سایت و راهنمای نصب قرار می‌گیرد. گواهی در آینده اختیاری است.

### 11.2 Automatic Updates

برنامه هنگام Online شدن نسخه فعلی را با آخرین نسخه مقایسه می‌کند. Update باید:

* Signed
* Versioned
* Safe (Backup خودکار قبل از Update)
* Rollback-aware

باشد. در حالت LAN، نسخه سرور و Clientها باید سازگار باشند؛ Client ناسازگار پیام Update می‌گیرد.

### 11.3 Crash & Recovery

* Database Integrity Check در Startup
* Recovery خودکار از آخرین وضعیت سالم
* ذخیره پیش‌نویس فرم‌های طولانی (Draft) برای جلوگیری از از دست رفتن ورودی
* Error Logs
* Safe Startup Mode

### 11.4 Support Tools

* «ارسال گزارش خطا» (Log + اطلاعات سیستم، **بدون داده بیمار**) با تأیید کاربر
* صفحه System Info (نسخه، حالت نصب، وضعیت دیتابیس، Backup، License)

### 11.5 Full QA

```text
Functional · Database · Migration · Security · Offline · LAN Multi-user
License · Backup · Restore · Printing (Thermal/A4) · PDF
RTL · LTR · Persian · Pashto · English · Shamsi · Gregorian
Light · Dark · Installer · Update · Rollback · Performance (NFR)
Windows 10 · Windows 11 · Low-end Hardware
x64 (Intel/AMD) · x86 32-bit · ARM64
```

### 11.6 Pilot در کلینیک واقعی

قبل از Release نهایی، اجرای سیستم در **حداقل یک کلینیک واقعی** (ترجیحاً یکی تک‌کامپیوتر و یکی چندکامپیوتر) برای حداقل ۲ هفته، با این Workflow کامل:

```text
New Patient → Appointment → Check-in → Examination → Dental Chart
→ Treatment Plan → Treatment → Prescription → Invoice → Payment
→ Receipt → Cash Closing → Backup → Close → Restore
```

بازخوردهای Pilot قبل از Release اعمال می‌شوند.

### 11.7 Documentation

* راهنمای کاربر (سه زبان)
* راهنمای نصب تک‌کامپیوتر و چندکامپیوتر
* راهنمای Backup/Restore و نگهداری Recovery Key

---

# Definition of Done — Artaveo Dental V1

```text
Platform
✅ Windows 10/11 Desktop Application
✅ No Browser Dependency
✅ Offline Daily Operation
✅ Single-PC Mode (Default)
✅ Solo-Doctor Mode (Simplified UI)
✅ Multi-PC LAN Mode (Zero-IT Setup)
✅ Encrypted Local SQLite
✅ Fast Indexed Database
✅ Multi-Clinic Distribution (One Codebase)

License & Security
✅ License System + Plans (PC Count)
✅ Trial System
✅ Installation Tracking
✅ Manual Revocation & License Transfer
✅ Automatic Online Verification + Grace Period
✅ Read-only Expiration State (Data always accessible)
✅ Recovery Key
✅ Audit Logs
✅ Vendor Admin Panel

Backup
✅ Daily Automated Backup
✅ Trusted USB Backup
✅ Encrypted Cloud Backup (Clinic-owned Google Drive / OneDrive)
✅ Backup Retention
✅ Validated Restore

Printing
✅ A4 Compact Receipt
✅ Thermal Receipt (58/80mm)
✅ Real-size PDF Receipt
✅ Full A4 Documents
✅ Prescription Printing
✅ Clinical Documents (Post-op, Referral, Imaging Request, Medical Certificate, Lab Order)

Localization & UI
✅ Persian (Default) · Pashto · English
✅ RTL/LTR
✅ Mixed-language Patient Data
✅ Shamsi & Gregorian Calendar
✅ Light & Dark Theme
✅ Glass Design System + Performance Mode
✅ Clinic Branding

Clinical
✅ Patient Management + Duplicate Detection
✅ Medical History + Medical Alerts
✅ Attachments & Image Viewer
✅ Dental Chart (FDI/Universal/Palmer)
✅ Periodontal Chart
✅ Clinical Notes + Templates
✅ Treatment Plans
✅ Prescriptions
✅ Consent Forms

Operations
✅ Appointments
✅ Doctor Scheduling
✅ Reception Queue
✅ Follow-up & Recall
✅ Staff & Granular Permissions

Finance
✅ Billing (Immutable Invoices)
✅ Payments / Installments / Refunds
✅ Debt Tracking
✅ Doctor Commission
✅ Clinic Expenses
✅ Daily Cash Closing

Management
✅ Inventory
✅ Suppliers
✅ Dental Lab
✅ Reports + Profit & Loss
✅ Dashboard
✅ Notifications
✅ Excel Import/Export

Release
✅ Signed Automatic Updates
✅ Crash Recovery
✅ Production Installer
✅ Real-clinic Pilot Passed
```

### چیزهایی که عمداً در V1 داخل هسته محصول نمی‌ریزیم

```text
Online Patient Portal
Online Booking
SMS Gateway
WhatsApp Integration
Multi-Branch Cloud Sync
Multi-Currency
Advanced Accounting
Insurance Integration
AI Features
Mobile App
```

از نظر معماری برای آینده قابل اضافه‌شدن هستند (UUIDv7، API Contract، Data Conventions)، اما نباید V1 را سنگین و پرهزینه کنند.

**اصل V1 این است: کامل برای عملیات داخل کلینیک، ولی نه شلوغ با سرویس‌های جانبی غیرضروری.**

---

# جدول ریسک‌ها

| ریسک | احتمال | اثر | راهکار |
|---|---|---|---|
| مشکل در رندر PDF/چاپ RTL | متوسط | بالا | Spike فاز ۰ قبل از هر کار دیگر |
| ناسازگاری پرینترهای حرارتی ارزان | بالا | متوسط | پشتیبانی دو روش (Silent Print و ESC/POS) + تست روی چند مدل رایج بازار |
| مشکلات شبکه در کلینیک (Wi-Fi ضعیف) | متوسط | متوسط | اتصال مجدد خودکار، پیام واضح، توصیه کابل LAN برای سرور |
| گم شدن Recovery Key | متوسط | بالا | تأکید در Setup، چاپ اجباری، یادآوری دوره‌ای به Owner |
| خرابی هارد کامپیوتر سرور | متوسط | بالا | USB + Cloud Backup + هشدار Backup ناموفق |
| کامپیوترهای ضعیف | بالا | متوسط | حالت کارایی، NFR تست روی سخت‌افزار ضعیف |
| اینترنت کند برای Cloud Backup | بالا | پایین | آپلود توسط Google Drive/OneDrive (قابل ادامه)، تصاویر اختیاری |
| هشدار SmartScreen / آنتی‌ویروس برای Installer بدون امضا | بالا | پایین | راهنمای نصب، نصب حضوری اولیه، گزارش False Positive به آنتی‌ویروس‌ها؛ گواهی امضا در آینده اختیاری |
| از دست رفتن دیتابیس License مرکزی (پلن رایگان Supabase Backup خودکار ندارد) | پایین | بالا | Export دوره‌ای از Vendor Panel؛ License محلی امضاشده کلینیک‌ها مستقل از سرور کار می‌کند |
| غیرفعال شدن پروژه رایگان Supabase به‌علت عدم فعالیت | پایین | پایین | درخواست‌های Verification منظم پروژه را فعال نگه می‌دارد؛ Grace Period آفلاین ۳۰ روزه |

---

# معماری داده — اصل مهم نهایی

برای اطلاعات مرجع:

```text
patient.gender_id   = <id>
patient.province_id = <id>
patient.district_id = <id>
```

نه:

```text
gender   = "Male"
province = "Kabul"
district = "Bagrami"
```

و برای نمایش:

```text
reference ID → translation table → Current UI Language
```

یک ID می‌تواند در فارسی «کابل»، در انگلیسی «Kabul» و در پشتو «کابل» نمایش داده شود، بدون اینکه رکورد بیمار برای هر زبان دوباره ذخیره شود.

برای داده‌های آزاد بیمار مثل نام، آدرس یا توضیحات، متن اصلی همان چیزی است که کاربر وارد کرده و به اجبار ترجمه یا Normalize نمی‌شود.

---

# اصول معماری که نباید در هیچ فاز نقض شوند

**یک Codebase برای همه کلینیک‌ها.**

**Configuration به‌جای Fork.**

**Local Data برای عملیات.**

**Cloud فقط برای سرویس‌های مرکزی و قابلیت‌های اختیاری.**

**Domain نباید Dependency هسته برنامه باشد.**

**روزهای بدون اینترنت باید روز کاری عادی باشند.**

**تمام Business Logic در Rust Core؛ UI هیچ‌وقت مستقیم به دیتابیس دسترسی ندارد.**

**تک‌کامپیوتر و چندکامپیوتر از یک API Contract استفاده می‌کنند.**

**هیچ Feature جدیدی نباید Security، Backup، License یا Data Integrity را دور بزند.**

**هیچ تغییر UI نباید باعث وابستگی Business Logic به Presentation Layer شود.**

**Database Schema باید Migration-Based و Versioned باشد.**

**Clinic Data، License Data و Control Data باید از هم جدا باشند.**

**تمام عملیات حساس Audit شوند.**

**اسناد مالی صادرشده تغییر نمی‌کنند؛ فقط Void یا اصلاح می‌شوند.**

**داده بیمار هیچ‌وقت گروگان License نمی‌شود.**

**هر چیزی که امروز برای یک کلینیک ساخته می‌شود، باید از نظر معماری قابلیت استفاده توسط کلینیک دوم را داشته باشد.**
