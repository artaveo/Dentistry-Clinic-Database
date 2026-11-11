import { useEffect, useId, useMemo, useRef, useState } from "react";
import { ChevronDown, Plus } from "lucide-react";
import { latinDigits } from "../lib/dates";
import { useI18n } from "../i18n";
import { FloatingPanel } from "./Overlay";
import type { FloatingCloseReason } from "./Overlay";

export type ComboOption = { value: string; label: string; /** Second line / extra search text (code, unit, phone…). */ hint?: string; disabled?: boolean };

/** Search key: lower case, Latin digits, Arabic ي/ك as ی/ک, no zero-width marks. */
const norm = (s: string) => latinDigits(s).toLowerCase().replace(/ي/g, "ی").replace(/ك/g, "ک").replace(/[‌‎‏]/g, "").trim();

const MAX_SHOWN = 200;

/**
 * Searchable, themed dropdown for long lists (items, services, patients). Type to filter; ↑ ↓ move,
 * Enter picks, Escape closes (the typed text is dropped), Tab leaves. `value` is an option value or
 * "" for none. With `onCreate` the list ends in a "+ add new" row that receives the typed text.
 * For short lists (doctor, place) keep `<Select>`.
 */
export function Combobox({ options, value, onChange, placeholder, emptyText, onCreate, createLabel, disabled, invalid, id, label, "aria-describedby": describedBy, testId }: {
  options: ComboOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  /** Shown when nothing matches (default: "Nothing found"). */
  emptyText?: string;
  /** Adds the footer action; gets the text typed so far (may be ""). */
  onCreate?: (query: string) => void;
  /** Label of the footer action, e.g. "+ New item". */
  createLabel?: string;
  disabled?: boolean;
  /** Set by `<Field>`. */
  invalid?: boolean;
  id?: string;
  /** Accessible name when there is no `<Field>` label. */
  label?: string;
  "aria-describedby"?: string;
  testId?: string;
}) {
  const { t } = useI18n();
  const autoId = useId();
  const inputId = id ?? autoId;
  const listId = `${autoId}-list`;
  const anchorRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState<string | null>(null); // null = show the chosen option's label
  const [active, setActive] = useState(0);

  const selected = options.find((o) => o.value === value);
  const q = query === null ? "" : norm(query);
  const matches = useMemo(() => {
    const list = q ? options.filter((o) => norm(`${o.label} ${o.hint ?? ""}`).includes(q)) : options;
    return list;
  }, [options, q]);
  const shown = matches.slice(0, MAX_SHOWN);
  const hasCreate = !!onCreate;
  const rowCount = shown.length + (hasCreate ? 1 : 0);

  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-row="${active}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [active, open]);

  const openList = () => {
    if (disabled) return;
    if (!open) {
      const at = shown.findIndex((o) => o.value === value);
      setActive(at >= 0 ? at : 0);
      setOpen(true);
    }
  };
  const close = (reason?: FloatingCloseReason) => {
    setOpen(false);
    setQuery(null);
    if (reason && reason !== "outside") inputRef.current?.focus();
  };
  const choose = (row: number) => {
    if (row < shown.length) {
      const o = shown[row];
      if (o.disabled) return;
      onChange(o.value);
      close("escape");
    } else if (hasCreate) {
      const text = (query ?? "").trim();
      close("escape");
      onCreate?.(text);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!open) {
        openList();
        return;
      }
      if (rowCount > 0) setActive((a) => (e.key === "ArrowDown" ? (a + 1) % rowCount : (a - 1 + rowCount) % rowCount));
    } else if (e.key === "Enter") {
      if (open && rowCount > 0) {
        e.preventDefault();
        choose(Math.min(active, rowCount - 1));
      }
    } else if (e.key === "Tab" && open) {
      close();
    }
  };

  const optId = (i: number) => `${autoId}-opt-${i}`;
  const text = query ?? selected?.label ?? "";

  return (
    <div className="bu-combo" ref={anchorRef} data-testid={testId && `${testId}-field`}>
      <div className="control" data-invalid={invalid || undefined} data-disabled={disabled || undefined}>
        <input
          ref={inputRef}
          id={inputId}
          role="combobox"
          aria-expanded={open}
          aria-controls={open ? listId : undefined}
          aria-autocomplete="list"
          aria-activedescendant={open && rowCount > 0 ? optId(Math.min(active, rowCount - 1)) : undefined}
          aria-label={label}
          aria-invalid={invalid || undefined}
          aria-describedby={describedBy}
          autoComplete="off"
          spellCheck={false}
          disabled={disabled}
          placeholder={placeholder}
          value={text}
          onFocus={(e) => {
            e.currentTarget.select();
          }}
          onClick={openList}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
            setOpen(true);
          }}
          onKeyDown={onKeyDown}
          data-testid={testId}
        />
        <button type="button" className="control-action" tabIndex={-1} disabled={disabled} aria-label={t("ui.combo.open")} title={t("ui.combo.open")} onClick={() => (open ? close("escape") : (inputRef.current?.focus(), openList()))}>
          <ChevronDown aria-hidden />
        </button>
      </div>
      {open && (
        <FloatingPanel anchorRef={anchorRef} onClose={close} role="listbox" className="bu-combo-pop" matchAnchorWidth testId={testId && `${testId}-list`}>
          <div ref={listRef} id={listId} className="bu-combo-list" role="presentation">
            {shown.map((o, i) => (
              <div
                key={o.value}
                id={optId(i)}
                data-row={i}
                role="option"
                aria-selected={o.value === value}
                aria-disabled={o.disabled || undefined}
                className={["bu-combo-opt", i === active && "active", o.value === value && "chosen", o.disabled && "disabled"].filter(Boolean).join(" ")}
                onPointerDown={(e) => e.preventDefault()}
                onMouseMove={() => active !== i && setActive(i)}
                onClick={() => choose(i)}
                data-testid={testId && `${testId}-opt-${o.value}`}
              >
                <span className="bu-combo-label">{o.label}</span>
                {o.hint && <span className="bu-combo-hint">{o.hint}</span>}
              </div>
            ))}
            {shown.length === 0 && <div className="bu-combo-empty">{emptyText ?? t("ui.combo.empty")}</div>}
            {matches.length > shown.length && <div className="bu-combo-empty">{t("ui.combo.more")}</div>}
          </div>
          {hasCreate && (
            <div
              id={optId(shown.length)}
              data-row={shown.length}
              role="option"
              aria-selected={false}
              className={["bu-combo-opt", "bu-combo-create", active === shown.length && "active"].filter(Boolean).join(" ")}
              onPointerDown={(e) => e.preventDefault()}
              onMouseMove={() => active !== shown.length && setActive(shown.length)}
              onClick={() => choose(shown.length)}
              data-testid={testId && `${testId}-create`}
            >
              <Plus aria-hidden />
              <span className="bu-combo-label">{createLabel ?? t("ui.combo.create")}</span>
            </div>
          )}
        </FloatingPanel>
      )}
    </div>
  );
}
