# طراحی: سیستم حسابداری و گزارش‌دهی استاندارد کلینیک (Artaveo Dental)

تاریخ: ۱۴۰۵/۰۸/۲۷ (۲۰۲۶-۱۱-۱۸) · نسخه‌ی ۳. نسخه‌ی ۲ بازنویسی از صفر بود. نسخه‌ی ۳: مهاجرت داده حذف شد، چون همه‌ی داده‌ی فعلی آزمایشی است؛ schema کم‌حجم شد؛ دامنه‌ی کامل در ۵ فاز.
وضعیت: **فاز ۱ = فقط پژوهش و طراحی. هیچ کدی عوض نشده.**

**دستور مالک:** بخش مالی **از صفر** و به‌شکل یک سیستم استاندارد طراحی شود، نه ارتقای سیستم فعلی. پس سند سه بخش جدا دارد:
* **بخش الف: پژوهش.** قانون افغانستان، استاندارد دفترداری، و نرم‌افزارهای دندان.
* **بخش ب: سیستم ایده‌آل از صفر.** در این بخش هیچ نامی از جدول‌ها یا صفحه‌های فعلی برده نمی‌شود.
* **بخش ج: گذار.** جدول‌ها، کد، RPCها و صفحه‌های مالی فعلی حذف یا جایگزین می‌شوند. داده‌ی آزمایشی دور ریخته می‌شود. از این به بعد مهاجرت‌های نسخه‌دار از داده‌ی مشتری واقعی محافظت می‌کنند.
* **بخش د: فازهای اجرا.** هر فاز به یک نسخه‌ی قابل انتشار و کامل‌آزموده ختم می‌شود.

نشانه‌ها: ✅ = از منبع خوانده شد · ⚠ = تأییدنشده یا شاید کهنه · 💡 = تصمیم طراحی.
خواننده: مالک (فارسی ساده). نام جدول، ستون و فایل انگلیسی است تا مستقیم در کد استفاده شود.

> **قاعده‌ی طلایی:** برنامه **هیچ مبلغ یا نرخ مالیات را حساب نمی‌کند.** فقط دفترها و گزارش‌ها را طوری نگه می‌دارد که مأمور مالیه خودش دقیق حساب کند. تنها «مبلغ مالیات» در برنامه، پولی است که کلینیک واقعاً از معاش یا کرایه **کسر کرده** است. این مبلغ را حسابدار **دستی** وارد می‌کند، چون بدهی واقعی کلینیک به دولت است. برنامه آن را پیشنهاد یا محاسبه نمی‌کند.

---

## فهرست
* ۰. خلاصه برای مالک
* **الف. پژوهش:** الف۱ افغانستان · الف۲ استاندارد دفترداری · الف۳ نرم‌افزارهای دندان · الف۴ بررسی پژوهش قبلی
* **ب. سیستم ایده‌آل:** ب۱ اصول · ب۲ مفاهیم · ب۳ چارت حساب · ب۴ اسناد و چرخه‌ی کار · ب۵ قواعد ثبت · ب۶ مدل داده · ب۷ دوره و قفل و شماره · ب۸ صفحه‌های مالی · ب۹ گزارش‌ها · ب۱۰ تطبیق · ب۱۱ قواعد درستی · ب۱۲ دسترسی
* **ج. گذار:** ج۱ آنچه امروز هست و حذف می‌شود · ج۲ مهاجرت جایگزینی · ج۳ حفاظت از مشتری‌های واقعی · ج۴ RPCها و صفحه‌ها
* **د. اجرا:** فازها، فایل‌ها، آزمون، ریسک CI، اندازه
* **هـ. سؤال‌های مالک** (با پیش‌فرض)
* **و. منابع و آنچه تأیید نشد**

---

## ۰. خلاصه برای مالک

* **یک مدل استاندارد:**
  * **سند:** فاکتور، رسید، برگشت، یادداشت اعتباری، بل خرید، سند پرداخت، معاش، انتقال پول.
  * **دفتر روزنامه‌ی دوطرفه:** هر سند خودکار یک ثبت حسابداری می‌سازد که بدهکار و بستانکارش برابر است.
  * **چارت حساب استاندارد کلینیک.**
  * **زیر-دفترها:** بیماران، تأمین‌کنندگان، کارمندان، داکتران، صندوق و بانک. هرکدام همیشه با حساب کنترلش در دفتر کل برابر است.
* **دو مبنای گزارش با برچسب روشن:**
  * **نقدی** (پول گرفته‌شده و داده‌شده). برای اشخاص حقیقی، ماده ۳۸.
  * **تعهدی** (کار انجام‌شده و فاکتورشده). برای شرکت‌ها، ماده ۳۷، و برای مدیریت.
* **بخش مالی واحد، مطابق نظر استاد:**
  * جدول اصلی: «عواید».
  * زیربخش‌ها: اقساط، طلب‌ها، پیش‌پرداخت‌ها، تخفیف و بخشش، برگشت، مصارف و تأمین‌کنندگان، کارمندان و معاش، سهم داکتران، صندوق و بانک، دوره‌ها، دفاتر.
  * صفحه‌ی «تطبیق» که ثابت می‌کند همه‌ی جمع‌ها به هم و به پول واقعی صندوق و بانک می‌رسند.
* **دوره و قفل:**
  * ماه بسته می‌شود؛ ربع و سال قفل می‌شوند.
  * سند صادرشده هرگز ویرایش یا حذف نمی‌شود. اصلاح فقط با سند معکوس است.
  * شماره‌ها پیاپی و بدون فاصله‌اند.
  * گزارش رسمیِ صادرشده شماره و اثر انگشت (hash) دارد و چاپ دوباره‌اش عیناً همان است.
* **۱۸ گزارش رسمی و مدیریتی** با سربرگ قانونی: نام حقوقی، TIN، جواز، دوره‌ی شمسی، صفحه x از y، امضا و مهر. یک «بسته‌ی سالانه برای وزارت» هم با یک دکمه ساخته می‌شود.
* **کم‌حجم:**
  * شناسه، نوع و وضعیت عددی‌اند.
  * تاریخ عدد `yyyymmdd` است.
  * snapshot JSON ذخیره نمی‌شود.
  * بودجه‌ی index ۸ تا است.
  * قواعد `docs/design/db-size-rules.md` (سشن f-db-size) بر این بخش برنده است.
* **گذار:**
  * داده‌ی فعلی آزمایشی است. یک مهاجرت جدول‌ها و داده‌ی مالی قدیم را حذف و schema تازه را می‌سازد.
  * داده‌ی بیمار، درمان و انبار حفظ می‌شود.
  * کد، RPCها و صفحه‌های مالی قدیم حذف می‌شوند.
  * از این به بعد هر مهاجرت مالی افزایشی است و آزمون برابری دارد.
* **پنج فاز** (حدود ۳۲ تا ۳۸ روز کاری). هر فاز یک انتشار کامل است. شروع پس از گزارش f-db-size و سبز شدن CI آواتارها.

---

# الف. پژوهش

## الف۱. افغانستان
| موضوع | یافته | وضعیت |
|---|---|---|
| سال مالی | ماده ۳: سال هجری شمسی، ۱ حمل تا آخر حوت. شرکت می‌تواند کتباً سال دیگری بخواهد. | ✅ |
| ثبت سوابق | ماده ۳۶: سوابق **همه‌ی معاملات، دارایی‌های منقول و غیرمنقول و همه‌ی عواید** تهیه، نگه‌داری و به درخواست وزارت مالیه ارائه شود. | ✅ |
| مدت نگهداری | مقرره ۳۶.۵ رهنمود: **۵ سال** پس از پایان سال مربوط. | ✅ رهنمود ۲۰۰۹ · ⚠ نسخه‌ی تازه‌تر ندیدم |
| دفاتر لازم | مقرره ۳۹.۲: ژورنال، لجر، موجودی و شمارش انبار، ثبت دارایی‌ها، اطلاعات بانکی، اسناد حقوقی و قرارداد، صورت‌ها، فاکتور و رسید، صورت عواید، بیلانس. | ✅ |
| روش حساب | ماده ۳۷: شرکت و LLC **تعهدی**. ماده ۳۸: دیگران **نقدی** (زمان دریافت عاید و پرداخت مصرف). تغییر روش با درخواست کتبی ممکن است. | ✅ |
| BRT | ماده ۶۴: شامل اشخاص حقوقی، و اشخاص حقیقی با دریافت **≥ ۷۵۰٬۰۰۰ افغانی در ربع**. روی کل عواید، پیش از هر کسر. ماده ۸۸(۵): اظهارنامه‌ی هر ربع تا **۱۵ ماه بعد**. مقرره ۶۶.۴: برگشت‌ها از دریافتی کم می‌شود. | ✅ |
| نرخ BRT | قانون ۲۰۰۹: ۲٪. از ۲۰۱۵: ۴٪ (SIGAR، AAN). Quest (جولای ۲۰۲۶): «معمولاً ۴٪؛ اصلاحات ۱۴۰۵ عوضش نکرد». | ✅ تاریخی · برنامه نرخ لازم ندارد |
| مالیات ثابت | ماده ۷۴/۷۵: شخص حقیقی با عواید سالانه **< ۳٬۰۰۰٬۰۰۰** مالیات ثابت می‌دهد، به‌جای مالیات عایدات و BRT. بین ۵۰۰ هزار و ۳ میلیون: ۳٪، یا انتخاب مالیات عایدات + BRT برای حداقل ۳ سال. مقرره ۷۴.۶ «داکتران طبی» را صریحاً در این رژیم می‌آورد. مثال ۷۵.۵ دندان‌پزشکی به نام ملالی است: عاید ۹۰۰٬۰۰۰ و **مصارف مستند** ۶۰۰٬۰۰۰. | ✅ رهنمود ۲۰۰۹ · ⚠ آستانه‌ها شاید عوض شده |
| معاش | ماده ۵۸: کارفرمای **دو کارمند یا بیشتر** مالیات معاش را کسر می‌کند. ماده ۶۰: واریز تا **۱۰ روز** پس از ماه. ماده ۴۶(۳): صورت سالانه به هر کارمند تا یک ماه پس از سال. ماده ۶۱(۲): گزارش تلفیقی سالانه. | ✅ |
| نرخ معاش ۱۴۰۵ | از ۱۴۰۵/۰۴/۲۰: ۰ تا ۱۰٬۰۰۰ = ۰٪ · ۱۰٬۰۰۱ تا ۱۰۰٬۰۰۰ = ۱۰٪ · بیش از ۱۰۰٬۰۰۰ = ۱۵٪ + ۹٬۰۰۰. | ✅ Quest · **برنامه حساب نمی‌کند** |
| مالیات عایدات ۱۴۰۵ | از ۲۰٪ به ۱۰٪، «شامل مراکز صحی خصوصی». شرط: ثبت در وزارت و **اتصال الکترونیکی سیستم مالی با اداره‌ی عواید (ARD)**. اشخاص حقیقی: تا ۱۲۰٬۰۰۰ سالانه معاف. | ✅ Quest + سایت رسمی امارت |
| کرایه | ماده ۵۹: کرایه‌ی ساختمان تجاری بین ۱۰٬۰۰۰ و ۱۰۰٬۰۰۰ در ماه ۱۰٪، بیشتر از آن ۱۵٪. **مستأجر کسر و واریز می‌کند.** | ✅ کلینیک مستأجر مسئول است |
| قراردادی | ماده ۷۲: ۷٪ (بدون جواز) یا ۲٪ (با جواز) از مبلغ قرارداد. پرداخت‌کننده کسر و تا ۱۰ روز واریز می‌کند. | ✅ · ⚠ شمول خرید عادی را حسابدار بگوید |
| ارز | ماده ۴(۲): عاید ارزی به نرخ میانگین خرید د افغانستان بانک در آخر ماه به افغانی تبدیل می‌شود. استفاده از ارز خارجی در معاملات داخلی از ۲۰۲۱ منع است (BBC). | ✅ |
| بررسی و جریمه | ماده ۹۱: اگر اسناد تا ۲۰ روز داده نشود، روزانه ۱۰۰ افغانی (حقیقی) یا ۲۰۰ (حقوقی). ماده ۱۰۱: نداشتن سوابق ۵٬۰۰۰ یا ۲۰٬۰۰۰، و با قصد فرار ارجاع به سارنوالی. | ✅ |

