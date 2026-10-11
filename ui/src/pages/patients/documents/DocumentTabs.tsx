import { useState } from "react";
import type { LucideIcon } from "lucide-react";
import { ClipboardCheck, FilePlus2, FileSignature, FileText, FlaskConical, NotebookText, Pill, Plus, ScanLine, Send, Stethoscope } from "lucide-react";
import type { CalendarSystem, DocumentInfo, DocumentKind, PatientInfo } from "../../../../../shared/ts/contract";
import { useI18n } from "../../../i18n";
import { Button } from "../../../ui/Button";
import { Card, CardHeader } from "../../../ui/Card";
import { Popover } from "../../../ui/Overlay";
import { useToast } from "../../../ui/Toast";
import { useLetterhead } from "../../../print/letterhead";
import { AttachmentsTab } from "../AttachmentsTab";
import { DocumentEditor, OTHER_KINDS } from "./DocumentEditor";
import { DocumentList } from "./DocumentList";
import { PrescriptionEditor } from "./PrescriptionEditor";

/** Patient profile → «نسخه‌ها» (M6): every prescription, reprint without a new number, and a new one. */
export function PrescriptionsTab({ patient, canEdit, calendar }: { patient: PatientInfo; canEdit: boolean; calendar: CalendarSystem }) {
  const { t } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const [editing, setEditing] = useState(false);
  const [issued, setIssued] = useState<DocumentInfo | null>(null);
  const [refresh, setRefresh] = useState(0);
  const action = canEdit && letterhead ? <Button variant="primary" icon={Plus} onClick={() => setEditing(true)} data-testid="rx-new">{t("rx.new")}</Button> : undefined;
  return (
    <Card flush>
      <CardHeader icon={Pill} title={t("patients.tab.prescriptions")} description={t("rx.subtitle")} actions={action} />
      <DocumentList patientId={patient.id} kinds={["prescription"]} canEdit={canEdit} calendar={calendar} refreshKey={refresh} emptyIcon={Pill} emptyTitle={t("rx.none")} emptyHint={t("rx.noneHint")} emptyAction={action && <Button variant="primary" icon={Plus} onClick={() => setEditing(true)}>{t("rx.new")}</Button>} open={issued} onOpened={() => setIssued(null)} />
      {editing && letterhead && (
        <PrescriptionEditor
          patient={patient}
          letterhead={letterhead}
          calendar={calendar}
          onClose={() => setEditing(false)}
          onIssued={(d) => {
            setEditing(false);
            toast.success(t("rx.issued").replace("{number}", d.number));
            setRefresh((x) => x + 1);
            setIssued(d);
          }}
        />
      )}
    </Card>
  );
}

const KIND_ICON: Record<DocumentKind, LucideIcon> = {
  prescription: Pill,
  consent: FileSignature,
  post_op: ClipboardCheck,
  referral: Send,
  imaging_request: ScanLine,
  certificate: Stethoscope,
  lab_order: FlaskConical,
  record_summary: NotebookText,
};

/**
 * Patient profile → «اسناد» (M6): clinical documents written here (consent, after-treatment sheet,
 * referral, imaging request, certificate, lab order, record summary) and, below, the patient's files
 * and images (3.5) — where the signed copy of a consent form is kept.
 */
export function DocumentsTab({ patient, gender, canEdit, canEditFiles, calendar }: { patient: PatientInfo; gender: string | null; canEdit: boolean; canEditFiles: boolean; calendar: CalendarSystem }) {
  const { t } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const [menu, setMenu] = useState(false);
  const [kind, setKind] = useState<DocumentKind | null>(null);
  const [issued, setIssued] = useState<DocumentInfo | null>(null);
  const [refresh, setRefresh] = useState(0);
  const newMenu = canEdit && letterhead ? (
    <Popover
      open={menu}
      onClose={() => setMenu(false)}
      align="end"
      width={280}
      testId="doc-new-menu"
      trigger={<Button variant="primary" icon={FilePlus2} onClick={() => setMenu((m) => !m)} aria-expanded={menu} data-testid="doc-new">{t("docs.newDocument")}</Button>}
    >
      <div className="menu" role="menu">
        {OTHER_KINDS.map((k) => {
          const Icon = KIND_ICON[k];
          return (
            <button key={k} type="button" role="menuitem" className="menu-item" onClick={() => { setMenu(false); setKind(k); }} data-testid={`doc-new-${k}`}>
              <Icon aria-hidden />
              <span>{t(`docs.kind.${k}`)}</span>
            </button>
          );
        })}
      </div>
    </Popover>
  ) : undefined;
  return (
    <div className="stack-lg">
      <Card flush>
        <CardHeader icon={FileText} title={t("docs.clinicalTitle")} description={t("docs.clinicalHint")} actions={newMenu} />
        <DocumentList patientId={patient.id} kinds={OTHER_KINDS} canEdit={canEdit} calendar={calendar} refreshKey={refresh} emptyIcon={FileText} emptyTitle={t("docs.none")} emptyHint={t("docs.noneHint")} open={issued} onOpened={() => setIssued(null)} />
      </Card>
      <AttachmentsTab patientId={patient.id} canEdit={canEditFiles} />
      {kind && letterhead && (
        <DocumentEditor
          kind={kind}
          patient={patient}
          gender={gender}
          letterhead={letterhead}
          calendar={calendar}
          onClose={() => setKind(null)}
          onIssued={(d) => {
            setKind(null);
            toast.success(t("docs.issued").replace("{number}", d.number));
            setRefresh((x) => x + 1);
            setIssued(d);
          }}
        />
      )}
    </div>
  );
}
