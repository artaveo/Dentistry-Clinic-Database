import { useEffect, useMemo, useState } from "react";
import { FlaskConical, FolderPlus, Layers, Pencil, Plus, Search, Tags } from "lucide-react";
import type { CatalogInfo, DocumentTemplateInfo, ServiceCategoryInfo, ServiceInfo, ToothScope, Translations } from "../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../lib/api";
import { invalidateCatalog, useSpecialties } from "../lib/clinical";
import { digits, latinDigits } from "../lib/dates";
import { tr } from "../lib/medical";
import { formatAmount } from "../lib/money";
import { useI18n } from "../i18n";
import { Button, IconButton } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader, Stat } from "../ui/Card";
import { Switch } from "../ui/Controls";
import { Field, Select, TextInput } from "../ui/Field";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { TranslationsField } from "../ui/TranslationsField";
import { useToast } from "../ui/Toast";

const SCOPES: ToothScope[] = ["tooth", "teeth", "quadrant", "arch", "mouth", "none"];
const empty: Translations = { fa: "", ps: "", en: "" };
const hasName = (x: Translations) => !!(x.fa.trim() || x.ps.trim() || x.en.trim());

/**
 * The service catalog (5.1 + M3): category → service → variant, each with its code, names, price,
 * specialty, tooth scope and the documents it suggests. Seeded from the clinical-workflow spec, then
 * entirely the clinic's: prices start at zero and are set here.
 */
