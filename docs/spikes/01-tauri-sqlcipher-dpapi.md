# Spike 1 — Tauri 2 + Rust Core + SQLCipher + DPAPI

**سؤال:** Build، رمزنگاری، Migration و DPAPI روی Windows 10/11 کار می‌کند؟
**نتیجه:** ✅ بله. **تصمیم:** ADR-03 و ADR-04 (به‌روزشده).

## چه ساخته شد

| بخش | محل |
|---|---|
| مدیریت کلید: DPAPI (Windows)، Recovery Key، Wrap/Unwrap | `spikes/key-protect` |
| دیتابیس رمزشده، Migration Engine، Backup، Integrity، Restore | `spikes/sqlcipher-core` |
| پوسته Tauri 2 که همان Core را صدا می‌زند (`db_selftest`) | `spikes/tauri-shell` |
| Benchmark با ۱۰۰٬۰۰۰ بیمار | `spikes/sqlcipher-core/examples/bench.rs` |

## نسخه‌ها

* Tauri **2.12.1** (wry 0.57، webview2-com 0.39)
* rusqlite 0.32 با `bundled-sqlcipher-vendored-openssl` → **SQLCipher 4.5.7** روی **SQLite 3.45.3** (STRICT از 3.37 پشتیبانی می‌شود)
* OpenSSL 3.x به‌صورت **ایستا** داخل exe لینک می‌شود ← هیچ DLL اضافه‌ای لازم نیست.

## تست‌های خودکار (`cargo test -p sqlcipher-core -p key-protect`)

| تست | چه چیزی را ثابت می‌کند |
|---|---|
| `creates_encrypted_db_with_expected_pragmas` | نسخه SQLCipher 4، `journal_mode=wal`، `foreign_keys=1`؛ هدر فایل `SQLite format 3` **نیست** و داده بیمار به‌صورت متن ساده در فایل دیده نمی‌شود |
| `wrong_key_is_rejected` | کلید اشتباه ← خطای `WrongKey` (نه داده خراب) |
| `strict_and_foreign_keys_are_enforced` | جدول STRICT متن را در ستون پولی INTEGER رد می‌کند (ADR-06)؛ FK رکورد یتیم را رد می‌کند |
| `uuid_v7_ids_sort_by_creation_time` | UUIDv7 قابل مرتب‌سازی زمانی (ADR-05) |
| `reopen_with_os_protector_runs_integrity_checks` | باز کردن مجدد با کلید DPAPI + Integrity Check |
| `restore_on_new_pc_with_recovery_key` | **سناریوی کامپیوتر جدید:** فایل کلید DPAPI حذف می‌شود، با Recovery Key چاپی (حتی با حروف کوچک) بازیابی و کلید برای کامپیوتر جدید دوباره با DPAPI محافظت می‌شود |
| `vacuum_into_backup_stays_encrypted_and_restorable` | Backup با `VACUUM INTO` رمزشده (با همان کلید) و قابل Restore است، در حین کار با WAL |
| `migration_backup_drift_and_rollback` | Backup خودکار **قبل از** Migration؛ Migration خراب کاملاً Rollback می‌شود؛ ویرایش Migration اعمال‌شده تشخیص داده می‌شود (Checksum)؛ برنامه قدیمی دیتابیس جدیدتر را باز نمی‌کند |
| `recovery::*` | فرمت Recovery Key، تشخیص غلط تایپی با Checksum، Wrap/Unwrap |
| `protector::protect_roundtrip` | روی Windows: **DPAPI واقعی** (`CryptProtectData`/`CryptUnprotectData`) |

## یافته‌ها

1. **کلید خام به‌جای Passphrase.** کلید داده ۲۵۶ بیتی تصادفی است، پس PBKDF2 داخلی SQLCipher (۲۵۶ هزار دور) فقط Startup را کند می‌کند. با `PRAGMA key = "x'…'"` باز شدن دیتابیس فوری است.
2. **Integrity Check کامل گران است.** حدود 13ms به‌ازای هر MB. برای دیتابیس 1GB حدود 13 ثانیه ← با NFR «کمتر از ۳ ثانیه تا Login» در تضاد است. تصمیم: در Startup فقط اعتبار کلید + نسخه Schema بررسی شود؛ `cipher_integrity_check` و `quick_check` در پس‌زمینه بعد از Login و روی هر نسخه Backup اجرا شوند (ADR-03).
3. **`VACUUM INTO`** بهترین روش Backup آنلاین برای SQLCipher است: خروجی با همان کلید رمز می‌شود، فشرده (بدون صفحات خالی) است و Writerها را قفل نمی‌کند.
4. **DPAPI Machine Scope** انتخاب شد (نه User Scope) چون سرور LAN در فاز ۷ به‌صورت Windows Service اجرا می‌شود و کارمندان با حساب‌های ویندوزی مختلف از یک کامپیوتر استفاده می‌کنند. یک Entropy مخصوص برنامه اضافه شده تا Blob با برنامه دیگری قاطی نشود.
5. **محدودیت امنیتی (شفاف):** DPAPI جلوی کپی فایل دیتابیس/Backup به کامپیوتر دیگر را می‌گیرد، ولی اگر **کل هارد** دزدیده شود، مهاجم حرفه‌ای می‌تواند کلید Machine DPAPI را از Registry آفلاین استخراج کند. راهکار: توصیه/تشخیص BitLocker در Setup Wizard و پیام هشدار اگر فعال نیست (به جدول ریسک اضافه شد).
6. **Migration:** هر Migration در Transaction جدا با `BEGIN IMMEDIATE`؛ `schema_migrations` با Checksum SHA-256 + `PRAGMA user_version`. این طراحی مستقیماً به فاز ۱.۳ منتقل می‌شود.

## Benchmark (`cargo run --release -p sqlcipher-core --example bench`)

```text
insert 100000 patients (1 tx): 956ms
100 phone-prefix searches: 14.7ms total, 147µs avg
lookup by patient number: 114µs
VACUUM INTO encrypted backup (29 MB): 423ms
PRAGMA cipher_integrity_check: 175ms
PRAGMA quick_check: 209ms
```

نکته: جستجوی پیشوند شماره تلفن باید به‌صورت **بازه** (`phone >= ? AND phone < ?`) نوشته شود؛ `LIKE 'x%'` با Index پیش‌فرض (BINARY) استفاده نمی‌کند.

## اجرای روی Windows

* CI روی `windows-latest` همه تست‌ها را با DPAPI واقعی اجرا و Installer را می‌سازد.
* تست دستی روی Windows 10 22H2 و Windows 11: Installer را از Artifact نصب کنید → دکمه «Run database self-test» → باید `WindowsDpapiMachine`، نسخه 4.5.7 و `reopened: true` نمایش داده شود (چک‌لیست سخت‌افزار، بند H1).
