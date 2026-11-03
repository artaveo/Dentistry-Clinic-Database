// The compact receipt (Phase 6, 6.9–6.10): a document with its own small real size — thermal 58 or 80 mm, A6,
// A5 or a size of the clinic's choice — whatever printer the clinic has, or none. The same sheet is shown
// scaled in the preview, printed on the roll, on a small paper, once or several times on an A4 with cut
// marks, and written as a PDF of its own size (never an A4 with a small receipt in the middle).
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { FileDown, FolderOpen, Printer, Settings2 } from "lucide-react";
import type { CalendarSystem, Language, PaymentInfo } from "../../../shared/ts/contract";
import { inCalendar } from "../lib/calendar";
import { formatDateTime } from "../lib/dates";
import { tr } from "../lib/medical";
import { formatAmount } from "../lib/money";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Segmented } from "../ui/Controls";
import { Field, Select, TextInput } from "../ui/Field";
import { Badge, Notice } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { useToast } from "../ui/Toast";
import { DentalMark } from "../ui/Brand";
import { PrintLayer } from "./DocumentPreview";
import { docT } from "./docText";
import { isDesktop, listPrinters, printOn, revealFile, savePdfAt } from "./engine";
import { useLetterhead } from "./letterhead";
import type { Letterhead } from "./letterhead";

const PX_PER_MM = 96 / 25.4;
const A4 = { w: 210, h: 297 };

/** Where a receipt goes (6.9 Output Profiles). */
export type ReceiptMode = "thermal58" | "thermal80" | "a6" | "a5" | "custom" | "a4_one" | "a4_two" | "a4_four" | "pdf";

export type ReceiptProfile = {
  mode: ReceiptMode;
  /** null = no printer on this computer: PDF only. */
  printer: string | null;
  /** Custom compact size, millimetres. */
  customWidth: number;
  customHeight: number;
  /** Calibration for printers that print a little off (mm). */
  offsetX: number;
  offsetY: number;
};

const KEY = "artaveo.print.receipt";
export const DEFAULT_RECEIPT_PROFILE: ReceiptProfile = { mode: "pdf", printer: null, customWidth: 80, customHeight: 120, offsetX: 0, offsetY: 0 };
const MODES: ReceiptMode[] = ["thermal58", "thermal80", "a6", "a5", "custom", "a4_one", "a4_two", "a4_four", "pdf"];

export function loadReceiptProfile(): ReceiptProfile {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (v && MODES.includes(v.mode)) return { ...DEFAULT_RECEIPT_PROFILE, ...v };
  } catch {
    /* fall through */
  }
  return DEFAULT_RECEIPT_PROFILE;
}

export function saveReceiptProfile(p: ReceiptProfile) {
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    /* a per-computer convenience; printing still works with the defaults */
  }
}

/** Has this computer chosen how it prints receipts yet? */
export function hasReceiptProfile(): boolean {
  try {
    return localStorage.getItem(KEY) !== null;
  } catch {
    return false;
  }
}

/** The receipt's own width in millimetres, and the page it is printed on. */
function layoutOf(p: ReceiptProfile): { sheetW: number; copies: number; roll: boolean } {
  switch (p.mode) {
    case "thermal58":
      return { sheetW: 58, copies: 1, roll: true };
    case "thermal80":
      return { sheetW: 80, copies: 1, roll: true };
    case "a6":
      return { sheetW: 105, copies: 1, roll: false };
    case "a5":
    case "a4_one":
    case "pdf":
      return { sheetW: 148, copies: 1, roll: false };
    case "a4_two":
      return { sheetW: 148, copies: 2, roll: false };
    case "a4_four":
      return { sheetW: 105, copies: 4, roll: false };
    case "custom":
      return { sheetW: Math.min(Math.max(p.customWidth, 40), 210), copies: 1, roll: false };
  }
}