**نتیجه برای طراحی:**
* کلینیک کوچک احتمالاً «شخص حقیقی» است، شاید در رژیم مالیات ثابت. کلینیک بزرگ‌تر BRT ربعوار می‌دهد. شرکت تعهدی است. **یک دفتر باید هر سه را پوشش دهد؛ فقط برچسب مبنا عوض می‌شود.**
* چون مثال قانون «مصارف مستند» دارد، هر مصرف باید سند شماره‌دار داشته باشد.
* هیچ داده‌ی مالی نباید پیش از ۵ سال پاک شود.

## الف۲. استاندارد دفترداری کلینیک کوچک
* **دوطرفه:** هر رویداد حداقل دو سطر دارد و بدهکار = بستانکار. پنج نوع حساب: دارایی، بدهی، سرمایه، عاید، مصرف (Open Dental ✅).
* **دریافت ≠ فاکتور:**
  * رسید «پول وارد شد» است.
  * «تخصیص» (Allocation) آن پول را به فاکتورها نسبت می‌دهد.
  * پول تخصیص‌نیافته **پیش‌پرداخت بیمار** است و در حساب‌ها یک **بدهی** حساب می‌شود.
* **کاهش عاید و مصرف:**
  * تخفیف و یادداشت اعتباری **کاهش عاید** است (Contra-income).
  * **بخشش بدهی سوخت‌شده** (Write-off) در تعهدی **مصرف** است و در نقدی بی‌اثر.
* **جبران داکتر:**
  * کارمند معاشی: مصرف معاش، با کسر مالیات معاش.
  * داکتر مستقل با فیصدی: مصرف «حق‌الزحمه».
  * **شریک: برداشت از سرمایه، نه مصرف.**
* **خرید:** بل تأمین‌کننده بدهی (Payable) است. پرداخت آن، بدهی را صاف می‌کند.
* **صندوق و بانک:** هر حساب پول مانده‌ی جاری دارد. پایان روز، شمارش فیزیکی با مانده مقایسه می‌شود. پول تحویل‌شده به بانک یک «انتقال» است، نه مصرف.
* **قفل دوره (Lock date):** «همه‌ی ثبت‌های تا تاریخ X قفل شود، مثلاً هنگام بستن سال» (Open Dental ✅).

## الف۳. نرم‌افزارهای دندان
* **Open Dental** ✅: Production & Income، Daily Payments (فیلتر داکتر و نوع پرداخت)، Daily Adjustments، Daily Write-offs، Aging of A/R (۰–۳۰ / ۳۱–۶۰ / ۶۱–۹۰ / ۹۰+)، Deposit slip، ماژول حسابداری با Lock date.
* **Eaglesoft** (طبق zeldent ✅): «Day Sheet + Deposit Report» در پایان روز.
* **ADA و Dentrix** (پژوهش قبلی): نرخ وصول = وصول ÷ تولید.

## الف۴. بررسی پژوهش قبلی (`docs/research/finance-and-dashboard-standards.md`)
| ادعای قبلی | حکم |
|---|---|
| BRT روی کل دریافت، مبنای نقدی، ربعوار تا ۱۵ ماه بعد | ✅. اصلاح: «نقدی» فقط برای اشخاص حقیقی است؛ شرکت‌ها تعهدی‌اند. |
| نرخ پیش‌فرض ۲٪ | ⚠ کهنه؛ از ۲۰۱۵ چهار درصد است. پیشنهاد «مالیات تخمینی با نرخ قابل تنظیم» طبق دستور مالک **حذف شد**. |
| آستانه‌ی ۷۵۰٬۰۰۰ در ربع · BRT قابل کسر از مالیات عایدات | ✅ |
| معاش ⚠ · TIN ⚠ | حالا ✅ (ماده ۵۸ تا ۶۱). TIN واقعاً در برنامه نیست. |

---

# ب. سیستم ایده‌آل (از صفر)

> این بخش عمداً نامی از جدول‌ها و صفحه‌های فعلی نمی‌برد.

## ب۱. اصول
| # | اصل | دلیل |
|---|---|---|
| P1 | **دفتر کل دوطرفه، منبع حقیقت عددی است.** هر عدد هر گزارش یا از دفتر کل می‌آید، یا از زیر-دفتری که با دفتر کل برابر بودنش اثبات شده است. | مقرره ۳۹.۲؛ تطبیق قابل اثبات |
| P2 | **سند، منبع حقیقت حقوقی است.** کاربر سند ثبت می‌کند. دفتر کل از روی سند و در **همان تراکنش** خودکار پر می‌شود. کاربر عادی دبیت و کردیت نمی‌بیند. | ساده برای منشی؛ دقیق برای حسابدار |
| P3 | **سند صادرشده تغییرناپذیر است.** اصلاح یعنی ابطال با **ثبت معکوس**، یا سند اصلاحی. هر کار دلیل و ردپا (audit) دارد. | استاندارد؛ قابل دفاع پیش مأمور |
| P4 | **دو مبنا از یک دفتر:** نقدی و تعهدی، همیشه با برچسب. مبنای «رسمی» را شکل حقوقی کلینیک تعیین می‌کند. | ماده ۳۷ و ۳۸ |
| P5 | **دوره‌ی ماهانه‌ی شمسی، ربع و سال.** دوره‌ی قفل‌شده دست نمی‌خورد. | BRT ربعوار؛ اظهارنامه‌ی سالانه |
| P6 | **فقط افغانی** (عدد صحیح، واحد = پول، یعنی ۱/۱۰۰ افغانی). ارز خارجی فقط یادداشت است. | ماده ۴(۲)؛ منع ارز |
| P7 | **مالیات محاسبه نمی‌شود.** کسرهای تکلیفی دستی‌اند. | دستور مالک |
| P8 | **شماره‌ی پیاپی و بدون فاصله** برای هر نوع سند و هر سال شمسی، داخل همان تراکنش. باطل‌شده‌ها شماره‌شان را نگه می‌دارند. | ردیابی کامل |
| P9 | **گزارش رسمی «صادر» می‌شود:** شماره، پارامترها و hash محتوا ذخیره می‌شود. چاپ دوباره از دفتر تغییرناپذیر ساخته و با hash مقایسه می‌شود. | چاپ دوباره = همان سند، بدون ذخیره‌ی محتوا |
| P10 | **زنجیره‌ی hash** روی دفتر روزنامه. | دستکاری مستقیم دیتابیس آشکار شود |

## ب۲. مفاهیم (واژه‌نامه‌ی یکسان برای کد، UI و گزارش)
| مفهوم | دری | معنی |
|---|---|---|
| Account | حساب | یک سطر در چارت حساب‌ها (§ب۳) |
| Money account | حساب پول | صندوق یا بانک یا موبایل‌منی؛ نوعی از Account |
| Journal entry / line | ثبت روزنامه / سطر | بدهکار = بستانکار |
| Party | طرف حساب | بیمار، تأمین‌کننده، کارمند، داکتر، مالک |
| Sales invoice | فاکتور خدمات | ارزش کار انجام‌شده برای بیمار |
| Receipt | رسید دریافت | پول وارد شد (از بیمار) |
| Allocation | تخصیص | چه مقدار از یک رسید یا اعتبار به کدام فاکتور رفت |
| Patient deposit | پیش‌پرداخت (امانت) بیمار | پول تخصیص‌نیافته؛ بدهی کلینیک |
| Patient refund | برگشت پول به بیمار | پول خارج شد به بیمار |
| Credit note | یادداشت کاهش | کاهش قیمت یک فاکتور، با دلیل |
| Write-off | بخشش بدهی | طلب سوخت‌شده، با دلیل و تأیید |
| Payment plan | برنامه‌ی اقساط | سررسیدها برای بدهی یک یا چند فاکتور |
| Supplier bill | بل خرید | بدهی به تأمین‌کننده یا لابراتوار |
| Payment voucher | سند پرداخت | هر پولی که خارج می‌شود: مصرف، بل، معاش، سهم، مالیات، برداشت |
| Payroll | لیست معاش | معاش ماهانه‌ی کارمندان |
| Provider share statement | صورت سهم داکتر | سهم ماهانه‌ی داکتر، منجمد |
| Transfer | انتقال | صندوق به بانک و برعکس |
| Cash count | شمارش صندوق | بستن روز |
| Fiscal period | دوره | ماه شمسی |
| Official report | گزارش رسمی | نسخه‌ی صادرشده‌ی یک گزارش |

## ب۳. چارت حساب استاندارد کلینیک
| کد | حساب | نوع | زیر-دفتر |
|---|---|---|---|
| **1 دارایی** | | | |
| 1110 | صندوق پذیرش | asset · money=cash | دفتر صندوق |
| 1120 | تنخواه گردان | asset · money=cash | |
| 1210 | بانک (هر حساب بانکی یک سطر: 1211، 1212، …) | asset · money=bank | دفتر بانک |
| 1310 | طلب از بیماران | asset · **control** | زیر-دفتر بیمار |
| 1320 | پیش‌پرداخت معاش (مساعده) | asset · control | زیر-دفتر کارمند |
| 1330 | پیش‌پرداخت به داکتران | asset · control | زیر-دفتر داکتر |
| 1340 | پیش‌پرداخت به تأمین‌کنندگان | asset · control | زیر-دفتر تأمین‌کننده |
| 1510 | تجهیزات (اختیاری؛ برای ثبت دارایی) | asset | فهرست دارایی |
| **2 بدهی** | | | |
| 2110 | بدهی به تأمین‌کنندگان و لابراتوار | liability · control | زیر-دفتر تأمین‌کننده |
| 2210 | پیش‌پرداخت بیماران (امانت) | liability · control | زیر-دفتر بیمار |
| 2310 | معاش پرداخت‌نشده | liability · control | زیر-دفتر کارمند |
| 2320 | سهم داکتران پرداخت‌نشده | liability · control | زیر-دفتر داکتر |
| 2410 | مالیات کسرشده از معاش | liability | دفتر کسر تکلیفی |
| 2420 | مالیات تکلیفی کرایه | liability | دفتر کسر تکلیفی |
| 2430 | مالیات تکلیفی قراردادی | liability | دفتر کسر تکلیفی |
| 2510 | قرض‌ها | liability | |
| **3 سرمایه** | | | |
| 3110 | سرمایه‌ی مالک یا شرکا | equity | |
| 3120 | برداشت مالک یا شرکا | equity | زیر-دفتر مالک |
| 3210 | سود انباشته | equity | |
| 3900 | مانده‌ی افتتاحیه (فقط برای گذار) | equity | |
| **4 عاید** | | | |
| 4110 | عاید خدمات درمانی (زیرحساب اختیاری برای هر گروه خدمت) | income | |
| 4190 | سایر عواید (فروش متفرقه) | income | |
| 4910 | تخفیف‌ها | contra_income | |
| 4920 | یادداشت‌های کاهش | contra_income | |
| **5 مصرف** | | | |
| 5110 | معاش کارمندان | expense | |
| 5120 | حق‌الزحمه و سهم داکتران | expense | |
| 5210 | کرایه | expense | |
| 5220 | برق، آب، گاز و انترنت | expense | |
| 5230 | مواد و لوازم دندانی | expense | |
| 5240 | لابراتوار | expense | |
| 5250 | تعمیر و نگهداری | expense | |
| 5260 | دوا و مصرفی | expense | |
| 5270 | اداری، پاک‌کاری و متفرقه | expense | |
| 5280 | فیس‌ها و جوازها | expense | |
| 5810 | بدهی سوخت‌شده | expense | |
| 5910 | کسر و اضافه‌ی صندوق | expense | |

قواعد چارت:
* کاربر زیرحساب مصرف و حساب پول تازه می‌سازد.
* حساب سیستمی حذف نمی‌شود و نوعش عوض نمی‌شود.
* حسابی که ثبت دارد فقط غیرفعال می‌شود.
* «دسته‌ی مصرف» در UI یعنی همان زیرحساب‌های ۵.
* کنار هر حساب یک ستون «برچسب رسمی» هست که در گزارش رسمی چاپ می‌شود.

