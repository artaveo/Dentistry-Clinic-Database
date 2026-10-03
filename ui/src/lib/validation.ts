// Live validation (OF-001…003, DoD rule 10). Each rule returns a translation
// key or null. The same rules run in the Core; the UI checks first so the
// user sees the exact problem under the input before pressing the button.
import { useState } from "react";
import { ApiError } from "./api";
import { latinDigits } from "./dates";

export type Check<V> = (value: V) => string | null;

export const MIN_PASSWORD = 8;

export const v = {
  required: (s: string) => (s.trim() ? null : "rule.required"),
  /** Mirrors core `auth::validate_username`, with a precise message per mistake. */
  username: (s: string) => {
    if (!s) return "rule.required";
    if (/[^\x00-\x7F]/.test(s)) return "rule.username_latin";
    if (/[^A-Za-z0-9._-]/.test(s)) return "rule.username_chars";
    if (s.length < 3 || s.length > 32) return "rule.username_length";
    return null;
  },
  password: (s: string) => (!s ? "rule.required" : [...s].length < MIN_PASSWORD ? "rule.password_too_short" : null),
  displayName: (s: string) => (!s.trim() ? "rule.required" : [...s].length > 100 ? "rule.display_name_length" : null),
  fullName: (s: string) => (!s.trim() ? "rule.required" : [...s.trim()].length > 150 ? "rule.full_name_length" : null),
  clinicName: (s: string) => (!s.trim() ? "rule.required" : [...s].length > 120 ? "rule.clinic_name_length" : null),
  /** Optional; when given, a phone number of 7–15 digits (+, spaces and dashes allowed). */
  phone: (s: string) => {
    if (!s.trim()) return null;
    const d = latinDigits(s).replace(/[\s()+-]/g, "");
    return /^\d{7,15}$/.test(d) ? null : "rule.phone";
  },
  intRange: (min: number, max: number, key: string) => (s: string) => {
    const n = Number(latinDigits(String(s)));
    return String(s).trim() !== "" && Number.isInteger(n) && n >= min && n <= max ? null : key;
  },
};

/** Which form field a Core error belongs to (`field` names are request params). */
export type FieldMap<K extends string> = Partial<Record<string, K>>;

/**
 * Form state with live validation. An error shows once the user has typed in
 * (or left) that field, or after a submit attempt; a Core error with a
 * `field` lands under the matching input and clears as soon as it changes.
 */
export function useForm<T extends Record<string, string>>(initial: T, checks: Partial<{ [K in keyof T]: (value: T[K], all: T) => string | null }>) {
  const [values, setValues] = useState<T>(initial);
  const [touched, setTouched] = useState<Partial<Record<keyof T, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const [server, setServer] = useState<Partial<Record<keyof T, string>>>({});

  const errorOf = (k: keyof T, vals: T = values): string | null => {
    const c = checks[k];
    return (c ? c(vals[k], vals) : null) ?? server[k] ?? null;
  };

  return {
    values,
    set: <K extends keyof T>(k: K, value: T[K]) => {
      setValues((s) => ({ ...s, [k]: value }));
      setTouched((s) => ({ ...s, [k]: true }));
      setServer((s) => ({ ...s, [k]: undefined }));
    },
    blur: (k: keyof T) => setTouched((s) => ({ ...s, [k]: true })),
    /** Translation key of the visible error for `k`, if any. */
    error: (k: keyof T): string | null => (submitted || touched[k] ? errorOf(k) : null),
    /** Marks the form submitted; true when every field is valid. */
    validate: (): boolean => {
      setSubmitted(true);
      return (Object.keys(values) as (keyof T)[]).every((k) => !(checks[k]?.(values[k], values) ?? null));
    },
    /** Puts a Core field error under its input; false if it is not a field error. */
    serverError: (e: unknown, map: FieldMap<Extract<keyof T, string>> = {}): boolean => {
      if (!(e instanceof ApiError) || !e.field) return false;
      const k = (map[e.field] ?? e.field) as keyof T;
      if (!(k in values)) return false;
      setServer((s) => ({ ...s, [k]: e.rule ? `rule.${e.rule}` : `error.${e.code}` }));
      setSubmitted(true);
      return true;
    },
    reset: (next: T = initial) => {
      setValues(next);
      setTouched({});
      setSubmitted(false);
      setServer({});
    },
  };
}