/** Receipts are filed by Shamsi year and month: `Receipts/1405/07` (6.9). */
export function receiptFolder(localDate: string): string {
  const d = inCalendar(localDate, "shamsi");
  return `Receipts/${d.year}/${String(d.month).padStart(2, "0")}`;
}

/** The receipt at its real width; its height follows its content. */
export function ReceiptSheet({ payment, letterhead, lang, calendar, widthMm, footer, copy }: { payment: PaymentInfo; letterhead: Letterhead; lang: Language; calendar: CalendarSystem; widthMm: number; footer: string; copy: boolean }) {
  const t = docT(lang);
  const r = payment.receipt;
  const c = letterhead.clinic;
  const money = (v: number) => formatAmount(v, lang);
  const small = widthMm < 70;
  const title = payment.kind === "refund" ? t("rcpt.title.refund") : payment.kind === "credit" ? t("rcpt.title.credit") : payment.invoice_id ? t("rcpt.title.payment") : t("rcpt.title.advance");
  return (
    <article className={`rcpt ${small ? "rcpt-narrow" : ""}`} dir={lang === "en" ? "ltr" : "rtl"} lang={lang} style={{ width: `${widthMm}mm`, ["--doc-accent" as string]: c.color_primary }} data-testid={`receipt-${payment.number}`} data-width={widthMm}>
      <header className="rcpt-head">
        {letterhead.logo ? <img className="rcpt-logo" src={letterhead.logo} alt="" /> : <span className="rcpt-monogram">{[...c.name.trim()][0] ?? "؟"}</span>}
        <div className="rcpt-clinic">{c.name}</div>
        {(c.address || c.phone) && <div className="rcpt-meta">{c.address}{c.address && c.phone ? " · " : ""}{c.phone && <bdi className="ltr">{c.phone}</bdi>}</div>}
      </header>
      <div className="rcpt-title">{title}</div>
      {copy && <div className="rcpt-copy" data-testid="receipt-copy">{t("rcpt.copy")}</div>}
      <dl className="rcpt-facts">
        <div><dt>{t("rcpt.number")}</dt><dd><bdi className="ltr">{payment.number}</bdi></dd></div>
        <div><dt>{t("rcpt.date")}</dt><dd>{formatDateTime(payment.paid_at, lang, calendar)}</dd></div>
        <div><dt>{t("rcpt.patient")}</dt><dd><strong>{r.patient_name}</strong> <bdi className="ltr">{r.patient_number}</bdi></dd></div>
        {r.invoice_number && <div><dt>{t("rcpt.invoice")}</dt><dd><bdi className="ltr">{r.invoice_number}</bdi></dd></div>}
      </dl>
      {r.lines.length > 0 && (
        <ul className="rcpt-lines">
          {r.lines.map((l, i) => (
            <li key={i}><span>{tr(l.names, lang)}</span><span className="rcpt-amt">{money(l.amount)}</span></li>
          ))}
        </ul>
      )}
      <div className="rcpt-totals">
        {r.invoice_number && <div><span>{t("rcpt.invoiceTotal")}</span><span className="rcpt-amt">{money(r.invoice_net)}</span></div>}
        <div className="rcpt-main"><span>{payment.kind === "refund" ? t("rcpt.refunded") : t("rcpt.paid")}</span><span className="rcpt-amt">{money(payment.amount)}</span></div>
        {payment.method && <div><span>{t("rcpt.method")}</span><span>{tr(payment.method, lang)}</span></div>}
        {r.invoice_number && <div><span>{t("rcpt.paidSoFar")}</span><span className="rcpt-amt">{money(r.paid_after)}</span></div>}
        {r.invoice_number && <div className="rcpt-balance"><span>{t("rcpt.balance")}</span><span className="rcpt-amt">{money(r.remaining_after)}</span></div>}
        {r.credit_after > 0 && <div><span>{t("rcpt.credit")}</span><span className="rcpt-amt">{money(r.credit_after)}</span></div>}
      </div>
      {payment.reason && <div className="rcpt-note">{t("rcpt.reason")}: {payment.reason}</div>}
      {payment.note && payment.kind !== "refund" && <div className="rcpt-note">{payment.note}</div>}
      <footer className="rcpt-foot">
        {footer && <div>{footer}</div>}
        {payment.created_by_name && <div className="rcpt-meta">{t("rcpt.receivedBy")}: {payment.created_by_name}</div>}
        {letterhead.brandFooter && <div className="rcpt-brand"><DentalMark size={8} /> Artaveo Dental</div>}
      </footer>
    </article>
  );
}

