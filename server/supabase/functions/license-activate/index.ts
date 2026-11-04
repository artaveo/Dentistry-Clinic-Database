// POST { installation_id, code, clinic_name, computer_name, app_version, arch } → { license }
// The activation code is only permission to receive a license (roadmap 10.8), never a license itself.
import { issuePaid } from "../_shared/issue.ts";
import { normalizeCode, sha256Hex } from "../_shared/license.mjs";
import { admin, fail, installationId, json, logEvent, readBody, text } from "../_shared/server.ts";

Deno.serve(async (req) => {
  const body = await readBody(req);
  const key = installationId(body?.installation_id);
  const code = normalizeCode(body?.code);
  if (!body || !key) return fail("bad_request");
  if (!code) return fail("code_format");
  const db = admin();

  const hash = await sha256Hex(code);
  const { data: ac } = await db.from("activation_codes").select("*").eq("code_hash", hash).maybeSingle();
  if (!ac) {
    await logEvent(db, "activation_unknown_code", key, null, { hint: code.slice(0, 9) });
    return fail("code_unknown", 404);
  }
  if (ac.revoked_at) return fail("code_revoked", 403);
  if (new Date(ac.expires_at) < new Date()) return fail("code_expired", 403);

  const { data: lic } = await db.from("licenses").select("*").eq("id", ac.license_id).single();
  if (lic.status === "revoked") return fail("license_revoked", 403);

  const { data: inst } = await db.from("installations").select("*").eq("installation_key", key).maybeSingle();
  if (inst?.status === "blocked") return fail("installation_blocked", 403);

  const alreadyHere = inst?.license_id === lic.id && inst.status === "active";
  if (!alreadyHere) {
    if (ac.used >= ac.max_uses) {
      await logEvent(db, "activation_limit", key, ac.clinic_id, { license: lic.id });
      return fail("code_used_up", 409);
    }
    await db.from("activation_codes").update({ used: ac.used + 1 }).eq("id", ac.id);
  }

  await db.from("installations").upsert({
    installation_key: key,
    clinic_id: ac.clinic_id,
    license_id: lic.id,
    clinic_name: text(body.clinic_name),
    computer_name: text(body.computer_name),
    app_version: text(body.app_version, 32),
    arch: text(body.arch, 16),
    status: "active",
    last_seen: new Date().toISOString(),
  }, { onConflict: "installation_key" });
  await logEvent(db, "activated", key, ac.clinic_id, { license: lic.id });
  return json({ license: await issuePaid(lic, key) });
});
