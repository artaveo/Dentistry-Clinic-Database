import { describe, expect, it } from "vitest";
import { v } from "../validation";

// OF-001/003: each mistake gets its own precise message before the button is pressed.
describe("username", () => {
  it("names the exact problem", () => {
    expect(v.username("")).toBe("rule.required");
    expect(v.username("احمد")).toBe("rule.username_latin");
    expect(v.username("dr ahmad")).toBe("rule.username_chars");
    expect(v.username("ab")).toBe("rule.username_length");
    expect(v.username("a".repeat(33))).toBe("rule.username_length");
    expect(v.username("dr.ahmad_1")).toBeNull();
  });
});

describe("password", () => {
  it("needs at least 8 characters, Persian digits included", () => {
    expect(v.password("1234567")).toBe("rule.password_too_short");
    expect(v.password("۱۲۳۴۵۶۷۸")).toBeNull();
  });
});

describe("phone", () => {
  it("is optional but must look like a number when given", () => {
    expect(v.phone("")).toBeNull();
    expect(v.phone("۰۷۰۰ ۱۲۳ ۴۵۶")).toBeNull();
    expect(v.phone("+93 700-123-456")).toBeNull();
    expect(v.phone("abc")).toBe("rule.phone");
  });
});

describe("intRange", () => {
  it("accepts Persian digits and rejects out-of-range values", () => {
    const c = v.intRange(1, 240, "rule.session_timeout_range");
    expect(c("۱۰")).toBeNull();
    expect(c("0")).toBe("rule.session_timeout_range");
    expect(c("")).toBe("rule.session_timeout_range");
  });
});
