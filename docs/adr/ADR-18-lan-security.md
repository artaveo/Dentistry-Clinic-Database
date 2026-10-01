# ADR-18 — امنیت حالت شبکه: Pairing با SPAKE2 و TLS Pinning

**وضعیت:** پذیرفته‌شده · **شواهد:** [Spike 6](../spikes/06-lan-mode.md)

## تصمیم
* کشف: mDNS/DNS-SD با سرویس `_artaveo._tcp.local.` و TXT شامل نام کلینیک و ۴ بایت اول fingerprint.
* سرور یک گواهی **Self-signed ECDSA P-256** بلندمدت دارد؛ اعتماد فقط از **Pin** کردن SHA-256 گواهی می‌آید.
* Pairing: کد ۶ رقمی (اعتبار ۱۰ دقیقه، یک کد = یک کامپیوتر، حداکثر ۵ تلاش) با **SPAKE2 (Ed25519)** و تأیید کلید با HMAC که fingerprint گواهی را شامل است (Channel Binding).
* پس از Pairing: token تصادفی ۲۵۶ بیتی برای هر دستگاه؛ سرور فقط Hash آن را نگه می‌دارد؛ لغو دستگاه از صفحه مدیریت Clientها.
* Transport: TLS 1.2/1.3 با `rustls` (Provider: `ring`) + WebSocket؛ فریم‌های JSON با همان `method/params` فرمان‌های Tauri IPC؛ Eventها به‌صورت Push.
* فایروال: Installer قانون ورودی فقط برای exe برنامه و پروفایل‌های Private/Domain اضافه می‌کند.

## پیامدها
* حمله Brute-force آفلاین روی کد ۶ رقمی ممکن نیست؛ MITM در زمان Pairing شکست می‌خورد.
* تغییر/بازسازی گواهی سرور یعنی Pair مجدد همه Clientها (در «تبدیل حالت» فاز ۷.۶ گواهی حفظ می‌شود).
* شبکه Public ویندوز و Client Isolation روتر باید در Wizard تشخیص داده شوند؛ ورود دستی آدرس به‌عنوان راه دوم.
