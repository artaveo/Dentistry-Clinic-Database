import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { Switch } from "./Controls";
import { Dialog } from "./Overlay";

/**
 * Shared confirmation dialog (replaces `window.confirm`). Same props as the local one in
 * PlansTab (`title`, `text`, `switchLabel`, `confirm`, `onClose`, `onConfirm(flag)`, `testId`), so
 * it can be swapped in; everything else is optional. It never closes itself: the caller closes in
 * `onClose` / after `onConfirm` (the `useConfirm` hook does both). Focus starts on the first button,
 * which is Cancel, so Enter never confirms a deletion by accident.
 */
export function ConfirmDialog({ title, text, switchLabel, switchDefault = true, confirm, cancel, danger, onClose, onConfirm, testId = "confirm-dialog" }: {
  title: string;
  text?: ReactNode;
  /** An extra yes/no switch under the text (its value is passed to `onConfirm`). */
  switchLabel?: string;
  switchDefault?: boolean;
  /** Label of the confirm button (default "Confirm"). */
  confirm?: string;
  cancel?: string;
  /** Red confirm button, for deleting or voiding. */
  danger?: boolean;
  onClose: () => void;
  onConfirm: (flag: boolean) => unknown | Promise<unknown>;
  testId?: string;
}) {
  const { t } = useI18n();
  const [flag, setFlag] = useState(switchDefault);
  const [busy, setBusy] = useState(false);
  const mounted = useRef(true);
  useEffect(() => () => {
    mounted.current = false;
  }, []);
  const run = () => {
    setBusy(true);
    Promise.resolve()
      .then(() => onConfirm(flag))
      .catch(() => {})
      .finally(() => mounted.current && setBusy(false));
  };
  return (
    <Dialog
      title={title}
      onClose={onClose}
      testId={testId}
      footer={
        <>
          <Button onClick={onClose} data-testid={`${testId}-cancel`}>{cancel ?? t("common.cancel")}</Button>
          <Button variant={danger ? "danger" : "primary"} loading={busy} onClick={run} data-testid={`${testId}-confirm`}>{confirm ?? t("ui.confirm.ok")}</Button>
        </>
      }
    >
      <div className="stack">
        {text && <p>{text}</p>}
        {switchLabel && <Switch checked={flag} onChange={setFlag} label={switchLabel} testId={`${testId}-switch`} />}
      </div>
    </Dialog>
  );
}

export type ConfirmOptions = {
  title: string;
  text?: ReactNode;
  confirm?: string;
  cancel?: string;
  danger?: boolean;
  testId?: string;
};

/**
 * `window.confirm` replacement:
 *
 *   const { confirm, dialog } = useConfirm();
 *   …  if (!(await confirm({ title: "…", text: "…", danger: true }))) return;
 *   …  return <>{…}{dialog}</>;
 *
 * `confirm()` resolves `true` on the confirm button and `false` on Cancel, X, Escape or when the
 * component unmounts. Render `dialog` once anywhere in the component's output.
 */
export function useConfirm(): { confirm: (options: ConfirmOptions) => Promise<boolean>; dialog: ReactNode } {
  const [req, setReq] = useState<(ConfirmOptions & { resolve: (ok: boolean) => void }) | null>(null);
  const live = useRef(req);
  live.current = req;
  useEffect(() => () => live.current?.resolve(false), []);
  const confirm = useCallback(
    (options: ConfirmOptions) =>
      new Promise<boolean>((resolve) => {
        live.current?.resolve(false);
        setReq({ ...options, resolve });
      }),
    [],
  );
  const finish = (ok: boolean) => {
    req?.resolve(ok);
    setReq(null);
  };
  const dialog = req ? (
    <ConfirmDialog title={req.title} text={req.text} confirm={req.confirm} cancel={req.cancel} danger={req.danger} testId={req.testId} onClose={() => finish(false)} onConfirm={() => finish(true)} />
  ) : null;
  return { confirm, dialog };
}
