# ADR-20 — توزیع WebView2

**وضعیت:** پذیرفته‌شده · **شواهد:** [Spike 7](../spikes/07-webview2.md)

## تصمیم
* دو Installer از یک Build:
  * **استاندارد:** `webviewInstallMode = embedBootstrapper (silent)` — حدود 1.8MB اضافه؛ فقط اگر WebView2 نصب نباشد به اینترنت نیاز دارد.
  * **آفلاین:** `offlineInstaller (silent)` — حدود 130MB اضافه؛ برای کلینیک‌های بدون اینترنت.
* Evergreen Runtime (نه Fixed) تا آپدیت امنیتی Chromium توسط ویندوز انجام شود.
* هر دو Installer از وب‌سایت Artaveo منتشر می‌شوند و فعلاً **بدون Code Signing** هستند (تصمیم کسب‌وکار در رودمپ)؛ هشدار SmartScreen با راهنمای نصب پوشش داده می‌شود. حجم بیشتر Installer آفلاین تأثیری بر این هشدار ندارد. امضای Update با کلید Tauri Updater مستقل از این تصمیم و اجباری است.
* NSIS، `installMode = perMachine`، زبان‌های English + Persian؛ پشتو با `customLanguageFiles` در فاز ۱۰.
