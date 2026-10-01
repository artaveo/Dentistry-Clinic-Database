# core — Rust Core (فاز ۱)

تمام Business Logic، Auth، Permission، Audit و دسترسی به دیتابیس (ADR-01). از Spikeهای فاز ۰ به این‌جا منتقل می‌شود:

* `spikes/sqlcipher-core` ← لایه داده، Migration Engine، Backup
* `spikes/key-protect` ← مدیریت کلید و Recovery Key
* `spikes/shamsi` ← تقویم و ارقام
* `spikes/lan-mode` ← سرور LAN (فاز ۷)
* `spikes/escpos`، `spikes/usb-identity` ← چاپ حرارتی و Backup USB
