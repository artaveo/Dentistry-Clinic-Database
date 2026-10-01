import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { CircleAlert, CircleCheck, Info, LoaderCircle, RefreshCw, TriangleAlert } from "lucide-react";
import { useI18n } from "../i18n";
import { Button } from "./Button";

type Tone = "neutral" | "accent" | "success" | "warning" | "danger" | "info";

/** Status pill: soft background + text, never colour alone (design-direction §3-8). */
export function Badge({ tone = "neutral", dot, icon: Icon, children, testId }: { tone?: Tone; dot?: boolean; icon?: LucideIcon; children: ReactNode; testId?: string }) {
  return (
    <span className={`badge ${tone === "neutral" ? "" : tone}`} data-testid={testId}>
      {dot && <span className="dot" aria-hidden />}
      {Icon && <Icon aria-hidden />}
      {children}
    </span>
  );
}

const NOTICE_ICONS = { info: Info, warning: TriangleAlert, danger: CircleAlert, success: CircleCheck };

export function Notice({ tone = "info", title, children, testId }: { tone?: keyof typeof NOTICE_ICONS; title?: ReactNode; children?: ReactNode; testId?: string }) {
  const Icon = NOTICE_ICONS[tone];
  return (
    <div className={`notice ${tone === "info" ? "" : tone}`} role={tone === "danger" ? "alert" : undefined} data-testid={testId}>
      <Icon aria-hidden />
      <div className="notice-body">
        {title && <span className="notice-title">{title}</span>}
        {children && <span>{children}</span>}
      </div>
    </div>
  );
}

export function EmptyState({ icon: Icon, title, children, action }: { icon: LucideIcon; title: ReactNode; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="empty" data-testid="empty-state">
      <span className="empty-icon"><Icon aria-hidden /></span>
      <span className="empty-title">{title}</span>
      {children && <span>{children}</span>}
      {action && <div className="empty-actions">{action}</div>}
    </div>
  );
}

export function ErrorState({ message, onRetry }: { message: string; onRetry?: () => void }) {
  const { t } = useI18n();
  return (
    <div className="empty danger" role="alert" data-testid="error-state">
      <span className="empty-icon"><CircleAlert aria-hidden /></span>
      <span className="empty-title">{t("common.errorTitle")}</span>
      <span>{message}</span>
      {onRetry && (
        <div className="empty-actions">
          <Button icon={RefreshCw} onClick={onRetry}>{t("common.retry")}</Button>
        </div>
      )}
    </div>
  );
}

export function Loading({ label }: { label?: string }) {
  const { t } = useI18n();
  return (
    <div className="loading-block" role="status" aria-live="polite">
      <LoaderCircle className="spinner" aria-hidden />
      <span className="visually-hidden">{label ?? t("common.loading")}</span>
    </div>
  );
}

/** Placeholder rows while a table loads. */
export function SkeletonRows({ rows = 4, cols }: { rows?: number; cols: number }) {
  return (
    <>
      {Array.from({ length: rows }, (_, r) => (
        <tr key={r} aria-hidden>
          {Array.from({ length: cols }, (_, c) => (
            <td key={c}><div className="skeleton" style={{ width: `${45 + ((r * 7 + c * 13) % 40)}%` }} /></td>
          ))}
        </tr>
      ))}
    </>
  );
}
