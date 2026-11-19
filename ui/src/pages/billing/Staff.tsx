// «کارمندان و معاش» (finance phase 2): employees, the monthly payroll and what each employee is owed. A payroll
// is approved once per period and never edited (void it with a reason while nothing is paid on it); salaries and
// advances are paid with payment vouchers. Salary tax is entered by hand: the app does not compute tax.
import { useEffect, useMemo, useState } from "react";
import { Ban, CalendarRange, CheckCircle2, HandCoins, Plus, Printer, UserRound, UsersRound, Wallet } from "lucide-react";
import type { CalendarSystem, EmployeeInfo, NewPayrollLine, PayrollInfo, PayrollLineInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { afnInput, amountRule, invalidateAccounts, parseAfn } from "../../lib/billing";
import { monthRange, todayIso } from "../../lib/calendar";
import { formatDate } from "../../lib/dates";
import { formatAmount } from "../../lib/money";
import { useForm } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, CardHeader, Stat } from "../../ui/Card";
import { Checkbox, Segmented } from "../../ui/Controls";
import { DateField } from "../../ui/DateField";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Field, TextInput, Textarea } from "../../ui/Field";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { DocumentPreview } from "../../print/DocumentPreview";
import { PayrollSheet } from "../../print/finance";
import { useLetterhead } from "../../print/letterhead";
import { AmountInput, MoneyAccountSelect, StatusBadge, minorOf } from "./common";
import type { BillingPerms } from "./common";
import { VoidDialog } from "./BillingPage";

type Sub = "payroll" | "employees";

export function StaffTab({ perms, calendar }: { perms: BillingPerms; calendar: CalendarSystem }) {
  const { t } = useI18n();
  const [sub, setSub] = useState<Sub>("payroll");
  return (
    <div className="stack" data-testid="billing-staff">
      <Segmented<Sub>
        value={sub}
        onChange={setSub}
        label={t("billing.tab.staff")}
        options={[
          { value: "payroll", label: t("staff.payroll"), testId: "staff-payroll" },
          { value: "employees", label: t("staff.employees"), testId: "staff-employees" },
        ]}
      />
      {sub === "payroll" && <PayrollView perms={perms} calendar={calendar} />}
      {sub === "employees" && <EmployeesView perms={perms} calendar={calendar} />}
    </div>
  );
}

// ───────────────────────────── employees ─────────────────────────────

