// «تنظیمات انبار» — the clinic's own lists (categories, units, places, 8.1) and the materials a service
// uses (8.2): when a treatment of that service is done, its materials can be taken out of stock in one step.
import { useEffect, useState } from "react";
import { Boxes, Layers, MapPin, Pencil, Plus, Ruler, Save, Trash2 } from "lucide-react";
import type { StockLookupInfo, Translations } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { serviceName, useCatalog } from "../../lib/clinical";
import { parseQty, qtyInput, qtyRule } from "../../lib/inventory";
import type { LookupKind } from "../../lib/inventory";
import { useStockLookup } from "../../lib/inventory";
import { tr } from "../../lib/medical";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Switch } from "../../ui/Controls";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, Select } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { TranslationsField } from "../../ui/TranslationsField";
import { useToast } from "../../ui/Toast";
import { QtyInput, useAction, useItemOptions } from "./common";

const hasName = (n: Translations) => !!(n.fa.trim() || n.ps.trim() || n.en.trim());

const LOOKUPS: { kind: LookupKind; icon: typeof Layers }[] = [
  { kind: "categories", icon: Layers },
  { kind: "units", icon: Ruler },
  { kind: "locations", icon: MapPin },
];

export function InventorySettings() {
  const { t } = useI18n();
  return (
    <div className="stack" data-testid="inventory-settings">
      {LOOKUPS.map((l) => <LookupCard key={l.kind} kind={l.kind} icon={l.icon} />)}
      <MaterialsCard />
      <Notice tone="info">{t("invset.permHint")}</Notice>
    </div>
  );
}

function saveLookup(kind: LookupKind, p: { id: string | null; name: Translations; is_active: boolean; version: number | null }) {
  return kind === "categories" ? rpc("stock_categories.save", p) : kind === "units" ? rpc("stock_units.save", p) : rpc("stock_locations.save", p);
}