## ب۴. اسناد و چرخه‌ی کار
| سند | پیشوند | چه کسی | جریان |
|---|---|---|---|
| فاکتور خدمات | INV | منشی، حسابدار | از درمان‌های انجام‌شده (یک کلیک)، یا خط دستی. تخفیف روی سطر یا روی کل فاکتور (مبلغ یا درصد؛ ذخیره همیشه به مبلغ). داکتر هر خط ثبت می‌شود. → `issued` |
| رسید دریافت | RC | منشی | مبلغ، حساب پول، بیمار. **تخصیص** به یک یا چند فاکتور (پیش‌فرض: قدیمی‌ترین بدهی، یا فاکتوری که کاربر انتخاب کرده). اضافه = پیش‌پرداخت. |
| تخصیص اعتبار | (بدون سند پولی؛ AL) | منشی | پیش‌پرداخت موجود به فاکتور داده می‌شود. |
| برگشت پول | RF | دسترسی خاص + دلیل | از پیش‌پرداخت بیمار. اگر پول تخصیص‌یافته باشد، اول «لغو تخصیص» یا یادداشت کاهش. |
| یادداشت کاهش | CN | دسترسی خاص + دلیل | کاهش قیمت فاکتور (≤ مانده یا ≤ خالص). اگر پرداخت‌شده بود، مازاد به پیش‌پرداخت می‌رود. |
| بخشش بدهی | WO | مالک + دلیل | طلب سوخت‌شده. |
| برنامه‌ی اقساط | PP | منشی | برای مانده‌ی یک یا چند فاکتور. نسخه‌دار: تغییر = نسخه‌ی تازه، قبلی «جایگزین‌شده». |
| بل خرید | BL | انباردار، حسابدار | از تأمین‌کننده یا لابراتوار، با خطوط مصرف یا کالا. اگر کالا باشد، رسید انبار همراهش ساخته می‌شود. نسیه یا نقد. |
| سند پرداخت | PV | حسابدار | خروج پول از یک حساب پول. هدف: مصرف مستقیم (با خطوط حساب مصرف)، پرداخت بل، پرداخت معاش، مساعده، سهم داکتر، واریز مالیات کسرشده، برداشت مالک، بازپرداخت قرض. کسر تکلیفی روی خط (دستی). |
| سند دریافت دیگر | OR | حسابدار | پولی که از بیمار نیست: سرمایه‌گذاری مالک، قرض، فروش متفرقه. |
| لیست معاش | PR | حسابدار | ماهانه: پیش‌نویس → تأیید (ثبت بدهی معاش) → پرداخت با PV. |
| صورت سهم داکتر | DS | حسابدار، تأیید مالک | ماهانه بر اساس قاعده‌ی جبران. پس از تأیید منجمد می‌شود. پرداخت با PV. |
| انتقال | TR | حسابدار | از یک حساب پول به دیگری. |
| شمارش صندوق | CC | صندوق‌دار | پایان روز یا شیفت: مانده‌ی دفتری و شمارش‌شده. اختلاف با دلیل. |
| ثبت تعدیلی | JV | فقط حسابدار (دسترسی ویژه) | ثبت دستی دوطرفه برای اصلاحات خاص، با دلیل. |
| سند افتتاحیه | OB | مالک (یک‌بار) | مانده‌های شروع. |
| بستن سال | YC | حسابدار | انتقال عاید و مصرف به سود انباشته. |

**وضعیت‌ها:** همه‌ی اسناد `posted` یا `void` دارند. لیست معاش و صورت سهم داکتر حالت `draft` هم دارند. پیش‌نویس در دفتر ثبت نمی‌شود.

## ب۵. قواعد ثبت (Posting rules)
`M` = حساب پول سند · `AR` = 1310 · `DEP` = 2210 · `AP` = 2110. هر سطر بُعد طرف حساب (بیمار، داکتر، …) و بُعد خدمت را دارد.

| سند | بدهکار | بستانکار |
|---|---|---|
| INV | AR = خالص · 4910 = تخفیف | 4110 / 4190 = ناخالص هر خط (با داکتر و خدمت) |
| RC (تخصیص‌یافته) | M | AR |
| RC (اضافه، پیش‌پرداخت) | M | DEP |
| AL تخصیص اعتبار | DEP | AR |
| RF برگشت | DEP | M |
| CN یادداشت کاهش | 4920 | AR (و مازاد پرداخت‌شده: AR→DEP با یک AL معکوس) |
| WO بخشش | 5810 | AR |
| BL بل خرید | حساب مصرف یا کالا | AP (برای هر تأمین‌کننده) |
| PV پرداخت بل | AP | M (و 2430 = کسر تکلیفی) |
| PV مصرف مستقیم | حساب مصرف = ناخالص | M = خالص · 2420/2430 = کسر |
| PR تأیید معاش | 5110 = ناخالص | 2310 = خالص · 2410 = مالیات کسرشده · 1320 = کسر مساعده |
| PV معاش | 2310 | M |
| PV مساعده | 1320 | M |
| DS تأیید سهم | 5120 (یا 3120 اگر داکتر شریک است) | 2320 |
| PV سهم داکتر | 2320 | M |
| PV واریز مالیات | 2410/2420/2430 | M |
| PV برداشت مالک | 3120 | M |
| OR سرمایه یا قرض | M | 3110 / 2510 |
| TR | M مقصد | M مبدأ |
| CC با اختلاف | کسر: 5910 · اضافه: M | کسر: M · اضافه: 5910 |
| ابطال هر سند | ثبت معکوس دقیق، در تاریخ ابطال (اگر دوره باز است) | |
| YC | حساب‌های 4 (بستن) | حساب‌های 5 → 3210 |

**دو مبنا از یک دفتر:**
* **تعهدی:**
  * عاید = 4110 + 4190 − 4910 − 4920.
  * مصرف = همه‌ی حساب‌های 5.
  * نتیجه = سود تعهدی.
* **نقدی:**
  * **دریافت ناخالص از بیماران** = Σ بدهکار حساب‌های پول، وقتی طرف مقابل AR یا DEP است.
  * **برگشت** = Σ بستانکار حساب‌های پول در برابر DEP.
  * **مصارف پرداخت‌شده** = Σ بستانکار پول در برابر حساب‌های 5، و AP، 2310، 1320، 2320، 2410 تا 2430، به تفکیک حسابِ نهایی.
  * **جدا نشان داده می‌شوند و مصرف نیستند:** برداشت، سرمایه، قرض، انتقال.
* **پل بین دو مبنا** (در صفحه‌ی تطبیق):
  > عاید تعهدی − Δطلب بیماران + Δپیش‌پرداخت بیماران − بخشش = خالص دریافت نقدی

## ب۶. مدل داده (کم‌حجم)

### ب۶.۰ قواعد حجم
مالک از حجم دیتابیس ناراضی است: برای چند ده بیمار حدود ۲ MB. سشن `f-db-size` قواعد نهایی را در `docs/design/db-size-rules.md` می‌نویسد. **اگر آن سند با این بخش فرق داشت، آن سند برنده است.** تا آن وقت، این قواعد برای همه‌ی جدول‌های `fin_*` اجرا می‌شود:

| # | قاعده | چرا |
|---|---|---|
| S1 | **کلید سندها و سطرهای پرحجم مالی `INTEGER PRIMARY KEY`** (همان rowid). FK های داخل بخش مالی هم INTEGER. **استثنا:** شناسه‌ی بیمار، داکتر، خدمت، درمان و کاربر در بقیه‌ی برنامه UUID متنی است، پس این ستون‌ها `TEXT` می‌مانند. تأمین‌کننده و کارمند هم TEXT هستند، چون در `party_id` کنار بیمار می‌نشینند و نوع ستون باید یکی باشد. اگر `db-size-rules.md` شناسه‌ها را در کل برنامه عددی کند، این ستون‌ها هم INTEGER می‌شوند. | UUID متنی ۳۶ بایت است و در هر index و FK تکرار می‌شود. عدد ۱ تا ۴ بایت است. |
| S2 | **همه‌ی نوع‌ها، وضعیت‌ها و کدها `INTEGER`** با `CHECK (x BETWEEN …)`. معنی هر عدد در یک `enum` با `#[repr(u8)]` در `shared/src/fin.rs` تعریف می‌شود. جدول جستجو در دیتابیس ساخته نمی‌شود. | `'posted'`، `'salary_advance'` در هر سطر چند بایت است؛ عدد ۱ بایت. |
| S3 | **تاریخ = `INTEGER` به شکل `yyyymmdd`** (مثل `20261118`). **زمان = `INTEGER` ثانیه‌ی یونیکس UTC.** | ۴ بایت به‌جای ۱۰ و ۲۴ بایت. هنوز با چشم خواندنی است و مرتب‌سازی درست است. |
| S4 | **شماره‌ی سند متنی ذخیره نمی‌شود.** فقط `series_year` و `serial` (INTEGER) ذخیره می‌شود. متن `INV-1405-000123` از پیشوندِ نوع سند ساخته می‌شود. یکتایی با `UNIQUE(series_year, serial)` روی همان جدول است. | ۱۵ بایت در هر سطر و یک index متنی کمتر. |
| S5 | **متن تکراری نگه داشته نمی‌شود.** نام بیمار، داکتر و حساب از جدول اصلی‌شان خوانده می‌شود. فقط آنچه **حقوقاً باید منجمد بماند** ذخیره می‌شود: نام خدمت روی خط فاکتور (یک زبان: زبان داده‌ی کلینیک) و نام پرداخت‌کننده‌ی دستی. | حذف `names_json` سه‌زبانه و snapshot های JSON. |
| S6 | **snapshot چاپی ذخیره نمی‌شود.** رسید و فاکتور از سطرهای تغییرناپذیر دوباره ساخته می‌شوند و نتیجه همیشه یکسان است. | snapshot JSON هر رسید ۱ تا ۲ KB است. |
| S7 | **گزارش رسمی: فقط پارامترها و hash** ذخیره می‌شود، نه محتوا. چاپ دوباره گزارش را از دفتر تغییرناپذیر می‌سازد و hash را مقایسه می‌کند. اگر hash فرق کند، چاپ «ناهمخوان» علامت می‌خورد. | محتوای یک گزارش سالانه ممکن است صدها KB باشد. |
| S8 | **بودجه‌ی index:** فقط index هایی که در ب۶.۷ آمده‌اند. هر index تازه باید در PR دلیل داشته باشد (کدام کوئری، چند سطر). | هر index یک کپی از ستون‌هاست. |
| S9 | **hash = `BLOB` ۳۲ بایتی** (نه hex ۶۴ نویسه) و فقط روی سر ثبت، نه روی هر سطر. | |
| S10 | **ستون‌های audit سبک:** فقط `created_at` و `created_by` (INTEGER). تاریخچه‌ی تغییر در `audit_log` موجود است. ستون‌های `updated_*` و `version` فقط روی داده‌ی پایه‌ی ویرایش‌پذیر (حساب، تأمین‌کننده، کارمند، قاعده‌ی جبران). | |
| S11 | **جدول‌ها `STRICT`.** `WITHOUT ROWID` برای جدول‌های سطری با کلید ترکیبی (`fin_line`، خطوط فاکتور، بل، سند پرداخت، معاش، سهم، اقساط) و `fin_series`. این کار یک B-tree و یک ستون rowid را در هر سطر حذف می‌کند. | |

**تخمین حجم** (یک سال کلینیک پرکار: ۶٬۰۰۰ فاکتور، ۸٬۰۰۰ رسید):
* ثبت روزنامه: حدود ۲۰٬۰۰۰ سر ثبت × ~۶۰ بایت، و حدود ۶۰٬۰۰۰ سطر × ~۲۵ بایت = حدود ۲٫۷ MB با index ها.
* اسناد: کمتر از ۲ MB.
* برای چند ده بیمار: چند ده KB.
* آزمون حجم در CI (د۲) این را می‌پاید.

