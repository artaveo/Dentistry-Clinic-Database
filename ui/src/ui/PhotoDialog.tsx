// Set or remove a person's photo (OF-049, OF-050): from a file or from the webcam. The picture is
// cropped to a square and shrunk here (a 12-megapixel phone photo becomes a few KiB) before it is sent;
// the Core checks and shrinks it again and stores it encrypted like any attachment.
import { useEffect, useRef, useState } from "react";
import { Camera, ImageUp, Trash2 } from "lucide-react";
import { useI18n } from "../i18n";
import { isSessionError } from "../lib/api";
import { PHOTO_SIDE, squareCrop } from "../lib/avatar";
import { Button } from "./Button";
import { Notice } from "./Feedback";
import { Dialog } from "./Overlay";
import { PersonAvatar, type PersonAvatarProps } from "./Avatar";

const MAX_FILE = 5 * 1024 * 1024;

/** A square JPEG (base64, no `data:` prefix) of the centre of `source`. */
function squareJpeg(source: CanvasImageSource, width: number, height: number, mirror = false): string {
  const { sx, sy, side } = squareCrop(width, height);
  const size = Math.min(PHOTO_SIDE, side);
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("no canvas");
  ctx.imageSmoothingQuality = "high";
  if (mirror) {
    ctx.translate(size, 0);
    ctx.scale(-1, 1);
  }
  ctx.drawImage(source, sx, sy, side, side, 0, 0, size, size);
  return canvas.toDataURL("image/jpeg", 0.88).split(",")[1];
}

export function PhotoDialog({ title, person, hasPhoto, onSave, onClose, testId = "photo-dialog" }: {
  title: string;
  person: Omit<PersonAvatarProps, "size">;
  hasPhoto: boolean;
  /** `null` removes the photo. Throws on failure (the dialog shows the message). */
  onSave: (base64: string | null) => Promise<void>;
  onClose: () => void;
  testId?: string;
}) {
  const { t, err } = useI18n();
  const [pending, setPending] = useState<string | null>(null);
  const [camera, setCamera] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const fileRef = useRef<HTMLInputElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const canCamera = typeof navigator !== "undefined" && !!navigator.mediaDevices?.getUserMedia;

  const stopCamera = () => {
    streamRef.current?.getTracks().forEach((tr) => tr.stop());
    streamRef.current = null;
    setCamera(false);
  };
  useEffect(() => stopCamera, []);

  const pickFile = async (file: File | undefined) => {
    setError("");
    if (!file) return;
    if (file.size > MAX_FILE) return setError(t("avatar.photo.tooBig"));
    try {
      const bmp = await createImageBitmap(file);
      setPending(squareJpeg(bmp, bmp.width, bmp.height));
      bmp.close();
      stopCamera();
    } catch {
      setError(t("avatar.photo.badFile"));
    }
  };

  const startCamera = async () => {
    setError("");
    setPending(null);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: { width: { ideal: 640 }, height: { ideal: 640 }, facingMode: "user" }, audio: false });
      streamRef.current = stream;
      setCamera(true);
      requestAnimationFrame(() => {
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          void videoRef.current.play().catch(() => {});
        }
      });
    } catch {
      setError(t("avatar.photo.cameraError"));
    }
  };

  const capture = () => {
    const v = videoRef.current;
    if (!v || !v.videoWidth) return;
    // The preview is mirrored like a mirror; the saved photo is mirrored the same way, so it looks as seen.
    setPending(squareJpeg(v, v.videoWidth, v.videoHeight, true));
    stopCamera();
  };

  const save = async (value: string | null) => {
    setBusy(true);
    setError("");
    try {
      await onSave(value);
      onClose();
    } catch (x) {
      if (!isSessionError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={title}
      onClose={() => { stopCamera(); onClose(); }}
      testId={testId}
      footer={
        <>
          {hasPhoto && !pending && <Button variant="subtle" icon={Trash2} loading={busy} onClick={() => save(null)} data-testid="photo-remove">{t("avatar.photo.remove")}</Button>}
          <span className="grow" />
          <Button onClick={() => { stopCamera(); onClose(); }}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={!pending} loading={busy} onClick={() => pending && save(pending)} data-testid="photo-save">{t("common.save")}</Button>
        </>
      }
    >
      <div className="stack">
        {error && <Notice tone="danger" testId="photo-error">{error}</Notice>}
        <div className="photo-preview">
          {camera ? (
            <video ref={videoRef} muted playsInline style={{ transform: "scaleX(-1)" }} data-testid="photo-video" />
          ) : pending ? (
            <span className="person-avatar xl"><img src={`data:image/jpeg;base64,${pending}`} alt={t("avatar.photo.new")} data-testid="photo-pending" /></span>
          ) : (
            <PersonAvatar {...person} size="xl" />
          )}
          <span className="subtle t-caption" style={{ textAlign: "center" }}>{camera ? t("avatar.photo.cameraHint") : hasPhoto || pending ? t("avatar.photo.hint") : t("avatar.photo.defaultHint")}</span>
        </div>
        <div className="row" style={{ justifyContent: "center" }}>
          {camera ? (
            <Button variant="primary" icon={Camera} onClick={capture} data-testid="photo-capture">{t("avatar.photo.capture")}</Button>
          ) : (
            <>
              <Button icon={ImageUp} onClick={() => fileRef.current?.click()} data-testid="photo-choose">{t("avatar.photo.choose")}</Button>
              {canCamera && <Button icon={Camera} onClick={startCamera} data-testid="photo-camera">{t("avatar.photo.camera")}</Button>}
            </>
          )}
        </div>
        <input ref={fileRef} type="file" accept="image/png,image/jpeg,image/webp" hidden onChange={(e) => { void pickFile(e.target.files?.[0]); e.target.value = ""; }} data-testid="photo-file" />
      </div>
    </Dialog>
  );
}
