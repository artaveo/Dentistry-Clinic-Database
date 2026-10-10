import { useRef, useState } from "react";
import { Upload } from "lucide-react";
import type { AttachmentInfo, AttachmentKind } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Field, Select, Textarea, TextInput } from "../../ui/Field";
import { Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { useToast } from "../../ui/Toast";
import { readAttachmentFile } from "./readAttachmentFile";

const KINDS: AttachmentKind[] = ["xray", "photo", "document", "scan", "consent_form", "other"];

export function UploadAttachmentDialog({ patientId, defaultKind = "xray", onClose, onUploaded }: { patientId: string; defaultKind?: AttachmentKind; onClose: () => void; onUploaded: (a: AttachmentInfo) => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const fileRef = useRef<HTMLInputElement>(null);
  const [kind, setKind] = useState<AttachmentKind>(defaultKind);
  const [tooth, setTooth] = useState("");
  const [description, setDescription] = useState("");
  const [fileName, setFileName] = useState<string | null>(null);
  const [base64, setBase64] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const pick = async (file: File | undefined) => {
    if (!file) return;
    setError("");
    try {
      const r = await readAttachmentFile(file);
      setBase64(r.base64);
      setFileName(r.name);
    } catch (x) {
      setError(t((x as Error).message));
    }
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!base64 || !fileName) {
      setError(t("rule.required"));
      return;
    }
    setBusy(true);
    setError("");
    try {
      const a = await rpc("attachments.upload", { patient_id: patientId, kind, file_name: fileName, data_base64: base64, tooth: tooth.trim() || null, description: description.trim() || null, captured_at: null });
      toast.success(t("attachments.uploaded"));
      onUploaded(a);
    } catch (x) {
      setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("attachments.add")}
      onClose={onClose}
      testId="upload-attachment-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={Upload} loading={busy} type="submit" form="upload-attachment-form" data-testid="attachment-upload-submit">{t("attachments.add")}</Button>
        </>
      }
    >
      <form id="upload-attachment-form" className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("attachments.field.file")}>
          <input ref={fileRef} type="file" onChange={(e) => pick(e.target.files?.[0])} data-testid="attachment-file-input" />
        </Field>
        <Field label={t("attachments.field.kind")}>
          <Select value={kind} onChange={(e) => setKind(e.target.value as AttachmentKind)} data-testid="attachment-kind">
            {KINDS.map((k) => <option key={k} value={k}>{t(`attachments.kind.${k}`)}</option>)}
          </Select>
        </Field>
        <Field label={t("attachments.field.tooth")} optional>
          <TextInput dir="ltr" value={tooth} onChange={(e) => setTooth(e.target.value)} data-testid="attachment-tooth" />
        </Field>
        <Field label={t("attachments.field.description")} optional>
          <Textarea value={description} onChange={(e) => setDescription(e.target.value)} data-testid="attachment-description" />
        </Field>
      </form>
    </Dialog>
  );
}
