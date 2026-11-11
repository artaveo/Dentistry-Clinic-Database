import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { FileDown, FileUp, FolderOpen, MoreHorizontal, Search, UserPlus, UserRound, Users } from "lucide-react";
import type { ClinicProfile, ExportResult, PatientInfo } from "../../../../shared/ts/contract";
import type { Perms } from "../reception/useScheduling";
import { isSessionError, revealExport, rpc } from "../../lib/api";
import { ageOn, digits, formatDate } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, Page, PageHeader } from "../../ui/Card";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { Popover } from "../../ui/Overlay";
import { PatientForm } from "./PatientForm";
import { PatientProfile } from "./PatientProfile";
import type { BillingPerms } from "../billing/common";
import { ImportDialog } from "./ImportDialog";

type View = { kind: "list" } | { kind: "create" } | { kind: "edit"; patient: PatientInfo } | { kind: "profile"; id: string };

export type AppointmentPerms = Perms & { view: boolean };
/** Medical history, prescriptions and clinical documents (clinical.view / clinical.edit). */
export type ClinicalPerms = { view: boolean; edit: boolean };

/** `openRequest`: the global search (9.7) asks for one patient's profile; `n` makes a repeated ask count. */
export function PatientsPage({ canEdit, billing, clinical, appointments, clinic, clinicName, openRequest }: { canEdit: boolean; billing: BillingPerms; clinical: ClinicalPerms; appointments: AppointmentPerms; clinic: ClinicProfile | null; clinicName: string; openRequest?: { id: string; n: number } | null }) {
  const [view, setView] = useState<View>(openRequest ? { kind: "profile", id: openRequest.id } : { kind: "list" });
  useEffect(() => {
    if (openRequest) setView({ kind: "profile", id: openRequest.id });
  }, [openRequest?.n]);
  if (view.kind === "create") return <PatientForm onDone={(p) => setView({ kind: "profile", id: p.id })} onCancel={() => setView({ kind: "list" })} />;
  if (view.kind === "edit") return <PatientForm patient={view.patient} onDone={(p) => setView({ kind: "profile", id: p.id })} onCancel={() => setView({ kind: "profile", id: view.patient.id })} />;
  if (view.kind === "profile") return <PatientProfile patientId={view.id} canEdit={canEdit} billing={billing} clinical={clinical} appointments={appointments} clinic={clinic} clinicName={clinicName} onBack={() => setView({ kind: "list" })} onEdit={(p) => setView({ kind: "edit", patient: p })} />;
  return <PatientList canEdit={canEdit} onCreate={() => setView({ kind: "create" })} onOpen={(id) => setView({ kind: "profile", id })} />;
}

const PAGE = 100;

/** B11: a table row that opens something is reachable by Tab and opens with Enter or Space. */
const rowKeys = (open: () => void) => (e: KeyboardEvent<HTMLElement>) => {
  if (e.target !== e.currentTarget) return;
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    open();
  }
};

/** Age from the exact birth date, else the approximate age typed at registration (marked with ~). */
function ageText(p: PatientInfo, lang: Parameters<typeof digits>[1]): string {
  const exact = p.date_of_birth ? ageOn(p.date_of_birth) : null;
  if (exact !== null) return digits(exact, lang);
  return p.approximate_age !== null ? `~${digits(p.approximate_age, lang)}` : "—";
}

