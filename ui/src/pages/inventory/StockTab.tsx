// «موجودی» — what is on the shelf (8.1), what is running out (8.3) and what is expiring or expired (8.4),
// with the day-to-day stock operations: receive, take out, and the corrections of one batch (8.2).
import { useEffect, useState } from "react";
import { ArrowLeftRight, CalendarX2, ChevronRight, FlaskConical, Hourglass, Minus, PackageMinus, PackagePlus, Pencil, Plus, Power, Search, Trash2, Undo2, TriangleAlert, SlidersHorizontal } from "lucide-react";
import type { CalendarSystem, StockAlerts, StockBatchInfo, StockFilter, StockItemInfo, StockMovementInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { digits, formatDate } from "../../lib/dates";
import { BATCH_TONE, batchQ, formatQty, itemQ, moveQ, useStockLookup } from "../../lib/inventory";
import { useLive } from "../../lib/lan";
import { tr } from "../../lib/medical";
import { formatAmount } from "../../lib/money";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader, Stat } from "../../ui/Card";
import { Segmented, Switch } from "../../ui/Controls";
import { Badge, EmptyState, ErrorState, SkeletonRows } from "../../ui/Feedback";
import { Select, TextInput } from "../../ui/Field";
import { useConfirm } from "../../ui/ConfirmDialog";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { Level, canConsume } from "./common";
import type { InventoryPerms } from "./common";
import { AdjustDialog, ConsumeDialog, ItemFormDialog, ReceiveDialog, ReturnDialog, TransferDialog } from "./stockDialogs";

type Filter = "all" | StockFilter;
type Modal =
  | { kind: "item"; item?: StockItemInfo }
  | { kind: "receive"; item?: StockItemInfo }
  | { kind: "consume"; item: StockItemInfo }
  | { kind: "detail"; item: StockItemInfo };

