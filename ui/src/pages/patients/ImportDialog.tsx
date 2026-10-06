import { useEffect, useRef, useState } from "react";
import { FileSpreadsheet, FileUp, Sparkles } from "lucide-react";
import type { ImportField, ImportInspectResult, ImportPatientsResult } from "../../../../shared/ts/contract";
import { isSessionError, rpc } from "../../lib/api";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Field, Select } from "../../ui/Field";
import { Badge, Notice } from "../../ui/Feedback";
import { Dialog } from "../../ui/Overlay";
import { Switch } from "../../ui/Controls";
import { useToast } from "../../ui/Toast";

export const IMPORT_FIELDS: ImportField[] = [
  "full_name", "father_name", "phone", "secondary_phone", "date_of_birth", "approximate_age", "gender", "province", "address",
  "emergency_contact_name", "emergency_contact_phone", "notes", "registration_date",
];

type Mapping = Record<number, ImportField | "">;

function toMapping(info: ImportInspectResult): Mapping {
  const m: Mapping = {};
  info.headers.forEach((_, i) => (m[i] = ""));
  info.suggested.forEach((s) => (m[s.column] = s.field));
  return m;
}

/**
 * Patient import from Excel (.xlsx) or CSV (3.7, OF-017): the file is read first, each column is
 * matched to a patient field by its header (Dari, Pashto or English) and the user can correct the
 * match; the preview then shows every row's exact problem before anything is written.
 */
