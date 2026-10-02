import { useEffect, useState } from "react";
import type { LabeledItem } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { useI18n } from "../../i18n";

/** Reference data (gender, referral source, relationship, …) for the current language (1.5). */
export function useReferenceList(typeCode: string): LabeledItem[] {
  const { lang } = useI18n();
  const [items, setItems] = useState<LabeledItem[]>([]);
  useEffect(() => {
    let live = true;
    rpc("reference.list", { type_code: typeCode, language: lang }).then((v) => live && setItems(v)).catch(() => live && setItems([]));
    return () => {
      live = false;
    };
  }, [typeCode, lang]);
  return items;
}
