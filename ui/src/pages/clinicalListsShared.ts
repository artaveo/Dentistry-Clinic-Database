// Helpers shared by the tabs of «فهرست‌های بالینی» (Phase 5A lists and the Phase 5B ones).
import { useEffect, useState } from "react";
import type { Translations } from "../../../shared/ts/contract";
import { ApiError, isSessionError } from "../lib/api";
import { useI18n } from "../i18n";

export const emptyTranslations: Translations = { fa: "", ps: "", en: "" };
export const hasText = (x: Translations) => !!(x.fa.trim() || x.ps.trim() || x.en.trim());

/** Error text of a failed save: the rule's message, else the code's. */
export function useFailure() {
  const { t, err } = useI18n();
  return (x: unknown) => (x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
}

/** A list loaded once, reloadable after an edit. */
export function useList<T>(load: () => Promise<T[]>) {
  const { err } = useI18n();
  const [items, setItems] = useState<T[] | null>(null);
  const [error, setError] = useState("");
  const reload = () => load().then((v) => { setItems(v); setError(""); }).catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    reload();
  }, []);
  return { items, error, reload };
}
