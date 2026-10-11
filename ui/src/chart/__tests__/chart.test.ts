import { describe, expect, it } from "vitest";
import { layout } from "../geometry";
import { itemState, scopeFits, viewForSymbol, viewShows } from "../model";
import { palmer, surfacesText, teethLabel, toothLabel, universal } from "../numbering";

describe("tooth numbering (ADR-14: FDI stored, Universal/Palmer shown)", () => {
  it("maps FDI to Universal for adult and baby teeth", () => {
    expect([18, 11, 21, 28, 38, 31, 41, 48].map(universal)).toEqual(["1", "8", "9", "16", "17", "24", "25", "32"]);
    expect([55, 51, 61, 65, 75, 71, 81, 85].map(universal)).toEqual(["A", "E", "F", "J", "K", "O", "P", "T"]);
  });

  it("marks the Palmer quadrant around the tooth number", () => {
    expect([16, 21, 33, 44, 55].map(palmer)).toEqual(["6┘", "└1", "┌3", "4┐", "E┘"]);
  });

  it("shows the interface's digits and joins neighbours in one quadrant", () => {
    expect(toothLabel(36, "fdi", "fa")).toBe("۳۶");
    expect(toothLabel(36, "fdi", "en")).toBe("36");
    expect(teethLabel([26, 24, 25], "fdi", "en")).toBe("24–26");
    expect(teethLabel([11, 21], "fdi", "en")).toBe("11, 21");
    expect(teethLabel([17, 18, 21], "fdi", "fa")).toBe("۱۷، ۱۸، ۲۱");
  });

  it("names front-tooth and upper-palatal surfaces as the doctor reads them", () => {
    expect(surfacesText("MOD", 36)).toBe("MOD");
    expect(surfacesText("OB", 11)).toBe("IF");
    expect(surfacesText("L", 16)).toBe("P");
    expect(surfacesText("L", 46)).toBe("L");
  });
});

describe("chart model (M5)", () => {
  it("turns item statuses into chart colours", () => {
    expect(itemState("planned")).toBe("planned");
    expect(itemState("in_progress")).toBe("progress");
    expect(itemState("completed")).toBe("done");
  });

  it("shows each specialty view only its own work", () => {
    expect(viewShows("general", "surface")).toBe(true);
    expect(viewShows("ortho", "bracket")).toBe(true);
    expect(viewShows("ortho", "crown")).toBe(false);
    expect(viewShows("implant", "implant")).toBe(true);
    expect(viewShows("prosth", "bridge")).toBe(true);
    expect(viewShows("prosth", "root_canal")).toBe(false);
    expect(viewForSymbol("implant_crown")).toBe("implant");
    expect(viewForSymbol("veneer")).toBe("prosth");
    expect(viewForSymbol("surface")).toBe("general");
  });

  it("checks a service's tooth scope against the selection", () => {
    expect(scopeFits("tooth", [])).toBe(false);
    expect(scopeFits("tooth", [36])).toBe(true);
    expect(scopeFits("teeth", [36])).toBe(false);
    expect(scopeFits("teeth", [35, 36, 37])).toBe(true);
    expect(scopeFits("mouth", [])).toBe(true);
  });

  it("lays out both jaws for every dentition, inside the chart width", () => {
    for (const d of ["permanent", "mixed", "primary"] as const) {
      const { upper, lower } = layout(d, []);
      expect(upper.length).toBeGreaterThan(0);
      expect(lower.length).toBe(upper.length);
      const last = upper[upper.length - 1];
      expect(upper[0].x).toBeGreaterThanOrEqual(0);
      expect(last.x + last.w).toBeLessThanOrEqual(1080);
    }
    expect(layout("permanent", []).upper.map((s) => s.fdi)).toEqual([18, 17, 16, 15, 14, 13, 12, 11, 21, 22, 23, 24, 25, 26, 27, 28]);
  });
});
