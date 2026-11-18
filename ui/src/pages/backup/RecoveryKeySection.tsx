import { useState } from "react";
import { createPortal } from "react-dom";
import { KeyRound, LockKeyhole, Printer } from "lucide-react";
import type { CalendarSystem, RegenerateRecoveryKeyResult } from "../../../../shared/ts/contract";
import { ApiError, rpc } from "../../lib/api";
import { digits, formatDateTime } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Checkbox } from "../../ui/Controls";
import { Notice } from "../../ui/Feedback";
import { Field, PasswordInput } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";

/**
 * Regenerate the Recovery Key (Owner only). The key from setup is shown once and never stored, so a
 * lost paper can only be replaced, not looked up. The Owner confirms with their password; the new key is
 * shown once, with the same print sheet as the setup wizard, and the dialog cannot be closed before the
 * Owner confirms they stored it.
 */
export function RecoveryKeySection({ clinicName, calendar }: { clinicName: string; calendar?: CalendarSystem }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <div className="stack-lg" data-testid="recovery-key-section">
      <Card>
        <CardHeader icon={KeyRound} title={t("rk.title")} description={t("rk.hint")} />
        <p className="subtle">{t("rk.why")}</p>
        <div className="row">
          <Button variant="primary" icon={KeyRound} onClick={() => setOpen(true)} data-testid="rk-open">{t("rk.make")}</Button>
        </div>
      </Card>
      {open && <RegenerateDialog clinicName={clinicName} calendar={calendar} onClose={() => setOpen(false)} />}
    </div>
  );
}

function RegenerateDialog({ clinicName, calendar, onClose }: { clinicName: string; calendar?: CalendarSystem; onClose: () => void }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const [password, setPassword] = useState("");
  const [passwordError, setPasswordError] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<RegenerateRecoveryKeyResult | null>(null);
  const [confirmed, setConfirmed] = useState(false);

  const make = async () => {
    if (!password) return setPasswordError(t("rule.required"));
    setBusy(true);
    setPasswordError("");
    try {
      setResult(await rpc("backup.regenerate_key", { password }));
      setPassword("");
    } catch (e) {
      if (e instanceof ApiError && e.field === "password") setPasswordError(t(e.rule ? `rule.${e.rule}` : "rule.wrong_password"));
      else toast.error(err(e));
    } finally {
      setBusy(false);
    }
  };

  // Once the new key exists it is the only copy on screen: closing waits for the Owner's confirmation.
  const close = () => (result && !confirmed ? toast.error(t("rk.mustConfirm")) : onClose());
  const n = (x: number) => digits(x, lang);
  const line = (key: string, count: number) => t(key).replace("{n}", n(count));

  return (
    <Dialog
      title={result ? t("rk.new.title") : t("rk.dialog.title")}
      onClose={close}
      testId="rk-dialog"
      footer={
        result ? (
          <Button variant="primary" disabled={!confirmed} onClick={onClose} data-testid="rk-close">{t("rk.close")}</Button>
        ) : (
          <>
            <Button onClick={onClose} data-testid="rk-cancel">{t("common.cancel")}</Button>
            <Button variant="danger" icon={KeyRound} loading={busy} onClick={make} data-testid="rk-confirm">{t("rk.make")}</Button>
          </>
        )
      }
    >
      {!result ? (
        <form className="stack" onSubmit={(e) => { e.preventDefault(); void make(); }}>
          <p>{t("rk.confirm.lead")}</p>
          <ul className="plain-list">
            <li>{t("rk.confirm.paper")}</li>
            <li>{t("rk.confirm.backups")}</li>
            <li>{t("rk.confirm.away")}</li>
          </ul>
          <Field label={t("rk.password")} hint={t("rk.passwordHint")} error={passwordError}>
            <PasswordInput icon={LockKeyhole} autoComplete="current-password" value={password} invalid={!!passwordError} onChange={(e) => { setPassword(e.target.value); setPasswordError(""); }} data-testid="rk-password" />
          </Field>
        </form>
      ) : (
        <div className="stack">
          <Notice tone="warning" title={t("recovery.warningTitle")}>{t("recovery.warning")}</Notice>
          <div className="recovery-key" data-testid="rk-key">{result.recovery_key}</div>
          <div className="row">
            <Button icon={Printer} onClick={() => window.print()} data-testid="rk-print">{t("recovery.print")}</Button>
          </div>
          <ul className="plain-list subtle" data-testid="rk-result">
            <li>{line("rk.result.local", result.local_updated)}</li>
            {result.usb_updated > 0 && <li>{line("rk.result.usb", result.usb_updated)}</li>}
            {result.cloud_updated > 0 && <li>{line("rk.result.cloud", result.cloud_updated)}</li>}
            {result.failed > 0 && <li>{line("rk.result.failed", result.failed)}</li>}
          </ul>
          <Notice tone="info">{t("rk.result.away")}</Notice>
          <Checkbox checked={confirmed} onChange={setConfirmed} testId="rk-confirmed">{t("recovery.confirm")}</Checkbox>
          {/* On paper this block is the only thing printed; it sits on <body> so the dialog's own layout cannot clip it. */}
          {createPortal(
            <div className="print-only" data-testid="rk-sheet">
              <h1>{t("recovery.sheetTitle")}</h1>
              <p>{t("recovery.sheetClinic")}: {clinicName}</p>
              <p>{t("recovery.sheetDate")}: {formatDateTime(new Date().toISOString(), lang, calendar)}</p>
              <div className="recovery-key">{result.recovery_key}</div>
              <p>{t("recovery.warning")}</p>
            </div>,
            document.body,
          )}
        </div>
      )}
    </Dialog>
  );
}
