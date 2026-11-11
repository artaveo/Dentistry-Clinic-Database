import { useEffect, useState } from "react";
import type { LucideIcon } from "lucide-react";
import {
  CalendarClock, Ellipsis, FileHeart, FileText, Grid3x3, History, Images, Merge, NotebookPen, Pencil, ScrollText, Trash2, TriangleAlert, Wallet,
} from "lucide-react";
import type { ClinicProfile, MedicalHistoryInfo, PatientInfo, TreatmentItemInfo } from "../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../lib/api";
import { ageOn, digits, formatDate } from "../../lib/dates";
import { alertsOf, tr, useMedicalQuestions } from "../../lib/medical";
import { useI18n } from "../../i18n";
import { BackButton, Button, IconButton } from "../../ui/Button";
import { Card, CardHeader, Page } from "../../ui/Card";
import { Segmented } from "../../ui/Controls";
import { Badge, ErrorState, Loading, Notice } from "../../ui/Feedback";
import { Avatar } from "../../ui/Brand";
import { Popover } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { useConfirm } from "../../ui/ConfirmDialog";
import { MedicalAlertBanner } from "./MedicalAlertBanner";
import { MedicalHistoryTab } from "./MedicalHistoryTab";
import { AttachmentsTab } from "./AttachmentsTab";
import { DocumentsTab, PrescriptionsTab } from "./documents/DocumentTabs";
import { PatientAuditTab } from "./PatientAuditTab";
import { PatientInvoicesTab, PatientPaymentsTab } from "../billing/PatientBilling";
import type { BillingPerms } from "../billing/common";
import { MergeDialog } from "./MergeDialog";
import { PatientAppointmentsTab } from "./PatientAppointmentsTab";
import type { AppointmentPerms, ClinicalPerms } from "./PatientsPage";
import { useReferenceList } from "./useReferenceList";
import { useLive } from "../../lib/lan";
import { PresenceBanner, usePresence } from "../../shell/Presence";
import { ChartTab } from "./clinical/ChartTab";
import { NotesTab } from "./clinical/NotesTab";
import { PlansTab } from "./clinical/PlansTab";
import { TreatmentsTab } from "./clinical/TreatmentsTab";
import { TimelineTab } from "./clinical/TimelineTab";
import { CourseCard } from "./clinical/CourseCard";

/** U2: seven tabs. «چارت و درمان», «حساب مالی» and «اسناد» hold several of the former tabs as sub-sections; history is in a menu. */
type MainTab = "overview" | "care" | "notes" | "medical" | "appointments" | "finance" | "documents" | "timeline" | "audit";
type CareSub = "chart" | "treatments" | "plans";
type FinanceSub = "invoices" | "payments";
type DocSub = "documents" | "prescriptions";
/** The thirteen former tabs: other screens and links still name a place by these. */
type Target = "overview" | "medical" | "appointments" | "chart" | "notes" | "plans" | "treatments" | "prescriptions" | "invoices" | "payments" | "documents" | "timeline" | "audit";

const TABS: { id: Exclude<MainTab, "timeline" | "audit">; icon: LucideIcon; labelKey: string }[] = [
  { id: "overview", icon: FileText, labelKey: "patients.tab.overview" },
  { id: "care", icon: Grid3x3, labelKey: "patients.tab.care" },
  { id: "notes", icon: NotebookPen, labelKey: "patients.tab.clinicalNotes" },
  { id: "medical", icon: FileHeart, labelKey: "patients.tab.medicalHistory" },
  { id: "appointments", icon: CalendarClock, labelKey: "patients.tab.appointments" },
  { id: "finance", icon: Wallet, labelKey: "patients.tab.finance" },
  { id: "documents", icon: Images, labelKey: "patients.tab.docs" },
];

