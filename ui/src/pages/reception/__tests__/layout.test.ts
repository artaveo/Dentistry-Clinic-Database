import { describe, expect, it } from "vitest";
import { layoutLanes } from "../layout";

describe("calendar lanes", () => {
  it("gives a lone appointment the whole column", () => {
    expect(layoutLanes([{ id: "a", start: 540, end: 570 }])).toEqual([{ id: "a", start: 540, end: 570, lane: 0, lanes: 1 }]);
  });

  it("puts overlapping appointments side by side and back-to-back ones apart", () => {
    const r = layoutLanes([
      { id: "a", start: 540, end: 600 },
      { id: "b", start: 570, end: 630 },
      { id: "c", start: 630, end: 660 }, // starts exactly when b ends: own cluster, full width
    ]);
    const by = Object.fromEntries(r.map((p) => [p.id, p]));
    expect([by.a.lane, by.b.lane]).toEqual([0, 1]);
    expect([by.a.lanes, by.b.lanes]).toEqual([2, 2]);
    expect([by.c.lane, by.c.lanes]).toEqual([0, 1]);
  });

  it("reuses a freed lane inside one cluster", () => {
    const r = layoutLanes([
      { id: "a", start: 0, end: 100 },
      { id: "b", start: 10, end: 50 },
      { id: "c", start: 60, end: 90 }, // lane 1 is free again after b
    ]);
    const by = Object.fromEntries(r.map((p) => [p.id, p]));
    expect(by.c.lane).toBe(1);
    expect(by.a.lanes).toBe(2);
  });
});
