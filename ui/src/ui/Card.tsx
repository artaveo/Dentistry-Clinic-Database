import type { HTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

/** Content-layer glass card (design-direction §3-1). */
export function Card({ className, flush, children, ...rest }: HTMLAttributes<HTMLElement> & { flush?: boolean }) {
  return (
    <section className={["glass", "card", flush && "card-flush", className].filter(Boolean).join(" ")} {...rest}>
      {children}
    </section>
  );
}

export function CardHeader({ icon: Icon, title, description, actions }: { icon?: LucideIcon; title: ReactNode; description?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="card-header">
      {Icon && <span className="card-icon"><Icon aria-hidden /></span>}
      <div className="card-titles">
        <h2 className="t-title">{title}</h2>
        {description && <p className="card-desc">{description}</p>}
      </div>
      {actions && <div className="row">{actions}</div>}
    </header>
  );
}

export function CardFooter({ children }: { children: ReactNode }) {
  return <footer className="card-footer">{children}</footer>;
}

/** Title + one-line description + the page's actions (Notion pattern). */
export function PageHeader({ title, description, actions }: { title: ReactNode; description?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="page-header">
      <div className="page-titles">
        <h1 className="t-title-lg" data-testid="page-title">{title}</h1>
        {description && <p className="page-desc">{description}</p>}
      </div>
      {actions && <div className="page-actions">{actions}</div>}
    </header>
  );
}

export function Page({ children, testId }: { children: ReactNode; testId?: string }) {
  return <div className="page" data-testid={testId}>{children}</div>;
}

type Tone = "accent" | "success" | "warning" | "danger" | "info";

export function Stat({ icon: Icon, label, value, sub, tone = "accent", testId }: { icon: LucideIcon; label: ReactNode; value: ReactNode; sub?: ReactNode; tone?: Tone; testId?: string }) {
  return (
    <div className="glass stat">
      <span className={`stat-icon ${tone === "accent" ? "" : tone}`}><Icon aria-hidden /></span>
      <div className="stat-body">
        <span className="stat-label">{label}</span>
        <span className="stat-value" data-testid={testId}>{value}</span>
        {sub && <span className="stat-sub">{sub}</span>}
      </div>
    </div>
  );
}
