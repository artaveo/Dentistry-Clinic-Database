import { useEffect, type ButtonHTMLAttributes, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { ArrowLeft, LoaderCircle } from "lucide-react";

type Variant = "primary" | "secondary" | "subtle" | "danger" | "link";

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  size?: "sm" | "md" | "lg";
  icon?: LucideIcon;
  /** Icon after the label; directional ones mirror in RTL with `flipIcon`. */
  iconEnd?: LucideIcon;
  flipIcon?: boolean;
  loading?: boolean;
  block?: boolean;
  children?: ReactNode;
};

/** The one button of the app (design-direction §3-5). */
export function Button({ variant = "secondary", size = "md", icon: Icon, iconEnd: IconEnd, flipIcon, loading, block, className, children, disabled, type, ...rest }: ButtonProps) {
  const cls = ["btn", `btn-${variant}`, size !== "md" && `btn-${size}`, block && "btn-block", className].filter(Boolean).join(" ");
  return (
    <button type={type ?? "button"} className={cls} disabled={disabled || loading} aria-busy={loading || undefined} {...rest}>
      {loading ? <LoaderCircle className="spin" aria-hidden /> : Icon && <Icon aria-hidden className={flipIcon ? "flip-rtl" : undefined} />}
      {children}
      {IconEnd && !loading && <IconEnd aria-hidden className={flipIcon ? "flip-rtl" : undefined} />}
    </button>
  );
}

/** Icon-only button; `label` is required for screen readers and the tooltip. */
export function IconButton({ icon: Icon, label, variant = "subtle", size = "md", className, ...rest }: Omit<ButtonProps, "children" | "icon"> & { icon: LucideIcon; label: string }) {
  return (
    <Button variant={variant} size={size} className={["icon-btn", className].filter(Boolean).join(" ")} aria-label={label} title={label} {...rest}>
      <Icon aria-hidden />
    </Button>
  );
}

/**
 * Back button of an inner page (OF-023): a real button in the top corner with an arrow,
 * the same on every inner page. Alt+← does the same, unless the user is typing.
 */
export function BackButton({ onBack, label, testId }: { onBack: () => void; label: string; testId?: string }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === "ArrowLeft" && !isTyping(e.target)) {
        e.preventDefault();
        onBack();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack]);
  return (
    <div className="back-row">
      <Button variant="subtle" icon={ArrowLeft} flipIcon className="back-btn" onClick={onBack} aria-keyshortcuts="Alt+ArrowLeft" data-testid={testId}>
        {label}
      </Button>
    </div>
  );
}

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
}
