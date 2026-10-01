import { useEffect, useMemo, useState } from "react";
import { useI18n } from "../i18n";

export type PaletteItem = { id: string; label: string; onSelect: () => void };

/** `Ctrl+K` navigation (roadmap 2.2). Full cross-entity search is Phase 9;
 * this only jumps between the pages already in the sidebar. */
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
    <div className="dialog-backdrop" onClick={onClose} data-testid="command-palette">
      <div className="dialog" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true">
        <div className="dialog-header">
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={t("shell.commandPalette.placeholder")}
            data-testid="command-palette-input"
          />
        </div>
        <div className="dialog-body">
          {filtered.length === 0 ? (
            <div className="empty-state">{t("shell.commandPalette.empty")}</div>
          ) : (
            <ul className="command-list">
              {filtered.map((item, i) => (
                <li key={item.id}>
                  <button
                    type="button"
                    data-active={i === active}
                    onClick={() => {
                      item.onSelect();
                      onClose();
                    }}
                    data-testid={`command-item-${item.id}`}
                  >
                    {item.label}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
