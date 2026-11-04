// «انبار» — Phase 8: stock and its alerts, purchases, suppliers and labs, the dental lab cases, and the
// lists the stock uses. What a person sees follows their permissions.
import { useMemo, useState } from "react";
import { FlaskConical, PackageSearch, Settings2, ShoppingCart, Truck } from "lucide-react";
import type { ClinicProfile } from "../../../../shared/ts/contract";
import { useI18n } from "../../i18n";
import { Page, PageHeader } from "../../ui/Card";
import { LabTab } from "./LabTab";
import { PurchasesTab } from "./PurchasesTab";
import { StockTab } from "./StockTab";
import { SuppliersTab } from "./SuppliersTab";
import { InventorySettings } from "./InventorySettings";
import { canLab } from "./common";
import type { InventoryPerms } from "./common";

type TabId = "stock" | "purchases" | "suppliers" | "lab" | "settings";

const TAB_ICONS = { stock: PackageSearch, purchases: ShoppingCart, suppliers: Truck, lab: FlaskConical, settings: Settings2 };

export function InventoryPage({ perms, clinic }: { perms: InventoryPerms; clinic: ClinicProfile | null }) {
  const { t } = useI18n();
  const tabs = useMemo<TabId[]>(
    () => [
      ...(perms.view ? (["stock", "purchases", "suppliers"] as TabId[]) : []),
      ...(perms.view || perms.clinicalView || canLab(perms) ? (["lab"] as TabId[]) : []),
      ...(perms.manage ? (["settings"] as TabId[]) : []),
    ],
    [perms],
  );
  const [tab, setTab] = useState<TabId>(tabs[0] ?? "stock");
  const calendar = clinic?.calendar_system ?? "shamsi";
  return (
    <Page testId="page-inventory">
      <PageHeader title={t("nav.inventory")} description={t("inventory.subtitle")} />
      <div className="profile-tabs" role="tablist">
        {tabs.map((id) => {
          const Icon = TAB_ICONS[id];
          return (
            <button key={id} type="button" role="tab" aria-selected={tab === id} className="profile-tab" onClick={() => setTab(id)} data-testid={`inventory-tab-${id}`}>
              <Icon aria-hidden />
              <span>{t(`inventory.tab.${id}`)}</span>
            </button>
          );
        })}
      </div>
      {tab === "stock" && <StockTab perms={perms} calendar={calendar} onOpenLab={() => tabs.includes("lab") && setTab("lab")} />}
      {tab === "purchases" && <PurchasesTab perms={perms} calendar={calendar} />}
      {tab === "suppliers" && <SuppliersTab perms={perms} calendar={calendar} />}
      {tab === "lab" && <LabTab perms={perms} calendar={calendar} canCreatePatients={perms.patientsEdit} />}
      {tab === "settings" && <InventorySettings />}
    </Page>
  );
}
