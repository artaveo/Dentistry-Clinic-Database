# راهنمای پشتیبانی — پیدا کردن سریع باگ

برای تیم Artaveo. هدف: وقتی چند مشتری هم‌زمان زنگ بزنند، در چند دقیقه بفهمید خطا از کجاست.

## چه چیزی ثبت می‌شود

| چه | کجا | چه می‌گوید |
|---|---|---|
| لاگ روزانه | `C:\ProgramData\ArtaveoDental\logs\artaveo.YYYY-MM-DD.log` (۱۴ روز) | هر خط یک JSON؛ `level`، زمان، پیام |
| خطای Core | همان لاگ، پیام `request failed` | نام متد (`method`)، کامپیوتری که درخواست داد (`origin`)، متن خطا، و `reference` |
| خطای صفحه | همان لاگ، پیام `UI ERROR` | صفحه‌ای که کاربر در آن بود (`place`)، پیام و stack |
| کرش برنامه | همان لاگ پیام `PANIC` + فایل کامل در `crashes\crash-<زمان>.txt` | فایل و خط دقیق کد (`where`)، thread، backtrace، نسخه و commit |
| باز نشدن کلینیک | لاگ `core failed to start` + صفحهٔ «حالت امن» | دلیل (`reason`): `key_unavailable`، `wrong_key`، `damaged`، `newer_than_app`، `migration`، `other` |
| فایل خراب‌شده | `quarantine\<زمان>\` | هیچ‌وقت پاک نمی‌شود؛ برای بررسی بعدی |

هیچ داده بیمار در لاگ، کرش و گزارش نیست (فقط شناسه‌ها).

## وقتی مشتری زنگ می‌زند

۱. از مشتری **کد پیگیری** را بخواهید. هر خطای داخلی روی صفحه با «کد پیگیری: K7M2QX» (شش حرف) نشان داده می‌شود.
۲. اگر کد ندارد: از او بخواهید **سیستم ← گزارش خطا برای پشتیبانی ← ساخت گزارش** را بزند و فایل `artaveo-support-….txt` را بفرستد (واتساپ/ایمیل). برنامه خودش چیزی نمی‌فرستد.
۳. اگر برنامه اصلاً باز نمی‌شود: در «حالت امن» دکمهٔ **ساخت گزارش خطا** همان فایل را می‌سازد.

## خواندن گزارش

فایل گزارش به ترتیب این بخش‌ها را دارد:

1. **مشخصات**: `app_version`، `git_commit`، معماری، ویندوز، نسخهٔ دیتابیس، وضعیت integrity، آخرین Backup، وضعیت لایسنس.
2. **crash files**: نام فایل و `where:` (فایل:خط). این معمولاً جواب نهایی است: `git checkout <git_commit>` و خط را باز کنید.
3. **errors in the last two log files**: فقط خطوط ERROR دو روز اخیر. دنبال `reference` یا `method` بگردید.
4. **newest log lines**: ۳۰۰ خط آخر برای دیدن آنچه پیش از خطا رخ داد.

## جست‌وجوی سریع در لاگ

```powershell
# همهٔ خطاهای امروز
Select-String -Path C:\ProgramData\ArtaveoDental\logs\artaveo.*.log -Pattern '"level":"ERROR"'

# یک کد پیگیری مشخص
Select-String -Path C:\ProgramData\ArtaveoDental\logs\artaveo.*.log -Pattern 'K7M2QX'

# فقط خطاهای صفحه (UI)
Select-String -Path C:\ProgramData\ArtaveoDental\logs\artaveo.*.log -Pattern 'UI ERROR'
```

## اولویت‌بندی وقتی چند مشتری با هم زنگ می‌زنند

۱. `app_version` و `git_commit` همهٔ گزارش‌ها را کنار هم بگذارید. اگر همه یک نسخه‌اند و `where:` یا `method` یکسان است، یک باگ مشترک است و یک رفع کافی است.
۲. `PANIC` و «حالت امن» بالاترین اولویت‌اند (برنامه کار نمی‌کند). سپس `request failed` با `method` مالی (`payments.*`, `invoices.*`). سپس `UI ERROR`.
۳. `integrity: failed` یعنی فایل دیتابیس مشکل دارد؛ مشتری را به «حالت امن ← بازگردانی از آخرین پشتیبان» هدایت کنید و فایل `quarantine` را نگه دارید.
۴. بعد از رفع: نسخهٔ جدید را منتشر کنید؛ برنامه خودش در کلینیک‌های آنلاین نسخه را پیشنهاد می‌دهد و پیش از نصب Backup می‌گیرد.
