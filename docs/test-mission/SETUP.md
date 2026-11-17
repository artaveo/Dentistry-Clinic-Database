# SETUP — راه‌اندازی محیط تست

## واقعیت‌های کشف‌شده
* **هر پوشه‌ی داده = یک کلینیک.** اپ چندکلینیکی نیست. `ARTAVEO_DATA_DIR` (یا `--data-dir` در dev server) پوشه را تعیین می‌کند؛ دیتابیس `clinic.db` (SQLCipher) داخل آن است.
* نصب واقعی روی این لپ‌تاپ: `C:\ProgramData\ArtaveoDental` (و `ArtaveoDental-old`). **کپی پشتیبان:** `E:\Projects\Artaveo-TestData\_original-backup\`. این پوشه‌ها را دست نزنید.
* Core واقعی بدون Tauri: `artaveo-dev-server` (HTTP، همان router) + UI با Vite. UI آدرس Core را از `VITE_RPC_URL` می‌گیرد.
* حالت mock (`VITE_MOCK=1`) فقط برای اسکرین‌شات است؛ **برای تست منطق استفاده نشود** (Core واقعی نیست).

## Binary مشترک
ساخت یک‌بار توسط مادر: `E:\Projects\Artaveo-TestData\bin\artaveo-dev-server.exe` (کپی از `target\debug`). سشن‌ها **cargo build را موازی اجرا نکنند**؛ از همین کپی استفاده کنند.

## بالا آوردن یک کلینیک (مثال)
```powershell
$env:ARTAVEO_ENV="test"
E:\Projects\Artaveo-TestData\bin\artaveo-dev-server.exe --data-dir E:\Projects\Artaveo-TestData\<area>\clinicA --port 88X1
# UI (در پوشه‌ی ui):
$env:VITE_RPC_URL="http://127.0.0.1:88X1/rpc"; npx vite --port 58X1 --strictPort
```
## پورت‌ها و پوشه‌ها
| سشن | پوشه‌ی داده | dev-server | vite |
|---|---|---|---|
| patients-appts | `...\patients-appts\clinicA|B` | 18811, 18812 | 5811, 5812 |
| chart-treatment | `...\chart-treatment\clinicA|B` | 18821, 18822 | 5821, 5822 |
| finance-reports | `...\finance-reports\clinicA|B` | 18831, 18832 | 5831, 5832 |
| admin-stock-roles | `...\admin-stock-roles\clinicA|B` | 18841, 18842 | 5841, 5842 |
| backup-reinstall | `...\backup-reinstall\clinicA|B` | 18851, 18852 | 5851, 5852 |
| ui-polish | `...\ui-polish\clinicA|B` | 18861, 18862 | 5861, 5862 |

> **تغییر پورت:** پورت‌های 8761–8860 در ویندوز رزرو است (خطای 10013). dev-server باید روی **18xxx** باشد (مثلاً 18821)؛ vite همان 58xx.

## قواعد
* هیچ دیتابیس/export/پشتیبان تستی وارد git نشود. فقط `docs/test-mission/*.md` و اسکرین‌شات‌های کوچک لازم commit شوند (مادر commit می‌کند).
* داده‌ی بیمار کاملاً ساختگی.
* فقط پیدا کردن باگ؛ **کد منبع را ویرایش نکنید** (رفع‌ها مرحله‌ی بعد).

## ساخت محلی

نتیجه: `artaveo-dev-server.exe`، `artaveo-seed.exe` و `artaveo-dump.exe` روی همین لپ‌تاپ ARM64 ساخته شدند (target: `aarch64-pc-windows-gnullvm`) و در `E:\Projects\Artaveo-TestData\bin\` هستند. بررسی: اجرای dev-server با `--data-dir E:\Projects\Artaveo-TestData\_envcheck --port 8899` و `curl http://127.0.0.1:8899/health` جواب `200 ok` داد.

علت شکست قبلی: `openssl-src` برای `aarch64-pc-windows-gnullvm` هدف نمی‌شناخت. Strawberry Perl دانلود نشد، چون سرعت اینترنت حدود ۱۷ کیلوبایت بر ثانیه است (فایل ۳۰۴ مگابایت). راه‌حل بدون دانلود:

1. یک کپی از `openssl-src-300.6.1+3.6.3` در `E:\Projects\Artaveo-TestData\tools\openssl-src-patched` با یک خط اضافه در `src/lib.rs`: `"aarch64-pc-windows-gnullvm" => "mingwarm64",` (هدف `mingwarm64` در خود OpenSSL موجود است).
2. perl گیت (`C:\Program Files\Git\usr\bin\perl`) دو ماژول ندارد: `IPC::Cmd` و `Pod::Usage`. دو فایل ساختگی کوچک ساخته شد: `tools\perl-stubs\IPC\Cmd.pm` و `tools\perl-stubs\Pod\Usage.pm` (و کپی `IPC\Cmd.pm` داخل `openssl-src-patched\openssl\util\perl\IPC\`).
3. `make` وجود نداشت: `mingw32-make.exe` از بسته llvm-mingw (WinGet) به `tools\bin\make.exe` کپی شد.

چیزی دانلود یا نصب نشد (فقط کرِیت‌های cargo). کد repo تغییر نکرد؛ `Cargo.lock` به حالت قبل از ساخت برگردانده شد.

دستور نهایی (Git Bash):

```bash
T=/e/Projects/Artaveo-TestData
export PATH="$PATH:$T/tools/bin"
export MSYS2_ENV_CONV_EXCL=PERL5LIB PERL5LIB=$T/tools/perl-stubs
export CARGO_TARGET_DIR=$T/cargo-target OPENSSL_SRC_PERL=/usr/bin/perl
cd "/e/Projects/Dentistry Clinic Database"
cargo build -p artaveo-core --features dev-server --bins --release \
  --config 'patch.crates-io.openssl-src.path="E:/Projects/Artaveo-TestData/tools/openssl-src-patched"'
```

نکته: ساخت کامل اول حدود ۳۴ دقیقه طول کشید؛ ساخت‌های بعدی چند ثانیه. بعد از ساخت، `git checkout Cargo.lock` را نزنید (قبل از ما هم تغییر داشت)؛ نسخه قبلی در `E:\Projects\Artaveo-TestData\Cargo.lock.before` است.

اجرا: `E:\Projects\Artaveo-TestData\bin\artaveo-dev-server.exe --data-dir <پوشه> --port <پورت>`

UI: در پوشه `ui` دستور `npx tsc --noEmit` بدون خطا تمام شد و `npx vite --version` نسخه vite/8.3.2 (win32-arm64، node v24.19.0) را نشان داد.
