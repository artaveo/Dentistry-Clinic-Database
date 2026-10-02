import { useEffect, useState } from "react";
import type { LucideIcon } from "lucide-react";
import {
  Activity, CalendarClock, ClipboardList, FileHeart, FileText, Grid3x3, Images, Merge, NotebookPen,
  Pencil, Pill, Receipt, ScrollText, Trash2, Wallet,
} from "lucide-react";
import type { PatientInfo } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { formatDate } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../../ui/Card";
import { Badge, ErrorState, Loading } from "../../ui/Feedback";
import { Avatar } from "../../ui/Brand";
import { useToast } from "../../ui/Toast";
import { MedicalAlertBanner } from "./MedicalAlertBanner";
import { MedicalHistoryTab } from "./MedicalHistoryTab";
import { AttachmentsTab } from "./AttachmentsTab";
import { PatientAuditTab } from "./PatientAuditTab";
import { MergeDialog } from "./MergeDialog";
import { useReferenceList } from "./useReferenceList";

type TabId = "overview" | "medical" | "appointments" | "chart" | "notes" | "plans" | "treatments" | "prescriptions" | "invoices" | "payments" | "documents" | "timeline" | "audit";

const TABS: { id: TabId; icon: LucideIcon; labelKey: string; ready: boolean }[] = [
  { id: "overview", icon: FileText, labelKey: "patients.tab.overview", ready: true },
  { id: "medical", icon: FileHeart, labelKey: "patients.tab.medicalHistory", ready: true },
  { id: "appointments", icon: CalendarClock, labelKey: "patients.tab.appointments", ready: false },
  { id: "chart", icon: Grid3x3, labelKey: "patients.tab.dentalChart", ready: false },
  { id: "notes", icon: NotebookPen, labelKey: "patients.tab.clinicalNotes", ready: false },
  { id: "plans", icon: ClipboardList, labelKey: "patients.tab.treatmentPlans", ready: false },
  { id: "treatments", icon: Activity, labelKey: "patients.tab.treatments", ready: false },
  { id: "prescriptions", icon: Pill, labelKey: "patients.tab.prescriptions", ready: false },
  { id: "invoices", icon: Receipt, labelKey: "patients.tab.invoices", ready: false },
  { id: "payments", icon: Wallet, labelKey: "patients.tab.payments", ready: false },
  { id: "documents", icon: Images, labelKey: "patients.tab.documents", ready: true },
  { id: "timeline", icon: ScrollText, labelKey: "patients.tab.timeline", ready: false },
  { id: "audit", icon: ScrollText, labelKey: "patients.tab.audit", ready: true },
];