### ب۶.۱ کدهای عددی (در `shared/src/fin.rs`)
| ستون | کدها |
|---|---|
| `account.type` | 1 asset · 2 liability · 3 equity · 4 income · 5 contra_income · 6 expense |
| `account.party_kind`، `line.party_kind` | 0 none · 1 patient · 2 supplier · 3 employee · 4 provider · 5 owner |
| `account.money_kind` | 0 none · 1 cash · 2 bank · 3 mobile |
| `doc_kind` (نوع سند؛ همان پیشوند شماره) | 1 INV · 2 RC · 3 AL · 4 RF · 5 CN · 6 WO · 7 PP · 8 BL · 9 PV · 10 OR · 11 PR · 12 DS · 13 TR · 14 CC · 15 JV · 16 OB · 17 YC · 18 RPT |
| `status` (همه‌ی اسناد) | 0 draft · 1 posted · 2 void (· 3 superseded فقط برنامه‌ی اقساط) |
| `period.status` | 0 open · 1 closed · 2 locked |
| `credit_note.kind` | 1 reduction · 2 write_off |
| `voucher.purpose` | 1 expense · 2 bill · 3 salary · 4 advance · 5 provider_share · 6 tax_remittance · 7 owner_drawing · 8 loan_repayment · 9 other |
| `withholding_kind` / `tax_kind` | 0 none · 1 wage · 2 rent · 3 contractor |
| `other_receipt.source` | 1 capital · 2 loan · 3 sale · 4 other |
| `supplier.kind` | 1 supplier · 2 lab · 3 landlord · 4 service · 5 other |
| `comp.model` | 0 none · 1 salary · 2 percent · 3 salary_percent · 4 partner |
| `comp_rule.kind` / `basis` | 1 percent · 2 fixed / 1 production · 2 collection |
| `legal_form` / `tax_regime` | 1 natural_person · 2 partnership · 3 llc · 4 corporation / 0 unknown · 1 fixed · 2 income_brt |
| `plan.frequency` | 1 monthly · 2 custom |

### ب۶.۲ هسته‌ی حسابداری
```sql
CREATE TABLE fin_account (
  id INTEGER PRIMARY KEY, code INTEGER NOT NULL UNIQUE,        -- 1110
  name_fa TEXT NOT NULL, name_ps TEXT NOT NULL, name_en TEXT NOT NULL,
  official_label TEXT,                                          -- برچسب در گزارش رسمی (اختیاری)
  type INTEGER NOT NULL CHECK (type BETWEEN 1 AND 6),
  parent_id INTEGER REFERENCES fin_account(id),
  is_control INTEGER NOT NULL DEFAULT 0, party_kind INTEGER NOT NULL DEFAULT 0,
  money_kind INTEGER NOT NULL DEFAULT 0, bank_name TEXT, bank_account_no TEXT,
  flags INTEGER NOT NULL DEFAULT 1,                             -- bit0 active · bit1 system
  sort INTEGER NOT NULL DEFAULT 0, updated_at INTEGER, updated_by TEXT, version INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE fin_period (
  id INTEGER PRIMARY KEY,                                       -- = shamsi_year*100 + month (140508)
  from_date INTEGER NOT NULL, to_date INTEGER NOT NULL,
  status INTEGER NOT NULL DEFAULT 0 CHECK (status BETWEEN 0 AND 2),
  closed_at INTEGER, closed_by TEXT, locked_at INTEGER, locked_by TEXT, note TEXT
) STRICT;                                                       -- ربع و سال از id حساب می‌شود

CREATE TABLE fin_series (
  doc_kind INTEGER NOT NULL, shamsi_year INTEGER NOT NULL, last_serial INTEGER NOT NULL,
  PRIMARY KEY (doc_kind, shamsi_year)) STRICT, WITHOUT ROWID;

CREATE TABLE fin_entry (                                        -- سر ثبت روزنامه
  id INTEGER PRIMARY KEY,                                       -- ترتیب ثبت = ترتیب زنجیره‌ی hash
  series_year INTEGER NOT NULL, serial INTEGER NOT NULL,        -- JV-1405-000001
  entry_date INTEGER NOT NULL,
  doc_kind INTEGER NOT NULL, doc_id INTEGER,                    -- سند منبع
  reverses_id INTEGER REFERENCES fin_entry(id),
  memo TEXT, hash BLOB NOT NULL,                                -- sha256(hash قبلی ‖ محتوای ثبت)
  created_at INTEGER NOT NULL, created_by TEXT,
  UNIQUE (series_year, serial)) STRICT;

CREATE TABLE fin_line (                                         -- سطر ثبت
  entry_id INTEGER NOT NULL REFERENCES fin_entry(id),
  line_no INTEGER NOT NULL,
  account_id INTEGER NOT NULL REFERENCES fin_account(id),
  amount INTEGER NOT NULL CHECK (amount <> 0),                  -- مثبت = بدهکار، منفی = بستانکار
  party_kind INTEGER NOT NULL DEFAULT 0, party_id TEXT,
  provider_id TEXT, service_id TEXT,
  PRIMARY KEY (entry_id, line_no)) STRICT, WITHOUT ROWID;
-- trigger: هیچ UPDATE/DELETE روی fin_entry و fin_line.
```
* **یک ستون علامت‌دار به‌جای دو ستون بدهکار و بستانکار** (S1/S5): توازن یعنی `SUM(amount) = 0` برای هر ثبت.
* بدهکار و بستانکار فقط در نمایش و چاپ جدا می‌شوند.
* شرح سطر ذخیره نمی‌شود؛ از سند منبع ساخته می‌شود.

### ب۶.۳ طرف‌های حساب و جبران
```sql
-- بیمار و داکتر و کاربر: از جدول‌های اصلی برنامه (party_id = شناسه‌ی آنجا).
CREATE TABLE fin_supplier (
  id TEXT PRIMARY KEY, kind INTEGER NOT NULL CHECK (kind BETWEEN 1 AND 5), name TEXT NOT NULL,
  tin TEXT, license_no TEXT, contact TEXT, phone TEXT, address TEXT,
  default_account_id INTEGER REFERENCES fin_account(id), flags INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL, updated_at INTEGER, version INTEGER NOT NULL DEFAULT 1) STRICT;
CREATE TABLE fin_employee (
  id TEXT PRIMARY KEY, full_name TEXT NOT NULL, father_name TEXT, national_id TEXT, tin TEXT,
  phone TEXT, position TEXT NOT NULL, provider_id TEXT, user_id TEXT,
  hire_date INTEGER NOT NULL, end_date INTEGER, flags INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL, updated_at INTEGER, version INTEGER NOT NULL DEFAULT 1) STRICT;
CREATE TABLE fin_comp_plan (                                    -- نسخه‌دار با تاریخ
  id INTEGER PRIMARY KEY, party_kind INTEGER NOT NULL CHECK (party_kind IN (3,4)), party_id TEXT NOT NULL,
  model INTEGER NOT NULL CHECK (model BETWEEN 0 AND 4), base_salary INTEGER NOT NULL DEFAULT 0,
  valid_from INTEGER NOT NULL, valid_to INTEGER, created_at INTEGER NOT NULL, created_by TEXT) STRICT;
CREATE TABLE fin_comp_rule (
  id INTEGER PRIMARY KEY, plan_id INTEGER NOT NULL REFERENCES fin_comp_plan(id), service_id TEXT,
  kind INTEGER NOT NULL, value INTEGER NOT NULL CHECK (value >= 0), basis INTEGER NOT NULL) STRICT;
```

### ب۶.۴ بیماران: فروش، دریافت، طلب
```sql
CREATE TABLE fin_invoice (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  patient_id TEXT NOT NULL, doc_date INTEGER NOT NULL, status INTEGER NOT NULL DEFAULT 1,
  net INTEGER NOT NULL CHECK (net >= 0),                        -- منجمد؛ = Σ خطوط
  note TEXT, void_reason TEXT, voided_at INTEGER, voided_by TEXT, print_count INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_invoice_line (
  invoice_id INTEGER NOT NULL REFERENCES fin_invoice(id), line_no INTEGER NOT NULL,
  treatment_item_id TEXT, service_id TEXT, name TEXT NOT NULL,   -- نام منجمد، یک زبان
  teeth TEXT, provider_id TEXT, quantity INTEGER NOT NULL CHECK (quantity BETWEEN 1 AND 1000),
  unit_price INTEGER NOT NULL, list_price INTEGER NOT NULL,     -- list ≠ unit = تغییر قیمت (audit)
  discount INTEGER NOT NULL DEFAULT 0,                          -- تخفیف خط + سهم خط از تخفیف کل
  revenue_account_id INTEGER NOT NULL,
  PRIMARY KEY (invoice_id, line_no)) STRICT, WITHOUT ROWID;
-- ناخالص خط = unit_price × quantity · خالص خط = ناخالص − discount (محاسبه، نه ذخیره)

CREATE TABLE fin_receipt (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  patient_id TEXT NOT NULL, doc_date INTEGER NOT NULL, money_account_id INTEGER NOT NULL,
  amount INTEGER NOT NULL CHECK (amount > 0), payer_name TEXT, note TEXT,  -- note شامل یادداشت ارزی
  status INTEGER NOT NULL DEFAULT 1, void_reason TEXT, print_count INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_allocation (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  patient_id TEXT NOT NULL, doc_date INTEGER NOT NULL,
  receipt_id INTEGER,                                           -- NULL = از پیش‌پرداخت
  invoice_id INTEGER NOT NULL, amount INTEGER NOT NULL CHECK (amount > 0),
  status INTEGER NOT NULL DEFAULT 1, reason TEXT, created_at INTEGER NOT NULL, created_by TEXT,
  UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_refund (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  patient_id TEXT NOT NULL, doc_date INTEGER NOT NULL, money_account_id INTEGER NOT NULL,
  amount INTEGER NOT NULL CHECK (amount > 0), reason TEXT NOT NULL, status INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_credit_note (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  kind INTEGER NOT NULL CHECK (kind IN (1,2)), invoice_id INTEGER NOT NULL, doc_date INTEGER NOT NULL,
  amount INTEGER NOT NULL CHECK (amount > 0), reason TEXT NOT NULL, status INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_plan (                                         -- برنامه‌ی اقساط
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  patient_id TEXT NOT NULL, version INTEGER NOT NULL DEFAULT 1, replaces_id INTEGER,
  status INTEGER NOT NULL DEFAULT 1, total INTEGER NOT NULL, frequency INTEGER NOT NULL,
  reason TEXT, created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial, version)) STRICT;
CREATE TABLE fin_plan_invoice (plan_id INTEGER NOT NULL, invoice_id INTEGER NOT NULL,
  PRIMARY KEY (plan_id, invoice_id)) STRICT, WITHOUT ROWID;
CREATE TABLE fin_plan_item (plan_id INTEGER NOT NULL, no INTEGER NOT NULL, due_date INTEGER NOT NULL,
  amount INTEGER NOT NULL CHECK (amount > 0), PRIMARY KEY (plan_id, no)) STRICT, WITHOUT ROWID;
```
**قواعد محاسبه** (هیچ‌کدام ذخیره نمی‌شود):
* مانده‌ی فاکتور = `net` − Σتخصیص‌ها − Σیادداشت‌ها.
* پیش‌پرداخت بیمار = مانده‌ی بیمار در حساب 2210.
* وضعیت هر قسط = تخصیص‌ها به ترتیب FIFO.

آزمون‌ها برابری این‌ها با حساب‌های کنترل را ثابت می‌کنند.

### ب۶.۵ خرید، مصرف، پرداخت، دریافت دیگر
```sql
CREATE TABLE fin_bill (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  supplier_id TEXT NOT NULL, doc_date INTEGER NOT NULL, due_date INTEGER, vendor_ref TEXT,
  status INTEGER NOT NULL DEFAULT 1, void_reason TEXT, created_at INTEGER NOT NULL, created_by TEXT,
  UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_bill_line (bill_id INTEGER NOT NULL, line_no INTEGER NOT NULL, account_id INTEGER NOT NULL,
  amount INTEGER NOT NULL CHECK (amount > 0), description TEXT, stock_receipt_id INTEGER,
  PRIMARY KEY (bill_id, line_no)) STRICT, WITHOUT ROWID;
CREATE TABLE fin_voucher (                                      -- سند پرداخت (هر خروج پول)
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  doc_date INTEGER NOT NULL, money_account_id INTEGER NOT NULL, purpose INTEGER NOT NULL,
  party_kind INTEGER NOT NULL DEFAULT 0, party_id TEXT, payee_name TEXT, reference TEXT,
  status INTEGER NOT NULL DEFAULT 1, void_reason TEXT, created_at INTEGER NOT NULL, created_by TEXT,
  UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_voucher_line (voucher_id INTEGER NOT NULL, line_no INTEGER NOT NULL,
  account_id INTEGER NOT NULL, ref_kind INTEGER, ref_id INTEGER,  -- بل / معاش / صورت سهم
  amount INTEGER NOT NULL CHECK (amount > 0),                   -- ناخالص
  withholding_kind INTEGER NOT NULL DEFAULT 0, withholding INTEGER NOT NULL DEFAULT 0,  -- دستی
  tax_period INTEGER,                                           -- برای واریز مالیات: 140508
  description TEXT, PRIMARY KEY (voucher_id, line_no)) STRICT, WITHOUT ROWID;
-- خالص پرداخت = Σ(amount − withholding)
CREATE TABLE fin_other_receipt (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL, doc_date INTEGER NOT NULL,
  money_account_id INTEGER NOT NULL, source INTEGER NOT NULL, account_id INTEGER NOT NULL,
  amount INTEGER NOT NULL CHECK (amount > 0), payer_name TEXT, note TEXT, status INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial)) STRICT;
```

