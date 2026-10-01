# ADR-23 — محل داده و Transportهای Core (فاز ۱)

**وضعیت:** پذیرفته‌شده

## تصمیم
* **محل داده (Production، ویندوز):** `%ProgramData%\ArtaveoDental` — مشترک بین همه حساب‌های ویندوز و سرویس حالت سرور (فاز ۷)، هم‌راستا با DPAPI در Machine Scope (ADR-04). Installer به گروه Users دسترسی Modify می‌دهد. Uninstall این پوشه را پاک نمی‌کند (Data Preservation).
  محتوا: `clinic.db` (+ WAL)، `clinic.key` (DPAPI)، `clinic.rkey` (Recovery-wrapped)، `backups/`، `logs/`.
* **محیط‌ها:** `ARTAVEO_ENV=development|test|production` (پیش‌فرض: debug=development، release=production)؛ `ARTAVEO_DATA_DIR` برای تست/توسعه.
* **Transportها (ADR-02):** یک روتر `Core::handle(RpcRequest) -> RpcResponse`:
  * Tauri IPC: فرمان واحد `rpc` در `app/` (هیچ منطق دیگری در پوسته نیست).
  * HTTP توسعه: `artaveo-dev-server` (feature `dev-server`، فقط 127.0.0.1، هرگز در Release) — برای توسعه UI در مرورگر و تست E2E Playwright روی Core واقعی.
  * LAN (فاز ۷) از همین Envelope استفاده می‌کند.
* **Contract:** `shared/src/lib.rs` منبع واحد؛ `shared/ts/contract.ts` به‌صورت خودکار تولید و در CI بررسی می‌شود.
* **Permissionها در کد**، Roleها مجموعه‌ای از Permission؛ در هر Startup با دیتابیس همگام می‌شوند.
* **ID داده‌های Seed** (ولایات، داده مرجع، Roleهای سیستمی) قطعی و یکسان در همه کلینیک‌ها (`ids::seed_id`).
