import { useEffect, useState } from "react";
import type { CatalogInfo, DoctorInfo, Language, ServiceInfo, SpecialtyInfo } from "../../../shared/ts/contract";
import { rpc } from "./api";
import { tr } from "./medical";

/**
 * Reference lists of Phase 5A shared by many screens (specialties M2, the service catalog M3), each
 * fetched once per session; `invalidate…` after an edit so the next screen sees the change.
 */
function cached<T>(load: () => Promise<T>) {
  let cache: Promise<T> | null = null;
  const listeners = new Set<() => void>();
  return {
    invalidate: () => {
      cache = null;
      listeners.forEach((l) => l());
    },
    use: (): T | null => {
      const [value, setValue] = useState<T | null>(null);
      const [tick, setTick] = useState(0);
      useEffect(() => {
        const bump = () => setTick((x) => x + 1);
        listeners.add(bump);
        return () => {
          listeners.delete(bump);
        };
      }, []);
      useEffect(() => {
        let live = true;
        cache ??= load().catch((e) => {
          cache = null;
          throw e;
        });
        cache.then((v) => live && setValue(v)).catch(() => {});
        return () => {
          live = false;
        };
      }, [tick]);
      return value;
    },
  };
}

const specialties = cached(() => rpc("specialties.list", { include_inactive: true }));
const catalog = cached(() => rpc("catalog.get", { include_inactive: false }));

/** Every specialty, switched-off ones included (a doctor keeps a specialty the clinic switched off). */
export const useSpecialties = specialties.use;
export const invalidateSpecialties = specialties.invalidate;
/** The active service catalog (categories, services and variants). */
export const useCatalog = catalog.use;
export const invalidateCatalog = catalog.invalidate;

/** "Orthodontics، Oral surgery" — a doctor's specialties in `lang`, or the old free text from before v0.5.0. */
export function specialtiesText(d: DoctorInfo, list: SpecialtyInfo[] | null, lang: Language): string {
  const names = d.specialty_ids.map((id) => tr(list?.find((s) => s.id === id)?.label, lang)).filter(Boolean);
  if (names.length) return names.join(lang === "en" ? ", " : "، ");
  return d.specialty ?? "";
}

/** A service's full name: "Root canal treatment — Two canals" for a variant. */
export function serviceName(s: ServiceInfo, cat: CatalogInfo | null, lang: Language): string {
  const parent = s.parent_id ? cat?.services.find((x) => x.id === s.parent_id) : null;
  return parent ? `${tr(parent.name, lang)} — ${tr(s.name, lang)}` : tr(s.name, lang);
}
