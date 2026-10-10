import { useEffect, useState } from "react";
import type { ClinicProfile } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";

/** What every printed document's letterhead and footer need (2.1b): the clinic, its logo, the brand-mark setting. */
export type Letterhead = { clinic: ClinicProfile; logo: string | null; brandFooter: boolean };

let cache: Promise<Letterhead> | null = null;

export function invalidateLetterhead() {
  cache = null;
}

export function useLetterhead(): Letterhead | null {
  const [v, setV] = useState<Letterhead | null>(null);
  useEffect(() => {
    let live = true;
    cache ??= Promise.all([rpc("clinic.get", {}), rpc("app.clinic_logo", {}), rpc("settings.get", {})])
      .then(([clinic, logo, settings]) => ({ clinic, logo: logo.data_url, brandFooter: settings.print_brand_footer }))
      .catch((e) => {
        cache = null;
        throw e;
      });
    cache.then((x) => live && setV(x)).catch(() => {});
    return () => {
      live = false;
    };
  }, []);
  return v;
}
