# ADR-03 — دیتابیس: SQLite + SQLCipher (به‌روزرسانی فاز ۰)

**وضعیت:** پذیرفته‌شده · **شواهد:** [Spike 1](../spikes/01-tauri-sqlcipher-dpapi.md)

## زمینه
تصمیم اولیه: SQLCipher + WAL + Foreign Keys + STRICT. Spike نشان داد چه نسخه‌ای، چگونه Build و چه تنظیماتی لازم است.

## تصمیم
* `rusqlite` با `bundled-sqlcipher-vendored-openssl` (در حال حاضر SQLCipher 4.5.7 / SQLite 3.45.3)؛ OpenSSL ایستا لینک می‌شود.
* کلید: **۲۵۶ بیت تصادفی خام** (`PRAGMA key = "x'…'"`) — بدون PBKDF2.
* PRAGMAها در هر اتصال: `journal_mode=WAL`، `foreign_keys=ON`، `synchronous=FULL`، `busy_timeout=5000`.
* Migration: یک Transaction `IMMEDIATE` برای هر Migration، جدول `schema_migrations` با Checksum SHA-256، `user_version`؛ برنامه قدیمی‌تر دیتابیس جدیدتر را باز نمی‌کند.
* Backup قبل از Migration و Backup آنلاین با `VACUUM INTO` (خروجی رمزشده با همان کلید).
* **Startup:** فقط اعتبار کلید + نسخه Schema. `cipher_integrity_check` و `quick_check` **در پس‌زمینه** بعد از Login و روی هر Backup.
* جستجوی پیشوندی روی ستون‌های Index‌شده به‌صورت بازه (`>= ? AND < ?`) نوشته شود، نه `LIKE`.

## پیامدها
* Build روی Windows به Perl نیاز دارد (برای OpenSSL؛ روی GitHub Runner موجود است).
* Integrity Check کامل حدود 13ms/MB است؛ اجرای آن در Startup با NFR «۳ ثانیه تا Login» سازگار نیست.
