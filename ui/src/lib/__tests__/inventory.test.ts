import { describe, expect, it } from "vitest";
import { formatQty, parseQty, parseTeeth, qtyInput, qtyRule, teethText } from "../inventory";

describe("quantities typed with up to two decimals (stored as hundredths of a unit)", () => {
  it("reads Latin and Persian digits and decimals", () => {
    expect(parseQty("2")).toBe(200);
    expect(parseQty("۲٫۵")).toBe(250);
    expect(parseQty("0.25")).toBe(25);
    expect(parseQty("1,000")).toBe(100000);
  });
  it("refuses what is not a number", () => {
    for (const s of ["", "abc", "1.234", "-1", "2x"]) expect(parseQty(s)).toBeNull();
  });
  it("needs more than zero unless zero is allowed", () => {
    expect(qtyRule("")).toBe("rule.required");
    expect(qtyRule("0")).toBe("rule.stock_quantity");
    expect(qtyRule("x")).toBe("rule.stock_quantity");
    expect(qtyRule("0", true)).toBeNull();
    expect(qtyRule("2.5")).toBeNull();
  });
  it("shows a stored quantity as the number a person edits and reads", () => {
    expect(qtyInput(250)).toBe("2.5");
    expect(qtyInput(300)).toBe("3");
    expect(formatQty(250, "en")).toBe("2.5");
    expect(formatQty(100000, "en")).toBe("1,000");
    expect(formatQty(250, "fa")).toBe("۲٫۵");
    expect(formatQty(300, "fa")).toBe("۳");
  });
});

describe("tooth numbers typed for a lab case (FDI)", () => {
  it("reads numbers separated by spaces or commas, in any digits", () => {
    expect(parseTeeth("36, 37 ۳۸")).toEqual([36, 37, 38]);
    expect(parseTeeth("14،15")).toEqual([14, 15]);
    expect(parseTeeth("")).toEqual([]);
    expect(parseTeeth("36 36")).toEqual([36]);
  });
  it("refuses what is not a tooth", () => {
    for (const s of ["9", "19", "99", "ab", "36 90"]) expect(parseTeeth(s)).toBeNull();
  });
  it("writes them back as a list", () => {
    expect(teethText([14, 15, 16])).toBe("14, 15, 16");
  });
});
