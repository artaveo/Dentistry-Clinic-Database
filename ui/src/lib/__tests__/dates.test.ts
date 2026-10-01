import { describe, expect, it } from "vitest";
import { digits, formatDateTime } from "../dates";

describe("digits", () => {
  it("converts Latin digits to Persian/Pashto for fa and ps, not en", () => {
    expect(digits(1405, "fa")).toBe("۱۴۰۵");
    expect(digits(1405, "ps")).toBe("۱۴۰۵");
    expect(digits(1405, "en")).toBe("1405");
  });
});

describe("formatDateTime", () => {
  // 2026-10-01T12:00:00Z is 2026-10-01 16:30 in Kabul (+04:30) = 9 Mizan 1405
  // (matches core/src/calendar.rs's `today_in_roadmap` test, ADR-21).
  const iso = "2026-10-01T12:00:00Z";

  it("defaults to the Shamsi calendar with Afghan month names", () => {
    expect(formatDateTime(iso, "fa")).toContain("میزان");
    expect(formatDateTime(iso, "fa")).toContain("۱۴۰۵");
  });

  it("switches to the Gregorian calendar when the clinic chose it", () => {
    const s = formatDateTime(iso, "en", "gregorian");
    expect(s).toContain("October");
    expect(s).toContain("2026");
  });

  it("falls back to an em dash for a missing date", () => {
    expect(formatDateTime(null, "en")).toBe("—");
    expect(formatDateTime(undefined, "fa")).toBe("—");
  });
});
