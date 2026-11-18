// One picture for every person in the app (OF-049, OF-050): the real photo if there is one, else the
// default illustration chosen by sex and age, else the initial. Every step falls back to the next if
// an image cannot be loaded, so a missing photo file never leaves a broken image behind.
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import type { DoctorInfo, PatientAvatar, PersonSex } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { defaultArt, type AvatarArt, type AvatarKind } from "../lib/avatar";
import { initial } from "./Brand";
import { usePatientAvatars } from "./usePatientAvatars";
import patientMan from "../assets/avatars/patient-man.webp";
import patientWoman from "../assets/avatars/patient-woman.webp";
import patientBoy from "../assets/avatars/patient-boy.webp";
import patientGirl from "../assets/avatars/patient-girl.webp";
import doctorMale from "../assets/avatars/doctor-male.webp";
import doctorFemale from "../assets/avatars/doctor-female.webp";
import doctorMaleFigure from "../assets/avatars/doctor-male-figure.webp";
import doctorFemaleFigure from "../assets/avatars/doctor-female-figure.webp";
import dentalUnit from "../assets/avatars/dental-unit.webp";

const ART: Record<AvatarArt, string> = {
  "patient-man": patientMan,
  "patient-woman": patientWoman,
  "patient-boy": patientBoy,
  "patient-girl": patientGirl,
  "doctor-male": doctorMale,
  "doctor-female": doctorFemale,
};

/** Fired after a doctor is saved (photo, sex), so the header picture follows. */
export const DOCTORS_CHANGED = "artaveo:doctors-changed";

export const illustrations ={ doctorMaleFigure, doctorFemaleFigure, dentalUnit };

// Photos are content-addressed (the id is the hash of the bytes), so a loaded one never changes.
const photos = new Map<string, Promise<string | null>>();

export function loadPhoto(id: string): Promise<string | null> {
  let p = photos.get(id);
  if (!p) {
    p = rpc("photos.get", { id })
      .then((r) => r.data_url)
      .catch(() => {
        photos.delete(id); // try again next time (the file may come back with a restore)
        return null;
      });
    photos.set(id, p);
  }
  return p;
}

function usePhoto(id: string | null | undefined): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    setUrl(null);
    if (!id) return;
    let live = true;
    loadPhoto(id).then((u) => live && setUrl(u));
    return () => {
      live = false;
    };
  }, [id]);
  return url;
}

export type AvatarSize = "xs" | "sm" | "md" | "lg" | "xl";

export type PersonAvatarProps = {
  kind: AvatarKind;
  name: string;
  sex?: PersonSex | null;
  age?: number | null;
  photoId?: string | null;
  size?: AvatarSize;
  /** A coloured ring (a doctor's calendar colour). */
  ring?: string;
  /** Next to the visible name: hidden from screen readers so the name is not read twice. */
  decorative?: boolean;
  className?: string;
  testId?: string;
};

/** A patient's picture in a list that only knows the id (`usePatientAvatars`). */
export function PatientPic({ id, name, avatars, size = "sm", decorative = true, testId }: { id: string; name: string; avatars: Map<string, PatientAvatar>; size?: AvatarSize; decorative?: boolean; testId?: string }) {
  const a = avatars.get(id);
  return <PersonAvatar kind="patient" name={name} sex={a?.sex} age={a?.age} photoId={a?.photo_id} size={size} decorative={decorative} testId={testId} />;
}

/** One patient's picture by id (dialogs and chips that only know the id). */
export function PatientPicById({ id, name, size = "sm" }: { id: string; name: string; size?: AvatarSize }) {
  const avatars = usePatientAvatars([id]);
  return <PatientPic id={id} name={name} avatars={avatars} size={size} />;
}

/** A doctor's picture, ringed with the calendar colour. `doctor` may be missing (list not loaded yet). */
export function DoctorPic({ doctor, name, color, size = "sm", decorative = true, testId }: { doctor?: DoctorInfo | null; name: string; color?: string; size?: AvatarSize; decorative?: boolean; testId?: string }) {
  return <PersonAvatar kind="doctor" name={doctor?.full_name ?? name} sex={doctor?.sex} photoId={doctor?.photo_id} ring={color ?? doctor?.color} size={size} decorative={decorative} testId={testId} />;
}

export function PersonAvatar({ kind, name, sex, age, photoId, size = "md", ring, decorative, className = "", testId }: PersonAvatarProps) {
  const photo = usePhoto(photoId);
  const art = defaultArt(kind, sex, age);
  // Which sources failed to load; the next one in line is shown instead.
  const [broken, setBroken] = useState<Record<string, true>>({});
  const src = photo && !broken[photo] ? photo : art && !broken[ART[art]] ? ART[art] : null;
  const style: CSSProperties | undefined = ring ? ({ "--avatar-ring": ring } as CSSProperties) : undefined;
  const cls = `person-avatar ${size} ${ring ? "ringed" : ""} ${src ? "" : "letter"} ${kind} ${className}`;
  const kindOf = !src ? "initial" : src === photo ? "photo" : "art";
  if (!src) {
    return (
      // The letter is drawn by CSS (`data-initial`), so it is not part of the row's text.
      <span className={cls} style={style} role={decorative ? undefined : "img"} aria-label={decorative ? undefined : name} aria-hidden={decorative || undefined} data-testid={testId} data-avatar={kindOf} data-initial={initial(name)} />
    );
  }
  return (
    <span className={cls} style={style} data-testid={testId} data-avatar={kindOf}>
      <img src={src} alt={decorative ? "" : name} draggable={false} loading="lazy" decoding="async" onError={() => setBroken((b) => ({ ...b, [src]: true }))} />
    </span>
  );
}
