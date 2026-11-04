// POST { installation_id, app_version, last_backup_at, last_backup_ok } → { license } | { revoked: true }
// Called by the app about every 7 days while online. Each answer is a freshly signed license, so
// `verified_at` moves forward; a renewal or a revocation entered in the Vendor Panel arrives here.
import { issuePaid, issueTrial } from "../_shared/issue.ts";
import { admin, fail, installationId, json, logEvent, readBody, text } from "../_shared/server.ts";

Deno.serve(async (req) => {
  const body = await readBody(req);
  const key = installationId(body?.installation_id);
  if (!body || !key) return fail("bad_request");
  const db = admin();

  const { data: inst } = await db.from("installations").select("*").eq("installation_key", key).maybeSingle();
  if (!inst) {
    await logEvent(db, "unknown_installation", key, null, {});
    return fail("installation_unknown", 404);
  }
  if (inst.status === "blocked" || inst.status === "released") {
    return json({ revoked: true, reason: inst.status });
  }

  const lastBackup = typeof body.last_backup_at === "string" ? new Date(body.last_backup_at) : null;
  await db.from("installations").update({
    last_seen: new Date().toISOString(),
    app_version: text(body.app_version, 32) ?? inst.app_version,
    last_backup_at: lastBackup && !isNaN(lastBackup.getTime()) ? lastBackup.toISOString() : inst.last_backup_at,
    last_backup_ok: typeof body.last_backup_ok === "boolean" ? body.last_backup_ok : inst.last_backup_ok,
  }).eq("id", inst.id);

  if (!inst.license_id) {
    if (!inst.trial_started_at) return fail("installation_unknown", 404);
    return json({ license: await issueTrial(inst) });
  }
  const { data: lic } = await db.from("licenses").select("*").eq("id", inst.license_id).single();
  if (lic.status === "revoked") {
    await logEvent(db, "verify_revoked", key, lic.clinic_id, { license: lic.id });
    return json({ revoked: true, reason: "license_revoked" });
  }
  return json({ license: await issuePaid(lic, key) });
});
