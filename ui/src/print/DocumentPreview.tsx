import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { FileDown, FolderOpen, Printer, Settings2 } from "lucide-react";
import type { Paper } from "../../../shared/ts/contract";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Badge, Notice } from "../ui/Feedback";
import { Select } from "../ui/Field";
import { Segmented } from "../ui/Controls";
import { Dialog } from "../ui/Overlay";
import { useToast } from "../ui/Toast";
import { isDesktop, listPrinters, loadProfile, PAPER_MM, printOn, printPage, printerHasPaper, revealFile, savePdf, saveProfile, type PrintProfile } from "./engine";

const PX_PER_MM = 96 / 25.4;

/**
 * The print-only layer (ADR-13): while a document is open for printing, its real-size sheet is also
 * rendered here, outside the app, and `@media print` shows only this layer. `@page` gets the exact page
 * size, so WebView2 (and the browser's print preview) print it at 100 %, never shrunk to fit.
 */
export function PrintLayer({ page, compact, children }: { page: { w: number; h: number }; compact: boolean; children: ReactNode }) {
  const [root] = useState(() => {
    let el = document.getElementById("print-root");
    if (!el) {
      el = document.createElement("div");
      el.id = "print-root";
      document.body.appendChild(el);
    }
    return el;
  });
  useEffect(() => {
    document.body.classList.add("print-doc");
    return () => document.body.classList.remove("print-doc");
  }, []);
  return createPortal(
    <>
      <style>{`@page { size: ${page.w}mm ${page.h}mm; margin: 0; }`}</style>
      <div className={compact ? "print-compact" : "print-plain"} style={{ width: `${page.w}mm` }} data-testid="print-layer">
        {children}
      </div>
    </>,
    root,
  );
}

/** The sheet scaled down to fit the dialog, keeping its real proportions. */
export function Scaled({ paper, children }: { paper: Paper; children: ReactNode }) {
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(560);
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(el.clientWidth));
    ro.observe(el);
    setWidth(el.clientWidth);
    return () => ro.disconnect();
  }, []);
  const { w, h } = PAPER_MM[paper];
  const scale = Math.min(1, (width - 24) / (w * PX_PER_MM));
  return (
    <div className="doc-preview" ref={box} data-testid="doc-preview">
      {/* Scaled from its top-left corner inside a left-to-right frame: an over-wide child overflows by
          its parent's direction, so in an RTL page it would hang off the left edge. The sheet keeps
          its own direction. */}
      <div className="doc-preview-paper" dir="ltr" style={{ width: w * PX_PER_MM * scale, height: h * PX_PER_MM * scale }}>
        <div style={{ transform: `scale(${scale})`, transformOrigin: "top left", width: `${w}mm` }}>{children}</div>
      </div>
    </div>
  );
}

/**
 * Preview, print and PDF for one document (ADR-13 Output Profiles): on this computer's printer
 * (silently), on A4 at real size for a printer without small paper, or only as a PDF of the
 * document's exact size. Every print or PDF is counted by the Core (`onPrinted`), so a reprint is
 * a reprint, never a new document.
 */