export function CatalogPage() {
  const { t, err, lang } = useI18n();
  const specialties = useSpecialties();
  const [catalog, setCatalog] = useState<CatalogInfo | null>(null);
  const [templates, setTemplates] = useState<DocumentTemplateInfo[]>([]);
  const [error, setError] = useState("");
  const [category, setCategory] = useState<string>("all");
  const [query, setQuery] = useState("");
  const [editingCat, setEditingCat] = useState<{ cat?: ServiceCategoryInfo } | null>(null);
  const [editing, setEditing] = useState<{ service?: ServiceInfo; parent?: ServiceInfo; categoryId?: string } | null>(null);

  const load = () =>
    rpc("catalog.get", { include_inactive: true })
      .then((c) => { setCatalog(c); setError(""); })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
    rpc("document_templates.list", { kind: null, include_inactive: false }).then(setTemplates).catch(() => {});
  }, []);
  const saved = () => {
    invalidateCatalog();
    load();
  };

  const q = query.trim().toLowerCase();
  const shown = useMemo(() => {
    if (!catalog) return [];
    const matches = (s: ServiceInfo) => !q || s.code.toLowerCase().includes(q) || [s.name.fa, s.name.ps, s.name.en].some((n) => n.toLowerCase().includes(q));
    return catalog.services.filter((s) => (category === "all" || s.category_id === category) && (matches(s) || (s.parent_id && matches(catalog.services.find((p) => p.id === s.parent_id)!)) || catalog.services.some((v) => v.parent_id === s.id && matches(v))));
  }, [catalog, category, q]);
  const unpriced = catalog?.services.filter((s) => s.is_active && s.price === 0 && !catalog.services.some((v) => v.parent_id === s.id)).length ?? 0;
  const specialtyName = (id: string | null) => tr(specialties?.find((s) => s.id === id)?.label, lang) || "—";

  if (error && !catalog) return <Page><ErrorState message={error} onRetry={load} /></Page>;
  return (
    <Page testId="page-catalog">
      <PageHeader
        title={t("nav.catalog")}
        description={t("catalog.subtitle")}
        actions={
          <>
            <Button icon={FolderPlus} onClick={() => setEditingCat({})} data-testid="catalog-category-new">{t("catalog.newCategory")}</Button>
            <Button variant="primary" icon={Plus} onClick={() => setEditing({ categoryId: category === "all" ? catalog?.categories[0]?.id : category })} disabled={!catalog?.categories.length} data-testid="catalog-service-new">{t("catalog.newService")}</Button>
          </>
        }
      />
      <div className="stats-row">
        <Stat icon={Layers} label={t("catalog.categories")} value={digits(catalog?.categories.length ?? 0, lang)} />
        <Stat icon={Tags} label={t("catalog.services")} value={digits(catalog?.services.length ?? 0, lang)} />
        <Stat icon={Tags} tone={unpriced ? "warning" : "success"} label={t("catalog.unpriced")} value={digits(unpriced, lang)} sub={unpriced ? t("catalog.unpricedHint") : undefined} testId="catalog-unpriced" />
      </div>
      <div className="catalog-layout">
        <Card flush className="catalog-cats">
          <CardHeader icon={Layers} title={t("catalog.categories")} />
          <div className="catalog-cat-list" role="listbox" aria-label={t("catalog.categories")}>
            <button type="button" role="option" aria-selected={category === "all"} className="catalog-cat" onClick={() => setCategory("all")} data-testid="catalog-cat-all">
              <span className="cell-strong">{t("catalog.all")}</span>
            </button>
            {catalog?.categories.map((c) => (
              <div key={c.id} className="catalog-cat-row">
                <button type="button" role="option" aria-selected={category === c.id} className={`catalog-cat ${c.is_active ? "" : "inactive"}`} onClick={() => setCategory(c.id)} data-testid={`catalog-cat-${c.code}`}>
                  <span className="cell-strong">{tr(c.name, lang)}</span>
                  <span className="subtle t-caption">{specialtyName(c.specialty_id)} · {digits(catalog.services.filter((s) => s.category_id === c.id && !s.parent_id).length, lang)}</span>
                </button>
                <IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditingCat({ cat: c })} data-testid={`catalog-cat-edit-${c.code}`} />
              </div>
            ))}
          </div>
        </Card>
        <Card flush>
          <div className="catalog-toolbar">
            <TextInput icon={Search} placeholder={t("catalog.search")} value={query} onChange={(e) => setQuery(e.target.value)} data-testid="catalog-search" />
          </div>
          <div className="table-wrap">
            <table className="table" data-testid="catalog-table">
              <thead>
                <tr><th>{t("catalog.code")}</th><th>{t("catalog.name")}</th><th>{t("catalog.specialty")}</th><th>{t("catalog.scope")}</th><th className="num">{t("catalog.price")}</th><th className="cell-actions"><span className="visually-hidden">{t("common.edit")}</span></th></tr>
              </thead>
              <tbody>
                {!catalog && <SkeletonRows cols={6} />}
                {shown.map((s) => {
                  const hasVariants = catalog!.services.some((v) => v.parent_id === s.id);
                  return (
                    <tr key={s.id} className={`${s.parent_id ? "row-variant" : ""} ${s.is_active ? "" : "row-inactive"}`} data-testid={`catalog-row-${s.code}`}>
                      <td><bdi className="chip-code">{s.code}</bdi></td>
                      <td>
                        <span className={s.parent_id ? "" : "cell-strong"}>{s.parent_id ? `↳ ${tr(s.name, lang)}` : tr(s.name, lang)}</span>
                        <span className="row" style={{ gap: 4, marginTop: 2 }}>
                          {s.sessions > 1 && <Badge>{t("catalog.sessionsN").replace("{n}", digits(s.sessions, lang))}</Badge>}
                          {s.lab_required && <Badge tone="info" icon={FlaskConical}>{t("catalog.lab")}</Badge>}
                          {s.needs_surface && <Badge>{t("catalog.surface")}</Badge>}
                          {!s.is_active && <Badge dot>{t("users.inactive")}</Badge>}
                        </span>
                      </td>
                      <td>{specialtyName(s.specialty_id)}</td>
                      <td>{t(`catalog.scope.${s.tooth_scope}`)}</td>
                      <td className="num">{hasVariants ? <span className="subtle">{t("catalog.byVariant")}</span> : s.price ? formatAmount(s.price, lang) : <Badge tone="warning" dot>{t("catalog.noPrice")}</Badge>}</td>
                      <td className="cell-actions">
                        <div className="row" style={{ justifyContent: "flex-end", flexWrap: "nowrap" }}>
                          {!s.parent_id && <IconButton icon={Plus} size="sm" label={t("catalog.addVariant")} onClick={() => setEditing({ parent: s })} data-testid={`catalog-variant-new-${s.code}`} />}
                          <IconButton icon={Pencil} size="sm" label={t("common.edit")} onClick={() => setEditing({ service: s })} data-testid={`catalog-edit-${s.code}`} />
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {catalog && shown.length === 0 && <EmptyState icon={Tags} title={t("catalog.none")} />}
          </div>
        </Card>
      </div>
      {editingCat && <CategoryDialog cat={editingCat.cat} onClose={() => setEditingCat(null)} onSaved={() => { setEditingCat(null); saved(); }} />}
      {editing && catalog && <ServiceDialog catalog={catalog} templates={templates} {...editing} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); saved(); }} />}
    </Page>
  );
}

