import { describe, expect, it } from "vitest";
import { digits, formatDate, formatDateTime, formatHour, formatTime, latinDigits, to12, to24 } from "../dates";

describe("digits", () => {
  it("converts Latin digits to Persian/Pashto for fa and ps, not en", () => {
    expect(digits(1405, "fa")).toBe("۱۴۰۵");
    expect(digits(1405, "ps")).toBe("۱۴۰۵");
    expect(digits(1405, "en")).toBe("1405");
  });
  it("reads Persian and Arabic-Indic digits back as Latin (ADR-09)", () => {
    expect(latinDigits("۱۲:۳۰")).toBe("12:30");
    expect(latinDigits("٤٥")).toBe("45");
  });
});

describe("12-hour clock (OF-007)", () => {
  it("converts both ways around midnight and noon", () => {
    expect(to12(0)).toEqual({ hour: 12, pm: false });
    expect(to12(12)).toEqual({ hour: 12, pm: true });
    expect(to12(19)).toEqual({ hour: 7, pm: true });
    expect(to24(12, false)).toBe(0);
    expect(to24(12, true)).toBe(12);
    expect(to24(7, true)).toBe(19);
  });
  it("uses ق.ظ/ب.ظ, غ.م/غ.و and AM/PM", () => {
    expect(formatHour(19, "fa")).toBe("۷:۰۰ ب.ظ");
    expect(formatTime("08:05", "fa")).toBe("۸:۰۵ ق.ظ");
    expect(formatTime("13:30", "ps")).toBe("۱:۳۰ غ.و");
    expect(formatTime("00:15", "en")).toBe("12:15 AM");
  });
});

describe("formatDateTime", () => {
  // 2026-10-01T12:00:00Z is 2026-10-01 16:30 in Kabul (+04:30) = 9 Mizan 1405
  // (matches core/src/calendar.rs's `today_in_roadmap` test, ADR-21).
  const iso = "2026-10-01T12:00:00Z";

  it("is one natural phrase: date, then «ساعت» and a 12-hour time (OF-006)", () => {
    expect(formatDateTime(iso, "fa")).toBe("۹ میزان ۱۴۰۵، ساعت ۴:۳۰ ب.ظ");
    expect(formatDateTime(iso, "en")).toBe("9 Mizan 1405, 4:30 PM");
    expect(formatDateTime("2026-10-01T20:44:00Z", "fa")).toBe("۱۰ میزان ۱۴۰۵، ساعت ۱:۱۴ ق.ظ");
  });

  it("switches to the Gregorian calendar when the clinic chose it", () => {
    expect(formatDate(iso, "en", "gregorian")).toBe("1 October 2026");
  });

  it("falls back to an em dash for a missing date", () => {
    expect(formatDateTime(null, "en")).toBe("—");
    expect(formatDateTime(undefined, "fa")).toBe("—");
  });
});