export function DocumentPreview({ title, paper, fileStem, sheet, onPrinted, onClose, extra, canPrint = true, testId = "doc-preview-dialog" }: {
  title: ReactNode;
  paper: Paper;
  /** File name of the PDF: the document number. */
  fileStem: string;
  sheet: ReactNode;
  onPrinted?: (pdf: boolean) => Promise<void> | void;
  onClose: () => void;
  /** Extra actions in the side panel (language of the card, attach the signed copy …). */
  extra?: ReactNode;
  canPrint?: boolean;
  testId?: string;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const desktop = isDesktop();
  const [profile, setProfile] = useState<PrintProfile>(loadProfile());
  const [printers, setPrinters] = useState<string[] | null>(null);
  const [fits, setFits] = useState<boolean | null>(null);
  const [busy, setBusy] = useState<"" | "print" | "pdf">("");
  const [savedPath, setSavedPath] = useState<string | null>(null);
  // A PDF is always the document's own size (ADR-13), even when this computer prints small papers on A4.
  const [pdfLayout, setPdfLayout] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    listPrinters().then((l) => {
      setPrinters(l.printers);
      // First time on this computer: the Windows default printer.
      if (profile.printer === null && l.default && !localStorage.getItem("artaveo.print")) update({ ...profile, printer: l.default });
    }).catch(() => setPrinters([]));
  }, []);
  useEffect(() => {
    if (profile.printer && paper !== "a4") printerHasPaper(profile.printer, paper).then(setFits);
    else setFits(null);
  }, [profile.printer, paper]);

  const update = (p: PrintProfile) => {
    setProfile(p);
    saveProfile(p);
  };
  const page = pdfLayout ? { ...PAPER_MM[paper], compact: false } : printPage(paper, profile);

  const print = async () => {
    setError("");
    setBusy("print");
    try {
      await printOn(profile.printer, page);
      await onPrinted?.(false);
      if (desktop) toast.success(t("print.sent"));
    } catch (x) {
      setError(t("print.failed").replace("{detail}", String(x)));
    } finally {
      setBusy("");
    }
  };
  const pdf = async () => {
    setError("");
    setBusy("pdf");
    setPdfLayout(true);
    try {
      // Let React lay the sheet out at its own size before the page is captured (a timer, not
      // animation frames, which a hidden window or a test clock may never deliver).
      await new Promise((r) => setTimeout(r, 60));
      const path = await savePdf(fileStem, paper);
      await onPrinted?.(true);
      if (path) {
        setSavedPath(path);
        toast.success(t("print.pdfSaved"));
      }
    } catch (x) {
      setError(t("print.failed").replace("{detail}", String(x)));
    } finally {
      setPdfLayout(false);
      setBusy("");
    }
  };

  return (
    <Dialog title={title} onClose={onClose} wide testId={testId}>
      <div className="doc-preview-layout">
        <Scaled paper={paper}>{sheet}</Scaled>
        <aside className="doc-preview-side">
          <div className="stack" style={{ gap: "var(--space-3)" }}>
            <div className="row" style={{ justifyContent: "space-between" }}>
              <span className="field-label">{t("print.paper")}</span>
              <Badge tone="accent" testId="doc-paper">{t(`print.paper.${paper}`)}</Badge>
            </div>
            <label className="field">
              <span className="field-label">{t("print.printer")}</span>
              <Select value={profile.printer ?? ""} onChange={(e) => update({ ...profile, printer: e.target.value || null })} disabled={!desktop} data-testid="print-printer">
                <option value="">{t("print.noPrinter")}</option>
                {(printers ?? []).map((p) => <option key={p} value={p}>{p}</option>)}
              </Select>
              {!desktop && <span className="field-hint">{t("print.browserHint")}</span>}
            </label>
            {paper !== "a4" && (
              <div className="field">
                <span className="field-label">{t("print.smallPaper")}</span>
                <Segmented<PrintProfile["smallPaper"]>
                  value={profile.smallPaper}
                  onChange={(v) => update({ ...profile, smallPaper: v })}
                  label={t("print.smallPaper")}
                  block
                  options={[
                    { value: "native", label: t(`print.paper.${paper}`), testId: "print-small-native" },
                    { value: "compact_a4", label: t("print.compactA4"), testId: "print-small-compact" },
                  ]}
                />
                <span className="field-hint">{profile.smallPaper === "compact_a4" ? t("print.compactHint") : t("print.nativeHint")}</span>
                {fits === false && profile.smallPaper === "native" && <Notice tone="warning" testId="print-no-paper">{t("print.noSmallPaper").replace("{paper}", t(`print.paper.${paper}`))}</Notice>}
              </div>
            )}
            {error && <Notice tone="danger" testId="print-error">{error}</Notice>}
            {extra}
          </div>
          <div className="stack" style={{ gap: "var(--space-2)" }}>
            <Button variant="primary" icon={Printer} loading={busy === "print"} disabled={!canPrint || (desktop && !profile.printer)} onClick={print} block data-testid="doc-print">{t("print.print")}</Button>
            <Button icon={FileDown} loading={busy === "pdf"} disabled={!canPrint} onClick={pdf} block data-testid="doc-pdf">{t("print.pdf")}</Button>
            {savedPath && <Button variant="subtle" icon={FolderOpen} onClick={() => revealFile(savedPath)} block data-testid="doc-reveal">{t("print.openFolder")}</Button>}
            {desktop && !profile.printer && <span className="subtle t-caption row"><Settings2 size={14} aria-hidden />{t("print.pdfOnlyHint")}</span>}
          </div>
        </aside>
      </div>
      <PrintLayer page={page} compact={page.compact}>{sheet}</PrintLayer>
    </Dialog>
  );
}
