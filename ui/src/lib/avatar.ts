// Which default picture a person gets (OF-049, OF-050). Pure, so it is unit-tested.
import type { PersonSex } from "../../../shared/ts/contract";

export type AvatarKind = "patient" | "doctor" | "user";
export type AvatarArt = "patient-man" | "patient-woman" | "patient-boy" | "patient-girl" | "doctor-male" | "doctor-female";

/** Under this age a patient is drawn as a boy or a girl. */
export const CHILD_AGE = 13;

/**
 * The illustration for a person, or `null` when nothing is known (unknown sex, or a plain user):
 * the caller then shows the initial. Doctors are drawn as doctors at any age.
 */
export function defaultArt(kind: AvatarKind, sex: PersonSex | null | undefined, age?: number | null): AvatarArt | null {
  if (kind === "user" || !sex) return null;
  if (kind === "doctor") return sex === "female" ? "doctor-female" : "doctor-male";
  const child = age !== null && age !== undefined && age >= 0 && age < CHILD_AGE;
  if (sex === "female") return child ? "patient-girl" : "patient-woman";
  return child ? "patient-boy" : "patient-man";
}

/** Longest side of the picture the UI uploads; the Core shrinks it again to the same size. */
export const PHOTO_SIDE = 256;

/** The centred square of an image, as a crop box (for a round photo without distortion). */
export function squareCrop(width: number, height: number): { sx: number; sy: number; side: number } {
  const side = Math.min(width, height);
  // Portrait photos keep the top (the face), landscape ones are centred.
  return { sx: Math.round((width - side) / 2), sy: height > width ? Math.round((height - side) * 0.2) : 0, side };
}