function PatientList({ canEdit, onCreate, onOpen }: { canEdit: boolean; onCreate: () => void; onOpen: (id: string) => void }) {
  const { t, err, lang } = useI18n();
  const [items, setItems] = useState<PatientInfo[] | null>(null);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");
  const [importing, setImporting] = useState(false);
  const [exported, setExported] = useState<ExportResult | null>(null);
  const [total, setTotal] = useState(0);
  const [moreBusy, setMoreBusy] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const debounce = useRef<number | undefined>(undefined);
  // A newer search must win over a slower older answer.
  const seq = useRef(0);

  const load = (q: string) => {
    const mine = ++seq.current;
    return rpc("patients.list", { query: q || null, status: null, limit: PAGE, offset: 0 })
      .then((r) => {
        if (mine !== seq.current) return;
        setItems(r.items);
        setTotal(r.total);
        setError("");
      })
      .catch((e) => {
        if (mine === seq.current && !isSessionError(e)) setError(err(e));
      });
  };

  // B2: the next page is appended; the list never stops silently.
  const loadMore = () => {
    if (!items || moreBusy) return;
    const mine = seq.current;
    setMoreBusy(true);
    rpc("patients.list", { query: query || null, status: null, limit: PAGE, offset: items.length })
      .then((r) => {
        if (mine !== seq.current) return;
        setItems((prev) => (prev ? [...prev, ...r.items.filter((x) => !prev.some((y) => y.id === x.id))] : r.items));
        setTotal(r.total);
      })
      .catch((e) => {
        if (!isSessionError(e)) setError(err(e));
      })
      .finally(() => setMoreBusy(false));
  };

  useEffect(() => {
    load("");
  }, []);

  const onQuery = (q: string) => {
    setQuery(q);
    window.clearTimeout(debounce.current);
    debounce.current = window.setTimeout(() => load(q), 200);
  };

  // OF-022: the Core writes a real .xlsx into the exports folder; the user gets a message and a button to open it.
  const exportPatients = async () => {
    setError("");
    setExported(null);
    try {
      setExported(await rpc("patients.export", {}));
    } catch (x) {
      setError(err(x));
    }
  };

  return (
    <Page testId="page-patients">
      <PageHeader
        title={t("patients.title")}
        description={t("patients.subtitle")}
        actions={
          canEdit ? (
            <>
              <Popover
                open={menuOpen}
                onClose={() => setMenuOpen(false)}
                width={240}
                testId="patients-more-menu"
                trigger={<IconButton icon={MoreHorizontal} label={t("patients.moreActions")} aria-haspopup="menu" aria-expanded={menuOpen} onClick={() => setMenuOpen((o) => !o)} data-testid="patients-more" />}
              >
                <div className="menu-section" role="menu">
                  <button type="button" role="menuitem" className="menu-item" onClick={() => { setMenuOpen(false); setImporting(true); }} data-testid="patients-import-open"><FileUp aria-hidden />{t("patients.import")}</button>
                  <button type="button" role="menuitem" className="menu-item" onClick={() => { setMenuOpen(false); exportPatients(); }} data-testid="patients-export"><FileDown aria-hidden />{t("patients.export")}</button>
                </div>
              </Popover>
              <Button variant="primary" icon={UserPlus} onClick={onCreate} data-testid="add-patient-open">{t("patients.add")}</Button>
            </>
          ) : undefined
        }
      />
      {exported && (
        <Notice tone="success" testId="export-done">
          {t("patients.export.done").replace("{name}", exported.file_name).replace("{n}", digits(exported.rows, lang))}{" "}
          <Button variant="link" icon={FolderOpen} onClick={() => revealExport(exported.file_path).catch((x) => setError(err(x)))} data-testid="export-open-folder">
            {t("patients.export.openFolder")}
          </Button>
        </Notice>
      )}
      <div className="control" style={{ maxWidth: 420 }}>
        <span className="control-icon"><Search aria-hidden /></span>
        <input value={query} onChange={(e) => onQuery(e.target.value)} placeholder={t("patients.search.placeholder")} data-testid="patients-search" />
      </div>
      <Card flush>
        {error ? (
          <ErrorState message={error} onRetry={() => load(query)} />
        ) : (
          <div className="table-wrap">
            <table className="table" data-testid="patient-list">
              <thead>
                <tr>
                  <th>{t("patients.column.number")}</th>
                  <th>{t("patients.column.name")}</th>
                  <th>{t("patients.column.father")}</th>
                  <th>{t("patients.column.phone")}</th>
                  <th>{t("patients.column.age")}</th>
                  <th>{t("patients.column.registered")}</th>
                </tr>
              </thead>
              <tbody>
                {!items && <SkeletonRows cols={6} />}
                {items?.map((p) => (
                  <tr key={p.id} className="patient-row" tabIndex={0} onClick={() => onOpen(p.id)} onKeyDown={rowKeys(() => onOpen(p.id))} style={{ cursor: "pointer" }} data-testid={`patient-row-${p.patient_number}`}>
                    <td><bdi className="ltr num">{p.patient_number}</bdi></td>
                    <td className="cell-strong">{p.full_name}{p.status !== "active" && <> <Badge dot>{t("patients.status.inactive")}</Badge></>}</td>
                    <td>{p.father_name ?? "—"}</td>
                    <td><bdi className="ltr">{p.phone ?? "—"}</bdi></td>
                    <td>{ageText(p, lang)}</td>
                    <td>{formatDate(p.registration_date, lang)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {items?.length === 0 && <EmptyState icon={query ? Search : Users} title={t(query ? "patients.emptySearch" : "patients.empty")} action={!query && canEdit ? <Button variant="primary" icon={UserRound} onClick={onCreate}>{t("patients.add")}</Button> : undefined} />}
          </div>
        )}
      </Card>
      {items && items.length > 0 && !error && (
        <div className="patients-list-foot" data-testid="patients-count">
          <span className="subtle t-caption">{t("patients.showing").replace("{shown}", digits(items.length, lang)).replace("{total}", digits(total, lang))}</span>
          {total > items.length && <Button size="sm" loading={moreBusy} onClick={loadMore} data-testid="patients-load-more">{t("patients.loadMore")}</Button>}
        </div>
      )}
      {importing && (
        <ImportDialog
          onClose={() => setImporting(false)}
          onImported={() => {
            setImporting(false);
            load(query);
          }}
        />
      )}
    </Page>
  );
}