function useEmployees(includeInactive: boolean, tick: number) {
  const { err } = useI18n();
  const [list, setList] = useState<EmployeeInfo[] | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    rpc("fin.employees.list", { include_inactive: includeInactive })
      .then((v) => {
        setList(v);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [includeInactive, tick]);
  return { list, error };
}

function EmployeesView({ perms, calendar }: { perms: BillingPerms; calendar: CalendarSystem }) {
  const { t, lang } = useI18n();
  const toast = useToast();
  const [all, setAll] = useState(false);
  const [tick, setTick] = useState(0);
  const { list, error } = useEmployees(all, tick);
  const [editing, setEditing] = useState<EmployeeInfo | "new" | null>(null);
  const [paying, setPaying] = useState<{ e: EmployeeInfo; kind: "salary" | "advance" } | null>(null);
  if (error && !list) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const money = (v: number) => formatAmount(v, lang);
  const owed = (list ?? []).reduce((s, e) => s + e.owed, 0);
  const advances = (list ?? []).reduce((s, e) => s + e.advances, 0);
  return (
    <>
      <div className="stat-grid">
        <Stat icon={Wallet} label={t("staff.owedTotal")} value={list ? money(owed) : "…"} tone="warning" testId="staff-owed" />
        <Stat icon={HandCoins} label={t("staff.advancesTotal")} value={list ? money(advances) : "…"} tone="info" testId="staff-advances" />
      </div>
      <Card flush>
        <CardHeader
          icon={UsersRound}
          title={t("staff.employees")}
          description={t("staff.employees.hint")}
          actions={
            <div className="row">
              <Checkbox checked={all} onChange={setAll}>{t("staff.showInactive")}</Checkbox>
              {perms.payroll && <Button variant="primary" icon={Plus} onClick={() => setEditing("new")} data-testid="employee-add">{t("staff.employee.new")}</Button>}
            </div>
          }
        />
        <div className="table-wrap">
          <table className="table" data-testid="employees-table">
            <thead><tr><th>{t("staff.name")}</th><th>{t("staff.position")}</th><th>{t("staff.hireDate")}</th><th className="num">{t("staff.baseSalary")}</th><th className="num">{t("staff.owed")}</th><th className="num">{t("staff.advances")}</th><th /></tr></thead>
            <tbody>
              {!list && <SkeletonRows cols={7} />}
              {list?.map((e) => (
                <tr key={e.id} className={e.is_active ? undefined : "row-void"} data-testid={`employee-${e.id}`}>
                  <td><span className="cell-strong">{e.full_name}</span>{e.father_name && <span className="subtle"> · {e.father_name}</span>}</td>
                  <td>{e.position}</td>
                  <td>{formatDate(e.hire_date, lang, calendar)}</td>
                  <td className="num">{money(e.base_salary)}</td>
                  <td className="num cell-strong">{e.owed ? money(e.owed) : "—"}</td>
                  <td className="num">{e.advances ? money(e.advances) : "—"}</td>
                  <td className="cell-actions">
                    <span className="row">
                      {perms.payroll && perms.edit && e.owed > 0 && <Button size="sm" variant="link" onClick={() => setPaying({ e, kind: "salary" })} data-testid={`employee-pay-${e.id}`}>{t("staff.paySalary")}</Button>}
                      {perms.payroll && perms.edit && e.is_active && <Button size="sm" variant="link" onClick={() => setPaying({ e, kind: "advance" })} data-testid={`employee-advance-${e.id}`}>{t("staff.giveAdvance")}</Button>}
                      {perms.payroll && <Button size="sm" variant="link" onClick={() => setEditing(e)} data-testid={`employee-edit-${e.id}`}>{t("common.edit")}</Button>}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {list && !list.length && <EmptyState icon={UserRound} title={t("staff.none")}>{t("staff.noneHint")}</EmptyState>}
      </Card>
      {editing && (
        <EmployeeDialog
          employee={editing === "new" ? null : editing}
          calendar={calendar}
          onClose={() => setEditing(null)}
          onDone={() => {
            setEditing(null);
            toast.success(t("common.saved"));
            setTick((x) => x + 1);
          }}
        />
      )}
      {paying && (
        <StaffPayDialog
          kind={paying.kind}
          employee={paying.e}
          calendar={calendar}
          onClose={() => setPaying(null)}
          onDone={() => {
            setPaying(null);
            invalidateAccounts();
            toast.success(t("staff.paid"));
            setTick((x) => x + 1);
          }}
        />
      )}
    </>
  );
}

function EmployeeDialog({ employee, calendar, onClose, onDone }: { employee: EmployeeInfo | null; calendar: CalendarSystem; onClose: () => void; onDone: () => void }) {
  const { t, err } = useI18n();
  const [hire, setHire] = useState(employee?.hire_date ?? todayIso());
  const [end, setEnd] = useState(employee?.end_date ?? "");
  const [active, setActive] = useState(employee?.is_active ?? true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const form = useForm(
    {
      name: employee?.full_name ?? "",
      father: employee?.father_name ?? "",
      position: employee?.position ?? "",
      tin: employee?.tin ?? "",
      nid: employee?.national_id ?? "",
      phone: employee?.phone ?? "",
      base: employee ? afnInput(employee.base_salary) : "",
      note: employee?.note ?? "",
    },
    {
      name: (s) => (s.trim() ? null : "rule.required"),
      position: (s) => (s.trim() ? null : "rule.required"),
      base: (s) => amountRule(s, true),
    },
  );
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      const v = form.values;
      await rpc("fin.employees.save", {
        id: employee?.id ?? null,
        full_name: v.name.trim(),
        father_name: v.father.trim() || null,
        position: v.position.trim(),
        tin: v.tin.trim() || null,
        national_id: v.nid.trim() || null,
        phone: v.phone.trim() || null,
        doctor_id: employee?.doctor_id ?? null,
        hire_date: hire,
        end_date: end || null,
        base_salary: parseAfn(v.base) ?? 0,
        note: v.note.trim() || null,
        is_active: active,
        version: employee?.version ?? null,
      });
      onDone();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  const fe = (k: Parameters<typeof form.error>[0]) => form.error(k) && t(form.error(k)!);
  return (
    <Dialog
      title={employee ? employee.full_name : t("staff.employee.new")}
      onClose={onClose}
      testId="employee-dialog"
      dismissOnScrim={false}
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={submit} data-testid="employee-save">{t("common.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="grid-2">
          <Field label={t("staff.name")} error={fe("name")}><TextInput value={form.values.name} onChange={(e) => form.set("name", e.target.value)} maxLength={100} data-testid="employee-name" autoFocus /></Field>
          <Field label={t("staff.fatherName")} optional><TextInput value={form.values.father} onChange={(e) => form.set("father", e.target.value)} maxLength={100} data-testid="employee-father" /></Field>
          <Field label={t("staff.position")} error={fe("position")}><TextInput value={form.values.position} onChange={(e) => form.set("position", e.target.value)} maxLength={60} data-testid="employee-position" /></Field>
          <Field label={t("staff.baseSalary")} hint={t("staff.baseSalary.hint")} error={fe("base")}><AmountInput value={form.values.base} onChange={(v) => form.set("base", v)} testId="employee-base" /></Field>
          <Field label={t("staff.hireDate")}><DateField value={hire} onChange={setHire} calendar={calendar} label={t("staff.hireDate")} testId="employee-hire" yearsBefore={30} yearsAfter={1} /></Field>
          <Field label={t("staff.endDate")} optional><DateField value={end} onChange={setEnd} calendar={calendar} label={t("staff.endDate")} testId="employee-end" yearsBefore={30} yearsAfter={1} /></Field>
          <Field label={t("fin.legal.tin")} optional><TextInput value={form.values.tin} onChange={(e) => form.set("tin", e.target.value)} maxLength={20} className="ltr" data-testid="employee-tin" /></Field>
          <Field label={t("staff.nationalId")} optional><TextInput value={form.values.nid} onChange={(e) => form.set("nid", e.target.value)} maxLength={30} className="ltr" data-testid="employee-nid" /></Field>
          <Field label={t("staff.phone")} optional><TextInput value={form.values.phone} onChange={(e) => form.set("phone", e.target.value)} maxLength={30} className="ltr" data-testid="employee-phone" /></Field>
        </div>
        <Field label={t("staff.note")} optional><Textarea rows={2} value={form.values.note} onChange={(e) => form.set("note", e.target.value)} maxLength={300} /></Field>
        {employee && <Checkbox checked={active} onChange={setActive}>{t("staff.active")}</Checkbox>}
      </form>
    </Dialog>
  );
}

/** Pays an employee's salary (up to what the payrolls owe them) or gives an advance. */
function StaffPayDialog({ kind, employee, calendar, onClose, onDone }: { kind: "salary" | "advance"; employee: EmployeeInfo; calendar: CalendarSystem; onClose: () => void; onDone: () => void }) {
  const { t, lang, err } = useI18n();
  const [date, setDate] = useState(todayIso());
  const [account, setAccount] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const max = kind === "salary" ? employee.owed : Infinity;
  const form = useForm(
    { amount: kind === "salary" ? afnInput(employee.owed) : "", reference: "" },
    { amount: (s) => amountRule(s) ?? (minorOf(s) > max ? "rule.amount_too_high" : null) },
  );
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (!form.validate() || account === null) return;
    setBusy(true);
    try {
      await rpc("fin.vouchers.create", {
        date, account_id: account, purpose: kind, supplier_id: null, employee_id: employee.id, doctor_id: null,
        name: null, reference: form.values.reference.trim() || null, note: null, lines: [], amount: minorOf(form.values.amount), withholding: null,
      });
      onDone();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={`${t(kind === "salary" ? "staff.paySalary" : "staff.giveAdvance")} · ${employee.full_name}`}
      description={kind === "salary" ? t("staff.paySalary.hint").replace("{n}", formatAmount(employee.owed, lang)) : t("staff.giveAdvance.hint")}
      onClose={onClose}
      testId="staff-pay-dialog"
      dismissOnScrim={false}
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={Wallet} loading={busy} onClick={submit} data-testid="staff-pay-save">{t("common.save")}</Button></>}
    >
      <form className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <div className="grid-2">
          <Field label={t("pay.amount")} error={form.error("amount") && t(form.error("amount")!)}><AmountInput value={form.values.amount} onChange={(v) => form.set("amount", v)} testId="staff-pay-amount" autoFocus /></Field>
          <Field label={t("exp.paidFrom")}><MoneyAccountSelect value={account} onChange={setAccount} testId="staff-pay-account" /></Field>
          <Field label={t("doc.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("doc.date")} testId="staff-pay-date" yearsBefore={1} yearsAfter={0} /></Field>
          <Field label={t("fin.reference")} optional><TextInput value={form.values.reference} onChange={(e) => form.set("reference", e.target.value)} maxLength={60} /></Field>
        </div>
      </form>
    </Dialog>
  );
}

// ───────────────────────────── payroll ─────────────────────────────

type Draft = { employee_id: string; base: string; extra: string; commission: string; advance: string; tax: string };

const toDraft = (l: PayrollLineInfo): Draft => ({
  employee_id: l.employee_id,
  base: afnInput(l.base),
  extra: l.extra ? afnInput(l.extra) : "",
  commission: l.commission ? afnInput(l.commission) : "",
  advance: l.advance ? afnInput(l.advance) : "",
  tax: l.tax ? afnInput(l.tax) : "",
});

function PayrollView({ perms, calendar }: { perms: BillingPerms; calendar: CalendarSystem }) {
  const { t, lang, err } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const month = monthRange(todayIso(), calendar);
  const [from, setFrom] = useState(month.from);
  const [to, setTo] = useState(month.to);
  const [payroll, setPayroll] = useState<PayrollInfo | null>(null);
  const [rows, setRows] = useState<Draft[]>([]);
  const [date, setDate] = useState(todayIso());
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [voiding, setVoiding] = useState(false);
  const [printing, setPrinting] = useState(false);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (from > to) return;
    rpc("fin.payroll.draft", { from, to })
      .then((p) => {
        setPayroll(p);
        setRows(p.lines.map(toDraft));
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  }, [from, to, tick]);
  const approved = payroll?.id != null;
  const lines = useMemo(
    () =>
      rows.map((r) => {
        const gross = minorOf(r.base) + minorOf(r.extra) + minorOf(r.commission);
        return { gross, net: gross - minorOf(r.advance) - minorOf(r.tax) };
      }),
    [rows],
  );
  if (error && !payroll) return <ErrorState message={error} onRetry={() => setTick((x) => x + 1)} />;
  const money = (v: number) => formatAmount(v, lang);
  const set = (i: number, k: keyof Draft, v: string) => setRows(rows.map((r, j) => (j === i ? { ...r, [k]: v } : r)));
  const sum = (k: keyof Draft) => rows.reduce((s, r) => s + minorOf(r[k]), 0);
  const gross = lines.reduce((s, l) => s + l.gross, 0);
  const net = lines.reduce((s, l) => s + l.net, 0);
  const invalid = lines.some((l) => l.net < 0) || rows.some((r) => ["base", "extra", "commission", "advance", "tax"].some((k) => r[k as keyof Draft] !== "" && amountRule(r[k as keyof Draft], true)));
  const approve = async () => {
    setBusy(true);
    setError("");
    try {
      const lines: NewPayrollLine[] = rows.map((r) => ({ employee_id: r.employee_id, base: minorOf(r.base), extra: minorOf(r.extra), commission: minorOf(r.commission), advance: minorOf(r.advance), tax: minorOf(r.tax), note: null }));
      const p = await rpc("fin.payroll.create", { from, to, date, lines, note: null });
      setPayroll(p);
      invalidateAccounts();
      toast.success(t("staff.payroll.approved"));
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  const cell = (i: number, k: keyof Draft) =>
    approved ? null : <AmountInput value={rows[i][k]} onChange={(v) => set(i, k, v)} testId={`payroll-${k}-${i}`} invalid={rows[i][k] !== "" && !!amountRule(rows[i][k], true)} />;
  return (
    <>
      <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end", justifyContent: "space-between" }}>
        <div className="row" style={{ flexWrap: "wrap", alignItems: "flex-end" }}>
          <Field label={t("period.from")}><DateField value={from} onChange={setFrom} calendar={calendar} label={t("period.from")} testId="payroll-from" yearsBefore={2} yearsAfter={0} /></Field>
          <Field label={t("period.to")}><DateField value={to} onChange={setTo} calendar={calendar} label={t("period.to")} testId="payroll-to" yearsBefore={2} yearsAfter={1} /></Field>
        </div>
        {approved && (
          <div className="row">
            <Button icon={Printer} disabled={!letterhead} onClick={() => setPrinting(true)} data-testid="payroll-print">{t("staff.payroll.print")}</Button>
            {perms.void && perms.payroll && payroll?.status === "posted" && <Button icon={Ban} onClick={() => setVoiding(true)} data-testid="payroll-void">{t("fin.void")}</Button>}
          </div>
        )}
      </div>
      {from > to && <Notice tone="warning">{t("rule.period_range")}</Notice>}
      {error && payroll && <Notice tone="danger" testId="payroll-error">{error}</Notice>}
      <Card flush>
        <CardHeader
          icon={CalendarRange}
          title={approved ? `${t("staff.payroll")} · ${payroll?.number}` : t("staff.payroll.draft")}
          description={approved ? `${formatDate(payroll!.date!, lang, calendar)} · ${payroll?.created_by_name ?? ""}` : t("staff.payroll.hint")}
          actions={approved ? <StatusBadge status={payroll!.status!} /> : <Badge tone="warning">{t("staff.payroll.notApproved")}</Badge>}
        />
        <div className="table-wrap">
          <table className="table payroll-grid" data-testid="payroll-table">
            <thead><tr><th>{t("staff.name")}</th><th className="num">{t("staff.base")}</th><th className="num">{t("staff.extra")}</th><th className="num">{t("staff.commission")}</th><th className="num">{t("exp.gross")}</th><th className="num">{t("staff.advanceBack")}</th><th className="num">{t("staff.tax")}</th><th className="num">{t("staff.net")}</th></tr></thead>
            <tbody>
              {!payroll && <SkeletonRows cols={8} />}
              {payroll?.lines.map((l, i) => (
                <tr key={l.employee_id} data-testid={`payroll-row-${i}`}>
                  <td><span className="cell-strong">{l.full_name}</span> <span className="subtle">· {l.position}</span>{!approved && l.advances_open > 0 && <div className="subtle">{t("staff.advancesOpen")}: {money(l.advances_open)}</div>}</td>
                  <td className="num">{cell(i, "base") ?? money(l.base)}</td>
                  <td className="num">{cell(i, "extra") ?? money(l.extra)}</td>
                  <td className="num">{cell(i, "commission") ?? money(l.commission)}</td>
                  <td className="num">{money(approved ? l.gross : lines[i]?.gross ?? 0)}</td>
                  <td className="num">{cell(i, "advance") ?? money(l.advance)}</td>
                  <td className="num">{cell(i, "tax") ?? money(l.tax)}</td>
                  <td className={`num cell-strong ${!approved && (lines[i]?.net ?? 0) < 0 ? "text-danger" : ""}`}>{money(approved ? l.net : lines[i]?.net ?? 0)}</td>
                </tr>
              ))}
            </tbody>
            {payroll && payroll.lines.length > 0 && (
              <tfoot>
                <tr className="table-total">
                  <td>{t("ov.total")}</td>
                  <td className="num">{money(approved ? payroll.lines.reduce((s, l) => s + l.base, 0) : sum("base"))}</td>
                  <td className="num">{money(approved ? payroll.lines.reduce((s, l) => s + l.extra, 0) : sum("extra"))}</td>
                  <td className="num">{money(approved ? payroll.lines.reduce((s, l) => s + l.commission, 0) : sum("commission"))}</td>
                  <td className="num" data-testid="payroll-gross">{money(approved ? payroll.gross : gross)}</td>
                  <td className="num">{money(approved ? payroll.advance : sum("advance"))}</td>
                  <td className="num">{money(approved ? payroll.tax : sum("tax"))}</td>
                  <td className="num" data-testid="payroll-net">{money(approved ? payroll.net : net)}</td>
                </tr>
              </tfoot>
            )}
          </table>
        </div>
        {payroll && !payroll.lines.length && <EmptyState icon={UsersRound} title={t("staff.none")}>{t("staff.noneHint")}</EmptyState>}
      </Card>
      {!approved && perms.payroll && payroll && payroll.lines.length > 0 && (
        <div className="row" style={{ justifyContent: "flex-end", alignItems: "flex-end", flexWrap: "wrap" }}>
          <Notice tone="info">{t("exp.withheldKind.hint")}</Notice>
          <Field label={t("staff.payroll.date")}><DateField value={date} onChange={setDate} calendar={calendar} label={t("staff.payroll.date")} testId="payroll-date" yearsBefore={1} yearsAfter={0} /></Field>
          <Button variant="primary" icon={CheckCircle2} loading={busy} disabled={invalid || from > to} onClick={approve} data-testid="payroll-approve">{t("staff.payroll.approve")}</Button>
        </div>
      )}
      {voiding && payroll?.id != null && (
        <VoidDialog
          id={payroll.id}
          number={payroll.number ?? ""}
          onClose={() => setVoiding(false)}
          onDone={() => {
            setVoiding(false);
            invalidateAccounts();
            toast.success(t("fin.voided"));
            setTick((x) => x + 1);
          }}
        />
      )}
      {printing && payroll && letterhead && (
        <DocumentPreview title={t("staff.payroll")} paper="a4" fileStem={payroll.number ?? "PR"} testId="payroll-preview" onClose={() => setPrinting(false)} sheet={<PayrollSheet payroll={payroll} letterhead={letterhead} lang={lang} calendar={calendar} />} />
      )}
    </>
  );
}
