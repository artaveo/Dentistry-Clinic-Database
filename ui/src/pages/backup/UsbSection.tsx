import { useCallback, useEffect, useState } from "react";
import { CircleCheck, TriangleAlert, Unplug, Usb } from "lucide-react";
import type { CalendarSystem, UsbOverview, UsbVolumeInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { formatBytes, formatDateTime } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Badge, EmptyState, ErrorState, Loading, Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";

const POLL_MS = 3_000;

/**
 * Trusted USB backup drives (roadmap 10.3-10.4). A drive is recognised by the signed marker the app
 * writes on it, never by its drive letter. A new one is only offered; nothing is written to a drive
 * until the Owner presses "Use for backup".
 */
export function UsbSection({ canManage, calendar, onChanged }: { canManage: boolean; calendar?: CalendarSystem; onChanged: () => void }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const [data, setData] = useState<UsbOverview | null>(null);
  const [error, setError] = useState("");
  const [pick, setPick] = useState<UsbVolumeInfo | null>(null);
  const [forget, setForget] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    rpc("backup.usb", {})
      .then((d) => {
        setData(d);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    load();
    // A drive plugged in a moment ago appears by itself; no refresh button needed.
    const id = window.setInterval(() => !document.hidden && load(), POLL_MS);
    return () => window.clearInterval(id);
  }, [load]);

  const act = async (fn: () => Promise<UsbOverview>, done: string) => {
    setBusy(true);
    try {
      setData(await fn());
      toast.success(t(done));
      onChanged();
    } catch (e) {
      toast.error(err(e));
    } finally {
      setBusy(false);
      setPick(null);
      setForget(null);
    }
  };

  if (error && !data) return <ErrorState message={error} onRetry={load} />;
  if (!data) return <Loading />;
  const cloned = data.volumes.some((v) => v.verdict === "cloned");

  return (
    <div className="stack-lg" data-testid="usb-section">
      <Card>
        <CardHeader icon={Usb} title={t("bk.usb.title")} description={t("bk.usb.hint")} />
        {cloned && <Notice tone="danger" title={t("bk.usb.clonedTitle")} testId="usb-cloned">{t("bk.usb.clonedBody")}</Notice>}
        <ul className="plain-list" aria-live="polite" data-testid="usb-volumes">
          {data.volumes.map((v) => (
            <li key={v.mount} className="drive-row" data-testid="usb-volume">
              <Usb aria-hidden />
              <span className="drive-main">
                <strong>{[v.vendor, v.product].filter(Boolean).join(" ") || t("bk.usb.unnamed")}</strong>
                <span className="subtle t-caption">
                  <bdi className="ltr num">{v.mount}</bdi>
                  {v.label ? ` · ${v.label}` : ""}
                  {v.free_bytes !== null && ` · ${t("bk.usb.free").replace("{size}", formatBytes(v.free_bytes, lang))}`}
                </span>
              </span>
              {v.verdict === "trusted" && <Badge tone="success" icon={CircleCheck}>{t("bk.usb.trusted")}</Badge>}
              {v.verdict === "cloned" && <Badge tone="danger" icon={TriangleAlert}>{t("bk.usb.cloned")}</Badge>}
              {v.verdict === "new" && (
                <>
                  <Badge tone="info">{t("bk.usb.new")}</Badge>
                  {canManage && <Button variant="primary" onClick={() => setPick(v)} data-testid="usb-use">{t("bk.usb.use")}</Button>}
                </>
              )}
            </li>
          ))}
        </ul>
        {data.volumes.length === 0 && <EmptyState icon={Usb} title={t("bk.usb.none")}>{t("bk.usb.noneHint")}</EmptyState>}
      </Card>

      {data.trusted.length > 0 && (
        <Card flush>
          <div style={{ padding: "var(--space-5) var(--space-5) 0" }}>
            <CardHeader icon={CircleCheck} title={t("bk.usb.trustedTitle")} description={t("bk.usb.trustedHint")} />
          </div>
          <div className="table-wrap">
            <table className="table" data-testid="usb-trusted">
              <thead>
                <tr><th>{t("bk.usb.device")}</th><th>{t("bk.usb.serial")}</th><th>{t("bk.usb.since")}</th><th>{t("bk.usb.lastSeen")}</th><th>{t("lan.device.status")}</th><th /></tr>
              </thead>
              <tbody>
                {data.trusted.map((d) => (
                  <tr key={d.id} data-testid="usb-trusted-row">
                    <td className="cell-strong">{[d.vendor, d.product].filter(Boolean).join(" ") || d.label || "—"}</td>
                    <td className="cell-num">{d.serial_hint ? <bdi className="ltr">…{d.serial_hint}</bdi> : <span className="subtle">{t("bk.usb.noSerial")}</span>}</td>
                    <td className="cell-num">{formatDateTime(d.trusted_at, lang, calendar)}</td>
                    <td className="cell-num">{d.last_seen_at ? formatDateTime(d.last_seen_at, lang, calendar) : "—"}</td>
                    <td>{d.connected ? <Badge tone="success" dot>{t("lan.device.connected")}</Badge> : <Badge icon={Unplug}>{t("bk.usb.away")}</Badge>}</td>
                    <td className="cell-actions">{canManage && <Button variant="subtle" onClick={() => setForget(d.id)} data-testid="usb-forget">{t("bk.usb.forget")}</Button>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      )}

      {pick && (
        <Dialog
          title={t("bk.usb.useTitle")}
          description={t("bk.usb.useBody")}
          onClose={() => setPick(null)}
          footer={
            <>
              <Button variant="subtle" onClick={() => setPick(null)}>{t("common.cancel")}</Button>
              <Button variant="primary" loading={busy} onClick={() => act(() => rpc("backup.usb_trust", { mount: pick.mount }), "bk.usb.trustedDone")} data-testid="usb-use-confirm">{t("bk.usb.use")}</Button>
            </>
          }
        >
          <p className="muted" style={{ margin: 0 }}>
            <strong>{[pick.vendor, pick.product].filter(Boolean).join(" ")}</strong> · <bdi className="ltr num">{pick.mount}</bdi>
          </p>
        </Dialog>
      )}
      {forget && (
        <Dialog
          title={t("bk.usb.forgetTitle")}
          description={t("bk.usb.forgetBody")}
          onClose={() => setForget(null)}
          footer={
            <>
              <Button variant="subtle" onClick={() => setForget(null)}>{t("common.cancel")}</Button>
              <Button variant="danger" loading={busy} onClick={() => act(() => rpc("backup.usb_forget", { device_id: forget }), "bk.usb.forgotten")} data-testid="usb-forget-confirm">{t("bk.usb.forget")}</Button>
            </>
          }
        >
          <span />
        </Dialog>
      )}
    </div>
  );
}