function CategoryDialog({ cat, onClose, onSaved }: { cat?: ServiceCategoryInfo; onClose: () => void; onSaved: () => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const specialties = useSpecialties();
  const [name, setName] = useState<Translations>(cat?.name ?? empty);
  const [specialty, setSpecialty] = useState(cat?.specialty_id ?? "");
  const [active, setActive] = useState(cat?.is_active ?? true);
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const save = async () => {
    setSubmitted(true);
    if (!hasName(name)) return;
    setBusy(true);
    try {
      await rpc("catalog.save_category", { id: cat?.id ?? null, version: cat?.version ?? 0, name, specialty_id: specialty || null, is_active: active });
      toast.success(t("common.saved"));
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) setError(x instanceof ApiError && x.rule ? t(`rule.${x.rule}`) : err(x));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog title={cat ? t("catalog.editCategory") : t("catalog.newCategory")} onClose={onClose} wide testId="catalog-category-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="catalog-category-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        <TranslationsField label={t("catalog.name")} value={name} onChange={setName} error={submitted && !hasName(name) ? t("rule.required") : null} testId="catalog-category-name" />
        <Field label={t("catalog.specialty")} hint={t("catalog.categorySpecialtyHint")} optional>
          <Select value={specialty} onChange={(e) => setSpecialty(e.target.value)} data-testid="catalog-category-specialty">
            <option value="">—</option>
            {specialties?.filter((s) => s.is_active || s.id === specialty).map((s) => <option key={s.id} value={s.id}>{tr(s.label, lang)}</option>)}
          </Select>
        </Field>
        {cat && <Switch checked={active} onChange={setActive} label={t("catalog.active")} testId="catalog-category-active" />}
      </div>
    </Dialog>
  );
}

function ServiceDialog({ catalog, templates, service, parent, categoryId, onClose, onSaved }: {
  catalog: CatalogInfo;
  templates: DocumentTemplateInfo[];
  service?: ServiceInfo;
  parent?: ServiceInfo;
  categoryId?: string;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const specialties = useSpecialties();
  // A new variant starts as a copy of its service's settings.
  const base = service ?? parent;
  const parentOf = service?.parent_id ? catalog.services.find((s) => s.id === service.parent_id) : parent;
  const [code, setCode] = useState(service?.code ?? (parent ? `${parent.code}-` : ""));
  const [name, setName] = useState<Translations>(service?.name ?? empty);
  const [cat, setCat] = useState(service?.category_id ?? parent?.category_id ?? categoryId ?? catalog.categories[0]?.id ?? "");
  const [price, setPrice] = useState(service?.price ? String(service.price / 100) : "");
  const [specialty, setSpecialty] = useState(service?.specialty_id ?? "");
  const [scope, setScope] = useState<ToothScope>(base?.tooth_scope ?? "tooth");
  const [surface, setSurface] = useState(base?.needs_surface ?? false);
  const [sessions, setSessions] = useState(String(base?.sessions ?? 1));
  const [lab, setLab] = useState(base?.lab_required ?? false);
  const [consent, setConsent] = useState(base?.consent_template_id ?? "");
  const [postOp, setPostOp] = useState(base?.post_op_template_id ?? "");
  const [active, setActive] = useState(service?.is_active ?? true);
  const [submitted, setSubmitted] = useState(false);
  const [server, setServer] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const priceNum = Number(latinDigits(price.trim() || "0").replace(/[,٬]/g, ""));
  const errors: Record<string, string | null> = {
    code: !code.trim() ? "rule.required" : !/^[A-Za-z0-9_.-]{1,20}$/.test(code.trim()) ? "rule.service_code_format" : null,
    name: !hasName(name) ? "rule.required" : null,
    price: !Number.isFinite(priceNum) || priceNum < 0 || priceNum > 100_000_000 ? "rule.service_price" : null,
    sessions: !/^\d+$/.test(latinDigits(sessions.trim())) || Number(latinDigits(sessions)) < 1 || Number(latinDigits(sessions)) > 60 ? "rule.service_sessions" : null,
  };
  const show = (k: string) => (server[k] ? t(server[k]) : submitted && errors[k] ? t(errors[k]!) : null);
  const inherited = specialties?.find((s) => s.id === catalog.categories.find((c) => c.id === cat)?.specialty_id);

  const save = async () => {
    setSubmitted(true);
    setServer({});
    if (Object.values(errors).some(Boolean)) return;
    setBusy(true);
    try {
      await rpc("catalog.save_service", {
        id: service?.id ?? null, version: service?.version ?? 0, code: code.trim(), category_id: cat, parent_id: parentOf?.id ?? null, name,
        price: Math.round(priceNum * 100), specialty_id: specialty || null, tooth_scope: scope, needs_surface: surface, sessions: Number(latinDigits(sessions)),
        lab_required: lab, consent_template_id: consent || null, post_op_template_id: postOp || null, is_active: active,
      });
      toast.success(t("common.saved"));
      onSaved();
    } catch (x) {
      if (isSessionError(x)) return;
      if (x instanceof ApiError && x.field) setServer({ [x.field === "label" ? "name" : x.field]: x.rule ? `rule.${x.rule}` : `error.${x.code}` });
      else setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  const title = service ? t("catalog.editService") : parent ? t("catalog.newVariantOf").replace("{name}", tr(parent.name, lang)) : t("catalog.newService");
  return (
    <Dialog title={title} onClose={onClose} wide testId="catalog-service-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" loading={busy} onClick={save} data-testid="catalog-service-save">{t("common.save")}</Button></>}>
      <div className="stack">
        {error && <Notice tone="danger">{error}</Notice>}
        {parentOf && <Notice tone="info">{t("catalog.variantHint").replace("{name}", tr(parentOf.name, lang))}</Notice>}
        <div className="grid-2">
          <Field label={t("catalog.code")} hint={t("catalog.codeHint")} error={show("code")}>
            <TextInput dir="ltr" value={code} onChange={(e) => setCode(e.target.value)} data-testid="catalog-service-code" />
          </Field>
          <Field label={t("catalog.priceAfn")} hint={t("catalog.priceHint")} error={show("price")}>
            <TextInput inputMode="numeric" suffix={lang === "en" ? "AFN" : "افغانی"} value={price} onChange={(e) => setPrice(e.target.value)} data-testid="catalog-service-price" />
          </Field>
        </div>
        <TranslationsField label={t("catalog.name")} value={name} onChange={setName} error={show("name")} testId="catalog-service-name" />
        <div className="grid-2">
          {!parentOf && (
            <Field label={t("catalog.category")}>
              <Select value={cat} onChange={(e) => setCat(e.target.value)} data-testid="catalog-service-category">
                {catalog.categories.map((c) => <option key={c.id} value={c.id}>{tr(c.name, lang)}</option>)}
              </Select>
            </Field>
          )}
          <Field label={t("catalog.specialty")} hint={inherited ? t("catalog.specialtyInherited").replace("{name}", tr(inherited.label, lang)) : undefined} optional>
            <Select value={specialty} onChange={(e) => setSpecialty(e.target.value)} data-testid="catalog-service-specialty">
              <option value="">{t("catalog.specialtyFromCategory")}</option>
              {specialties?.filter((s) => s.is_active || s.id === specialty).map((s) => <option key={s.id} value={s.id}>{tr(s.label, lang)}</option>)}
            </Select>
          </Field>
          <Field label={t("catalog.scope")}>
            <Select value={scope} onChange={(e) => setScope(e.target.value as ToothScope)} data-testid="catalog-service-scope">
              {SCOPES.map((s) => <option key={s} value={s}>{t(`catalog.scope.${s}`)}</option>)}
            </Select>
          </Field>
          <Field label={t("catalog.sessions")} hint={t("catalog.sessionsHint")} error={show("sessions")}>
            <TextInput inputMode="numeric" value={sessions} onChange={(e) => setSessions(e.target.value)} data-testid="catalog-service-sessions" />
          </Field>
          <Field label={t("catalog.consent")} hint={t("catalog.consentHint")} optional error={show("consent_template_id")}>
            <Select value={consent} onChange={(e) => setConsent(e.target.value)} data-testid="catalog-service-consent">
              <option value="">—</option>
              {templates.filter((x) => x.kind === "consent").map((x) => <option key={x.id} value={x.id}>{tr(x.title, lang)}</option>)}
            </Select>
          </Field>
          <Field label={t("catalog.postOp")} optional error={show("post_op_template_id")}>
            <Select value={postOp} onChange={(e) => setPostOp(e.target.value)} data-testid="catalog-service-postop">
              <option value="">—</option>
              {templates.filter((x) => x.kind === "post_op").map((x) => <option key={x.id} value={x.id}>{tr(x.title, lang)}</option>)}
            </Select>
          </Field>
        </div>
        <div className="row" style={{ gap: "var(--space-6)", flexWrap: "wrap" }}>
          <Switch checked={surface} onChange={setSurface} label={t("catalog.surfaceLabel")} testId="catalog-service-surface" />
          <Switch checked={lab} onChange={setLab} label={t("catalog.labLabel")} testId="catalog-service-lab" />
          {service && <Switch checked={active} onChange={setActive} label={t("catalog.active")} testId="catalog-service-active" />}
        </div>
      </div>
    </Dialog>
  );
}
