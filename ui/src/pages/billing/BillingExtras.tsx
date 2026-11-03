// Expenses and profit (6.6), doctor commission (6.5) and the billing settings (payment methods, expense
// categories, commission rules, the receipt's footer and this computer's receipt printer, 6.12).
import { useEffect, useState } from "react";
import { Banknote, Percent, Plus, Printer, Trash2, Wallet } from "lucide-react";
import type { BillingSettings, CalendarSystem, CommissionBasis, CommissionReport, CommissionRuleInfo, ExpenseCategoryInfo, ExpenseInfo, FinanceSummary, PaymentMethodInfo, Translations } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { amountRule, invalidatePaymentMethods, parseAfn, usePaymentMethods } from "../../lib/billing";
import { monthRange, todayIso } from "../../lib/calendar";
import { useCatalog } from "../../lib/clinical";
import { serviceName } from "../../lib/clinical";
import { digits, formatDate } from "../../lib/dates";
import { tr } from "../../lib/medical";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader, Stat } from "../../ui/Card";
import { Checkbox, Segmented, Switch } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, Select, TextInput } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { TranslationsField } from "../../ui/TranslationsField";
import { useToast } from "../../ui/Toast";
import { DocumentPreview } from "../../print/DocumentPreview";
import { CommissionSheet } from "../../print/finance";
import { useLetterhead } from "../../print/letterhead";
import { ReceiptSettings } from "../../print/receipt";
import { useScheduling } from "../reception/useScheduling";
import { AmountInput, invalidateBillingSettings, minorOf } from "./common";
import type { BillingPerms } from "./common";
import { ReasonDialog } from "./dialogs";

const hasName = (n: Translations) => !!(n.fa.trim() || n.ps.trim() || n.en.trim());

// ───────────────────────────── expenses ─────────────────────────────

