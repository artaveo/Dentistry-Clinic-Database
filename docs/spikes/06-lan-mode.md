# Spike 6 — LAN Mode

**سؤال:** کشف سرور با mDNS، Pairing، TLS و WebSocket روی یک Router معمولی کار می‌کند؟
**نتیجه:** ✅ بله (پروتکل کامل + تست‌ها). **تصمیم:** ADR-18.

## پروتکل

```text
Client                                     Server (کامپیوتر سرور کلینیک)
  │ mDNS browse _artaveo._tcp ────────────▶ advertise (clinic, fp-hint)
  │ TLS (هنوز اعتماد نشده؛ fingerprint ثبت می‌شود)
  │ PairStart{SPAKE2 A} ──────────────────▶ کد ۶ رقمی روی صفحه سرور
  │ ◀──────────────────────── PairReply{SPAKE2 B}
  │ PairConfirm{HMAC(K,"client"‖cert_fp)} ▶ حداکثر ۵ تلاش برای هر کد
  │ ◀─ PairOk{HMAC(K,"server"‖cert_fp), device_id, token}
  │ Pin(cert_fp) + ذخیره token
  │
  │ TLS (Pinned) + Auth{device_id, token}
  │ Rpc{id, method, params} ◀──▶ RpcResult      ◀── Event{topic, data} (Real-time)
```

* **چرا SPAKE2؟** کد ۶ رقمی فقط ۱۰⁶ حالت دارد. با HMAC ساده، مهاجمی که ترافیک را ببیند می‌تواند کد را آفلاین در کسری از ثانیه پیدا کند. با SPAKE2 هر حدس نیاز به یک تلاش آنلاین دارد و سرور بعد از ۵ تلاش کد را باطل می‌کند.
* **Channel Binding:** fingerprint گواهی TLS در MACهای تأیید وارد شده؛ اگر مهاجم TLS را وسط قطع کند (MITM)، fingerprint دو طرف فرق می‌کند و Pairing شکست می‌خورد.
* سرور فقط `SHA-256(token)` را نگه می‌دارد. حذف یک کامپیوتر از لیست = باطل شدن token آن.
* RPC همان `method/params` فرمان‌های Tauri IPC است (ADR-02: یک Contract، دو Transport).

## تست‌ها (`cargo test -p lan-mode`)

| تست | نتیجه |
|---|---|
| `pair_connect_rpc_and_realtime_push` | دو Client (پذیرش، دکتر) Pair می‌شوند؛ پذیرش `queue.check_in` می‌زند و صفحه دکتر Event را فوری دریافت می‌کند؛ میانگین RTT روی loopback 21µs (release) |
| `wrong_code_is_limited_and_reveals_nothing` | کد اشتباه ← ۴،۳،۲،۱،۰ تلاش باقی؛ بعد از ۵ خطا حتی کد درست هم کار نمی‌کند؛ هیچ token صادر نمی‌شود |
| `pinned_client_refuses_impostor_server` | سرور دیگری روی آدرس دیگر (یا IP تغییرکرده توسط DHCP) با گواهی متفاوت رد می‌شود |
| `unknown_device_token_is_rejected` | token جعلی ← `AuthFailed` |
| `mdns_discovery_on_real_network` (ignored) | در این محیط هم اجرا و **پاس** شد؛ در CI اختیاری است چون Multicast روی Runnerها تضمین نیست |

## یافته‌ها

1. **rustls با Provider `ring`** (نه aws-lc-rs پیش‌فرض) ← در Windows به CMake/NASM نیازی نیست.
2. **فایروال:** NSIS Hook (`spikes/tauri-shell/windows/hooks.nsh`) قانون ورودی را فقط برای exe برنامه و فقط پروفایل‌های **Private/Domain** اضافه می‌کند. اگر ویندوز Wi-Fi کلینیک را «Public» شناخته باشد قانون اعمال نمی‌شود ← Setup Wizard حالت سرور باید نوع شبکه را تشخیص دهد و تغییر به Private را پیشنهاد کند (فاز ۷.۱).
3. **Client Isolation:** بعضی Routerها (به‌خصوص شبکه مهمان) ارتباط دستگاه‌ها با هم و mDNS را می‌بندند ← پیام واضح + امکان وارد کردن دستی آدرس به‌عنوان راه دوم (کد Pairing همچنان امنیت را تضمین می‌کند).
4. **IPv4 ترجیح داده می‌شود**؛ برخی Routerهای خانگی Multicast IPv6 را خراب می‌کنند.
5. **گواهی بلندمدت (تا ۲۰۷۵):** اعتماد از Pin می‌آید، نه از CA یا تاریخ انقضا؛ تغییر گواهی = Pair مجدد (عمداً).

## باقی‌مانده (نیازمند سخت‌افزار)

تست با ۳ کامپیوتر روی یک Router معمولی (TP-Link/Tenda) با Wi-Fi و کابل — [چک‌لیست H4](hardware-checklist.md). نمونه‌ها: `cargo run -p lan-mode --example lan_server` و `--example lan_client -- <code>`.
