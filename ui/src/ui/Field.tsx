import { cloneElement, forwardRef, isValidElement, useId, useState } from "react";
import type { InputHTMLAttributes, ReactElement, ReactNode, SelectHTMLAttributes, TextareaHTMLAttributes } from "react";
import type { LucideIcon } from "lucide-react";
import { ChevronDown, CircleAlert, Eye, EyeOff, Info } from "lucide-react";
import { useI18n } from "../i18n";

/**
 * Field + Hint + Error (OF-003, design-direction §3-5): the format hint is
 * visible from the start; once the value is invalid the same line turns into
 * the exact error, the control turns red, and screen readers are told.
 */
export function Field({
  label,
  hint,
  error,
  optional,
  children,
  className,
}: {
  label: ReactNode;
  hint?: ReactNode;
  error?: string | null;
  optional?: boolean;
  children: ReactElement<{ id?: string; invalid?: boolean; "aria-describedby"?: string }>;
  className?: string;
}) {
  const { t } = useI18n();
  const id = useId();
  const msgId = `${id}-msg`;
  const control = isValidElement(children)
    ? cloneElement(children, { id: children.props.id ?? id, invalid: !!error, "aria-describedby": hint || error ? msgId : undefined })
    : children;
  return (
    <div className={["field", className].filter(Boolean).join(" ")}>
      <label className="field-label" htmlFor={children.props.id ?? id}>
        {label}
        {optional && <span className="optional">{t("common.optional")}</span>}
      </label>
      {control}
      {error ? (
        <div className="field-error" id={msgId} role="alert" data-testid="field-error">
          <CircleAlert aria-hidden />
          <span>{error}</span>
        </div>
      ) : hint ? (
        <div className="field-hint" id={msgId}>
          <Info aria-hidden />
          <span>{hint}</span>
        </div>
      ) : null}
    </div>
  );
}

type InputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "size"> & { invalid?: boolean; icon?: LucideIcon; large?: boolean; suffix?: ReactNode };

export const TextInput = forwardRef<HTMLInputElement, InputProps>(function TextInput({ invalid, icon: Icon, large, suffix, className, disabled, ...rest }, ref) {
  return (
    <div className={["control", large && "control-lg", className].filter(Boolean).join(" ")} data-invalid={invalid || undefined} data-disabled={disabled || undefined}>
      {Icon && <span className="control-icon"><Icon aria-hidden /></span>}
      <input ref={ref} aria-invalid={invalid || undefined} disabled={disabled} {...rest} />
      {suffix && <span className="control-suffix">{suffix}</span>}
    </div>
  );
});

export const PasswordInput = forwardRef<HTMLInputElement, InputProps>(function PasswordInput({ invalid, icon: Icon, large, className, ...rest }, ref) {
  const { t } = useI18n();
  const [shown, setShown] = useState(false);
  return (
    <div className={["control", large && "control-lg", className].filter(Boolean).join(" ")} data-invalid={invalid || undefined}>
      {Icon && <span className="control-icon"><Icon aria-hidden /></span>}
      <input ref={ref} type={shown ? "text" : "password"} dir="ltr" aria-invalid={invalid || undefined} autoComplete="off" {...rest} />
      <button type="button" className="control-action" onClick={() => setShown((s) => !s)} aria-label={shown ? t("common.hidePassword") : t("common.showPassword")} title={shown ? t("common.hidePassword") : t("common.showPassword")} tabIndex={-1}>
        {shown ? <EyeOff aria-hidden /> : <Eye aria-hidden />}
      </button>
    </div>
  );
});

type TextareaProps = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "size"> & { invalid?: boolean };

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea({ invalid, className, rows = 3, ...rest }, ref) {
  return (
    <div className={["control", "control-textarea", className].filter(Boolean).join(" ")} data-invalid={invalid || undefined}>
      <textarea ref={ref} aria-invalid={invalid || undefined} rows={rows} {...rest} />
    </div>
  );
});

export function Select({ invalid, className, children, disabled, ...rest }: SelectHTMLAttributes<HTMLSelectElement> & { invalid?: boolean }) {
  return (
    <div className={["control", className].filter(Boolean).join(" ")} data-invalid={invalid || undefined} data-disabled={disabled || undefined}>
      <select aria-invalid={invalid || undefined} disabled={disabled} {...rest}>
        {children}
      </select>
      <ChevronDown className="chevron" aria-hidden />
    </div>
  );
}
