import { useEffect, useRef, useState } from "react";
import { rpc } from "./api";

/**
 * Unsent form drafts (OF-020, roadmap rule 14). A moment after the user changes a long form, its values
 * are saved in the Core (inside the encrypted database), so a locked screen, a minimised window or a
 * closed app does not lose them. When the form opens again an unfinished draft is offered; the draft is
 * removed when the form is saved or cancelled. Nothing is written until the user changes something.
 */
export function useFormDraft<T extends Record<string, string>>(formKey: string, values: T, enabled: boolean) {
  const [offer, setOffer] = useState<T | null>(null);
  const started = useRef(JSON.stringify(values));
  const timer = useRef<number | undefined>(undefined);
  // The newest change that has not been written yet.
  const pending = useRef<string | null>(null);

  const write = () => {
    window.clearTimeout(timer.current);
    if (pending.current === null) return;
    const json = pending.current;
    pending.current = null;
    rpc("drafts.save", { form_key: formKey, data_json: json }).catch(() => {});
  };
  // Kept current so the timer and the unmount cleanup always write through the latest form key.
  const writeRef = useRef(write);
  writeRef.current = write;

  useEffect(() => {
    if (!enabled) return;
    rpc("drafts.get", { form_key: formKey })
      .then((r) => {
        if (!r.data_json) return;
        try {
          setOffer(JSON.parse(r.data_json) as T);
        } catch {
          /* a damaged draft is ignored, the form starts empty */
        }
      })
      .catch(() => {});
  }, [formKey, enabled]);

  useEffect(() => {
    const json = JSON.stringify(values);
    if (!enabled || json === started.current) return;
    pending.current = json;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => writeRef.current(), 600);
  }, [values, enabled]);

  // Leaving the form saves what is still waiting, so the last keystrokes are never lost.
  useEffect(() => () => writeRef.current(), []);

  return {
    /** A draft found when the form opened, for the user to continue or discard. */
    offer,
    /** Hides the offer after the user chose what to do with it. */
    settle: () => setOffer(null),
    /** Removes the stored draft; call after a successful save or a cancel. */
    clear: () => {
      pending.current = null;
      window.clearTimeout(timer.current);
      if (enabled) rpc("drafts.delete", { form_key: formKey }).catch(() => {});
    },
  };
}
