// License format shared by the Supabase Edge Functions and checked by the
// desktop Core (core/src/license.rs, ADR-15). Plain JavaScript on purpose so
// Deno (the functions) and Node (the tests in CI) run the very same file.
//
// A license file is `{ "payload": "<json text>", "signature": "<base64>" }`.
// The signature is Ed25519 over the UTF-8 bytes of the payload TEXT, so no
// canonical-JSON rules are needed on either side.

export const PLANS = {
  basic: { computers: 1 },
  standard: { computers: 3 },
  pro: { computers: 6 },
  trial: { computers: 6 },
};

export const TRIAL_DAYS = 30;
export const VERIFY_EVERY_DAYS = 7;
export const DEFAULT_OFFLINE_GRACE_DAYS = 30;

const enc = new TextEncoder();

const b64 = {
  encode: (bytes) => btoa(String.fromCharCode(...new Uint8Array(bytes))),
  decode: (text) => Uint8Array.from(atob(text), (c) => c.charCodeAt(0)),
};

// PKCS#8 wrapper for a raw 32-byte Ed25519 seed (RFC 8410).
const PKCS8_PREFIX = Uint8Array.from([0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20]);

async function importPrivate(seedB64) {
  const seed = b64.decode(seedB64);
  if (seed.length !== 32) throw new Error("signing key must be a 32-byte Ed25519 seed");
  const pkcs8 = new Uint8Array(PKCS8_PREFIX.length + 32);
  pkcs8.set(PKCS8_PREFIX);
  pkcs8.set(seed, PKCS8_PREFIX.length);
  return crypto.subtle.importKey("pkcs8", pkcs8, { name: "Ed25519" }, false, ["sign"]);
}

async function importPublic(publicB64) {
  return crypto.subtle.importKey("raw", b64.decode(publicB64), { name: "Ed25519" }, false, ["verify"]);
}

export async function generateKeyPair() {
  const pair = await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"]);
  const pkcs8 = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
  const publicRaw = new Uint8Array(await crypto.subtle.exportKey("raw", pair.publicKey));
  return { privateSeed: b64.encode(pkcs8.slice(PKCS8_PREFIX.length)), publicKey: b64.encode(publicRaw) };
}

const addDays = (iso, days) => new Date(new Date(iso).getTime() + days * 86_400_000).toISOString();

/** Builds the payload text for one installation. */
export function buildPayload({ licenseId, clinicId, installationId, kind, plan, startsAt, expiresAt, now, offlineGraceDays }) {
  if (!PLANS[plan]) throw new Error(`unknown plan ${plan}`);
  return JSON.stringify({
    v: 1,
    license_id: licenseId,
    clinic_id: clinicId,
    installation_id: installationId,
    kind,
    plan,
    computers: PLANS[plan].computers,
    issued_at: now,
    verified_at: now,
    starts_at: startsAt,
    expires_at: expiresAt,
    offline_grace_days: offlineGraceDays ?? DEFAULT_OFFLINE_GRACE_DAYS,
    verify_every_days: VERIFY_EVERY_DAYS,
  });
}

export async function signPayload(privateSeedB64, payloadText) {
  const key = await importPrivate(privateSeedB64);
  const sig = await crypto.subtle.sign({ name: "Ed25519" }, key, enc.encode(payloadText));
  return { payload: payloadText, signature: b64.encode(sig) };
}

export async function verifyLicense(publicKeyB64, file) {
  const key = await importPublic(publicKeyB64);
  return crypto.subtle.verify({ name: "Ed25519" }, key, b64.decode(file.signature), enc.encode(file.payload));
}

/** Renewal counts from the previous expiry, even when paid late (roadmap 10.8); a first purchase counts from today. */
export function renewedExpiry(previousExpiry, purchasedAt, months) {
  const end = new Date(previousExpiry ?? purchasedAt);
  end.setUTCMonth(end.getUTCMonth() + months);
  return end.toISOString();
}

export function trialExpiry(now) {
  return addDays(now, TRIAL_DAYS);
}

// No 0/O/1/I so a code read over the phone or sent by SMS is not misheard.
const CODE_ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/** `ARTA-7K2M-9QXP-4HTD` */
export function generateActivationCode() {
  const bytes = crypto.getRandomValues(new Uint8Array(12));
  const chars = Array.from(bytes, (b) => CODE_ALPHABET[b % CODE_ALPHABET.length]);
  return `ARTA-${chars.slice(0, 4).join("")}-${chars.slice(4, 8).join("")}-${chars.slice(8, 12).join("")}`;
}

export function normalizeCode(text) {
  const body = String(text ?? "").toUpperCase().replace(/[^A-Z0-9]/g, "").replace(/^ARTA/, "");
  if (body.length !== 12 || [...body].some((c) => !CODE_ALPHABET.includes(c))) return null;
  return `ARTA-${body.slice(0, 4)}-${body.slice(4, 8)}-${body.slice(8, 12)}`;
}

export async function sha256Hex(text) {
  const digest = await crypto.subtle.digest("SHA-256", enc.encode(text));
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}