/** Header chip: the first medical alert, with the others (and their details) in a popover. */
function AlertChip({ patientId, refreshKey, onOpenHistory }: { patientId: string; refreshKey: number; onOpenHistory: () => void }) {
  const { t, lang } = useI18n();
  const questions = useMedicalQuestions();
  const [history, setHistory] = useState<MedicalHistoryInfo | null>(null);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    let live = true;
    rpc("medical_history.get", { patient_id: patientId }).then((h) => live && setHistory(h)).catch(() => live && setHistory(null));
    return () => {
      live = false;
    };
  }, [patientId, refreshKey]);
  if (!history || !questions) return null;
  const alerts = alertsOf(questions, history.answers);
  if (!alerts.length) return null;
  const text = (a: (typeof alerts)[number]) => {
    const parts = [a.a.detail_text && (a.q.detail_kind === "months" ? `${digits(a.a.detail_text, lang)} ${t("medicalHistory.months")}` : a.a.detail_text), a.a.detail_choice && t(`mh.choice.${a.a.detail_choice}`)].filter(Boolean);
    return parts.length ? `${tr(a.q.label, lang)} (${parts.join("، ")})` : tr(a.q.label, lang);
  };
  return (
    <Popover
      open={open}
      onClose={() => setOpen(false)}
      align="start"
      width={320}
      testId="profile-alert-pop"
      trigger={
        <button type="button" className="profile-alert" aria-expanded={open} onClick={() => setOpen((x) => !x)} data-testid="profile-alert-chip">
          <TriangleAlert aria-hidden />
          <span className="profile-alert-text">{tr(alerts[0].q.label, lang)}</span>
          {alerts.length > 1 && <span className="profile-alert-more">+{digits(alerts.length - 1, lang)}</span>}
        </button>
      }
    >
      <div className="profile-alert-list" role="alert">
        <ul className="medical-alert-list">
          {alerts.map((a) => (
            <li key={a.q.id}>
              <strong>{text(a)}</strong>
              {a.q.alert_note && <span className="medical-alert-note"> — {tr(a.q.alert_note, lang)}</span>}
            </li>
          ))}
        </ul>
        <Button size="sm" variant="link" onClick={() => { setOpen(false); onOpenHistory(); }}>{t("medicalHistory.open")}</Button>
      </div>
    </Popover>
  );
}

