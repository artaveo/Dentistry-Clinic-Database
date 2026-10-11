import { useEffect, useState } from "react";
import type { LucideIcon } from "lucide-react";
import {
  Activity, CalendarClock, ClipboardList, FileHeart, FileText, Grid3x3, Images, Merge, NotebookPen,
  History, Pencil, Pill, Receipt, ScrollText, Trash2, Wallet,
} from "lucide-react";
import type { ClinicProfile, PatientInfo, TreatmentItemInfo } from "../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../lib/api";
import { digits, formatDate } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { BackButton, Button, IconButton } from "../../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../../ui/Card";
import { Badge, ErrorState, Loading, Notice } from "../../ui/Feedback";
import { Avatar } from "../../ui/Brand";
import { useToast } from "../../ui/Toast";
import { MedicalAlertBanner } from "./MedicalAlertBanner";
import { MedicalHistoryTab } from "./MedicalHistoryTab";
import { AttachmentsTab } from "./AttachmentsTab";
import { DocumentsTab, PrescriptionsTab } from "./documents/DocumentTabs";
import { PatientAuditTab } from "./PatientAuditTab";
import { MergeDialog } from "./MergeDialog";
import { PatientAppointmentsTab } from "./PatientAppointmentsTab";
import type { AppointmentPerms, ClinicalPerms } from "./PatientsPage";
import { useReferenceList } from "./useReferenceList";
import { ChartTab } from "./clinical/ChartTab";
import { NotesTab } from "./clinical/NotesTab";
import { PlansTab } from "./clinical/PlansTab";
import { TreatmentsTab } from "./clinical/TreatmentsTab";
import { TimelineTab } from "./clinical/TimelineTab";
import { CourseCard } from "./clinical/CourseCard";

type TabId = "overview" | "medical" | "appointments" | "chart" | "notes" | "plans" | "treatments" | "prescriptions" | "invoices" | "payments" | "documents" | "timeline" | "audit";

const TABS: { id: TabId; icon: LucideIcon; labelKey: string; ready: boolean }[] = [
  { id: "overview", icon: FileText, labelKey: "patients.tab.overview", ready: true },
  { id: "medical", icon: FileHeart, labelKey: "patients.tab.medicalHistory", ready: true },
  { id: "appointments", icon: CalendarClock, labelKey: "patients.tab.appointments", ready: false },
  { id: "chart", icon: Grid3x3, labelKey: "patients.tab.dentalChart", ready: true },
  { id: "notes", icon: NotebookPen, labelKey: "patients.tab.clinicalNotes", ready: true },
  { id: "plans", icon: ClipboardList, labelKey: "patients.tab.treatmentPlans", ready: true },
  { id: "treatments", icon: Activity, labelKey: "patients.tab.treatments", ready: true },
  { id: "prescriptions", icon: Pill, labelKey: "patients.tab.prescriptions", ready: true },
  { id: "invoices", icon: Receipt, labelKey: "patients.tab.invoices", ready: false },
  { id: "payments", icon: Wallet, labelKey: "patients.tab.payments", ready: false },
  { id: "documents", icon: Images, labelKey: "patients.tab.documents", ready: true },
  { id: "timeline", icon: History, labelKey: "patients.tab.timeline", ready: true },
  { id: "audit", icon: ScrollText, labelKey: "patients.tab.audit", ready: true },
];

