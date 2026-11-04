// Regenerates server/test-vectors/license.json: a license signed by a throw-away TEST key.
// The Rust Core verifies this file in a unit test, which proves both sides agree on the format.
//   node server/scripts/make-test-vector.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { buildPayload, generateKeyPair, signPayload } from "../supabase/functions/_shared/license.mjs";

const { privateSeed, publicKey } = await generateKeyPair();
const payload = buildPayload({
  licenseId: "lic-test-1",
  clinicId: "clinic-test-1",
  installationId: "inst-test-1",
  kind: "paid",
  plan: "standard",
  startsAt: "2026-01-01T00:00:00.000Z",
  expiresAt: "2027-01-01T00:00:00.000Z",
  now: "2026-11-01T08:00:00.000Z",
});
const file = await signPayload(privateSeed, payload);
const tampered = { ...file, payload: payload.replace('"standard"', '"pro"') };

mkdirSync(new URL("../test-vectors/", import.meta.url), { recursive: true });
writeFileSync(
  new URL("../test-vectors/license.json", import.meta.url),
  JSON.stringify({ public_key: publicKey, valid: file, tampered }, null, 2) + "\n",
);
console.log("written");
