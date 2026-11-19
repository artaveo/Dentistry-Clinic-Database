// Doctor shares (the preview of what each doctor earned) and the finance settings: the clinic's legal profile
// for official reports, the chart of accounts the clinic extends (expense and money accounts), the receipt, and
// the doctor share rules.
import { useEffect, useState } from "react";
import { Building2, Landmark, Percent, Plus, Printer, Trash2, Wallet } from "lucide-react";
import type { AccountInfo, CalendarSystem, CommissionBasis, CommissionReport, CommissionRuleInfo, FinanceSettings, LegalForm, LegalProfile, MoneyKind, TaxRegime } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { accountName, amountRule, invalidateAccounts, parseAfn, useAccounts } from "../../lib/billing";
import { monthRange, todayIso } from "../../lib/calendar";
import { serviceName, useCatalog } from "../../lib/clinical";
import { digits, formatDate } from "../../lib/dates";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { Checkbox, Segmented, Switch } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice } from "../../ui/Feedback";
import { Field, Select, TextInput } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { DocumentPreview } from "../../print/DocumentPreview";
import { CommissionSheet } from "../../print/finance";
import { useLetterhead } from "../../print/letterhead";
import { ReceiptSettings } from "../../print/receipt";
import { useScheduling } from "../reception/useScheduling";
import { invalidateBillingSettings, minorOf, useBillingSettings } from "./common";

// ───────────────────────────── doctor shares ─────────────────────────────

