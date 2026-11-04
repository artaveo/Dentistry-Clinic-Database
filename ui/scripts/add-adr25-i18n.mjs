// One-off helper: the ADR-25 license texts (key + hardware approval + token-driven grace), [Dari, Pashto, English].
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const S = {
  "lic.state.unactivated": ["لایسنس وارد نشده", "جواز نه دی ننوتل شوی", "No license entered"],
  "lic.state.unactivated.hint": ["کلید لایسنس را که از آرتاویو گرفته‌اید وارد کنید. تا آن موقع (در نسخهٔ نهایی) برنامه فقط‌خواندنی است.", "هغه جواز کیلي چې له آرتاویو مو اخیستی دلته ولیکئ. تر هغه (په وروستۍ نسخه کې) برنامه یوازې د لوستلو وړ ده.", "Enter the license key you got from Artaveo. Until then (in the final build) the program is read-only."],
  "lic.state.expired_warning": ["لایسنس تمام شده؛ تمدید کنید", "جواز پای ته رسېدلی؛ نوی یې کړئ", "License ended; please renew"],
  "lic.state.expired_warning.hint": ["هنوز مهلت دارید و برنامه کار می‌کند، ولی پس از پایان مهلت فقط‌خواندنی می‌شود. لایسنس را از آرتاویو تمدید کنید.", "تاسو لا مهلت لرئ او برنامه کار کوي، خو د مهلت له پای وروسته یوازې د لوستلو وړ کیږي. جواز له آرتاویو نوی کړئ.", "You are still in the grace period and the program works, but it becomes read-only when it ends. Renew with Artaveo."],
  "lic.state.pending": ["منتظر تأیید آرتاویو", "د آرتاویو تایید ته انتظار", "Waiting for Artaveo's approval"],
  "lic.state.pending.hint": ["کلید لایسنس درست است ولی این کامپیوتر هنوز تأیید نشده. آرتاویو تأیید می‌کند و پس از آن برنامه خودش فعال می‌شود.", "د جواز کیلي سمه ده خو دا کمپیوټر لا نه دی تایید شوی. آرتاویو یې تاییدوي او بیا برنامه پخپله فعالیږي.", "The key is correct but this computer is not approved yet. Artaveo approves it and the program then activates itself."],
  "lic.state.blocked": ["این کامپیوتر مسدود شده", "دا کمپیوټر بند شوی", "This computer is blocked"],
  "lic.state.blocked.hint": ["آرتاویو این کامپیوتر را مسدود کرده. برنامه فقط‌خواندنی است. با پشتیبانی تماس بگیرید.", "آرتاویو دا کمپیوټر بند کړی. برنامه یوازې د لوستلو وړ ده. د ملاتړ سره اړیکه ونیسئ.", "Artaveo blocked this computer. The program is read-only. Contact support."],
  "lic.badge.expired_warning": ["تمدید شود", "نوی شي", "Renew"],
  "lic.badge.pending": ["منتظر تأیید", "تایید ته انتظار", "Awaiting approval"],
  "lic.badge.blocked": ["مسدود", "بند", "Blocked"],
  "lic.banner.pending": ["این کامپیوتر هنوز توسط آرتاویو تأیید نشده و برنامه فقط‌خواندنی است.", "دا کمپیوټر لا د آرتاویو لخوا نه دی تایید شوی او برنامه یوازې د لوستلو وړ ده.", "Artaveo has not approved this computer yet and the program is read-only."],
  "lic.banner.blocked": ["این کامپیوتر مسدود شده و برنامه فقط‌خواندنی است.", "دا کمپیوټر بند شوی او برنامه یوازې د لوستلو وړ ده.", "This computer is blocked and the program is read-only."],
  "lic.fingerprint": ["اثرانگشت این کامپیوتر", "د دې کمپیوټر ګوتلاسۍ", "This computer's fingerprint"],
  "lic.fingerprintHint": ["آرتاویو برای تأیید کامپیوتر همین کد کوتاه را در پنل می‌بیند.", "آرتاویو د کمپیوټر د تایید لپاره همدا لنډ کوډ په پینل کې ویني.", "Artaveo sees this short code in the panel when approving the computer."],
  "lic.pending.title": ["کلید ثبت شد، منتظر تأیید", "کیلي ثبت شوه، تایید ته انتظار", "Key registered, waiting for approval"],
  "lic.pending.body": ["کد این کامپیوتر: {fp}. اگر آرتاویو پرسید همین را بگویید. بعد از تأیید، با اولین اتصال به اینترنت (یا «بررسی اکنون») برنامه فعال می‌شود.", "د دې کمپیوټر کوډ: {fp}. که آرتاویو وپوښتل همدا ووایاست. له تایید وروسته، د لومړۍ انټرنېټ اړیکې سره (یا «اوس تایید کړه») برنامه فعالیږي.", "This computer's code: {fp}. Give it if Artaveo asks. After approval the program activates at the next internet connection (or \"Check now\")."],
  "lic.activate.hint": ["کلید لایسنس را از آرتاویو بگیرید. کلید فقط برای همین کلینیک است و هر کامپیوتر باید جداگانه تأیید شود.", "د جواز کیلي له آرتاویو واخلئ. کیلي یوازې د همدې کلینیک لپاره ده او هر کمپیوټر باید جلا تایید شي.", "Get the license key from Artaveo. It is only for this clinic and every computer is approved separately."],
  "lic.activate.code": ["کلید لایسنس", "د جواز کیلي", "License key"],
  "lic.activate.codeHint": ["شش گروه چهارحرفی مثل 0A1B-2C3D-4E5F-6A7B-8C9D-0E1F", "شپږ ډلې د څلورو توریو لکه 0A1B-2C3D-4E5F-6A7B-8C9D-0E1F", "Six groups of four, like 0A1B-2C3D-4E5F-6A7B-8C9D-0E1F"],
  "lic.offline.hint": ["کد درخواست را برای آرتاویو بفرستید (پیامک یا WhatsApp) و «کد پاسخ» (توکن امضاشده) را اینجا بچسبانید.", "د غوښتنې کوډ آرتاویو ته ولیږئ (SMS یا WhatsApp) او «د ځواب کوډ» (لاسلیک شوی توکن) دلته پیسټ کړئ.", "Send the request code to Artaveo (SMS or WhatsApp) and paste the response code (the signed token) here."],
  "lic.offline.step1Hint": ["شامل شناسهٔ نصب و اثرانگشت این کامپیوتر است؛ هیچ داده‌ای از بیماران ندارد.", "د دې کمپیوټر د نصب پیژند او ګوتلاسۍ لري؛ د ناروغانو هیڅ معلومات نه لري.", "It holds this computer's installation id and fingerprint; no patient data."],
  "lic.reject.token_invalid": ["پاسخ سرور معتبر نبود؛ دوباره تلاش کنید.", "د سرور ځواب معتبر نه و؛ بیا هڅه وکړئ.", "The server's answer was not valid; try again."],
  "lic.reject.token_mismatch": ["این توکن برای این کامپیوتر ساخته نشده.", "دا توکن د دې کمپیوټر لپاره نه دی جوړ شوی.", "This token was not made for this computer."],
  "lic.reject.no_key": ["هنوز کلید لایسنس وارد نشده.", "لا د جواز کیلي نه ده ننوتل شوې.", "No license key has been entered yet."],
  "lic.reject.unknown_key": ["این کلید لایسنس درست نیست.", "دا د جواز کیلي سمه نه ده.", "This license key is not correct."],
  "lic.reject.invalid_request": ["درخواست درست نبود.", "غوښتنه سمه نه وه.", "The request was not valid."],
  "lic.reject.too_many_pending": ["تعداد درخواست‌های منتظر تأیید زیاد است؛ با آرتاویو تماس بگیرید.", "د تایید ته په انتظار غوښتنو شمېر ډېر دی؛ له آرتاویو سره اړیکه ونیسئ.", "Too many requests are waiting for approval; contact Artaveo."],
  "notif.kind.license_expired": ["لایسنس تمام شده", "جواز پای ته رسېدلی", "License ended"],
  "notif.msg.license_expired": ["لایسنس {detail} روز پیش تمام شده؛ تا پایان مهلت تمدید کنید، وگرنه برنامه فقط‌خواندنی می‌شود.", "جواز {detail} ورځې مخکې پای ته رسېدلی؛ تر مهلت پای نوی یې کړئ، که نه برنامه یوازې د لوستلو وړ کیږي.", "The license ended {detail} days ago; renew before the grace ends or the program becomes read-only."],
  "wizard.trial.body": ["برای کار با برنامه کلید لایسنس لازم است. آرتاویو برای شما کلید و (در صورت نیاز) نسخهٔ آزمایشی می‌دهد؛ کلید را بعد از نصب در بخش «مجوز» وارد کنید.", "د برنامې لپاره د جواز کیلي په کار ده. آرتاویو تاسو ته کیلي او (که اړتیا وي) آزمایښتي نسخه درکوي؛ کیلي له نصب وروسته د «جواز» په برخه کې ولیکئ.", "The program needs a license key. Artaveo gives you the key and, if needed, a trial; enter it in the License page after installing."],
};

for (const [i, lang] of ["fa", "ps", "en"].entries()) {
  const file = path.join(root, `src/i18n/${lang}.json`);
  const dict = JSON.parse(fs.readFileSync(file, "utf8"));
  for (const [k, v] of Object.entries(S)) dict[k] = v[i];
  fs.writeFileSync(file, JSON.stringify(dict, null, 2) + "\n");
}
console.log(`set ${Object.keys(S).length} keys`);
