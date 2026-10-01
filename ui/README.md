# ui — React + TypeScript (فاز ۱ و ۲)

فقط UI؛ هیچ دسترسی مستقیم به دیتابیس (ADR-01). تصمیم‌های فاز ۰ که این‌جا اعمال می‌شوند:

* فونت‌ها: Vazirmatn + Noto Sans Arabic برای ؋ (ADR-19) — `spikes/tauri-shell/dist/fonts`
* Date Picker: `@internationalized/date` + React Aria با `PersianCalendar` (ADR-21)
* قالب‌های چاپ HTML (`receipt.js`, `receipt.css`, `sheet.html` برای N-up) و Raster حرارتی (`raster.js`) — `spikes/tauri-shell/dist/`