export function PatientProfile({ patientId, canEdit, onBack, onEdit }: { patientId: string; canEdit: boolean; onBack: () => void; onEdit: (p: PatientInfo) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [patient, setPatient] = useState<PatientInfo | null>(null);
  const [error, setError] = useState("");
  const [tab, setTab] = useState<TabId>("overview");
  const [merging, setMerging] = useState(false);
  // Bumped after every medical-history save so the alert banner (mounted
  // once, independently) re-fetches instead of showing stale data.
  const [medicalRefreshKey, setMedicalRefreshKey] = useState(0);
  const genders = useReferenceList("gender");
  const referrals = useReferenceList("referral_source");

  const load = () =>
    rpc("patients.get", { patient_id: patientId })
      .then((p) => {
        setPatient(p);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patientId]);

  const remove = async () => {
    if (!patient || !window.confirm(t("patients.deleteConfirm"))) return;
    await rpc("patients.delete", { id: patient.id, version: patient.version });
    toast.success(t("patients.deleted"));
    onBack();
  };

  if (error && !patient) return <Page><ErrorState message={error} onRetry={load} /></Page>;
  if (!patient) return <Page><Loading /></Page>;

  const genderLabel = genders.find((g) => g.id === patient.gender_id)?.label;
  const referralLabel = referrals.find((r) => r.id === patient.referral_source_id)?.label;

  return (
    <Page testId="page-patient-profile">
      <PageHeader
        title={patient.full_name}
        description={<bdi className="ltr num">{patient.patient_number}</bdi>}
        actions={
          canEdit ? (
            <>
              <Button icon={Merge} onClick={() => setMerging(true)} data-testid="patient-merge-open">{t("patients.merge")}</Button>
              <Button icon={Pencil} onClick={() => onEdit(patient)} data-testid="patient-edit-open">{t("common.edit")}</Button>
              <IconButton icon={Trash2} label={t("patients.delete")} onClick={remove} data-testid="patient-delete" />
            </>
          ) : undefined
        }
      />
      <Button variant="link" onClick={onBack} data-testid="patient-back">{t("patients.back")}</Button>

      <MedicalAlertBanner key={medicalRefreshKey} patientId={patient.id} />

      <div className="profile-tabs" role="tablist">
        {TABS.map(({ id, icon: Icon, labelKey, ready }) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            disabled={!ready}
            title={ready ? undefined : t("patients.tab.comingSoon")}
            className="profile-tab"
            onClick={() => ready && setTab(id)}
            data-testid={`patient-tab-${id}`}
          >
            <Icon aria-hidden />
            <span>{t(labelKey)}</span>
          </button>
        ))}
      </div>

      {tab === "overview" && (
        <Card>
          <CardHeader title={t("patients.tab.overview")} />
          <div className="person" style={{ marginBottom: "var(--space-4)" }}>
            <Avatar name={patient.full_name} size="lg" />
            <div className="person-text">
              <span className="cell-strong">{patient.full_name}</span>
              {patient.father_name && <span className="person-sub">{t("patient.field.fatherName")}: {patient.father_name}</span>}
            </div>
            {patient.status === "active" ? <Badge tone="success" dot>{t("patients.status.active")}</Badge> : <Badge dot>{t("patients.status.inactive")}</Badge>}
          </div>
          <dl className="kv">
            <div><dt>{t("patient.field.gender")}</dt><dd>{genderLabel ?? "—"}</dd></div>
            <div><dt>{t("patient.field.dateOfBirth")}</dt><dd>{patient.date_of_birth ? formatDate(patient.date_of_birth, lang) : patient.approximate_age != null ? `~${patient.approximate_age}` : "—"}</dd></div>
            <div><dt>{t("patient.field.phone")}</dt><dd><bdi className="ltr">{patient.phone ?? "—"}</bdi></dd></div>
            <div><dt>{t("patient.field.secondaryPhone")}</dt><dd><bdi className="ltr">{patient.secondary_phone ?? "—"}</bdi></dd></div>
            <div><dt>{t("patient.field.address")}</dt><dd>{patient.address ?? "—"}</dd></div>
            <div><dt>{t("patient.field.emergencyName")}</dt><dd>{patient.emergency_contact_name ?? "—"}</dd></div>
            <div><dt>{t("patient.field.emergencyPhone")}</dt><dd><bdi className="ltr">{patient.emergency_contact_phone ?? "—"}</bdi></dd></div>
            <div><dt>{t("patient.field.referralSource")}</dt><dd>{referralLabel ?? "—"}</dd></div>
            <div><dt>{t("patient.field.registrationDate")}</dt><dd>{formatDate(patient.registration_date, lang)}</dd></div>
            {patient.notes && <div><dt>{t("patient.field.notes")}</dt><dd>{patient.notes}</dd></div>}
          </dl>
        </Card>
      )}
      {tab === "medical" && (
        <MedicalHistoryTab patientId={patient.id} canEdit={canEdit} onSaved={() => setMedicalRefreshKey((k) => k + 1)} />
      )}
      {tab === "documents" && <AttachmentsTab patientId={patient.id} canEdit={canEdit} />}
      {tab === "audit" && <PatientAuditTab patientId={patient.id} />}

      {merging && patient && (
        <MergeDialog
          patient={patient}
          onClose={() => setMerging(false)}
          onMerged={() => {
            setMerging(false);
            toast.success(t("patients.merge.done"));
            load();
          }}
        />
      )}
    </Page>
  );
}
