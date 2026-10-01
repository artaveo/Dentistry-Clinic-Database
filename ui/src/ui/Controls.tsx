import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { Check } from "lucide-react";

export function Checkbox({ checked, onChange, children, disabled, testId }: { checked: boolean; onChange: (v: boolean) => void; children: ReactNode; disabled?: boolean; testId?: string }) {
  return (
    <label className="check">
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} data-testid={testId} />
      <span className="box" aria-hidden><Check /></span>
      <span>{children}</span>
    </label>
  );
}

export function Switch({ checked, onChange, label, disabled, testId }: { checked: boolean; onChange: (v: boolean) => void; label: ReactNode; disabled?: boolean; testId?: string }) {
  return (
    <label className="switch">
      <input type="checkbox" role="switch" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} data-testid={testId} />
      <span className="track" aria-hidden><span className="thumb" /></span>
      <span>{label}</span>
    </label>
  );
}

export type SegmentOption<V extends string> = { value: V; label: ReactNode; icon?: LucideIcon; testId?: string };

/** 2–4 mutually exclusive choices (theme, calendar, AM/PM). */
export function Segmented<V extends string>({ value, onChange, options, label, block }: { value: V; onChange: (v: V) => void; options: SegmentOption<V>[]; label: string; block?: boolean }) {
  return (
    <div className={["segmented", block && "block"].filter(Boolean).join(" ")} role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={value === o.value} aria-pressed={value === o.value} onClick={() => onChange(o.value)} data-testid={o.testId}>
          {o.icon && <o.icon aria-hidden />}
          {o.label}
        </button>
      ))}
    </div>
  );
}

export type OptionCard<V extends string> = { value: V; title: ReactNode; hint?: ReactNode; icon?: LucideIcon; disabled?: boolean; badge?: ReactNode; testId?: string };

/** Large choice cards with an icon, title and one-line explanation (wizard, install mode). */
export function OptionCards<V extends string>({ value, onChange, options, label, columns = 1 }: { value: V; onChange: (v: V) => void; options: OptionCard<V>[]; label: string; columns?: number }) {
  return (
    <div className="options" role="radiogroup" aria-label={label} style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>
      {options.map((o) => (
        <button key={o.value} type="button" className="option" role="radio" aria-checked={value === o.value} aria-pressed={value === o.value} disabled={o.disabled} onClick={() => onChange(o.value)} data-testid={o.testId}>
          {o.icon && <span className="option-icon"><o.icon aria-hidden /></span>}
          <span className="option-body">
            <span className="option-title">{o.title}{o.badge}</span>
            {o.hint && <span className="option-hint">{o.hint}</span>}
          </span>
          <span className="option-check" aria-hidden>{value === o.value && <Check />}</span>
        </button>
      ))}
    </div>
  );
}
