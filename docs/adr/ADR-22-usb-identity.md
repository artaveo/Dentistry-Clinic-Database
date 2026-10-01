# ADR-22 — هویت دستگاه USB برای Backup

**وضعیت:** پذیرفته‌شده · **شواهد:** [Spike 8](../spikes/08-usb-identity.md)

## تصمیم
* دستگاه‌ها بر اساس **BusType = USB** شناسایی می‌شوند (فلش و هارد اکسترنال)، نه Drive Type و نه Drive Letter.
* هنگام «Use for Backup»: فایل `ArtaveoBackup/device.json` شامل `device_id` تصادفی و HMAC-SHA256 با کلید مخفی کلینیک نوشته می‌شود؛ `device_id` + Vendor/Product + Serial سخت‌افزاری (در صورت معتبر بودن) در دیتابیس ذخیره می‌شود.
* اتصال بعدی: Marker معتبر + تطابق Serial ← `Trusted`؛ Marker معتبر ولی Serial متفاوت ← `Cloned` (Backup انجام نمی‌شود، هشدار)؛ بدون Marker ← «New Backup Device Detected».
* Serialهای جعلی/تکراری شناخته‌شده نادیده گرفته می‌شوند.
* تشخیص اتصال با Polling `GetLogicalDrives` هر ۲ ثانیه.
