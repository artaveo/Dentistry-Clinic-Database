// POST { installation_id } → { ok: true }
// Self-service transfer (roadmap 10.8): the old computer gives its place back so a new one can
// activate with a new code. Limited to 3 per license per year; beyond that the vendor does it by hand.
import { admin, fail, installationId, json, logEvent, readBody } from "../_shared/server.ts";

const MAX_PER_YEAR = 3;

Deno.serve(async (req) => {
  const body = await readBody(req);
  const key = installationId(body?.installation_id);
  if (!body || !key) return fail("bad_request");
  const db = admin();

  const { data: inst } = await db.from("installations").select("*").eq("installation_key", key).maybeSingle();
  if (!inst || !inst.license_id) return fail("installation_unknown", 404);
  if (inst.status !== "active") return fail("installation_blocked", 403);

  const since = new Date(Date.now() - 365 * 86_400_000).toISOString();
  const { count } = await db.from("security_events").select("id", { count: "exact", head: true })
    .eq("kind", "released").eq("clinic_id", inst.clinic_id).gte("at", since);
  if ((count ?? 0) >= MAX_PER_YEAR) return fail("release_limit", 429);

  // The activation code that brought this computer gets its place back too.
  const { data: code } = await db.from("activation_codes").select("*").eq("license_id", inst.license_id).gt("used", 0)
    .order("created_at", { ascending: false }).limit(1).maybeSingle();
  if (code) await db.from("activation_codes").update({ used: Math.max(0, code.used - 1) }).eq("id", code.id);

  await db.from("installations").update({ status: "released", license_id: null }).eq("id", inst.id);
  await logEvent(db, "released", key, inst.clinic_id, { license: inst.license_id });
  return json({ ok: true });
});
