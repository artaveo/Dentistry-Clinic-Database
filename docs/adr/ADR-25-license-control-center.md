# ADR-25 — لایسنس دندان از طریق Artaveo Control Center (Neon)

**وضعیت:** پذیرفته‌شده و پیاده‌شده در v0.11.1 · جایگزین طرح Supabase فاز ۱۰ (ADR-16)

## تصمیم‌ها (از گفتگوی مالک)
* یک Control Center مرکزی روی Neon (`Artaveo-Control-Center`، schema `cc`) برای همهٔ محصول‌ها. دندان یک **محصول** (`products.code = 'dental'`) است؛ پروژهٔ Neon جدا برای دندان **ساخته نمی‌شود** (Neon زیرپروژه ندارد و معماری Control Center محصول جدید را فقط با ردیف جدید می‌گیرد). داده بیمار هیچ‌وقت آنجا نمی‌رود.
* لایسنس فقط روی کامپیوتر **سرور** کلینیک است؛ کلاینت‌های LAN با همان لایسنس مدیریت می‌شوند و سقفشان از `max_activations`/پلن می‌آید.
* هر دستگاه با `installation_id` (تصادفی) + **اثرانگشت سخت‌افزار** (هش) شناخته می‌شود و **تا تأیید دستی مالک در پنل** فعال نمی‌شود.
* همهٔ اعداد مهلت داخل **توکن** است و از سرور تنظیم می‌شود، نه از کد برنامه.
* مالک همیشه کنترل کامل دارد: باطل/وصل، تمدید تا تاریخ دقیق، ترایل با مدت دلخواه، مسدود کردن دستگاه.

## جریان
```text
کلینیک: license_key + installation_id + fingerprint + app_version + nonce → POST validate
سرور:   کلید؟ لغو؟ منقضی؟ دستگاه تأییدشده؟ ظرفیت؟ → توکن JWS (Ed25519) 
برنامه: امضا با کلید عمومی ← fingerprint توکن == fingerprint این کامپیوتر؟ ← ذخیره کنار دیتابیس
```
دستگاه ناشناس: ردیف `activations` با `status = 'pending'` (نام کامپیوتر، زمان، نسخه). برنامه می‌گوید «در انتظار تأیید ارتاویو» و فقط‌خواندنی می‌ماند.

## فرمت توکن (payload JWS، `alg = EdDSA`)
```json
{
  "v": 2,
  "iss": "artaveo-control-center",
  "result": "valid | pending | blocked | expired | revoked | unknown_key | version_blocked",
  "product": "dental",
  "company_id": "uuid",
  "license_id": "uuid",
  "kind": "trial | paid",
  "plan": "basic | standard | pro",
  "max_computers": 3,
  "features": ["lan", "..."],
  "installation_id": "...",
  "fingerprint": "sha256-hex (۶۴ حرف)",
  "nonce": "همان nonce درخواست",
  "issued_at": "UTC",
  "starts_at": "UTC",
  "expires_at": "UTC | null",
  "check_every_days": 30,
  "offline_grace_days": 120,
  "post_expiry_grace_days": 120,
  "valid_until": "UTC = min(issued_at + offline_grace_days، expires_at + post_expiry_grace_days)"
}
```
قواعد برنامه (در Core، همان منطق فاز ۱۰):
* `now <= expires_at` و `now <= valid_until` → فعال. `kind = trial` → ترایل.
* `now > expires_at` و `now <= valid_until` → فعال با **نوار هشدار** «لایسنس تمام شده، تمدید کنید» (قابل بستن، با هر بار باز شدن برنامه دوباره می‌آید).
* `now > valid_until` → **فقط‌خواندنی** (دیدن، چاپ، Export، پشتیبان و بازیابی همیشه کار می‌کند).
* `result` غیر از `valid` (pending/blocked/revoked) → فقط‌خواندنی، با پیام مخصوص.
* `now` از بزرگ‌ترین زمان دیده‌شده عقب‌تر از ۲۴ ساعت → فقط‌خواندنی (تشخیص دستکاری ساعت).
* `nonce` و `fingerprint` و `installation_id` توکن باید با درخواست/این کامپیوتر برابر باشد، وگرنه توکن رد می‌شود (جلوگیری از کپی توکن).
* اگر `check_every_days` گذشته و اینترنت هست، برنامه خودکار دوباره `validate` می‌زند.

## اثرانگشت سخت‌افزار
`SHA-256("artaveo-dental/fp/v1" ‖ MachineGuid ‖ سریال مادربرد (اگر معتبر) ‖ سریال دیسک سیستم (اگر معتبر))` به‌صورت hex. مک‌آدرس **استفاده نمی‌شود** (قابل تغییر و ناپایدار). سریال‌های جعلی شناخته‌شده (مثل `0123456789ABCDEF`) نادیده گرفته می‌شوند. به کاربر فقط ۸ حرف اول به‌صورت `AB12-CD34` نشان داده می‌شود.

## تغییرات لازم در Control Center (migration پیشنهادی، فقط روی branch `development` و فقط با تأیید مالک)
```sql
-- activations: دستگاه با اثرانگشت و وضعیت تأیید
alter table cc.activations
  add column fingerprint     text,
  add column computer_name   text,
  add column approved_at     timestamptz,
  add column approved_by     text;
alter table cc.activations drop constraint activations_status_check;
alter table cc.activations add constraint activations_status_check
  check (status in ('pending','active','blocked','revoked'));
create unique index activations_license_fp on cc.activations (license_id, fingerprint)
  where status in ('pending','active');

-- licenses: ترایل و اعداد مهلت (قابل تنظیم برای هر کلینیک)
alter table cc.licenses
  add column kind                   text not null default 'paid' check (kind in ('trial','paid')),
  add column check_every_days       int  not null default 30  check (check_every_days between 1 and 365),
  add column offline_grace_days     int  not null default 120 check (offline_grace_days between 1 and 730),
  add column post_expiry_grace_days int  not null default 120 check (post_expiry_grace_days between 0 and 730);
```
توابع مدیریتی تازه (همه audit‌دار، مثل بقیه): `admin_approve_activation`, `admin_block_activation`, `admin_issue_trial(company, days)`, `admin_set_grace(license, check, offline, post_expiry)`. `validate_license` باید fingerprint و computer_name را بگیرد و فقط برای دستگاه `active` توکن `valid` بدهد؛ برای دستگاه تازه ردیف `pending` بسازد (محدود به حداکثر ۲۰ ردیف pending برای هر لایسنس تا spam نشود). ظرفیت: تعداد `active` ≤ `max_activations`.

## پیامد برای کد
* Core: `core/src/license.rs` فرمت `{payload, signature}` را با JWS و فیلدهای بالا عوض می‌کند؛ اثرانگشت در Core ساخته می‌شود؛ فاصلهٔ چک و مهلت‌ها از توکن خوانده می‌شود؛ مهلت ۱۴ روز آزاد اولیه حذف می‌شود و فقط ترایل ۳۰ روزهٔ سرور می‌ماند.
* `server/` (Supabase) حذف می‌شود.
* پنل مدیریت: برنامهٔ مرکزی بعدی؛ تا آن موقع توابع `admin_*` در کنسول Neon.
