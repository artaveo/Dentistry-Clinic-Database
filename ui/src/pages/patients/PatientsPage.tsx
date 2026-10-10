import { useEffect, useRef, useState } from "react";
import { FileDown, FileUp, FolderOpen, Search, UserPlus, UserRound, Users } from "lucide-react";
import type { ClinicProfile, ExportResult, PatientInfo } from "../../../../shared/ts/contract";
import type { Perms } from "../reception/useScheduling";
import { isSessionError, revealExport, rpc } from "../../lib/api";
import { digits, formatDate } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Card, Page, PageHeader } from "../../ui/Card";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../ui/Feedback";
import { PatientForm } from "./PatientForm";
import { PatientProfile } from "./PatientProfile";
import { ImportDialog } from "./ImportDialog";

type View = { kind: "list" } | { kind: "create" } | { kind: "edit"; patient: PatientInfo } | { kind: "profile"; id: string };

export type AppointmentPerms = Perms & { view: boolean };
/** Medical history, prescriptions and clinical documents (clinical.view / clinical.edit). */
export type ClinicalPerms = { view: boolean; edit: boolean };

export function PatientsPage({ canEdit, clinical, appointments, clinic, clinicName }: { canEdit: boolean; clinical: ClinicalPerms; appointments: AppointmentPerms; clinic: ClinicProfile | null; clinicName: string }) {
  const [view, setView] = useState<View>({ kind: "list" });
  if (view.kind === "create") return <PatientForm onDone={(p) => setView({ kind: "profile", id: p.id })} onCancel={() => setView({ kind: "list" })} />;
  if (view.kind === "edit") return <PatientForm patient={view.patient} onDone={(p) => setView({ kind: "profile", id: p.id })} onCancel={() => setView({ kind: "profile", id: view.patient.id })} />;
  if (view.kind === "profile") return <PatientProfile patientId={view.id} canEdit={canEdit} clinical={clinical} appointments={appointments} clinic={clinic} clinicName={clinicName} onBack={() => setView({ kind: "list" })} onEdit={(p) => setView({ kind: "edit", patient: p })} />;
  return <PatientList canEdit={canEdit} onCreate={() => setView({ kind: "create" })} onOpen={(id) => setView({ kind: "profile", id })} />;
}

function PatientList({ canEdit, onCreate, onOpen }: { canEdit: boolean; onCreate: () => void; onOpen: (id: string) => void }) {
  const { t, err, lang } = useI18n();
  const [items, setItems] = useState<PatientInfo[] | null>(null);
  const [query, setQuery] = useState("");
  const [error, setError] = useState("");
  const [importing, setImporting] = useState(false);
  const [exported, setExported] = useState<ExportResult | null>(null);
  const debounce = useRef<number | undefined>(undefined);

  const load = (q: string) =>
    rpc("patients.list", { query: q || null, status: null, limit: 100, offset: 0 })
      .then((r) => {
        setItems(r.items);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));

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
              <Button icon={FileUp} onClick={() => setImporting(true)} data-testid="patients-import-open">{t("patients.import")}</Button>
              <Button icon={FileDown} onClick={exportPatients} data-testid="patients-export">{t("patients.export")}</Button>
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
                  <th>{t("patients.column.registered")}</th>
                  <th>{t("patients.column.status")}</th>
                </tr>
              </thead>
              <tbody>
                {!items && <SkeletonRows cols={6} />}
                {items?.map((p) => (
                  <tr key={p.id} onClick={() => onOpen(p.id)} style={{ cursor: "pointer" }} data-testid={`patient-row-${p.patient_number}`}>
                    <td><bdi className="ltr num">{p.patient_number}</bdi></td>
                    <td className="cell-strong">{p.full_name}</td>
                    <td>{p.father_name ?? "—"}</td>
                    <td><bdi className="ltr">{p.phone ?? "—"}</bdi></td>
                    <td>{formatDate(p.registration_date, lang)}</td>
                    <td>{p.status === "active" ? <Badge tone="success" dot>{t("patients.status.active")}</Badge> : <Badge dot>{t("patients.status.inactive")}</Badge>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {items?.length === 0 && <EmptyState icon={query ? Search : Users} title={t(query ? "patients.emptySearch" : "patients.empty")} action={!query && canEdit ? <Button variant="primary" icon={UserRound} onClick={onCreate}>{t("patients.add")}</Button> : undefined} />}
          </div>
        )}
      </Card>
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
