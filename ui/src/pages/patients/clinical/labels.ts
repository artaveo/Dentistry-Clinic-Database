// Words for the clinical record of Phase 5B (chart, plans, sessions), shared by its screens.
import type { CatalogInfo, ItemStatus, Language, PlanStatus, ServiceInfo, ToothNumbering, TreatmentItemInfo } from "../../../../../shared/ts/contract";
import { digits } from "../../../lib/dates";
import { serviceName } from "../../../lib/clinical";
import { surfacesText, teethLabel } from "../../../chart/numbering";

export { surfacesText };
import type { ChartState } from "../../../chart/model";

type T = (key: string) => string;

export const PLAN_TONE: Record<PlanStatus, "neutral" | "accent" | "success" | "warning" | "danger" | "info"> = {
  proposed: "info",
  accepted: "accent",
  in_progress: "warning",
  completed: "success",
  cancelled: "neutral",
};

export const ITEM_TONE: Record<ItemStatus, "neutral" | "accent" | "success" | "warning" | "danger" | "info"> = {
  planned: "danger",
  in_progress: "warning",
  completed: "success",
  cancelled: "neutral",
};

export function stateText(t: T, s: ChartState): string {
  return t(`chart.state.${s}`);
}

/** "جلسه ۲ از ۳" while under way, "۳ جلسه" otherwise; empty for a one-visit treatment. */
export function sessionText(t: T, lang: Language, i: TreatmentItemInfo): string {
  if (i.sessions_planned <= 1) return "";
  if (i.status === "in_progress") {
    const n = Math.min(i.sessions_planned, i.sessions_done + 1);
    return t("chart.sessionOf").replace("{n}", digits(n, lang)).replace("{of}", digits(i.sessions_planned, lang));
  }
  return t("chart.sessions").replace("{n}", digits(i.sessions_planned, lang));
}

export function serviceOf(catalog: CatalogInfo | null, id: string): ServiceInfo | undefined {
  return catalog?.services.find((s) => s.id === id);
}

/** "عصب‌کشی — دوکاناله" of an item's service. */
export function itemService(catalog: CatalogInfo | null, i: { service_id: string }, lang: Language): string {
  const s = serviceOf(catalog, i.service_id);
  return s ? serviceName(s, catalog, lang) : "—";
}

/** "عصب‌کشی ۳۶ — جلسه ۲ از ۳" (the appointment card, the queue). */
export function itemTitle(t: T, catalog: CatalogInfo | null, i: TreatmentItemInfo, numbering: ToothNumbering, lang: Language): string {
  const teeth = teethLabel(i.teeth, numbering, lang);
  const session = sessionText(t, lang, i);
  return [`${itemService(catalog, i, lang)}${teeth ? ` ${teeth}` : ""}`, session].filter(Boolean).join(" — ");
}

/** AFN amount from minor units for an input box (no grouping). */
export function amountInput(minor: number): string {
  return minor ? String(minor / 100) : "";
}

export function parseAmount(s: string, latin: (x: string) => string): number | null {
  const n = Number(latin(s.trim() || "0").replace(/[,٬\s]/g, ""));
  return Number.isFinite(n) && n >= 0 && n <= 100_000_000 ? Math.round(n * 100) : null;
}