export function ImportDialog({ onClose, onImported }: { onClose: () => void; onImported: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [file, setFile] = useState<{ name: string; base64: string } | null>(null);
  const [info, setInfo] = useState<ImportInspectResult | null>(null);
  const [sheet, setSheet] = useState<string>("");
  const [mapping, setMapping] = useState<Mapping>({});
  const [preview, setPreview] = useState<ImportPatientsResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const seq = useRef(0);

  // `hasHeader` is the user's choice (OF-042); null lets the Core detect it from the first row.
  const inspect = async (f: { name: string; base64: string }, sheetName?: string, hasHeader: boolean | null = null) => {
    setError("");
    setPreview(null);
    try {
      const r = await rpc("patients.import_inspect", { file_base64: f.base64, file_name: f.name, sheet: sheetName ?? null, has_header: hasHeader });
      setInfo(r);
      setSheet(r.sheet ?? "");
      setMapping(toMapping(r));
    } catch (x) {
      setInfo(null);
      if (!isSessionError(x)) setError(x && typeof x === "object" && "rule" in x && (x as { rule: string | null }).rule ? t(`rule.${(x as { rule: string }).rule}`) : err(x));
    }
  };

  const pick = (picked: File | undefined) => {
    if (!picked) return;
    const reader = new FileReader();
    reader.onload = () => {
      const f = { name: picked.name, base64: (reader.result as string).split(",")[1] };
      setFile(f);
      inspect(f);
    };
    reader.readAsDataURL(picked);
  };

  const columns = () => Object.entries(mapping).filter(([, f]) => f).map(([c, f]) => ({ column: Number(c), field: f as ImportField }));
  const hasName = columns().some((c) => c.field === "full_name");
  const duplicates = (() => {
    const seen = new Set<string>();
    return columns().filter((c) => (seen.has(c.field) ? true : (seen.add(c.field), false))).map((c) => c.field);
  })();

  // Preview again whenever the mapping changes.
  useEffect(() => {
    if (!file || !info || !hasName || duplicates.length) {
      setPreview(null);
      return;
    }
    const mine = ++seq.current;
    const id = window.setTimeout(() => {
      rpc("patients.import", { file_base64: file.base64, file_name: file.name, sheet: sheet || null, mapping: columns(), commit: false, has_header: info.has_header })
        .then((r) => mine === seq.current && setPreview(r))
        .catch((x) => mine === seq.current && !isSessionError(x) && setError(err(x)));
    }, 250);
    return () => window.clearTimeout(id);
  }, [file, info, sheet, mapping]);

  const commit = async () => {
    if (!file) return;
    setBusy(true);
    setError("");
    try {
      const r = await rpc("patients.import", { file_base64: file.base64, file_name: file.name, sheet: sheet || null, mapping: columns(), commit: true, has_header: info?.has_header ?? null });
      toast.success(t("patients.import.done").replace("{imported}", String(r.imported)));
      onImported();
    } catch (x) {
      if (!isSessionError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  const problem = (e: ImportPatientsResult["errors"][number]) => (e.rule ? t(`rule.${e.rule}`) : e.message);

  // OF-042: when the button is disabled, the reason is written next to it.
  const blocker = !file
    ? t("patients.import.blockNoFile")
    : !info
      ? null
      : !hasName
        ? t("rule.import_no_name_column")
        : duplicates.length
          ? t("rule.import_column")
          : !preview
            ? t("patients.import.blockChecking")
            : preview.total - preview.skipped <= 0
              ? t("patients.import.blockNoValid")
              : null;

  return (
    <Dialog
      title={t("patients.import.title")}
      description={t("patients.import.hint")}
      onClose={onClose}
      testId="import-dialog"
      wide
      footer={
        <>
          {blocker && <span className="subtle t-caption import-blocker" data-testid="import-commit-blocker">{blocker}</span>}
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={FileUp} disabled={!preview || preview.total - preview.skipped <= 0} loading={busy} onClick={commit} data-testid="import-commit">{t("patients.import.commit")}</Button>
        </>
      }
    >
      <div className="stack">
        {error && <Notice tone="danger" testId="import-error">{error}</Notice>}
        <input type="file" accept=".csv,.xlsx,text/csv,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" onChange={(e) => pick(e.target.files?.[0])} data-testid="import-file-input" />
        {info && (
          <>
            {info.sheets.length > 1 && (
              <Field label={t("patients.import.sheet")}>
                <Select value={sheet} onChange={(e) => file && inspect(file, e.target.value)} data-testid="import-sheet">
                  {info.sheets.map((s) => <option key={s} value={s}>{s}</option>)}
                </Select>
              </Field>
            )}
            <Switch checked={info.has_header} onChange={(v) => file && inspect(file, sheet || undefined, v)} label={t("patients.import.firstRowIsTitle")} testId="import-has-header" />
            <Notice tone="info" title={<span className="row"><Sparkles size={16} aria-hidden />{t("patients.import.mapTitle")}</span>}>{t("patients.import.mapHint")}</Notice>
            <div className="table-wrap">
              <table className="table" data-testid="import-mapping">
                <thead><tr><th>{t("patients.import.fileColumn")}</th><th>{t("patients.import.example")}</th><th>{t("patients.import.patientField")}</th></tr></thead>
                <tbody>
                  {info.headers.map((h, i) => (
                    <tr key={i}>
                      <td className="cell-strong">{h || <span className="subtle">#{i + 1}</span>}</td>
                      <td className="subtle t-caption">{info.sample_rows.slice(0, 3).map((r) => r[i]).filter(Boolean).join(" · ") || "—"}</td>
                      <td>
                        <Select value={mapping[i] ?? ""} onChange={(e) => setMapping((m) => ({ ...m, [i]: e.target.value as ImportField | "" }))} aria-label={h} data-testid={`import-map-${i}`}>
                          <option value="">{t("patients.import.skipColumn")}</option>
                          {IMPORT_FIELDS.map((f) => <option key={f} value={f}>{t(`import.field.${f}`)}</option>)}
                        </Select>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {!hasName && <Notice tone="warning" testId="import-no-name">{t("rule.import_no_name_column")}</Notice>}
            {duplicates.length > 0 && <Notice tone="warning">{t("rule.import_column")}</Notice>}
          </>
        )}
        {preview && (
          <>
            <div className="row" data-testid="import-summary">
              <Badge icon={FileSpreadsheet}>{t("patients.import.total")}: {preview.total}</Badge>
              <Badge tone="success">{t("patients.import.willImport")}: {preview.total - preview.skipped}</Badge>
              <Badge tone="danger">{t("patients.import.skipped")}: {preview.skipped}</Badge>
            </div>
            {preview.errors.length > 0 && (
              <div className="table-wrap">
                <table className="table" data-testid="import-errors">
                  <thead><tr><th>#</th><th>{t("patients.import.patientField")}</th><th>{t("patients.import.rowErrors")}</th></tr></thead>
                  <tbody>
                    {preview.errors.map((e, i) => (
                      <tr key={i}><td className="cell-num">{e.row_number}</td><td>{e.field ? t(`import.field.${e.field}`) : "—"}</td><td>{problem(e)}</td></tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        )}
      </div>
    </Dialog>
  );
}
