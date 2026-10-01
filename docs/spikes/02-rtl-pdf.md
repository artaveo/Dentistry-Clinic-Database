# Spike 2 — RTL PDF

**سؤال:** PDF فارسی/پشتو با متن مختلط انگلیسی و اعداد، با Page Size دلخواه (A6، 80mm) درست تولید می‌شود؟
**نتیجه:** ✅ بله. **تصمیم:** ADR-13 (قطعی‌شده) و ADR-19 (فونت).

## روش

* یک مدل رسید واحد: `spikes/tauri-shell/dist/receipt.js` + `receipt.css`، استفاده‌شده در `receipt.html` (یک رسید) و `sheet.html` (N-up) — همان فایل‌هایی که برنامه Tauri نمایش می‌دهد.
* تولید PDF با موتور Chromium (در CI ویندوز با **Microsoft Edge** که هم‌خانواده WebView2 است؛ در برنامه با `ICoreWebView2::PrintToPdf` — دستور `save_pdf` در `spikes/tauri-shell/src/print.rs`).
* `npm run render` دوازده PDF می‌سازد و `npm run verify` آن‌ها را به‌صورت ماشینی بررسی می‌کند (pdf.js).

| خروجی | ابعاد | زبان |
|---|---|---|
| `receipt-80mm-fa.pdf` | 80mm × ارتفاع محتوا | دری، ارقام فارسی |
| `receipt-80mm-ps.pdf` | 80mm × ارتفاع محتوا | پشتو |
| `receipt-80mm-en.pdf` | 80mm | انگلیسی (LTR) |
| `receipt-80mm-fa-latn.pdf` | 80mm | دری با ارقام لاتین (ADR-09) |
| `receipt-58mm-ps.pdf` | 58mm | پشتو |
| `receipt-a6-fa.pdf` | A6 (105×148) | دری |
| `receipt-a4-compact-*.pdf` | A4 | Spike 4 |
| `receipt-a5-fa.pdf`, `receipt-a6-ps.pdf` | A5، A6 | Spike 4b |
| `sheet-2up-fa.pdf`, `sheet-4up-ps.pdf` | A4 (N-up) | Spike 4b |

## بررسی‌های خودکار (همه PASS)

* اندازه صفحه با خطای کمتر از 0.5mm
* فقط فونت‌های همراه برنامه Embed شده‌اند (هیچ Fallback به فونت سیستم)
* شناسه‌های LTR (`RC-1405-000123`، `P-000123`) داخل متن RTL سالم می‌مانند
* حروف خاص پشتو (ښ ټ ډ ې ړ) وجود دارند؛ ارقام فارسی؛ علامت افغانی ؋ با Glyph واقعی
* بررسی بصری PNGها (اتصال حروف، ترتیب Bidi، ZWNJ در «دندان‌پزشکی»، «عصب‌کشی») ✔

## یافته‌ها

1. **Vazirmatn علامت ؋ (U+060B) را ندارد.** Chromium بی‌صدا به فونت سیستم (FreeSerif/Segoe) Fallback می‌کرد ← خروجی روی هر کامپیوتر متفاوت. راهکار: `@font-face` با `unicode-range: U+060B` از **Noto Sans Arabic** (OFL). بررسی «فقط فونت‌های همراه» در verify این را برای همیشه قفل می‌کند.
2. **Bidi:** شناسه‌ها و شماره تلفن باید داخل `<span class="ltr">` با `unicode-bidi: isolate` و `white-space: nowrap` باشند؛ در غیر این صورت در 58mm از وسط خط تیره شکسته می‌شوند.
3. **کاغذ رول:** ارتفاع صفحه = ارتفاع اندازه‌گیری‌شده محتوا + حاشیه ← هیچ کاغذ سفید اضافه مصرف نمی‌شود.
4. **Intl/ICU:** `fa-AF` و `ps-AF` نام ماه‌های افغانی (میزان/تله) را می‌دهند، ولی `en-AF` نام ایرانی «Mehr» و `ps-AF` در `dateStyle: full` رشته «AP» نشان می‌دهد ← فقط اعداد تقویم از ICU گرفته می‌شوند و نام ماه‌ها از جدول خودمان (`shamsi.js` و `shamsi` crate).
5. **کپی متن از PDF:** لیگاتور «لا» هنگام کپی/جستجو در PDF به‌صورت «ال» استخراج می‌شود (ToUnicode در Chromium). روی چاپ و نمایش اثری ندارد؛ فقط جستجوی متن داخل PDF. ریسک کم، ثبت شد.
6. فرمت پول: `Intl.NumberFormat('fa-AF')` جداکننده هزارگان «٬» و ارقام فارسی را درست می‌دهد؛ AFN بدون اعشار نمایش داده می‌شود، ذخیره به واحد کوچک ×100 (ADR-06).
