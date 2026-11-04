import { describe, expect, it } from "vitest";
import { GROUPS, PRESETS, REPORTS, TAG_VALUES, isNumeric, presetRange, tagLabels } from "../reports";

describe("report catalog", () => {
  it("has every report once, in a known group, with unique kinds", () => {
    const kinds = REPORTS.map((r) => r.kind);
    expect(new Set(kinds).size).toBe(kinds.length);
    expect(kinds.length).toBe(26);
    for (const r of REPORTS) expect(GROUPS).toContain(r.group);
  });

  it("only offers a doctor or service filter where the Core uses it", () => {
    const withDoctor = REPORTS.filter((r) => r.filters.includes("doctor")).map((r) => r.kind);
    expect(withDoctor).toEqual(["doctor_revenue", "doctor_commission", "service_revenue", "treatments", "procedures", "doctor_activity"]);
  });
});

describe("period presets", () => {
  it("run from the first to the last day, never past today", () => {
    for (const p of PRESETS) {
      const r = presetRange(p, "gregorian", "2026-11-04");
      expect(r.from <= r.to, p).toBe(true);
    }
    expect(presetRange("today", "gregorian", "2026-11-04")).toEqual({ from: "2026-11-04", to: "2026-11-04" });
    expect(presetRange("week", "gregorian", "2026-11-04")).toEqual({ from: "2026-10-29", to: "2026-11-04" });
    expect(presetRange("month", "gregorian", "2026-11-04")).toEqual({ from: "2026-11-01", to: "2026-11-04" });
    expect(presetRange("lastMonth", "gregorian", "2026-11-04")).toEqual({ from: "2026-10-01", to: "2026-10-31" });
    expect(presetRange("year", "gregorian", "2026-11-04")).toEqual({ from: "2026-01-01", to: "2026-11-04" });
  });

  it("follows the Shamsi calendar for a Shamsi clinic", () => {
    // 4 Nov 2026 is 13 Aban 1405; Aban starts on 23 Oct 2026.
    expect(presetRange("month", "shamsi", "2026-11-04")).toEqual({ from: "2026-10-23", to: "2026-11-04" });
    // Nowruz 1405 is 21 March 2026.
    expect(presetRange("year", "shamsi", "2026-11-04").from).toBe("2026-03-21");
  });
});

describe("cells", () => {
  it("lines up numbers and money on the right only", () => {
    expect(isNumeric({ type: "money", value: 1 })).toBe(true);
    expect(isNumeric({ type: "int", value: 1 })).toBe(true);
    expect(isNumeric({ type: "text", value: "1" })).toBe(false);
    expect(isNumeric({ type: "code", value: "INV-1" })).toBe(false);
  });

  it("sends only the words it has for tag values", () => {
    const t = (k: string) => (k === "reports.val.void" ? "Void" : k);
    expect(tagLabels(t, TAG_VALUES)).toEqual({ void: "Void" });
  });
});
