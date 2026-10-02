import { useEffect, useState } from "react";
import { FileText, Images, Plus, Trash2 } from "lucide-react";
import type { AttachmentInfo, AttachmentKind } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { formatDate } from "../../lib/dates";
import { useI18n } from "../../i18n";
import { Button, IconButton } from "../../ui/Button";
import { Card, CardHeader } from "../../ui/Card";
import { EmptyState, ErrorState, SkeletonRows } from "../../ui/Feedback";
import { useToast } from "../../ui/Toast";
import { UploadAttachmentDialog } from "./UploadAttachmentDialog";
import { AttachmentViewer } from "./AttachmentViewer";

const KIND_LABEL: Record<AttachmentKind, string> = {
  xray: "attachments.kind.xray",
  photo: "attachments.kind.photo",
  document: "attachments.kind.document",
  scan: "attachments.kind.scan",
  consent_form: "attachments.kind.consent_form",
  other: "attachments.kind.other",
};

function isImage(mime: string) {
  return mime.startsWith("image/");
}

export function AttachmentsTab({ patientId, canEdit }: { patientId: string; canEdit: boolean }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const [items, setItems] = useState<AttachmentInfo[] | null>(null);
  const [error, setError] = useState("");
  const [uploading, setUploading] = useState(false);
  const [viewing, setViewing] = useState<AttachmentInfo | null>(null);

  const load = () =>
    rpc("attachments.list", { patient_id: patientId })
      .then((v) => {
        setItems(v);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, [patientId]);

  const remove = async (a: AttachmentInfo) => {
    if (!window.confirm(t("attachments.deleteConfirm"))) return;
    await rpc("attachments.delete", { id: a.id, version: a.version });
    toast.success(t("attachments.deleted"));
    load();
  };

  return (
    <Card flush>
      <CardHeader
        title={t("attachments.title")}
        description={t("attachments.subtitle")}
        actions={canEdit ? <Button variant="primary" icon={Plus} onClick={() => setUploading(true)} data-testid="attachment-add-open">{t("attachments.add")}</Button> : undefined}
      />
      <div className="attachment-grid" data-testid="attachment-grid">
        {!items && <div className="attachment-skeleton"><SkeletonRows cols={1} /></div>}
        {items?.map((a) => (
          <div key={a.id} className="attachment-card" data-testid={`attachment-${a.id}`}>
            <button type="button" className="attachment-thumb" onClick={() => setViewing(a)} data-testid={`attachment-open-${a.id}`}>
              {isImage(a.mime_type) ? <AttachmentImage id={a.id} /> : <FileText aria-hidden />}
            </button>
            <div className="attachment-meta">
              <span className="cell-strong">{t(KIND_LABEL[a.kind])}</span>
              <span className="subtle t-caption">{formatDate(a.captured_at, lang)}{a.tooth ? ` · ${a.tooth}` : ""}</span>
            </div>
            {canEdit && <IconButton icon={Trash2} label={t("attachments.delete")} size="sm" onClick={() => remove(a)} data-testid={`attachment-delete-${a.id}`} />}
          </div>
        ))}
      </div>
      {error && <ErrorState message={error} onRetry={load} />}
      {items?.length === 0 && !error && <EmptyState icon={Images} title={t("attachments.empty")} />}
      {uploading && <UploadAttachmentDialog patientId={patientId} onClose={() => setUploading(false)} onUploaded={() => { setUploading(false); load(); }} />}
      {viewing && <AttachmentViewer attachment={viewing} onClose={() => setViewing(null)} />}
    </Card>
  );
}

/** Lazily fetches and shows a small preview; the browser decodes/scales it (ADR-12 known limitation: no server-side thumbnail yet). */
function AttachmentImage({ id }: { id: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    rpc("attachments.file", { id, thumbnail: true }).then((d) => live && setSrc(d.data_url));
    return () => {
      live = false;
    };
  }, [id]);
  return src ? <img src={src} alt="" /> : <span className="skeleton" style={{ width: "100%", height: "100%" }} />;
}
