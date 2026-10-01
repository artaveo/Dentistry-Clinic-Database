import { useEffect, useState } from "react";
import type { LabeledItem } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { useI18n } from "../i18n";
import { Field, Select } from "../ui/Field";

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
      <Field label={t("wizard.clinicInfo.province")} optional>
        <Select id="province" value={provinceId ?? ""} onChange={(e) => onChange(e.target.value || null, null)} data-testid="setup-province">
          <option value="">{t("common.choose")}</option>
          {provinces.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
        </Select>
      </Field>
      <Field label={t("wizard.clinicInfo.district")} optional hint={!provinceId ? t("hint.districtAfterProvince") : undefined}>
        <Select id="district" value={districtId ?? ""} disabled={!provinceId} onChange={(e) => onChange(provinceId, e.target.value || null)} data-testid="setup-district">
          <option value="">{t("common.choose")}</option>
          {districts.map((d) => <option key={d.id} value={d.id}>{d.label}</option>)}
        </Select>
      </Field>
    </>
  );
}