export function ExpensesTab({ perms, calendar }: { perms: BillingPerms; calendar: CalendarSystem }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const [from, setFrom] = useState(monthRange(todayIso(), calendar).from);
  const [to, setTo] = useState(todayIso());
  const [expenses, setExpenses] = useState<ExpenseInfo[] | null>(null);
  const [summary, setSummary] = useState<FinanceSummary | null>(null);
  const [error, setError] = useState("");
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<ExpenseInfo | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (from > to) return;
    Promise.all([rpc("expenses.list", { from, to }), rpc("billing.summary", { from, to })])
      .then(([e, s]) => {
        setExpenses(e);
        setSummary(s);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [from, to, tick]);
  if (error && !expenses) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const money = (v: number) => formatAmount(v, lang);
  return (
    <div className="stack" data-testid="billing-expenses">
      <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
        <Field label={t("period.from")}><DateField value={from} onChange={setFrom} calendar={calendar} label={t("period.from")} testId="expenses-from" yearsBefore={3} yearsAfter={0} /></Field>
        <Field label={t("period.to")}><DateField value={to} onChange={setTo} calendar={calendar} label={t("period.to")} testId="expenses-to" yearsBefore={3} yearsAfter={0} /></Field>
        {from > to && <Notice tone="warning">{t("rule.period_range")}</Notice>}
      </div>
      <div className="stat-grid">
        <Stat icon={Banknote} label={t("profit.received")} value={summary ? money(summary.received - summary.refunded) : "…"} tone="success" testId="profit-received" />
        <Stat icon={Wallet} label={t("profit.expenses")} value={summary ? money(summary.expenses) : "…"} tone="warning" testId="profit-expenses" />
        <Stat icon={Percent} label={t("profit.profit")} value={summary ? money(summary.profit) : "…"} tone={summary && summary.profit < 0 ? "danger" : "accent"} sub={summary ? `${t("profit.invoiced")}: ${money(summary.invoiced)}` : undefined} testId="profit-profit" />
      </div>
      <Card flush>
        <CardHeader icon={Wallet} title={t("billing.tab.expenses")} description={t("expenses.hint")} actions={perms.edit ? <Button variant="primary" icon={Plus} onClick={() => setAdding(true)} data-testid="expense-add-open">{t("expenses.add")}</Button> : undefined} />
        <div className="table-wrap">
          <table className="table" data-testid="expenses-table">
            <thead><tr><th>{t("doc.date")}</th><th>{t("expenses.category")}</th><th>{t("expenses.description")}</th><th>{t("expenses.payee")}</th><th className="num">{t("pay.amount")}</th><th /></tr></thead>
            <tbody>
              {!expenses && <SkeletonRows cols={6} />}
              {expenses?.map((e) => (
                <tr key={e.id} data-testid={`expense-${e.id}`}>
                  <td>{formatDate(e.local_date, lang, calendar)}</td>
                  <td><Badge>{tr(e.category, lang)}</Badge></td>
                  <td>{e.description}{!e.from_cash && <span className="t-caption subtle"> · {t("expenses.notFromCash")}</span>}</td>
                  <td>{e.payee ?? "—"}</td>
                  <td className="num cell-strong">{money(e.amount)}</td>
                  <td className="cell-actions">{perms.void && <IconButton icon={Trash2} label={t("common.delete")} onClick={() => setRemoving(e)} data-testid={`expense-delete-${e.id}`} />}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {expenses && !expenses.length && <EmptyState icon={Wallet} title={t("expenses.none")}>{t("expenses.noneHint")}</EmptyState>}
      </Card>
      {adding && <ExpenseDialog calendar={calendar} onClose={() => setAdding(false)} onDone={() => { setAdding(false); toast.success(t("expenses.saved")); setTick((x) => x + 1); }} />}
      {removing && (
        <ReasonDialog
          title={t("expenses.delete")}
          description={`${removing.description} · ${money(removing.amount)}`}
          confirm={t("common.delete")}
          danger
          onClose={() => setRemoving(null)}
          onConfirm={async (reason) => {
            await rpc("expenses.delete", { id: removing.id, version: removing.version, reason });
            setRemoving(null);
            toast.success(t("expenses.deleted"));
            setTick((x) => x + 1);
          }}
        />
      )}
    </div>
  );
}

function ExpenseDialog({ calendar, onClose, onDone }: { calendar: CalendarSystem; onClose: () => void; onDone: () => void }) {
  const { t, lang, err } = useI18n();
  const [categories, setCategories] = useState<ExpenseCategoryInfo[]>([]);
  const [date, setDate] = useState(todayIso());
  const [fromCash, setFromCash] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm({ category: "", amount: "", description: "", payee: "" }, { category: (s) => (s ? null : "rule.required"), amount: (s) => amountRule(s), description: (s) => (s.trim() ? null : "rule.required") });
  useEffect(() => {
    rpc("expense_categories.list", { include_inactive: false }).then((c) => {
      setCategories(c);
      if (c.length) form.set("category", c[0].id);
    }).catch((e) => setError(err(e)));
  }, []);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await rpc("expenses.add", { category_id: form.values.category, amount: parseAfn(form.values.amount) ?? 0, date, description: form.values.description.trim(), payee: form.values.payee.trim() || null, from_cash: fromCash });
      onDone();
    } catch (x) {
      if (!form.serverError(x, { category_id: "category" })) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={t("expenses.add")} onClose={onClose} testId="expense-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Wallet} loading={busy} onClick={submit} data-testid="expense-save">{t("common.save")}</Button></>}>
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("expenses.category")} error={form.error("category") && t(form.error("category")!)}>
          <Select value={form.values.category} onChange={(e) => form.set("category", e.target.value)} data-testid="expense-category">{categories.map((c) => <option key={c.id} value={c.id}>{tr(c.name, lang)}</option>)}</Select>
        </Field>
        <Field label={t("pay.amount")} error={form.error("amount") && t(form.error("amount")!)}><AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="expense-amount" /></Field>
        <Field label={t("doc.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="expense-date" yearsBefore={1} yearsAfter={0} /></Field>
        <Field label={t("expenses.description")} error={form.error("description") && t(form.error("description")!)}><TextInput value={form.values.description} onChange={(e) => form.set("description", e.target.value)} maxLength={200} data-testid="expense-description" /></Field>
        <Field label={t("expenses.payee")} optional><TextInput value={form.values.payee} onChange={(e) => form.set("payee", e.target.value)} maxLength={100} data-testid="expense-payee" /></Field>
        <Checkbox checked={fromCash} onChange={setFromCash} testId="expense-from-cash">{t("expenses.fromCash")}</Checkbox>
      </form>
    </Dialog>
  );
}

// ───────────────────────────── commission ─────────────────────────────

export function CommissionTab({ calendar }: { calendar: CalendarSystem }) {
  const { t, lang, err } = useI18n();
  const letterhead = useLetterhead();
  const [from, setFrom] = useState(monthRange(todayIso(), calendar).from);
  const [to, setTo] = useState(todayIso());
  const [report, setReport] = useState<CommissionReport | null>(null);
  const [error, setError] = useState("");
  const [printing, setPrinting] = useState(false);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (from > to) return;
    rpc("commission.report", { doctor_id: null, from, to })
      .then((r) => {
        setReport(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [from, to, tick]);
  const settings = useBillingSettingsLocal();
  if (error && !report) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const money = (v: number) => formatAmount(v, lang);
  return (
    <div className="stack" data-testid="billing-commission">
      <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
        <Field label={t("period.from")}><DateField value={from} onChange={setFrom} calendar={calendar} label={t("period.from")} testId="commission-from" yearsBefore={3} yearsAfter={0} /></Field>
        <Field label={t("period.to")}><DateField value={to} onChange={setTo} calendar={calendar} label={t("period.to")} testId="commission-to" yearsBefore={3} yearsAfter={0} /></Field>
        <Button icon={Printer} disabled={!report || !letterhead || !report.doctors.length} onClick={() => setPrinting(true)} data-testid="commission-print">{t("comm.print")}</Button>
      </div>
      {settings && !settings.commission_enabled && <Notice tone="info" testId="commission-off">{t("comm.off")}</Notice>}
      {report && settings?.commission_enabled && <Notice tone="info">{t(`comm.basis.${report.basis}.hint`)}</Notice>}
      {report?.doctors.map((d) => (
        <Card flush key={d.doctor_id} data-testid={`commission-doctor-${d.doctor_id}`}>
          <CardHeader icon={Percent} title={d.doctor_name} description={`${t("comm.base")}: ${money(d.base)}`} actions={<Badge tone="accent">{t("comm.owed")}: {money(d.commission)}</Badge>} />
          <div className="table-wrap">
            <table className="table">
              <thead><tr><th>{t("doc.date")}</th><th>{t("rcpt.invoice")}</th><th>{t("doc.patient")}</th><th>{t("inv.col.service")}</th><th className="num">{t("comm.base")}</th><th className="num">{t("comm.amount")}</th></tr></thead>
              <tbody>
                {d.lines.map((l, i) => (
                  <tr key={i}><td>{formatDate(l.date, lang, calendar)}</td><td><bdi className="ltr num">{l.invoice_number}</bdi></td><td>{l.patient_name}</td><td>{tr(l.names, lang)}</td><td className="num">{money(l.base)}</td><td className="num cell-strong">{money(l.commission)}</td></tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      ))}
      {report && settings?.commission_enabled && !report.doctors.length && <EmptyState icon={Percent} title={t("comm.none")}>{t("comm.noneHint")}</EmptyState>}
      {printing && report && letterhead && <DocumentPreview title={t("comm.title")} paper="a4" fileStem={`COMM-${from}-${to}`} testId="commission-preview" onClose={() => setPrinting(false)} sheet={<CommissionSheet report={report} letterhead={letterhead} lang={lang} calendar={calendar} />} />}
    </div>
  );
}

function useBillingSettingsLocal() {
  const [s, setS] = useState<BillingSettings | null>(null);
  useEffect(() => {
    rpc("billing.settings_get", {}).then(setS).catch(() => {});
  }, []);
  return s;
}

// ───────────────────────────── settings ─────────────────────────────

export function BillingSettingsTab() {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const [settings, setSettings] = useState<BillingSettings | null>(null);
  const [footer, setFooter] = useState("");
  const [rules, setRules] = useState<CommissionRuleInfo[] | null>(null);
  const [categories, setCategories] = useState<ExpenseCategoryInfo[] | null>(null);
  const methods = usePaymentMethods();
  const { doctors } = useScheduling();
  const catalog = useCatalog();
  const [editMethod, setEditMethod] = useState<PaymentMethodInfo | "new" | null>(null);
  const [editCategory, setEditCategory] = useState<ExpenseCategoryInfo | "new" | null>(null);
  const [editRule, setEditRule] = useState<CommissionRuleInfo | "new" | null>(null);
  const [error, setError] = useState("");
  const load = () =>
    Promise.all([rpc("billing.settings_get", {}), rpc("commission.rules_list", {}), rpc("expense_categories.list", { include_inactive: true })])
      .then(([s, r, c]) => {
        setSettings(s);
        setFooter(s.receipt_footer);
        setRules(r);
        setCategories(c);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, []);
  if (error && !settings) return <ErrorState message={error} onRetry={load} />;
  const save = async (next: BillingSettings) => {
    try {
      const s = await rpc("billing.settings_save", next);
      invalidateBillingSettings();
      setSettings(s);
      toast.success(t("common.saved"));
    } catch (x) {
      toast.error(err(x));
    }
  };
  const doctorName = (id: string | null) => (id ? doctors?.find((d) => d.id === id)?.full_name ?? "—" : t("comm.rule.everyDoctor"));
  const serviceLabel = (id: string | null) => {
    const s = id ? catalog?.services.find((x) => x.id === id) : null;
    return s ? serviceName(s, catalog, lang) : t("comm.rule.everyService");
  };
  const ruleValue = (r: CommissionRuleInfo) => (r.kind === "percent" ? `${digits(r.value / 100, lang)}٪` : formatAmount(r.value, lang));
  return (
    <div className="stack" data-testid="billing-settings">
      <Card>
        <CardHeader icon={Printer} title={t("billing.settings.receipt")} description={t("billing.settings.receipt.hint")} />
        <div className="stack">
          <Field label={t("billing.settings.footer")} hint={t("billing.settings.footer.hint")}>
            <TextInput value={footer} onChange={(e) => setFooter(e.target.value)} maxLength={200} data-testid="settings-footer" />
          </Field>
          <div className="row">
            <Button onClick={() => settings && save({ ...settings, receipt_footer: footer })} disabled={!settings || footer === settings.receipt_footer} data-testid="settings-footer-save">{t("common.save")}</Button>
          </div>
          <ReceiptSettings />
        </div>
      </Card>

      <Card>
        <CardHeader icon={Wallet} title={t("billing.settings.methods")} description={t("billing.settings.methods.hint")} actions={<Button icon={Plus} onClick={() => setEditMethod("new")} data-testid="method-add">{t("common.add")}</Button>} />
        <ul className="plain-list set-list" data-testid="methods-list">
          {(methods ?? []).map((m) => (
            <li key={m.id} className="set-row">
              <span className="cell-strong">{tr(m.name, lang)}</span>
              {m.is_cash && <Badge tone="success">{t("method.cash")}</Badge>}
              {!m.is_active && <Badge>{t("common.inactive")}</Badge>}
              <Button size="sm" variant="link" onClick={() => setEditMethod(m)} data-testid={`method-edit-${m.code}`}>{t("common.edit")}</Button>
            </li>
          ))}
        </ul>
      </Card>

      <Card>
        <CardHeader icon={Wallet} title={t("billing.settings.categories")} description={t("billing.settings.categories.hint")} actions={<Button icon={Plus} onClick={() => setEditCategory("new")} data-testid="category-add">{t("common.add")}</Button>} />
        <ul className="plain-list set-list" data-testid="categories-list">
          {(categories ?? []).map((c) => (
            <li key={c.id} className="set-row">
              <span className="cell-strong">{tr(c.name, lang)}</span>
              {!c.is_active && <Badge>{t("common.inactive")}</Badge>}
              <Button size="sm" variant="link" onClick={() => setEditCategory(c)}>{t("common.edit")}</Button>
            </li>
          ))}
        </ul>
      </Card>

      <Card>
        <CardHeader icon={Percent} title={t("billing.settings.commission")} description={t("billing.settings.commission.hint")} />
        <div className="stack">
          <Switch checked={!!settings?.commission_enabled} onChange={(on) => settings && save({ ...settings, commission_enabled: on })} disabled={!settings} label={t("billing.settings.commission.enable")} testId="commission-enabled" />
          {settings?.commission_enabled && (
            <>
              <div className="field">
                <span className="field-label">{t("billing.settings.commission.basis")}</span>
                <Segmented<CommissionBasis> value={settings.commission_basis} onChange={(b) => save({ ...settings, commission_basis: b })} label={t("billing.settings.commission.basis")} options={[{ value: "payment", label: t("comm.basis.payment"), testId: "commission-basis-payment" }, { value: "treatment", label: t("comm.basis.treatment"), testId: "commission-basis-treatment" }]} />
                <span className="field-hint">{t(`comm.basis.${settings.commission_basis}.hint`)}</span>
              </div>
              <div className="table-wrap">
                <table className="table" data-testid="commission-rules">
                  <thead><tr><th>{t("appt.doctor")}</th><th>{t("inv.col.service")}</th><th className="num">{t("comm.rule.value")}</th><th /></tr></thead>
                  <tbody>
                    {(rules ?? []).map((r) => (
                      <tr key={r.id}>
                        <td>{doctorName(r.doctor_id)}</td>
                        <td>{serviceLabel(r.service_id)}</td>
                        <td className="num cell-strong">{ruleValue(r)} <span className="subtle t-caption">{r.kind === "fixed" ? t("comm.rule.perUnit") : ""}</span></td>
                        <td className="cell-actions"><span className="row"><Button size="sm" variant="link" onClick={() => setEditRule(r)}>{t("common.edit")}</Button><IconButton icon={Trash2} label={t("common.delete")} size="sm" onClick={async () => { await rpc("commission.rules_delete", { id: r.id, version: r.version }); load(); }} /></span></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              {rules && !rules.length && <Notice tone="warning">{t("comm.rule.none")}</Notice>}
              <div className="row"><Button icon={Plus} onClick={() => setEditRule("new")} data-testid="commission-rule-add">{t("comm.rule.add")}</Button></div>
            </>
          )}
        </div>
      </Card>

      {editMethod && <MethodDialog method={editMethod === "new" ? null : editMethod} onClose={() => setEditMethod(null)} onDone={() => { setEditMethod(null); invalidatePaymentMethods(); toast.success(t("common.saved")); }} />}
      {editCategory && <CategoryDialog category={editCategory === "new" ? null : editCategory} onClose={() => setEditCategory(null)} onDone={() => { setEditCategory(null); toast.success(t("common.saved")); load(); }} />}
      {editRule && <RuleDialog rule={editRule === "new" ? null : editRule} onClose={() => setEditRule(null)} onDone={() => { setEditRule(null); toast.success(t("common.saved")); load(); }} />}
    </div>
  );
}

function MethodDialog({ method, onClose, onDone }: { method: PaymentMethodInfo | null; onClose: () => void; onDone: () => void }) {
  const { t, err } = useI18n();
  const [name, setName] = useState<Translations>(method?.name ?? { fa: "", ps: "", en: "" });
  const [isCash, setCash] = useState(method?.is_cash ?? false);
  const [active, setActive] = useState(method?.is_active ?? true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const save = async () => {
    setSubmitted(true);
    if (!hasName(name)) return;
    setBusy(true);
    try {
      await rpc("payment_methods.save", { id: method?.id ?? null, name, is_cash: isCash, is_active: active, version: method?.version ?? null });
      onDone();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={method ? t("billing.settings.method.edit") : t("billing.settings.method.new")} onClose={onClose} testId="method-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="method-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("catalog.name")} value={name} onChange={setName} error={submitted && !hasName(name) ? t("rule.required") : null} testId="method-name" />
        <Checkbox checked={isCash} onChange={setCash} disabled={!!method?.is_system} testId="method-is-cash">{t("method.isCash")}</Checkbox>
        <Checkbox checked={active} onChange={setActive} disabled={method?.code === "cash"} testId="method-active">{t("common.active")}</Checkbox>
      </div>
    </Dialog>
  );
}

function CategoryDialog({ category, onClose, onDone }: { category: ExpenseCategoryInfo | null; onClose: () => void; onDone: () => void }) {
  const { t, err } = useI18n();
  const [name, setName] = useState<Translations>(category?.name ?? { fa: "", ps: "", en: "" });
  const [active, setActive] = useState(category?.is_active ?? true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const save = async () => {
    setSubmitted(true);
    if (!hasName(name)) return;
    setBusy(true);
    try {
      await rpc("expense_categories.save", { id: category?.id ?? null, name, is_active: active, version: category?.version ?? null });
      onDone();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={category ? t("billing.settings.category.edit") : t("billing.settings.category.new")} onClose={onClose} testId="category-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="category-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("catalog.name")} value={name} onChange={setName} error={submitted && !hasName(name) ? t("rule.required") : null} testId="category-name" />
        <Checkbox checked={active} onChange={setActive} testId="category-active">{t("common.active")}</Checkbox>
      </div>
    </Dialog>
  );
}

function RuleDialog({ rule, onClose, onDone }: { rule: CommissionRuleInfo | null; onClose: () => void; onDone: () => void }) {
  const { t, lang, err } = useI18n();
  const { doctors } = useScheduling();
  const catalog = useCatalog();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [kind, setKind] = useState<"percent" | "fixed">(rule?.kind ?? "percent");
  const form = useForm(
    { doctor: rule?.doctor_id ?? "", service: rule?.service_id ?? "", value: rule ? String(rule.kind === "percent" ? rule.value / 100 : rule.value / 100) : "" },
    { value: (s) => { const r = amountRule(s, true); if (r) return r; return kind === "percent" && minorOf(s) > 10_000 ? "rule.commission_value" : null; } },
  );
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      const minor = parseAfn(form.values.value) ?? 0;
      await rpc("commission.rules_save", { id: rule?.id ?? null, doctor_id: form.values.doctor || null, service_id: form.values.service || null, kind, value: minor, version: rule?.version ?? null });
      onDone();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  const parents = new Set((catalog?.services ?? []).filter((s) => s.parent_id).map((s) => s.parent_id));
  return (
    <Dialog title={t("comm.rule.add")} description={t("comm.rule.hint")} onClose={onClose} testId="rule-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={submit} data-testid="rule-save">{t("common.save")}</Button></>}>
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("appt.doctor")}>
          <Select value={form.values.doctor} onChange={(e) => form.set("doctor", e.target.value)} data-testid="rule-doctor">
            <option value="">{t("comm.rule.everyDoctor")}</option>
            {(doctors ?? []).map((d) => <option key={d.id} value={d.id}>{d.full_name}</option>)}
          </Select>
        </Field>
        <Field label={t("inv.col.service")}>
          <Select value={form.values.service} onChange={(e) => form.set("service", e.target.value)} data-testid="rule-service">
            <option value="">{t("comm.rule.everyService")}</option>
            {(catalog?.services ?? []).filter((s) => !parents.has(s.id)).map((s) => <option key={s.id} value={s.id}>{serviceName(s, catalog, lang)}</option>)}
          </Select>
        </Field>
        <Segmented<"percent" | "fixed"> value={kind} onChange={setKind} label={t("comm.rule.kind")} block options={[{ value: "percent", label: t("comm.rule.percent"), testId: "rule-kind-percent" }, { value: "fixed", label: t("comm.rule.fixed"), testId: "rule-kind-fixed" }]} />
        <Field label={kind === "percent" ? t("comm.rule.percentValue") : t("comm.rule.fixedValue")} hint={kind === "percent" ? t("comm.rule.percentHint") : t("comm.rule.fixedHint")} error={form.error("value") && t(form.error("value")!)}>
          <TextInput inputMode="decimal" dir="ltr" value={form.values.value} onChange={(e) => form.set("value", e.target.value)} suffix={kind === "percent" ? "%" : lang === "en" ? "AFN" : "افغانی"} data-testid="rule-value" />
        </Field>
      </form>
    </Dialog>
  );
}
