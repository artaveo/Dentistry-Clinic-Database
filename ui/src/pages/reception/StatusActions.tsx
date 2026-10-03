import { useState } from "react";
import { Ban, CalendarCheck, CheckCheck, DoorOpen, Play, UserRoundX } from "lucide-react";
import type { AppointmentInfo, AppointmentStatus, FollowUpInput, RecallKind } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { addDays, todayIso } from "../../lib/calendar";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { DateField } from "../../ui/DateField";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Switch } from "../../ui/Controls";
import { Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import type { CalendarSystem } from "../../../../shared/ts/contract";
import { RECALL_KINDS, ruleText } from "./labels";

type Perms = { edit: boolean; treat: boolean };

/**
 * The next steps of an appointment (4.4/4.5): confirm, patient arrived, start treatment,
 * complete (with an optional follow-up), cancel, no-show. Only the steps the workflow
 * allows — and the user may perform — are shown.
 */
export function StatusActions({ appt, perms, calendar, onChanged, size = "sm", only }: {
  appt: AppointmentInfo;
  perms: Perms;
  calendar: CalendarSystem;
  onChanged: (a: AppointmentInfo) => void;
  size?: "sm" | "md";
  /** Limit to some steps (the queue cards show fewer). */
  only?: AppointmentStatus[];
}) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [busy, setBusy] = useState<AppointmentStatus | null>(null);
  const [dialog, setDialog] = useState<"complete" | "cancel" | "no_show" | null>(null);
  const isToday = appt.date === todayIso();

  const move = async (status: AppointmentStatus, extra: { reason?: string | null; follow_up?: FollowUpInput | null } = {}) => {
    setBusy(status);
    try {
      const updated = await rpc("appointments.set_status", { id: appt.id, version: appt.version, status, reason: extra.reason ?? null, follow_up: extra.follow_up ?? null });
      toast.success(t(`appt.done.${status}`));
      setDialog(null);
      onChanged(updated);
    } catch (x) {
      if (!isSessionError(x)) toast.error(ruleText(t, x, err));
      throw x;
    } finally {
      setBusy(null);
    }
  };

  const s = appt.status;
  const steps: { to: AppointmentStatus; icon: typeof Play; variant: "primary" | "secondary" | "danger"; show: boolean; open?: "complete" | "cancel" | "no_show" }[] = [
    { to: "confirmed", icon: CalendarCheck, variant: "secondary", show: perms.edit && s === "scheduled" },
    { to: "checked_in", icon: DoorOpen, variant: "primary", show: perms.edit && (s === "scheduled" || s === "confirmed") && isToday },
    { to: "in_treatment", icon: Play, variant: "primary", show: perms.treat && s === "checked_in" },
    { to: "completed", icon: CheckCheck, variant: "primary", show: perms.treat && s === "in_treatment", open: "complete" },
    { to: "no_show", icon: UserRoundX, variant: "secondary", show: perms.edit && (s === "scheduled" || s === "confirmed"), open: "no_show" },
    { to: "cancelled", icon: Ban, variant: "danger", show: perms.edit && (s === "scheduled" || s === "confirmed" || s === "checked_in"), open: "cancel" },
  ];

  return (
    <>
      {steps.filter((x) => x.show && (!only || only.includes(x.to))).map((x) => (
        <Button key={x.to} size={size} variant={x.variant} icon={x.icon} loading={busy === x.to} onClick={() => (x.open ? setDialog(x.open) : move(x.to).catch(() => {}))} data-testid={`appt-action-${x.to}`}>
          {t(`appt.action.${x.to}`)}
        </Button>
      ))}
      {dialog === "complete" && <CompleteDialog calendar={calendar} onClose={() => setDialog(null)} onConfirm={(f) => move("completed", { follow_up: f })} />}
      {(dialog === "cancel" || dialog === "no_show") && <ReasonDialog kind={dialog} onClose={() => setDialog(null)} onConfirm={(reason) => move(dialog === "cancel" ? "cancelled" : "no_show", { reason })} />}
    </>
  );
}

