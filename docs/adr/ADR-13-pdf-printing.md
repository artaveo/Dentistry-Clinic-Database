# ADR-13 — PDF و چاپ (قطعی‌شده در فاز ۰)

**وضعیت:** پذیرفته‌شده · **شواهد:** [Spike 2](../spikes/02-rtl-pdf.md)، [Spike 3](../spikes/03-thermal-printing.md)، [Spike 4](../spikes/04-a4-compact.md)، [Spike 4b](../spikes/09-small-paper.md)

## تصمیم
1. **یک قالب HTML** برای هر سند؛ همان قالب برای نمایش، PDF، چاپ A4/A5/A6 و رسید حرارتی.
2. **PDF:** WebView2 `ICoreWebView2_7::PrintToPdf` با Page Size و Margin به میلی‌متر. کاغذ رول: ارتفاع = ارتفاع محتوا.
3. **چاپ معمولی (A4/A5/A6/Compact روی A4):** WebView2 `Print` (PrintAsync) با `PrinterName`، Page Size و Margin — بدون Dialog.
4. **پرینتر حرارتی — روش اصلی: ESC/POS Raster.** رسید داخل WebView2 به PNG با عرض دقیق هد (576/384 نقطه) رستر می‌شود، Rust Core آن را 1-bit و با `GS v 0` (باندهای ۱۲۸ خطی) + Cut + Drawer به‌صورت **RAW** به Spooler ویندوز می‌فرستد.
5. **پرینتر حرارتی — روش پشتیبان:** همان Silent Print بند ۳ برای پرینترهایی که ESC/POS نیستند. انتخاب روش در تنظیمات هر پرینتر.
6. متن هرگز به‌صورت متن ESC/POS ارسال نمی‌شود (عدم پشتیبانی از اتصال حروف و حروف پشتو).
7. **اندازه سند همیشه کوچک و استاندارد است و PDF همیشه اندازه واقعی سند را دارد (نه A4)**، مستقل از پرینتر کلینیک. Output Profileها:

   | Profile | پیاده‌سازی |
   |---|---|
   | Thermal 58/80mm | بند ۴ (یا ۵) |
   | A4 Compact | رسید در اندازه واقعی با موقعیت مطلق (mm) روی A4؛ پیش‌فرض نیمه بالا |
   | A5/A6 روی پرینتر معمولی | Page Size واقعی؛ سینی توسط درایور انتخاب می‌شود (WebView2 انتخاب سینی ندارد)؛ پشتیبانی اندازه و سینی دستی با `DeviceCapabilitiesW` تشخیص داده می‌شود |
   | N-up (۲ یا ۴ روی A4) | چیدمان خودمان با رسیدهای اندازه واقعی + خط برش و تیک لبه؛ `PagesPerSide` (کوچک‌نمایی) استفاده نمی‌شود؛ رسیدی که جا نشود ۱-up چاپ می‌شود |
   | فقط PDF | `PrintToPdf` در اندازه واقعی سند |

## پیامدها
* کیفیت و چیدمان رسید روی همه پرینترها یکسان است و فقط به WebView2 وابسته است.
* Raster در WebView به دلیل لود ناهمزمان فونت در تصویر SVG، تا پایدار شدن رندر تکرار می‌شود (تست خودکار دارد).

## پیاده‌سازی در فاز 5A (v0.5.0)

* **سند:** `ui/src/print/DocumentSheet.tsx` هر سند را با واحد mm و اندازه واقعی کاغذ می‌سازد (A4/A5/A6، حاشیه از `MARGIN_MM`)؛ پیش‌نمایش همان سند است که با `transform: scale` کوچک شده (`DocumentPreview.tsx`).
* **چاپ و PDF:** سند در `#print-root` رندر می‌شود که فقط در `@media print` و با `@page { size: Wmm Hmm; margin: 0 }` دیده می‌شود. فرمان‌های Tauri در `app/src/print.rs`: `print_page` (چاپ بی‌صدا با WebView2 روی پرینتر انتخاب‌شده، اندازه کاغذ از سند) و `save_pdf` (`PrintToPdf` در اندازه سند، در پوشه `exports/documents`). در مرورگر (حالت توسعه) پنجره چاپ مرورگر باز می‌شود.
* **Output Profile هر کامپیوتر:** `localStorage` با کلید `artaveo.print` = `{ printer, smallPaper: "native" | "compact_a4" }`؛ «بدون پرینتر» = `printer: null` (فقط PDF). `compact_a4` همان A4 Compact بالا برای A5/A6 است، با خط برش. پشتیبانی اندازه کاغذ پرینتر با فرمان `printer_papers` (`DeviceCapabilitiesW`) بررسی و هشدار داده می‌شود.
* **اسناد بالینی:** شماره سری سالانه برای هر نوع (`RX`، `CF`، `PO`، `RF`، `IR`، `MC`، `LO`، `MR`)، محتوای منجمد هنگام صدور، شمارش چاپ و PDF، باطل کردن با دلیل (`core/src/documents.rs`).
* **باقی‌مانده برای فاز ۶:** پرینتر حرارتی (بند ۴ و ۵) و N-up؛ رسید و فاکتور فاز ۶ همین `DocumentSheet`/`DocumentPreview` را با کاغذ تازه استفاده می‌کنند.
* **تست:** E2E اندازه صفحه PDF (MediaBox) را برای A5، A4 و A4 Compact بررسی می‌کند (`ui/e2e/v050.spec.ts`).
