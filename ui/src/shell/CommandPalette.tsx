import { useEffect, useMemo, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { CornerDownLeft, Search, SearchX } from "lucide-react";
import { useI18n } from "../i18n";
import { EmptyState } from "../ui/Feedback";

export type PaletteItem = { id: string; label: string; icon?: LucideIcon; onSelect: () => void };

/** `Ctrl+K` navigation (roadmap 2.2; Raycast pattern). Full cross-entity
 * search is Phase 9; this only jumps between the pages in the sidebar. */
export function CommandPalette({ items, onClose }: { items: PaletteItem[]; onClose: () => void }) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? items.filter((i) => i.label.toLowerCase().includes(q)) : items;
  }, [items, query]);
  const [active, setActive] = useState(0);

  useEffect(() => setActive(0), [query]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") return onClose();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => Math.min(filtered.length - 1, i + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(0, i - 1));
    } else if (e.key === "Enter" && filtered[active]) {
      filtered[active].onSelect();
      onClose();
    }
  };

  return (
    <div className="scrim palette-scrim" onPointerDown={(e) => e.target === e.currentTarget && onClose()} data-testid="command-palette">
      <div className="glass-overlay palette" role="dialog" aria-modal="true" aria-label={t("shell.commandPalette.title")}>
        <div className="palette-input">
          <Search aria-hidden />
          <input autoFocus value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={onKeyDown} placeholder={t("shell.commandPalette.placeholder")} data-testid="command-palette-input" />
          <kbd>Esc</kbd>
        </div>
        {filtered.length === 0 ? (
          <EmptyState icon={SearchX} title={t("shell.commandPalette.empty")} />
        ) : (
          <div className="palette-list" role="listbox">
            <div className="t-overline palette-group">{t("shell.commandPalette.pages")}</div>
            {filtered.map((item, i) => (
              <button
                key={item.id}
                type="button"
                role="option"
                aria-selected={i === active}
                className="palette-item"
                data-active={i === active}
                onMouseEnter={() => setActive(i)}
                onClick={() => {
                  item.onSelect();
                  onClose();
                }}
                data-testid={`command-item-${item.id}`}
              >
                {item.icon && <span className="palette-icon"><item.icon aria-hidden /></span>}
                <span>{item.label}</span>
                {i === active && <CornerDownLeft className="palette-enter flip-rtl" aria-hidden />}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