export function PatientProfile({ patientId, canEdit, clinical, appointments, clinic, clinicName, onBack, onEdit }: { patientId: string; canEdit: boolean; clinical: ClinicalPerms; appointments: AppointmentPerms; clinic: ClinicProfile | null; clinicName: string; onBack: () => void; onEdit: (p: PatientInfo) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [patient, setPatient] = useState<PatientInfo | null>(null);
  const [error, setError] = useState("");
  const [tab, setTab] = useState<TabId>("overview");
  const [merging, setMerging] = useState(false);
  const [blockedBy, setBlockedBy] = useState<number | null>(null);
  // A note opened from a treatment (5.6: each session has its own note).
  const [noteFor, setNoteFor] = useState<TreatmentItemInfo | null>(null);
  // Bumped when clinical work changes, so the overview's course card reloads.
  const [clinicalKey, setClinicalKey] = useState(0);
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

  // OF-041: a patient with open visits is not deleted; say how many, and where to cancel them.
  const remove = async () => {
    if (!patient || !window.confirm(t("patients.deleteConfirm"))) return;
    try {
      await rpc("patients.delete", { id: patient.id, version: patient.version });
    } catch (x) {
      if (isSessionError(x)) return;
      if (x instanceof ApiError && x.rule === "patient_has_open_appointments") {
        const open = await rpc("appointments.list", { date_from: null, date_to: null, doctor_id: null, chair_id: null, patient_id: patient.id, statuses: ["scheduled", "confirmed", "checked_in", "in_treatment"], limit: 200, offset: 0 });
        setBlockedBy(open.length);
        return;
      }
      setError(err(x));
      return;
    }
    toast.success(t("patients.deleted"));
    onBack();
  };

  if (error && !patient) return <Page><ErrorState message={error} onRetry={load} /></Page>;
  if (!patient) return <Page><Loading /></Page>;

  const genderLabel = genders.find((g) => g.id === patient.gender_id)?.label;
  // Clinical tabs need clinical.view (reception sees the visits and the files, not the medical record).
  const CLINICAL: TabId[] = ["medical", "prescriptions", "chart", "notes", "plans", "treatments", "timeline"];
  const available = (id: TabId, ready: boolean) =>
    id === "appointments" ? appointments.view : CLINICAL.includes(id) ? ready && clinical.view : ready;
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
      <BackButton onBack={onBack} label={t("patients.back")} testId="patient-back" />

      {blockedBy !== null && (
        <Notice tone="warning" testId="patient-delete-blocked">
          {t("patients.delete.blocked").replace("{n}", digits(blockedBy, lang))}{" "}
          <Button variant="link" onClick={() => { setBlockedBy(null); setTab("appointments"); }} data-testid="patient-delete-show">{t("patients.delete.show")}</Button>
        </Notice>
      )}
      {clinical.view && <MedicalAlertBanner key={`alert-${medicalRefreshKey}`} patientId={patient.id} hideReview={tab === "medical"} onOpenHistory={() => setTab("medical")} />}

      <div className="profile-tabs" role="tablist">
        {TABS.map(({ id, icon: Icon, labelKey, ready }) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            disabled={!available(id, ready)}
            title={available(id, ready) ? undefined : t("patients.tab.comingSoon")}
            className="profile-tab"
            onClick={() => available(id, ready) && setTab(id)}
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
      {tab === "overview" && clinical.view && <CourseCard key={`course-${clinicalKey}`} patient={patient} calendar={clinic?.calendar_system ?? "shamsi"} onOpen={() => setTab("plans")} />}
      {tab === "medical" && (
        <MedicalHistoryTab patientId={patient.id} female={genders.find((g) => g.id === patient.gender_id)?.code === "female"} canEdit={clinical.edit} onSaved={() => setMedicalRefreshKey((k) => k + 1)} />
      )}
      {tab === "appointments" && <PatientAppointmentsTab patient={patient} perms={appointments} clinic={clinic} clinicName={clinicName} />}
      {tab === "prescriptions" && <PrescriptionsTab patient={patient} canEdit={clinical.edit} calendar={clinic?.calendar_system ?? "shamsi"} />}
      {tab === "documents" && (clinical.view ? (
        <DocumentsTab patient={patient} gender={genders.find((g) => g.id === patient.gender_id)?.code ?? null} canEdit={clinical.edit} canEditFiles={canEdit} calendar={clinic?.calendar_system ?? "shamsi"} />
      ) : (
        <AttachmentsTab patientId={patient.id} canEdit={canEdit} />
      ))}
      {tab === "chart" && <ChartTab patient={patient} canEdit={clinical.edit} clinic={clinic} onChanged={() => setClinicalKey((k) => k + 1)} />}
      {tab === "plans" && (
        <PlansTab patient={patient} clinic={clinic} canEdit={clinical.edit} calendar={clinic?.calendar_system ?? "shamsi"} booking={{ edit: appointments.edit, treat: appointments.treat, createPatients: appointments.createPatients }} clinicName={clinicName} onChanged={() => setClinicalKey((k) => k + 1)} />
      )}
      {tab === "treatments" && (
        <TreatmentsTab patient={patient} clinic={clinic} calendar={clinic?.calendar_system ?? "shamsi"} onOpenChart={() => setTab("chart")} onNote={(i) => { setNoteFor(i); setTab("notes"); }} />
      )}
      {tab === "notes" && <NotesTab key={noteFor?.id ?? "notes"} patient={patient} clinic={clinic} canEdit={clinical.edit} calendar={clinic?.calendar_system ?? "shamsi"} initialItem={noteFor} onChanged={() => setNoteFor(null)} />}
      {tab === "timeline" && <TimelineTab patient={patient} calendar={clinic?.calendar_system ?? "shamsi"} />}
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
