import type { AppointmentStatus, RecallKind, RecallStatus } from "../../../../shared/ts/contract";
import { ApiError } from "../../lib/api";

export type Tone = "neutral" | "accent" | "success" | "warning" | "danger" | "info";

export const STATUS_TONE: Record<AppointmentStatus, Tone> = {
  scheduled: "neutral",
  confirmed: "info",
  checked_in: "warning",
  in_treatment: "accent",
  completed: "success",
  cancelled: "danger",
  no_show: "danger",
  rescheduled: "neutral",
};

export const RECALL_KINDS: RecallKind[] = ["checkup", "cleaning", "follow_up", "no_show", "other"];
export const RECALL_STATUS_TONE: Record<RecallStatus, Tone> = { pending: "warning", contacted: "info", booked: "accent", done: "success", dismissed: "neutral" };

/** Appointments in these statuses still occupy their doctor, chair and patient. */
export const LIVE: AppointmentStatus[] = ["scheduled", "confirmed", "checked_in", "in_treatment"];
/** Only these can still be moved by drag & drop or by editing the time. */
export const MOVABLE: AppointmentStatus[] = ["scheduled", "confirmed"];

/** Name of a role: the clinic's own label for custom roles, the translation for built-in ones. */
export function roleText(t: (k: string) => string, code: string, label?: string | null): string {
  return label ?? t(`role.${code}`);
}

/** The exact rule a Core error broke ("the chair is busy…"), else the general message of its code. */
export function ruleText(t: (k: string) => string, e: unknown, fallback: (e: unknown) => string): string {
  return e instanceof ApiError && e.rule ? t(`rule.${e.rule}`) : fallback(e);
}

/** Rules that mean "outside the doctor's schedule" — reception may still book on purpose. */
export const SCHEDULE_RULES = ["outside_working_hours", "doctor_on_break", "doctor_on_leave"];
