# Artaveo Dental

نرم‌افزار دسکتاپ ویندوز برای مدیریت کامل کلینیک دندان‌پزشکی — آفلاین، دیتابیس محلی رمزشده، تک‌کامپیوتر یا چندکامپیوتر در شبکه داخلی، چندزبانه (دری، پشتو، انگلیسی).

* نقشه راه: [Artaveo_Dental_Roadmap.md](Artaveo_Dental_Roadmap.md)
* فاز ۰ (Technical Spikes): ✅ — [گزارش‌ها](docs/spikes/README.md) · [ADRها](docs/adr/README.md) · [جدول ریسک](docs/risk-register.md)
* فاز ۱ (Engineering Foundation): [راهنمای تست دستی](docs/testing/phase-1-manual-test.md) · Installerها در GitHub Releases

## ساختار

```text
core/      Rust Core — تمام منطق، دیتابیس SQLCipher، Migration، Auth (Argon2id)، Permission، Audit، Backup
  migrations/   SQL نسخه‌دار (هر Migration در Transaction، Backup قبل از اجرا)
  seeds/        داده مرجع و جغرافیا (CSV، سه زبان)
  src/bin/      artaveo-dev-server (HTTP توسعه/E2E) · artaveo-seed (دیتابیس آزمایشی بزرگ)
shared/    API Contract (Rust) → shared/ts/contract.ts (تولید خودکار)
app/       پوسته Tauri 2 — فقط فرمان `rpc` + Installer (NSIS، WebView2 آنلاین/آفلاین)
ui/        React + TypeScript + Vite — بدون دسترسی مستقیم به دیتابیس
  e2e/          تست Playwright روی Core واقعی
spikes/    نمونه‌های فاز ۰ (workspace جدا)
docs/      ADR، گزارش Spikeها، راهنمای تست، یادداشت انتشار
```

## توسعه

پیش‌نیاز: Rust stable، Node 22. روی ویندوز: Strawberry Perl (برای OpenSSL ایستا) و WebView2.

```bash
cargo test -p artaveo-core -p artaveo-shared          # تست‌های Core و Contract (+ تولید contract.ts)

# UI در مرورگر روی Core واقعی:
cargo run -p artaveo-core --features dev-server --bin artaveo-dev-server -- --data-dir .artaveo-development
cd ui && npm ci && npm run dev                          # http://localhost:5173

# UI بدون Rust (کامپیوتری که Core را نمی‌سازد): Core شبیه‌سازی‌شده در مرورگر، فقط برای کار UI و اسکرین‌شات
cd ui && VITE_MOCK=1 npx vite --port 5199                # ?mock=seeded = کلینیک آماده با داده نمونه
node scripts/design-screens.mjs http://127.0.0.1:5199 ../docs/design/screenshots/<version>   # اسکرین‌شات طراحی
node scripts/guide-screens.mjs http://127.0.0.1:5199     # عکس‌های راهنمای تست دستی

cd ui && npx playwright test                            # E2E (dev server را خودش اجرا می‌کند؛ قبلش cargo build --features dev-server)
cargo run --release -p artaveo-core --bin artaveo-seed -- --data-dir /tmp/big --audit 100000

# برنامه دسکتاپ (ویندوز):
cd ui && npm run build && cd ../app && ../ui/node_modules/.bin/tauri build --target x86_64-pc-windows-msvc
```

نسخه (SemVer) فقط در `Cargo.toml` (workspace) و `ui/package.json` است؛ `scripts/check-version.mjs` یکسان بودنشان را بررسی می‌کند. انتشار: Tag `vX.Y.Z` → CI سه معماری را می‌سازد، نصب و اجرا را تست می‌کند و GitHub Release می‌سازد.