function LookupCard({ kind, icon }: { kind: LookupKind; icon: typeof Layers }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const { items, error, reload } = useStockLookup(kind, true);
  const [editing, setEditing] = useState<StockLookupInfo | "new" | null>(null);
  const toggle = async (i: StockLookupInfo) => {
    try {
      await saveLookup(kind, { id: i.id, name: i.name, is_active: !i.is_active, version: i.version });
      reload();
    } catch (e) {
      if (!isSessionError(e)) toast.error(err(e));
    }
  };
  return (
    <Card flush data-testid={`lookup-${kind}`}>
      <CardHeader icon={icon} title={t(`invset.${kind}`)} description={t(`invset.${kind}.hint`)} actions={<Button icon={Plus} onClick={() => setEditing("new")} data-testid={`lookup-add-${kind}`}>{t("common.add")}</Button>} />
      {error ? (
        <ErrorState message={err(error)} onRetry={reload} />
      ) : (
        <div className="table-wrap">
          <table className="table">
            <tbody>
              {!items && <SkeletonRows cols={3} rows={3} />}
              {items?.map((i) => (
                <tr key={i.id} data-testid={`lookup-row-${i.id}`}>
                  <td className="cell-strong">{tr(i.name, lang)}{i.is_system && <> <Badge>{t("invset.system")}</Badge></>}</td>
                  <td><Switch checked={i.is_active} onChange={() => toggle(i)} label={t("supplier.active")} testId={`lookup-active-${i.id}`} /></td>
                  <td className="cell-actions"><IconButton icon={Pencil} label={t("common.edit")} onClick={() => setEditing(i)} data-testid={`lookup-edit-${i.id}`} /></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {editing && <LookupDialog kind={kind} item={editing === "new" ? undefined : editing} onClose={() => setEditing(null)} onDone={() => { setEditing(null); toast.success(t("invset.saved")); reload(); }} />}
    </Card>
  );
}

function LookupDialog({ kind, item, onClose, onDone }: { kind: LookupKind; item?: StockLookupInfo; onClose: () => void; onDone: () => void }) {
  const { t } = useI18n();
  const [name, setName] = useState<Translations>(item?.name ?? { fa: "", ps: "", en: "" });
  const [touched, setTouched] = useState(false);
  const { busy, error, run } = useAction();
  const invalid = touched && !hasName(name);
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (!hasName(name)) return;
    void run(async () => {
      await saveLookup(kind, { id: item?.id ?? null, name, is_active: item?.is_active ?? true, version: item?.version ?? null });
      onDone();
    });
  };
  return (
    <Dialog
      dismissOnScrim={false}
      title={t(item ? "invset.edit" : "invset.add")}
      onClose={onClose}
      wide
      testId="lookup-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Save} loading={busy} onClick={submit} data-testid="lookup-save">{t("common.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("invset.name")} value={name} onChange={setName} error={invalid ? t("rule.required") : null} testId="lookup-name" />
      </form>
    </Dialog>
  );
}

// ───────────────────────────── materials of a service ─────────────────────────────

type Row = { item: string; qty: string };

function MaterialsCard() {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const catalog = useCatalog();
  const items = useItemOptions();
  const [service, setService] = useState("");
  const [rows, setRows] = useState<Row[] | null>(null);
  const [error, setError] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const { busy, error: saveError, run } = useAction();
  const services = (catalog?.services ?? []).filter((s) => s.is_active);

  useEffect(() => {
    if (!service) return setRows(null);
    setSubmitted(false);
    rpc("service_materials.list", { service_id: service })
      .then((r) => {
        setRows(r.map((m) => ({ item: m.item_id, qty: qtyInput(m.qty) })));
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [service]);

  const patch = (i: number, p: Partial<Row>) => setRows((rs) => rs?.map((r, k) => (k === i ? { ...r, ...p } : r)) ?? null);
  const rule = (r: Row) => (!r.item ? "rule.required" : qtyRule(r.qty));
  const save = () => {
    setSubmitted(true);
    if (!rows || rows.some(rule)) return;
    void run(async () => {
      const saved = await rpc("service_materials.save", { service_id: service, materials: rows.map((r) => ({ item_id: r.item, qty: parseQty(r.qty) ?? 0 })) });
      setRows(saved.map((m) => ({ item: m.item_id, qty: qtyInput(m.qty) })));
      toast.success(t("invset.materials.saved"));
    });
  };
  return (
    <Card data-testid="materials-card">
      <CardHeader icon={Boxes} title={t("invset.materials")} description={t("invset.materials.hint")} />
      <div className="stack">
        <Field label={t("invset.materials.service")}>
          <Select value={service} onChange={(e) => setService(e.target.value)} data-testid="materials-service">
            <option value="">{t("invset.materials.pick")}</option>
            {services.map((s) => <option key={s.id} value={s.id}>{serviceName(s, catalog, lang)}</option>)}
          </Select>
        </Field>
        {error && <ErrorState message={error} />}
        {saveError && <Notice tone="danger">{saveError}</Notice>}
        {rows && (
          <>
            {!rows.length && <EmptyState icon={Boxes} title={t("invset.materials.none")}>{t("invset.materials.noneHint")}</EmptyState>}
            <div className="stack" data-testid="materials-rows">
              {rows.map((r, i) => {
                const item = items?.find((x) => x.id === r.item);
                return (
                  <div className="mat-row" key={i} data-testid={`material-${i}`}>
                    <Field label={t("stock.col.item")} error={submitted && !r.item ? t("rule.required") : null}>
                      <Select value={r.item} onChange={(e) => patch(i, { item: e.target.value })} data-testid={`material-item-${i}`}>
                        <option value="">{t("stock.pick.item")}</option>
                        {items?.map((x) => <option key={x.id} value={x.id}>{tr(x.name, lang)}</option>)}
                      </Select>
                    </Field>
                    <Field label={t("invset.materials.qty")} error={submitted && r.item && qtyRule(r.qty) ? t(qtyRule(r.qty)!) : null}>
                      <QtyInput value={r.qty} onChange={(v) => patch(i, { qty: v })} unit={item ? tr(item.unit, lang) : undefined} testId={`material-qty-${i}`} />
                    </Field>
                    <IconButton icon={Trash2} label={t("purchase.removeLine")} onClick={() => setRows((rs) => rs?.filter((_, k) => k !== i) ?? null)} data-testid={`material-remove-${i}`} />
                  </div>
                );
              })}
            </div>
            <div className="row">
              <Button icon={Plus} onClick={() => setRows((rs) => [...(rs ?? []), { item: "", qty: "" }])} data-testid="material-add">{t("invset.materials.add")}</Button>
              <Button variant="primary" icon={Save} loading={busy} onClick={save} data-testid="materials-save">{t("common.save")}</Button>
              {rows.length > 0 && <span className="t-caption subtle">{t("invset.materials.perUnit")}</span>}
            </div>
          </>
        )}
      </div>
    </Card>
  );
}

