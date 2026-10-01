# PHASE 0 — گزارش Technical Spikes

> وضعیت: **کامل از نظر نرم‌افزاری** — همه ۹ Spike پیاده‌سازی، تست خودکار و تصمیم‌گیری شده‌اند.
> مواردی که فقط با **سخت‌افزار فیزیکی** قابل تأیید نهایی هستند (پرینتر حرارتی واقعی، فلش USB واقعی، Router کلینیک)
> با چک‌لیست دقیق در [hardware-checklist.md](hardware-checklist.md) آمده‌اند و در [جدول ریسک](../risk-register.md) با وضعیت «شناخته‌شده + راهکار» ثبت شده‌اند؛ هیچ ریسک «نامعلوم» باقی نمانده است.

## خلاصه

| # | Spike | نتیجه | تصمیم نهایی | کد | گزارش |
|---|---|---|---|---|---|
| 1 | Tauri 2 + Rust Core + SQLCipher + DPAPI | ✅ موفق | SQLCipher 4.5.7 (rusqlite، OpenSSL ایستا)، کلید خام ۲۵۶ بیتی، DPAPI در Machine Scope + Recovery Key | `spikes/sqlcipher-core`, `spikes/key-protect`, `spikes/tauri-shell` | [01](01-tauri-sqlcipher-dpapi.md) |
| 2 | RTL PDF | ✅ موفق | قالب HTML مشترک + WebView2 `PrintToPdf`؛ فونت Vazirmatn + Noto Sans Arabic (فقط برای ؋) | `spikes/print-lab`, `spikes/tauri-shell/dist` | [02](02-rtl-pdf.md) |
| 3 | Thermal Printing | ✅ موفق | **ESC/POS Raster** (اصلی) + WebView2 Silent Print (پشتیبان) | `spikes/escpos` | [03](03-thermal-printing.md) |
| 4 | A4 Compact Print | ✅ موفق | موقعیت‌دهی مطلق CSS به میلی‌متر + Silent Print با Page Size | `spikes/tauri-shell/dist/receipt.html` | [04](04-a4-compact.md) |
| 4b | Small Paper on Normal Printer | ✅ موفق | A5/A6 با Page Size واقعی + تشخیص اندازه/سینی دستی از درایور؛ N-up (۲ یا ۴ رسید) روی A4 با خط برش، بدون کوچک‌نمایی | `spikes/tauri-shell/dist/sheet.html`, `spikes/escpos/src/paper.rs` | [09](09-small-paper.md) |
| 5 | Shamsi Calendar | ✅ موفق | Core: ماژول Rust خودمان؛ UI: `@internationalized/date` (React Aria)؛ نام ماه‌ها از جدول خودمان | `spikes/shamsi` | [05](05-shamsi-calendar.md) |
| 6 | LAN Mode | ✅ موفق | mDNS + Pairing با **SPAKE2** + TLS Pinning + WebSocket (rustls/ring) | `spikes/lan-mode` | [06](06-lan-mode.md) |
| 7 | WebView2 | ✅ موفق | Installer عادی `embedBootstrapper` + نسخه آفلاین `offlineInstaller` | `spikes/tauri-shell/tauri*.json` | [07](07-webview2.md) |
| 8 | USB Identity | ✅ موفق | Marker امضاشده (HMAC) روی فلش + Serial سخت‌افزاری (در صورت معتبر بودن) | `spikes/usb-identity` | [08](08-usb-identity.md) |

## شواهد قابل تکرار

```bash
cd spikes
cargo test                                   # 33 تست Rust (Linux/Windows)
cargo test -p lan-mode -- --ignored          # کشف mDNS روی شبکه واقعی
cargo run --release -p sqlcipher-core --example bench
cd print-lab && npm ci && npm run all        # تولید و اعتبارسنجی 12 PDF + رستر حرارتی + مقایسه تقویم
```

CI (`.github/workflows/phase0-spikes.yml`) همین‌ها را روی **Linux** و **Windows** اجرا می‌کند؛ در Windows علاوه بر آن DPAPI واقعی، لیست پرینترهای Spooler و اندازه‌کاغذ/سینی‌های گزارش‌شده توسط درایورها، API های USB و ساخت هر دو Installer (آنلاین/آفلاین WebView2) انجام و خروجی‌ها به‌صورت Artifact ذخیره می‌شوند.

## اعداد اندازه‌گیری‌شده

| معیار (NFR) | هدف | نتیجه Spike |
|---|---|---|
| جستجوی بیمار با ۱۰۰٬۰۰۰ رکورد (دیتابیس رمز‌شده) | < 200ms | **0.15ms** میانگین (جستجوی پیشوند تلفن با Index) |
| درج ۱۰۰٬۰۰۰ بیمار (یک Transaction) | — | 0.96s |
| Backup رمزشده (`VACUUM INTO`) | < 2min برای 1GB | 29MB در 0.43s ← حدود 15s برای 1GB |
| RTT درخواست LAN (TLS + WebSocket) | < 300ms | 21µs روی loopback (سربار پروتکل ناچیز است؛ تأخیر واقعی = شبکه) |
| `cipher_integrity_check` + `quick_check` | — | حدود 13ms/MB ← **برای 1GB حدود 13s** → در Startup اجرا نمی‌شود (ADR-03) |

## فرض چاپ (رودمپ ۶.۹)

بسیاری از کلینیک‌ها پرینتر حرارتی ندارند یا اصلاً پرینتر ندارند. همه Spikeهای چاپ با این فرض انجام شدند: **اندازه سند همیشه کوچک و استاندارد است و PDF همیشه اندازه واقعی رسید را دارد (نه A4)**. پنج Output Profile همگی پوشش داده و تست شدند:

| Output Profile | Spike | خروجی تست‌شده |
|---|---|---|
| Thermal 58/80mm | 3 | PDF رول با ارتفاع محتوا + ESC/POS Raster |
| A4 Compact (رسید در اندازه واقعی روی بخشی از A4، پیش‌فرض نیمه بالا) | 4 | `receipt-a4-compact-*.pdf` |
| A5/A6 روی پرینتر معمولی (Manual Feed) | 4b | `receipt-a5-fa.pdf`, `receipt-a6-*.pdf` |
| N-up با خط برش (۲ یا ۴ روی A4) | 4b | `sheet-2up-fa.pdf`, `sheet-4up-ps.pdf` |
| فقط PDF | 2 | همه PDFها در اندازه واقعی سند |

## تغییرات ADR

ADR-03، ADR-04 و ADR-13 به‌روزرسانی شدند و ADR-18 تا ADR-22 اضافه شدند. جزئیات: [docs/adr](../adr/README.md).
