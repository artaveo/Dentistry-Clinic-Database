// POST (vendor only, Bearer = the vendor's Supabase session) { request_code, license_id } → { response_code }
// Offline activation (roadmap 10.8): the clinic has no internet. The app shows a request code, the
// vendor pastes it in the Vendor Panel, and the code that comes back is the signed license itself.
import { issuePaid } from "../_shared/issue.ts";
import { admin, fail, json, logEvent, readBody, text } from "../_shared/server.ts";

Deno.serve(async (req) => {
  const body = await readBody(req);
  if (!body) return fail("bad_request");
  const db = admin();

  const jwt = (req.headers.get("authorization") ?? "").replace(/^Bearer\s+/i, "");
  const { data: auth } = await db.auth.getUser(jwt);
  if (!auth?.user) return fail("unauthorized", 401);
  const { data: vendor } = await db.from("vendor_users").select("user_id").eq("user_id", auth.user.id).maybeSingle();
  if (!vendor) return fail("unauthorized", 403);

  let request: Record<string, unknown>;
  try {
    request = JSON.parse(atob(String(body.request_code ?? "").replace(/-/g, "+").replace(/_/g, "/")));
  } catch {
    return fail("request_code_invalid");
  }
  const key = text(request.installation_id, 64);
  if (!key || !/^[A-Za-z0-9-]{16,64}$/.test(key)) return fail("request_code_invalid");

  const { data: lic } = await db.from("licenses").select("*").eq("id", String(body.license_id ?? "")).maybeSingle();
  if (!lic) return fail("license_unknown", 404);
  if (lic.status === "revoked") return fail("license_revoked", 403);

  await db.from("installations").upsert({
    installation_key: key,
    clinic_id: lic.clinic_id,
    license_id: lic.id,
    clinic_name: text(request.clinic_name),
    computer_name: text(request.computer_name),
    app_version: text(request.app_version, 32),
    status: "active",
    last_seen: new Date().toISOString(),
  }, { onConflict: "installation_key" });
  await logEvent(db, "activated_offline", key, lic.clinic_id, { license: lic.id, by: auth.user.id });

  const file = await issuePaid(lic, key);
  return json({ response_code: btoa(JSON.stringify(file)) });
});