### ب۶.۶ معاش، سهم داکتر، صندوق، گزارش، پروفایل
```sql
CREATE TABLE fin_payroll (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  period_id INTEGER NOT NULL,                                   -- 140508
  doc_date INTEGER NOT NULL, status INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial)) STRICT;
-- قاعده در Rust: یک posted برای هر ماه.
CREATE TABLE fin_payroll_line (payroll_id INTEGER NOT NULL, employee_id TEXT NOT NULL,
  days_worked INTEGER, base INTEGER NOT NULL, allowances INTEGER NOT NULL DEFAULT 0,
  overtime INTEGER NOT NULL DEFAULT 0, commission INTEGER NOT NULL DEFAULT 0,
  advance_deduction INTEGER NOT NULL DEFAULT 0, other_deduction INTEGER NOT NULL DEFAULT 0,
  wage_tax_withheld INTEGER NOT NULL DEFAULT 0,                 -- دستی
  note TEXT, PRIMARY KEY (payroll_id, employee_id)) STRICT, WITHOUT ROWID;
-- ناخالص و خالص محاسبه می‌شوند؛ CHECK در Rust: خالص ≥ 0.
CREATE TABLE fin_share (                                        -- صورت سهم داکتر
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  provider_id TEXT NOT NULL, from_date INTEGER NOT NULL, to_date INTEGER NOT NULL, basis INTEGER NOT NULL,
  base_total INTEGER NOT NULL, share_total INTEGER NOT NULL, status INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_share_line (share_id INTEGER NOT NULL, line_no INTEGER NOT NULL,
  invoice_id INTEGER NOT NULL, invoice_line_no INTEGER NOT NULL, base INTEGER NOT NULL, share INTEGER NOT NULL,
  PRIMARY KEY (share_id, line_no)) STRICT, WITHOUT ROWID;      -- منجمد؛ JSON نیست
CREATE TABLE fin_transfer (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL, doc_date INTEGER NOT NULL,
  from_account_id INTEGER NOT NULL, to_account_id INTEGER NOT NULL, amount INTEGER NOT NULL CHECK (amount > 0),
  reference TEXT, status INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL, created_by TEXT,
  UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_cash_count (
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL,
  money_account_id INTEGER NOT NULL, doc_date INTEGER NOT NULL, book_balance INTEGER NOT NULL,
  counted INTEGER NOT NULL CHECK (counted >= 0), reason TEXT,   -- اجباری اگر اختلاف ≠ 0
  created_at INTEGER NOT NULL, created_by TEXT, UNIQUE (series_year, serial)) STRICT;
CREATE TABLE fin_report_issue (                                 -- گزارش رسمی صادرشده (S7)
  id INTEGER PRIMARY KEY, series_year INTEGER NOT NULL, serial INTEGER NOT NULL, kind INTEGER NOT NULL,
  params TEXT NOT NULL, basis INTEGER NOT NULL, period_status INTEGER NOT NULL, content_hash BLOB NOT NULL,
  print_count INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL, created_by TEXT,
  UNIQUE (series_year, serial)) STRICT;
-- پروفایل حقوقی: کلیدهای setting (یک‌سطری): fin.legal_name، fin.legal_form، fin.tax_regime، fin.tin،
-- fin.business_license_no، fin.health_license_no، fin.tax_office، fin.owner_name، fin.owner_father_name،
-- fin.official_address، fin.basis_override، fin.go_live_date.
```

### ب۶.۷ بودجه‌ی index (همه‌ی index های بخش مالی)
همه‌ی index ها به‌جز کلیدهای اصلی و `UNIQUE(series_year, serial)` هر سند:

| index | برای کدام کوئری |
|---|---|
| `fin_line(account_id, entry_id)` | دفتر کل، مانده‌ی حساب، ترازنامه |
| `fin_line(party_kind, party_id) WHERE party_id IS NOT NULL` | زیر-دفتر بیمار، تأمین‌کننده، کارمند، داکتر |
| `fin_entry(entry_date)` | گزارش‌های بازه‌ای و نگهبان دوره |
| `fin_entry(doc_kind, doc_id)` | پیدا کردن ثبتِ یک سند (و یکتایی ثبت اصلی، در Rust) |
| `fin_invoice(patient_id)` · `fin_receipt(patient_id)` · `fin_allocation(invoice_id)` | حساب بیمار، مانده‌ی فاکتور |
| `fin_plan_item(due_date)` | سررسیدها و یادآوری |

**جمع: ۸ index اضافه.** گزارش‌های تاریخی همه از `fin_entry(entry_date)` و جدول‌های کوچک می‌خوانند. `fin_line` بدون index تاریخ است، چون از راه `entry_id` به سر ثبت وصل می‌شود.

## ب۷. دوره، قفل و شماره
* **ماه شمسی = یک دوره.** دوره‌ها خودکار ساخته می‌شوند.
  * `open`: ثبت آزاد است.
  * `closed`: فقط کاربر با دسترسی `finance.close` (با دلیل) ثبت می‌کند. ماه را برای گزارش ماهانه می‌بندند.
  * `locked`: هیچ ثبت، ابطال یا تعدیلی ممکن نیست. ربع را پس از تسلیم اظهارنامه قفل می‌کنند و سال را پس از اظهارنامه‌ی سالانه. بازکردن قفل فقط مالک، با دلیل، و در audit.
* **اصلاح خطای دوره‌ی قفل‌شده:** سند اصلی دست نمی‌خورد و ابطال هم نمی‌شود. یک سند اصلاحی در اولین دوره‌ی باز ثبت می‌شود، با ارجاع به سند اصلی. گزارش «ابطال‌ها و اصلاحات» آن را نشان می‌دهد.
* **تاریخ سند:**
  * رسید، برگشت، فاکتور، تخصیص: همیشه امروز.
  * مصرف، بل، معاش، انتقال و سند پرداخت: می‌توانند گذشته باشند، ولی نه در دوره‌ی بسته یا قفل، و نه بیش از ۳۶۵ روز. آینده ممنوع است.
* **شماره:** `PREFIX-<سال شمسیِ doc_date>-NNNNNN` از `fin_series`، داخل همان تراکنش. باطل‌شده‌ها شماره را نگه می‌دارند. گزارش R16 فاصله‌ها را آشکار می‌کند (باید خالی باشد).

## ب۸. صفحه‌های بخش «مالی» (یک بخش، یک ساختار)
```
مالی
├─ نمای کلی و تطبیق        انتخاب ماه/ربع/سال؛ کارت‌ها؛ جدول تطبیق (ب۱۰)؛ هشدارها
├─ عواید  ← جدول اصلی (نظر استاد)
│   ├─ دریافت‌ها            رسیدها، برگشت‌ها، تخصیص‌ها                 [نقدی]
│   ├─ فاکتورها             فاکتورها، یادداشت‌ها، بخشش‌ها             [تعهدی]
│   ├─ اقساط                برنامه‌ها، سررسیدها، عقب‌افتاده‌ها
│   ├─ طلب‌ها               سن بدهی هر بیمار (as-of)
│   ├─ پیش‌پرداخت‌ها         مانده‌ی امانت هر بیمار
│   └─ تخفیف و اصلاح        تخفیف، یادداشت کاهش، بخشش، برگشت (با دلیل)
├─ مصارف و تأمین‌کنندگان
│   ├─ مصارف                به حساب مصرف (دسته)
│   ├─ بل‌ها و بدهی‌ها       بل‌های باز، سن بدهی تأمین‌کننده
│   └─ کسرهای تکلیفی        کرایه و قراردادی: کسرشده، واریزشده، مانده
├─ کارمندان و معاش          کارمندان، قرارداد معاش، لیست ماهانه، مساعده، مالیات کسرشده
├─ سهم داکتران              قاعده‌ها، پیش‌نمایش ماه، صورت‌های تأییدشده، پرداخت، مانده
├─ صندوق و بانک             حساب‌ها با مانده، دفتر هر حساب، شمارش روز، انتقال
├─ دوره‌ها                  ۱۲ ماه هر سال، بستن/قفل، بستن سال، بسته‌ی سالانه‌ی وزارت
├─ دفاتر (حسابدار)          روزنامه، دفتر کل، ترازنامه‌ی آزمایشی، ثبت تعدیلی، سلامت دفتر
├─ گزارش‌ها                 R1…R18 (ب۹) با صدور رسمی
└─ تنظیمات                  پروفایل حقوقی، چارت حساب، حساب‌های پول، سری شماره‌ها، دسترسی
```
**صفحه‌ی بیمار** («حساب مالی» در پرونده):
* بالای صفحه: صورت‌حساب بیمار (فاکتورها، دریافت‌ها، تخصیص‌ها، مانده، پیش‌پرداخت، اقساط).
* دکمه‌ها: «فاکتور از درمان‌ها»، «دریافت»، «برنامه‌ی اقساط»، «برگشت».
* همه با همان دیالوگ‌های بخش مالی.

**جدول اصلی «عواید ← دریافت‌ها»:**
* ستون‌ها: ردیف، تاریخ، شماره (RC، RF)، بیمار (شماره پرونده و نام)، نوع (دریافت، برگشت)، حساب پول، **دریافت**، **برگشت**، تخصیص‌یافته به (فاکتورها)، به پیش‌پرداخت، ثبت‌کننده.
* جمع پایین ثابت (sticky): دریافت، برگشت، خالص، تعداد.
* فیلترها: بازه، حساب پول، داکتر، کاربر.

**«عواید ← فاکتورها»:** تاریخ، INV، بیمار، داکتر(ها)، ناخالص، تخفیف، یادداشت، بخشش، **خالص**، تخصیص‌یافته، **مانده**، وضعیت، قسط‌دار.

**اصول بصری** (در فاز UI با ui-ux-pro-max و impeccable اجرا شود):
* جدول متراکم، با اعداد هم‌عرض و راست‌چین.
* برچسب مبنا («نقدی» یا «تعهدی») همیشه کنار عنوان.
* رنگ فقط برای وضعیت: باطل خاکستری و خط‌خورده، عقب‌افتاده کهربایی، قفل با نماد قفل.
* زیربخش‌ها به‌صورت نوار فرعی؛ بدون شبکه‌ی کارت‌های تکراری.
* هر عدد کلیک‌خور است و به جزئیات می‌رود.

