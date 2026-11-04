import { useCallback, useEffect, useState } from "react";
import { CircleAlert, CircleCheck, Copy, KeyRound, RefreshCw, ShieldAlert, ShieldCheck, WifiOff } from "lucide-react";
import type { CalendarSystem, LicenseState, LicenseStatus } from "../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../lib/api";
import { digits, formatDate, formatDateTime } from "../lib/dates";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { Badge, ErrorState, Loading, Notice } from "../ui/Feedback";
import { Field, TextInput, Textarea } from "../ui/Field";
import { Dialog } from "../ui/Overlay";
import { useToast } from "../ui/Toast";

type Tone = "success" | "info" | "warning" | "danger";
const TONE: Record<LicenseState, Tone> = {
  active: "success",
  trial: "info",
  unactivated: "info",
  pending: "info",
  expired_warning: "warning",
  grace: "warning",
  expired: "danger",
  revoked: "danger",
  blocked: "danger",
  clock_tampered: "danger",
  invalid: "danger",
  needs_verification: "danger",
};

/** The server's reason (`code_unknown`, `license_revoked`, ...) travels in the error detail after the code. */
function reason(e: unknown): string | null {
  return e instanceof ApiError && e.code === "license_rejected" ? (e.detail.split(": ").pop() ?? null) : null;
}

/**
 * License (roadmap 10.8-10.9): what state the clinic is in, how to start the trial, activate with a
 * code (online) or offline, check now, or hand the place to another computer. Never alarming about
 * the data: the page says plainly that patient records stay readable whatever happens.
 */
