import { describe, expect, it } from "vitest";
import { formatAmount } from "../money";

describe("formatAmount (ADR-06: integer minor units, ×100)", () => {
  it("formats whole AFN amounts in English with Latin digits", () => {
    expect(formatAmount(250000, "en")).toBe("2,500 AFN");
  });

  it("formats whole AFN amounts in Dari with Persian digits and separators", () => {
    expect(formatAmount(250000, "fa")).toBe("۲٬۵۰۰ افغانی");
  });

  it("keeps cents when the amount is not a whole AFN", () => {
    expect(formatAmount(250050, "en")).toBe("2,500.50 AFN");
  });

  it("rounds floating point drift before formatting", () => {
    expect(formatAmount(99.999999, "en")).toBe("1 AFN");
  });
});