/** The sheet(s) on the page this mode prints: one roll, one small paper, or several on an A4 with cut marks. */
function ReceiptPage({ mode, copies, sheetW, children }: { mode: ReceiptMode; copies: number; sheetW: number; children: ReactNode }) {
  if (copies === 1) return <>{children}</>;
  const cols = copies === 4 ? 2 : 1;
  const cell = { width: `${A4.w / cols}mm`, height: `${A4.h / (copies / cols)}mm` };
  return (
    <div className="rcpt-sheetgrid" data-mode={mode} style={{ width: `${A4.w}mm`, height: `${A4.h}mm`, gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
      {Array.from({ length: copies }, (_, i) => (
        <div key={i} className="rcpt-cell" style={{ ...cell, justifyContent: "center" }}>
          <div style={{ width: `${sheetW}mm` }}>{children}</div>
        </div>
      ))}
    </div>
  );
}

/** The page that tests a printer: a frame at the receipt's width and a ruler, to see where it lands (6.9 calibration). */
function TestPage({ profile }: { profile: ReceiptProfile }) {
  const { t } = useI18n();
  const { sheetW, roll } = layoutOf(profile);
  return (
    <div className="rcpt-testpage" style={{ width: `${roll ? sheetW : A4.w}mm`, transform: `translate(${profile.offsetX}mm, ${profile.offsetY}mm)` }}>
      <div className="rcpt-test-frame"><span>Artaveo Dental · {t(`rcpt.mode.${profile.mode}`)}</span><span>{sheetW} mm</span></div>
      <div className="rcpt-test-ruler">{Array.from({ length: 11 }, (_, i) => <span key={i}>{i * 10}</span>)}</div>
    </div>
  );
}

/** Prints the test page; `layer` is the print layer to render while it runs (instead of the receipt's own). */
export function useTestPrint(profile: ReceiptProfile) {
  const [testing, setTesting] = useState(false);
  const { roll, sheetW } = layoutOf(profile);
  const page = roll ? { w: sheetW, h: 80 } : A4;
  const run = async () => {
    setTesting(true);
    try {
      await new Promise((r) => setTimeout(r, 80));
      await printOn(profile.printer, page);
    } finally {
      setTesting(false);
    }
  };
  const layer = testing ? <PrintLayer page={page} compact={false}><TestPage profile={profile} /></PrintLayer> : null;
  return { testing, run, layer };
}

/** What gets sent to the printer or saved: the page size of the mode, its offsets, and its sheets. */
function useReceiptPage(profile: ReceiptProfile, heightMm: number) {
  const { sheetW, copies, roll } = layoutOf(profile);
  let page: { w: number; h: number };
  switch (profile.mode) {
    case "thermal58":
    case "thermal80":
      page = { w: sheetW, h: Math.max(30, Math.ceil(heightMm) + 4) };
      break;
    case "a6":
      page = { w: 105, h: 148 };
      break;
    case "a5":
      page = { w: 148, h: 210 };
      break;
    case "custom":
      page = { w: sheetW, h: Math.max(40, profile.customHeight) };
      break;
    case "a4_one":
    case "a4_two":
    case "a4_four":
      page = A4;
      break;
    case "pdf":
      page = { w: 148, h: Math.max(60, Math.ceil(heightMm) + 8) };
      break;
  }
  return { page, sheetW, copies, roll };
}

/**
 * Preview, print and PDF of one receipt or voucher. Every print or PDF is counted by the Core
 * (`payments.mark_printed`): a reprint is the same receipt marked «کپی»; it never records a payment.
 */
export function ReceiptPreview({ payment, calendar, footer, onPrinted, onClose }: { payment: PaymentInfo; calendar: CalendarSystem; footer: string; onPrinted: (pdf: boolean) => Promise<PaymentInfo | void>; onClose: () => void }) {
  const { t, lang: uiLang } = useI18n();
  const toast = useToast();
  const letterhead = useLetterhead();
  const desktop = isDesktop();
  const [profile, setProfile] = useState<ReceiptProfile>(loadReceiptProfile());
  const [lang, setLang] = useState<Language>(uiLang);
  const [printers, setPrinters] = useState<string[] | null>(null);
  const [setup, setSetup] = useState(!hasReceiptProfile());
  const [busy, setBusy] = useState<"" | "print" | "pdf">("");
  const [error, setError] = useState("");
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const [current, setCurrent] = useState(payment);
  const measure = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState(120);
  const copy = current.print_count > 0;
  const test = useTestPrint(profile);

  const update = (p: ReceiptProfile) => {
    setProfile(p);
    saveReceiptProfile(p);
  };
  useEffect(() => {
    listPrinters().then((l) => setPrinters(l.printers)).catch(() => setPrinters([]));
  }, []);
  const { page, sheetW, copies } = useReceiptPage(profile, height);
  useLayoutEffect(() => {
    const el = measure.current;
    if (el) setHeight(el.offsetHeight / PX_PER_MM);
  }, [letterhead, lang, sheetW, current.print_count, footer]);

  if (!letterhead) return null;
  const sheet = <ReceiptSheet payment={current} letterhead={letterhead} lang={lang} calendar={calendar} widthMm={sheetW} footer={footer} copy={copy} />;
  const placed = (
    <div style={{ transform: profile.offsetX || profile.offsetY ? `translate(${profile.offsetX}mm, ${profile.offsetY}mm)` : undefined }}>
      <ReceiptPage mode={profile.mode} copies={copies} sheetW={sheetW}>{sheet}</ReceiptPage>
    </div>
  );
  const canPrint = profile.mode !== "pdf" && (!desktop || !!profile.printer);

  const finish = async (pdf: boolean) => {
    const next = await onPrinted(pdf);
    if (next) setCurrent(next);
  };
  const print = async () => {
    setError("");
    setBusy("print");
    try {
      await printOn(profile.printer, page);
      await finish(false);
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
    try {
      await new Promise((r) => setTimeout(r, 60));
      // A PDF is the receipt's own size, filed by year and month (6.9).
      const path = await savePdfAt(current.number, { w: sheetW, h: Math.ceil(height) + 8 }, receiptFolder(current.local_date));
      await finish(true);
      if (path) {
        setSavedPath(path);
        toast.success(t("print.pdfSaved"));
      }
    } catch (x) {
      setError(t("print.failed").replace("{detail}", String(x)));
    } finally {
      setBusy("");
    }
  };

  const sheetPx = sheetW * PX_PER_MM;
  const scale = Math.min(1.4, 330 / sheetPx);
  return (
    <Dialog title={t("rcpt.preview")} onClose={onClose} wide testId="receipt-preview">
      <div className="doc-preview-layout">
        <div className="doc-preview" data-testid="receipt-preview-paper">
          <div className="rcpt-desk" dir="ltr" style={{ width: sheetPx * scale, height: Math.max(height, 40) * PX_PER_MM * scale }}>
            <div style={{ transform: `scale(${scale})`, transformOrigin: "top left", width: `${sheetW}mm` }}>{sheet}</div>
          </div>
        </div>
        <aside className="doc-preview-side">
          <div className="stack" style={{ gap: "var(--space-3)" }}>
            <div className="row" style={{ justifyContent: "space-between" }}>
              <span className="field-label">{t("rcpt.size")}</span>
              <Badge tone="accent" testId="receipt-size">{t(`rcpt.mode.${profile.mode}`)}</Badge>
            </div>
            <Segmented<Language> value={lang} onChange={setLang} label={t("rcpt.language")} block options={[{ value: "fa", label: "دری" }, { value: "ps", label: "پښتو" }, { value: "en", label: "EN" }]} />
            {current.print_count > 0 && <Notice tone="info" testId="receipt-reprint-note">{t("rcpt.reprintNote").replace("{n}", String(current.print_count))}</Notice>}
            {error && <Notice tone="danger" testId="print-error">{error}</Notice>}
            <Button icon={Settings2} onClick={() => setSetup(true)} block data-testid="receipt-setup-open">{t("rcpt.setup")}</Button>
          </div>
          <div className="stack" style={{ gap: "var(--space-2)" }}>
            <Button variant="primary" icon={Printer} loading={busy === "print"} disabled={!canPrint} onClick={print} block data-testid="receipt-print">{t("print.print")}</Button>
            <Button icon={FileDown} loading={busy === "pdf"} onClick={pdf} block data-testid="receipt-pdf">{t("print.pdf")}</Button>
            {savedPath && <Button variant="subtle" icon={FolderOpen} onClick={() => revealFile(savedPath)} block data-testid="receipt-reveal">{t("print.openFolder")}</Button>}
            {!canPrint && <span className="subtle t-caption">{profile.mode === "pdf" ? t("rcpt.pdfOnlyMode") : t("print.pdfOnlyHint")}</span>}
          </div>
        </aside>
      </div>
      {/* Measures the sheet at its real width; off screen. */}
      <div className="rcpt-measure" aria-hidden><div ref={measure} style={{ width: `${sheetW}mm` }}>{sheet}</div></div>
      {test.layer ?? <PrintLayer page={page} compact={false}>{placed}</PrintLayer>}
      {setup && <ReceiptSetupDialog profile={profile} printers={printers ?? []} desktop={desktop} onChange={update} onTest={test.run} testing={test.testing} onClose={() => setSetup(false)} />}
    </Dialog>
  );
}

const MODE_OPTIONS: { value: ReceiptMode; group: "roll" | "small" | "a4" | "pdf" }[] = [
  { value: "thermal80", group: "roll" },
  { value: "thermal58", group: "roll" },
  { value: "a6", group: "small" },
  { value: "a5", group: "small" },
  { value: "custom", group: "small" },
  { value: "a4_one", group: "a4" },
  { value: "a4_two", group: "a4" },
  { value: "a4_four", group: "a4" },
  { value: "pdf", group: "pdf" },
];

/** How this computer prints receipts (6.12): the printer and the kind of paper, with a test page and calibration. */
export function ReceiptSetupDialog({ profile, printers, desktop, onChange, onTest, testing, onClose }: { profile: ReceiptProfile; printers: string[]; desktop: boolean; onChange: (p: ReceiptProfile) => void; onTest: () => void; testing: boolean; onClose: () => void }) {
  const { t } = useI18n();
  const [p, setP] = useState(profile);
  const set = (next: Partial<ReceiptProfile>) => {
    const v = { ...p, ...next };
    setP(v);
    onChange(v);
  };
  const num2 = (s: string, min: number, max: number, fallback: number) => {
    const n = Number(s);
    return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback;
  };
  return (
    <Dialog title={t("rcpt.setup")} description={t("rcpt.setup.hint")} onClose={onClose} testId="receipt-setup">
      <div className="stack">
        <div className="field">
          <span className="field-label">{t("rcpt.setup.paper")}</span>
          <div className="rcpt-modes" role="radiogroup" aria-label={t("rcpt.setup.paper")}>
            {MODE_OPTIONS.map((o) => (
              <button key={o.value} type="button" role="radio" aria-checked={p.mode === o.value} className="rcpt-mode" onClick={() => set({ mode: o.value })} data-testid={`receipt-mode-${o.value}`}>
                <span className="rcpt-mode-title">{t(`rcpt.mode.${o.value}`)}</span>
                <span className="rcpt-mode-hint">{t(`rcpt.mode.${o.value}.hint`)}</span>
              </button>
            ))}
          </div>
        </div>
        {p.mode === "custom" && (
          <div className="grid-2">
            <Field label={t("rcpt.setup.width")} hint={t("rcpt.setup.mm")}><TextInput inputMode="numeric" value={String(p.customWidth)} onChange={(e) => set({ customWidth: num2(e.target.value, 40, 210, p.customWidth) })} data-testid="receipt-custom-width" /></Field>
            <Field label={t("rcpt.setup.height")} hint={t("rcpt.setup.mm")}><TextInput inputMode="numeric" value={String(p.customHeight)} onChange={(e) => set({ customHeight: num2(e.target.value, 40, 297, p.customHeight) })} data-testid="receipt-custom-height" /></Field>
          </div>
        )}
        {p.mode !== "pdf" && (
          <Field label={t("print.printer")} hint={desktop ? t("rcpt.setup.printerHint") : t("print.browserHint")}>
            <Select value={p.printer ?? ""} onChange={(e) => set({ printer: e.target.value || null })} disabled={!desktop} data-testid="receipt-printer">
              <option value="">{t("print.noPrinter")}</option>
              {printers.map((x) => <option key={x} value={x}>{x}</option>)}
            </Select>
          </Field>
        )}
        {p.mode !== "pdf" && (
          <div className="grid-2">
            <Field label={t("rcpt.setup.offsetX")} hint={t("rcpt.setup.offsetHint")}><TextInput inputMode="decimal" dir="ltr" value={String(p.offsetX)} onChange={(e) => set({ offsetX: num2(e.target.value, -10, 10, p.offsetX) })} data-testid="receipt-offset-x" /></Field>
            <Field label={t("rcpt.setup.offsetY")} hint={t("rcpt.setup.offsetHint")}><TextInput inputMode="decimal" dir="ltr" value={String(p.offsetY)} onChange={(e) => set({ offsetY: num2(e.target.value, -10, 10, p.offsetY) })} data-testid="receipt-offset-y" /></Field>
          </div>
        )}
        <div className="row">
          {p.mode !== "pdf" && <Button icon={Printer} loading={testing} disabled={desktop && !p.printer} onClick={onTest} data-testid="receipt-test-print">{t("rcpt.setup.test")}</Button>}
          <Button variant="primary" onClick={onClose} data-testid="receipt-setup-done">{t("common.done")}</Button>
        </div>
      </div>
    </Dialog>
  );
}

/** The print settings card of «تنظیمات» (6.12): how this computer prints receipts, with a test page. */
export function ReceiptSettings() {
  const { t } = useI18n();
  const desktop = isDesktop();
  const [profile, setProfile] = useState<ReceiptProfile>(loadReceiptProfile());
  const [printers, setPrinters] = useState<string[]>([]);
  const [open, setOpen] = useState(false);
  const test = useTestPrint(profile);
  useEffect(() => {
    listPrinters().then((l) => setPrinters(l.printers)).catch(() => {});
  }, []);
  const update = (p: ReceiptProfile) => {
    setProfile(p);
    saveReceiptProfile(p);
  };
  return (
    <div className="row" style={{ justifyContent: "space-between", gap: "var(--space-4)" }}>
      <div className="stack" style={{ gap: 2 }}>
        <span className="cell-strong">{t("rcpt.settings.title")}</span>
        <span className="subtle t-caption" data-testid="receipt-profile-summary">{t(`rcpt.mode.${profile.mode}`)}{profile.printer ? ` · ${profile.printer}` : ""}</span>
      </div>
      <Button icon={Settings2} onClick={() => setOpen(true)} data-testid="receipt-settings-open">{t("rcpt.setup")}</Button>
      {test.layer}
      {open && <ReceiptSetupDialog profile={profile} printers={printers} desktop={desktop} onChange={update} onTest={test.run} testing={test.testing} onClose={() => setOpen(false)} />}
    </div>
  );
}
