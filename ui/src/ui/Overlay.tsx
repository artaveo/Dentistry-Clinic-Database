import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { X } from "lucide-react";
import { useI18n } from "../i18n";
import { IconButton } from "./Button";

/** Closes on Escape and on a pointer press outside `ref`. */
export function useDismiss(open: boolean, onClose: () => void, ref: React.RefObject<HTMLElement | null>) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    const onDown = (e: PointerEvent) => ref.current && !ref.current.contains(e.target as Node) && onClose();
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onDown);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onDown);
    };
  }, [open, onClose, ref]);
}

/** Anchored transient surface (menus, notifications) — Acrylic layer. */
export function Popover({ open, onClose, trigger, children, align = "end", width, testId }: { open: boolean; onClose: () => void; trigger: ReactNode; children: ReactNode; align?: "start" | "end"; width?: number; testId?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useDismiss(open, onClose, ref);
  return (
    <div className="popover-anchor" ref={ref}>
      {trigger}
      {open && (
        <div className={`glass-overlay popover ${align === "start" ? "start" : ""}`} style={width ? { width } : undefined} data-testid={testId}>
          {children}
        </div>
      )}
    </div>
  );
}

/**
 * Modal dialog: centred, scrim + blur, focus moves inside and returns to the
 * opener on close, Escape closes.
 */
export function Dialog({ title, description, onClose, children, footer, wide, testId }: { title: ReactNode; description?: ReactNode; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean; testId?: string }) {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    const first = ref.current?.querySelector<HTMLElement>("input, select, textarea, button:not([data-close])");
    first?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      opener?.focus?.();
    };
  }, []);
  return (
    <div className="scrim" onPointerDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className={`glass-overlay dialog ${wide ? "wide" : ""}`} role="dialog" aria-modal="true" aria-label={typeof title === "string" ? title : undefined} data-testid={testId}>
        <div className="dialog-header">
          <div className="dialog-titles">
            <h2 className="t-title">{title}</h2>
            {description && <p className="muted">{description}</p>}
          </div>
          <IconButton icon={X} label={t("common.close")} size="sm" onClick={onClose} data-close />
        </div>
        <div className="dialog-body">{children}</div>
        {footer && <div className="dialog-footer">{footer}</div>}
      </div>
    </div>
  );
}
