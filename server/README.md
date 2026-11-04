# Artaveo central services (Phase 10)

Licenses, activation and update metadata live in a **free Supabase project** (ADR-16). It never holds
patient data (ADR-17). The desktop app only knows the public verification key and the public project URL.

```text
supabase/migrations/   tables + Row Level Security
supabase/functions/    Edge Functions the app calls (trial, activate, verify, release) + license-offline for the vendor
supabase/functions/_shared/license.mjs   license format, signing, codes (also run by `node --test` in CI)
vendor-panel/          static web page for Artaveo staff (host `server/` as-is, e.g. on Cloudflare Pages)
test-vectors/          a license signed by a throw-away key; the Rust Core verifies it in its own tests
scripts/               gen-license-key.mjs (one time), make-test-vector.mjs
```

## One-time setup (owner)

1. Create a Supabase project (free plan). Apply `supabase/migrations/*.sql` (SQL editor or `supabase db push`).
2. Put the **private signing key** (the file `server/.secrets/license-signing-key.txt`, never committed) in the
   function secrets, then store a copy in your password manager and delete the file from the laptop:
   `supabase secrets set LICENSE_SIGNING_KEY=<contents of the file>`
3. Deploy the functions with `--no-verify-jwt` (the app has no user session; `license-offline` checks the vendor itself):
   `supabase functions deploy license-trial license-activate license-verify license-release license-offline --no-verify-jwt`
4. Create the first vendor user in Supabase Auth, then run in the SQL editor:
   `insert into vendor_users(user_id, display_name) values ('<auth user id>', 'Your name');`
5. Fill `vendor-panel/config.js` with the project URL and the **publishable** key. Never put the service-role key there.
6. Build the desktop app with the project URL: `ARTAVEO_LICENSE_URL=https://<project>.supabase.co/functions/v1` and
   `ARTAVEO_LICENSE_ANON_KEY=<publishable key>` set at build time (see `core/src/license.rs`).

The public verification key is `core/license_public_key.txt`. If the private key is ever lost, no new license can be
signed and a new key pair means a new app release: keep the backup.
