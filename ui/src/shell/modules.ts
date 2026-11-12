import { useClinicSettings } from "../lib/clinical";

/**
 * Which optional parts of the app the clinic switched on (Settings → general). Read from the one cached
 * `settings.get`, so there is no extra request. Until it has loaded both are false: an optional part appears
 * once the answer is known and never flashes for a clinic that has it off.
 */
export function useModules(): { ready: boolean; perio: boolean; stock: boolean } {
  const s = useClinicSettings();
  return { ready: s !== null, perio: s?.show_perio_chart ?? false, stock: s?.use_stock_module ?? false };
}
