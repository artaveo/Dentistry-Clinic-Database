import { useCallback, useEffect, useState } from "react";
import { CircleCheck, CircleX, Laptop, MonitorOff, Network as NetworkIcon, Plus, Server, Trash2, Unplug, UserX } from "lucide-react";
import type { CalendarSystem, LanDevice, LanStatus } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { digits, formatDateTime } from "../lib/dates";
import { appMode, autostartEnabled, setAutostart, unpairFromServer, useLive, type AppMode } from "../lib/lan";
import { isDesktop } from "../print/engine";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { Switch } from "../ui/Controls";
import { Badge, EmptyState, ErrorState, Loading, Notice } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { useToast } from "../ui/Toast";

const POLL_MS = 2_000;

/**
 * Settings › Network (roadmap 7.1, 7.4, 7.6): make this computer the clinic server, add other
 * computers with a 6-digit code, see who is connected and remove a computer's access. On a client
 * computer it only shows the connection and lets it be disconnected.
 */
export function NetworkPage({ calendar, onChanged }: { calendar?: CalendarSystem; onChanged?: () => void }) {
  const { t, err } = useI18n();
  const [mode, setMode] = useState<AppMode | null>(null);
  useEffect(() => {
    appMode().then(setMode);
  }, []);
  if (!mode) return <Page testId="page-network"><Loading /></Page>;
  return (
    <Page testId="page-network">
      <PageHeader title={t("lan.title")} description={t("lan.subtitle")} />
      {mode.mode === "client" ? <ClientCard server={mode.server_name} /> : <ServerCards calendar={calendar} err={err} onChanged={onChanged} />}
    </Page>
  );
}

function ClientCard({ server }: { server: string }) {
  const { t } = useI18n();
  const toast = useToast();
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const disconnect = async () => {
    setBusy(true);
    try {
      await unpairFromServer();
      window.location.reload();
    } catch {
      toast.error(t("error.internal"));
      setBusy(false);
    }
  };
  return (
    <>
      <Card data-testid="lan-client-card">
        <CardHeader icon={Laptop} title={t("lan.client.title")} description={t("lan.client.hint").replace("{name}", server)} actions={<Button icon={Unplug} onClick={() => setConfirm(true)} data-testid="lan-disconnect">{t("lan.client.disconnect")}</Button>} />
      </Card>
      {confirm && (
        <Dialog
          title={t("lan.client.disconnectTitle")}
          description={t("lan.client.disconnectBody")}
          onClose={() => setConfirm(false)}
          footer={<><Button variant="subtle" onClick={() => setConfirm(false)}>{t("common.cancel")}</Button><Button variant="danger" loading={busy} onClick={disconnect} data-testid="lan-disconnect-confirm">{t("lan.client.disconnect")}</Button></>}
        >
          <span />
        </Dialog>
      )}
    </>
  );
}

