# Spike 8 — USB Identity

**سؤال:** خواندن Serial/Volume ID واقعی USB در Windows.
**نتیجه:** ✅ **تصمیم:** ADR-22.

## آنچه ساخته شد (`spikes/usb-identity`)

* `windows.rs`: پیمایش Drive Letterها، `IOCTL_STORAGE_QUERY_PROPERTY` (Vendor، Product، **Serial سخت‌افزاری**، BusType) بدون نیاز به Admin (Handle با دسترسی صفر)، و `GetVolumeInformationW` (Volume Serial، Label، File System).
* `marker.rs`: فایل `ArtaveoBackup/device.json` روی فلش با `device_id` تصادفی و **HMAC-SHA256** با کلید مخفی کلینیک (که در دیتابیس رمزشده است).
* `verify()`: نتیجه `Trusted` / `Cloned` / `Unknown`.
* `examples/usb_probe` — لیست و پایش اتصال/قطع (هر ۲ ثانیه `GetLogicalDrives`).

## یافته‌ها

| منبع شناسه | قابل اعتماد؟ |
|---|---|
| Drive Letter (`E:`) | ❌ با هر اتصال ممکن است عوض شود |
| Volume Serial | ❌ با هر Format عوض می‌شود |
| Serial سخت‌افزاری | ⚠️ بهترین سیگنال سیستم‌عامل، ولی فلش‌های ارزان یا Serial ندارند یا Serial تکراری جعلی دارند (`0123456789ABCDEF`، `AA00000000011234`) ← لیست سیاه |
| Marker امضاشده روی فلش | ✅ پایدار، مخصوص همان کلینیک، قابل جعل نیست |

* **هارد اکسترنال USB** در ویندوز `DRIVE_FIXED` گزارش می‌شود نه `DRIVE_REMOVABLE` ← فیلتر بر اساس **BusType = USB** است.
* تشخیص اتصال با Polling ارزان `GetLogicalDrives` (بدون نیاز به پنجره پیام `WM_DEVICECHANGE`) — مناسب Service حالت سرور.

## تصمیم

هویت = Marker معتبر برای همین کلینیک **و** (اگر فلش Serial معتبر داشت) مطابقت Serial. اگر Marker روی فلش دیگری کپی شود ← `Cloned` و Backup انجام نمی‌شود. فلش بدون Marker ← «New Backup Device Detected».

## باقی‌مانده

تست با ۵ فلش واقعی (برند و ارزان) و یک هارد اکسترنال — [چک‌لیست H5](hardware-checklist.md).
