# Artaveo Dental

نرم‌افزار دسکتاپ ویندوز برای مدیریت کامل کلینیک دندان‌پزشکی — آفلاین، دیتابیس محلی رمزشده، تک‌کامپیوتر یا چندکامپیوتر در شبکه داخلی، چندزبانه (دری، پشتو، انگلیسی).

* نقشه راه کامل: [Artaveo_Dental_Roadmap.md](Artaveo_Dental_Roadmap.md)
* **فاز ۰ (Technical Spikes):** ✅ انجام شد — [گزارش‌ها](docs/spikes/README.md) · [ADRها](docs/adr/README.md) · [جدول ریسک](docs/risk-register.md) · [چک‌لیست سخت‌افزار](docs/spikes/hardware-checklist.md)

## ساختار مخزن

```text
.
├── Artaveo_Dental_Roadmap.md   نقشه راه و تصمیم‌های پایه
├── core/                       Rust Core — Business Logic، دیتابیس، Audit (فاز ۱)
├── app/                        Tauri Shell و Installer (فاز ۱)
├── ui/                         React + TypeScript (فاز ۱–۲)
├── shared/                     API Contract و Typeهای مشترک (فاز ۱)
├── spikes/                     فاز ۰ — نمونه‌های اثبات مفهوم (Cargo workspace)
│   ├── key-protect/            DPAPI + Recovery Key
│   ├── sqlcipher-core/         SQLCipher، Migration، Backup، Restore، Benchmark
│   ├── shamsi/                 تقویم شمسی افغانی + ارقام
│   ├── escpos/                 چاپ حرارتی ESC/POS Raster + Spooler ویندوز
│   ├── usb-identity/           هویت فلش/هارد USB
│   ├── lan-mode/               mDNS + Pairing (SPAKE2) + TLS Pinning + WebSocket
│   ├── tauri-shell/            Tauri 2: اتصال همه Spikeها، WebView2، چاپ بی‌صدا، قالب‌های رسید/N-up، Installer
│   └── print-lab/              Node + Chromium/Edge: PDFهای RTL، رستر، مقایسه تقویم
├── docs/
│   ├── spikes/                 گزارش هر Spike
│   ├── adr/                    ADRهای به‌روز/جدید
│   └── risk-register.md
└── .github/workflows/          CI روی Linux و Windows
```

## اجرای Spikeها

پیش‌نیاز: Rust (stable)، Node 22، و روی Linux یک Chrome/Chromium (`CHROME_PATH`). روی Windows: Perl (برای OpenSSL) و Microsoft Edge.

```bash
cd spikes
cargo test                                            # تست‌های Rust
cargo run --release -p sqlcipher-core --example bench # کارایی با ۱۰۰٬۰۰۰ بیمار
cd print-lab && npm ci && npm run all                 # PDF/رستر/تقویم + اعتبارسنجی

# Windows
cd spikes/tauri-shell && npx @tauri-apps/cli@^2 dev   # برنامه نمونه
npx @tauri-apps/cli@^2 build                          # Installer آنلاین
npx @tauri-apps/cli@^2 build --config tauri.offline.conf.json   # Installer آفلاین
```