export function CommissionTab({ calendar }: { calendar: CalendarSystem }) {
  const { t, lang, err } = useI18n();
  const letterhead = useLetterhead();
  const settings = useBillingSettings();
  const [from, setFrom] = useState(monthRange(todayIso(), calendar).from);
  const [to, setTo] = useState(todayIso());
  const [report, setReport] = useState<CommissionReport | null>(null);
  const [error, setError] = useState("");
  const [printing, setPrinting] = useState(false);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (from > to) return;
    rpc("fin.shares.preview", { doctor_id: null, from, to })
      .then((r) => {
        setReport(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [from, to, tick]);
  if (error && !report) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const money = (v: number) => formatAmount(v, lang);
  return (
    <div className="stack" data-testid="billing-commission">
      <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
        <Field label={t("period.from")}><DateField value={from} onChange={setFrom} calendar={calendar} label={t("period.from")} testId="commission-from" yearsBefore={3} yearsAfter={0} /></Field>
        <Field label={t("period.to")}><DateField value={to} onChange={setTo} calendar={calendar} label={t("period.to")} testId="commission-to" yearsBefore={3} yearsAfter={0} /></Field>
        <Button icon={Printer} disabled={!report || !letterhead || !report.doctors.length} onClick={() => setPrinting(true)} data-testid="commission-print">{t("comm.print")}</Button>
      </div>
      {settings && !settings.share_enabled && <Notice tone="info" testId="commission-off">{t("comm.off")}</Notice>}
      {report && settings?.share_enabled && <Notice tone="info">{t(`comm.basis.${report.basis}.hint`)} {t("comm.previewNote")}</Notice>}
      {report?.doctors.map((d) => (
        <Card flush key={d.doctor_id} data-testid={`commission-doctor-${d.doctor_id}`}>
          <CardHeader icon={Percent} title={d.doctor_name} description={`${t("comm.base")}: ${money(d.base)}`} actions={<Badge tone="accent">{t("comm.owed")}: {money(d.commission)}</Badge>} />
          <div className="table-wrap">
            <table className="table">
              <thead><tr><th>{t("doc.date")}</th><th>{t("rcpt.invoice")}</th><th>{t("doc.patient")}</th><th>{t("inv.col.service")}</th><th className="num">{t("comm.base")}</th><th className="num">{t("comm.amount")}</th></tr></thead>
              <tbody>
                {d.lines.map((l, i) => (
                  <tr key={i}><td>{formatDate(l.date, lang, calendar)}</td><td><bdi className="ltr num">{l.invoice_number}</bdi></td><td>{l.patient_name}</td><td>{l.name}</td><td className="num">{money(l.base)}</td><td className="num cell-strong">{money(l.commission)}</td></tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      ))}
      {report && settings?.share_enabled && !report.doctors.length && <EmptyState icon={Percent} title={t("comm.none")}>{t("comm.noneHint")}</EmptyState>}
      {printing && report && letterhead && <DocumentPreview title={t("comm.title")} paper="a4" fileStem={`COMM-${from}-${to}`} testId="commission-preview" onClose={() => setPrinting(false)} sheet={<CommissionSheet report={report} letterhead={letterhead} lang={lang} calendar={calendar} />} />}
    </div>
  );
}

// ───────────────────────────── settings ─────────────────────────────

export function BillingSettingsTab() {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const accounts = useAccounts();
  const [settings, setSettings] = useState<FinanceSettings | null>(null);
  const [footer, setFooter] = useState("");
  const [rules, setRules] = useState<CommissionRuleInfo[] | null>(null);
  const { doctors } = useScheduling();
  const catalog = useCatalog();
  const [editAccount, setEditAccount] = useState<AccountInfo | "expense" | "money" | null>(null);
  const [editRule, setEditRule] = useState<CommissionRuleInfo | "new" | null>(null);
  const [error, setError] = useState("");
  const load = () =>
    Promise.all([rpc("fin.settings.get", {}), rpc("fin.shares.rules_list", {})])
      .then(([s, r]) => {
        setSettings(s);
        setFooter(s.receipt_footer);
        setRules(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, []);
  if (error && !settings) return <ErrorState message={error} onRetry={load} />;
  const save = async (next: FinanceSettings) => {
    try {
      const s = await rpc("fin.settings.save", next);
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
  const money = (accounts ?? []).filter((a) => a.money);
  const expenses = (accounts ?? []).filter((a) => a.kind === "expense" && ![5000, 5810, 5910].includes(a.id));
  const accountRow = (a: AccountInfo) => (
    <li key={a.id} className="set-row" data-testid={`account-${a.id}`}>
      <bdi className="ltr num subtle">{a.id}</bdi>
      <span className="cell-strong">{accountName(a, t)}</span>
      {a.bank_name && <span className="subtle">{a.bank_name}{a.bank_no ? ` · ${a.bank_no}` : ""}</span>}
      {!a.is_active && <Badge>{t("common.inactive")}</Badge>}
      {a.money && <span className="num subtle">{formatAmount(a.balance, lang)}</span>}
      <Button size="sm" variant="link" onClick={() => setEditAccount(a)} data-testid={`account-edit-${a.id}`}>{t("common.edit")}</Button>
    </li>
  );
  return (
    <div className="stack" data-testid="billing-settings">

      <Card>
        <CardHeader icon={Landmark} title={t("fin.settings.money")} description={t("fin.settings.money.hint")} actions={<Button icon={Plus} onClick={() => setEditAccount("money")} data-testid="money-account-add">{t("common.add")}</Button>} />
        <ul className="plain-list set-list" data-testid="money-accounts">{money.map(accountRow)}</ul>
      </Card>

      <Card>
        <CardHeader icon={Wallet} title={t("billing.settings.categories")} description={t("fin.settings.expenses.hint")} actions={<Button icon={Plus} onClick={() => setEditAccount("expense")} data-testid="category-add">{t("common.add")}</Button>} />
        <ul className="plain-list set-list" data-testid="categories-list">{expenses.map(accountRow)}</ul>
      </Card>

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
        <CardHeader icon={Percent} title={t("billing.settings.commission")} description={t("billing.settings.commission.hint")} />
        <div className="stack">
          <Switch checked={!!settings?.share_enabled} onChange={(on) => settings && save({ ...settings, share_enabled: on })} disabled={!settings} label={t("billing.settings.commission.enable")} testId="commission-enabled" />
          {settings?.share_enabled && (
            <>
              <div className="field">
                <span className="field-label">{t("billing.settings.commission.basis")}</span>
                <Segmented<CommissionBasis> value={settings.share_basis} onChange={(b) => save({ ...settings, share_basis: b })} label={t("billing.settings.commission.basis")} options={[{ value: "payment", label: t("comm.basis.payment"), testId: "commission-basis-payment" }, { value: "treatment", label: t("comm.basis.treatment"), testId: "commission-basis-treatment" }]} />
                <span className="field-hint">{t(`comm.basis.${settings.share_basis}.hint`)}</span>
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
                        <td className="cell-actions"><span className="row"><Button size="sm" variant="link" onClick={() => setEditRule(r)}>{t("common.edit")}</Button><IconButton icon={Trash2} label={t("common.delete")} size="sm" onClick={async () => { await rpc("fin.shares.rules_delete", { id: r.id, version: r.version }); load(); }} /></span></td>
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

      {editAccount && <AccountDialog account={typeof editAccount === "string" ? null : editAccount} parent={editAccount === "money" ? 1100 : 5000} onClose={() => setEditAccount(null)} onDone={() => { setEditAccount(null); invalidateAccounts(); toast.success(t("common.saved")); }} />}
      {editRule && <RuleDialog rule={editRule === "new" ? null : editRule} onClose={() => setEditRule(null)} onDone={() => { setEditRule(null); toast.success(t("common.saved")); load(); }} />}
      {settings && <LegalCard settings={settings} onSave={save} />}
    </div>
  );
}

/** The clinic as the tax office knows it: printed on every official report (never used to compute tax). */
function LegalCard({ settings, onSave }: { settings: FinanceSettings; onSave: (s: FinanceSettings) => Promise<void> }) {
  const { t } = useI18n();
  const [legal, setLegal] = useState<LegalProfile>(settings.legal);
  const [busy, setBusy] = useState(false);
  const set = (patch: Partial<LegalProfile>) => setLegal({ ...legal, ...patch });
  const text = (k: keyof LegalProfile, max: number, ltr = false) => (
    <Field label={t(`fin.legal.${k}`)} optional>
      <TextInput value={(legal[k] as string | null) ?? ""} onChange={(e) => set({ [k]: e.target.value || null })} maxLength={max} dir={ltr ? "ltr" : undefined} data-testid={`legal-${k}`} />
    </Field>
  );
  return (
    <Card>
      <CardHeader icon={Building2} title={t("fin.legal.title")} description={t("fin.legal.hint")} />
      <div className="stack">
        <div className="grid-2">
          {text("legal_name", 120)}
          {text("tin", 20, true)}
        </div>
        <div className="grid-2">
          <Field label={t("fin.legal.legal_form")} optional>
            <Select value={legal.legal_form ?? ""} onChange={(e) => set({ legal_form: (e.target.value || null) as LegalForm | null })} data-testid="legal-form">
              <option value="">—</option>
              {(["natural_person", "partnership", "llc", "corporation"] as const).map((f) => <option key={f} value={f}>{t(`fin.legal.form.${f}`)}</option>)}
            </Select>
          </Field>
          <Field label={t("fin.legal.tax_regime")} optional hint={t("fin.legal.tax_regime.hint")}>
            <Select value={legal.tax_regime ?? ""} onChange={(e) => set({ tax_regime: (e.target.value || null) as TaxRegime | null })} data-testid="legal-regime">
              <option value="">—</option>
              {(["unknown", "fixed", "income_brt"] as const).map((f) => <option key={f} value={f}>{t(`fin.legal.regime.${f}`)}</option>)}
            </Select>
          </Field>
        </div>
        <div className="grid-2">
          {text("business_license_no", 40, true)}
          {text("health_license_no", 40, true)}
        </div>
        <div className="grid-2">
          {text("owner_name", 100)}
          {text("owner_father_name", 100)}
        </div>
        <div className="grid-2">
          {text("tax_office", 120)}
          {text("official_address", 300)}
        </div>
        <div className="row">
          <Button variant="primary" loading={busy} onClick={async () => { setBusy(true); try { await onSave({ ...settings, legal }); } finally { setBusy(false); } }} data-testid="legal-save">{t("common.save")}</Button>
        </div>
      </div>
    </Card>
  );
}

/** A clinic's own expense or money account, or the name / on-off of an existing one. */
function AccountDialog({ account, parent, onClose, onDone }: { account: AccountInfo | null; parent: number; onClose: () => void; onDone: () => void }) {
  const { t, err } = useI18n();
  const isMoney = account ? !!account.money : parent === 1100;
  const [name, setName] = useState(account?.name ?? "");
  const [money, setMoney] = useState<MoneyKind>(account?.money ?? "bank");
  const [bankName, setBankName] = useState(account?.bank_name ?? "");
  const [bankNo, setBankNo] = useState(account?.bank_no ?? "");
  const [active, setActive] = useState(account?.is_active ?? true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const system = !!account?.is_system;
  const valid = system || !!name.trim();
  const save = async () => {
    if (!valid) return;
    setBusy(true);
    setError("");
    try {
      await rpc("fin.accounts.save", {
        id: account?.id ?? null,
        parent_id: account ? null : parent,
        name: name.trim() || null,
        money: isMoney ? money : null,
        bank_name: bankName.trim() || null,
        bank_no: bankNo.trim() || null,
        is_active: active,
        version: account?.version ?? null,
      });
      onDone();
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={account ? accountName(account, t) : t(isMoney ? "fin.settings.money.new" : "billing.settings.category.new")} onClose={onClose} testId="category-dialog" footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" disabled={!valid} loading={busy} onClick={save} data-testid="category-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        {!system && <Field label={t("catalog.name")}><TextInput value={name} onChange={(e) => setName(e.target.value)} maxLength={80} data-testid="category-name" autoFocus /></Field>}
        {isMoney && !account && (
          <Segmented<MoneyKind> value={money} onChange={setMoney} label={t("fin.account")} block options={(["cash", "bank", "mobile"] as const).map((m) => ({ value: m, label: t(`fin.money.${m}`), testId: `money-kind-${m}` }))} />
        )}
        {isMoney && money !== "cash" && (
          <div className="grid-2">
            <Field label={t("fin.bankName")} optional><TextInput value={bankName} onChange={(e) => setBankName(e.target.value)} maxLength={80} data-testid="account-bank-name" /></Field>
            <Field label={t("fin.bankNo")} optional><TextInput value={bankNo} onChange={(e) => setBankNo(e.target.value)} maxLength={40} dir="ltr" data-testid="account-bank-no" /></Field>
          </div>
        )}
        <Checkbox checked={active} onChange={setActive} disabled={account?.id === 1110} testId="category-active">{t("common.active")}</Checkbox>
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
    { doctor: rule?.doctor_id ?? "", service: rule?.service_id ?? "", value: rule ? String(rule.value / 100) : "" },
    { value: (s) => { const r = amountRule(s, true); if (r) return r; return kind === "percent" && minorOf(s) > 10_000 ? "rule.commission_value" : null; } },
  );
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await rpc("fin.shares.rules_save", { id: rule?.id ?? null, doctor_id: form.values.doctor || null, service_id: form.values.service || null, kind, value: parseAfn(form.values.value) ?? 0, version: rule?.version ?? null });
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
