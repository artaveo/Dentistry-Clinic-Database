// The print engine's screen side (ADR-13, Phase 5A). A document is laid out once, at its real size in
// millimetres (DocumentSheet); the same layout is shown scaled in the preview, printed silently on a
// chosen printer through WebView2, or written as a PDF of exactly the document's size. Which printer
// and how small papers are put on it are per-computer choices (a clinic's computers have different
// printers, and in LAN mode each prints on its own), so they live with this computer, not the clinic.
import type { Paper } from "../../../shared/ts/contract";

/** Real paper sizes in millimetres (portrait). */
export const PAPER_MM: Record<Paper, { w: number; h: number }> = {
  a4: { w: 210, h: 297 },
  a5: { w: 148, h: 210 },
  a6: { w: 105, h: 148 },
};

/** Inner margin of the document's own layout, by paper (the printer adds none: WebView2 margins are 0). */
export const MARGIN_MM: Record<Paper, number> = { a4: 16, a5: 11, a6: 7 };

/**
 * How documents leave this computer:
 * - `printer`: the printer to print on silently, or null = no printer here, PDF only.
 * - `smallPaper`: A5/A6 documents on their own small paper (`native`, the tray is the driver's) or
 *   at real size on the top of an A4 sheet with a cut line (`compact_a4`), for printers without small paper.
 */
export type PrintProfile = { printer: string | null; smallPaper: "native" | "compact_a4" };

const PROFILE_KEY = "artaveo.print";

export function loadProfile(): PrintProfile {
  try {
    const v = JSON.parse(localStorage.getItem(PROFILE_KEY) ?? "null");
    if (v && (typeof v.printer === "string" || v.printer === null) && (v.smallPaper === "native" || v.smallPaper === "compact_a4")) return v;
  } catch {
    /* fall through */
  }
  return { printer: null, smallPaper: "native" };
}

export function saveProfile(p: PrintProfile) {
  try {
    localStorage.setItem(PROFILE_KEY, JSON.stringify(p));
  } catch {
    /* a per-device convenience; printing still works with the defaults */
  }
}

/** True inside the desktop app (WebView2); false in a browser (development, E2E). */
export function isDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window && import.meta.env.VITE_MOCK !== "1";
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export type PrinterList = { printers: string[]; default: string | null };
export type PaperInfo = { name: string; width_mm: number; height_mm: number };

export async function listPrinters(): Promise<PrinterList> {
  if (!isDesktop()) return { printers: [], default: null };
  return invoke<PrinterList>("list_printers");
}

/** Whether the printer's driver offers a paper of about this size (±3 mm). */
export async function printerHasPaper(printer: string, paper: Paper): Promise<boolean | null> {
  if (!isDesktop()) return null;
  try {
    const papers = await invoke<PaperInfo[]>("printer_papers", { printer });
    const { w, h } = PAPER_MM[paper];
    return papers.some((p) => (Math.abs(p.width_mm - w) <= 3 && Math.abs(p.height_mm - h) <= 3) || (Math.abs(p.width_mm - h) <= 3 && Math.abs(p.height_mm - w) <= 3));
  } catch {
    return null;
  }
}

/** The page the printer gets: the paper itself, or A4 for a small document printed compactly. */
export function printPage(paper: Paper, profile: PrintProfile): { w: number; h: number; compact: boolean } {
  const compact = paper !== "a4" && profile.smallPaper === "compact_a4";
  return compact ? { ...PAPER_MM.a4, compact } : { ...PAPER_MM[paper], compact };
}

/** Prints the print layer silently on `printer` (desktop) or opens the browser's print dialog. */
export async function printOn(printer: string | null, page: { w: number; h: number }): Promise<void> {
  if (isDesktop() && printer) {
    await invoke("print_page", { printer, page: { widthMm: page.w, heightMm: page.h } });
    return;
  }
  window.print();
}

/** Writes the print layer as a PDF of exactly `paper` into the documents folder (desktop); returns its path. */
export async function savePdf(fileStem: string, paper: Paper): Promise<string | null> {
  if (!isDesktop()) {
    // In a browser the print dialog offers "Save as PDF" (the page size is the document's, via @page).
    window.print();
    return null;
  }
  const { w, h } = PAPER_MM[paper];
  return invoke<string>("save_pdf", { fileStem, page: { widthMm: w, heightMm: h } });
}

export async function revealFile(path: string): Promise<void> {
  if (!isDesktop()) return;
  await invoke("reveal_export", { path });
}
