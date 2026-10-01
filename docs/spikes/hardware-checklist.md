# چک‌لیست تأیید سخت‌افزاری فاز ۰

این موارد با کد و CI تأیید شده‌اند ولی نتیجه نهایی آن‌ها به **دستگاه فیزیکی** وابسته است. هر بند را روی سخت‌افزار واقعی اجرا و نتیجه را در ستون آخر ثبت کنید. Installer از Artifact `phase0-windows` در GitHub Actions دانلود می‌شود.

| # | سناریو | مراحل | معیار قبولی | نتیجه |
|---|---|---|---|---|
| H1 | SQLCipher + DPAPI روی Win10 22H2 و Win11 | نصب Installer → «Run database self-test» | `WindowsDpapiMachine`، SQLCipher 4.5.7، `reopened: true` | ☐ Win10 ☐ Win11 |
| H1b | انتقال به کامپیوتر دیگر | کپی `spike.db` و `.key` به PC دوم و باز کردن | باز **نشود**؛ با Recovery Key باز شود | ☐ |
| H2 | WebView2 آفلاین | VM ویندوز ۱۰ تمیز، بدون اینترنت، Installer `offline-*.exe` | برنامه بدون خطا باز شود | ☐ |
| H3 | پرینتر حرارتی 80mm | 80mm · دری → «ESC/POS raster» و «WebView2 silent print» | بدون Dialog؛ متن خوانا؛ برش خودکار؛ بدون کاغذ اضافه | ☐ Xprinter ☐ Epson ☐ Rongta |
| H3b | پرینتر حرارتی 58mm | 58mm · پښتو → «ESC/POS raster» | حروف پشتو خوانا | ☐ |
| H4 | LAN با ۳ کامپیوتر | PC1: `lan_server`؛ PC2/PC3: `lan_client <code>` (یکی Wi-Fi، یکی کابل) | کشف خودکار، Pair، دریافت Event در کمتر از 300ms | ☐ |
| H4b | قطع سرور | سرور را ببندید | Client خطای واضح بدهد | ☐ |
| H5 | USB | `usb_probe` با ۵ فلش + ۱ هارد اکسترنال | Vendor/Product درست؛ Serial جعلی ← `null`؛ هارد اکسترنال هم لیست شود | ☐ |
| H7 | A5/A6 روی پرینتر معمولی | لیزری + جوهرافشان؛ «Check A5/A6 + manual tray»؛ کاغذ A5 و A6 در سینی دستی؛ A5 · دری → silent print | اندازه/سینی درست تشخیص داده شود؛ چاپ از سینی دستی بدون تغییر تنظیمات (یا با یک‌بار تنظیم Preferences) | ☐ لیزری ☐ جوهرافشان |
| H7b | N-up با خط برش | «2 on A4» و «4 on A4» → silent print → برش با قیچی/کاتر | خط برش و تیک‌ها چاپ شوند؛ قطعه‌ها A5/A6 با رسید کامل | ☐ |
| H6 | A4 Compact | پرینتر لیزری A4 → «A4 compact» → «silent print» | رسید در محل تنظیم‌شده ±1mm | ☐ |

اگر بندی رد شد: نتیجه را در [جدول ریسک](../risk-register.md) به‌روز کنید؛ برای H3 روش پشتیبان (Silent Print) از قبل آماده است.