export function StockTab({ perms, calendar, onOpenLab }: { perms: InventoryPerms; calendar: CalendarSystem; onOpenLab: () => void }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const categories = useStockLookup("categories");
  const [search, setSearch] = useState("");
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  // Switched-off items are hidden by default; this brings them back so they can be switched on again.
  const [showOff, setShowOff] = useState(false);
  const [items, setItems] = useState<StockItemInfo[] | null>(null);
  const [alerts, setAlerts] = useState<StockAlerts | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const [modal, setModal] = useState<Modal | null>(null);
  const reload = () => setTick((x) => x + 1);
  useLive(["stock", "stock_items", "purchases", "lab_cases", "suppliers"], reload);

  useEffect(() => {
    const id = window.setTimeout(() => setQuery(search.trim()), 200);
    return () => window.clearTimeout(id);
  }, [search]);
  useEffect(() => {
    rpc("stock_items.list", itemQ({ search: query || null, category_id: category || null, only: filter === "all" ? null : filter, include_inactive: showOff }))
      .then((r) => {
        setItems(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [query, category, filter, showOff, tick]);
  useEffect(() => {
    rpc("stock_alerts.list", { expiring_days: null }).then(setAlerts).catch(() => {});
  }, [tick]);

  if (error && !items) return <ErrorState message={error} onRetry={reload} />;
  const done = (message: string) => {
    setModal(null);
    toast.success(message);
    reload();
  };
  const unitOf = (i: StockItemInfo) => tr(i.unit, lang);
  const asked = !!query || !!category || filter !== "all";
  return (
    <div className="stack" data-testid="stock-tab">
      <div className="stat-grid four">
        <Stat icon={PackageMinus} label={t("stock.alert.low")} value={alerts ? digits(alerts.low.length, lang) : "…"} tone={alerts?.low.length ? "warning" : "success"} testId="stock-alert-low" />
        <Stat icon={CalendarX2} label={t("stock.alert.expired")} value={alerts ? digits(alerts.expired.length, lang) : "…"} tone={alerts?.expired.length ? "danger" : "success"} sub={t("stock.alert.batches")} testId="stock-alert-expired" />
        <Stat icon={Hourglass} label={t("stock.alert.expiring")} value={alerts ? digits(alerts.expiring.length, lang) : "…"} tone={alerts?.expiring.length ? "warning" : "success"} sub={t("stock.alert.batches")} testId="stock-alert-expiring" />
        <button type="button" className="stat-link" onClick={onOpenLab} data-testid="stock-alert-lab" title={t("stock.alert.labOpen")}>
          <Stat icon={FlaskConical} label={t("stock.alert.lab")} value={alerts ? digits(alerts.lab_delayed.length, lang) : "…"} tone={alerts?.lab_delayed.length ? "danger" : "success"} sub={t("stock.alert.labCases")} />
        </button>
      </div>

      <Card flush>
        <CardHeader
          icon={SlidersHorizontal}
          title={t("stock.title")}
          description={t("stock.hint")}
          actions={
            perms.manage ? (
              <>
                <Button icon={Plus} onClick={() => setModal({ kind: "item" })} data-testid="item-add-open">{t("stock.item.new")}</Button>
                <Button variant="primary" icon={PackagePlus} onClick={() => setModal({ kind: "receive" })} data-testid="receive-open">{t("stock.receive.title")}</Button>
              </>
            ) : undefined
          }
        />
        <div className="row toolbar">
          <div className="toolbar-search">
            <TextInput icon={Search} value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("stock.search")} aria-label={t("stock.search")} data-testid="stock-search" />
          </div>
          <Select value={category} onChange={(e) => setCategory(e.target.value)} aria-label={t("stock.item.category")} data-testid="stock-category">
            <option value="">{t("stock.allCategories")}</option>
            {categories.items?.map((c) => <option key={c.id} value={c.id}>{tr(c.name, lang)}</option>)}
          </Select>
          <Segmented<Filter>
            value={filter}
            onChange={setFilter}
            label={t("stock.filter")}
            options={[
              { value: "all", label: t("stock.filter.all"), testId: "stock-filter-all" },
              { value: "low", label: t("stock.filter.low"), testId: "stock-filter-low" },
              { value: "expiring", label: t("stock.filter.expiring"), testId: "stock-filter-expiring" },
              { value: "expired", label: t("stock.filter.expired"), testId: "stock-filter-expired" },
            ]}
          />
          <div className="stock-show-off">
            <Switch checked={showOff} onChange={setShowOff} label={t("stock.showInactive")} testId="stock-show-inactive" />
          </div>
        </div>
        <div className="table-wrap">
          <table className="table" data-testid="stock-table">
            <thead>
              <tr>
                <th>{t("stock.col.item")}</th>
                <th>{t("stock.col.inStock")}</th>
                <th>{t("stock.col.status")}</th>
                <th>{t("stock.col.nearestExpiry")}</th>
                <th className="num">{t("stock.col.value")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {!items && <SkeletonRows cols={6} />}
              {items?.map((i) => (
                <tr key={i.id} data-testid={`stock-row-${i.id}`}>
                  <td>
                    <button type="button" className="link-btn plain" onClick={() => setModal({ kind: "detail", item: i })} data-testid={`stock-open-${i.id}`}>{tr(i.name, lang)}</button>
                    <div className="t-caption subtle">{tr(i.category, lang)}{i.code ? ` · ${i.code}` : ""}{!i.is_active ? ` · ${t("stock.inactive")}` : ""}</div>
                  </td>
                  <td>
                    <div className="qty-cell">
                      <span className="num cell-strong">{formatQty(i.usable_qty, lang)}</span>
                      <span className="t-caption subtle">{unitOf(i)}</span>
                      <Level qty={i.usable_qty} min={i.min_qty} />
                    </div>
                    {i.min_qty > 0 && <div className="t-caption subtle">{t("stock.minShort")} {formatQty(i.min_qty, lang)}</div>}
                  </td>
                  <td>
                    <div className="row tight">
                      {i.qty === 0 && <Badge tone="neutral" testId={`stock-out-${i.id}`}>{t("stock.status.out")}</Badge>}
                      {i.low && i.qty > 0 && <Badge tone="warning" icon={TriangleAlert} testId={`stock-low-${i.id}`}>{t("stock.status.low")}</Badge>}
                      {i.expired_qty > 0 && <Badge tone="danger" icon={CalendarX2} testId={`stock-expired-${i.id}`}>{t("stock.status.expired")} · {formatQty(i.expired_qty, lang)}</Badge>}
                      {i.expiring_qty > 0 && <Badge tone="warning" icon={Hourglass} testId={`stock-expiring-${i.id}`}>{t("stock.status.expiring")} · {formatQty(i.expiring_qty, lang)}</Badge>}
                      {i.qty > 0 && !i.low && !i.expired_qty && !i.expiring_qty && <Badge tone="success" dot>{t("stock.status.ok")}</Badge>}
                    </div>
                  </td>
                  <td>{i.nearest_expiry ? formatDate(i.nearest_expiry, lang, calendar) : "—"}</td>
                  <td className="num">{formatAmount(i.value, lang)}</td>
                  <td className="cell-actions">
                    {canConsume(perms) && <IconButton icon={Minus} label={t("stock.consume.title")} disabled={i.usable_qty <= 0} onClick={() => setModal({ kind: "consume", item: i })} data-testid={`stock-consume-${i.id}`} />}
                    {perms.manage && <IconButton icon={PackagePlus} label={t("stock.receive.title")} onClick={() => setModal({ kind: "receive", item: i })} data-testid={`stock-receive-${i.id}`} />}
                    <IconButton icon={ChevronRight} label={t("stock.details")} className="flip-rtl" onClick={() => setModal({ kind: "detail", item: i })} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {items && !items.length && (
          <EmptyState
            icon={PackageMinus}
            title={t(asked ? "stock.none.filtered" : "stock.none")}
            action={!asked && perms.manage ? <Button variant="primary" icon={Plus} onClick={() => setModal({ kind: "item" })}>{t("stock.item.new")}</Button> : undefined}
          >
            {t(asked ? "stock.none.filteredHint" : "stock.noneHint")}
          </EmptyState>
        )}
      </Card>

      {modal?.kind === "item" && <ItemFormDialog item={modal.item} onClose={() => setModal(null)} onDone={() => done(t("stock.item.saved"))} />}
      {modal?.kind === "receive" && <ReceiveDialog item={modal.item} calendar={calendar} onClose={() => setModal(null)} onDone={() => done(t("stock.receive.done"))} />}
      {modal?.kind === "consume" && <ConsumeDialog item={modal.item} onClose={() => setModal(null)} onDone={() => done(t("stock.consume.done"))} />}
      {modal?.kind === "detail" && (
        <ItemDetail
          item={modal.item}
          perms={perms}
          calendar={calendar}
          tick={tick}
          onClose={() => setModal(null)}
          onChanged={reload}
          onEdit={(i) => setModal({ kind: "item", item: i })}
          onReceive={(i) => setModal({ kind: "receive", item: i })}
          onConsume={(i) => setModal({ kind: "consume", item: i })}
        />
      )}
    </div>
  );
}

// ───────────────────────────── one item: batches and history ─────────────────────────────

function ItemDetail({ item, perms, calendar, tick, onClose, onChanged, onEdit, onReceive, onConsume }: {
  item: StockItemInfo;
  perms: InventoryPerms;
  calendar: CalendarSystem;
  tick: number;
  onClose: () => void;
  onChanged: () => void;
  onEdit: (i: StockItemInfo) => void;
  onReceive: (i: StockItemInfo) => void;
  onConsume: (i: StockItemInfo) => void;
}) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [info, setInfo] = useState(item);
  const [batches, setBatches] = useState<StockBatchInfo[] | null>(null);
  const [moves, setMoves] = useState<StockMovementInfo[] | null>(null);
  const [error, setError] = useState("");
  const [local, setLocal] = useState(0);
  const [sub, setSub] = useState<{ kind: "adjust" | "transfer" | "return"; batch: StockBatchInfo } | null>(null);
  useEffect(() => {
    Promise.all([rpc("stock_items.get", { id: item.id }), rpc("stock_batches.list", batchQ({ item_id: item.id })), rpc("stock_movements.list", moveQ({ item_id: item.id }))])
      .then(([i, b, m]) => {
        setInfo(i);
        setBatches(b);
        setMoves(m);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [item.id, tick, local]);
  const changed = (message: string) => {
    setSub(null);
    toast.success(message);
    setLocal((x) => x + 1);
    onChanged();
  };
  // Switch the item off or on again: the same save as the form, with only the active flag changed.
  const toggleActive = async () => {
    try {
      await rpc("stock_items.save", {
        id: info.id,
        code: info.code,
        name: info.name,
        category_id: info.category_id,
        unit_id: info.unit_id,
        min_qty: info.min_qty,
        note: info.note,
        is_active: !info.is_active,
        version: info.version,
      });
      toast.success(t(info.is_active ? "stock.item.deactivated" : "stock.item.reactivated"));
      setLocal((x) => x + 1);
      onChanged();
    } catch (e) {
      if (!isSessionError(e)) setError(err(e));
    }
  };
  // Delete: an item with history is only archived by the core, and the message says which one happened.
  const removeItem = async () => {
    if (!(await confirm({ title: t("cleanup.confirm.title"), text: t("stock.item.deleteConfirm"), confirm: t("common.delete"), danger: true }))) return;
    try {
      const r = await rpc("stock_items.delete", { id: info.id });
      toast.success(t(r.archived ? "stock.item.archived" : "stock.item.deleted"));
      onChanged();
      onClose();
    } catch (e) {
      if (!isSessionError(e)) setError(err(e));
    }
  };
  const unit = tr(info.unit, lang);
  const sign = (q: number) => `${q > 0 ? "+" : "−"}${formatQty(Math.abs(q), lang)}`;
  return (
    <Dialog
      title={tr(info.name, lang)}
      description={`${tr(info.category, lang)}${info.code ? ` · ${info.code}` : ""}`}
      onClose={onClose}
      wide
      testId="item-detail"
      footer={
        <>
          {perms.manage && <Button icon={Pencil} onClick={() => onEdit(info)} data-testid="item-edit-open">{t("common.edit")}</Button>}
          {perms.manage && <Button icon={Power} onClick={() => void toggleActive()} data-testid="item-toggle-active">{t(info.is_active ? "stock.item.deactivate" : "stock.item.reactivate")}</Button>}
          {perms.manage && <Button variant="danger" icon={Trash2} onClick={() => void removeItem()} data-testid="item-delete">{t("common.delete")}</Button>}
          {canConsume(perms) && <Button icon={Minus} disabled={info.usable_qty <= 0} onClick={() => onConsume(info)}>{t("stock.consume.title")}</Button>}
          {perms.manage && <Button variant="primary" icon={PackagePlus} onClick={() => onReceive(info)}>{t("stock.receive.title")}</Button>}
        </>
      }
    >
      <div className="stack">
        {error && <ErrorState message={error} onRetry={() => setLocal((x) => x + 1)} />}
        <dl className="facts">
          <div><dt>{t("stock.col.inStock")}</dt><dd className="num cell-strong">{formatQty(info.usable_qty, lang)} {unit}</dd></div>
          <div><dt>{t("stock.item.min")}</dt><dd className="num">{info.min_qty ? `${formatQty(info.min_qty, lang)} ${unit}` : "—"}</dd></div>
          <div><dt>{t("stock.col.value")}</dt><dd className="num">{formatAmount(info.value, lang)}</dd></div>
          <div><dt>{t("stock.col.nearestExpiry")}</dt><dd>{info.nearest_expiry ? formatDate(info.nearest_expiry, lang, calendar) : "—"}</dd></div>
        </dl>

        <h3 className="t-subtitle">{t("stock.batches")}</h3>
        <div className="table-wrap">
          <table className="table" data-testid="item-batches">
            <thead>
              <tr>
                <th>{t("stock.lot")}</th>
                <th>{t("stock.place")}</th>
                <th>{t("stock.expiry")}</th>
                <th className="num">{t("stock.qtyLeft")}</th>
                <th className="num">{t("stock.unitCost")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {!batches && <SkeletonRows cols={6} rows={2} />}
              {batches?.map((b) => (
                <tr key={b.id} data-testid={`batch-${b.id}`}>
                  <td><bdi className="ltr">{b.lot_no ?? "—"}</bdi>{b.supplier_name && <div className="t-caption subtle">{b.supplier_name}</div>}</td>
                  <td>{tr(b.location, lang)}</td>
                  <td>
                    {b.expiry_date ? formatDate(b.expiry_date, lang, calendar) : "—"}
                    {b.expiry_date && b.state !== "valid" && <div><Badge tone={BATCH_TONE[b.state]} icon={b.state === "expired" ? CalendarX2 : Hourglass}>{t(`stock.batch.${b.state}`)}</Badge></div>}
                  </td>
                  <td className="num cell-strong">{formatQty(b.qty_left, lang)} <span className="t-caption subtle">{unit}</span></td>
                  <td className="num">{formatAmount(b.unit_cost, lang)}</td>
                  <td className="cell-actions">
                    {perms.manage && (
                      <>
                        <IconButton icon={SlidersHorizontal} label={t("stock.adjust.title")} onClick={() => setSub({ kind: "adjust", batch: b })} data-testid={`batch-adjust-${b.id}`} />
                        <IconButton icon={ArrowLeftRight} label={t("stock.transfer.title")} onClick={() => setSub({ kind: "transfer", batch: b })} data-testid={`batch-transfer-${b.id}`} />
                        <IconButton icon={Undo2} label={t("stock.return.title")} onClick={() => setSub({ kind: "return", batch: b })} data-testid={`batch-return-${b.id}`} />
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {batches && !batches.length && <p className="muted pad">{t("stock.batches.none")}</p>}
        </div>

        <h3 className="t-subtitle">{t("stock.history")}</h3>
        <div className="table-wrap">
          <table className="table" data-testid="item-history">
            <thead>
              <tr>
                <th>{t("doc.date")}</th>
                <th>{t("stock.move.kind")}</th>
                <th className="num">{t("stock.qty")}</th>
                <th>{t("stock.lot")}</th>
                <th>{t("stock.move.by")}</th>
                <th>{t("stock.reason")}</th>
              </tr>
            </thead>
            <tbody>
              {!moves && <SkeletonRows cols={6} rows={2} />}
              {moves?.map((m) => (
                <tr key={m.id}>
                  <td>{formatDate(m.local_date, lang, calendar)}</td>
                  <td>{t(`stock.move.${m.kind}`)}</td>
                  <td className={`num cell-strong ${m.qty > 0 ? "money-success" : ""}`}><bdi className="ltr">{sign(m.qty)}</bdi></td>
                  <td><bdi className="ltr">{m.lot_no ?? "—"}</bdi></td>
                  <td>{m.created_by_name ?? "—"}</td>
                  <td>{m.reason ?? ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {moves && !moves.length && <p className="muted pad">{t("stock.history.none")}</p>}
        </div>
      </div>
      {dialog}
      {sub?.kind === "adjust" &&<AdjustDialog batch={sub.batch} onClose={() => setSub(null)} onDone={() => changed(t("stock.adjust.done"))} />}
      {sub?.kind === "transfer" && <TransferDialog batch={sub.batch} onClose={() => setSub(null)} onDone={() => changed(t("stock.transfer.done"))} />}
      {sub?.kind === "return" && <ReturnDialog batch={sub.batch} onClose={() => setSub(null)} onDone={() => changed(t("stock.return.done"))} />}
    </Dialog>
  );
}
