import { describe, expect, it } from "vitest";
import { addDays, daysBetween, fromCalendar, fromMinutes, inCalendar, isInMonth, monthRange, monthWeeks, nowMinutes, shiftMonth, todayIso, toMinutes, weekDays, weekdayIndex, weekStart } from "../calendar";

describe("calendar helpers", () => {
  it("the Afghan week starts on Saturday", () => {
    expect(weekdayIndex("2026-10-03")).toBe(0); // Saturday
    expect(weekdayIndex("2026-10-04")).toBe(1);
    expect(weekdayIndex("2026-10-09")).toBe(6); // Friday
    expect(weekStart("2026-10-08")).toBe("2026-10-03");
    expect(weekStart("2026-10-03")).toBe("2026-10-03");
    expect(weekDays("2026-10-06")).toEqual(["2026-10-03", "2026-10-04", "2026-10-05", "2026-10-06", "2026-10-07", "2026-10-08", "2026-10-09"]);
  });

  it("adds days across month and year ends", () => {
    expect(addDays("2026-12-31", 1)).toBe("2027-01-01");
    expect(addDays("2028-02-28", 1)).toBe("2028-02-29");
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
    expect(daysBetween("2026-10-03", "2026-10-10")).toBe(7);
  });

  it("converts to the Solar Hijri calendar and back", () => {
    // 1 Hamal 1403 = 20 March 2024 (the same pair the Core's tests use).
    expect(inCalendar("2024-03-20", "shamsi")).toEqual({ year: 1403, month: 1, day: 1 });
    expect(fromCalendar(1403, 1, 1, "shamsi")).toBe("2024-03-20");
    expect(inCalendar("2026-10-03", "gregorian")).toEqual({ year: 2026, month: 10, day: 3 });
    // Round trip over several years.
    for (let i = 0; i < 2000; i += 37) {
      const iso = addDays("2020-01-01", i);
      const j = inCalendar(iso, "shamsi");
      expect(fromCalendar(j.year, j.month, j.day, "shamsi")).toBe(iso);
    }
  });

  it("rejects days that do not exist", () => {
    expect(fromCalendar(1403, 7, 31, "shamsi")).toBeNull(); // Mizan has 30 days
    expect(fromCalendar(1403, 6, 31, "shamsi")).toBe("2024-09-21"); // Sunbula has 31
    expect(fromCalendar(2026, 2, 30, "gregorian")).toBeNull();
    expect(fromCalendar(2026, 13, 1, "gregorian")).toBeNull();
    expect(fromCalendar(2026, 1, 0, "gregorian")).toBeNull();
  });

  it("cuts months by the clinic's calendar", () => {
    const m = monthRange("2024-03-25", "shamsi"); // inside Hamal 1403
    expect(m).toEqual({ from: "2024-03-20", to: "2024-04-19", year: 1403, month: 1 });
    expect(monthRange("2026-02-10", "gregorian")).toMatchObject({ from: "2026-02-01", to: "2026-02-28" });
    expect(shiftMonth("2026-01-31", "gregorian", 1)).toBe("2026-02-28");
    expect(inCalendar(shiftMonth("2024-03-25", "shamsi", 1), "shamsi")).toMatchObject({ year: 1403, month: 2 });
  });

  it("builds full Saturday-first week rows for the month grid", () => {
    const weeks = monthWeeks("2026-10-15", "gregorian");
    expect(weeks.every((w) => w.length === 7)).toBe(true);
    expect(weeks[0][0]).toBe("2026-09-26"); // Saturday before 1 Oct (a Thursday)
    expect(weeks.flat()).toContain("2026-10-31");
    expect(isInMonth("2026-09-30", "2026-10-15", "gregorian")).toBe(false);
    expect(isInMonth("2026-10-31", "2026-10-15", "gregorian")).toBe(true);
  });

  it("tells today's date and clock in Kabul time, not the computer's", () => {
    // 20:00 UTC on 2 Oct is already 00:30 on 3 Oct in Kabul (UTC+04:30).
    const at = new Date("2026-10-02T20:00:00Z");
    expect(todayIso(at)).toBe("2026-10-03");
    expect(nowMinutes(at)).toBe(30);
  });

  it("converts times of day", () => {
    expect(toMinutes("09:30")).toBe(570);
    expect(fromMinutes(570)).toBe("09:30");
    expect(fromMinutes(1440)).toBe("24:00");
  });
});
