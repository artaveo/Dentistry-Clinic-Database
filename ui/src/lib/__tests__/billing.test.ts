import { describe, expect, it } from "vitest";
import { afnInput, amountRule, parseAfn } from "../billing";

describe("amounts typed in afghani (ADR-06: stored ×100)", () => {
  it("reads Latin and Persian digits, separators and decimals", () => {
    expect(parseAfn("2500")).toBe(250000);
    expect(parseAfn("۲۵۰۰")).toBe(250000);
    expect(parseAfn("2,500")).toBe(250000);
    expect(parseAfn("۲٬۵۰۰٫۵")).toBe(250050);
    expect(parseAfn("12.25")).toBe(1225);
  });
  it("refuses what is not a number", () => {
    for (const s of ["", "abc", "1.234", "-5", "1e3", "12,5x"]) expect(parseAfn(s)).toBeNull();
  });
  it("shows a stored amount as the plain number a person edits", () => {
    expect(afnInput(250000)).toBe("2500");
    expect(afnInput(250050)).toBe("2500.5");
    expect(afnInput(1225)).toBe("12.25");
  });
  it("needs an amount above zero unless zero is allowed", () => {
    expect(amountRule("")).toBe("rule.required");
    expect(amountRule("0")).toBe("rule.billing_amount");
    expect(amountRule("0", true)).toBeNull();
    expect(amountRule("5")).toBeNull();
  });
});