## ب۹. گزارش‌ها
### ب۹.۱ قالب چاپ رسمی
```
┌──────────────────────────────────────────────────────────────────┐
│ [لوگو] نام حقوقی کلینیک                        TIN: ………          │
│        نام تجارتی · جواز صحت: … · جواز تجارت: … · دفتر مالیاتی: …  │
│        آدرس رسمی · تلفن                                          │
│ دفتر دریافت‌ها — ربع دوم ۱۴۰۵ (سرطان–سنبله)                       │
│ دوره: ۱۴۰۵/۰۴/۰۱–۱۴۰۵/۰۶/۳۱ · مبنا: نقدی · ارز: افغانی            │
│ شماره: RPT-1405-000012 · وضعیت دوره: قفل‌شده                      │
├──────────────────────────────────────────────────────────────────┤
│ جدول با ردیف · «نقل از صفحه‌ی قبل» · جمع صفحه · «نقل به صفحه‌ی بعد» │
├──────────────────────────────────────────────────────────────────┤
│ تهیه‌کننده: …… امضا ……   تأییدکننده (مالک): …… امضا ……   [مهر]     │
│ تهیه: ۱۴۰۵/۰۷/۱۰ ۱۰:۲۲ · Artaveo Dental x.y · کد: 3f9a…c2  صفحه ۱ از ۴│
└──────────────────────────────────────────────────────────────────┘
```
* **صفحه:** A4 عمودی، و افقی برای جدول‌های پهن.
* **عدد و تاریخ:** ارقام فارسی برای دری و پشتو. جداکننده‌ی هزار. بدون اعشار وقتی `.00` است. تاریخ شمسی.
* **Excel:** برگ داده (عدد واقعی، بدون ادغام سلول) + برگ سربرگ + برگ جمع‌ها. نام فایل = شماره‌ی گزارش.
* **PDF:** همان قالب چاپ؛ متادیتا = شماره و hash.
* **زبان:** پیش‌فرض دری؛ پشتو و انگلیسی قابل انتخاب.
* **صدور رسمی:** پارامترها و hash محتوا در `fin_report_issue` ذخیره می‌شود. چاپ دوباره گزارش را از دفتر می‌سازد و hash را مقایسه می‌کند (S7). اگر دوره هنوز باز است، واترمارک «پیش‌نویس – دوره باز» دارد.

### ب۹.۲ مجموعه‌ی گزارش‌ها (R = رسمی برای مالیه · M = مدیریتی)
| کد | نام | مبنا | ستون‌ها | جمع و کنترل |
|---|---|---|---|---|
| R1 | دفتر دریافت‌ها | نقدی | ردیف، تاریخ، شماره، پرداخت‌کننده، پرونده، فاکتور(ها)، حساب پول، دریافت، برگشت | ماه، ربع، کل · «از شماره‌ی … تا …، n سند، بدون فاصله» |
| R2 | خلاصه‌ی دریافت ربعوار | نقدی | ۳ ماه: دریافت ناخالص، برگشت، خالص، تعداد رسید، تعداد بیمار | جمع ربع (**بدون هیچ ستون مالیات**) |
| R3 | صورت عواید و مصارف سالانه | نقدی و تعهدی (دو ستون) | عواید (خدمات، سایر، −تخفیف، −کاهش)؛ مصارف به حساب؛ معاش؛ سهم داکتر؛ بخشش (فقط تعهدی)؛ خالص؛ مقایسه با سال قبل | پل بین دو مبنا |
| R4 | دفتر روزنامه | — | JV، تاریخ، سند منبع، حساب، طرف، شرح، بدهکار، بستانکار | Σبدهکار = Σبستانکار |
| R5 | دفتر کل | — | برای هر حساب: مانده‌ی اول، سطرها، مانده‌ی جاری، مانده‌ی آخر | |
| R6 | ترازنامه‌ی آزمایشی | — | کد، حساب، مانده‌ی اول، گردش بدهکار، گردش بستانکار، مانده‌ی آخر | برابر |
| R7 | دفتر مصارف | نقدی و تعهدی | سند (PV، BL)، تاریخ، حساب، شرح، پرداخت‌شونده، TIN پرداخت‌شونده، بل فروشنده، ناخالص، کسر تکلیفی، خالص، حساب پول | به حساب مصرف |
| R8 | دفتر معاش (ماهانه) + صورت سالانه‌ی هر کارمند | — | کد، نام، نام پدر، TIN، پایه، اضافه، کمیسیون، ناخالص، مساعده، سایر کسر، مالیات کسرشده (دستی)، خالص، پرداخت‌شده | ماده ۴۶(۳)، ۶۱(۲) |
| R9 | دفتر کسرهای تکلیفی | — | ماه، نوع (معاش، کرایه، قراردادی)، طرف، مبنا (ناخالص)، کسرشده، واریزشده (PV و شماره‌ی آویز)، تاریخ، مانده | = 2410 تا 2430 |
| R10 | دفتر فاکتورها | تعهدی | فاکتورها با خالص، تخصیص، مانده؛ باطل‌ها با دلیل | |
| R11 | سن طلب بیماران (as-of) | — | بیمار، تلفن، ۰–۳۰، ۳۱–۶۰، ۶۱–۹۰، ۹۰+، جمع، آخرین دریافت | = 1310 |
| R12 | اقساط و عقب‌افتاده‌ها | — | بیمار، برنامه، قسط n از m، سررسید، مبلغ، پرداخت، مانده، روز تأخیر | |
| R13 | پیش‌پرداخت بیماران | — | بیمار، دریافت، تخصیص، برگشت، مانده | = 2210 |
| R14 | سن بدهی به تأمین‌کنندگان | — | تأمین‌کننده، بل‌های باز، سطل‌های سن | = 2110 |
| R15 | دفتر صندوق و دفتر بانک | — | تاریخ، سند، شرح، ورود، خروج، مانده‌ی جاری؛ شمارش‌ها | مانده = حساب |
| R16 | پیوستگی شماره‌ها | — | پیشوند، سال، اولین، آخرین، تعداد، فاصله‌ها، باطل‌ها | |
| R17 | ابطال‌ها و اصلاحات | — | سند، سند معکوس یا اصلاحی، مبلغ، دلیل، کاربر، تاریخ | |
| R18 | بیلانس | تعهدی | دارایی، بدهی، سرمایه | دارایی = بدهی + سرمایه |
| M1 | تولید و وصول (روزانه، ماهانه)، نرخ وصول | | | |
| M2 | تولید هر داکتر · عاید هر خدمت | تعهدی | | |
| M3 | صورت سهم داکتر (برای خود داکتر) | | | |
| M4 | Day sheet: صفحه‌ی روزانه‌ی فاکتور، دریافت و تعدیل‌ها | | | |

**بسته‌ی سالانه برای وزارت** (در «دوره‌ها» ← سال):
* محتوا: R3، چهار نسخه‌ی R2، R1، R7، R8 سالانه، R9، R11، R14، R16، و R18 اگر کلینیک شرکت است.
* در یک PDF، با فهرست و شماره‌های رسمی پیاپی.
* اگر ماه بازی باقی مانده باشد، واترمارک «پیش‌نویس» می‌خورد.

## ب۱۰. تطبیق (صفحه‌ی «نمای کلی»؛ همیشه باید «✔» باشد)
```
                                                   ماه: عقرب ۱۴۰۵
A  دریافت از بیماران                                 ۱۷۹٬۷۰۰   = R1
B  − برگشت به بیماران                                  (۲٬۰۰۰)   = R1
C  = خالص دریافت                                    ۱۷۷٬۷۰۰
D  − مصارف پرداخت‌شده (به حساب)                       (۲۰٬۰۵۰)  = R7 نقدی
E  − معاش، مساعده و سهم داکتر پرداخت‌شده                 (…)      = R8، سهم
F  − پرداخت بل‌ها                                        (…)      = R14
G  ± واریز مالیات، برداشت، سرمایه، قرض (جدا)             (…)
H  = تغییر مانده‌ی صندوق‌ها و بانک‌ها                   ✔ = Σ(آخر − اول) در R15
I  اختلاف شمارش صندوق                                   −۵۰ (۲۲ روز شمرده‌شده از ۲۶)

تعهدی: عاید خالص − مصارف تعهدی = سود تعهدی                   = R3
پل: عاید تعهدی − Δطلب + Δپیش‌پرداخت − بخشش = C               ✔
زیر-دفترها: Σمانده‌ی فاکتورها = 1310 ✔ · Σامانت = 2210 ✔ · Σبل‌های باز = 2110 ✔
            Σمعاش پرداخت‌نشده = 2310 ✔ · Σسهم پرداخت‌نشده = 2320 ✔ · کسرها = 2410..2430 ✔
دفتر: Σبدهکار = Σبستانکار ✔ · زنجیره‌ی hash ✔ · شماره‌ها بدون فاصله ✔
```

## ب۱۱. قواعد درستی
1. **تغییرناپذیری:**
   * trigger ضد UPDATE و DELETE روی `fin_entry` و `fin_line`. روی `fin_report_issue` فقط `print_count` قابل تغییر است.
   * روی اسناد فقط این تغییرها مجاز است: از `posted` به `void` (با `void_reason` و `voided_*`)، و ستون‌های `print_*`.
   * چاپ هر سند (مثل رسید) از سطرهای تغییرناپذیر ساخته می‌شود (S6). هیچ snapshot یا UPDATE بعدی لازم نیست.
2. **ابطال = ثبت معکوس** با `reverses_id`.
   * ابطال رسیدِ تخصیص‌یافته اول تخصیص‌های آن را باطل می‌کند.
   * ابطال فاکتورِ دارای تخصیص ممنوع است. اول تخصیص‌ها لغو می‌شوند و پول به پیش‌پرداخت برمی‌گردد.
3. **نگهبان دوره:** یک تابع واحد، `periods::guard(date, op)`، اول هر نوشتن مالی اجرا می‌شود.
4. **گرد کردن:**
   * فقط عدد صحیح پول ذخیره می‌شود.
   * هر تقسیم (تخفیف کل بین خطوط، سهم داکتر، اقساط مساوی) با روش **بزرگ‌ترین باقی‌مانده** انجام می‌شود تا جمع اجزا دقیقاً برابر کل باشد.
   * اقساط ماهانه روی **همان روز ماه شمسی** می‌افتند (برای ماه کوتاه‌تر، آخرین روز ماه).
5. **فقط افغانی.** `foreign_note` فقط متن است.
6. **audit:** همه‌ی اسناد، تغییر وضعیت دوره، صدور گزارش رسمی، تغییر چارت حساب و دسترسی‌ها.
7. **سلامت دفتر:** هر شب و با دکمه بررسی می‌شود:
   * توازن هر ثبت؛
   * زنجیره‌ی hash؛
   * برابری هر زیر-دفتر با حساب کنترلش؛
   * هر سند دقیقاً یک ثبت اصلی دارد؛
   * شماره‌ها فاصله ندارند.

   خطاها در مرکز اعلان‌ها نشان داده می‌شوند.
8. **نگهداری:** هیچ پاک‌سازی خودکار؛ حداقل ۵ سال.

## ب۱۲. دسترسی (permission)
| کد | مالک | حسابدار | منشی/صندوق‌دار | داکتر |
|---|---|---|---|---|
| `finance.view` | ✔ | ✔ | فقط عواید و بیمار | فقط سهم خود |
| `finance.receive` (فاکتور، رسید، تخصیص، اقساط) | ✔ | ✔ | ✔ | |
| `finance.adjust` (یادداشت، برگشت، ابطال) | ✔ | ✔ | | |
| `finance.write_off` | ✔ | | | |
| `finance.spend` (بل، سند پرداخت، انتقال) | ✔ | ✔ | (شمارش صندوق) | |
| `finance.payroll` | ✔ | ✔ | | |
| `finance.shares` (تأیید صورت سهم) | ✔ | پیش‌نویس | | |
| `finance.close` (بستن ماه) | ✔ | ✔ | | |
| `finance.lock` (قفل و بازکردن قفل) | ✔ | | | |
| `finance.ledger` (دفاتر، ثبت تعدیلی، چارت) | ✔ | ✔ | | |
| `finance.official_report` (صدور رسمی) | ✔ | ✔ | | |
| `finance.override_price` | ✔ | ✔ | | |

---

# ج. گذار از سیستم فعلی (جایگزینی، نه مهاجرت داده)

**تصمیم مالک (۱۴۰۵/۰۸/۲۷):**
* همه‌ی دیتابیس‌های موجود داده‌ی آزمایشی و دورریختنی‌اند.
* پس **مهاجرت داده، دروازه‌ی برابری و آرشیو قدیم ساخته نمی‌شود.**
* یک مهاجرت عادی جدول‌های مالی قدیم را حذف و جدول‌های تازه را می‌سازد. داده‌ی مالی قدیم دور ریخته می‌شود.
* از این به بعد فقط مهاجرت‌های نسخه‌دار معمولی است، برای حفاظت از مشتری‌های واقعی آینده (ج۳).

