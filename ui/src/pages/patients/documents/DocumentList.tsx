import { useEffect, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { Ban, Eye, FileSignature, Paperclip, Printer } from "lucide-react";
import type { CalendarSystem, DocumentInfo, DocumentKind } from "../../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../../lib/api";
import { digits, formatDate } from "../../../lib/dates";
import { useI18n } from "../../../i18n";
import { Button, IconButton } from "../../../ui/Button";
import { Field, Textarea } from "../../../ui/Field";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../../../ui/Feedback";
import { Dialog } from "../../../ui/Overlay";
import { useToast } from "../../../ui/Toast";
import { DocumentPreview } from "../../../print/DocumentPreview";
import { DocumentSheet } from "../../../print/DocumentSheet";
import { useLetterhead } from "../../../print/letterhead";
import { UploadAttachmentDialog } from "../UploadAttachmentDialog";

/** A patient's printed documents of some kinds, newest first, with reprint, void and the signed copy. */
export function DocumentList({ patientId, kinds, canEdit, calendar, refreshKey, emptyIcon, emptyTitle, emptyHint, emptyAction, open, onOpened }: {
  patientId: string;
  kinds: DocumentKind[];
  canEdit: boolean;
  calendar: CalendarSystem;
  refreshKey: number;
  emptyIcon: LucideIcon;
  emptyTitle: string;
  emptyHint?: string;
  emptyAction?: React.ReactNode;
  /** A document to show straight away (the one just issued). */
  open?: DocumentInfo | null;
  onOpened?: () => void;
}) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const [items, setItems] = useState<DocumentInfo[] | null>(null);
  const [error, setError] = useState("");
  const [viewing, setViewing] = useState<DocumentInfo | null>(null);
  const [voiding, setVoiding] = useState<DocumentInfo | null>(null);
  const [attaching, setAttaching] = useState<DocumentInfo | null>(null);

  const load = () =>
    rpc("documents.list", { patient_id: patientId, kind: null })
      .then((v) => {
        setItems(v.filter((d) => kinds.includes(d.kind)));
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patientId, refreshKey]);
  useEffect(() => {
    if (open) {
      setViewing(open);
      onOpened?.();
    }
  }, [open]);

  const printed = async (d: DocumentInfo, pdf: boolean) => {
    const after = await rpc("documents.mark_printed", { id: d.id, pdf });
    // Closed meanwhile? Stay closed — only refresh a preview that is still open on this document.
    setViewing((v) => (v && v.id === after.id ? after : v));
    load();
  };

  if (error && !items) return <ErrorState message={error} onRetry={load} />;
  return (
    <>
      <div className="table-wrap">
        <table className="table" data-testid={`documents-${kinds.join("-")}`}>
          <thead>
            <tr>
              <th>{t("docs.number")}</th>
              <th>{t("docs.kind")}</th>
              <th>{t("docs.date")}</th>
              <th>{t("docs.doctor")}</th>
              <th>{t("docs.printed")}</th>
              <th className="cell-actions"><span className="visually-hidden">{t("docs.actions")}</span></th>
            </tr>
          </thead>
          <tbody>
            {!items && <SkeletonRows cols={6} />}
            {items?.map((d) => (
              <tr key={d.id} data-testid={`doc-row-${d.number}`} className={d.status === "void" ? "row-void" : undefined}>
                <td><bdi className="ltr num cell-strong">{d.number}</bdi></td>
                <td>
                  <span className="cell-strong">{d.content.kind === "consent" || d.content.kind === "post_op" ? d.content.title : t(`docs.kind.${d.kind}`)}</span>
                  {d.kind === "prescription" && d.content.kind === "prescription" && <span className="subtle t-caption"><br /><bdi className="ltr">{d.content.items.map((i) => i.name).join("، ")}</bdi></span>}
                </td>
                <td>{formatDate(d.issued_at, lang, calendar)}</td>
                <td>{d.doctor?.name ?? "—"}</td>
                <td>
                  {d.status === "void" ? (
                    <Badge tone="danger" dot testId={`doc-void-${d.number}`}>{t("docs.void")}</Badge>
                  ) : d.print_count > 0 ? (
                    <span className="subtle">{t("docs.printedTimes").replace("{n}", digits(d.print_count, lang))}</span>
                  ) : (
                    <Badge tone="warning" dot>{t("docs.notPrinted")}</Badge>
                  )}
                  {d.attachment_id && <span className="subtle t-caption row" style={{ gap: 4 }}><Paperclip size={13} aria-hidden />{t("docs.signedAttached")}</span>}
                </td>
                <td className="cell-actions">
                  <div className="row" style={{ justifyContent: "flex-end", flexWrap: "nowrap" }}>
                    <Button size="sm" icon={d.status === "void" ? Eye : Printer} onClick={() => setViewing(d)} data-testid={`doc-open-${d.number}`}>{d.status === "void" ? t("docs.view") : t("docs.print")}</Button>
                    {canEdit && d.status !== "void" && d.kind === "consent" && <IconButton icon={FileSignature} size="sm" label={t("docs.attachSigned")} onClick={() => setAttaching(d)} data-testid={`doc-attach-${d.number}`} />}
                    {canEdit && d.status !== "void" && <IconButton icon={Ban} size="sm" label={t("docs.voidAction")} onClick={() => setVoiding(d)} data-testid={`doc-void-open-${d.number}`} />}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {items?.length === 0 && <EmptyState icon={emptyIcon} title={emptyTitle} action={emptyAction}>{emptyHint}</EmptyState>}
      </div>

      {viewing && letterhead && (
        <DocumentPreview
          title={`${viewing.content.kind === "consent" || viewing.content.kind === "post_op" ? viewing.content.title : t(`docs.kind.${viewing.kind}`)} · ${viewing.number}`}
          paper={viewing.paper}
          fileStem={viewing.number}
          canPrint={viewing.status !== "void"}
          sheet={<DocumentSheet doc={viewing} letterhead={letterhead} calendar={calendar} />}
          onPrinted={(pdf) => printed(viewing, pdf)}
          onClose={() => setViewing(null)}
          extra={viewing.status === "void" ? <Notice tone="danger" testId="doc-void-notice">{t("docs.voidNotice").replace("{reason}", viewing.void_reason ?? "")}</Notice> : viewing.print_count > 0 ? <span className="subtle t-caption">{t("docs.reprintHint")}</span> : undefined}
        />
      )}
      {voiding && <VoidDialog doc={voiding} onClose={() => setVoiding(null)} onDone={() => { setVoiding(null); toast.success(t("docs.voided")); load(); }} />}
      {attaching && (
        <UploadAttachmentDialog
          patientId={patientId}
          defaultKind="consent_form"
          onClose={() => setAttaching(null)}
          onUploaded={async (a) => {
            try {
              await rpc("documents.attach_scan", { id: attaching.id, version: attaching.version, attachment_id: a.id });
              toast.success(t("docs.signedSaved"));
            } catch (x) {
              if (!isSessionError(x)) toast.error(err(x));
            }
            setAttaching(null);
            load();
          }}
        />
      )}
    </>
  );
}

function VoidDialog({ doc, onClose, onDone }: { doc: DocumentInfo; onClose: () => void; onDone: () => void }) {
  const { t, err } = useI18n();
  const [reason, setReason] = useState("");
  const [touched, setTouched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const invalid = !reason.trim() ? t("rule.required") : reason.trim().length > 300 ? t("rule.document_text") : null;
  const submit = async () => {
    setTouched(true);
    if (invalid) return;
    setBusy(true);
    try {
      await rpc("documents.void", { id: doc.id, version: doc.version, reason: reason.trim() });
      onDone();
    } catch (x) {
      if (!isSessionError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t("docs.voidTitle").replace("{number}", doc.number)}
      description={t("docs.voidHint")}
      onClose={onClose}
      testId="doc-void-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="danger" icon={Ban} loading={busy} onClick={submit} data-testid="doc-void-confirm">{t("docs.voidAction")}</Button></>}
    >
      {error && <Notice tone="danger">{error}</Notice>}
      <Field label={t("docs.voidReason")} error={touched ? invalid : null}>
        <Textarea rows={3} value={reason} onChange={(e) => { setReason(e.target.value); setTouched(true); }} data-testid="doc-void-reason" />
      </Field>
    </Dialog>
  );
}
