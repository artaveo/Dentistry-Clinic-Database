import { useEffect, useState } from "react";
import type { LabeledItem } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { useI18n } from "../i18n";

/** Province → District cascading picker (roadmap 2.5), backed by the
 * already-seeded `geo.provinces`/`geo.districts` reference data (1.5). */
export function GeoPicker({
  provinceId,
  districtId,
  onChange,
}: {
  provinceId: string | null;
  districtId: string | null;
  onChange: (provinceId: string | null, districtId: string | null) => void;
}) {
  const { t, lang } = useI18n();
  const [provinces, setProvinces] = useState<LabeledItem[]>([]);
  const [districts, setDistricts] = useState<LabeledItem[]>([]);

  useEffect(() => {
    rpc("geo.provinces", { language: lang }).then(setProvinces).catch(() => setProvinces([]));
  }, [lang]);

  useEffect(() => {
    if (!provinceId) {
      setDistricts([]);
      return;
    }
    rpc("geo.districts", { province_id: provinceId, language: lang }).then(setDistricts).catch(() => setDistricts([]));
  }, [provinceId, lang]);

  return (
    <>
      <label htmlFor="province">{t("wizard.clinicInfo.province")}</label>
      <select
        id="province"
        value={provinceId ?? ""}
        onChange={(e) => onChange(e.target.value || null, null)}
        data-testid="setup-province"
      >
        <option value="">—</option>
        {provinces.map((p) => (
          <option key={p.id} value={p.id}>{p.label}</option>
        ))}
      </select>
      <label htmlFor="district">{t("wizard.clinicInfo.district")}</label>
      <select
        id="district"
        value={districtId ?? ""}
        disabled={!provinceId}
        onChange={(e) => onChange(provinceId, e.target.value || null)}
        data-testid="setup-district"
      >
        <option value="">—</option>
        {districts.map((d) => (
          <option key={d.id} value={d.id}>{d.label}</option>
        ))}
      </select>
    </>
  );
}
