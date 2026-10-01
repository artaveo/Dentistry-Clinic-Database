# Spike 3 — Thermal Printing (58/80mm)

**سؤال:** چاپ مستقیم روی پرینتر حرارتی بدون Dialog ممکن است؟ (WebView2 Silent Print در برابر ESC/POS)
**نتیجه:** ✅ هر دو روش پیاده‌سازی شدند. **تصمیم:** ESC/POS Raster روش اصلی، Silent Print پشتیبان (ADR-13).

## چرا متن ESC/POS ممکن نیست

فونت‌ها و Code Pageهای داخلی پرینترهای حرارتی اتصال حروف عربی‌نویس را انجام نمی‌دهند، Bidi ندارند و حروف پشتو (ښ ږ ګ ډ ټ ڼ) را اصلاً ندارند. پس **هیچ متنی به‌صورت متن ارسال نمی‌شود.**

## مسیر انتخاب‌شده

```text
receipt.html (همان قالب PDF)
   │  WebView2: SVG foreignObject → canvas  (spikes/tauri-shell/dist/raster.js)
   ▼
PNG دقیقاً 576 نقطه (80mm) یا 384 نقطه (58mm) — 203dpi
   │  Rust Core: Threshold/Dither → 1-bit  (spikes/escpos/src/bitmap.rs)
   ▼
ESC/POS: ESC @ · GS v 0 (باندهای 128 خطی) · Feed · Cut · (Drawer)
   │  Windows Spooler, DataType=RAW  (spikes/escpos/src/spooler.rs)
   ▼
پرینتر — بدون Dialog
```

## شواهد

* `escpos` تست‌ها: هدر `GS v 0`، تقسیم به باند، رد تصویر عریض‌تر از هد، Dither.
* `png2escpos --preview` تصویر 1-bit دقیق «آنچه هد می‌سوزاند» را می‌سازد؛ رسید 58mm پشتو در 1-bit کاملاً خوانا است.
* رستر داخل WebView (`raster.js`) با اسکرین‌شات مرورگر مقایسه می‌شود (نسبت جوهر 0.98 / 0.975). در نسخه اول متن خالی چاپ می‌شد (فونت‌های داخل تصویر SVG بعد از `decode()` لود می‌شوند) — اکنون رندر تا پایدار شدن تکرار می‌شود و تست این خطا را می‌گیرد.
* CI ویندوز: `png2escpos --list` لیست پرینترهای Spooler را با `EnumPrintersW` برمی‌گرداند.
* حجم یک رسید 80mm حدود 49KB ESC/POS است (روی USB/شبکه کمتر از 1 ثانیه).

## مقایسه دو روش

| معیار | ESC/POS Raster (اصلی) | WebView2 Silent Print (پشتیبان) |
|---|---|---|
| وابستگی به درایور | فقط پذیرش RAW (همه پرینترهای POS رایج) | درایور GDI ویندوز با Page Size صحیح |
| کنترل برش کاغذ / کشو پول | ✅ دستور مستقیم | ❌ وابسته به تنظیمات درایور |
| طول کاغذ | دقیق به اندازه محتوا | وابسته به درایور (برخی درایورها کاغذ اضافه Feed می‌کنند) |
| کیفیت متن | 203dpi 1-bit (Threshold) — خوانا | رندر درایور |
| پرینترهای غیر ESC/POS | ❌ | ✅ |

## باقی‌مانده (نیازمند سخت‌افزار)

تست روی مدل‌های رایج بازار افغانستان (Xprinter XP-58/XP-80، Epson TM-T20، Rongta، Sewoo) — [چک‌لیست H3](hardware-checklist.md). ریسک در جدول ریسک با راهکار دوروشه ثبت شده است.
