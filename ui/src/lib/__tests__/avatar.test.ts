import { describe, expect, it } from "vitest";
import { CHILD_AGE, defaultArt, squareCrop } from "../avatar";

describe("defaultArt (OF-049/OF-050)", () => {
  it("draws adult patients by sex", () => {
    expect(defaultArt("patient", "male", 40)).toBe("patient-man");
    expect(defaultArt("patient", "female", 40)).toBe("patient-woman");
  });
  it("draws children under the child age as a boy or a girl", () => {
    expect(defaultArt("patient", "male", 7)).toBe("patient-boy");
    expect(defaultArt("patient", "female", 0)).toBe("patient-girl");
    expect(defaultArt("patient", "female", CHILD_AGE - 1)).toBe("patient-girl");
    expect(defaultArt("patient", "female", CHILD_AGE)).toBe("patient-woman");
  });
  it("treats an unknown age as an adult", () => {
    expect(defaultArt("patient", "male", null)).toBe("patient-man");
    expect(defaultArt("patient", "female")).toBe("patient-woman");
  });
  it("has no picture when the sex is unknown or for plain users", () => {
    expect(defaultArt("patient", null, 30)).toBeNull();
    expect(defaultArt("doctor", undefined)).toBeNull();
    expect(defaultArt("user", "male")).toBeNull();
  });
  it("draws doctors as doctors", () => {
    expect(defaultArt("doctor", "female", 8)).toBe("doctor-female");
    expect(defaultArt("doctor", "male")).toBe("doctor-male");
  });
});

describe("squareCrop", () => {
  it("centres landscape photos and keeps the top of portrait ones", () => {
    expect(squareCrop(400, 200)).toEqual({ sx: 100, sy: 0, side: 200 });
    expect(squareCrop(200, 400)).toEqual({ sx: 0, sy: 40, side: 200 });
    expect(squareCrop(300, 300)).toEqual({ sx: 0, sy: 0, side: 300 });
  });
});
