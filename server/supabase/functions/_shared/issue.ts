// Builds and signs the license an installation receives. Used by trial, activate, verify and offline.
import { buildPayload, signPayload } from "./license.mjs";
import { signingKey } from "./server.ts";

// deno-lint-ignore no-explicit-any
type Row = Record<string, any>;

/** A paid license for `installation`, valid per `license`'s own dates. */
export async function issuePaid(license: Row, installationKey: string, now = new Date().toISOString()) {
  const payload = buildPayload({
    licenseId: license.id,
    clinicId: license.clinic_id,
    installationId: installationKey,
    kind: "paid",
    plan: license.plan,
    startsAt: new Date(license.starts_at).toISOString(),
    expiresAt: new Date(license.expires_at).toISOString(),
    now,
    offlineGraceDays: license.offline_grace_days,
  });
  return signPayload(signingKey(), payload);
}

/** The 30-day trial of one installation (it has no license row). */
export async function issueTrial(installation: Row, now = new Date().toISOString()) {
  const payload = buildPayload({
    licenseId: `trial-${installation.id}`,
    clinicId: installation.clinic_id ?? `trial-${installation.id}`,
    installationId: installation.installation_key,
    kind: "trial",
    plan: "trial",
    startsAt: new Date(installation.trial_started_at).toISOString(),
    expiresAt: new Date(installation.trial_expires_at).toISOString(),
    now,
  });
  return signPayload(signingKey(), payload);
}
