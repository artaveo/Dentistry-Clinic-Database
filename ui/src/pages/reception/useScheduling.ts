import { useCallback, useEffect, useState } from "react";
import type { ChairInfo, DoctorInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";

/**
 * Solo Clinic Mode (roadmap 2.6): a clinic set up as "solo" with a single doctor gets the simple UI
 * (no doctor picker, no doctor column, no doctor filter). Adding a second doctor switches every screen
 * to the multi-doctor UI by itself, with no migration — the data is the same either way.
 */
export function isSolo(clinic: { clinic_mode: string } | null | undefined, doctors: unknown[] | null | undefined): boolean {
  return clinic?.clinic_mode === "solo" && (doctors?.length ?? 0) <= 1;
}

export type Perms = { edit: boolean; treat: boolean; createPatients: boolean; manageDoctors: boolean };

/** The clinic's active doctors and chairs, for the calendar, the queue and the booking form. */
export function useScheduling() {
  const [doctors, setDoctors] = useState<DoctorInfo[] | null>(null);
  const [chairs, setChairs] = useState<ChairInfo[]>([]);
  const [error, setError] = useState<unknown>(null);
  const load = useCallback(() => {
    Promise.all([rpc("doctors.list", { include_inactive: false }), rpc("chairs.list", { include_inactive: false })])
      .then(([d, c]) => {
        setDoctors(d);
        setChairs(c);
        setError(null);
      })
      .catch((e) => !isSessionError(e) && setError(e));
  }, []);
  useEffect(load, [load]);
  return { doctors, chairs, error, reload: load };
}
