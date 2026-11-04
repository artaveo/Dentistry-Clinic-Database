// POST { installation_id, clinic_name, computer_name, app_version, arch } → { license }
// A trial is given once per installation (roadmap 10.8). Asking again returns the same trial.
import { issuePaid, issueTrial } from "../_shared/issue.ts";
import { trialExpiry } from "../_shared/license.mjs";
import { admin, fail, installationId, json, logEvent, readBody, text } from "../_shared/server.ts";

Deno.serve(async (req) => {
  const body = await readBody(req);
  const key = installationId(body?.installation_id);
  if (!body || !key) return fail("bad_request");
  const db = admin();

  const { data: existing } = await db.from("installations").select("*").eq("installation_key", key).maybeSingle();
  if (existing) {
    if (existing.status === "blocked") return fail("installation_blocked", 403);
    if (existing.license_id) {
      const { data: lic } = await db.from("licenses").select("*").eq("id", existing.license_id).single();
      return json({ license: await issuePaid(lic, key) });
    }
    if (existing.trial_started_at) return json({ license: await issueTrial(existing) });
  }

  const now = new Date().toISOString();
  const clinicName = text(body.clinic_name);
  const computerName = text(body.computer_name);

  // The same clinic and computer asking again under a new installation id is a re-install trying
  // for a second trial: allowed to continue, but the vendor sees it.
  if (clinicName && computerName) {
    const { count } = await db.from("installations").select("id", { count: "exact", head: true })
      .eq("clinic_name", clinicName).eq("computer_name", computerName).not("trial_started_at", "is", null);
    if ((count ?? 0) > 0) await logEvent(db, "trial_repeat_suspect", key, null, { clinicName, computerName });
  }

  const row = {
    installation_key: key,
    clinic_name: clinicName,
    computer_name: computerName,
    app_version: text(body.app_version, 32),
    arch: text(body.arch, 16),
    status: "trial",
    trial_started_at: now,
    trial_expires_at: trialExpiry(now),
  };
  const { data: created, error } = await db.from("installations").upsert(row, { onConflict: "installation_key" })
    .select("*").single();
  if (error) return fail("server_error", 500);
  await logEvent(db, "trial_started", key, null, { clinicName });
  return json({ license: await issueTrial(created) });
});
