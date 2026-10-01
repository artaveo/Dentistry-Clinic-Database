# Spike 7 — WebView2 Runtime

**سؤال:** نصب خودکار WebView2 در Installer برای سیستم‌هایی که ندارند.
**نتیجه:** ✅ **تصمیم:** ADR-20.

## گزینه‌های Tauri 2 (`bundle.windows.webviewInstallMode`)

| حالت | حجم اضافه | اینترنت لازم؟ | مناسب برای |
|---|---|---|---|
| `downloadBootstrapper` (پیش‌فرض Tauri) | ۰ | بله | — |
| `embedBootstrapper` | حدود 1.8MB | فقط اگر WebView2 نصب نباشد | **Installer اصلی** |
| `offlineInstaller` | حدود 130MB | خیر | **Installer آفلاین** برای کلینیک‌های بدون اینترنت |
| `fixedRuntime` | حدود 180MB | خیر | رد شد: آپدیت امنیتی Chromium به عهده ما می‌افتد |

* Windows 11 و Windows 10 به‌روز (از ۲۰۲۱) WebView2 Evergreen را از قبل دارند؛ مشکل اصلی Windows 10 قدیمی/بدون آپدیت در کلینیک‌های آفلاین است ← نسخه آفلاین.
* پیکربندی: `spikes/tauri-shell/tauri.conf.json` (آنلاین) و `tauri.offline.conf.json` (با `--config` روی آن Merge می‌شود). CI هر دو را می‌سازد.
* `installMode: perMachine` — برای قانون فایروال و Service حالت سرور لازم است (نیاز به UAC یک‌باره هنگام نصب).
* حالت `silent: true` ← نصب WebView2 بدون پنجره اضافه.

## یافته‌ها

1. **زبان Installer:** NSIS در Tauri فارسی (`Persian`) دارد ولی **پشتو ندارد** ← در فاز ۱۰ فایل زبان سفارشی با `customLanguageFiles` اضافه شود.
2. WebView2 همان موتوری است که PDF/چاپ را انجام می‌دهد (ADR-13) ← در CI ویندوز تولید PDF با **Edge** انجام می‌شود تا رفتار موتور واقعی تست شود.
3. نسخه حداقل WebView2 برای APIهای چاپ: `PrintToPdf` (ICoreWebView2_7) و `Print`/`PrinterName` (ICoreWebView2_16، Runtime 1.0.1518+) — Evergreen همیشه جدیدتر است؛ Installer آفلاین نسخه جدید را همراه دارد.

## باقی‌مانده

نصب روی یک **Windows 10 تمیز بدون اینترنت** (VM) با Installer آفلاین — [چک‌لیست H2](hardware-checklist.md).
