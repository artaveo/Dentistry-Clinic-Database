import { describe, expect, it } from "vitest";
import { SURFACE, brandPalette, contrast } from "../color";

// Whatever colour a clinic picks, the UI stays readable (WCAG AA, design-direction §3-4).
const picks = ["#0e7490", "#f59e0b", "#fde047", "#ffffff", "#000000", "#1e1b4b", "#be185d", "#22c55e", "#64748b"];

describe("brandPalette", () => {
  for (const hex of picks) {
    for (const theme of ["light", "dark"] as const) {
      it(`${hex} in ${theme} meets AA`, () => {
        const p = brandPalette(hex, theme);
        expect(contrast(p.onAccent, p.accent)).toBeGreaterThanOrEqual(4.5);
        expect(contrast(p.accentText, SURFACE[theme])).toBeGreaterThanOrEqual(4.5);
      });
    }
  }

  it("keeps the clinic's own colour when it is already accessible", () => {
    expect(brandPalette("#0e7490", "light").accent).toBe("#0e7490");
    expect(brandPalette("#0e7490", "light").onAccent).toBe("#ffffff");
  });

  it("puts dark text on a light clinic colour instead of failing white", () => {
    expect(brandPalette("#f59e0b", "light").onAccent).toBe("#0b1220");
  });
});
