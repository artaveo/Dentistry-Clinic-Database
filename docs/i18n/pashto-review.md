# بازبینی متن‌های پشتو — رابط برنامه

> این فایل به‌صورت خودکار از فایل‌های ترجمه برنامه ساخته شده است
> (`node ui/scripts/export-pashto-review.mjs`). هر بار که متن‌های رابط
> برنامه تغییر کنند، دوباره ساخته می‌شود. راهنمای بازبینی: [README.md](README.md).

تعداد متن‌ها: 216

| کلید (فقط برای توسعه‌دهنده) | English (مرجع) | دری (مرجع — تأییدشده) | پښتو (نیازمند بازبینی) | یادداشت بازبینی |
|---|---|---|---|---|
| `app.name` | Artaveo Dental | آرتاویو دنتال | ارتاویو ډینټل | |
| `lang.fa` | دری | دری | دری | |
| `lang.ps` | پښتو | پښتو | پښتو | |
| `lang.en` | English | English | English | |
| `common.save` | Save | ذخیره | ساتل | |
| `common.cancel` | Cancel | لغو | لغوه | |
| `common.loading` | Loading… | در حال بارگذاری… | پورته کېږي… | |
| `common.yes` | Yes | بله | هو | |
| `common.no` | No | نخیر | نه | |
| `common.saved` | Saved | ذخیره شد | وساتل شو | |
| `setup.title` | Clinic setup | راه‌اندازی کلینیک | د کلینیک جوړول | |
| `setup.intro` | First run on this computer. Enter the clinic details and the owner account. | اولین اجرای برنامه روی این کامپیوتر. اطلاعات کلینیک و حساب مالک را وارد کنید. | پر دې کمپیوټر د پروګرام لومړی ځل چلېدل. د کلینیک او د مالک د حساب معلومات ولیکئ. | |
| `setup.language` | Default language | زبان پیش‌فرض | اصلي ژبه | |
| `setup.clinicName` | Clinic name | نام کلینیک | د کلینیک نوم | |
| `setup.owner` | Owner account | حساب مالک (Owner) | د مالک حساب (Owner) | |
| `setup.next` | Next | بعدی | بل | |
| `setup.back` | Back | قبلی | مخکنی | |
| `setup.create` | Create clinic | ایجاد کلینیک | کلینیک جوړ کړئ | |
| `setup.creating` | Creating the encrypted database… | در حال ایجاد دیتابیس رمزنگاری‌شده… | کوډ شوې ډیټابیس جوړېږي… | |
| `wizard.step.welcome` | Welcome | خوش‌آمدید | ښه راغلاست | |
| `wizard.step.language` | Language | زبان | ژبه | |
| `wizard.step.install_mode` | Install mode | حالت نصب | د نصب ډول | |
| `wizard.step.clinic_info` | Clinic info | اطلاعات کلینیک | د کلینیک معلومات | |
| `wizard.step.hours` | Hours & calendar | ساعات کاری و تقویم | د کار ساعتونه او کالیندر | |
| `wizard.step.clinic_type` | Clinic type | نوع کلینیک | د کلینیک ډول | |
| `wizard.step.branding` | Appearance & branding | ظاهر و برندینگ | ښکلا او برانډینګ | |
| `wizard.step.trial` | Trial | نسخه آزمایشی | ازمېښتي نسخه | |
| `wizard.step.backup` | Backup | پشتیبان‌گیری | بیک اپ | |
| `wizard.step.owner` | Owner account | حساب مالک | د مالک حساب | |
| `wizard.welcome.title` | Welcome to Artaveo Dental | به آرتاویو دنتال خوش آمدید | آرتاویو ډینټل ته ښه راغلاست | |
| `wizard.welcome.body` | A few simple steps to set up your clinic: language, clinic info, working hours, appearance, and finally the owner account. | در چند مرحله ساده، کلینیک خود را راه‌اندازی می‌کنیم: زبان، اطلاعات کلینیک، ساعات کاری، ظاهر برنامه و در پایان حساب مالک. | په څو ساده مرحلو کې به ستاسو کلینیک جوړ کړو: ژبه، د کلینیک معلومات، د کار ساعتونه، د پروګرام بڼه او په پای کې د مالک حساب. | |
| `wizard.welcome.start` | Start setup | شروع راه‌اندازی | د جوړولو پیل | |
| `wizard.installMode.title` | How does this computer work? | این کامپیوتر چطور کار می‌کند؟ | دا کمپیوټر څنګه کار کوي؟ | |
| `wizard.installMode.single.title` | Single computer | تک‌کامپیوتر | یو کمپیوټر | |
| `wizard.installMode.single.hint` | Everything on this computer. The default, and the right choice to start. | همه چیز روی همین کامپیوتر. پیش‌فرض و برای شروع مناسب است. | هر څه پر همدې کمپیوټر دي. اصلي ټاکنه ده او د پیل لپاره ښه ده. | |
| `wizard.installMode.server.title` | Clinic server | سرور کلینیک | د کلینیک سرور | |
| `wizard.installMode.server.hint` | This computer holds the database; other computers connect to it. | این کامپیوتر دیتابیس را نگه می‌دارد و کامپیوترهای دیگر به آن وصل می‌شوند. | دا کمپیوټر ډیټابیس ساتي او نور کمپیوټرونه ورسره نښلي. | |
| `wizard.installMode.client.title` | Connect to a clinic server | اتصال به سرور کلینیک | د کلینیک سرور سره نښلېدل | |
| `wizard.installMode.client.hint` | Connect to a clinic server running on another computer. | به سرور کلینیک که روی کامپیوتر دیگری اجرا است وصل شوید. | له هغه سرور سره ونښلئ چې پر بل کمپیوټر چلېږي. | |
| `wizard.installMode.comingSoon` | Available in a later phase | در فاز بعدی فعال می‌شود | په راتلونکي مرحله کې فعالېږي | |
| `wizard.clinicInfo.title` | Clinic info | اطلاعات کلینیک | د کلینیک معلومات | |
| `wizard.clinicInfo.logo` | Clinic logo (optional) | لوگوی کلینیک (اختیاری) | د کلینیک لوګو (اختیاري) | |
| `wizard.clinicInfo.logoRemove` | Remove logo | حذف لوگو | لوګو لرې کړئ | |
| `wizard.clinicInfo.province` | Province | ولایت | ولایت | |
| `wizard.clinicInfo.district` | District | ولسوالی / ناحیه | ولسوالۍ | |
| `wizard.clinicInfo.address` | Address | آدرس | پته | |
| `wizard.clinicInfo.phone` | Phone | شماره تماس | د اړیکې شمېره | |
| `wizard.hours.title` | Working hours & calendar | ساعات کاری و تقویم | د کار ساعتونه او کالیندر | |
| `wizard.hours.calendar` | Calendar for showing dates | تقویم نمایش تاریخ | د نېټې ښودلو کالیندر | |
| `wizard.hours.calendar.shamsi` | Solar Hijri (Shamsi) | هجری شمسی | لمریز (شمسي) | |
| `wizard.hours.calendar.gregorian` | Gregorian | میلادی | میلادي | |
| `wizard.hours.workingHours` | Clinic working hours | ساعات کاری کلینیک | د کلینیک د کار ساعتونه | |
| `wizard.hours.open` | Open | باز | خلاص | |
| `wizard.day.0` | Saturday | شنبه | شنبه | |
| `wizard.day.1` | Sunday | یکشنبه | یکشنبه | |
| `wizard.day.2` | Monday | دوشنبه | دوشنبه | |
| `wizard.day.3` | Tuesday | سه‌شنبه | سه‌شنبه | |
| `wizard.day.4` | Wednesday | چهارشنبه | چارشنبه | |
| `wizard.day.5` | Thursday | پنج‌شنبه | پنجشنبه | |
| `wizard.day.6` | Friday | جمعه | جمعه | |
| `wizard.clinicType.title` | Clinic type | نوع کلینیک | د کلینیک ډول | |
| `wizard.clinicType.solo.title` | Solo doctor | تک‌دکتر | یو ډاکټر | |
| `wizard.clinicType.solo.hint` | Only one doctor; forms get simpler and the doctor picker is hidden. | فقط یک دکتر؛ فرم‌ها ساده‌تر می‌شوند و انتخاب دکتر پنهان است. | یوازې یو ډاکټر؛ فورمې ساده کېږي او د ډاکټر ټاکنه پټه وي. | |
| `wizard.clinicType.multi.title` | Multiple doctors | چند دکتر | څو ډاکتران | |
| `wizard.clinicType.multi.hint` | Several doctors at the clinic; you can also add more later. | چند دکتر در کلینیک؛ می‌توانید بعداً هم اضافه کنید. | په کلینیک کې څو ډاکتران؛ وروسته هم کولی شئ نور زیات کړئ. | |
| `wizard.branding.title` | Appearance & colors | ظاهر و رنگ‌ها | ښکلا او رنګونه | |
| `wizard.branding.theme` | Display mode | حالت نمایش | د ښودنې ډول | |
| `wizard.branding.preview` | Clinic colors | رنگ‌های کلینیک | د کلینیک رنګونه | |
| `wizard.branding.colorPrimary` | Primary color | رنگ اصلی | اصلي رنګ | |
| `wizard.branding.colorSecondary` | Secondary color | رنگ دوم | دویم رنګ | |
| `wizard.branding.colorAccent` | Accent color | رنگ تاکیدی | ټینګارې رنګ | |
| `theme.light` | Light | روشن | روښانه | |
| `theme.dark` | Dark | تیره | تیاره | |
| `theme.system` | Match system | مطابق سیستم | د سیسټم سره سم | |
| `wizard.trial.title` | Trial | نسخه آزمایشی | ازمېښتي نسخه | |
| `wizard.trial.body` | You can start with the trial right now. The full licensing and activation system is completed in Phase 10; no restriction applies yet. | می‌توانید همین حالا با نسخه آزمایشی شروع کنید. سامانه کامل لایسنس و فعال‌سازی در فاز ۱۰ تکمیل می‌شود؛ فعلاً هیچ محدودیتی اعمال نمی‌شود. | تاسو کولی شئ همدا اوس له ازمېښتي نسخې سره پیل وکړئ. بشپړ د جواز او فعالولو سیسټم په ۱۰ مرحله کې بشپړېږي؛ اوس مهال هیڅ محدودیت نه لګېږي. | |
| `wizard.trial.ack` | I understand, continue | متوجه شدم و می‌خواهم ادامه دهم | پوه شوم، دوام غواړم | |
| `wizard.backup.title` | Backup | پشتیبان‌گیری | بیک اپ | |
| `wizard.backup.body` | Daily and manual backups are already active. You can change the daily backup time later from Settings. | از همین حالا پشتیبان‌گیری روزانه و دستی فعال است. ساعت پشتیبان روزانه را بعداً از بخش تنظیمات می‌توانید تغییر دهید. | ورځنی او لاسي بیک اپ له همدا اوسه فعال دي. د ورځني بیک اپ ساعت وروسته د امستنو له برخې بدلولی شئ. | |
| `recovery.title` | Recovery Key | کلید بازیابی (Recovery Key) | د بیا راګرځولو کیلي (Recovery Key) | |
| `recovery.warning` | This key is shown only once. Print it or write it down and keep it somewhere safe. Without it, the owner password cannot be recovered and the data cannot be moved to a new computer. | این کلید فقط همین یک بار نمایش داده می‌شود. آن را چاپ کنید یا روی کاغذ بنویسید و در جای امن نگه دارید. بدون این کلید، بازیابی رمز مالک و انتقال اطلاعات به کامپیوتر جدید ممکن نیست. | دا کیلي یوازې همدا یو ځل ښودل کېږي. چاپ یې کړئ یا پر کاغذ یې ولیکئ او په خوندي ځای کې یې وساتئ. له دې کیلي پرته د مالک پټنوم بیا راګرځول او نوي کمپیوټر ته د معلوماتو لېږدول ممکن نه دي. | |
| `recovery.print` | Print key | چاپ کلید | کیلي چاپ کړئ | |
| `recovery.confirm` | I have printed or written down the key and stored it safely | کلید را چاپ کردم یا نوشتم و در جای امن گذاشتم | کیلي مې چاپ یا ولیکله او په خوندي ځای کې مې کېښوده | |
| `recovery.continue` | Continue to sign in | ادامه به ورود | ننوتلو ته دوام | |
| `recovery.sheetTitle` | Artaveo Dental Recovery Key | کلید بازیابی آرتاویو دنتال | د ارتاویو ډینټل د بیا راګرځولو کیلي | |
| `recovery.sheetClinic` | Clinic | کلینیک | کلینیک | |
| `recovery.sheetDate` | Date | تاریخ | نېټه | |
| `login.title` | Sign in | ورود | ننوتل | |
| `login.username` | Username | نام کاربری | کارن نوم | |
| `login.password` | Password | رمز عبور | پټنوم | |
| `login.passwordRepeat` | Repeat password | تکرار رمز عبور | پټنوم بیا ولیکئ | |
| `login.displayName` | Display name | نام نمایشی | ښکاره نوم | |
| `login.submit` | Sign in | ورود | ننوتل | |
| `login.forgot` | Forgot the owner password? Recover with the Recovery Key | رمز مالک را فراموش کرده‌اید؟ بازیابی با کلید بازیابی | د مالک پټنوم مو هېر شوی؟ د بیا راګرځولو په کیلي یې بیا راوګرځوئ | |
| `recover.title` | Recover owner password | بازیابی رمز مالک | د مالک پټنوم بیا راګرځول | |
| `recover.key` | Recovery Key | کلید بازیابی | د بیا راګرځولو کیلي | |
| `recover.newPassword` | New password | رمز جدید | نوی پټنوم | |
| `recover.submit` | Reset owner password | تغییر رمز مالک | د مالک پټنوم بدل کړئ | |
| `recover.done` | The owner password was changed. Sign in now. | رمز مالک تغییر کرد. اکنون وارد شوید. | د مالک پټنوم بدل شو. اوس ننوځئ. | |
| `recover.back` | Back to sign in | بازگشت به ورود | ننوتلو ته ورګرځېدل | |
| `shell.lock` | Lock screen | قفل صفحه | پرده قفل کړئ | |
| `shell.logout` | Sign out | خروج | وتل | |
| `nav.system` | About | درباره برنامه | د پروګرام په اړه | |
| `nav.backup` | Backup | پشتیبان‌گیری | بیک اپ | |
| `nav.users` | Users | کاربران | کاروونکي | |
| `nav.audit` | Audit log | گزارش رویدادها | د پېښو راپور | |
| `nav.clinic` | Clinic info | اطلاعات کلینیک | د کلینیک معلومات | |
| `nav.settings` | Settings | تنظیمات | امستنې | |
| `nav.password` | Change password | تغییر رمز | پټنوم بدلول | |
| `shell.sidebar.toggle` | Open/close menu | باز/بسته کردن منو | مینو خلاصول/بندول | |
| `shell.commandPalette.placeholder` | Search pages… | جستجوی صفحه… | د پاڼې لټون… | |
| `shell.commandPalette.empty` | Nothing found. | چیزی پیدا نشد. | هیڅ ونه موندل شول. | |
| `shell.commandPalette.hint` | Press Ctrl+K to open this | برای باز کردن این پنجره Ctrl+K را بزنید | د دې خلاصولو لپاره Ctrl+K کېکاږئ | |
| `shell.notifications.title` | Notifications | اعلان‌ها | خبرتیاوې | |
| `shell.notifications.empty` | No notifications yet. | هیچ اعلانی نیست. | تر اوسه هیڅ خبرتیا نشته. | |
| `shell.status.online` | Online | آنلاین | آنلاین | |
| `shell.status.offline` | Offline | آفلاین | آفلاین | |
| `shell.status.installMode.single` | Single computer | تک‌کامپیوتر | یو کمپیوټر | |
| `shell.status.installMode.server` | Clinic server | سرور کلینیک | د کلینیک سرور | |
| `shell.status.installMode.client` | Connected to server | اتصال به سرور | له سرور سره وصل | |
| `shell.appearance` | Appearance | ظاهر برنامه | بڼه | |
| `shell.perf.label` | Performance mode | حالت کارایی | د کارایی حالت | |
| `shell.perf.full` | Full (visual effects) | کامل (جلوه‌های بصری) | بشپړ (بصري افکتونه) | |
| `shell.perf.reduced` | Reduced (weak computers) | کاهش‌یافته (کامپیوترهای ضعیف) | کم شوی (کمزوري کمپیوټرونه) | |
| `clinic.title` | Clinic info | اطلاعات کلینیک | د کلینیک معلومات | |
| `clinic.saved` | Saved. | ذخیره شد. | وساتل شو. | |
| `clinic.address` | Address | آدرس | پته | |
| `clinic.phone` | Phone | شماره تماس | د اړیکې شمېره | |
| `clinic.calendarSystem` | Calendar | تقویم | کالیندر | |
| `clinic.clinicMode` | Clinic type | نوع کلینیک | د کلینیک ډول | |
| `clinic.theme` | Display mode | حالت نمایش | د ښودنې ډول | |
| `clinic.colors` | Colors | رنگ‌ها | رنګونه | |
| `clinic.workingHours` | Working hours | ساعات کاری | د کار ساعتونه | |
| `lock.title` | Screen locked | صفحه قفل است | پرده قفل ده | |
| `lock.hint` | Enter your password to continue. | برای ادامه رمز عبور خود را وارد کنید. | د دوام لپاره خپل پټنوم ولیکئ. | |
| `lock.unlock` | Unlock | باز کردن | خلاصول | |
| `system.title` | About / System Info | درباره برنامه / اطلاعات سیستم | د پروګرام په اړه / د سیستم معلومات | |
| `system.version` | App version | نسخه برنامه | د پروګرام نسخه | |
| `system.commit` | Build id | شناسه ساخت | د جوړښت پېژند | |
| `system.buildArch` | Installed build for processor | نسخه نصب‌شده برای پردازنده | نصب شوې نسخه د پروسیسر لپاره | |
| `system.machineArch` | This computer's processor | پردازنده این کامپیوتر | د دې کمپیوټر پروسیسر | |
| `system.emulated` | This build is for a different processor and runs emulated. Install the build for this computer. | این نسخه برای پردازنده دیگری ساخته شده و به‌صورت شبیه‌سازی اجرا می‌شود. نسخه مخصوص این کامپیوتر را نصب کنید. | دا نسخه د بل پروسیسر لپاره جوړه شوې او په شبیه‌سازۍ چلېږي. د دې کمپیوټر ځانګړې نسخه نصب کړئ. | |
| `system.os` | Operating system | سیستم‌عامل | عامل سیستم | |
| `system.computer` | Computer name | نام کامپیوتر | د کمپیوټر نوم | |
| `system.environment` | Environment | محیط اجرا | د چلولو چاپېریال | |
| `system.encryption` | Database encryption | رمزنگاری دیتابیس | د ډیټابیس کوډ کول | |
| `system.encrypted` | Encrypted ✓ | رمزنگاری‌شده ✓ | کوډ شوې ✓ | |
| `system.notEncrypted` | Not encrypted ✗ | رمزنگاری نشده ✗ | کوډ شوې نه ده ✗ | |
| `system.keyProtection` | Key protection | محافظت کلید | د کیلي ساتنه | |
| `system.key.windows_dpapi_machine` | Windows DPAPI (this computer) | Windows DPAPI (همین کامپیوتر) | Windows DPAPI (همدا کمپیوټر) | |
| `system.key.insecure_dev_file` | Test file (development only) | فایل آزمایشی (فقط محیط توسعه) | ازمېښتي فایل (یوازې د پراختیا لپاره) | |
| `system.schema` | Database schema version | نسخه ساختار دیتابیس | د ډیټابیس جوړښت نسخه | |
| `system.dbSize` | Database size | حجم دیتابیس | د ډیټابیس اندازه | |
| `system.integrity` | Database health check | بررسی سلامت دیتابیس | د ډیټابیس روغتیا ازموینه | |
| `system.integrity.pending` | Pending | در انتظار | انتظار | |
| `system.integrity.running` | Checking… | در حال بررسی… | ازمویل کېږي… | |
| `system.integrity.ok` | Healthy ✓ | سالم ✓ | روغ ✓ | |
| `system.integrity.failed` | Problem found ✗ | مشکل دارد ✗ | ستونزه لري ✗ | |
| `system.lastBackup` | Last backup | آخرین پشتیبان | وروستی بیک اپ | |
| `system.noBackup` | No backup yet | هنوز پشتیبانی گرفته نشده | تر اوسه بیک اپ نه دی اخیستل شوی | |
| `system.dataDir` | Data folder | محل اطلاعات | د معلوماتو ځای | |
| `backup.title` | Local backup | پشتیبان‌گیری محلی | سیمه‌ییز بیک اپ | |
| `backup.hint` | A daily backup is taken automatically. You can also back up manually at any time. | پشتیبان روزانه به‌صورت خودکار گرفته می‌شود. هر زمان هم می‌توانید دستی پشتیبان بگیرید. | ورځنی بیک اپ په خپله اخیستل کېږي. هر وخت لاسي بیک اپ هم اخیستلی شئ. | |
| `backup.now` | Back up now | همین حالا پشتیبان بگیر | همدا اوس بیک اپ واخلئ | |
| `backup.running` | Backing up and verifying… | در حال پشتیبان‌گیری و بررسی… | بیک اپ اخیستل کېږي او ازمویل کېږي… | |
| `backup.done` | Backup created and verified. | پشتیبان گرفته و بررسی شد. | بیک اپ واخیستل شو او وازمویل شو. | |
| `backup.file` | File | فایل | فایل | |
| `backup.date` | Time | زمان | وخت | |
| `backup.size` | Size | حجم | اندازه | |
| `backup.kind` | Type | نوع | ډول | |
| `backup.kind.manual` | Manual | دستی | لاسي | |
| `backup.kind.daily` | Daily | روزانه | ورځنی | |
| `backup.kind.pre_migration` | Before update | پیش از به‌روزرسانی | له تازه کېدو مخکې | |
| `backup.verified` | Verified | بررسی‌شده | ازمویل شوی | |
| `users.title` | Users | کاربران | کاروونکي | |
| `users.add` | Add user | افزودن کاربر | کاروونکی زیات کړئ | |
| `users.role` | Role | نقش | رول | |
| `users.active` | Active | فعال | فعال | |
| `users.edit` | Edit | ویرایش | سمول | |
| `users.created` | User created. | کاربر ایجاد شد. | کاروونکی جوړ شو. | |
| `role.owner` | Owner | مالک | مالک | |
| `role.administrator` | Administrator | مدیر | مدیر | |
| `role.receptionist` | Receptionist | پذیرش | پذیرش | |
| `role.doctor` | Doctor | داکتر | ډاکټر | |
| `role.accountant` | Accountant | حسابدار | محاسب | |
| `role.assistant` | Assistant | دستیار | مرستیال | |
| `audit.title` | Audit log (read-only) | گزارش رویدادها (فقط خواندنی) | د پېښو راپور (یوازې لوستل) | |
| `audit.when` | Time | زمان | وخت | |
| `audit.who` | User | کاربر | کاروونکی | |
| `audit.action` | Action | عملیات | کړنه | |
| `audit.entity` | Item | مورد | مورد | |
| `audit.computer` | Computer | کامپیوتر | کمپیوټر | |
| `audit.more` | Load more | موارد بیشتر | نور | |
| `settings.title` | Settings | تنظیمات | امستنې | |
| `settings.timeout` | Auto-lock after (idle minutes) | قفل خودکار پس از (دقیقه بی‌کاری) | له بې‌کارۍ وروسته خپلکاره قفل (دقیقې) | |
| `settings.backupHour` | Daily backup hour (0–23) | ساعت پشتیبان روزانه (۰ تا ۲۳) | د ورځني بیک اپ ساعت (۰ تر ۲۳) | |
| `settings.keep` | Backups to keep | تعداد پشتیبان‌های نگهداری‌شده | د ساتل شویو بیک اپونو شمېر | |
| `password.title` | Change password | تغییر رمز عبور | پټنوم بدلول | |
| `password.current` | Current password | رمز فعلی | اوسنی پټنوم | |
| `password.new` | New password | رمز جدید | نوی پټنوم | |
| `password.changed` | Password changed. | رمز عبور تغییر کرد. | پټنوم بدل شو. | |
| `error.mismatch` | Passwords do not match. | رمزها یکسان نیستند. | پټنومونه یو شان نه دي. | |
| `error.not_set_up` | The clinic is not set up yet. | کلینیک هنوز راه‌اندازی نشده است. | کلینیک تر اوسه نه دی جوړ شوی. | |
| `error.already_set_up` | This computer is already set up. | این کامپیوتر قبلاً راه‌اندازی شده است. | دا کمپیوټر دمخه جوړ شوی. | |
| `error.unauthenticated` | Please sign in. | لطفاً وارد شوید. | مهرباني وکړئ ننوځئ. | |
| `error.session_expired` | Your session ended. Please sign in again. | نشست شما پایان یافته است. دوباره وارد شوید. | ستاسو ناسته پای ته ورسېده. بیا ننوځئ. | |
| `error.session_locked` | The screen is locked. | صفحه قفل است. | پرده قفل ده. | |
| `error.forbidden` | You are not allowed to do this. | شما اجازه این کار را ندارید. | تاسو د دې کار اجازه نه لرئ. | |
| `error.invalid_credentials` | Wrong username or password. | نام کاربری یا رمز عبور اشتباه است. | کارن نوم یا پټنوم ناسم دی. | |
| `error.account_locked` | Too many failed attempts: the account is locked for 15 minutes. | به‌دلیل تلاش‌های ناموفق، حساب ۱۵ دقیقه قفل شد. | د ناکامو هڅو له امله حساب ۱۵ دقیقې قفل شو. | |
| `error.recovery_key_invalid` | The Recovery Key is not correct. | کلید بازیابی درست نیست. | د بیا راګرځولو کیلي سمه نه ده. | |
| `error.validation` | The entered information is not valid. | اطلاعات واردشده معتبر نیست. | ورکړل شوي معلومات سم نه دي. | |
| `error.conflict` | Another user changed this record. Refresh and try again. | این رکورد توسط کاربر دیگری تغییر کرده است. صفحه را تازه کنید. | دا ریکارډ بل کاروونکي بدل کړی. پاڼه تازه کړئ. | |
| `error.not_found` | Not found. | مورد پیدا نشد. | ونه موندل شو. | |
| `error.unknown_method` | Invalid request. | درخواست نامعتبر. | ناسمه غوښتنه. | |
| `error.internal` | Internal error. Details were written to the log file. | خطای داخلی برنامه. جزئیات در فایل گزارش ثبت شد. | د پروګرام داخلي تېروتنه. جزئیات د راپور په فایل کې ثبت شول. | |
