// Phase 9: what the reports screen knows about each report (group, filters, icon) and how a typed cell of a
// report row is shown. The Core sends values, never text, so digits, calendar and language follow the user.
import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import {
  Activity, Ban, BadgeDollarSign, BarChart3, CalendarClock, CalendarDays, ClipboardList, Coins, HandCoins, Hourglass, Layers, Megaphone, PackageCheck, PackageMinus,
  PackageX, Percent, Receipt, ReceiptText, Scale, ShoppingCart, Smile, Stethoscope, Truck, UserPlus, Wallet, Wrench,
} from "lucide-react";
import type { CalendarSystem, Language, ReportCell, ReportKind } from "../../../shared/ts/contract";
import { addDays, fromCalendar, inCalendar, monthRange, shiftMonth, todayIso } from "./calendar";
import { digits, formatDate, monthName } from "./dates";
import { tr } from "./medical";
import { formatAmount } from "./money";

export type ReportGroup = "financial" | "clinical" | "inventory";
export type ReportFilter = "doctor" | "service";
/** `range`: a first and a last day; `asOf`: only the last day matters; `now`: the state today, no period. */
export type ReportPeriod = "range" | "asOf" | "now";

export type ReportMeta = { kind: ReportKind; group: ReportGroup; icon: LucideIcon; period: ReportPeriod; filters: ReportFilter[] };

export const REPORTS: ReportMeta[] = [
  { kind: "daily_revenue", group: "financial", icon: CalendarDays, period: "range", filters: [] },
  { kind: "monthly_revenue", group: "financial", icon: BarChart3, period: "range", filters: [] },
  { kind: "payments", group: "financial", icon: Receipt, period: "range", filters: [] },
  { kind: "outstanding", group: "financial", icon: Hourglass, period: "now", filters: [] },
  { kind: "discounts", group: "financial", icon: Percent, period: "range", filters: [] },
  { kind: "refunds", group: "financial", icon: Ban, period: "range", filters: [] },
  { kind: "doctor_revenue", group: "financial", icon: Stethoscope, period: "range", filters: ["doctor"] },
  { kind: "doctor_commission", group: "financial", icon: HandCoins, period: "range", filters: ["doctor"] },
  { kind: "service_revenue", group: "financial", icon: BadgeDollarSign, period: "range", filters: ["service", "doctor"] },
  { kind: "expenses", group: "financial", icon: Wallet, period: "range", filters: [] },
  { kind: "profit_loss", group: "financial", icon: Scale, period: "range", filters: [] },
  { kind: "cash_closings", group: "financial", icon: Coins, period: "range", filters: [] },
  { kind: "patients", group: "clinical", icon: UserPlus, period: "range", filters: [] },
  { kind: "treatments", group: "clinical", icon: ClipboardList, period: "range", filters: ["doctor", "service"] },
  { kind: "procedures", group: "clinical", icon: Layers, period: "range", filters: ["doctor", "service"] },
  { kind: "tooth_conditions", group: "clinical", icon: Smile, period: "range", filters: [] },
  { kind: "doctor_activity", group: "clinical", icon: Activity, period: "range", filters: ["doctor"] },
  { kind: "follow_ups", group: "clinical", icon: CalendarClock, period: "range", filters: [] },
  { kind: "recall_list", group: "clinical", icon: ReceiptText, period: "asOf", filters: [] },
  { kind: "referral_sources", group: "clinical", icon: Megaphone, period: "range", filters: [] },
  { kind: "stock", group: "inventory", icon: PackageCheck, period: "now", filters: [] },
  { kind: "low_stock", group: "inventory", icon: PackageMinus, period: "now", filters: [] },
  { kind: "purchases", group: "inventory", icon: ShoppingCart, period: "range", filters: [] },
  { kind: "consumption", group: "inventory", icon: Wrench, period: "range", filters: [] },
  { kind: "expiry", group: "inventory", icon: PackageX, period: "asOf", filters: [] },
  { kind: "supplier_balance", group: "inventory", icon: Truck, period: "now", filters: [] },
];

export const GROUPS: ReportGroup[] = ["financial", "clinical", "inventory"];

export type PeriodPreset = "today" | "week" | "month" | "lastMonth" | "year";
export const PRESETS: PeriodPreset[] = ["today", "week", "month", "lastMonth", "year"];

/** First and last day of a quick choice, in the clinic's calendar. */
export function presetRange(preset: PeriodPreset, calendar: CalendarSystem, today: string = todayIso()): { from: string; to: string } {
  switch (preset) {
    case "today":
      return { from: today, to: today };
    case "week":
      return { from: addDays(today, -6), to: today };
    case "month": {
      const m = monthRange(today, calendar);
      return { from: m.from, to: m.to > today ? today : m.to };
    }
    case "lastMonth": {
      const m = monthRange(shiftMonth(today, calendar, -1), calendar);
      return { from: m.from, to: m.to };
    }
    case "year": {
      const y = inCalendar(today, calendar).year;
      return { from: fromCalendar(y, 1, 1, calendar) ?? addDays(today, -365), to: today };
    }
  }
}

const group = (n: number, lang: Language) => digits(n.toLocaleString("en-US"), lang);

/** `1405-03` → «حمل ۱۴۰۵»; a Gregorian clinic gets «March 2026». */
function monthLabel(key: string, calendar: CalendarSystem, lang: Language): string {
  const [y, m] = key.split("-").map(Number);
  if (!y || !m) return key;
  return `${monthName(calendar, m, lang)} ${digits(y, lang)}`;
}

/** Numbers and money line up on the right; everything else on the reading side. */
export function isNumeric(c: ReportCell): boolean {
  return c.type === "int" || c.type === "money" || c.type === "qty" || c.type === "percent";
}

export function renderCell(c: ReportCell, lang: Language, calendar: CalendarSystem, t: (k: string) => string): ReactNode {
  switch (c.type) {
    case "empty":
      return <span className="subtle">—</span>;
    case "text":
      return c.value;
    case "code":
      return <bdi className="ltr">{c.value}</bdi>;
    case "int":
      return group(c.value, lang);
    case "money":
      return formatAmount(c.value, lang);
    case "date":
      return formatDate(c.value, lang, calendar);
    case "month":
      return monthLabel(c.value, calendar, lang);
    case "name":
      return tr(c.value, lang);
    case "tag": {
      const k = `reports.val.${c.value}`;
      const v = t(k);
      return v === k ? c.value : v;
    }
    case "qty": {
      const whole = c.value / 100;
      return digits(whole.toLocaleString("en-US", { maximumFractionDigits: 2 }), lang).replace(".", lang === "en" ? "." : "٫");
    }
    case "percent":
      return `${digits((c.value / 100).toLocaleString("en-US", { maximumFractionDigits: 1 }), lang)}${lang === "en" ? "%" : "٪"}`;
  }
}

/** The words of every `Tag` value, for the Excel export (the Core cannot translate). */
export function tagLabels(t: (k: string) => string, keys: string[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const k of keys) {
    const text = t(`reports.val.${k}`);
    if (text !== `reports.val.${k}`) out[k] = text;
  }
  return out;
}

export const TAG_VALUES = ["received", "void", "pending", "contacted", "booked", "done", "dismissed", "checkup", "cleaning", "follow_up", "no_show", "other", "expired", "expiring", "lab", "supplier", "unknown"];
