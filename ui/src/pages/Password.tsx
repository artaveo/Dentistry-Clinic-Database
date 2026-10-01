import { useState } from "react";
import { KeyRound, LockKeyhole } from "lucide-react";
import { rpc } from "../lib/api";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Field, PasswordInput } from "../ui/Field";
import { Notice } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { useToast } from "../ui/Toast";

/** Change own password (user menu). Every field validates live (OF-003). */
export function PasswordDialog({ onClose }: { onClose: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const form = useForm(
    { current_password: "", new_password: "", repeat: "" },
    { current_password: v.required, new_password: v.password, repeat: (r, all) => (!r ? "rule.required" : r !== all.new_password ? "rule.mismatch" : null) },
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const e = (k: "current_password" | "new_password" | "repeat") => form.error(k) && t(form.error(k)!);

  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await rpc("auth.change_password", { current_password: form.values.current_password, new_password: form.values.new_password });
      toast.success(t("password.changed"));
      onClose();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("password.title")}
      description={t("password.subtitle")}
      onClose={onClose}
      testId="password-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={KeyRound} loading={busy} type="submit" form="password-form" data-testid="password-save">{t("password.submit")}</Button>
        </>
      }
    >
      <form id="password-form" className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("password.current")} error={e("current_password")}>
          <PasswordInput icon={LockKeyhole} value={form.values.current_password} onChange={(x) => form.set("current_password", x.target.value)} onBlur={() => form.blur("current_password")} data-testid="password-current" />
        </Field>
        <Field label={t("password.new")} hint={t("hint.password")} error={e("new_password")}>
          <PasswordInput icon={KeyRound} value={form.values.new_password} onChange={(x) => form.set("new_password", x.target.value)} onBlur={() => form.blur("new_password")} data-testid="password-new" />
        </Field>
        <Field label={t("login.passwordRepeat")} error={e("repeat")}>
          <PasswordInput icon={KeyRound} value={form.values.repeat} onChange={(x) => form.set("repeat", x.target.value)} onBlur={() => form.blur("repeat")} data-testid="password-repeat" />
        </Field>
      </form>
    </Dialog>
  );
}