export function PatientProfile({ patientId, canEdit, billing, clinical, appointments, clinic, clinicName, onBack, onEdit }: { patientId: string; canEdit: boolean; billing: BillingPerms; clinical: ClinicalPerms; appointments: AppointmentPerms; clinic: ClinicProfile | null; clinicName: string; onBack: () => void; onEdit: (p: PatientInfo) => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [patient, setPatient] = useState<PatientInfo | null>(null);
  const [error, setError] = useState("");
  const [tab, setTab] = useState<MainTab>("overview");
  const [careSub, setCareSub] = useState<CareSub>("chart");
  const [financeSub, setFinanceSub] = useState<FinanceSub>("invoices");
  const [docSub, setDocSub] = useState<DocSub>("documents");
  const [merging, setMerging] = useState(false);
  const [menu, setMenu] = useState(false);
  const [historyMenu, setHistoryMenu] = useState(false);
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

  /** Go to the place a former tab now lives in. */
  const go = (target: Target) => {
    switch (target) {
      case "chart":
      case "treatments":
      case "plans":
        setCareSub(target);
        setTab("care");
        break;
      case "invoices":
      case "payments":
        setFinanceSub(target);
        setTab("finance");
        break;
      case "documents":
      case "prescriptions":
        setDocSub(target);
        setTab("documents");
        break;
      default:
        setTab(target);
    }
  };

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
  // Roadmap 7.3: who else has this record open, and changes made on other computers.
  const others = usePresence("patient", patientId);
  useLive(["patients"], load);
  useLive(["chart", "notes", "plans", "treatment_items", "medical_history", "invoices", "payments", "documents", "attachments", "perio", "prescriptions", "appointments"], () => {
    setClinicalKey((k) => k + 1);
    setMedicalRefreshKey((k) => k + 1);
  });

  // OF-041: a patient with open visits is not deleted; say how many, and where to cancel them.
  const remove = async () => {
    if (!patient || !(await confirm({ title: t("cleanup.confirm.title"), text: t("patients.deleteConfirm"), confirm: t("common.delete"), danger: true }))) return;
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
  const available = (id: MainTab) =>
    id === "appointments" ? appointments.view : id === "finance" ? billing.view : id === "care" || id === "notes" || id === "medical" || id === "timeline" ? clinical.view : true;
  const referralLabel = referrals.find((r) => r.id === patient.referral_source_id)?.label;
  const age = patient.date_of_birth ? ageOn(patient.date_of_birth) : patient.approximate_age;
  const inChart = tab === "care" && careSub === "chart";

  return (
    <div className="page profile-page" data-testid="page-patient-profile">
      <div className="profile-sticky">
        <header className="profile-head">
          <BackButton onBack={onBack} label={t("patients.backShort")} testId="patient-back" />
          <Avatar name={patient.full_name} size="sm" />
          <h1 className="profile-name" data-testid="page-title">{patient.full_name}</h1>
          <bdi className="ltr num profile-id">{patient.patient_number}</bdi>
          {age != null && <span className="profile-age">{patient.date_of_birth ? "" : "~"}{t("patients.header.age").replace("{n}", digits(age, lang))}</span>}
          {patient.status !== "active" && <Badge dot>{t("patients.status.inactive")}</Badge>}
          {clinical.view && <AlertChip patientId={patient.id} refreshKey={medicalRefreshKey} onOpenHistory={() => go("medical")} />}
          <span className="grow" />
          {canEdit && (
            <>
              <Button size="sm" icon={Pencil} onClick={() => onEdit(patient)} data-testid="patient-edit-open">{t("common.edit")}</Button>
              <Popover
                open={menu}
                onClose={() => setMenu(false)}
                width={240}
                testId="patient-menu"
                trigger={<IconButton icon={Ellipsis} size="sm" label={t("patients.moreActions")} aria-expanded={menu} onClick={() => setMenu((x) => !x)} data-testid="patient-more" />}
              >
                <button type="button" className="menu-item" onClick={() => { setMenu(false); setMerging(true); }} data-testid="patient-merge-open">
                  <Merge aria-hidden /> {t("patients.merge")}
                </button>
                <button type="button" className="menu-item danger" onClick={() => { setMenu(false); remove(); }} data-testid="patient-delete">
                  <Trash2 aria-hidden /> {t("patients.delete")}
                </button>
              </Popover>
            </>
          )}
        </header>

        <div className="profile-tabs" role="tablist">
          {TABS.map(({ id, icon: Icon, labelKey }) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={tab === id}
              disabled={!available(id)}
              title={available(id) ? undefined : t("patients.tab.comingSoon")}
              className="profile-tab"
              onClick={() => available(id) && setTab(id)}
              data-testid={`patient-tab-${id}`}
            >
              <Icon aria-hidden />
              <span>{t(labelKey)}</span>
            </button>
          ))}
          <span className="grow" />
          <Popover
            open={historyMenu}
            onClose={() => setHistoryMenu(false)}
            width={240}
            testId="patient-history-menu"
            trigger={
              <button type="button" className="profile-tab" role="tab" aria-selected={tab === "timeline" || tab === "audit"} aria-expanded={historyMenu} onClick={() => setHistoryMenu((x) => !x)} data-testid="patient-tab-history">
                <History aria-hidden />
                <span>{t("patients.tab.history")}</span>
              </button>
            }
          >
            {clinical.view && (
              <button type="button" className="menu-item" onClick={() => { setHistoryMenu(false); setTab("timeline"); }} data-testid="patient-tab-timeline">
                <History aria-hidden /> {t("patients.tab.timeline")}
              </button>
            )}
            <button type="button" className="menu-item" onClick={() => { setHistoryMenu(false); setTab("audit"); }} data-testid="patient-tab-audit">
              <ScrollText aria-hidden /> {t("patients.tab.audit")}
            </button>
          </Popover>
        </div>
      </div>

      <PresenceBanner others={others} />

      {blockedBy !== null && (
        <Notice tone="warning" testId="patient-delete-blocked">
          {t("patients.delete.blocked").replace("{n}", digits(blockedBy, lang))}{" "}
          <Button variant="link" onClick={() => { setBlockedBy(null); go("appointments"); }} data-testid="patient-delete-show">{t("patients.delete.show")}</Button>
        </Notice>
      )}
      {/* On the chart the header chip carries the allergies, so the chart keeps the whole height. */}
      {clinical.view && !inChart && <MedicalAlertBanner key={`alert-${medicalRefreshKey}`} patientId={patient.id} hideReview={tab === "medical"} onOpenHistory={() => go("medical")} />}

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
      {tab === "overview" && clinical.view && <CourseCard key={`course-${clinicalKey}`} patient={patient} calendar={clinic?.calendar_system ?? "shamsi"} onOpen={() => go("plans")} />}
      {tab === "medical" && (
        <MedicalHistoryTab patientId={patient.id} female={genders.find((g) => g.id === patient.gender_id)?.code === "female"} canEdit={clinical.edit} onSaved={() => setMedicalRefreshKey((k) => k + 1)} />
      )}
      {tab === "appointments" && <PatientAppointmentsTab patient={patient} perms={appointments} clinic={clinic} clinicName={clinicName} />}

      {tab === "care" && (
        <>
          <div className="profile-sub">
            <Segmented<CareSub>
              value={careSub}
              onChange={setCareSub}
              label={t("patients.tab.care")}
              options={[
                { value: "chart", label: t("patients.tab.dentalChart"), testId: "patient-sub-chart" },
                { value: "treatments", label: t("patients.tab.treatments"), testId: "patient-sub-treatments" },
                { value: "plans", label: t("patients.tab.treatmentPlans"), testId: "patient-sub-plans" },
              ]}
            />
          </div>
          {careSub === "chart" && <ChartTab patient={patient} canEdit={clinical.edit} clinic={clinic} onChanged={() => setClinicalKey((k) => k + 1)} />}
          {careSub === "treatments" && (
            <TreatmentsTab patient={patient} clinic={clinic} calendar={clinic?.calendar_system ?? "shamsi"} onOpenChart={() => go("chart")} onNote={(i) => { setNoteFor(i); go("notes"); }} onInvoice={billing.edit ? () => go("invoices") : undefined} canTakeMaterials={clinical.edit} />
          )}
          {careSub === "plans" && (
            <PlansTab patient={patient} clinic={clinic} canEdit={clinical.edit} calendar={clinic?.calendar_system ?? "shamsi"} booking={{ edit: appointments.edit, treat: appointments.treat, createPatients: appointments.createPatients }} clinicName={clinicName} onChanged={() => setClinicalKey((k) => k + 1)} />
          )}
        </>
      )}
      {tab === "notes" && <NotesTab key={noteFor?.id ?? "notes"} patient={patient} clinic={clinic} canEdit={clinical.edit} calendar={clinic?.calendar_system ?? "shamsi"} initialItem={noteFor} onChanged={() => setNoteFor(null)} />}

      {tab === "finance" && (
        <>
          <div className="profile-sub">
            <Segmented<FinanceSub>
              value={financeSub}
              onChange={setFinanceSub}
              label={t("patients.tab.finance")}
              options={[
                { value: "invoices", label: t("patients.tab.invoices"), testId: "patient-sub-invoices" },
                { value: "payments", label: t("patients.tab.payments"), testId: "patient-sub-payments" },
              ]}
            />
          </div>
          {financeSub === "invoices" && <PatientInvoicesTab patient={patient} perms={billing} canOverride={billing.override} clinic={clinic} onChanged={() => setClinicalKey((k) => k + 1)} />}
          {financeSub === "payments" && <PatientPaymentsTab patient={patient} perms={billing} clinic={clinic} onChanged={() => setClinicalKey((k) => k + 1)} />}
        </>
      )}

      {tab === "documents" && (
        <>
          {clinical.view && (
            <div className="profile-sub">
              <Segmented<DocSub>
                value={docSub}
                onChange={setDocSub}
                label={t("patients.tab.docs")}
                options={[
                  { value: "documents", label: t("patients.tab.documents"), testId: "patient-sub-documents" },
                  { value: "prescriptions", label: t("patients.tab.prescriptions"), testId: "patient-sub-prescriptions" },
                ]}
              />
            </div>
          )}
          {clinical.view && docSub === "prescriptions" && <PrescriptionsTab patient={patient} canEdit={clinical.edit} calendar={clinic?.calendar_system ?? "shamsi"} />}
          {clinical.view && docSub === "documents" && (
            <DocumentsTab patient={patient} gender={genders.find((g) => g.id === patient.gender_id)?.code ?? null} canEdit={clinical.edit} canEditFiles={canEdit} calendar={clinic?.calendar_system ?? "shamsi"} />
          )}
          {!clinical.view && <AttachmentsTab patientId={patient.id} canEdit={canEdit} />}
        </>
      )}

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
      {dialog}
    </div>
  );
}
