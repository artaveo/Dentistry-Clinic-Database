// Helpers shared by the Edge Functions. Secrets come from the function environment only
// (`supabase secrets set LICENSE_SIGNING_KEY=...`); nothing secret is ever in the app (roadmap 10.10).
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.45.4";

export const admin = () =>
  createClient(Deno.env.get("SUPABASE_URL")!, Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!, {
    auth: { persistSession: false },
  });

export const signingKey = () => {
  const key = Deno.env.get("LICENSE_SIGNING_KEY");
  if (!key) throw new Error("LICENSE_SIGNING_KEY is not set");
  return key;
};

export const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

export const fail = (error: string, status = 400) => json({ error }, status);

/** Reads and checks the small JSON the app sends. */
export async function readBody(req: Request): Promise<Record<string, unknown> | null> {
  if (req.method !== "POST") return null;
  try {
    const text = await req.text();
    if (text.length > 4096) return null;
    const body = JSON.parse(text);
    return body && typeof body === "object" ? body : null;
  } catch {
    return null;
  }
}

export const text = (v: unknown, max = 120): string | null =>
  typeof v === "string" && v.trim().length > 0 ? v.trim().slice(0, max) : null;

export const installationId = (v: unknown): string | null => {
  const s = text(v, 64);
  return s && /^[A-Za-z0-9-]{16,64}$/.test(s) ? s : null;
};

export async function logEvent(
  db: ReturnType<typeof admin>,
  kind: string,
  installation_key: string | null,
  clinic_id: string | null,
  detail: Record<string, unknown> = {},
) {
  await db.from("security_events").insert({ kind, installation_key, clinic_id, detail });
}
