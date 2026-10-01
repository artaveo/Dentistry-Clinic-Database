import { describe, expect, it } from "vitest";
import { dirOf, translate } from "../index";

describe("translate", () => {
  it("resolves a known key per language", () => {
    expect(translate("fa", "app.name")).toBe("آرتاویو دنتال");
    expect(translate("en", "app.name")).toBe("Artaveo Dental");
    expect(translate("ps", "app.name")).toBe("ارتاویو ډینټل");
  });

  it("falls back to the key itself when nothing matches (never a blank UI string)", () => {
    expect(translate("en", "no.such.key")).toBe("no.such.key");
  });
});

describe("dirOf", () => {
  it("is RTL for Dari and Pashto, LTR for English", () => {
    expect(dirOf("fa")).toBe("rtl");
    expect(dirOf("ps")).toBe("rtl");
    expect(dirOf("en")).toBe("ltr");
  });
});
