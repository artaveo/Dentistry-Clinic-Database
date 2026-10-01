import type { ButtonHTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { LoaderCircle } from "lucide-react";

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
