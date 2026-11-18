// Pictures for lists that only know patient ids (calendar, queue, recalls, search): one
// `patients.avatars` call per screen for the ids not seen yet.
import { useEffect, useState } from "react";
import type { PatientAvatar, PatientInfo } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { ageOn } from "../lib/dates";

const known = new Map<string, PatientAvatar>();
const listeners = new Set<() => void>();
const notify = () => listeners.forEach((fn) => fn());

/** A patient was saved (photo changed, sex or birth date edited): lists show the new picture. */
export function rememberPatient(p: PatientInfo) {
  known.set(p.id, { patient_id: p.id, sex: p.sex, age: p.date_of_birth ? ageOn(p.date_of_birth) : p.approximate_age, photo_id: p.photo_id });
  notify();
}

const MAX = 500;

export function usePatientAvatars(ids: (string | null | undefined)[]): Map<string, PatientAvatar> {
  const [, bump] = useState(0);
  const key = [...new Set(ids.filter((x): x is string => !!x))].sort().join(",");
  useEffect(() => {
    const fn = () => bump((n) => n + 1);
    listeners.add(fn);
    return () => {
      listeners.delete(fn);
    };
  }, []);
  useEffect(() => {
    const missing = key ? key.split(",").filter((id) => !known.has(id)).slice(0, MAX) : [];
    if (!missing.length) return;
    rpc("patients.avatars", { patient_ids: missing })
      .then((list) => {
        list.forEach((a) => known.set(a.patient_id, a));
        notify();
      })
      // No permission or an old server: lists keep the initials.
      .catch(() => {});
  }, [key]);
  return known;
}
