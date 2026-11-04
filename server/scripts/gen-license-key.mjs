// One-time: creates the license signing key pair (ADR-15).
//   node server/scripts/gen-license-key.mjs
// The PUBLIC key goes into core/license_public_key.txt (committed, shipped in the EXE).
// The PRIVATE seed goes ONLY into the Supabase secret LICENSE_SIGNING_KEY and the owner's
// password manager. It must never be committed or put in the EXE.
import { generateKeyPair } from "../supabase/functions/_shared/license.mjs";

const { privateSeed, publicKey } = await generateKeyPair();
console.log(JSON.stringify({ publicKey, privateSeed }, null, 2));
