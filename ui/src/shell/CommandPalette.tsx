import { useEffect, useMemo, useRef, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { CalendarDays, CornerDownLeft, FlaskConical, Loader2, ReceiptText, Receipt, Search, SearchX, UserRound } from "lucide-react";
import type { CalendarSystem, SearchHit, SearchKind } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { formatDate } from "../lib/dates";
import { useI18n } from "../i18n";
import { EmptyState } from "../ui/Feedback";

/** `group` is a translated heading; items without one are listed under "Pages". */
export type PaletteItem = { id: string; label: string; icon?: LucideIcon; group?: string; onSelect: () => void };

const KIND_ICON: Record<SearchKind, LucideIcon> = { patient: UserRound, appointment: CalendarDays, invoice: ReceiptText, receipt: Receipt, lab_case: FlaskConical };
const KINDS: SearchKind[] = ["patient", "appointment", "invoice", "receipt", "lab_case"];
const MIN_CHARS = 2;
const DEBOUNCE_MS = 180;

type Entry = { id: string; group: string; label: string; sub?: string; date?: string; icon?: LucideIcon; run: () => void };

/** `Ctrl+K` (roadmap 2.2; Raycast pattern): jumps between pages and, since Phase 9, finds patients, visits,
 * invoices, receipts and lab cases from one box (9.7). The Core answers from its indexes, not by scanning. */
export function CommandPalette({ items, calendar, hideLabCases, onOpenHit, onClose }: { items: PaletteItem[]; calendar: CalendarSystem; /** Lab work belongs to the stock module: no lab-case hits when it is off. */ hideLabCases?: boolean; onOpenHit: (hit: SearchHit) => void; onClose: () => void }) {
  const { t, lang } = useI18n();
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [searched, setSearched] = useState("");
  const seq = useRef(0);
  const q = query.trim();

  useEffect(() => {
    if ([...q].length < MIN_CHARS) {
      seq.current++;
      setHits([]);
      setSearching(false);
      setSearched("");
      return;
    }
    const mine = ++seq.current;
    setSearching(true);
    const id = window.setTimeout(() => {
      rpc("search.global", { query: q, per_kind: 5 })
        .then((r) => {
          if (mine !== seq.current) return;
          setHits(r.hits);
          setSearched(q);
        })
        .catch((e) => {
          if (mine === seq.current && !isSessionError(e)) setHits([]);
        })
        .finally(() => mine === seq.current && setSearching(false));
    }, DEBOUNCE_MS);
    return () => window.clearTimeout(id);
  }, [q]);

  const entries = useMemo<Entry[]>(() => {
    const lower = q.toLowerCase();
    const pages = (lower ? items.filter((i) => i.label.toLowerCase().includes(lower)) : items).map<Entry>((i) => ({ id: i.id, group: i.group ?? t("shell.commandPalette.pages"), label: i.label, icon: i.icon, run: i.onSelect }));
    const found = KINDS.filter((k) => !(hideLabCases && k === "lab_case")).flatMap((k) =>
      hits.filter((h) => h.kind === k).map<Entry>((h) => ({ id: `${h.kind}-${h.id}`, group: t(`search.kind.${h.kind}`), label: h.title, sub: h.subtitle ?? undefined, date: h.date ?? undefined, icon: KIND_ICON[h.kind], run: () => onOpenHit(h) })),
    );
    // What was found comes first once a search has an answer: that is what the person typed for.
    return found.length ? [...found, ...pages] : pages;
  }, [items, hits, q, t, onOpenHit, hideLabCases]);
  const [active, setActive] = useState(0);
  useEffect(() => setActive(0), [q, hits]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") return onClose();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => Math.min(entries.length - 1, i + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(0, i - 1));
    } else if (e.key === "Enter" && entries[active]) {
      entries[active].run();
      onClose();
    }
  };

  const noResults = entries.length === 0 && !searching;
  return (
    <div className="scrim palette-scrim" onPointerDown={(e) => e.target === e.currentTarget && onClose()} data-testid="command-palette">
      <div className="glass-overlay palette" role="dialog" aria-modal="true" aria-label={t("shell.commandPalette.title")}>
        <div className="palette-input">
          {searching ? <Loader2 className="spin" aria-hidden /> : <Search aria-hidden />}
          <input autoFocus value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={onKeyDown} placeholder={t("shell.commandPalette.placeholder")} role="combobox" aria-expanded="true" aria-controls="palette-list" aria-activedescendant={entries[active] ? `pal-${entries[active].id}` : undefined} data-testid="command-palette-input" />
          <kbd>Esc</kbd>
        </div>
        {noResults ? (
          <EmptyState icon={SearchX} title={t("shell.commandPalette.empty")}><span data-testid="palette-no-hits">{searched ? t("shell.commandPalette.noRecords").replace("{q}", searched) : t("shell.commandPalette.tryThis")}</span></EmptyState>
        ) : (
          <div className="palette-list" role="listbox" id="palette-list">
            {entries.map((item, i) => (
              <div key={item.id}>
                {(i === 0 || entries[i - 1].group !== item.group) && <div className="t-overline palette-group">{item.group}</div>}
                <button
                  type="button"
                  id={`pal-${item.id}`}
                  role="option"
                  aria-selected={i === active}
                  className="palette-item"
                  data-active={i === active}
                  onMouseEnter={() => setActive(i)}
                  onClick={() => {
                    item.run();
                    onClose();
                  }}
                  data-testid={`command-item-${item.id}`}
                >
                  {item.icon && <span className="palette-icon"><item.icon aria-hidden /></span>}
                  <span className="palette-text">
                    <span>{item.label}</span>
                    {item.sub && <span className="subtle palette-sub">{item.sub}</span>}
                  </span>
                  {item.date && <span className="subtle palette-date num">{formatDate(item.date, lang, calendar)}</span>}
                  {i === active && <CornerDownLeft className="palette-enter flip-rtl" aria-hidden />}
                </button>
              </div>
            ))}
            {searched && !searching && hits.length === 0 && <div className="palette-note subtle" data-testid="palette-no-hits">{t("shell.commandPalette.noRecords").replace("{q}", searched)}</div>}
          </div>
        )}
      </div>
    </div>
  );
}
