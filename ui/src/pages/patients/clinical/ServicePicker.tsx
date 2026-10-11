import { useMemo, useState } from "react";
import { Search } from "lucide-react";
import type { CatalogInfo, ServiceInfo } from "../../../../../shared/ts/contract";
import { latinDigits } from "../../../lib/dates";
import { serviceName } from "../../../lib/clinical";
import { tr } from "../../../lib/medical";
import { formatAmount } from "../../../lib/money";
import { useI18n } from "../../../i18n";
import { TextInput } from "../../../ui/Field";
import { scopeFits } from "../../../chart/model";

const norm = (s: string) => latinDigits(s).toLowerCase().replace(/ي/g, "ی").replace(/ك/g, "ک").replace(/‌/g, " ");

/**
 * "نوع کار" — the catalog to pick a service from (M5, both paths): search, category chips, and, when
 * teeth are already selected on the chart, the services that fit that selection first (tooth first).
 * A service with variants is offered through its variants.
 */
export function ServicePicker({ catalog, selectedTeeth, value, onPick, testId = "service-picker" }: {
  catalog: CatalogInfo | null;
  selectedTeeth: number[];
  value: string | null;
  onPick: (s: ServiceInfo) => void;
  testId?: string;
}) {
  const { t, lang } = useI18n();
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<string>("");
  const services = useMemo(() => {
    if (!catalog) return [];
    const parents = new Set(catalog.services.filter((s) => s.parent_id).map((s) => s.parent_id));
    const q = norm(query.trim());
    return catalog.services
      .filter((s) => !parents.has(s.id))
      .filter((s) => !category || s.category_id === category)
      .filter((s) => !q || norm(`${serviceName(s, catalog, "fa")} ${serviceName(s, catalog, "ps")} ${serviceName(s, catalog, "en")} ${s.code}`).includes(q))
      .map((s) => ({ s, fits: scopeFits(s.tooth_scope, selectedTeeth) }))
      .sort((a, b) => Number(b.fits) - Number(a.fits));
  }, [catalog, query, category, selectedTeeth]);
  if (!catalog) return null;
  return (
    <div className="service-picker" data-testid={testId}>
      <TextInput icon={Search} value={query} placeholder={t("chart.serviceSearch")} onChange={(e) => setQuery(e.target.value)} data-testid={`${testId}-search`} />
      <div className="chip-group" role="group" aria-label={t("catalog.categories")}>
        <button type="button" className="chip-toggle" aria-pressed={!category} onClick={() => setCategory("")}>{t("chart.allCategories")}</button>
        {catalog.categories.map((c) => (
          <button key={c.id} type="button" className="chip-toggle" aria-pressed={category === c.id} onClick={() => setCategory(c.id)} data-testid={`${testId}-cat-${c.code}`}>{tr(c.name, lang)}</button>
        ))}
      </div>
      <ul className="service-list" role="listbox" aria-label={t("chart.service")}>
        {services.map(({ s, fits }) => (
          <li key={s.id}>
            <button
              type="button"
              role="option"
              aria-selected={value === s.id}
              className={`service-option ${fits ? "" : "unfit"}`}
              onClick={() => onPick(s)}
              title={fits ? undefined : t(`chart.scopeHint.${s.tooth_scope}`)}
              data-testid={`${testId}-${s.code}`}
            >
              <span className="service-option-name">{serviceName(s, catalog, lang)}</span>
              <span className="service-option-meta">
                <bdi className="ltr">{s.code}</bdi>
                <span>{t(`catalog.scope.${s.tooth_scope}`)}</span>
                {s.price > 0 && <span className="num">{formatAmount(s.price, lang)}</span>}
              </span>
            </button>
          </li>
        ))}
        {!services.length && <li className="subtle t-caption service-empty">{t("chart.noService")}</li>}
      </ul>
    </div>
  );
}