## ج۱. آنچه امروز هست و حذف می‌شود (v0.12.4)
| بخش | جدول، فایل، RPC امروز | چرا کنار می‌رود |
|---|---|---|
| فاکتور | `invoice`، `invoice_line` · `invoices.*` | تخفیف فقط روی کل فاکتور؛ «دریافت» و «تخصیص» یکی‌اند. |
| پرداخت | `payment(kind=payment\|credit\|refund)` · `payments.*` | سه مفهوم در یک جدول؛ snapshot با UPDATE. |
| یادداشت | `credit_note` | کاهش قیمت و بخشش بدهی جدا نیستند. |
| اقساط | `installment` | DELETE و بازنویسی؛ «ماهانه» = ۳۰ روز. |
| مصارف | `expense`، `expense_category` · `expenses.*` | بدون شماره؛ حذف نرم؛ بدون حساب پول، تأمین‌کننده یا کسر. |
| صندوق | `cash_closing` · `cash.*` | بدون مانده؛ یک صندوق. |
| کمیسیون | `commission_rule` + `billing.commission_*` · `commission.*` | تسویه ندارد. |
| تأمین‌کننده | `supplier_payment` (که یک `expense` می‌سازد) · `supplier_payments.*`، `supplier_ledger.list` | بدهی تأمین‌کننده خارج از دفتر است. |
| روش پرداخت | `payment_method` · `payment_methods.*` | به حساب پول وصل نیست. |
| خلاصه | `billing.summary/account/debtors/statement/installments_due/settings_*` | |
| گزارش | ۱۲ گزارش مالی در `core/src/reports.rs` | «سود» بی‌برچسب. |
| کد | `core/src/billing.rs` (کل فایل) | |
| UI | `ui/src/pages/billing/*`، `ui/src/lib/billing.ts` (+ تست)، `ui/src/print/{receipt,finance}.tsx` | |

## ج۲. مهاجرت جایگزینی (یک فایل SQL، بدون انتقال داده)
`core/migrations/00NN_finance_v2.sql`. شماره = اولین شماره‌ی آزاد هنگام merge (الان آخرین `0019_avatars` است).
مرحله‌ها، به همین ترتیب:
1. `DROP TABLE` به ترتیب وابستگی: `installment`، `credit_note`، `payment`، `invoice_line`، `invoice`، `cash_closing`، `commission_rule`، `supplier_payment`، `expense`، `expense_category`، `payment_method`.
2. `DELETE FROM setting WHERE key LIKE 'billing.%'`. ردیف‌های مالی `document_counter` (پیشوندهای `INV`، `RC`، `CU`، `RV`، `CN`، `SP`) حذف می‌شوند.
3. `UPDATE treatment_item SET billed_at = NULL`، چون فاکتورها رفته‌اند و درمان‌ها دوباره «قابل فاکتور» می‌شوند.
4. **تأمین‌کننده:**
   * جدول `supplier` به `fin_supplier` تبدیل می‌شود (ستون‌های `tin`، `license_no` و `kind` عددی).
   * ستون‌های `supplier_id` در `purchase`، `stock_batch`، `stock_movement` و `lab_case` به جدول تازه اشاره می‌کنند.
   * این کار با بازسازی جدول در SQLite انجام می‌شود: `PRAGMA foreign_keys=OFF` → جدول تازه → کپی → حذف → تغییر نام → `PRAGMA foreign_key_check`.
   * داده‌ی انبار (کالا و موجودی) **حفظ می‌شود**، چون انبار بخش مالی نیست.
5. **خرید انبار:**
   * `purchase` در ماژول انبار به‌عنوان «رسید انبار» می‌ماند (کالا، lot، انقضا).
   * ستون `total` آن دیگر مالی نیست. منبع مالی `fin_bill` است.
   * رسیدهای قدیم بل ندارند. این برای داده‌ی آزمایشی قبول است.
6. ساخت همه‌ی جدول‌های `fin_*` (ب۶)، trigger های ضد ویرایش، و seed چارت حساب (ب۳) با حساب‌های پول پیش‌فرض: صندوق 1110 و بانک 1210.
7. **پرمیشن‌ها:** `billing.*` و `reports.view` (بخش مالی) به `finance.*` تبدیل می‌شوند (ب۱۲)، با نگاشت نقش‌های موجود (مالک و حسابدار همه، منشی `finance.view/receive`).
8. پشتیبان پیش از مهاجرت را چارچوب `db.rs` خودکار می‌گیرد ✅. اگر مالک داده‌ی آزمایشی قدیم را بخواهد، همان‌جاست.

## ج۳. حفاظت از مشتری‌های واقعی از این به بعد
* هر تغییر schema مالی بعد از این یک **مهاجرت نسخه‌دار افزایشی** است: `ADD COLUMN`، جدول تازه، یا بازسازی جدول با کپی کامل داده. **هیچ DROP جدول دارای داده‌ی مالی.**
* هر مهاجرتی که به داده‌ی مالی دست می‌زند، در همان PR یک **آزمون مهاجرت** دارد:
  * دیتابیس fixture نسخه‌ی قبل با سناریوی استاندارد (د۲) ساخته می‌شود؛
  * مهاجرت اجرا می‌شود؛
  * ترازنامه‌ی آزمایشی، مانده‌ی همه‌ی حساب‌ها و R1 تا R3 پیش و پس از مهاجرت **عیناً برابر** است.
* `fin.health.check` (ب۱۱) بعد از هر مهاجرت خودکار اجرا می‌شود. اگر خطا بدهد، اعلان پشتیبانی می‌آید.
* پشتیبان پیش از مهاجرت (موجود) حفظ می‌شود.

## ج۴. RPCها و صفحه‌ها: حذف و جایگزین
### RPCها (قرارداد `shared`)
| حذف | جایگزین |
|---|---|
| `payment_methods.*` | `fin.accounts.list/save` (حساب‌های پول بخشی از چارت‌اند) |
| `invoices.*` | `fin.invoices.billable/list/get/create/void/print` · `fin.credit_notes.create/void` · `fin.plans.create/replace/get` |
| `payments.*` | `fin.receipts.create/void/list/get/print` · `fin.allocations.create/void` · `fin.refunds.create/void` |
| `billing.account/statement/debtors/installments_due` | `fin.patients.account/statement` · `fin.receivables.aging` · `fin.plans.due` |
| `billing.summary` | `fin.overview.get` |
| `billing.settings_*`، `commission.*` | `fin.comp.*` · `fin.shares.preview/create/post/void` · `fin.settings.*` |
| `expense_categories.*`، `expenses.*` | `fin.accounts.*` (حساب‌های مصرف) · `fin.vouchers.create/void/list/get` |
| `cash.*` | `fin.cash.counts.*` · `fin.transfers.*` · `fin.money.ledger` |
| `supplier_payments.*`، `supplier_ledger.list` | `fin.vouchers.*` (purpose=bill) · `fin.bills.*` · `fin.suppliers.ledger` |
| `suppliers.*` | `fin.suppliers.*` (انبار هم از همین استفاده می‌کند) |
| `purchases.create/void` | `inventory.receipts.create/void`؛ اختیاری در همان تراکنش `fin.bills.create` |
| ۱۲ گزارش مالی `reports.run` | `fin.reports.run/issue/reprint/export`. گزارش‌های بالینی و انبار در `reports.*` می‌مانند. |
| تازه | `fin.employees.*` · `fin.payroll.*` · `fin.periods.*` · `fin.ledger.journal/general/trial_balance/adjust` · `fin.legal.get/save` · `fin.health.check` · `fin.other_receipts.*` |

### صفحه‌ها
| امروز | سرنوشت |
|---|---|
| `ui/src/pages/billing/*` (BillingPage با ۷ زبانه، BillingExtras، dialogs، InvoiceDetail، common) | **حذف**. جایگزین `ui/src/pages/finance/*` با ساختار ب۸. |
| `PatientBilling.tsx` (تب مالی پرونده) | جایگزین `ui/src/pages/finance/PatientAccount.tsx`. |
| `ui/src/lib/billing.ts` (+ تست) | جایگزین `ui/src/lib/finance.ts`. |
| `ui/src/print/receipt.tsx`، `print/finance.tsx` | بازنویسی روی اسناد تازه + قالب رسمی (ب۹.۱). |
| انبار: SuppliersTab، PurchasesTab (پرداخت و مانده)، LabTab (هزینه) | پول و مانده در «مالی ← بل‌ها و تأمین‌کنندگان». انبار فقط کالا، رسید انبار و دکمه‌ی «ساخت بل». |
| ReportsPage (بخش مالی) | به «مالی ← گزارش‌ها» می‌رود. ReportsPage فقط بالینی و انبار می‌ماند. |
| Dashboard (درآمد امروز، مانده) | از `fin.overview` می‌خواند. |
| Users و نقش‌ها | پرمیشن‌های `finance.*`. |
| `PlansTab.tsx`، `PatientsPage.tsx`، `PatientProfile.tsx`، `Shell.tsx` (لینک‌ها و نشان‌های مالی) | فراخوانی‌ها به `fin.*`. |
| mock های dev (`mockCore.ts`، `mockInventory.ts`) | بازنویسی برای RPCهای تازه. |

---

# د. فازهای اجرا (هر فاز = انتشار کامل و آزموده)
> قاعده ۱۵: build و test فقط روی GitHub Actions. rustfmt از شاخه‌ی `fmt/*`. هر فاز یک PR است و `shared/ts/contract.ts` در آن بازسازی می‌شود.
> شروع فاز ۱ فقط بعد از دو چیز: گزارش `f-db-size` (و `docs/design/db-size-rules.md`)، و سبز شدن CI آواتارها.

| فاز | محتوا | در پایان فاز | اندازه |
|---|---|---|---|
| **۱. هسته + همه‌ی کارهای امروز روی مدل تازه** | مهاجرت جایگزینی (ج۲). `core/src/fin/`: دفتر و قواعد ثبت، حساب‌ها، سری شماره، دوره (همه open)، فاکتور، رسید، تخصیص، برگشت، یادداشت و بخشش، اقساط، طلب و سن بدهی، پیش‌پرداخت، تأمین‌کننده و بل، سند پرداخت، دریافت دیگر، صندوق و بانک، انتقال، شمارش صندوق، قاعده‌های سهم و پیش‌نمایش، پروفایل حقوقی، سلامت دفتر. UI: «مالی» با نمای کلی و تطبیق، عواید و ۶ زیربخش، مصارف و تأمین‌کنندگان، صندوق و بانک، تنظیمات؛ حساب بیمار؛ چاپ رسید و فاکتور؛ انبار وصل به بل. گزارش‌های R1، R7، R10 تا R15 و M1، M2، M4. حذف کامل `billing.rs` و UI قدیم. | سیستم مالی تازه با همه‌ی کارهای امروز، به‌علاوه‌ی دفتر، سن بدهی، حساب‌های پول، بل و کسر تکلیفی. | **۱۲–۱۴ روز** |
| **۲. کارمندان، معاش، سهم داکتر** | کارمند، قرارداد جبران، لیست معاش ماهانه، مساعده، صورت سهم منجمد و پرداخت، دفتر کسرها؛ R8، R9، M3. | معاش و سهم داکتر کامل. | ۶–۷ روز |
| **۳. دوره، قفل، دفاتر** | بستن و قفل ماه، ربع و سال؛ نگهبان دوره در همه‌ی نویسنده‌ها؛ R4، R5، R6، R16، R17، R18؛ ثبت تعدیلی؛ صفحه‌ی «دفاتر». | دفتر کامل، قفل‌پذیر، قابل ارائه. | ۵–۶ روز |
| **۴. گزارش رسمی و مالیاتی** | صدور رسمی با hash (S7)، قالب سربرگ و امضا، Excel چندبرگه، PDF؛ R2، R3 (دو مبنا و پل)؛ بسته‌ی سالانه برای وزارت؛ بستن سال؛ سند افتتاحیه؛ سه زبان. | هدف اصلی مالک کامل. | ۵–۶ روز |
| **۵. یکپارچه‌سازی و پرداخت نهایی** | داشبورد مالی (B-03: بازه، تولید در برابر وصول، نرخ وصول، اقساط عقب‌افتاده) از `fin.overview`؛ اعلان‌های مالی (سررسید قسط، سلامت دفتر، ماه بسته‌نشده)؛ راهنمای کاربر مالی؛ بازبینی بصری با impeccable؛ آزمون حجم و کارایی با داده‌ی یک سال؛ e2e کامل نقش‌ها. | آماده‌ی نمایش به مشتری اول. | ۴–۵ روز |

