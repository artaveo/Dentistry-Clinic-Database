import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
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

export type FloatingCloseReason = "escape" | "tab" | "outside";

/**
 * A popup anchored to `anchorRef` but drawn on `document.body` (position: fixed), so a dialog's
 * `overflow` never clips it. It flips above the anchor when there is no room below and stays inside
 * the window. Escape (it wins over a dialog underneath), a press outside it and the anchor, and Tab
 * leaving it all call `onClose(reason)`; the caller decides whether to move focus back to its trigger.
 * Mount it only while open.
 */
export function FloatingPanel({ anchorRef, onClose, children, className, label, role = "dialog", testId, matchAnchorWidth }: {
  anchorRef: React.RefObject<HTMLElement | null>;
  onClose: (reason: FloatingCloseReason) => void;
  children: ReactNode;
  className?: string;
  label?: string;
  role?: "dialog" | "listbox" | "menu";
  testId?: string;
  matchAnchorWidth?: boolean;
}) {
  const panelRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const [pos, setPos] = useState<{ top: number; left: number; width?: number } | null>(null);

  const place = useCallback(() => {
    const anchor = anchorRef.current;
    const panel = panelRef.current;
    if (!anchor || !panel) return;
    const r = anchor.getBoundingClientRect();
    const margin = 8;
    const width = matchAnchorWidth ? r.width : undefined;
    const pw = width ?? panel.offsetWidth;
    const ph = panel.offsetHeight;
    const rtl = getComputedStyle(anchor).direction === "rtl";
    const left = Math.max(margin, Math.min(rtl ? r.right - pw : r.left, window.innerWidth - pw - margin));
    const below = window.innerHeight - r.bottom - margin;
    const above = r.top - margin;
    const wanted = ph <= below || below >= above ? r.bottom + 6 : r.top - ph - 6;
    const top = Math.max(margin, Math.min(wanted, window.innerHeight - ph - margin));
    setPos((prev) => (prev && prev.top === top && prev.left === left && prev.width === width ? prev : { top, left, width }));
  }, [anchorRef, matchAnchorWidth]);

  // Re-measure after every render: the content (month → year view, filtered list) changes size.
  useLayoutEffect(() => {
    place();
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Capture phase + stopPropagation: a dialog underneath must not also close.
      e.stopPropagation();
      e.preventDefault();
      closeRef.current("escape");
    };
    const onDown = (e: PointerEvent) => {
      const target = e.target as Node;
      if (panelRef.current?.contains(target) || anchorRef.current?.contains(target)) return;
      closeRef.current("outside");
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onDown);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onDown);
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [anchorRef, place]);

  return createPortal(
    <div
      ref={panelRef}
      className={["glass-overlay", "floating-panel", className].filter(Boolean).join(" ")}
      data-floating-layer=""
      role={role}
      aria-label={label}
      dir={document.documentElement.dir || undefined}
      style={{ top: pos?.top ?? 0, left: pos?.left ?? 0, width: pos?.width, visibility: pos ? "visible" : "hidden" }}
      onKeyDown={(e) => {
        // Tab walks through the panel; leaving it at either end closes it (the caller returns focus to its trigger).
        if (e.key !== "Tab" || role !== "dialog" || !panelRef.current) return;
        const items = focusableIn(panelRef.current);
        const active = document.activeElement;
        if (items.length === 0 || (!e.shiftKey && active === items[items.length - 1]) || (e.shiftKey && active === items[0])) {
          e.preventDefault();
          closeRef.current("tab");
        }
      }}
      data-testid={testId}
    >
      {children}
    </div>,
    document.body,
  );
}

const openDialogs: object[] = [];

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function focusableIn(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => el.getClientRects().length > 0);
}

/**
 * Modal dialog: centred, scrim + blur, focus moves inside, Tab cycles inside (focus trap) and
 * returns to the opener on close, Escape and the X close.
 *
 * `dismissOnScrim` (default true): a press on the dark area closes the dialog. Forms pass `false`
 * so a stray click never throws away what was typed; Escape and the X still close.
 */
export function Dialog({ title, description, onClose, children, footer, wide, testId, dismissOnScrim = true }: { title: ReactNode; description?: ReactNode; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean; testId?: string; dismissOnScrim?: boolean }) {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    const first = ref.current?.querySelector<HTMLElement>("input, select, textarea, button:not([data-close])");
    first?.focus();
    // Dialogs can stack (a receipt over an invoice): Escape and Tab act only on the one on top.
    const me = {};
    openDialogs.push(me);
    const onKey = (e: KeyboardEvent) => {
      if (openDialogs[openDialogs.length - 1] !== me) return;
      if (e.key === "Escape") {
        closeRef.current();
        return;
      }
      const root = ref.current;
      if (e.key !== "Tab" || !root) return;
      const active = document.activeElement as HTMLElement | null;
      // A calendar or list popup drawn on <body> manages its own Tab.
      if (active?.closest("[data-floating-layer]")) return;
      const items = focusableIn(root);
      if (items.length === 0) {
        e.preventDefault();
        root.focus();
        return;
      }
      const firstItem = items[0];
      const lastItem = items[items.length - 1];
      if (!active || !root.contains(active)) {
        e.preventDefault();
        firstItem.focus();
      } else if (e.shiftKey && (active === firstItem || active === root)) {
        e.preventDefault();
        lastItem.focus();
      } else if (!e.shiftKey && active === lastItem) {
        e.preventDefault();
        firstItem.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      openDialogs.splice(openDialogs.indexOf(me), 1);
      opener?.focus?.();
    };
  }, []);
  return (
    <div className="scrim" onPointerDown={(e) => dismissOnScrim && e.target === e.currentTarget && onClose()}>
      <div ref={ref} tabIndex={-1} className={`glass-overlay dialog ${wide ? "wide" : ""}`} role="dialog" aria-modal="true" aria-label={typeof title === "string" ? title : undefined} data-testid={testId}>
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
