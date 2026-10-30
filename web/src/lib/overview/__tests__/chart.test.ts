import { describe, expect, it } from "vitest";

import { axisOf, daysAgoOf, roundTicks, slotsOf } from "../chart";
import { seriesOf } from "./series";

describe("roundTicks", () => {
  it("keeps the largest d × 10ⁿ under the maximum, and its half", () => {
    expect(roundTicks(12_400)).toEqual([10_000, 5_000]);
  });

  // Mutation: the 60 % rule dropped leaves 1 000 under a bar of 1 900.
  it("falls back to a half step when d × 10ⁿ leaves the top bar too high", () => {
    expect(roundTicks(1_900)).toEqual([1_500, 750]);
  });

  it("draws no mark on an empty series", () => {
    expect(roundTicks(0)).toEqual([]);
  });
});

describe("slotsOf", () => {
  const asOf = "2026-10-20T14:00:00Z";

  // Mutation: a slot's width kept apart from the response — bars of 6 h drawn on hourly slots.
  it("lays the slots of the response's grid, from `from` to `asOf` at `interval`", () => {
    const counts = (["5m", "1h", "6h", "1d"] as const).map((step) => slotsOf(seriesOf(step, asOf, []), []).length);

    expect(counts).toEqual([12, 24, 28, 30]);
    expect(slotsOf(seriesOf("6h", asOf, []), [])[0]?.from).toEqual(new Date("2026-10-13T14:00:00Z"));
  });

  it("has no slot before the first fact", () => {
    expect(slotsOf({ from: null, asOf: null, interval: "1h" }, [])).toEqual([]);
  });

  it("sums a slot's bars across protocols", () => {
    const slots = slotsOf(seriesOf("1h", asOf, []), [
      { from: "2026-10-20T13:00:00Z", value: "10" },
      { from: "2026-10-20T13:00:00Z", value: "5.5" },
    ]);
    expect(slots[23]).toEqual({
      from: new Date("2026-10-20T13:00:00Z"),
      to: new Date(asOf),
      value: 15.5,
    });
  });

  // Mutation: a bar placed by its index, not its `from`, closes the hole on the left.
  it("leaves the slot of a missing bar empty", () => {
    const slots = slotsOf(seriesOf("1d", asOf, []), [{ from: "2026-10-19T14:00:00Z", value: "7" }]);
    expect(slots[0]?.value).toBeNull();
    expect(slots[29]?.value).toBe(7);
  });

  // Mutation: the index floored — a bar off the grid counted in the slot it overlaps.
  it("puts a bar off the grid in no slot", () => {
    const slots = slotsOf(seriesOf("1d", asOf, []), [{ from: "2026-10-19T15:00:00Z", value: "7" }]);
    expect(slots.every((s) => s.value === null)).toBe(true);
  });

  // Mutation: a null summed as 0 — a bar nothing valued drawn as one at zero.
  it("sums the valued bars of a slot, an unvalued one left out", () => {
    const slots = slotsOf(seriesOf("1h", asOf, []), [
      { from: "2026-10-20T13:00:00Z", value: "2" },
      { from: "2026-10-20T13:00:00Z", value: null },
      { from: "2026-10-20T12:00:00Z", value: null },
    ]);
    expect(slots[23]?.value).toBe(2);
    expect(slots[22]?.value).toBeNull();
  });
});

describe("daysAgoOf", () => {
  // Mutation: the slot's index taken for its day — the 7 days' slices six days apart each.
  it("counts the 24-hour slices before the last slot a slot falls in", () => {
    const week = slotsOf(seriesOf("6h", "2026-10-24T12:00:00Z", []), []);
    const month = slotsOf(seriesOf("1d", "2026-10-24T12:00:00Z", []), []);
    expect([0, 3, 4, 23, 24, 27].map((i) => daysAgoOf(week, i))).toEqual([6, 6, 5, 1, 0, 0]);
    expect([0, 29].map((i) => daysAgoOf(month, i))).toEqual([29, 0]);
  });
});

describe("axisOf", () => {
  const hours = slotsOf(seriesOf("1h", "2026-10-24T12:00:00Z", []), []);

  // Mutation: the last slot left to the step — the axis would end hours before `asOf`.
  it("labels every sixth hour, and always the last one in UTC", () => {
    expect(axisOf("24h", hours, String)).toEqual(["12:00", "18:00", "00:00", "06:00", "11:00 UTC"]);
  });

  // Mutation: the hour alone named — the last hour's labels all "13:00", or one per tranche.
  it("labels the last hour every 15 minutes, to the minute, and its last tranche in UTC", () => {
    const tranches = slotsOf(seriesOf("5m", "2026-10-20T14:05:00Z", []), []);
    expect(axisOf("1h", tranches, String)).toEqual(["13:05", "13:20", "13:35", "13:50", "14:00 UTC"]);
  });

  // Mutation: the 30 days' rule over the 7 days' slices — today named twice.
  it("names each of the 7 days once, by how long ago it was", () => {
    const slices = slotsOf(seriesOf("6h", "2026-10-24T12:00:00Z", []), []);
    expect(axisOf("7d", slices, (d) => `J-${d}`)).toEqual(["J-6", "J-5", "J-4", "J-3", "J-2", "J-1", "J-0"]);
  });
});