**جمع: حدود ۳۲ تا ۳۸ روز کاری** (قبلاً ۴۵ تا ۵۲). حدود ۸ تا ۱۰ روز کار مهاجرت و برابری حذف شد. فاز ۲ و ۳ بعد از فاز ۱ می‌توانند موازی باشند (فایل‌های جدا). فاز ۴ بعد از ۳ است. فاز ۵ آخر است.

### د۱. فایل‌ها (تقریبی)
* **Rust:**
  * `core/src/fin/{mod,codes,ledger,accounts,series,periods,invoices,receipts,allocations,refunds,credit_notes,plans,receivables,suppliers,bills,vouchers,other_receipts,money,cash,transfers,comp,shares,employees,payroll,overview,reports,official,legal,health}.rs`
  * `core/migrations/00NN_finance_v2.sql`
  * `core/src/api.rs`، `core/src/auth.rs`
  * فایل‌های انبار (`inventory*.rs`: تأمین‌کننده، رسید انبار ← بل)
  * `core/src/reports.rs` (حذف گزارش‌های مالی)، `core/src/dashboard`/notifications در `reports.rs`
  * حذف `core/src/billing.rs` و آزمون‌های مالی قدیم (`tests_core_money.rs` بخش مالی، `tests.rs` بخش‌های مالی) با آزمون‌های تازه
* **Shared:** `shared/src/fin.rs` (انواع و enum های عددی `#[repr(u8)]`)؛ حذف انواع billing؛ `shared/ts/contract.ts`.
* **UI:**
  * `ui/src/pages/finance/*`، `ui/src/lib/finance.ts`، `ui/src/print/*`
  * انبار، Dashboard، Users، تب‌های بیمار، Shell
  * `ui/src/i18n/{fa,ps,en}.json`
  * mock های dev
* **مستند:** `docs/guides/*` (راهنمای مالی)، یادداشت انتشار («داده‌ی مالی آزمایشی پاک شد»).

### د۲. آزمون‌ها
* **Rust با اعداد دستی:**
  * برای هر قاعده‌ی ب۵ یک آزمون.
  * **سناریوی استاندارد «کلینیک A/B»** با اعداد دستی test-mission، روی مدل تازه: A = فاکتور ۸۰٬۵۰۰، دریافت ۳۵٬۲۵۰، برگشت ۷۵۰، مصارف ۴۴٬۳۰۰، طلب ۴۶٬۰۰۰. B = ۵۳٬۰۰۰ / ۱۷٬۷۰۰ / ۲٬۰۰۰ / ۵٬۳۰۰ / ۳۷٬۳۰۰.
  * معاش ۳ کارمند با مساعده.
  * سهم داکتر منجمد (یادداشت کاهش بعدی عددش را عوض نمی‌کند).
  * اقساط ۲۴ماهه روی همان روز ماه شمسی، با جمع دقیق.
  * سن بدهی در مرز ۳۰ و ۳۱ روز.
  * تقسیم تخفیف با بزرگ‌ترین باقی‌مانده.
* **Property** (seed ثابت): رشته‌ای تصادفی از اسناد و ابطال‌ها، و پس از آن همیشه: Σamount هر ثبت = ۰، زیر-دفتر = حساب کنترل، پل نقدی و تعهدی، زنجیره‌ی hash.
* **مهاجرت جایگزینی:** fixture دیتابیس v0.12.4 با داده‌ی انبار و مالی ← مهاجرت ← جدول‌های قدیم نیست، داده‌ی انبار و بیمار سالم، `foreign_key_check` خالی، `billed_at` همه NULL.
* **حجم:**
  * سناریوی ۵۰ بیمار و ۲۰۰ سند ← اندازه‌ی دیتابیس پس از `VACUUM` زیر سقفی که `db-size-rules.md` تعیین می‌کند.
  * سناریوی یک سال پرکار ← گزارش‌ها زیر ۳ ثانیه (NFR).
* **قفل و رسمی:** ثبت، ابطال و تعدیل در دوره‌ی قفل رد می‌شود. چاپ دوباره‌ی RPT همان hash را می‌دهد.
* **UI e2e:**
  * مسیر منشی: فاکتور ← دریافت و تخصیص ← اقساط ← برگشت.
  * مسیر حسابدار: بل، سند پرداخت، معاش، شمارش صندوق، بستن ماه، صدور R1.
  * نقش‌ها.
* **آزمون دستی:** فقط یک آزمون دستی نهایی در پایان محصول (حافظه‌ی پروژه).

### د۳. ریسک‌های CI و فنی
* **بازسازی `supplier` و FK های انبار** خطرناک‌ترین بخش مهاجرت است ← آزمون fixture و `foreign_key_check`.
* **شماره‌ی مهاجرت** با سشن‌های موازی تداخل دارد ← لحظه‌ی merge تعیین شود.
* **حذف انواع billing از `contract.ts`** همه‌ی import های UI را می‌شکند ← حذف backend و UI قدیم در همان PR فاز ۱.
* **سشن‌های موازی UI** (layout، آواتار) روی Shell و صفحه‌های بیمار کار می‌کنند ← فاز ۱ بعد از merge آن‌ها، و تغییر در آن فایل‌ها حداقلی.
* **زمان CI:** property tests و fixture زمان را بالا می‌برند ← تعداد محدود، seed ثابت.
* **enum عددی:** عوض کردن ترتیب کدها داده را خراب می‌کند ← کدها در `fin/codes.rs` ثابت، با آزمونی که هر کد را به عدد ثابتش قفل می‌کند.

# هـ. سؤال‌های مالک یا حسابدار (پیش‌فرض‌ها اجازه‌ی شروع می‌دهند)
| # | سؤال | پیش‌فرض |
|---|---|---|
| Q1 | کلینیک اولین مشتری شخص حقیقی است یا شرکت؟ رژیم مالیاتی «ثابت» است یا «مالیات عایدات + BRT»؟ | قابل انتخاب در پروفایل حقوقی. پیش‌فرض: شخص حقیقی، رژیم «نامعلوم»، مبنای رسمی **نقدی**. هر دو مبنا همیشه چاپ می‌شوند. |
| Q2 | داکترها معمولاً معاشی‌اند، فیصدی یا شریک؟ | برای هر داکتر قابل انتخاب. پیش‌فرض فیصدی (مثل امروز). شریک ← برداشت. |
| Q3 | مالیات کسرشده از معاش و کرایه دستی وارد شود؟ | بله، دستی. |
| Q4 | دالر لازم است؟ | نه؛ فقط افغانی. مبلغ دالری فقط یادداشت. |
| Q5 | چه کسی ماه را می‌بندد؟ چه کسی قفل می‌کند؟ | بستن: حسابدار یا مالک. قفل: فقط مالک. |
| Q6 | خرید مواد در مبنای تعهدی هنگام خرید مصرف حساب شود (بدون ارزش‌گذاری انبار)؟ | بله. |
| Q7 | گزارش رسمی به چه زبانی؟ | دری؛ پشتو و انگلیسی قابل انتخاب. |
| Q8 | هنگام راه‌اندازی هر کلینیک (مشتری تازه)، مانده‌ی واقعی صندوق و بانک وارد شود؟ | بله، یک سند افتتاحیه‌ی اختیاری. اگر وارد نشود، صفر. |
| Q9 | برگشت از پولی که به فاکتور خورده: آیا قبول است که همیشه «لغو تخصیص + برگشت» باشد (دو سند)؟ | بله. استاندارد است و جمع‌ها عوض نمی‌شوند. |

---

# و. منابع
| منبع | استفاده | وضعیت |
|---|---|---|
| [Income Tax Law 2009 (momp.gov.af)](https://momp.gov.af/sites/default/files/2019-03/Income_Tax_Law.pdf) | مواد ۳، ۴(۲)، ۳۶، ۳۷، ۳۸، ۴۶، ۵۸ تا ۶۱، ۶۴ تا ۶۶، ۷۲، ۸۸(۵)، ۹۱، ۱۰۱ | ✅ متن ۲۰۰۹ خوانده شد · ⚠ اصلاحات بعدی در این PDF نیست |
| [Income Tax Manual 2009 (Quest)](https://questfinancial.af/wp-content/uploads/2024/08/Income-Tax-Manual_English_2009_CLEAN.pdf) | مقررات ۳۶.۵، ۳۹.۲، ۶۶.۴، ۷۴.۶، ۷۵.۵، مواد ۷۴/۷۵ | ✅ |
| [Quest: New tax rates, July 2026](https://questfinancial.af/afghanistans-new-tax-rates-a-detailed-guide-for-businesses-and-individuals/) | CIT ۱۰٪ شامل مراکز صحی؛ معاش؛ BRT «۴٪» بی‌تغییر؛ شرط اتصال به ARD؛ ماده ۵۹ | ✅ (منبع حرفه‌ای خصوصی) |
| [سایت امارت: کاهش مالیات، ۱۲ جولای ۲۰۲۶](https://www.alemarahenglish.af/ministry-of-finance-announces-tax-reductions-across-various-economic-sectors/) | ۱۰٪ برای شفاخانه‌ها و مراکز صحی؛ معافیت ۱۲۰٬۰۰۰ | ✅ |
| [SIGAR via govinfo](https://www.govinfo.gov/content/pkg/GOVPUB-Y3_P31-PURL-gpo151191/pdf/GOVPUB-Y3_P31-PURL-gpo151191.pdf)، [AAN](https://www.afghanistan-analysts.org/en/reports/economy-development-environment/afghanistans-government-revenue-continuing-robust-growth-in-the-face-of-economic-weakness/) | BRT از ۲٪ به ۴٪ (۲۰۱۵) | ✅ خلاصه‌ی جست‌وجو |
| [investinafghanistan.af](https://investinafghanistan.af/page/taxation) | «مالیات پیشکی ۲٪ ربعوار» | ⚠ با ۴٪ نمی‌خواند؛ بی‌اثر، چون برنامه نرخ ندارد |
| [ICNL](https://www.icnl.org/wp-content/uploads/Afghanistan_inctax.pdf) | BRT قابل کسر از مالیات عایدات | ✅ خلاصه |
| [BBC: ban on foreign currencies](https://www.bbc.com/news/business-59129470) | منع ارز خارجی | ✅ خلاصه |
| [DDD Invoices](https://dddinvoices.com/learn/e-invoicing-afghanistan) | e-invoicing اجباری نیست | ⚠ تجاری |
| [Open Dental: Accounting](https://www.opendental.com/manual/accounting.html)، [Reports](https://www.opendental.com/site/0_reports.html) | ۵ نوع حساب، Lock date، گزارش‌ها | ✅ |
| [Zeldent: reconciliation](https://www.zeldent.com/blog/the-complete-guide-to-dental-practice-reconciliation) | Day sheet + Deposit report | ✅ خلاصه |
| کد Artaveo v0.12.4 و `docs/test-mission/*` | بخش ج۱ و آزمون‌ها | ✅ خوانده شد |

**تأییدنشده (⚠):**
1. نرخ و فرم فعلی BRT برای کلینیک دندان، و آستانه‌های امروز مالیات ثابت (ماده ۷۴/۷۵).
2. فرم‌های فعلی ARD و فرمت «اتصال الکترونیکی». دفتر دوطرفه‌ی این طرح پایه‌ی درستی برای هر خروجی آینده است.
3. طول و قالب TIN، و قالب شماره‌ی جواز صحت عامه.
4. الزام قانون کار افغانستان درباره‌ی دفتر معاش. منبع افغانی پیدا نشد؛ ستون‌ها از ماده ۴۶ و ۶۱ و عرف بین‌المللی گرفته شد.
5. آیا فیصدی داکتر مستقل مشمول کسر قراردادی ماده ۷۲ است.
6. آیا ۵ سال نگهداری در مقررات تازه همان است.

**این سند مشاوره‌ی مالیاتی نیست.** پیش از انتشار، یک حسابدار یا مستوفیت محلی بخش‌های ب۳، ب۵، ب۹ و هـ را ببیند.