function ServerCards({ calendar, err, onChanged }: { calendar?: CalendarSystem; err: (e: unknown) => string; onChanged?: () => void }) {
  const { t, lang } = useI18n();
  const toast = useToast();
  const [status, setStatus] = useState<LanStatus | null>(null);
  const [devices, setDevices] = useState<LanDevice[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [revoke, setRevoke] = useState<LanDevice | null>(null);
  const [turnOff, setTurnOff] = useState(false);
  const [autostart, setAuto] = useState<boolean | null>(null);

  const load = useCallback(() => {
    Promise.all([rpc("lan.status", {}), rpc("lan.devices", {})])
      .then(([s, d]) => {
        setStatus(s);
        setDevices(d);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    load();
    const id = window.setInterval(() => !document.hidden && load(), POLL_MS);
    return () => window.clearInterval(id);
  }, [load]);
  useLive(["lan"], load);
  useEffect(() => {
    autostartEnabled().then(setAuto);
  }, []);

  if (error && !status) return <ErrorState message={error} onRetry={load} />;
  if (!status) return <Loading />;

  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await fn();
      load();
      onChanged?.();
    } catch (e) {
      toast.error(err(e));
    } finally {
      setBusy(false);
    }
  };
  const full = status.paired_count >= status.client_limit;
  const starting = status.enabled && !status.running;
  const count = t("lan.computers.count").replace("{n}", digits(status.paired_count, lang)).replace("{limit}", digits(status.client_limit, lang));

  return (
    <>
      <Card>
        <CardHeader icon={Server} title={t("lan.server.title")} description={t("lan.server.hint")} />
        <Switch
          checked={status.enabled}
          disabled={busy}
          onChange={(on) => (on ? act(() => rpc("lan.enable", {})) : status.connected_count > 0 ? setTurnOff(true) : act(() => rpc("lan.disable", {})))}
          label={t("lan.server.switch")}
          testId="lan-switch"
        />
        <div className="lan-facts" style={{ marginTop: "var(--space-5)" }}>
          <div className="lan-fact">
            <span className="lan-fact-label">{t("lan.status.label")}</span>
            <span role="status" aria-atomic="true" data-testid="lan-status">
              {status.running ? <Badge tone="success" icon={CircleCheck}>{t("lan.status.running")}</Badge> : starting ? <Badge tone="info">{t("lan.status.starting")}</Badge> : <Badge icon={CircleX}>{t("lan.status.off")}</Badge>}
            </span>
          </div>
          <div className="lan-fact">
            <span className="lan-fact-label">{t("lan.computers.label")}</span>
            <span className="lan-fact-value" data-testid="lan-count">{count}</span>
          </div>
          {status.running && (
            <div className="lan-fact">
              <span className="lan-fact-label">{t("lan.address")}</span>
              <span className="lan-fact-value"><bdi className="ltr num" data-testid="lan-address">{(status.addresses.length ? status.addresses : [status.computer_name]).map((a) => `${a}:${status.port}`).join("  ·  ")}</bdi></span>
              <span className="subtle t-caption">{t("lan.addressHint")}</span>
            </div>
          )}
          {status.fingerprint && (
            <div className="lan-fact">
              <span className="lan-fact-label">{t("lan.fingerprint")}</span>
              <span className="lan-fact-value"><bdi className="ltr num">{status.fingerprint}</bdi></span>
              <span className="subtle t-caption">{t("lan.fingerprintHint")}</span>
            </div>
          )}
        </div>
        {status.enabled && <div style={{ marginTop: "var(--space-5)" }}><Notice tone="info">{t("lan.networkHint")}</Notice></div>}
      </Card>

      <Card flush>
        <div style={{ padding: "var(--space-5) var(--space-5) 0" }}>
          <CardHeader
            icon={NetworkIcon}
            title={t("lan.computers.title")}
            description={t("lan.computers.hint")}
            actions={<Button variant="primary" icon={Plus} disabled={!status.running || full || busy} onClick={() => act(() => rpc("lan.pairing_open", {}))} data-testid="lan-add">{t("lan.add")}</Button>}
          />
          {status.running && full && <Notice tone="warning">{t("lan.limitReached").replace("{n}", digits(status.client_limit, lang))}</Notice>}
        </div>
        {status.pairing && (
          <div style={{ padding: "var(--space-5)" }} data-testid="lan-pairing">
            <div className="pair-code">
              <span className="t-overline">{t("lan.pairing.title")}</span>
              <span className="pair-digits" data-testid="lan-code">{digits(status.pairing.code, lang)}</span>
              <span className="pair-meta" role="status" aria-atomic="true">
                {t("lan.pairing.valid").replace("{m}", digits(Math.max(1, Math.ceil(status.pairing.expires_in_seconds / 60)), lang)).replace("{n}", digits(status.pairing.attempts_left, lang))}
              </span>
              <p className="muted" style={{ margin: 0 }}>{t("lan.pairing.how")}</p>
              <Button variant="subtle" onClick={() => act(() => rpc("lan.pairing_cancel", {}))} data-testid="lan-pairing-cancel">{t("common.cancel")}</Button>
            </div>
          </div>
        )}
        <div className="table-wrap">
          <table className="table" data-testid="lan-devices">
            <thead>
              <tr><th>{t("lan.device.name")}</th><th>{t("lan.device.user")}</th><th>{t("lan.device.lastSeen")}</th><th>{t("lan.device.status")}</th><th /></tr>
            </thead>
            <tbody>
              {devices.map((d) => (
                <tr key={d.id} data-testid="lan-device">
                  <td className="cell-strong">{d.name}</td>
                  <td>{d.last_user ?? "—"}</td>
                  <td className="cell-num">{d.last_seen_at ? formatDateTime(d.last_seen_at, lang, calendar ?? "shamsi") : "—"}</td>
                  <td>
                    {d.revoked ? <Badge tone="danger" icon={UserX}>{t("lan.device.revoked")}</Badge> : d.connected ? <Badge tone="success" dot>{t("lan.device.connected")}</Badge> : <Badge icon={MonitorOff}>{t("lan.device.offline")}</Badge>}
                  </td>
                  <td className="cell-actions">{!d.revoked && <Button variant="subtle" icon={Trash2} onClick={() => setRevoke(d)} data-testid="lan-revoke">{t("lan.device.revoke")}</Button>}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {devices.length === 0 && <EmptyState icon={Laptop} title={t("lan.devices.empty")}>{t("lan.devices.emptyHint")}</EmptyState>}
        </div>
      </Card>

      {isDesktop() && (
        <Card>
          <CardHeader icon={Server} title={t("lan.background.title")} description={t("lan.background.hint")} />
          <Switch
            checked={autostart === true}
            disabled={autostart === null}
            onChange={(on) => setAutostart(on).then(() => setAuto(on)).catch(() => toast.error(t("error.internal")))}
            label={t("lan.autostart")}
            testId="lan-autostart"
          />
        </Card>
      )}

      {revoke && (
        <Dialog
          title={t("lan.revoke.title").replace("{name}", revoke.name)}
          description={t("lan.revoke.body")}
          onClose={() => setRevoke(null)}
          footer={<><Button variant="subtle" onClick={() => setRevoke(null)}>{t("common.cancel")}</Button><Button variant="danger" loading={busy} onClick={() => act(() => rpc("lan.revoke", { id: revoke.id })).then(() => setRevoke(null))} data-testid="lan-revoke-confirm">{t("lan.device.revoke")}</Button></>}
        >
          <span />
        </Dialog>
      )}
      {turnOff && (
        <Dialog
          title={t("lan.off.title")}
          description={t("lan.off.body").replace("{n}", digits(status.connected_count, lang))}
          onClose={() => setTurnOff(false)}
          footer={<><Button variant="subtle" onClick={() => setTurnOff(false)}>{t("common.cancel")}</Button><Button variant="danger" loading={busy} onClick={() => act(() => rpc("lan.disable", {})).then(() => setTurnOff(false))} data-testid="lan-off-confirm">{t("lan.off.confirm")}</Button></>}
        >
          <span />
        </Dialog>
      )}
    </>
  );
}
