// «تأمین‌کنندگان و لابراتوارها» — who the clinic buys from and sends work to (8.5, 8.6), what it owes each
// (purchases + lab work − returns − payments), and the payments made. A payment to a supplier is also an
// expense, so the cash closing and the profit report stay right.
import { useEffect, useState } from "react";
import { Ban, BookOpen, Building2, FlaskConical, Pencil, Plus, Truck, Wallet } from "lucide-react";
import type { CalendarSystem, SupplierInfo, SupplierKind, SupplierPaymentInfo, LedgerEntry } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { amountRule, parseAfn, usePaymentMethods } from "../../lib/billing";
import { todayIso } from "../../lib/calendar";
import { formatDate } from "../../lib/dates";
import { spayQ, supplierQ } from "../../lib/inventory";
import { useLive } from "../../lib/lan";
import { tr } from "../../lib/medical";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Checkbox, Segmented, Switch } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, Select, TextInput, Textarea } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { AmountInput } from "../billing/common";
import { ReasonDialog } from "../billing/dialogs";
import { useAction } from "./common";
import type { InventoryPerms } from "./common";

type Kind = "all" | SupplierKind;

export function SuppliersTab({ perms, calendar }: { perms: InventoryPerms; calendar: CalendarSystem }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const [kind, setKind] = useState<Kind>("all");
  const [owing, setOwing] = useState(false);
  const [rows, setRows] = useState<SupplierInfo[] | null>(null);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const [editing, setEditing] = useState<SupplierInfo | "new" | null>(null);
  const [ledger, setLedger] = useState<SupplierInfo | null>(null);
  const reload = () => setTick((x) => x + 1);
  useLive(["suppliers", "purchases", "supplier_payments", "lab_cases", "stock"], reload);
  useEffect(() => {
    rpc("suppliers.list", supplierQ({ kind: kind === "all" ? null : kind, owing_only: owing, include_inactive: true }))
      .then((r) => {
        setRows(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [kind, owing, tick]);
  if (error && !rows) return <ErrorState message={error} onRetry={reload} />;
  const money = (v: number) => formatAmount(v, lang);
  return (
    <div className="stack" data-testid="suppliers-tab">
      <Card flush>
        <CardHeader
          icon={Truck}
          title={t("inventory.tab.suppliers")}
          description={t("supplier.hint")}
          actions={perms.manage ? <Button variant="primary" icon={Plus} onClick={() => setEditing("new")} data-testid="supplier-add-open">{t("supplier.new")}</Button> : undefined}
        />
        <div className="row toolbar">
          <Segmented<Kind>
            value={kind}
            onChange={setKind}
            label={t("supplier.kind")}
            options={[
              { value: "all", label: t("supplier.kind.all"), testId: "supplier-kind-all" },
              { value: "supplier", label: t("supplier.kind.supplier"), testId: "supplier-kind-supplier" },
              { value: "lab", label: t("supplier.kind.lab"), testId: "supplier-kind-lab" },
            ]}
          />
          <Checkbox checked={owing} onChange={setOwing} testId="supplier-owing">{t("supplier.owingOnly")}</Checkbox>
        </div>
        <div className="table-wrap">
          <table className="table" data-testid="suppliers-table">
            <thead>
              <tr>
                <th>{t("supplier.name")}</th>
                <th>{t("supplier.contact")}</th>
                <th className="num">{t("supplier.bought")}</th>
                <th className="num">{t("supplier.paid")}</th>
                <th className="num">{t("supplier.balance")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {!rows && <SkeletonRows cols={6} />}
              {rows?.map((s) => (
                <tr key={s.id} data-testid={`supplier-${s.id}`}>
                  <td>
                    <button type="button" className="link-btn plain" onClick={() => setLedger(s)} data-testid={`supplier-open-${s.id}`}>{s.name}</button>
                    <div className="row tight">
                      <Badge tone={s.kind === "lab" ? "info" : "neutral"} icon={s.kind === "lab" ? FlaskConical : Building2}>{t(`supplier.kind.${s.kind}`)}</Badge>
                      {!s.is_active && <Badge>{t("stock.inactive")}</Badge>}
                    </div>
                  </td>
                  <td>
                    {s.contact_person ?? "—"}
                    {s.phone && <div className="t-caption subtle"><bdi className="ltr num">{s.phone}</bdi></div>}
                  </td>
                  <td className="num">{money(s.purchases_total + s.lab_total - s.returns_total)}</td>
                  <td className="num">{money(s.paid_total)}</td>
                  <td className={`num cell-strong ${s.balance > 0 ? "money-danger" : ""}`} data-testid={`supplier-balance-${s.id}`}>{money(s.balance)}</td>
                  <td className="cell-actions">
                    <IconButton icon={BookOpen} label={t("supplier.ledger")} onClick={() => setLedger(s)} />
                    {perms.manage && <IconButton icon={Pencil} label={t("common.edit")} onClick={() => setEditing(s)} data-testid={`supplier-edit-${s.id}`} />}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {rows && !rows.length && <EmptyState icon={Truck} title={t("supplier.none")}>{t("supplier.noneHint")}</EmptyState>}
      </Card>
      {editing && <SupplierDialog supplier={editing === "new" ? undefined : editing} onClose={() => setEditing(null)} onDone={() => { setEditing(null); toast.success(t("supplier.saved")); reload(); }} />}
      {ledger && <LedgerDialog supplier={ledger} perms={perms} calendar={calendar} tick={tick} onClose={() => setLedger(null)} onChanged={reload} />}
    </div>
  );
}

function SupplierDialog({ supplier, onClose, onDone }: { supplier?: SupplierInfo; onClose: () => void; onDone: () => void }) {
  const { t } = useI18n();
  const [kind, setKind] = useState<SupplierKind>(supplier?.kind ?? "supplier");
  const [active, setActive] = useState(supplier?.is_active ?? true);
  const { busy, error, run } = useAction();
  const form = useForm(
    { name: supplier?.name ?? "", contact: supplier?.contact_person ?? "", phone: supplier?.phone ?? "", address: supplier?.address ?? "", note: supplier?.note ?? "" },
    { name: (s) => (s.trim() ? null : "rule.required") },
  );
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.validate()) return;
    void run(
      async () => {
        await rpc("suppliers.save", {
          id: supplier?.id ?? null,
          kind,
          name: form.values.name.trim(),
          contact_person: form.values.contact.trim() || null,
          phone: form.values.phone.trim() || null,
          address: form.values.address.trim() || null,
          note: form.values.note.trim() || null,
          is_active: active,
          version: supplier?.version ?? null,
        });
        onDone();
      },
      (x) => form.serverError(x, { contact_person: "contact" }),
    );
  };
  return (
    <Dialog
      title={t(supplier ? "supplier.edit" : "supplier.new")}
      onClose={onClose}
      testId="supplier-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={submit} data-testid="supplier-save">{t("common.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        {!supplier && (
          <Segmented<SupplierKind>
            value={kind}
            onChange={setKind}
            label={t("supplier.kind")}
            block
            options={[
              { value: "supplier", label: t("supplier.kind.supplier"), icon: Building2, testId: "supplier-new-supplier" },
              { value: "lab", label: t("supplier.kind.lab"), icon: FlaskConical, testId: "supplier-new-lab" },
            ]}
          />
        )}
        <Field label={t("supplier.name")} error={form.error("name") && t(form.error("name")!)}>
          <TextInput value={form.values.name} onChange={(e) => form.set("name", e.target.value)} maxLength={100} data-testid="supplier-name" />
        </Field>
        <div className="grid-2">
          <Field label={t("supplier.contact")} optional><TextInput value={form.values.contact} onChange={(e) => form.set("contact", e.target.value)} maxLength={100} data-testid="supplier-contact" /></Field>
          <Field label={t("supplier.phone")} optional><TextInput value={form.values.phone} onChange={(e) => form.set("phone", e.target.value)} maxLength={30} dir="ltr" inputMode="tel" data-testid="supplier-phone" /></Field>
        </div>
        <Field label={t("supplier.address")} optional><TextInput value={form.values.address} onChange={(e) => form.set("address", e.target.value)} maxLength={200} data-testid="supplier-address" /></Field>
        <Field label={t("stock.item.note")} optional><Textarea rows={2} value={form.values.note} onChange={(e) => form.set("note", e.target.value)} maxLength={300} data-testid="supplier-note" /></Field>
        {supplier && <Switch checked={active} onChange={setActive} label={t("supplier.active")} testId="supplier-active" />}
      </form>
    </Dialog>
  );
}

// ───────────────────────────── history, balance, payments ─────────────────────────────

function LedgerDialog({ supplier, perms, calendar, tick, onClose, onChanged }: { supplier: SupplierInfo; perms: InventoryPerms; calendar: CalendarSystem; tick: number; onClose: () => void; onChanged: () => void }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const [info, setInfo] = useState(supplier);
  const [entries, setEntries] = useState<LedgerEntry[] | null>(null);
  const [payments, setPayments] = useState<SupplierPaymentInfo[] | null>(null);
  const [error, setError] = useState("");
  const [local, setLocal] = useState(0);
  const [paying, setPaying] = useState(false);
  const [voiding, setVoiding] = useState<SupplierPaymentInfo | null>(null);
  useEffect(() => {
    Promise.all([rpc("suppliers.get", { id: supplier.id }), rpc("supplier_ledger.list", { supplier_id: supplier.id }), rpc("supplier_payments.list", spayQ({ supplier_id: supplier.id, from: "2000-01-01", to: todayIso() }))])
      .then(([s, l, p]) => {
        setInfo(s);
        setEntries(l);
        setPayments(p);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [supplier.id, tick, local]);
  const money = (v: number) => formatAmount(v, lang);
  const changed = (message: string) => {
    setPaying(false);
    setVoiding(null);
    toast.success(message);
    setLocal((x) => x + 1);
    onChanged();
  };
  return (
    <Dialog
      title={info.name}
      description={t(`supplier.kind.${info.kind}`)}
      onClose={onClose}
      wide
      testId="ledger-dialog"
      footer={perms.manage && perms.billingEdit ? <Button variant="primary" icon={Wallet} onClick={() => setPaying(true)} data-testid="supplier-pay-open">{t("supplier.pay")}</Button> : undefined}
    >
      <div className="stack">
        {error && <ErrorState message={error} onRetry={() => setLocal((x) => x + 1)} />}
        <dl className="facts">
          <div><dt>{t("supplier.bought")}</dt><dd className="num">{money(info.purchases_total + info.lab_total - info.returns_total)}</dd></div>
          <div><dt>{t("supplier.paid")}</dt><dd className="num">{money(info.paid_total)}</dd></div>
          <div><dt>{t("supplier.balance")}</dt><dd className={`num cell-strong ${info.balance > 0 ? "money-danger" : ""}`} data-testid="ledger-balance">{money(info.balance)}</dd></div>
          <div><dt>{t("supplier.phone")}</dt><dd>{info.phone ? <bdi className="ltr num">{info.phone}</bdi> : "—"}</dd></div>
        </dl>
        <h3 className="t-subtitle">{t("supplier.ledger")}</h3>
        <div className="table-wrap">
          <table className="table" data-testid="ledger-table">
            <thead>
              <tr>
                <th>{t("doc.date")}</th>
                <th>{t("stock.move.kind")}</th>
                <th>{t("supplier.ledger.what")}</th>
                <th className="num">{t("pay.amount")}</th>
                <th className="num">{t("supplier.balance")}</th>
              </tr>
            </thead>
            <tbody>
              {!entries && <SkeletonRows cols={5} rows={2} />}
              {entries?.map((e, i) => (
                <tr key={i}>
                  <td>{formatDate(e.local_date, lang, calendar)}</td>
                  <td>{t(`supplier.ledger.${e.kind}`)}</td>
                  <td>{e.number && <bdi className="ltr num">{e.number}</bdi>} {e.description}</td>
                  <td className={`num ${e.amount < 0 ? "money-success" : ""}`}>{money(Math.abs(e.amount))}</td>
                  <td className="num cell-strong">{money(e.balance)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {entries && !entries.length && <p className="muted pad">{t("supplier.ledger.none")}</p>}
        </div>
        <h3 className="t-subtitle">{t("supplier.payments")}</h3>
        <div className="table-wrap">
          <table className="table" data-testid="supplier-payments">
            <thead>
              <tr>
                <th>{t("purchase.number")}</th>
                <th>{t("doc.date")}</th>
                <th>{t("pay.method")}</th>
                <th className="num">{t("pay.amount")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {payments?.map((p) => (
                <tr key={p.id} data-testid={`spay-${p.id}`}>
                  <td><bdi className="ltr num">{p.number}</bdi>{p.void && <> <Badge icon={Ban}>{t("purchase.void")}</Badge></>}</td>
                  <td>{formatDate(p.local_date, lang, calendar)}</td>
                  <td>{p.method ? tr(p.method, lang) : "—"}</td>
                  <td className={`num cell-strong ${p.void ? "strike" : ""}`}>{money(p.amount)}</td>
                  <td className="cell-actions">{perms.billingVoid && perms.manage && !p.void && <IconButton icon={Ban} label={t("supplier.payment.void")} onClick={() => setVoiding(p)} data-testid={`spay-void-${p.id}`} />}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {payments && !payments.length && <p className="muted pad">{t("supplier.payments.none")}</p>}
        </div>
      </div>
      {paying && <PayDialog supplier={info} calendar={calendar} onClose={() => setPaying(false)} onDone={() => changed(t("supplier.payment.saved"))} />}
      {voiding && (
        <ReasonDialog
          title={t("supplier.payment.void")}
          description={`${voiding.number} · ${money(voiding.amount)}`}
          confirm={t("supplier.payment.void")}
          danger
          onClose={() => setVoiding(null)}
          onConfirm={async (reason) => {
            await rpc("supplier_payments.void", { id: voiding.id, version: voiding.version, reason });
            changed(t("supplier.payment.voided"));
          }}
        />
      )}
    </Dialog>
  );
}

function PayDialog({ supplier, calendar, onClose, onDone }: { supplier: SupplierInfo; calendar: CalendarSystem; onClose: () => void; onDone: () => void }) {
  const { t, lang } = useI18n();
  const methods = usePaymentMethods();
  const [date, setDate] = useState(todayIso());
  const { busy, error, run } = useAction();
  const form = useForm({ amount: supplier.balance > 0 ? String(supplier.balance / 100) : "", method: "", note: "" }, { amount: (s) => amountRule(s), method: (s) => (s ? null : "rule.required") });
  useEffect(() => {
    if (!form.values.method && methods?.length) form.set("method", (methods.find((m) => m.is_active && m.is_cash) ?? methods.find((m) => m.is_active))?.id ?? "");
  }, [methods]);
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.validate()) return;
    void run(
      async () => {
        await rpc("supplier_payments.add", { supplier_id: supplier.id, amount: parseAfn(form.values.amount) ?? 0, method_id: form.values.method, date, note: form.values.note.trim() || null });
        onDone();
      },
      (x) => form.serverError(x, { method_id: "method" }),
    );
  };
  return (
    <Dialog
      title={t("supplier.pay")}
      description={`${supplier.name} · ${t("supplier.balance")}: ${formatAmount(supplier.balance, lang)}`}
      onClose={onClose}
      testId="supplier-pay-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Wallet} loading={busy} onClick={submit} data-testid="supplier-pay-save">{t("supplier.pay.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Notice tone="info">{t("supplier.pay.expenseHint")}</Notice>
        <Field label={t("pay.amount")} error={form.error("amount") && t(form.error("amount")!)}>
          <AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="supplier-pay-amount" autoFocus />
        </Field>
        <Field label={t("pay.method")} error={form.error("method") && t(form.error("method")!)}>
          <Select value={form.values.method} onChange={(e) => form.set("method", e.target.value)} data-testid="supplier-pay-method">
            {methods?.filter((m) => m.is_active).map((m) => <option key={m.id} value={m.id}>{tr(m.name, lang)}</option>)}
          </Select>
        </Field>
        <Field label={t("doc.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="supplier-pay-date" yearsBefore={1} yearsAfter={0} /></Field>
        <Field label={t("purchase.note")} optional><TextInput value={form.values.note} onChange={(e) => form.set("note", e.target.value)} maxLength={300} data-testid="supplier-pay-note" /></Field>
      </form>
    </Dialog>
  );
}