function ReasonDialog({ kind, onClose, onConfirm }: { kind: "cancel" | "no_show"; onClose: () => void; onConfirm: (reason: string | null) => Promise<unknown> }) {
  const { t } = useI18n();
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    setBusy(true);
    try {
      await onConfirm(reason.trim() || null);
    } catch {
      /* the toast already says why */
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t(`appt.${kind}.title`)}
      description={t(`appt.${kind}.hint`)}
      onClose={onClose}
      testId={`appt-${kind}-dialog`}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="danger" loading={busy} onClick={submit} data-testid="appt-reason-confirm">{t(`appt.action.${kind === "cancel" ? "cancelled" : "no_show"}`)}</Button>
        </>
      }
    >
      <Field label={t("appt.cancel.reason")} hint={t("appt.cancel.reasonHint")} optional>
        <TextInput value={reason} maxLength={200} onChange={(e) => setReason(e.target.value)} data-testid="appt-reason-input" />
      </Field>
    </Dialog>
  );
}

const PRESETS: { key: string; days: number }[] = [
  { key: "week1", days: 7 },
  { key: "week2", days: 14 },
  { key: "month1", days: 30 },
  { key: "month3", days: 91 },
  { key: "month6", days: 182 },
];

/** Completing a visit may plan the patient's next one in the same step (4.6). */
function CompleteDialog({ calendar, onClose, onConfirm }: { calendar: CalendarSystem; onClose: () => void; onConfirm: (f: FollowUpInput | null) => Promise<unknown> }) {
  const { t } = useI18n();
  const [plan, setPlan] = useState(false);
  const [due, setDue] = useState(addDays(todayIso(), 14));
  const [kind, setKind] = useState<RecallKind>("follow_up");
  const [repeat, setRepeat] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const months = repeat.trim() ? Number(repeat) : null;
  const repeatError = months !== null && (!Number.isInteger(months) || months < 1 || months > 60) ? t("rule.repeat_months_range") : null;

  const submit = async () => {
    if (plan && repeatError) return;
    setBusy(true);
    try {
      await onConfirm(plan ? { due_date: due, kind, repeat_months: months, note: note.trim() || null } : null);
    } catch {
      /* the toast already says why */
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("appt.complete.title")}
      description={t("appt.complete.hint")}
      onClose={onClose}
      testId="appt-complete-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={CheckCheck} loading={busy} onClick={submit} data-testid="appt-complete-confirm">{t("appt.action.completed")}</Button>
        </>
      }
    >
      <div className="stack">
        <Switch checked={plan} onChange={setPlan} label={t("appt.followUp.plan")} testId="followup-toggle" />
        {plan && (
          <div className="stack" data-testid="followup-form">
            <div className="row" role="group" aria-label={t("appt.followUp.presets")}>
              {PRESETS.map((p) => (
                <Button key={p.key} size="sm" variant={due === addDays(todayIso(), p.days) ? "primary" : "secondary"} onClick={() => setDue(addDays(todayIso(), p.days))} data-testid={`followup-${p.key}`}>
                  {t(`appt.followUp.${p.key}`)}
                </Button>
              ))}
            </div>
            <Field label={t("appt.followUp.due")}>
              <DateField value={due} onChange={setDue} calendar={calendar} label={t("appt.followUp.due")} testId="followup-date" />
            </Field>
            <Field label={t("recall.kind")}>
              <Select value={kind} onChange={(e) => setKind(e.target.value as RecallKind)} data-testid="followup-kind">
                {RECALL_KINDS.filter((k) => k !== "no_show").map((k) => <option key={k} value={k}>{t(`recall.kind.${k}`)}</option>)}
              </Select>
            </Field>
            <Field label={t("recall.repeat")} hint={t("recall.repeatHint")} optional error={repeatError}>
              <TextInput inputMode="numeric" dir="ltr" value={repeat} onChange={(e) => setRepeat(e.target.value)} data-testid="followup-repeat" />
            </Field>
            <Field label={t("recall.note")} optional>
              <Textarea rows={2} value={note} maxLength={500} onChange={(e) => setNote(e.target.value)} data-testid="followup-note" />
            </Field>
            <Notice tone="info">{t("appt.followUp.callList")}</Notice>
          </div>
        )}
      </div>
    </Dialog>
  );
}