export function LicensePage({ calendar, canManage }: { calendar?: CalendarSystem; canManage: boolean }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [status, setStatus] = useState<LicenseStatus | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState<"" | "activate" | "verify">("");
  const [code, setCode] = useState("");
  const [codeError, setCodeError] = useState("");
  const [offline, setOffline] = useState(false);

  const load = useCallback(() => {
    rpc("license.status", {})
      .then((s) => {
        setStatus(s);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(load, [load]);

  const run = async (kind: typeof busy, fn: () => Promise<LicenseStatus>, done: string) => {
    setBusy(kind);
    try {
      setStatus(await fn());
      toast.success(t(done));
      window.dispatchEvent(new Event("artaveo:license-changed"));
    } catch (e) {
      const why = reason(e);
      if (kind === "activate" && why) setCodeError(t(`lic.reject.${why}`));
      else toast.error(why ? t(`lic.reject.${why}`) : err(e));
    } finally {
      setBusy("");
    }
  };

  if (error && !status) return <Page testId="page-license"><ErrorState message={error} onRetry={load} /></Page>;
  if (!status) return <Page testId="page-license"><Loading /></Page>;

  const n = (x: number) => digits(x, lang);
  const tone = TONE[status.state];
  const online = status.server_configured;
  const hasLicense = status.kind !== null;
  const Icon = tone === "success" ? ShieldCheck : tone === "danger" ? ShieldAlert : tone === "warning" ? CircleAlert : KeyRound;
  const headline = status.state === "trial" && status.days_left !== null
    ? t("lic.state.trial").replace("{n}", n(Math.max(status.days_left, 0)))
    : t(`lic.state.${status.state}`);

  const activate = (ev: React.FormEvent) => {
    ev.preventDefault();
    setCodeError("");
    if (!code.trim()) return setCodeError(t("rule.required"));
    run("activate", () => rpc("license.activate", { code }), "lic.activated");
  };

  return (
    <Page testId="page-license">
      <PageHeader
        title={t("lic.title")}
        description={t("lic.hint")}
        actions={canManage && online && hasLicense && (
          <Button icon={RefreshCw} loading={busy === "verify"} onClick={() => run("verify", () => rpc("license.verify", {}), "lic.verified")} data-testid="lic-verify">{t("lic.verify")}</Button>
        )}
      />

      <Card data-testid="lic-status">
        <div className={`lic-head tone-${tone}`}>
          <Icon aria-hidden />
          <div>
            <h2 className="t-title" data-testid="lic-headline">{headline}</h2>
            <p className="muted" style={{ margin: 0 }}>{t(`lic.state.${status.state}.hint`)}</p>
          </div>
          <span className="spacer" />
          <Badge tone={tone} testId="lic-state">{t(`lic.badge.${status.state}`)}</Badge>
        </div>
        {status.state === "pending" && (
          <Notice tone="info" title={t("lic.pending.title")} testId="lic-pending">{t("lic.pending.body").replace("{fp}", status.fingerprint_hint)}</Notice>
        )}
        {status.read_only && (
          <Notice tone="info" title={t("lic.readOnly.title")} testId="lic-readonly">
            <ul className="plain-list" style={{ marginTop: "var(--space-2)" }}>
              <li>{t("lic.readOnly.view")}</li>
              <li>{t("lic.readOnly.print")}</li>
              <li>{t("lic.readOnly.backup")}</li>
            </ul>
          </Notice>
        )}
        <div className="lan-facts" style={{ marginTop: "var(--space-5)" }}>
          <div className="lan-fact">
            <span className="lan-fact-label">{t("lic.plan")}</span>
            <span className="lan-fact-value" data-testid="lic-plan">{status.plan ? t(`lic.plan.${status.plan}`) : "—"}</span>
          </div>
          <div className="lan-fact">
            <span className="lan-fact-label">{t("lic.computers")}</span>
            <span className="lan-fact-value">{hasLicense ? t("lic.computersValue").replace("{n}", n(status.computers)) : "—"}</span>
          </div>
          <div className="lan-fact">
            <span className="lan-fact-label">{t("lic.validUntil")}</span>
            <span className="lan-fact-value">{status.expires_at ? formatDate(status.expires_at, lang, calendar) : "—"}</span>
            {status.days_left !== null && status.days_left >= 0 && <span className="subtle t-caption">{t("lic.daysLeft").replace("{n}", n(status.days_left))}</span>}
          </div>
          <div className="lan-fact">
            <span className="lan-fact-label">{t("lic.lastCheck")}</span>
            <span className="lan-fact-value">{status.verified_at ? formatDateTime(status.verified_at, lang, calendar) : "—"}</span>
            {status.grace_days_left !== null && <span className="subtle t-caption">{t("lic.graceLeft").replace("{n}", n(status.grace_days_left))}</span>}
          </div>
          <div className="lan-fact">
            <span className="lan-fact-label">{t("lic.fingerprint")}</span>
            <span className="lan-fact-value"><bdi className="ltr num" data-testid="lic-fingerprint">{status.fingerprint_hint}</bdi></span>
            <span className="subtle t-caption">{t("lic.fingerprintHint")}</span>
          </div>
          <div className="lan-fact">
            <span className="lan-fact-label">{t("lic.installation")}</span>
            <span className="lan-fact-value">
              <bdi className="ltr num" data-testid="lic-installation">{status.installation_id}</bdi>
            </span>
            <span className="subtle t-caption">{t("lic.installationHint")}</span>
          </div>
        </div>
      </Card>

      {canManage && (!hasLicense || status.kind === "trial" || status.read_only) && (
        <Card>
          <CardHeader icon={KeyRound} title={t("lic.activate.title")} description={t("lic.activate.hint")} />
          {!online && <Notice tone="warning" testId="lic-offline-build">{t("lic.noServer")}</Notice>}
          <form onSubmit={activate} noValidate className="lic-code-form">
            <Field label={t("lic.activate.code")} hint={t("lic.activate.codeHint")} error={codeError || null} className="code-input">
              <TextInput dir="ltr" placeholder="0A1B-2C3D-4E5F-6A7B-8C9D-0E1F" autoComplete="off" spellCheck={false} value={code} onChange={(x) => { setCode(x.target.value.toUpperCase()); setCodeError(""); }} data-testid="lic-code" />
            </Field>
            <div className="row">
              <Button type="submit" variant="primary" icon={ShieldCheck} loading={busy === "activate"} disabled={!online} data-testid="lic-activate">{t("lic.activate.go")}</Button>
              <Button variant="subtle" icon={WifiOff} onClick={() => setOffline(true)} data-testid="lic-offline-open">{t("lic.offline.open")}</Button>
            </div>
          </form>
        </Card>
      )}

      {offline && <OfflineDialog onClose={() => setOffline(false)} onApplied={(s) => { setStatus(s); setOffline(false); toast.success(t("lic.activated")); window.dispatchEvent(new Event("artaveo:license-changed")); }} />}
    </Page>
  );
}

/** Activation without internet (roadmap 10.8): copy a short code to the vendor, paste the answer back. */
function OfflineDialog({ onClose, onApplied }: { onClose: () => void; onApplied: (s: LicenseStatus) => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [request, setRequest] = useState("");
  const [response, setResponse] = useState("");
  const [problem, setProblem] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    rpc("license.offline_request", {}).then((r) => setRequest(r.request_code)).catch((e) => toast.error(err(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const copy = () => navigator.clipboard?.writeText(request).then(() => toast.success(t("lic.offline.copied")), () => {});
  const apply = async () => {
    setBusy(true);
    setProblem("");
    try {
      onApplied(await rpc("license.offline_apply", { response_code: response }));
    } catch (e) {
      setProblem(reason(e) ? t(`lic.reject.${reason(e)}`) : err(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      wide
      title={t("lic.offline.title")}
      description={t("lic.offline.hint")}
      onClose={onClose}
      testId="lic-offline-dialog"
      footer={
        <>
          <Button variant="subtle" onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" loading={busy} disabled={!response.trim()} icon={CircleCheck} onClick={apply} data-testid="lic-offline-apply">{t("lic.offline.apply")}</Button>
        </>
      }
    >
      <div className="stack">
        <Field label={t("lic.offline.step1")} hint={t("lic.offline.step1Hint")}>
          <Textarea readOnly rows={3} dir="ltr" value={request} onFocus={(x) => x.currentTarget.select()} data-testid="lic-offline-request" />
        </Field>
        <div className="row"><Button icon={Copy} onClick={copy}>{t("lic.offline.copy")}</Button></div>
        <Field label={t("lic.offline.step2")} hint={t("lic.offline.step2Hint")} error={problem || null}>
          <Textarea rows={4} dir="ltr" spellCheck={false} value={response} onChange={(x) => { setResponse(x.target.value); setProblem(""); }} data-testid="lic-offline-response" />
        </Field>
      </div>
    </Dialog>
  );
}
