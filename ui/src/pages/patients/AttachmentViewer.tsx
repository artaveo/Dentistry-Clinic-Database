import { useEffect, useRef, useState } from "react";
import { Download, RotateCcw, ZoomIn, ZoomOut } from "lucide-react";
import type { AttachmentInfo } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { useI18n } from "../../i18n";
import { IconButton } from "../../ui/Button";
import { Dialog } from "../../ui/Overlay";

/** Internal viewer with zoom/pan/brightness/contrast (3.5); non-images get a download link. */
export function AttachmentViewer({ attachment, onClose }: { attachment: AttachmentInfo; onClose: () => void }) {
  const { t } = useI18n();
  const [dataUrl, setDataUrl] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [brightness, setBrightness] = useState(100);
  const [contrast, setContrast] = useState(100);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const dragging = useRef<{ x: number; y: number } | null>(null);
  const isImage = attachment.mime_type.startsWith("image/");

  useEffect(() => {
    rpc("attachments.file", { id: attachment.id, thumbnail: false }).then((d) => setDataUrl(d.data_url));
  }, [attachment.id]);

  const reset = () => {
    setZoom(1);
    setBrightness(100);
    setContrast(100);
    setPan({ x: 0, y: 0 });
  };

  const onPointerDown = (e: React.PointerEvent) => {
    dragging.current = { x: e.clientX - pan.x, y: e.clientY - pan.y };
  };
  const onPointerMove = (e: React.PointerEvent) => {
    if (!dragging.current) return;
    setPan({ x: e.clientX - dragging.current.x, y: e.clientY - dragging.current.y });
  };
  const onPointerUp = () => {
    dragging.current = null;
  };

  return (
    <Dialog title={attachment.file_name} onClose={onClose} testId="attachment-viewer" wide>
      {!isImage ? (
        <div className="attachment-fallback">
          {dataUrl && <a href={dataUrl} download={attachment.file_name} data-testid="attachment-download"><Download aria-hidden /> {t("attachments.viewer.download")}</a>}
        </div>
      ) : (
        <div className="stack">
          <div className="row">
            <IconButton icon={ZoomOut} label={t("attachments.viewer.zoomOut")} onClick={() => setZoom((z) => Math.max(0.25, z - 0.25))} data-testid="viewer-zoom-out" />
            <IconButton icon={ZoomIn} label={t("attachments.viewer.zoomIn")} onClick={() => setZoom((z) => Math.min(5, z + 0.25))} data-testid="viewer-zoom-in" />
            <IconButton icon={RotateCcw} label={t("attachments.viewer.reset")} onClick={reset} data-testid="viewer-reset" />
            <label className="row t-caption">{t("attachments.viewer.brightness")}
              <input type="range" min={50} max={150} value={brightness} onChange={(e) => setBrightness(Number(e.target.value))} data-testid="viewer-brightness" />
            </label>
            <label className="row t-caption">{t("attachments.viewer.contrast")}
              <input type="range" min={50} max={150} value={contrast} onChange={(e) => setContrast(Number(e.target.value))} data-testid="viewer-contrast" />
            </label>
          </div>
          <div className="attachment-viewer-stage" onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerLeave={onPointerUp}>
            {dataUrl && (
              <img
                src={dataUrl}
                alt=""
                draggable={false}
                style={{ transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`, filter: `brightness(${brightness}%) contrast(${contrast}%)` }}
                data-testid="viewer-image"
              />
            )}
          </div>
        </div>
      )}
    </Dialog>
  );
}
