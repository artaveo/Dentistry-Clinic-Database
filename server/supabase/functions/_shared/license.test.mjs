import assert from "node:assert/strict";
import { test } from "node:test";
import {
  PLANS,
  buildPayload,
  generateActivationCode,
  generateKeyPair,
  normalizeCode,
  renewedExpiry,
  signPayload,
  trialExpiry,
  verifyLicense,
} from "./license.mjs";

const base = {
  licenseId: "l1",
  clinicId: "c1",
  installationId: "i1",
  kind: "paid",
  plan: "pro",
  startsAt: "2026-01-01T00:00:00.000Z",
  expiresAt: "2027-01-01T00:00:00.000Z",
  now: "2026-06-01T00:00:00.000Z",
};

test("a signed license verifies and a changed payload does not", async () => {
  const { privateSeed, publicKey } = await generateKeyPair();
  const file = await signPayload(privateSeed, buildPayload(base));
  assert.equal(await verifyLicense(publicKey, file), true);
  assert.equal(await verifyLicense(publicKey, { ...file, payload: file.payload.replace("pro", "basic") }), false);
  const other = await generateKeyPair();
  assert.equal(await verifyLicense(other.publicKey, file), false);
});

test("the payload carries the plan's computer count", () => {
  for (const [plan, { computers }] of Object.entries(PLANS)) {
    assert.equal(JSON.parse(buildPayload({ ...base, plan })).computers, computers);
  }
  assert.throws(() => buildPayload({ ...base, plan: "gold" }));
});

test("renewal starts at the previous expiry, not at the purchase date", () => {
  // Roadmap 10.8: previous expiry 2027-01-01, renewed on 2027-04-01 → the new term starts 2027-01-01.
  assert.equal(renewedExpiry("2027-01-01T00:00:00.000Z", "2027-04-01T00:00:00.000Z", 12), "2028-01-01T00:00:00.000Z");
  assert.equal(renewedExpiry("2027-01-01T00:00:00.000Z", "2026-12-01T00:00:00.000Z", 12), "2028-01-01T00:00:00.000Z");
  assert.equal(renewedExpiry(null, "2026-11-01T00:00:00.000Z", 12), "2027-11-01T00:00:00.000Z");
});

test("trial lasts 30 days", () => {
  assert.equal(trialExpiry("2026-11-01T00:00:00.000Z"), "2026-12-01T00:00:00.000Z");
});

test("activation codes are readable over the phone", () => {
  for (let i = 0; i < 200; i++) {
    const code = generateActivationCode();
    assert.match(code, /^ARTA-[A-Z2-9]{4}-[A-Z2-9]{4}-[A-Z2-9]{4}$/);
    assert.ok(!/[01OI]/.test(code.slice(5)), code);
    assert.equal(normalizeCode(code.toLowerCase().replaceAll("-", " ")), code);
  }
  assert.equal(normalizeCode("ARTA-7K2M-9QXP-4HT"), null);
  assert.equal(normalizeCode("ARTA-7K2M-9QXP-4HT0"), null);
});
